#[cfg(feature = "updates")]
use semver::Version;
use {
    crate::{
        built_info,
        exports::runtime as rt,
        settings::{
            source::{
                self,
                github::{GitHubLatestRelease, GitHubReleaseAsset, GitHubSource},
                Source,
            },
            state::{BootstrapState, UpdatePreference},
            RemoteAssetForm,
            SourceKind,
        },
    },
    anyhow::{anyhow, Context},
    futures::future::TryFutureExt,
    std::{
        borrow::Cow,
        fmt,
        future::Future,
        sync::{LazyLock, RwLock},
        time::Duration,
    },
    taimi_hoard::str_opt,
    tokio::{runtime, time::timeout},
    url::Url,
};
#[cfg(feature = "extension-nexus")]
use nexus::addon::AddonVersion as NexusVersion;

pub const GIT_REF_BRANCH_PREFIX: &'static str = "refs/heads/";
pub const GIT_REF_TAG_PREFIX: &'static str = "refs/tags/";
pub const GIT_REF_RELEASE_PREFIX: &'static str = "refs/tags/v";
pub const CHANNEL_DL_PREFIX: &'static str = "chan/";
pub const CHANNEL_DEBUG: &'static str = "debug";
pub const CHANNEL_ALPHA: &'static str = "alpha";
pub const CHANNEL_BETA: &'static str = "beta";
pub const CHANNEL_PRERELEASE: &'static str = "rc";
pub const CHANNEL_RELEASE_NAME: &'static str = "release";
pub const DLL_NAME: &'static str = "TaimiHUD.dll";

pub struct ResolvedVersion {
    pub release: GitHubLatestRelease,
    #[cfg(feature = "updates")]
    pub version: Option<Version>,
}

pub fn addon_version<'a>() -> Cow<'a, str> {
    let version = BootstrapState::read_with(|s| str_opt(s.update_override_version.clone())).map(Cow::Owned);
    version.unwrap_or_else(addon_version_build)
}
pub fn addon_version_build<'a>() -> Cow<'a, str> {
    match option_env!("ADDON_VERSION") {
        Some(v) => Cow::Borrowed(v),
        #[cfg(feature = "updates")]
        None => Cow::Owned(CRATE_SEMVER.to_string()),
        #[cfg(not(feature = "updates"))]
        None => Cow::Borrowed(rt::CRATE_VERSION),
    }
}

/// NOTE: called from within [BootstrapState] mutex, must not use `addon_version`!
#[cfg(taimi_has = "url-update-base")]
#[allow(dead_code)]
pub fn format_direct_url(channel: Option<&str>, version: Option<&str>) -> String {
    let channel = match channel.map(Cow::Borrowed) {
        #[cfg(taimi_has = "url-update-direct")]
        None if version.is_none() => return env!("ADDON_URL_UPDATE_DIRECT").into(),
        Some(c) => Some(c),
        None => crate_channel(),
    }
    .unwrap_or(Cow::Borrowed(CHANNEL_RELEASE_NAME));
    let update_base = env!("ADDON_URL_UPDATE_BASE");
    let version = version.map(Cow::Borrowed).unwrap_or_else(addon_version_build);
    let package = env!("CARGO_PKG_NAME");
    let ext = match () {
        #[cfg(windows)]
        _ => ".dll",
    };
    format!("{update_base}/{channel}/{package}{ext}?v={version}")
}

