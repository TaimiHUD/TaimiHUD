use {
    self::{account::EndpointAccountState, client::ApiClient},
    crate::{
        controller::Controller,
        exports::runtime as rt,
        settings::{
            state::{BootstrapState, SaveState, SavedApiToken},
            SettingsLock,
        },
        Interruption,
        InterruptionSignal,
    },
    anyhow::Context,
    bitvec::array::BitArray,
    futures::future::Either,
    std::{
        collections::BTreeSet,
        fmt,
        future::Future,
        path::{Path, PathBuf},
        pin::Pin,
        sync::Arc,
    },
    strum::VariantArray,
    taimi_api_client::{festivals::FestivalCategory, model::authenticated::Tokeninfo},
    taimi_hoard::paths::path_join_append,
    taimi_meta::ui::gameplay::{GameplayState, GameplayTransition},
    taimi_pack::attributes::Festival,
    taimi_sync::watched::{watch, Watched},
    tokio::{fs, io::AsyncReadExt, select, sync::mpsc, task::JoinSet, time},
};

#[cfg(feature = "paths")]
pub use self::festivals::FestivalState;
pub use self::{
    account::{ApiAccountInfo, ApiAccountState},
    achievements::{AchievementBits, AchievementId, AchievementState},
};

mod account;
mod achievements;
mod client;
#[cfg(feature = "paths")]
mod festivals;

pub type RequestBox = Pin<Box<dyn Future<Output = anyhow::Result<Option<ApiMessage>>> + Send + 'static>>;
/// TODO
pub type RaidState = BTreeSet<String>;
pub type SharedRaidState = Arc<RaidState>;
pub type SharedAchievementState = Arc<AchievementState>;

pub type ApiTimeZone = chrono::Utc;
pub type ApiTimestamp = chrono::DateTime<ApiTimeZone>;

pub struct ApiController {
    client: Option<Arc<ApiClient>>,
    rx: ApiReceiver,
    settings: SettingsLock,
    inflight: JoinSet<anyhow::Result<Option<ApiMessage>>>,
    active: bool,
    account_expiry: Pin<Box<time::Sleep>>,
}

impl ApiController {
    pub fn new(rx: ApiReceiver, settings: SettingsLock) -> Self {
        Self {
            rx,
            settings,
            client: None,
            inflight: Default::default(),
            active: true,
            account_expiry: Box::pin(time::sleep(ApiAccountState::EXPIRY_LAZY)),
        }
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        self.setup().await;

        while self.active {
            let int = self.turn().await;
            if let Some(reason) = int {
                let res = self.exit(reason).await;
                self.active = false;
                return res
            }
        }

        Ok(())
    }

    pub async fn turn(&mut self) -> Option<Interruption> {
        if self.rx.command.is_closed() {
            return Some(
                Interruption::try_drain_signals(&mut self.rx.command).unwrap_or(Interruption::Unspecified),
            )
        }
        let gameplay_prev = self.rx.gameplay.cached.clone().unwrap_or(GameplayState::INITIAL);
        select! {
            e = self.rx.command.recv() => match e {
                None =>
                    return Some(Interruption::Unspecified),
                Some(m) => {
                    let res = self.handle_message(m).await.context("API controller");
                    if let Some(Some(int)) = rt::log::error_ok(res) {
                        return Some(int)
                    }
                },
            },
            Some(res) = self.inflight.join_next(), if !self.inflight.is_empty() => match res {
                Ok(res) => match rt::log::error_ok(res.context("api request task")) {
                    Some(Some(m)) =>
                        return rt::log::error_ok(self.handle_message(m).await).flatten(),
                    None | Some(None) => (),
                },
                Err(e) => crate::log_join_error("api", e),
            },
            _ = self.account_expiry.as_mut() => {
                self.process_account_expiry();
            },
            gameplay = self.rx.gameplay.when_changed() => {
                let gameplay = *gameplay;
                let trans = gameplay.latest_transition_from(gameplay_prev);
                if let GameplayTransition::Loaded { initial: true, .. } = trans {
                    self.reload_all().await;
                }
            },
        }
        None
    }

