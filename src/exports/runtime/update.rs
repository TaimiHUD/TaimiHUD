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
    std::{fmt, future::Future, sync::LazyLock, time::Duration},
    tokio::{runtime, time::timeout},
    url::Url,
};

pub const GIT_REF_BRANCH_PREFIX: &'static str = "refs/heads/";
pub const GIT_REF_TAG_PREFIX: &'static str = "refs/tags/";
pub const GIT_REF_RELEASE_PREFIX: &'static str = "refs/tags/v";
pub const CHANNEL_DEBUG: &'static str = "debug";
pub const CHANNEL_PRERELEASE: &'static str = "rc";
pub const DLL_NAME: &'static str = "TaimiHUD.dll";

pub struct ResolvedVersion {
    pub release: GitHubLatestRelease,
    #[cfg(feature = "updates")]
    pub version: Option<Version>,
}

#[cfg(feature = "updates")]
pub static CRATE_SEMVER: LazyLock<Version> = LazyLock::new(|| {
    let (major, minor, patch, pre, build) = (
        option_env!("ADDON_VERSION_MAJOR").unwrap_or(env!("CARGO_PKG_VERSION_MAJOR")),
        option_env!("ADDON_VERSION_MINOR").unwrap_or(env!("CARGO_PKG_VERSION_MINOR")),
        option_env!("ADDON_VERSION_PATCH").unwrap_or(env!("CARGO_PKG_VERSION_PATCH")),
        option_env!("ADDON_VERSION_PRE").unwrap_or(env!("CARGO_PKG_VERSION_PRE")),
        option_env!("ADDON_VERSION_BUILD").unwrap_or(""),
    );
    let mut version = Version::new(
        major.parse().unwrap_or_default(),
        minor.parse().unwrap_or_default(),
        patch.parse().unwrap_or_default(),
    );
    version.pre = semver::Prerelease::new(pre).unwrap_or_default();
    version.build = semver::BuildMetadata::new(build).unwrap_or_default();
    version
});
pub fn crate_channel() -> Option<&'static str> {
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

pub static GH_REPO_SRC: LazyLock<GitHubSource> =
    LazyLock::new(|| GitHubSource::new_empty("TaimiHUD".into(), "TaimiHUD".into()));

impl ResolvedVersion {
    pub fn with_gh_release(release: GitHubLatestRelease) -> anyhow::Result<Self> {
        #[cfg(feature = "updates")]
        let version = release
            .tag_name
            .strip_prefix("v")
            .map(|v| {
                v.parse()
                    .with_context(|| format!("Latest version {} unrecognized", release.tag_name))
            })
            .transpose()?;
        Ok(Self {
            release,
            #[cfg(feature = "updates")]
            version,
        })
    }

    pub fn with_version_id(id: String) -> anyhow::Result<Self> {
        #[cfg(feature = "updates")]
        let version = match id.strip_prefix("v") {
            Some(release) => release
                .parse::<Version>()
                .map(Some)
                .with_context(|| format!("version {id} unrecognized"))?,
            _ => None,
        };
        Ok(Self {
            #[cfg(feature = "updates")]
            version,
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
        let req = GH_REPO_SRC.request_release_asset_browser(
            match self.version_channel() {
                #[cfg(todo = "unnecessary")]
                None => None,
                _ => Some(self.version_id()),
            },
            DLL_NAME,
        )?;
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

    pub fn is_update(&self) -> bool {
        let version_matches = match self {
            #[cfg(feature = "updates")]
            Self { version: Some(v), .. } if v.cmp_precedence(&CRATE_SEMVER).is_eq() => true,
            #[cfg(feature = "updates")]
            Self { version: Some(v), .. } if v.cmp_precedence(&CRATE_SEMVER).is_lt() => {
                log::info!("Ignoring outdated update {self}");
                return false
            },
            _ if Some(&self.release.tag_name[..]) == built_info::git_tag_name() => true,
            _ if self.version_tag().ok() == Some(rt::CRATE_VERSION) => true,
            _ => false,
        };
        if version_matches {
            log::info!("Up-to-date with latest version {self}!");
            return false
        }
        let is_dev_build = match built_info::git_release() {
            #[cfg(not(debug_assertions))]
            Some(..) => false,
            _ => true,
        };
        if self.release.prerelease && crate_channel().is_none() {
            log::info!("Skipping update to pre-release");
            return false
        } else if is_dev_build {
            log::info!("Refusing to update development build");
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

    pub(crate) fn version_pref_matches_crate(pref: &str) -> bool {
        if pref == rt::CRATE_VERSION {
            return true
        }

        #[cfg(feature = "updates")]
        if let Ok(version) = pref.parse::<Version>() {
            if CRATE_SEMVER.cmp_precedence(&version).is_eq() {
                return true
            }
        }

        false
    }

    /// returns [`release.is_authorized()`](ResolvedVersion::is_authorized)
    pub fn notify_latest(release: &ResolvedVersion) -> anyhow::Result<bool> {
        log::info!("Latest version is {}", release);
        if !release.is_update() {
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
