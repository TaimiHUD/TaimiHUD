use {
    crate::{
        exports::runtime::{self as rt, log::DeferredLogger},
        render::i18n,
        settings::state::{install::InstallId, save_state_backup, Installation, SavedApiToken},
    },
    anyhow::Context,
    arcffi::repr::EnumRepr,
    macro_rules_attribute::derive,
    serde::{Deserialize, Serialize},
    std::{
        ffi::{OsStr, OsString},
        fmt,
        fs,
        io,
        path::Path,
        sync::LazyLock,
    },
    taimi_hoard::write_owned,
    taimi_sync::watched,
    tokio::{sync::watch, time},
};

/// TODO: rename/move here
pub use crate::settings::arc::ArcUpdatePreference as UpdatePreference;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct BootstrapState {
    #[serde(default, skip_serializing_if = "Installation::id_is_empty")]
    pub install_id: InstallId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub addon_host_preference: Option<AddonHostName>,
    #[serde(default, skip_serializing_if = "taimi_hoard::is_false_ref")]
    pub addon_host_exclusive: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_addon_host: Option<AddonHostName>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub latest_addon_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update_preference: Option<UpdatePreference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update_host_preference: Option<AddonHostName>,
    #[serde(default, skip_serializing_if = "taimi_hoard::is_false_ref")]
    pub update_host_self: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update_remote_version: Option<String>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub update_override_channel: String,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub update_override_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gh_api_token: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub anet_api_token: Vec<SavedApiToken>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub addon_dir: Option<OsString>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_filter: Option<rt::log::LogFilterDesc>,
}

impl BootstrapState {
    pub const EMPTY: Self = Self {
        install_id: Installation::ID_EMPTY,
        addon_host_preference: None,
        addon_host_exclusive: false,
        latest_addon_host: None,
        latest_addon_version: String::new(),
        update_host_preference: None,
        update_host_self: false,
        update_preference: None,
        update_remote_version: None,
        update_override_channel: String::new(),
        update_override_version: String::new(),
        gh_api_token: None,
        anet_api_token: Vec::new(),
        addon_dir: None,
        language: None,
        log_filter: None,
    };

    pub fn new() -> Self {
        Self::EMPTY
    }