type VersionParts = (u64, u64, u64, &'static str, &'static str);
#[cfg(taimi_has = "version")]
pub static CRATE_SEMVER_PARTS: LazyLock<VersionParts> = LazyLock::new(|| {
    let (major, minor, patch, pre, build) = (
        option_env!("ADDON_VERSION_MAJOR").unwrap_or(env!("CARGO_PKG_VERSION_MAJOR")),
        option_env!("ADDON_VERSION_MINOR").unwrap_or(env!("CARGO_PKG_VERSION_MINOR")),
        option_env!("ADDON_VERSION_PATCH").unwrap_or(env!("CARGO_PKG_VERSION_PATCH")),
        option_env!("ADDON_VERSION_PRE").unwrap_or(env!("CARGO_PKG_VERSION_PRE")),
        option_env!("ADDON_VERSION_BUILD").unwrap_or(""),
    );
    (
        major.parse().unwrap_or_default(),
        minor.parse().unwrap_or_default(),
        patch.parse().unwrap_or_default(),
        pre,
        build,
    )
});
#[cfg(all(taimi_has = "version", feature = "extension-nexus"))]
pub static CRATE_ADDONAPI_VERSION: LazyLock<NexusVersion> = LazyLock::new(|| {
    let (major, minor, build, rev) = (
        option_env!("ADDONAPI_VERSION_MAJOR").unwrap_or(env!("CARGO_PKG_VERSION_MAJOR")),
        option_env!("ADDONAPI_VERSION_MINOR").unwrap_or(env!("CARGO_PKG_VERSION_MINOR")),
        option_env!("ADDONAPI_VERSION_BUILD").unwrap_or(env!("CARGO_PKG_VERSION_PATCH")),
        option_env!("ADDONAPI_VERSION_REVISION").unwrap_or("0"),
    );
    NexusVersion {
        major: major.parse().unwrap_or_default(),
        minor: minor.parse().unwrap_or_default(),
        build: build.parse().unwrap_or_default(),
        revision: rev.parse().unwrap_or_default(),
    }
});
#[cfg(all(taimi_has = "version", feature = "extension-nexus"))]
pub fn addonapi_version() -> NexusVersion {
    let version = OVERRIDE_VERSION.read().ok().map(|c| version_to_addonapi(&c));
    version.unwrap_or_else(|| CRATE_ADDONAPI_VERSION.clone())
}

#[cfg(feature = "updates")]
pub static CRATE_SEMVER: LazyLock<Version> = LazyLock::new(|| {
    let (major, minor, patch, pre, build) = *CRATE_SEMVER_PARTS;
    let mut version = Version::new(major, minor, patch);
    version.pre = semver::Prerelease::new(pre).unwrap_or_default();
    version.build = semver::BuildMetadata::new(build).unwrap_or_default();
    version
});
#[cfg(feature = "updates")]
pub fn crate_semver<'a>() -> Cow<'a, Version> {
    let version = OVERRIDE_VERSION.read().ok().map(|c| c.clone());
    let v = match version {
        None => None,
        Some(ref v) if v.major == 0 && v.minor == 0 && v.patch == 0 => None,
        Some(v) => Some(Cow::Owned(v)),
    };
    v.unwrap_or(Cow::Borrowed(&CRATE_SEMVER))
}
#[cfg(feature = "updates")]
fn parse_channel_override(channel: &String) -> Option<Cow<'_, str>> {
    match &channel[..] {
        CHANNEL_RELEASE_NAME | CHANNEL_DL_PREFIX => None,
        c if c.starts_with(CHANNEL_DL_PREFIX) => c.strip_prefix(CHANNEL_DL_PREFIX).map(Cow::Borrowed),
        c => Some(
            c.strip_prefix(CHANNEL_DL_PREFIX)
                .map(Cow::Borrowed)
                .unwrap_or_else(|| Cow::Owned(channel.clone())),
        ),
    }
}
pub fn crate_channel<'a>() -> Option<Cow<'a, str>> {
    #[cfg(feature = "updates")]
    {
        let channel = OVERRIDE_CHANNEL
            .write()
            .ok()
            .and_then(|c| str_opt(&*c).map(|c| parse_channel_override(c).map(Cow::into_owned)));
        if let Some(channel) = channel {
            return channel.map(Cow::Owned)
        }
    }
    crate_channel_build().map(Cow::Borrowed)
}
pub fn crate_channel_build() -> Option<&'static str> {
    #[allow(unreachable_patterns)]
    match option_env!("ADDON_VERSION_CHANNEL") {
        #[cfg(debug_assertions)]
        _ => Some(CHANNEL_DEBUG),
        Some("") => None,
        Some(c) => Some(c),
        #[cfg(feature = "updates")]
        None => version_channel(&CRATE_SEMVER),
        #[cfg(not(feature = "updates"))]
        None => Some("dev"),
    }
}
#[cfg(feature = "updates")]
static OVERRIDE_VERSION: RwLock<Version> = RwLock::new(Version::new(0, 0, 0));
#[cfg(feature = "updates")]
static OVERRIDE_CHANNEL: RwLock<String> = RwLock::new(String::new());
#[cfg(feature = "updates")]
pub fn try_override_version(version: &str) -> anyhow::Result<()> {
    let version = match str_opt(version) {
        Some(version) => Some(ResolvedVersion::parse_version_id(version)?),
        None => None,
    };
    Ok(override_version(version))
}
#[cfg(feature = "updates")]
pub fn override_version(version: Option<Version>) {
    BootstrapState::write_with(|s| {
        s.update_override_version = version.as_ref().map(ToString::to_string).unwrap_or_default();
    });
    let version = version.unwrap_or(Version::new(0, 0, 0));
    if let Ok(mut o) = OVERRIDE_VERSION.write() {
        *o = version;
    }
}
pub fn override_channel(channel: String) {
    if let Ok(mut o) = OVERRIDE_CHANNEL.write() {
        *o = channel.clone();
    }
    BootstrapState::write_with(|s| {
        s.update_override_channel = channel;
    });
}
/// for use by [BootstrapState] to synchronize config on launch
/// (avoiding deadlocks related to update operations is important)
#[cfg(feature = "updates")]
pub(crate) fn report_overrides(channel: &String, version: &String) -> anyhow::Result<()> {
    let mut res = Ok(());
    if let Some(channel) = str_opt(channel) {
        if let Ok(mut o) = OVERRIDE_CHANNEL.write() {
            *o = channel.clone();
        }
    }
    if let Some(version) = str_opt(version) {
        match ResolvedVersion::parse_version_id(version) {
            Ok(version) =>
                if let Ok(mut o) = OVERRIDE_VERSION.write() {
                    *o = version;
                },
            Err(e) => res = Err(e),
        }
    }

    res.context("parsing update overrides")
}

