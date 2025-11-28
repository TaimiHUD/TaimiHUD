use anyhow::Context;
use serde::Serialize;
use tokio_stream::StreamExt;
use crate::{controller::Controller, exports::runtime as rt, settings::{state::{BootstrapState, SavedApiToken}, SettingsLock}, Interruption};
use gw2lib_model::{Endpoint, FixedEndpoint};
use std::{fmt, future::Future, path::PathBuf, pin::Pin, sync::Arc};
use reqwest::{header, Client, Method, Request, Response};
use tokio::{fs, io::AsyncWriteExt, select, sync::mpsc, task::JoinSet};
use url::Url;

pub struct ApiController {
    settings: SettingsLock,
    active: bool,
    rx: mpsc::Receiver<ApiMessage>,
    client: Option<Arc<ApiClient>>,
    inflight: JoinSet<anyhow::Result<()>>,
}

impl ApiController {
    pub fn new(rx: mpsc::Receiver<ApiMessage>, settings: SettingsLock) -> Self {
        Self {
            settings,
            rx,
            active: true,
            client: None,
            inflight: Default::default(),
        }
    }

    pub fn client<'a>(&'a mut self, id: Option<&'_ str>) -> anyhow::Result<&'a Arc<ApiClient>> {
        let client_ok = match &self.client {
            Some(c) if c.matches_id(id) => true,
            _ => false,
        };
        Ok(match (&mut self.client, client_ok) {
            (Some(storage), true) => &*storage,
            (storage, _) => {
                let acc = crate::ACCOUNT_NAME_CELL.get().map(|acc| &acc[..])
                    .unwrap_or("");
                let token = BootstrapState::read_with(|s|
                    id.and_then(|id| s.anet_api_token.iter().find(|token| token.id == *id))
                        .or_else(|| s.anet_api_token(acc)).cloned()
                ).unwrap_or(SavedApiToken::UNAUTHENTICATED);
                let client = ApiClient::new(token)?;
                &*storage.insert(Arc::new(client))
            },
        })
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

    pub async fn setup(&mut self) {
    }

    pub async fn turn(&mut self) -> Option<Interruption> {
        if self.rx.is_closed() {
            return Some(self.exit_drain())
        }
        select! {
            m = self.rx.recv() => match m {
                None =>
                    return Some(Interruption::Unspecified),
                Some(m) => match self.handle_message(m).await.context("API controller") {
                    Ok(Some(int)) => {
                        self.rx.close();
                        return Some(int)
                    },
                    Ok(None) => (),
                    Err(e) => {
                        log::error!("{e:#}");
                    },
                },
            },
            Some(res) = self.inflight.join_next(), if !self.inflight.is_empty() => match res {
                Ok(res) => drop(rt::log::error_ok(res.context("api request task"))),
                Err(e) => crate::log_join_error("api", e),
            },
        }
        None
    }

    fn exit_drain(&mut self) -> Interruption {
        while let Ok(e) = self.rx.try_recv() {
            match e {
                ApiMessage::Exit(reason) =>
                    return reason,
                _ => (),
            }
        }
        Interruption::Unspecified
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
            ApiMessage::TokenClear => self.process_token_clear().await,
            ApiMessage::TokenAdd(token) => self.process_token_add(token).await
                .context("adding API token"),
            ApiMessage::TokenRemove(id) => Ok(self.process_token_remove(id).await),
            ApiMessage::RefreshAccount { endpoint, token_id } => self.process_refresh_account(endpoint, token_id).await
                .context("API refresh"),
            ApiMessage::Request(req) => {
                let fut = req(self.client(None)?);
                self.inflight.spawn(fut);
                Ok(())
            },
        }.map(|()| None)
    }

    async fn process_token_add(&mut self, token: SavedApiToken) -> anyhow::Result<()> {
        let mut client = ApiClient::new(token)?;

        if client.token.token().is_some() && client.token.id().is_none() {
            let info = rt::log::error_ok(client.request::<gw2lib_model::authenticated::Tokeninfo>().await);
            if let Some(info) = info {
                client.token.id = info.id;
                client.token.name = info.name;
                client.token.permissions = info.permissions.into_iter().map(|p|
                    ApiClient::lib_permission_display(p).to_string()
                ).collect();
            }
        }

        if client.token.token().is_some() && client.token.account_id().is_none() {
            let account = rt::log::error_ok(client.request::<gw2lib_model::authenticated::account::Account>().await);
            if let Some(account) = account {
                client.token.account_id = account.id;
                client.token.account_name = account.name;
            }
        }

        let client = &*self.client.insert(Arc::new(client));

        if client.token.token().is_some() {
            log::debug!("TODO: commit token elsewhere?");
            BootstrapState::write_with(|state| {
                let out = state.anet_api_token_mut(|t|
                    t.account_id == client.token.account_id
                    //|| t.id == client.token.id
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
        let is_current_token = self.client.as_ref()
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

    async fn process_refresh_account(&mut self, endpoint: ApiAccountInfo, token_id: Option<String>) -> anyhow::Result<()> {
        let client = {
            let token_id = token_id.as_ref().map(|id| &id[..]);
            self.client(token_id)
        }?;
        let context = || format!("Refreshing account {endpoint}");
        let res = client.request_execute(
            Self::account_info_request(client, endpoint)?
        ).await.with_context(context)?;
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

        Ok(())
    }

    pub fn current_account_token() -> Result<SavedApiToken, Option<String>> {
        let account_name = crate::ACCOUNT_NAME_CELL.get()
            .map(|acc| &acc[..]);
        let token = BootstrapState::read_with(|s|
            s.anet_api_token(account_name.unwrap_or(""))
                .and_then(|token| token.account_name()
                    .map(|_| token.clone())
                )
        );
        match token {
            Some(token) if account_name.is_none() || account_name == Some(&token.account_name[..]) =>
                Ok(token),
            _ => Err(account_name.map(ToOwned::to_owned)),
        }
    }

    pub fn account_path(account_name: &str) -> PathBuf {
        let parent = rt::addon_dir()
            .join("anet")
            .join("account");
        match account_name {
            account_name if account_name.is_empty() =>
                parent,
            account_name =>
                parent.join(account_name),
        }
    }
    pub fn account_info_path(account_name: &str, endpoint: ApiAccountInfo) -> PathBuf {
        Self::account_path(account_name)
            .join(format!("{endpoint}.json"))
    }
    pub fn account_info_request(client: &ApiClient, endpoint: ApiAccountInfo) -> anyhow::Result<Request> {
        match endpoint {
            ApiAccountInfo::Achievements =>
                client.new_request::<gw2lib_model::authenticated::account::achievements::AccountAchievements>(),
            ApiAccountInfo::RaidClears =>
                client.new_request::<gw2lib_model::authenticated::account::raids::RaidEvent>(),
        }
    }
}

pub enum ApiMessage {
    TokenClear,
    TokenAdd(SavedApiToken),
    TokenRemove(String),
    RefreshAccount {
        endpoint: ApiAccountInfo,
        token_id: Option<String>,
    },
    Request(Box<dyn FnOnce(&Arc<ApiClient>) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'static>> + Send + 'static>),
    Exit(Interruption),
}
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, strum::IntoStaticStr, strum::VariantArray)]
pub enum ApiAccountInfo {
    #[strum(serialize = "achievements")]
    Achievements,
    /// Weekly raid clears
    #[strum(serialize = "raids")]
    RaidClears,
}
impl fmt::Display for ApiAccountInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.into())
    }
}

