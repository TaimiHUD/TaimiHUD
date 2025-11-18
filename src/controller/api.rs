use anyhow::Context;
use serde::Serialize;
use crate::{controller::Controller, exports::runtime as rt, settings::{state::{BootstrapState, SavedApiToken}, SettingsLock}, Interruption};
use gw2lib_model::{Endpoint, FixedEndpoint};
use std::{fmt, future::Future, pin::Pin, sync::Arc};
use reqwest::{header, Client, Method, Request};
use tokio::{sync::mpsc, select, task::JoinSet};
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

    pub fn client(&mut self) -> anyhow::Result<&Arc<ApiClient>> {
        Ok(match &mut self.client {
            Some(c) => &*c,
            storage @ None => {
                let acc = crate::ACCOUNT_NAME_CELL.get().map(|acc| &acc[..])
                    .unwrap_or("");
                let token = BootstrapState::read_with(|s|
                    s.anet_api_token(acc).cloned()
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
            ApiMessage::Request(req) => {
                let fut = req(self.client()?);
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

        self.client = Some(Arc::new(client));

        Ok(())
    }

    async fn process_token_clear(&mut self) -> anyhow::Result<()> {
        let _ = self.client.take();

        Ok(())
    }
}

pub enum ApiMessage {
    TokenClear,
    TokenAdd(SavedApiToken),
    Request(Box<dyn FnOnce(&Arc<ApiClient>) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'static>> + Send + 'static>),
    Exit(Interruption),
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

    pub async fn request<E: FixedEndpoint + serde::de::DeserializeOwned>(&self) -> anyhow::Result<E> {
        let req = self.new_request::<E>()?;

        let context = |op: &'static str| move || format!("{op} API request {}", E::URL);
        let res = self.client.execute(req).await
            .and_then(|res| res.error_for_status())
            .with_context(context("sending"))?;
        res.json().await
            .with_context(context("decoding"))
    }

    pub fn lib_permission_display(perm: gw2lib_model::authenticated::Permissions) -> impl fmt::Display {
        rt::log::MaybeFmt::new(move |f| perm.serialize(f))
    }
}