#[cfg(taimi_has = "url-github")]
pub static GH_REPO_SRC: LazyLock<GitHubSource> = LazyLock::new(|| {
    GitHubSource::new_empty(
        env!("ADDON_URL_GITHUB_OWNER").into(),
        env!("ADDON_URL_GITHUB_REPO").into(),
    )
});

impl ResolvedVersion {
    pub fn with_gh_release(release: GitHubLatestRelease) -> anyhow::Result<Self> {
        #[cfg(feature = "updates")]
        let version = match &release.tag_name {
            tag if !tag.starts_with("v") => Ok(None),
            tag => Self::parse_version_id(tag)
                .with_context(|| format!("Latest version {} unrecognized", release.tag_name))
                .map(Some),
        }?;
        Ok(Self {
            release,
            #[cfg(feature = "updates")]
            version,
        })
    }

    pub fn with_version_id(id: String) -> anyhow::Result<Self> {
        #[cfg(feature = "updates")]
        let version = Self::parse_version_id(&id).with_context(|| format!("version {id} unrecognized"))?;
        Ok(Self {
            #[cfg(feature = "updates")]
            version: Some(version),
            release: GitHubLatestRelease {
                tag_name: id,
                ..GitHubLatestRelease::empty_with_url("https://taimihud.com".try_into()?)
            },
        })
    }

    pub async fn latest_release(patience: Duration) -> anyhow::Result<Self> {
        Self::latest_gh_release(&GH_REPO_SRC, patience).await
    }