    pub fn get() -> &'static watch::Sender<Self> {
        static LOCK: LazyLock<watch::Sender<BootstrapState>> =
            LazyLock::new(|| watch::Sender::new(BootstrapState::initial_load()));
        &LOCK
    }

    fn initial_load() -> Self {
        let res = match Self::read_file(Self::file_path()) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Self::new(),
            res => res.context("boot state file failed to load"),
        };
        match res {
            Ok(mut state) => {
                // clear update info the moment an update has occurred
                state.update_version();
                #[cfg(feature = "updates")]
                state.report_version();
                state
            },
            Err(e) => {
                log::error!(logger: DeferredLogger::BEST_EFFORT, "{e:#}");
                save_state_backup(Self::file_path());
                Self::new()
            },
        }
    }

    #[cfg(todo = "unnecessary")]
    pub fn is_empty(&self) -> bool {
        match self {
            Self {
                addon_host_preference: None,
                update_host_preference: None,
                update_preference: None,
                update_remote_version: None,
                addon_dir: None,
                language: None,
                log_filter: None,
            } => true,
            _ => false,
        }
    }

    pub fn file_path() -> &'static Path {
        Path::new("addons/Taimi/boot.json")
    }

    pub fn installation() -> &'static Installation {
        use sync_unsafe_cell::SyncUnsafeCell;
        static INSTALL: SyncUnsafeCell<Installation> = SyncUnsafeCell::new(Installation::EMPTY);
        unsafe {
            Self::try_write_with(move |s| {
                let install = &mut *INSTALL.get();
                if install.id.is_nil() {
                    install.id = s.install_id.clone();
                }
                if install.try_setup() {
                    let new_id = install.id.clone();
                    if s.install_id == new_id {
                        return false
                    }
                    s.install_id = new_id;
                    true
                } else {
                    false
                }
            });
            &*INSTALL.get()
        }
    }

    pub fn read_file(path: &Path) -> io::Result<Self> {
        let f = fs::File::open(path)?;
        serde_json::from_reader(io::BufReader::with_capacity(2048, f)).map_err(Into::into)
    }
    pub fn write_file((path, data): &(&Path, String)) -> anyhow::Result<()> {
        use std::io::Write;
        let _ = fs::create_dir_all(rt::addon_dir_fallback());
        let mut f = fs::File::create(path)?;
        f.write_all(data.as_bytes()).context("writing boot state")
    }
    pub fn start_save(&self) -> anyhow::Result<(&'static Path, String)> {
        let s = serde_json::to_string(self).context("boot state serialization error")?;

        Ok((Self::file_path(), s))
    }
    pub async fn save_to((path, data): &(&Path, String)) -> anyhow::Result<()> {
        use tokio::{fs, io::AsyncWriteExt};
        let _ = fs::create_dir_all(rt::addon_dir_fallback()).await;
        let mut f = fs::File::create(path).await?;
        f.write_all(data.as_bytes()).await.context("writing boot state")
    }

    pub fn read_with<R, F: FnOnce(&Self) -> R>(f: F) -> R {
        let state = Self::get().borrow();
        f(&state)
    }

    pub const SAVE_THROTTLE_TIMEOUT: time::Duration = time::Duration::from_secs(30);
    pub fn watch_initial_delay() -> watched::WatchThrottleDelay {
        Some(Box::pin(time::sleep(Self::SAVE_THROTTLE_TIMEOUT)))
    }
    pub async fn watch_dirty(
        receiver: &mut watch::Receiver<Self>,
        throttle: &mut watched::WatchThrottleDelay,
    ) -> Result<(), watch::error::RecvError> {
        if let Some(throttle) = throttle {
            throttle.await;
        }
        let _ = throttle.take();
        let res = receiver.changed().await;
        receiver.mark_changed();
        *throttle = Self::watch_initial_delay();

        res
    }

    pub fn write_with<F: FnOnce(&mut Self)>(f: F) {
        Self::get().send_modify(f)
    }
    pub fn try_write_with<F: FnOnce(&mut Self) -> bool>(f: F) -> bool {
        Self::get().send_if_modified(f)
    }

    #[inline]
    pub fn language_id(&self) -> Option<i18n::LanguageIdentifier> {
        self.language.as_ref().and_then(|l| {
            const REGION_ZH_CN: i18n::unic_subtags::Region = i18n::new_lang_id!(Region: "cn");
            if REGION_ZH_CN.as_str() == &l[..] {
                // nexus uses region instead of lang code for some reason, so adjust..
                return Some(i18n::LANG_ZH)
            }
            match l.parse() {
                Err(e) => {
                    log::info!(logger: DeferredLogger::BEST_EFFORT, "invalid configured language: {e}");
                    None
                },
                Ok(l) => Some(l),
            }
        })
    }

    /// only use this for UI, check [self.addon_host_is_preferred()] if verifying
    pub fn addon_host_preference(&self) -> AddonHostName {
        self.addon_host_preference.unwrap_or_else(|| {
            for prio in AddonHostName::HOST_PRIORITY {
                if prio.is_active() || prio.is_detected() == Some(true) {
                    return *prio
                }
            }
            AddonHostName::DEFAULT
        })
    }
    #[allow(unreachable_patterns)]
    pub fn addon_host_is_preferred(
        &self,
        requester: AddonHostName,
        exclusive: Option<bool>,
    ) -> Result<(), AddonHostName> {
        let exclusive = exclusive.unwrap_or(self.addon_host_exclusive);
        match (self.addon_host_preference, requester) {
            (Some(pref), req) if pref.contains(req) => Ok(()),
            (Some(pref), _) if !exclusive && pref.is_detected() == Some(false) => Ok(()),
            (Some(pref), _) => Err(pref),
            (None, ref req) => {
                for prio in AddonHostName::HOST_PRIORITY {
                    if prio == req {
                        return Ok(())
                    }
                    if prio.is_detected() == Some(true) {
                        return Err(*prio)
                    }
                }
                Err(AddonHostName::DEFAULT)
            },
            #[cfg(feature = "extension-nexus")]
            (None, AddonHostName::Nexus) => Ok(()),
            #[cfg(feature = "extension-nexus")]
            (None, _) if AddonHostName::is_nexus_detected() == Some(true) => Err(AddonHostName::Nexus),
            #[cfg(feature = "extension-arcdps")]
            (None, AddonHostName::ArcDPS) => Ok(()),
            (None, _) if AddonHostName::is_arcdps_detected() == Some(true) => Err(AddonHostName::ArcDPS),
            (None, req) if req == AddonHostName::DEFAULT => Ok(()),
            (None, _) => Err(AddonHostName::DEFAULT),
        }
    }
    pub fn update_host_is_preferred(&self, requester: AddonHostName) -> Result<(), Option<AddonHostName>> {
        match self.get_update_host_preference() {
            Some(Some(pref)) if pref.contains(requester) => Ok(()),
            Some(pref) => Err(pref),
            None if Some(requester) == self.reliable_addon_host_or(Some(requester))
                && self.addon_host_is_preferred(requester, Some(true)).is_ok() =>
                Ok(()),
            None => Err(None),
        }
    }

    fn default_update_preference() -> &'static UpdatePreference {
        let never = &UpdatePreference::Never;
        let ask = &UpdatePreference::ASK;
        #[allow(unreachable_patterns)]
        match () {
            #[cfg(debug_assertions)]
            _ => never,
            #[cfg(feature = "extension-nexus")]
            _ if crate::built_info::IS_TAGGED_RELEASE_OR_RC && rt::nexus_available() => never,
            #[cfg(feature = "updates")]
            _ if rt::update::crate_channel_build() != Some(rt::update::CHANNEL_DEBUG) => ask,
            _ => never,
        }
    }

    pub fn update_preference(&self) -> &UpdatePreference {
        self.update_preference
            .as_ref()
            .unwrap_or(Self::default_update_preference())
    }

    pub fn update_host_preference(&self) -> Option<AddonHostName> {
        self.get_update_host_preference()
            .unwrap_or_else(|| match self.reliable_addon_host_or(None) {
                Some(host) if host.is_preferred_host().is_ok() => Some(host),
                _ => None,
            })
    }
    /// oops serde_json and default make `Option<Option<T>>` awkward, right...
    pub fn get_update_host_preference(&self) -> Option<Option<AddonHostName>> {
        match self.update_host_self {
            true => Some(None),
            false => self.update_host_preference.map(Some),
        }
    }
    pub fn set_update_host_preference(&mut self, pref: Option<Option<AddonHostName>>) {
        self.update_host_self = matches!(pref, Some(None));
        self.update_host_preference = pref.flatten();
    }

    fn reliable_addon_host_or(&self, or: Option<AddonHostName>) -> Option<AddonHostName> {
        match self.latest_addon_host {
            Some(AddonHostName::All) => None,
            latest => {
                let current = match (Self::current_addon_host(), or) {
                    (Some(current), Some(or)) if current != or => None,
                    (current, or) => current.or(or),
                };
                match (current, latest) {
                    (Some(current), Some(latest)) if current != latest => None,
                    (current, latest) => current.or(latest),
                }
            },
        }
    }
    pub fn try_init_latest_host(&mut self, host: AddonHostName) -> bool {
        let latest = self.latest_addon_host;
        // saving immediately seems a little unnecessary maybe?
        let mut changed = false;
        let next = match latest {
            None => Some(host),
            Some(h) if h != host => {
                changed = true;
                Some(AddonHostName::All)
            },
            Some(..) => return false,
        };
        self.latest_addon_host = next;
        changed
    }
    pub fn try_update_latest_host(&mut self, host: Option<Option<AddonHostName>>) -> bool {
        let host = match host {
            _ if self.addon_host_preference == Some(AddonHostName::All) => None,
            Some(host) => host,
            None => Self::current_addon_host(),
        };
        if self.latest_addon_host == host {
            return false
        }
        self.latest_addon_host = host;
        true
    }

    pub fn current_addon_host() -> Option<AddonHostName> {
        for prio in AddonHostName::HOST_PRIORITY {
            if prio.is_active() {
                return Some(*prio)
            }
        }
        None
    }

    pub fn gh_api_token(&self) -> Option<&str> {
        match &self.gh_api_token {
            None => None,
            Some(token) if token.is_empty() => None,
            Some(token) => Some(token),
        }
    }
    pub fn anet_api_token<'a>(&'a self, acc: &str) -> Option<&'a SavedApiToken> {
        SavedApiToken::token_by_account_name(&self.anet_api_token, acc)
    }
    pub fn anet_api_token_mut<'a, F: FnMut(&SavedApiToken) -> bool>(
        &'a mut self,
        criteria: F,
    ) -> &'a mut SavedApiToken {
        SavedApiToken::get_token_mut(&mut self.anet_api_token, criteria)
    }

    pub fn init_addon_dir<D: AsRef<OsStr> + Into<OsString>>(addon_dir: D) -> bool {
        Self::try_write_with(|state| {
            let changed = if state.addon_dir.as_ref().map(|d| d.as_os_str()) != Some(addon_dir.as_ref()) {
                state.addon_dir = Some(addon_dir.into());
                true
            } else {
                false
            };

            changed
        })
    }

    pub(crate) fn update_version(&mut self) -> Option<()> {
        let addon_version = rt::update::addon_version_build();
        if self.latest_addon_version == addon_version {
            return None
        }

        let mut has_override = false;
        if !self.latest_addon_version.is_empty() {
            has_override =
                !self.update_override_version.is_empty() || !self.update_override_channel.is_empty();
            let info = match has_override {
                true => "overide",
                false => "info",
            };
            log::info!(logger: DeferredLogger::BEST_EFFORT, "clearing update {info} from {}", self.latest_addon_version);
        }

        self.update_remote_version.take();
        if has_override && self.update_preference == Some(UpdatePreference::Always) {
            self.update_preference.take();
        }
        if let Some(pref) = &mut self.update_preference {
            pref.take_authorization();
        }
        self.update_override_version.clear();
        self.update_override_channel.clear();
        write_owned(&mut self.latest_addon_version, addon_version);

        Some(())
    }

    #[cfg(feature = "updates")]
    fn report_version(&self) {
        let res =
            rt::update::report_overrides(&self.update_override_channel, &self.update_override_version);
        if let Err(e) = res {
            log::warn!(logger: DeferredLogger::BEST_EFFORT, "{e:#}");
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize, strum::IntoStaticStr, EnumRepr!)]
#[repr(u8)]
pub enum AddonHostName {
    ArcDPS,
    Nexus,
    All,
}

