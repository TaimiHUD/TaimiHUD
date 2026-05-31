use {
    super::ApiController,
    crate::{
        controller::api::{
            achievements,
            AchievementState,
            ApiClient,
            ApiMessage,
            ApiTimeZone,
            ApiTimestamp,
            RaidState,
        },
        exports::runtime as rt,
        settings::state::{BootstrapState, SavedApiToken},
    },
    anyhow::Context,
    futures::{future::Either, stream::StreamExt},
    reqwest::Request,
    serde::{Deserialize, Serialize},
    std::{collections::BTreeSet, fmt, ops, path::PathBuf, sync::Arc, time::Duration},
    strum::VariantArray,
    taimi_api_client::{
        model::authenticated::account::{
            achievements::AccountAchievements,
            raids::RaidEvent,
            Access,
            Account,
        },
        Endpoint,
        FixedEndpoint,
    },
    taimi_hoard::{lazyfmt, paths::path_join, str_opt_ref},
    tokio::{fs, io::AsyncWriteExt, time},
};

impl ApiController {
    pub fn current_account_token() -> Result<SavedApiToken, Option<String>> {
        let account_name = crate::ACCOUNT_NAME_CELL.get().map(|acc| &acc[..]);
        let token = BootstrapState::read_with(|s| {
            s.anet_api_token(account_name.unwrap_or(""))
                .and_then(|token| token.account_name().map(|_| token.clone()))
        });
        match token {
            Some(token) if account_name.is_none() || account_name == Some(&token.account_name[..]) =>
                Ok(token),
            _ => Err(account_name.map(ToOwned::to_owned)),
        }
    }

    pub(super) async fn request_account_state(client: &ApiClient) -> anyhow::Result<EndpointAccountState> {
        client.client.request_fixed::<EndpointAccountState>().await
    }
    async fn do_account_state_refresh(client: &ApiClient) -> anyhow::Result<Option<ApiMessage>> {
        Self::request_account_state(client)
            .await
            .map(ApiMessage::AccountStateUpdate)
            .map(Some)
    }
    pub(super) fn start_account_state_refresh(&mut self, token_id: Option<&str>) {
        let client = self.client(token_id).cloned();
        self.inflight.spawn(async move {
            let client = client?;
            Self::do_account_state_refresh(&client).await
        });
    }
    pub(super) fn update_account_state(&mut self, account: &EndpointAccountState) {
        let mut next_expiry = None;
        self.rx.account_state.write_if(|state| {
            let mut dirty = false;
            if state.commander != account.commander {
                state.commander = account.commander;
                dirty = true;
            }
            if !account.id.is_empty() && state.account_id != account.id {
                state.account_id = account.id.clone();
                dirty = true;
            }
            let last_modified: Option<ApiTimestamp> = rt::log::warn_ok(account.last_modified.parse());
            match last_modified {
                None => (),
                Some(timestamp) if state.last_modified == timestamp => (),
                Some(timestamp) => {
                    state.last_modified = timestamp;
                    dirty = true;
                },
            }
            if state.content_access != account.access {
                state.content_access = account.access.clone();
                dirty = true;
            }

            let now = ApiTimeZone::now();
            if state.check_expiry(&now) {
                dirty = true;
            }
            if state.update_available != Some(true) || dirty {
                next_expiry = state
                    .next_expiry(&now)
                    .map(|e| e.signed_duration_since(&now))
                    .and_then(|e| e.to_std().ok());
            }

            Some(dirty)
        });

        let delay = next_expiry.unwrap_or(ApiAccountState::EXPIRY_LAZY);
        self.account_expiry.as_mut().reset(time::Instant::now() + delay);
    }
    pub(super) fn process_account_expiry(&mut self) {
        let mut next_expiry = None;
        let mut auto_update = false;
        self.rx.account_state.write_if(|state| {
            let mut dirty = false;
            let prev_update = state.update_available;
            let now = ApiTimeZone::now();
            if state.check_expiry(&now) {
                dirty = true;
            }
            if prev_update != state.update_available && state.update_available == Some(true) {
                if state.data_update_available == Some(true) {
                    auto_update = true;
                }
            } else {
                next_expiry = state
                    .next_expiry(&now)
                    .map(|e| e.signed_duration_since(&now))
                    .and_then(|e| e.to_std().ok());
            }
            Some(dirty)
        });
        let delay = match auto_update {
            true if self.auto_update_enabled() => {
                // TODO: track endpoints individually...
                self.info_refresh(None);
                ApiAccountState::EXPIRY_AUTO
            },
            _ => next_expiry.unwrap_or(ApiAccountState::EXPIRY_LAZY),
        };
        self.account_expiry.as_mut().reset(time::Instant::now() + delay);
    }