    pub async fn latest_gh_release(src: &GitHubSource, patience: Duration) -> anyhow::Result<Self> {
        log::debug!("Checking for updates at {}...", src.name());

        let check = async move {
            let channel = crate_channel();
            let channel = channel.as_ref().map(|s| &s[..]);
            let latest_release = match channel {
                Some(..) => Ok(None),
                None => match src.get_release(None).await {
                    Ok(release) if release.prerelease => Ok(None),
                    res => res
                        .and_then(Self::with_gh_release)
                        .context("Requesting GH release")
                        .map(Some),
                },
            }?;
            if let Some(release) = latest_release {
                return Ok(release)
            }
            let mut releases: Vec<Self> = src
                .latest_releases(GitHubSource::RELEASES_RANGE_DEFAULT)
                .await?
                .into_iter()
                .map(|r| Self::with_gh_release(r).context("parsing GH release"))
                .filter_map(|release| {
                    if let Err(e) = &release {
                        log::debug!("{e:#}");
                    }
                    release.ok()
                })
                .filter(|release| release.version_channel() == channel)
                .collect();

            releases.sort_by(|l, r| match (&l, &r) {
                #[cfg(feature = "updates")]
                (Self { version: Some(l), .. }, Self { version: Some(r), .. }) => l.cmp_precedence(r),
                _ => l.release.created_at.cmp(&r.release.created_at),
            });
            let channel = channel.unwrap_or("");
            releases
                .into_iter()
                .last()
                .ok_or_else(|| anyhow!("no {channel} releases found at {}", src.name()))
        };
        timeout(patience, check)
            .await
            .context("Timed out while checking for updates")
            .and_then(|res| res)
    }

    pub fn latest_release_standalone<R, Fut, F>(patience: Duration, f: F) -> anyhow::Result<R>
    where
        Fut: Future<Output = anyhow::Result<R>>,
        F: FnOnce(Self) -> Fut,
    {
        Self::latest_gh_release_standalone(&GH_REPO_SRC, patience, f)
    }

    pub fn latest_gh_release_standalone<R, Fut, F>(
        src: &GitHubSource,
        patience: Duration,
        f: F,
    ) -> anyhow::Result<R>
    where
        Fut: Future<Output = anyhow::Result<R>>,
        F: FnOnce(Self) -> Fut,
    {
        let runner = runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("Failed to start update check")?;

        runner.block_on(Self::latest_gh_release(src, patience).and_then(f))
        //.context("Checking for updates")
    }

    pub async fn dll_url(&self, redir_ok: bool) -> anyhow::Result<Url> {
        let mut dlls = self
            .release
            .assets_for(SourceKind::Addon)
            .filter_map(|(asset, form)| match form {
                RemoteAssetForm::File { .. } => Some(asset),
                _ => None,
            });
        if let Some(asset) = dlls.next() {
            match asset.download_url().await {
                Ok(asset) => return Ok(asset),
                Err(e) => log::error!("{e:#}"),
            }
        }
        log::warn!("unsure of update url for {self}");
        let mut req = GH_REPO_SRC.request_release_asset_browser(
            match self.version_channel() {
                #[cfg(todo = "unnecessary")]
                None => None,
                _ => Some(self.version_id()),
            },
            DLL_NAME,
        )?;
        GitHubReleaseAsset::prepare_req_for_url(&mut req);
        let fallback_url = req.url().clone();
        if redir_ok {
            return Ok(fallback_url)
        }
        let fallback = source::get_location_for(req, GitHubReleaseAsset::REDIR_LIMIT)
            .await
            .with_context(|| format!("Expected associated dll with release {self}"));
        match fallback {
            Ok(f) => Ok(f),
            Err(e) => {
                log::error!("{e:#}");
                Ok(fallback_url)
            },
        }
    }

    pub fn is_update(&self, quiet: bool) -> bool {
        let (override_channel, override_version) = BootstrapState::read_with(|s| {
            (
                s.update_override_channel.clone(),
                s.update_override_version.clone(),
            )
        });
        let version_matches = match self {
            #[cfg(feature = "updates")]
            Self { version: Some(v), .. } if v.cmp_precedence(&crate_semver()).is_eq() => true,
            #[cfg(feature = "updates")]
            Self { version: Some(v), .. } if v.cmp_precedence(&crate_semver()).is_lt() => {
                if !quiet {
                    log::info!("Ignoring outdated update {self} vs {}", crate_semver());
                }
                return false
            },
            _ if override_version.is_empty()
                && Some(&self.release.tag_name[..]) == built_info::git_tag_name() =>
                true,
            _ if override_version.is_empty() && self.version_tag().ok() == Some(rt::CRATE_VERSION) => true,
            _ if self
                .version_tag()
                .ok()
                .map(|tag| tag == addon_version())
                .unwrap_or(false) =>
                true,
            _ => false,
        };
        if version_matches {
            if !quiet {
                log::info!("Up-to-date with latest version {self}!");
            }
            return false
        }
        let is_dev_build = match built_info::git_release() {
            #[cfg(not(debug_assertions))]
            Some(..) => false,
            _ => true,
        };
        if self.release.prerelease && crate_channel().is_none() {
            if !quiet {
                log::info!("Skipping update to pre-release");
            }
            return false
        } else if is_dev_build && override_channel.is_empty() {
            if !quiet {
                log::info!("Refusing to update development build");
            }
            return false
        }
        true
    }

