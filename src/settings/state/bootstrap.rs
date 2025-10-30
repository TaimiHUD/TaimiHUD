use {
    crate::{exports::runtime as rt, settings::state::save_state_backup},
    anyhow::Context,
    serde::{Deserialize, Serialize},
    std::{
        ffi::{OsStr, OsString},
        fmt,
        fs,
        io,
        path::Path,
        sync::LazyLock,
    },
    tokio::{sync::watch, time},
};

/// TODO: rename/move here
pub use crate::settings::arc::ArcUpdatePreference as UpdatePreference;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct BootstrapState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub addon_host_preference: Option<AddonHostName>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_addon_host: Option<AddonHostName>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update_preference: Option<UpdatePreference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update_host_preference: Option<Option<AddonHostName>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update_remote_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub addon_dir: Option<OsString>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_filter: Option<rt::log::LogFilterDesc>,
}

impl BootstrapState {
    pub const EMPTY: Self = Self {
        addon_host_preference: None,
        latest_addon_host: None,
        update_host_preference: None,
        update_preference: None,
        update_remote_version: None,
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
            Ok(state) => state,
            Err(e) => {
                log::error!("{e:#}");
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

    pub fn read_file(path: &Path) -> io::Result<Self> {
        let f = fs::File::open(path)?;
        serde_json::from_reader(io::BufReader::with_capacity(2048, f)).map_err(Into::into)
    }
    #[cfg(todo = "unnecessary")]
    pub fn write_file(&self, path: &Path) -> anyhow::Result<()> {
        let _ = fs::create_dir_all(rt::addon_dir_fallback());
        let f = fs::File::create(path)?;
        serde_json::to_writer(f, self).context("writing boot state")
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
    pub fn watch_initial_delay() -> rt::watched::WatchThrottleDelay {
        Some(Box::pin(time::sleep(Self::SAVE_THROTTLE_TIMEOUT)))
    }
    pub async fn watch_dirty(
        receiver: &mut watch::Receiver<Self>,
        throttle: &mut rt::watched::WatchThrottleDelay,
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

    pub fn addon_host_preference(&self) -> AddonHostName {
        self.addon_host_preference.unwrap_or_else(|| match () {
            _ if rt::nexus_available() => AddonHostName::Nexus,
            #[cfg(all(feature = "extension-nexus", feature = "extension-arcdps"))]
            _ if crate::exports::arcdps::check_for_nexus() => AddonHostName::Nexus,
            _ if rt::arcdps_available() => AddonHostName::ArcDPS,
            _ => AddonHostName::DEFAULT,
        })
    }

    fn default_update_preference() -> &'static UpdatePreference {
        #[allow(unreachable_patterns)]
        match () {
            #[cfg(debug_assertions)]
            _ => &UpdatePreference::Never,
            #[cfg(feature = "extension-nexus")]
            _ if crate::built_info::IS_TAGGED_RELEASE_OR_RC && rt::nexus_available() =>
                &UpdatePreference::Never,
            #[cfg(feature = "updates")]
            _ if rt::update::crate_channel() != Some(rt::update::CHANNEL_DEBUG) => &UpdatePreference::ASK,
            _ => &UpdatePreference::Never,
        }
    }

    pub fn update_preference(&self) -> &UpdatePreference {
        self.update_preference
            .as_ref()
            .unwrap_or(Self::default_update_preference())
    }

    pub fn update_host_preference(&self) -> Option<AddonHostName> {
        self.update_host_preference
            .unwrap_or_else(|| match self.reliable_addon_host() {
                Some(host) if self.addon_host_preference() == host => Some(host),
                _ => None,
            })
    }

    pub fn reliable_addon_host(&self) -> Option<AddonHostName> {
        match self.latest_addon_host {
            None => Self::current_addon_host(),
            Some(..) => None,
        }
    }

    pub fn current_addon_host() -> Option<AddonHostName> {
        match () {
            #[cfg(feature = "extension-nexus")]
            _ if rt::nexus_available() => Some(AddonHostName::Nexus),
            #[cfg(feature = "extension-arcdps")]
            _ if rt::arcdps_available() => Some(AddonHostName::ArcDPS),
            _ => None,
        }
    }

    pub fn init_addon_dir<D: AsRef<OsStr> + Into<OsString>>(addon_dir: D) -> bool {
        Self::try_write_with(|state| {
            let mut changed = if state.addon_dir.as_ref().map(|d| d.as_os_str()) != Some(addon_dir.as_ref())
            {
                state.addon_dir = Some(addon_dir.into());
                true
            } else {
                false
            };

            if let Some(current_host) = Self::current_addon_host() {
                let host = match state.latest_addon_host {
                    Some(prev_host) if prev_host != current_host => Some(current_host),
                    _ => None,
                };
                changed |= host.is_some();
                state.latest_addon_host = host;
            }

            changed
        })
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
pub enum AddonHostName {
    ArcDPS,
    Nexus,
}

impl AddonHostName {
    pub const ALL: [Self; 2] = [Self::ArcDPS, Self::Nexus];

    #[allow(unreachable_patterns)]
    pub const DEFAULT: Self = match () {
        #[cfg(feature = "extension-nexus")]
        _ => Self::Nexus,
        _ => Self::ArcDPS,
    };

    pub fn id(&self) -> &'static str {
        match self {
            Self::ArcDPS => "arcdps",
            Self::Nexus => "nexus",
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::ArcDPS => "ArcDPS",
            Self::Nexus => "Nexus",
        }
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