impl ApiMessage {
    pub fn try_send(self) {
        Controller::with_sender(|s| s.api_try_send(self));
    }
    pub fn for_request<Fut, F: FnOnce(&Arc<ApiClient>) -> Fut>(f: F) -> Self where
        F: Send + 'static,
        Fut: Future<Output = anyhow::Result<()>> + Send + 'static,
    {
        Self::Request(Box::new(move |client| {
            let fut = f(client);
            Box::pin(fut) as Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'static>>
        }))
    }
}

pub struct ApiClient {
    pub client: Client,
    pub token: SavedApiToken,
    pub base_url: Url,
    pub authorization: Option<header::HeaderValue>,
    pub locale: Option<header::HeaderValue>,
}

impl ApiClient {
    pub fn new(token: SavedApiToken) -> anyhow::Result<Self> {
        let client = crate::settings::source::new_client()
            .build().context("building api client")?;
        let base_url = Url::parse(token.base_url())
            .context("api base url")?;
        let authorization = {
            let mut auth = token.token().map(|token|
                header::HeaderValue::from_str(&format!("Bearer {token}"))
            ).transpose()?;
            if let Some(auth) = &mut auth {
                auth.set_sensitive(true);
            }
            auth
        };
        let locale = token.locale().map(|locale|
            header::HeaderValue::from_str(locale)
        ).transpose()?;

        Ok(Self {
            client,
            token,
            base_url,
            authorization,
            locale,
        })
    }

    /// https://wiki.guildwars2.com/wiki/API:2#Schemas
    ///
    /// https://api.guildwars2.com/v2.json?v=latest
    pub const V2_SCHEMA_VERSION: &'static str = "2025-08-29T01:00:00.000Z";
    /// Query argument `?v=`[Self::V2_SCHEMA_VERSION]
    pub const V2_SCHEMA_KEY: &'static str = "v";
    /// [Self::V2_SCHEMA_KEY] recommended instead due to
    /// [header being ignored sometimes?](https://github.com/gw2-api/issues/issues/106)
    pub const V2_SCHEMA_HEADER: header::HeaderName = header::HeaderName::from_static("x-schema-version");
    pub fn new_request<E: Endpoint>(&self) -> anyhow::Result<Request> {
        let mut url = self.base_url.join(E::URL)?;
        url.query_pairs_mut().append_pair(Self::V2_SCHEMA_KEY, E::VERSION);
        let mut req = Request::new(Method::GET, url);

        match (E::AUTHENTICATED, &self.authorization) {
            (true, None) =>
                anyhow::bail!("API token required for {}", E::URL),
            (true, Some(token)) => {
                req.headers_mut().insert(header::AUTHORIZATION, token.clone());
            },
            (false, _) => (),
        }
        if let (true, Some(locale)) = (E::LOCALE, &self.locale) {
            req.headers_mut().insert(header::ACCEPT_LANGUAGE, locale.clone());
        }

        Ok(req)
    }

    pub async fn request_execute(&self, req: Request) -> anyhow::Result<Response> {
        self.client.execute(req).await
            .and_then(|res| res.error_for_status())
            .map_err(Into::into)
    }
    pub async fn request<E: FixedEndpoint + serde::de::DeserializeOwned>(&self) -> anyhow::Result<E> {
        let req = self.new_request::<E>()?;

        let context = |op: &'static str| move || format!("{op} API request {}", E::URL);
        let res = self.request_execute(req).await
            .with_context(context("sending"))?;
        res.json().await
            .with_context(context("decoding"))
    }

    pub fn lib_permission_display(perm: gw2lib_model::authenticated::Permissions) -> impl fmt::Display {
        rt::log::MaybeFmt::new(move |f| perm.serialize(f))
    }

    pub fn matches_id(&self, token_id: Option<&'_ str>) -> bool {
        let Some(id) = token_id else { return true };
        &self.token.id[..] == id
    }
}