    async fn exit(&mut self, reason: Interruption) -> anyhow::Result<()> {
        self.inflight.abort_all();
        match reason {
            Interruption::Abort => return Ok(()),
            _ => (),
        }

        self.inflight.shutdown().await;

        Ok(())
    }

    async fn handle_message(&mut self, message: ApiMessage) -> anyhow::Result<Option<Interruption>> {
        match message {
            ApiMessage::Exit(reason) => return Ok(Some(reason)),
            #[cfg(todo)]
            ApiMessage::TokenClear => self.process_token_clear().await,
            ApiMessage::TokenAdd(token) => self.process_token_add(token).await.context("adding API token"),
            ApiMessage::Request(req) => {
                let fut = req(self.client(None)?);
                self.inflight.spawn(fut);
                Ok(())
            },
            ApiMessage::TokenRemove(id) => Ok(self.process_token_remove(id).await),
            ApiMessage::RefreshAccount { endpoint, token_id } =>
                Ok(self.process_refresh_account(endpoint, token_id)),
            ApiMessage::AccountInfoReload(endpoint, fresh) => {
                if fresh {
                    self.info_mark_updated(endpoint);
                }
                Ok(self.info_reload(endpoint))
            },
            ApiMessage::AccountInfoRefresh(endpoint) => Ok(self.info_refresh(endpoint)),
            ApiMessage::AccountInfoRaidClears(state) => {
                self.rx.raids.send_if_modified(|shared| {
                    if **shared == state {
                        return false
                    }
                    *Arc::make_mut(shared) = state;
                    true
                });
                Ok(())
            },
            ApiMessage::AccountInfoAchievements(state) => {
                self.rx.achievements.send_if_modified(|shared| {
                    if **shared == state {
                        return false
                    }
                    *Arc::make_mut(shared) = state;
                    true
                });
                Ok(())
            },
            ApiMessage::AccountStateUpdate(account) => {
                self.update_account_state(&account);
                Ok(())
            },
            ApiMessage::SetAutoUpdate(update) => {
                self.set_auto_update(update);
                Ok(())
            },
        }
        .map(|()| None)
    }

    async fn process_token_add(&mut self, token: SavedApiToken) -> anyhow::Result<()> {
        let mut client = ApiClient::new(token)?;

        if client.token.token().is_some() && client.token.id().is_none() {
            let info = rt::log::error_ok(client.client.request_fixed::<Tokeninfo>().await);
            if let Some(info) = info {
                client.token.id = info.id;
                client.token.name = info.name;
                client.token.permissions = info
                    .permissions
                    .into_iter()
                    .map(|p| ApiClient::lib_permission_display(p).to_string())
                    .collect();
            }
        }

        if client.token.token().is_some() && client.token.account_id().is_none() {
            let account = rt::log::error_ok(Self::request_account_state(&client).await);
            if let Some(account) = account {
                self.update_account_state(&account);
                client.token.account_id = account.id;
                client.token.account_name = account.name;
            }
        }

        let client = &*self.client.insert(Arc::new(client));

        if client.token.token().is_some() {
            log::debug!("TODO: commit token elsewhere?");
            BootstrapState::write_with(|state| {
                let out = state.anet_api_token_mut(
                    |t| t.account_id == client.token.account_id, //|| t.id == client.token.id
                );
                *out = client.token.clone();
            });
        }

        Ok(())
    }

    async fn process_token_remove(&mut self, id: String) {
        BootstrapState::try_write_with(|state| {
            let prev_len = state.anet_api_token.len();
            state.anet_api_token.retain(|token| token.id != id);
            prev_len != state.anet_api_token.len()
        });
        let is_current_token = self
            .client
            .as_ref()
            .map(|client| client.token.id == id)
            .unwrap_or(false);
        if is_current_token {
            let _ = self.client.take();
        }
    }

