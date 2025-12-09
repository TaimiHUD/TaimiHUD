use {
    super::ApiController,
    crate::{
        exports::runtime as rt,
        settings::state::{BootstrapState, SavedApiToken},
    },
    anyhow::Context,
    reqwest::{header, Request, Response},
    serde::Serialize,
    std::{fmt, sync::Arc},
    taimi_api_client::{client::ApiClient as Client, model},
    taimi_hoard::lazyfmt,
    url::Url,
};

pub(super) struct ApiClient {
    pub client: Client,
    /// TODO: remove this and add the account id field or whatever we need
    pub token: SavedApiToken,
}

impl ApiClient {
    pub fn new(token: SavedApiToken) -> anyhow::Result<Self> {
        let client = crate::settings::source::new_client()
            .build()
            .context("building api client")?;
        let mut client = Client::with_client(client)?;
        client.set_api_key(token.token())?;
        if let Some(base_url) = rt::log::error_ok(Url::parse(token.base_url())) {
            client.base_url = base_url;
        }
        let locale = token
            .locale()
            .map(|locale| header::HeaderValue::from_str(locale))
            .transpose();
        if let Some(locale) = rt::log::warn_ok(locale) {
            client.locale = locale;
        }

        Ok(Self { client, token })
    }

    pub async fn request_execute(&self, req: Request) -> anyhow::Result<Response> {
        self.client
            .client
            .execute(req)
            .await
            .and_then(|res| res.error_for_status())
            .map_err(Into::into)
    }

    pub fn lib_permission_display(perm: model::authenticated::Permissions) -> impl fmt::Display {
        lazyfmt::MaybeFmt::new(move |f| perm.serialize(f))
    }

    pub fn matches_id(&self, token_id: Option<&'_ str>) -> bool {
        let Some(id) = token_id else { return true };
        &self.token.id[..] == id
    }
}

impl ApiController {
    pub(super) fn client<'a>(&'a mut self, id: Option<&'_ str>) -> anyhow::Result<&'a Arc<ApiClient>> {
        let client_ok = match &self.client {
            Some(c) if c.matches_id(id) => true,
            _ => false,
        };
        Ok(match (&mut self.client, client_ok) {
            (Some(storage), true) => &*storage,
            (storage, _) => {
                let acc = crate::ACCOUNT_NAME_CELL.get().map(|acc| &acc[..]).unwrap_or("");
                let token = BootstrapState::read_with(|s| {
                    id.and_then(|id| s.anet_api_token.iter().find(|token| token.id == *id))
                        .or_else(|| s.anet_api_token(acc))
                        .cloned()
                })
                .unwrap_or(SavedApiToken::UNAUTHENTICATED);
                let client = ApiClient::new(token)?;
                &*storage.insert(Arc::new(client))
            },
        })
    }
}
