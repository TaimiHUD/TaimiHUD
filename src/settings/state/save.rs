use {
    crate::{
        exports::runtime::{self as rt, bindings::GameBinds},
        settings::{pathing::PathingSave, state::save_state_backup},
    },
    anyhow::Context,
    serde::{Deserialize, Serialize},
    std::{borrow::Cow, fs, io, path::Path, sync::LazyLock},
    taimi_sync::watched,
    tokio::time,
};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SaveState {
    #[serde(default, skip_serializing_if = "GameBinds::is_empty")]
    pub game_binds: GameBinds,
    /// TODO: put this in an api struct...
    #[serde(default, skip_serializing_if = "taimi_hoard::is_false_ref")]
    pub api_auto_update: bool,
    #[serde(default, skip_serializing_if = "PathingSave::is_empty_opt")]
    pub pathing_state: Option<PathingSave>,
    // TODO: move dpi scaling toggle here maybe?
}

impl SaveState {
    pub const EMPTY: Self = Self {
        game_binds: GameBinds::new(),
        api_auto_update: false,
        pathing_state: None,
    };

    pub fn new() -> Self {
        Self::EMPTY
    }

    pub fn get() -> &'static watched::Tx<Self> {
        static LOCK: LazyLock<watched::Tx<SaveState>> =
            LazyLock::new(|| watched::Tx::new(SaveState::initial_load()));
        &LOCK
    }

    fn initial_load() -> Self {
        let res = match Self::read_file(Self::file_path()) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Self::new(),
            res => res.context("state file failed to load"),
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

    pub fn is_empty(&self) -> bool {
        match self {
            Self { game_binds, .. } if !game_binds.is_empty() => false,
            Self { pathing_state: Some(pathing), .. } if !pathing.is_empty() => false,
            Self {
                game_binds: _,
                pathing_state: _,
                api_auto_update: false,
            } => true,
            _ => false,
        }
    }

    pub fn file_path() -> &'static Path {
        Path::new("addons/Taimi/state.json")
    }

    pub fn read_file(path: &Path) -> io::Result<Self> {
        let f = fs::File::open(path)?;
        serde_json::from_reader(io::BufReader::with_capacity(2048, f)).map_err(Into::into)
    }
    pub fn write_file(&self, path: &Path) -> anyhow::Result<()> {
        let _ = fs::create_dir_all(rt::addon_dir_fallback());
        let f = fs::File::create(path)?;
        serde_json::to_writer(f, self).context("writing state")
    }
    pub fn start_save(&self) -> anyhow::Result<(&'static Path, String)> {
        let s = serde_json::to_string(self).context("save state serialization error")?;

        Ok((Self::file_path(), s))
    }
    pub async fn save_to((path, data): &(&Path, String)) -> anyhow::Result<()> {
        use tokio::{fs, io::AsyncWriteExt};
        let _ = fs::create_dir_all(rt::addon_dir_fallback()).await;
        let mut f = fs::File::create(path).await?;
        f.write_all(data.as_bytes()).await.context("writing save state")
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
        receiver: &mut watched::Rx<Self>,
        throttle: &mut watched::WatchThrottleDelay,
    ) -> Result<(), watched::RxError> {
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

    pub fn game_binds_mut(&mut self) -> &mut GameBinds {
        &mut self.game_binds
    }

    pub fn pathing(&self) -> Cow<'_, PathingSave> {
        match &self.pathing_state {
            Some(pathing) => Cow::Borrowed(pathing),
            None => Default::default(),
        }
    }
    pub fn pathing_mut(&mut self) -> &mut PathingSave {
        self.pathing_state.get_or_insert_default()
    }
}