    pub fn is_authorized(&self) -> Option<bool> {
        BootstrapState::read_with(|state| state.update_preference().authorizes_version(self.version_id()))
    }

    #[cfg(todo)]
    fn is_allowed_auth(&self, authorized: Option<Result<Option<&str>, &str>>) -> Option<bool> {
        match authorized {
            Some(Err(unauthorized)) if unauthorized == self.version_id() => {
                log::info!("Update to {self} blacklisted, skipping");
                Some(false)
            },
            Some(Err(..)) => None,
            Some(Ok(None)) => Some(true),
            Some(Ok(Some(authorized))) if authorized == self.version_id() => Some(true),
            Some(Ok(Some(..))) | None => None,
        }
    }

    pub fn version_id(&self) -> &str {
        &self.release.tag_name
    }

    pub fn version_name(&self) -> &str {
        self.release
            .name
            .as_ref()
            .map(|s| &s[..])
            //.unwrap_or(self.version_tag().ok().unwrap_or(&self.release.tag_name))
            .unwrap_or(&self.release.tag_name)
    }

    pub fn version_tag(&self) -> anyhow::Result<&str> {
        self.release
            .tag_name
            .strip_prefix("v")
            .ok_or_else(|| anyhow!("Latest version {} unrecognized", self.release.tag_name))
    }

    pub fn version_channel(&self) -> Option<&str> {
        match self {
            // TODO: tag via name idk
            #[cfg(feature = "updates")]
            Self { version: Some(v), .. } => version_channel(v),
            _ if self.release.prerelease || self.release.tag_name.contains("-rc.") =>
                Some(CHANNEL_PRERELEASE),
            _ => None,
        }
    }

    pub fn parse_version_id(v: &str) -> Result<Version, semver::Error> {
        let version = v.strip_prefix("v").unwrap_or(v);
        Self::parse_version(version)
    }

    /// nexus tags require the 0.0.0.0 version scheme
    fn gloss_over_nexus_version(v: &str) -> &str {
        if v.as_bytes().iter().filter(|&&c| c == b'.').count() == 3 {
            v.strip_suffix(".0")
        } else {
            None
        }
        .unwrap_or(v)
    }

    fn parse_version(v: &str) -> Result<Version, semver::Error> {
        let v = Self::gloss_over_nexus_version(v);
        let mut v: Version = v.parse()?;
        if let Some(rc) = v.patch.checked_sub(900) {
            v.minor += 1;
            if v.pre.is_empty() {
                v.patch = 0;
                v.pre = semver::Prerelease::new(&format!("{CHANNEL_PRERELEASE}.{rc}")).unwrap_or_default()
            } else {
                v.patch = rc;
            }
        }
        Ok(v)
    }
}

impl fmt::Display for ResolvedVersion {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            #[cfg(feature = "updates")]
            Self { version: Some(version), .. } => fmt::Display::fmt(version, f),
            _ => f.write_str(self.version_name()),
        }
    }
}

pub struct Updater;

impl Updater {
    pub fn get_preference() -> UpdatePreference {
        let mut outdated = false;
        let pref = BootstrapState::read_with(|state| match state.update_preference() {
            UpdatePreference::Ask {
                authorized: Some(Ok(version) | Err(version)),
            } if version == rt::CRATE_VERSION => {
                outdated = true;
                UpdatePreference::ASK
            },
            UpdatePreference::Once { authorized } if authorized == rt::CRATE_VERSION => {
                outdated = true;
                UpdatePreference::Never
            },
            pref => pref.clone(),
        });
        if outdated {
            Self::mark_update_outdated(None);
        }
        pref
    }