    pub(super) async fn reload_all(&mut self) {
        if crate::ACCOUNT_NAME_CELL.get().is_none() {
            // TODO: remove this once we actually register for an event that populates it...
            // and remove the hacky call to this from initial gameplay load
            log::debug!("TODO: still unsure of account name...");
            return
        }
        let api_setup = self.setup_get().await;
        let reloads = api_setup
            .into_iter()
            .filter_map(|(endpoint, missing)| match missing {
                Either::Left(load_path) => Some((endpoint, load_path)),
                Either::Right(..) => None,
            });
        for (endpoint, load_path) in reloads {
            self.inflight.spawn(Self::load_info_from(endpoint, load_path));
        }
    }

    pub(super) fn info_reload(&mut self, endpoint: Option<ApiAccountInfo>) {
        let account = Self::current_account_token();
        let account_name = match &account {
            Ok(token) => token.account_name(),
            Err(name) => name.as_ref().map(|n| &n[..]),
        };
        let Some(account_name) = account_name else {
            log::warn!("can't refresh due to unknown account name");
            return
        };
        let endpoints = match endpoint {
            Some(..) => &[],
            None => ApiAccountInfo::DATA_ENDPOINTS,
        }
        .iter()
        .chain(endpoint.iter());
        for &endpoint in endpoints {
            let path = Self::account_info_path(account_name, endpoint);
            self.inflight.spawn(Self::load_info_from(endpoint, path));
        }
    }
    pub(super) fn info_mark_updated(&mut self, endpoint: Option<ApiAccountInfo>) {
        match endpoint {
            Some(ApiAccountInfo::Achievements) => {
                self.rx.account_state.write_with(|state| {
                    state.last_updated_achievements = ApiTimeZone::now();
                    state.data_update_available = Some(false);
                });
            },
            _ => (),
        }
    }

    pub(super) fn info_refresh(&mut self, endpoint: Option<ApiAccountInfo>) {
        let account = Self::current_account_token();
        let token_id = account
            .as_ref()
            .ok()
            .map(|acc| acc.id())
            .flatten()
            .map(ToOwned::to_owned);
        let endpoints = match endpoint {
            Some(..) => &[],
            None => ApiAccountInfo::DATA_ENDPOINTS,
        }
        .iter()
        .chain(endpoint.iter());
        for &endpoint in endpoints {
            self.process_refresh_account(endpoint, token_id.clone());
        }
        if endpoint.is_none() {
            let token_id = token_id.as_ref().map(|id| &id[..]);
            self.start_account_state_refresh(token_id);
        }
    }

    pub(super) fn process_refresh_account(&mut self, endpoint: ApiAccountInfo, token_id: Option<String>) {
        let client = {
            let token_id = token_id.as_ref().map(|id| &id[..]);
            self.client(token_id)
        }
        .cloned();
        self.inflight.spawn(Self::do_refresh_account(client, endpoint));
    }

    async fn do_refresh_account(
        client: anyhow::Result<Arc<ApiClient>>,
        endpoint: ApiAccountInfo,
    ) -> anyhow::Result<Option<ApiMessage>> {
        let context = || format!("Refreshing account {endpoint}");
        let client = client.with_context(context)?;
        match endpoint {
            ApiAccountInfo::Account => return Self::do_account_state_refresh(&client).await,
            ApiAccountInfo::RaidClears | ApiAccountInfo::Achievements => (),
        }
        let res = client
            .request_execute(Self::account_info_request(&client, endpoint)?)
            .await
            .with_context(context)?;
        let mut res = res.bytes_stream();

        let outpath = Self::account_info_path(&client.token.account_name, endpoint);
        if let Some(parent) = outpath.parent() {
            let _ = rt::log::warn_ok(fs::create_dir_all(parent).await.context("api account mkdir"));
        }
        let mut f = None;
        while let Some(buf) = res.next().await {
            let mut buf = buf.with_context(context)?;
            let f = match &mut f {
                f @ None => f.insert(fs::File::create(&outpath).await.with_context(context)?),
                Some(f) => f,
            };
            f.write_all_buf(&mut buf).await.with_context(context)?;
        }
        if let Some(f) = f {
            rt::log::error_ok(f.sync_all().await.with_context(context));
        }

        Ok(Some(ApiMessage::AccountInfoReload(Some(endpoint), true)))
    }