#[allow(unreachable_patterns)]
impl AddonHostName {
    pub const ALL: [Self; 3] = [Self::ArcDPS, Self::Nexus, Self::All];
    pub const HOST_PRIORITY: &'static [Self] = &[
        #[cfg(feature = "extension-nexus")]
        Self::Nexus,
        #[cfg(feature = "extension-arcdps")]
        Self::ArcDPS,
    ];

    pub const DEFAULT: Self = Self::HOST_PRIORITY[0];

    pub fn id(&self) -> &'static str {
        match self {
            Self::ArcDPS => "arcdps",
            Self::Nexus => "nexus",
            Self::All => "multi-addon-host",
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::ArcDPS => "ArcDPS",
            Self::Nexus => "Nexus",
            Self::All => "All",
        }
    }

    pub fn contains(&self, host: Self) -> bool {
        match (self, host) {
            (Self::All, _) => true,
            (&l, r) if l == r => true,
            _ => false,
        }
    }

    pub fn is_detected(&self) -> Option<bool> {
        match self {
            Self::ArcDPS => Self::is_arcdps_detected(),
            Self::Nexus => Self::is_nexus_detected(),
            Self::All => None,
        }
    }
    pub fn is_active(&self) -> bool {
        match self {
            Self::ArcDPS => rt::arcdps_available(),
            Self::Nexus => rt::nexus_available(),
            Self::All => false,
            // if we're running, maybe it counts?
            #[cfg(todo)]
            Self::All => true,
        }
    }
    pub fn is_loaded(&self) -> bool {
        match self {
            #[cfg(feature = "extension-arcdps")]
            Self::ArcDPS => crate::exports::arcdps::loaded(),
            #[cfg(feature = "extension-nexus")]
            Self::Nexus => crate::exports::nexus::loaded(),
            #[cfg(todo)]
            Self::All => true,
            _ => false,
        }
    }
    /// we *could* enumerate process dlls for symbols that are unique to each loader,
    /// but that might be excessive...
    #[cfg(todo)]
    pub fn is_detected_fallback(&self) -> bool {
        false
    }
    fn is_nexus_detected() -> Option<bool> {
        if rt::nexus_available() {
            return Some(true)
        }
        #[cfg(feature = "extension-nexus")]
        if crate::exports::nexus::loaded() {
            return Some(true)
        }
        match () {
            #[cfg(feature = "extension-nexus-extern")]
            () if crate::exports::nexus::r#extern::is_enumerated() => Some(true),
            #[cfg(all(feature = "extension-nexus", feature = "extension-arcdps"))]
            () => Some(crate::exports::arcdps::check_for_nexus()),
            _ => None,
        }
    }
    fn is_arcdps_detected() -> Option<bool> {
        if rt::arcdps_available() {
            return Some(true)
        }
        #[cfg(feature = "extension-arcdps")]
        if crate::exports::arcdps::loaded() {
            return Some(true)
        }
        #[cfg(feature = "extension-arcdps-extern")]
        if crate::exports::arcdps::r#extern::arc_args().is_some() {
            return Some(true)
        }
        #[cfg(todo)]
        #[cfg(all(feature = "extension-nexus", feature = "extension-arcdps"))]
        if crate::exports::nexus::check_for_arcdps() {
            return Some(true)
        }

        None
    }

    pub fn is_preferred_host(&self) -> Result<(), Self> {
        BootstrapState::read_with(|s| s.addon_host_is_preferred(*self, None))
    }
    pub fn is_explicit_preferred_host(&self) -> Result<(), Self> {
        BootstrapState::read_with(|s| match s.addon_host_preference {
            Some(pref @ Self::All) => Err(pref),
            _ => s.addon_host_is_preferred(*self, Some(true)),
        })
    }
    pub fn is_preferred_update_host(&self) -> Result<(), Option<Self>> {
        BootstrapState::read_with(|s| s.update_host_is_preferred(*self))
    }
}

impl Default for AddonHostName {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl fmt::Display for AddonHostName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.name())
    }
}