    #[cfg(todo = "unused")]
    pub(crate) fn version_pref_matches_crate(pref: &str) -> bool {
        if pref == rt::CRATE_VERSION {
            return true
        }

        #[cfg(feature = "updates")]
        if let Ok(version) = pref.parse::<Version>() {
            if crate_semver().cmp_precedence(&version).is_eq() {
                return true
            }
        }

        false
    }

    /// returns [`release.is_authorized()`](ResolvedVersion::is_authorized)
    pub fn notify_latest(release: &ResolvedVersion) -> anyhow::Result<bool> {
        log::info!("Latest version is {}", release);
        if !release.is_update(false) {
            return Ok(false)
        }
        if release.release.assets_for(SourceKind::Addon).next().is_none() {
            anyhow::bail!("Invalid update found");
        }

        Ok(match release.is_authorized() {
            None => {
                log::info!("Update requires user authorization");
                Self::mark_update_outdated(Some(release));
                false
            },
            Some(auth) => {
                if !auth {
                    log::info!("Update {release} blacklisted, skipping");
                }

                auth
            },
        })
    }

    pub fn mark_update_outdated(latest: Option<&ResolvedVersion>) {
        if let Some(latest) = latest {
            log::debug!("Recording latest available update: {latest}");
        }
        BootstrapState::write_with(|state| {
            if latest.is_none() && state.update_preference.is_none() {
                // nothing to do...
                return
            }
            let updated_pref = match state.update_preference {
                Some(UpdatePreference::Ask { authorized: Some(..) }) => Some(UpdatePreference::ASK),
                Some(UpdatePreference::Once { .. }) => Some(UpdatePreference::Never),
                _ => None,
            };
            if let Some(pref) = updated_pref {
                state.update_preference = Some(pref);
            }
            state.update_remote_version = latest.map(|r| r.version_id().into());
        });
    }

    pub async fn perform(release: &ResolvedVersion) -> rt::RuntimeResult<()> {
        #[cfg(feature = "extension-nexus")]
        if let Some(res) = crate::exports::nexus::perform_update(release).await? {
            return Ok(res)
        }

        Err(rt::RT_UNAVAILABLE)
    }
}

#[cfg(feature = "updates")]
fn version_channel(version: &Version) -> Option<&str> {
    match version {
        version if !version.pre.is_empty() => version.pre.split(".").next(),
        _ => None,
    }
}
#[cfg(feature = "extension-nexus")]
fn version_channel_parts(version: &Version) -> Option<(&str, u64)> {
    if version.pre.is_empty() { return None }
    let mut parts = version.pre.split(".");
    let channel = parts.next()?;
    let rev = match parts.next().map(|part| part.parse::<u64>()) {
        Some(Ok(rev)) => rev,
        _ => 0,
    };
    Some((channel, rev))
}
#[cfg(feature = "extension-nexus")]
fn version_to_addonapi(version: &Version) -> NexusVersion {
    let mut addonapi = NexusVersion {
        major: version.major as i16,
        minor: version.minor as i16,
        build: version.patch as i16,
        revision: 0,
    };
    match version_channel_parts(version) {
        Some((self::CHANNEL_PRERELEASE, rc)) => {
            addonapi.build = 900i16 + rc as i16;
            if addonapi.minor == 0 {
                addonapi.major -= 1;
                addonapi.minor = 99;
            } else {
                addonapi.minor -= 1;
            }
        },
        Some((channel, rc)) => {
            let offset = match channel {
                self::CHANNEL_ALPHA => 0x200i16,
                self::CHANNEL_BETA => 0x1c0i16,
                #[cfg(todo)]
                self::CHANNEL_PRERELEASE => -0x80i16,
                channel  =>
                    0x6c00i16 - channel.as_bytes().get(0).map(|l| l.to_ascii_lowercase().saturating_sub(b'a') as i16 * 0x400).unwrap_or(0),
            };
            addonapi.revision = (offset + rc as i16).min(-2);
        },
        _ => (),
    }
    addonapi
}