    /// TODO: this
    /// (assuming false as long as any data exists for now)
    pub fn api_info_outdated(&mut self, endpoint: ApiAccountInfo) -> bool {
        match endpoint {
            #[cfg(todo)]
            ApiAccountInfo::RaidClears => last_updated < start_of_week || recently_left_raid_map,
            #[cfg(todo)]
            ApiAccountInfo::Achievements =>
                last_updated < settings.achievement_update_period_id || recently_left_story_map,
            _ => self.auto_update_enabled(),
        }
    }

    pub fn account_info_path(account_name: &str, endpoint: ApiAccountInfo) -> PathBuf {
        let path = Self::account_path(account_name);
        path_join(path, endpoint.filename())
    }
    pub fn account_path(account_name: &str) -> PathBuf {
        let parent = rt::addon_dir().join("anet").join("account");
        match account_name {
            account_name if account_name.is_empty() => parent,
            account_name => parent.join(account_name),
        }
    }
    fn account_info_request(client: &ApiClient, endpoint: ApiAccountInfo) -> anyhow::Result<Request> {
        match endpoint {
            ApiAccountInfo::Achievements => client.client.new_request::<AccountAchievements>(()),
            ApiAccountInfo::RaidClears => client.client.new_request::<RaidEvent>(()),
            ApiAccountInfo::Account => client.client.new_request::<EndpointAccountState>(()),
        }
    }
    pub(super) async fn load_info_from(
        endpoint: ApiAccountInfo,
        path: PathBuf,
    ) -> anyhow::Result<Option<ApiMessage>> {
        let res = match endpoint {
            ApiAccountInfo::RaidClears => Self::api_load_info_from_raids(path)
                .await
                .map(ApiMessage::AccountInfoRaidClears),
            ApiAccountInfo::Achievements => Self::api_load_info_from_achievements(path)
                .await
                .map(ApiMessage::AccountInfoAchievements),
            ApiAccountInfo::Account => {
                // XXX: consider persisting this across game restart?
                return Ok(None)
            },
        }
        .with_context(|| format!("parsing account {endpoint}"));

        res.map(Some)
    }
    async fn api_load_info_from_achievements(path: PathBuf) -> anyhow::Result<AchievementState> {
        Self::deserialize_path::<achievements::serde_imp::achievement_state::AchievementApi>(&path)
            .await
            .map(Into::into)
    }
    async fn api_load_info_from_raids(path: PathBuf) -> anyhow::Result<RaidState> {
        let clears = Self::deserialize_path::<RaidEvent>(&path).await?;
        Ok(clears.into_iter().collect())
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, strum::IntoStaticStr, VariantArray)]
pub enum ApiAccountInfo {
    #[strum(serialize = "achievements")]
    Achievements,
    /// Weekly raid clears
    #[strum(serialize = "raids")]
    RaidClears,
    #[strum(serialize = "account")]
    Account,
}
impl ApiAccountInfo {
    pub const DATA_ENDPOINTS: &'static [Self] = &[Self::Achievements, Self::RaidClears];
    pub fn filename(self) -> impl fmt::Display + Into<String> {
        lazyfmt::MaybeFmt::new(move |f| write!(f, "{self}.json"))
    }
}
impl fmt::Display for ApiAccountInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.into())
    }
}

#[derive(Debug, Clone, Default)]
pub struct ApiAccountState {
    pub account_id: String,
    /// beware of catmander false negatives :<
    pub commander: bool,
    pub last_modified: ApiTimestamp,
    /// achievements only for now
    pub last_updated_achievements: ApiTimestamp,
    pub content_access: BTreeSet<String>,
    pub update_available: Option<bool>,
    pub data_update_available: Option<bool>,
}
impl ApiAccountState {
    pub fn is_empty(&self) -> bool {
        self.account_id().is_none()
    }