    async fn process_token_clear(&mut self) -> anyhow::Result<()> {
        let _ = self.client.take();
        log::debug!("TODO: remove all saved tokens");

        Ok(())
    }

    fn set_auto_update(&mut self, set: bool) {
        SaveState::write_with(|s| s.api_auto_update = set);
        if set {
            self.account_expiry
                .as_mut()
                .reset(time::Instant::now() + ApiAccountState::EXPIRY_MIN);
        }
    }
    fn auto_update_enabled(&self) -> bool {
        SaveState::read_with(|s| s.api_auto_update)
    }

    pub async fn setup(&mut self) {
        let api_setup = self.setup_get();

        let settings = self.settings.clone();
        let settings = async move {
            let _settings = settings.read().await;
            #[cfg(feature = "paths")]
            let festivals = _settings.pathing().festival_preferences();
            #[cfg(not(feature = "paths"))]
            let festivals = ();
            (festivals,)
        };

        let (settings, api_setup) = tokio::join!(settings, api_setup);
        let (_festivals,) = settings;

        #[cfg(feature = "paths")]
        Controller::with_sender(|s| {
            if let Some(api) = s.api.as_ref() {
                api.festivals.send_if_modified(|state| {
                    state.update_preferences(_festivals);
                    !state.on.is_empty() || !state.off.is_empty()
                });
            }
        });
        for api_setup in api_setup {
            if let Some(task) = Self::api_setup_get_each(api_setup) {
                self.inflight.spawn(task);
            }
        }
    }

    pub fn current_festival(&self) -> Option<Festival> {
        todo!()
    }

    /// list of paths to load or populate with given api token
    pub fn setup_get(
        &mut self,
    ) -> impl Future<Output = Vec<(ApiAccountInfo, Either<PathBuf, Option<String>>)>> + 'static {
        let (account_name, token_id) = match Self::current_account_token() {
            Ok(token) if token.id().is_none() => (Some(token.account_name), None),
            Ok(token) => (Some(token.account_name), Some(token.id)),
            Err(account_name) => (account_name, None),
        };

        let mut outdated: BitArray<[u32; 1]> = Default::default();
        for (&endpoint, mut outdated) in ApiAccountInfo::VARIANTS.iter().zip(outdated.iter_mut()) {
            *outdated = self.api_info_outdated(endpoint);
        }
        async move {
            let mut res = Vec::with_capacity(ApiAccountInfo::VARIANTS.len());
            if let Some(acc) = &account_name {
                let mut account_path = Self::account_path(&acc);
                for (&endpoint, outdated) in ApiAccountInfo::VARIANTS.into_iter().zip(outdated) {
                    path_join_append(&mut account_path, endpoint.filename());
                    let missing = fs::try_exists(&account_path).await.ok() == Some(false);
                    if !missing {
                        res.push((endpoint, Either::Left(account_path.clone())));
                    }
                    if missing || (token_id.is_some() && outdated) {
                        res.push((endpoint, Either::Right(token_id.clone())));
                    };
                    account_path.pop();
                }
            }
            res
        }
    }
    pub(super) fn api_setup_get_each(
        (endpoint, missing): (ApiAccountInfo, Either<PathBuf, Option<String>>),
    ) -> Option<RequestBox> {
        match missing {
            Either::Left(load_path) => return Some(Box::pin(Self::load_info_from(endpoint, load_path))),
            Either::Right(Some(token_id)) => {
                ApiMessage::RefreshAccount { endpoint, token_id: Some(token_id) }.try_send();
            },
            Either::Right(None) => (),
        }
        None
    }
    async fn deserialize_path<T: serde::de::DeserializeOwned>(path: &Path) -> anyhow::Result<T> {
        let mut f = fs::File::open(path).await?;
        let mut data = Vec::with_capacity(
            match () {
                #[cfg(windows)]
                () => {
                    use std::os::windows::fs::MetadataExt;
                    f.metadata()
                        .await
                        .ok()
                        .and_then(|meta| meta.file_size().try_into().ok())
                },
                #[cfg(not(windows))]
                _ => None::<usize>,
            }
            .unwrap_or(0x1000),
        );
        f.read_to_end(&mut data).await?;
        serde_json::from_slice::<T>(&data).map_err(anyhow::Error::from)
    }
}

