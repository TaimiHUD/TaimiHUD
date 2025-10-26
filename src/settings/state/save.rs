use {
    anyhow::Context,
    crate::exports::runtime::{
        self as rt,
        bindings::GameBinds,
    },
    serde::{Deserialize, Serialize},
    std::{
        fs,
        io,
        path::Path,
        sync::LazyLock,
    },
    tokio::{
        sync::watch,
        time,
    },
};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SaveState {
    #[serde(default, skip_serializing_if = "GameBinds::is_empty")]
    pub game_binds: GameBinds,
    // TODO: move dpi scaling toggle here maybe?
}

impl SaveState {
    pub const EMPTY: Self = Self {
        game_binds: GameBinds::new(),
    };

    pub fn new() -> Self {
        Self::EMPTY
    }

    pub fn get() -> &'static watch::Sender<Self> {
        static LOCK: LazyLock<watch::Sender<SaveState>> = LazyLock::new(|| {
            watch::Sender::new(SaveState::initial_load())
        });
        &LOCK
    }

    fn initial_load() -> Self {
        let res = match Self::read_file(Self::file_path()) {
            Err(e) if e.kind() == io::ErrorKind::NotFound =>
                return Self::new(),
            res => res.context("state file failed to load"),
        };
        match res {
            Ok(state) =>
                state,
            Err(e) => {
                log::error!("{e:#}");
                Self::new()
            },
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Self {
                game_binds,
            } if game_binds.is_empty() => true,
            _ => false,
        }
    }

    pub fn file_path() -> &'static Path {
        Path::new("addons/Taimi/state.json")
    }

    pub fn read_file(path: &Path) -> io::Result<Self> {
        let f = fs::File::open(path)?;
        serde_json::from_reader(io::BufReader::with_capacity(2048, f))
            .map_err(Into::into)
    }
    pub fn write_file(&self, path: &Path) -> anyhow::Result<()> {
        let _ = fs::create_dir_all(rt::addon_dir_fallback());
        let f = fs::File::create(path)?;
        serde_json::to_writer(f, self)
            .context("writing state")
    }
    pub fn start_save(&self) -> anyhow::Result<(&'static Path, String)> {
        let s = serde_json::to_string(self)
            .context("save state serialization error")?;

        Ok((Self::file_path(), s))
    }
    pub async fn save_to((path, data): &(&Path, String)) -> anyhow::Result<()> {
        use tokio::{fs, io::AsyncWriteExt};
        let _ = fs::create_dir_all(rt::addon_dir_fallback()).await;
        let mut f = fs::File::create(path).await?;
        f.write_all(data.as_bytes()).await
            .context("writing save state")
    }

    pub fn read_with<R, F: FnOnce(&Self) -> R>(f: F) -> R {
        let state = Self::get().borrow();
        f(&state)
    }

    pub const SAVE_THROTTLE_TIMEOUT: time::Duration = time::Duration::from_secs(30);
    pub fn watch_initial_delay() -> rt::watched::WatchThrottleDelay {
        Some(Box::pin(time::sleep(Self::SAVE_THROTTLE_TIMEOUT)))
    }
    pub async fn watch_dirty(receiver: &mut watch::Receiver<Self>, throttle: &mut rt::watched::WatchThrottleDelay) -> Result<(), watch::error::RecvError> {
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

    pub fn game_binds_mut(&mut self) -> &mut GameBinds {
        &mut self.game_binds
    }
}