    pub fn account_id(&self) -> Option<&str> {
        str_opt_ref(&self.account_id)
    }
    pub fn last_modified(&self) -> Option<&ApiTimestamp> {
        (self.last_modified > ApiTimestamp::UNIX_EPOCH).then_some(&self.last_modified)
    }
    pub fn last_updated_achievements(&self) -> Option<&ApiTimestamp> {
        (self.last_updated_achievements > ApiTimestamp::UNIX_EPOCH)
            .then_some(&self.last_updated_achievements)
    }

    #[cfg(todo = "unused")]
    pub fn has_commander(&self) -> Option<bool> {
        self.commander.then_some(true)
    }

    pub const EXPIRY_MIN: Duration = Duration::from_secs(60 * 5 + 10);
    pub const EXPIRY_MAX: Duration = Duration::from_secs(Self::EXPIRY_MIN.as_secs() + 60 * 3);
    pub const EXPIRY_LAZY: Duration = Duration::from_secs(60 * 60);
    pub const EXPIRY_AUTO: Duration = Duration::from_secs(60 * 20);
    pub fn expiry(&self) -> ops::Range<ApiTimestamp> {
        let start = self.last_modified + Self::EXPIRY_MIN;
        let end = self.last_modified + Self::EXPIRY_MAX;
        start..end
    }
    pub fn check_expiry(&mut self, now: &ApiTimestamp) -> bool {
        let expiry = self.expiry();
        let update = match now {
            now if now > &expiry.end => Some(Some(true)),
            now if now >= &expiry.start => Some(None),
            _ => None,
        };
        let mut changed = false;
        if let Some(update) = update {
            changed = update != self.update_available;
            self.update_available = update;
        }
        if self.update_available != Some(false) {
            changed |= self.check_data_expiry(now.clone());
        }
        changed
    }
    pub fn check_data_expiry(&mut self, now: ApiTimestamp) -> bool {
        let last_modified = self.last_modified.max(now);
        let data_update = match self.last_updated_achievements() {
            Some(&updated) if updated + Self::EXPIRY_MAX <= last_modified => Some(Some(true)),
            Some(&updated) if updated + Self::EXPIRY_MIN <= last_modified => Some(None),
            Some(..) => Some(Some(false)),
            None if self.last_modified().is_some() => Some(Some(true)),
            _ => None,
        };
        if let Some(update) = data_update {
            let changed = update != self.data_update_available;
            self.data_update_available = update;
            changed
        } else {
            false
        }
    }
    pub fn next_expiry(&mut self, now: &ApiTimestamp) -> Option<ApiTimestamp> {
        let expiry = self.expiry();
        match now {
            now if now > &expiry.end => None,
            now if now >= &expiry.start => Some(expiry.end),
            _ => Some(expiry.start),
        }
    }
}

/// [Account] but less brittle
#[derive(Debug, Clone, Default, Deserialize)]
pub(super) struct EndpointAccountState {
    pub id: String,
    pub name: String,
    pub last_modified: String,
    pub commander: bool,
    pub access: BTreeSet<String>,
}
impl EndpointAccountState {
    pub fn access_display(access: &Access) -> impl fmt::Display {
        // not copy but may as well be...
        let access = access.clone();
        lazyfmt::MaybeFmt::new(move |f| access.serialize(f))
    }
}
impl Endpoint for EndpointAccountState {
    const AUTHENTICATED: bool = <Account as Endpoint>::AUTHENTICATED;
    const LOCALE: bool = <Account as Endpoint>::LOCALE;
    const URL: &'static str = <Account as Endpoint>::URL;
    const VERSION: &'static str = <Account as Endpoint>::VERSION;
}
impl FixedEndpoint for EndpointAccountState {}
impl From<Account> for EndpointAccountState {
    fn from(account: Account) -> Self {
        let access = account
            .access
            .into_iter()
            .map(|e| EndpointAccountState::access_display(&e).to_string());
        Self {
            id: account.id,
            name: account.name,
            last_modified: account.last_modified,
            commander: account.commander,
            access: access.collect(),
        }
    }
}

impl ApiMessage {
    pub fn account_reload_all() -> Self {
        Self::AccountInfoReload(None, false)
    }
    #[cfg(todo = "unused")]
    pub fn account_reload_endpoint(endpoint: ApiAccountInfo) -> Self {
        Self::AccountInfoReload(Some(endpoint), false)
    }
}