pub enum ApiMessage {
    #[cfg(todo)]
    TokenClear,
    TokenAdd(SavedApiToken),
    TokenRemove(String),
    RefreshAccount {
        endpoint: ApiAccountInfo,
        token_id: Option<String>,
    },
    Request(Box<dyn FnOnce(&Arc<ApiClient>) -> RequestBox + Send + 'static>),
    Exit(Interruption),
    AccountInfoRefresh(Option<ApiAccountInfo>),
    AccountInfoReload(Option<ApiAccountInfo>, bool),
    AccountInfoRaidClears(RaidState),
    AccountInfoAchievements(AchievementState),
    AccountStateUpdate(EndpointAccountState),
    SetAutoUpdate(bool),
}
impl ApiMessage {
    pub fn try_send(self) {
        let _ = Controller::with_sender(|s| s.api_try_send(self));
    }
}
impl InterruptionSignal for ApiMessage {
    fn interrupted(&self) -> Option<Interruption> {
        match self {
            &Self::Exit(reason) => Some(reason),
            _ => None,
        }
    }
}

#[derive(Clone)]
pub struct ApiSender {
    pub command: mpsc::Sender<ApiMessage>,
    #[cfg(feature = "paths")]
    pub festivals: watch::Sender<FestivalState>,
    #[cfg(feature = "paths")]
    pub achievements: watch::Sender<SharedAchievementState>,
    #[cfg(feature = "paths")]
    pub raids: watch::Sender<SharedRaidState>,
    pub account_state: watch::Sender<ApiAccountState>,
}
pub struct ApiReceiver {
    pub command: mpsc::Receiver<ApiMessage>,
    #[cfg(feature = "paths")]
    pub festivals: watch::Sender<FestivalState>,
    #[cfg(feature = "paths")]
    pub achievements: watch::Sender<SharedAchievementState>,
    #[cfg(feature = "paths")]
    pub raids: watch::Sender<SharedRaidState>,
    pub gameplay: Watched<GameplayState>,
    pub account_state: Watched<ApiAccountState>,
}

impl ApiSender {
    pub fn new(gameplay: &watch::Sender<GameplayState>) -> (Self, ApiReceiver) {
        #[cfg(feature = "paths")]
        let festivals = {
            let initial = festivals::FestivalWindow::current_festivals();
            watch::Sender::new(FestivalState::new(initial))
        };
        #[cfg(feature = "paths")]
        let achievements = watch::Sender::new(Arc::new(AchievementState::default()));
        #[cfg(feature = "paths")]
        let raids = watch::Sender::new(Arc::new(RaidState::default()));
        let account_state = Watched::new_default();
        let (tx, rx) = mpsc::channel(32);

        let sender = Self {
            command: tx,
            #[cfg(feature = "paths")]
            festivals: festivals.clone(),
            #[cfg(feature = "paths")]
            achievements: achievements.clone(),
            #[cfg(feature = "paths")]
            raids: raids.clone(),
            account_state: account_state.watch.sender().clone(),
        };

        let receiver = ApiReceiver {
            command: rx,
            #[cfg(feature = "paths")]
            festivals,
            #[cfg(feature = "paths")]
            achievements,
            #[cfg(feature = "paths")]
            raids,
            gameplay: Watched::subscribe_to(gameplay),
            account_state,
        };

        (sender, receiver)
    }
}
impl fmt::Debug for ApiSender {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ApiSender").finish()
    }
}
