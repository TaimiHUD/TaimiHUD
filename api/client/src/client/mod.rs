use {
    crate::{
        Endpoint,
        EndpointWithId,
        FixedEndpoint,
        Gw2BulkEndpoint,
        Gw2Endpoint,
        IdRange,
        Language,
        RequestAll,
        RequestIds,
        RequestOne,
    },
    anyhow::Context,
    reqwest::{header, Client as HttpClient, ClientBuilder, Method, Request},
    serde::de::DeserializeOwned,
    std::{fmt, sync::Arc},
    tokio::sync::Semaphore,
    url::Url,
};

pub struct ApiClient {
    pub client: HttpClient,
    authorization: Option<header::HeaderValue>,
    pub locale: Option<header::HeaderValue>,
    pub base_url: Url,
    semaphore: Arc<Semaphore>,
}

pub fn new_client() -> ClientBuilder {
    let name = env!("CARGO_PKG_NAME");
    let version = env!("CARGO_PKG_VERSION");
    let user_agent = format!("{name}/{version}");
    HttpClient::builder().user_agent(user_agent)
}

pub fn build_client() -> anyhow::Result<HttpClient> {
    new_client().build().map_err(Into::into)
}

impl ApiClient {
    const SIMULTANEOUS_LIMIT: usize = 200;

    pub fn new() -> anyhow::Result<Self> {
        let client = build_client()?;
        Self::with_client(client)
    }
    pub fn with_client(client: HttpClient) -> anyhow::Result<Self> {
        let semaphore = Arc::new(Semaphore::new(Self::SIMULTANEOUS_LIMIT));
        let base_url = Url::parse(Self::BASE)?;
        Ok(Self {
            client,
            authorization: None,
            locale: None,
            base_url,
            semaphore,
        })
    }

    pub fn set_api_key<T: fmt::Display>(&mut self, api_key: Option<T>) -> anyhow::Result<()> {
        self.authorization = {
            let mut auth = api_key
                .map(|token| header::HeaderValue::from_str(&format!("Bearer {token}")))
                .transpose()?;
            if let Some(auth) = &mut auth {
                auth.set_sensitive(true);
            }
            auth
        };
        Ok(())
    }
    pub fn set_locale(&mut self, language: Language) {
        let locale = header::HeaderValue::from_str(&language.to_string())
            .with_context(|| format!("locale {language} not valid header, this should never fail!"));
        if let Err(e) = &locale {
            log::error!("{e:#}");
        }
        self.locale = locale.ok();
    }

    pub fn new_request<T: Endpoint>(&self, ids: impl IdRange) -> anyhow::Result<Request> {
        let mut req = Request::new(Method::GET, self.base_url_for::<T>());
        self.setup_request_for::<T>(&mut req)?;
        if let Some(k) = ids.id_key() {
            req.url_mut()
                .query_pairs_mut()
                .append_pair(k, &ids.id_string_value());
        }

        Ok(req)
    }
    pub async fn request_all<T: Gw2BulkEndpoint>(&self) -> anyhow::Result<Vec<T>> {
        self.request_bulk::<T>(RequestAll::<T>::new()).await
    }
    fn error_context<E: Endpoint>() -> impl Fn(&'static str) -> String {
        move |op| format!("{op} API request {}", E::URL)
    }
    pub async fn request_bulk<T: Gw2BulkEndpoint>(&self, ids: impl IdRange) -> anyhow::Result<Vec<T>> {
        if !T::ALL && ids.id_is_all() {
            anyhow::bail!("endpoint {} does not support ids=all", T::URL)
        }

        let req = self.new_request::<T>(ids)?;
        log::debug!("{:?}", req);
        let context = Self::error_context::<T>();
        let response = self
            .client
            .execute(req)
            .await
            .with_context(|| context("sending"))?;
        let response = response
            .error_for_status()?
            .json()
            .await
            .with_context(|| context("decoding"))?;
        Ok(response)
    }

    pub async fn request_one<T: Gw2BulkEndpoint>(&self, id: &T::IdType) -> anyhow::Result<T> {
        let req = self.new_request::<T>(RequestOne::<T>::from_ref(id))?;
        log::debug!("{:?}", req);
        let context = Self::error_context::<T>();
        let response = self
            .client
            .execute(req)
            .await
            .with_context(|| context("sending"))?;
        let [response] = response
            .error_for_status()?
            .json::<[T; 1]>()
            .await
            .with_context(|| context("decoding"))?;
        Ok(response)
    }
    pub async fn request<T: Gw2Endpoint>(&self, ids: impl IdRange) -> anyhow::Result<T> {
        if ids.id_is_multiple() {
            let key = ids.id_key().unwrap_or("");
            anyhow::bail!(
                "endpoint {}?{key}={} used without request_bulk",
                T::URL,
                ids.id_display_value()
            )
        }
        let req = self.new_request::<T>(ids)?;
        log::debug!("{:?}", req);
        let context = Self::error_context::<T>();
        let response = self
            .client
            .execute(req)
            .await
            .with_context(|| context("sending"))?;
        let response = response
            .error_for_status()?
            .json()
            .await
            .with_context(|| context("decoding"))?;
        Ok(response)
    }
    pub async fn request_fixed<T: Gw2Endpoint + FixedEndpoint>(&self) -> anyhow::Result<T> {
        let req = self.new_request::<T>(())?;
        log::debug!("{:?}", req);
        let context = Self::error_context::<T>();
        let response = self
            .client
            .execute(req)
            .await
            .with_context(|| context("sending"))?;
        let response = response
            .error_for_status()?
            .json()
            .await
            .with_context(|| context("decoding"))?;
        Ok(response)
    }
    pub async fn request_ids<T: Gw2Endpoint + EndpointWithId>(&self) -> anyhow::Result<Vec<T::IdType>>
    where
        <T as EndpointWithId>::IdType: DeserializeOwned,
    {
        let req = self.new_request::<T>(RequestIds::<T>::new())?;
        log::debug!("{:?}", req);
        let context = Self::error_context::<T>();
        let response = self
            .client
            .execute(req)
            .await
            .with_context(|| context("sending"))?;
        let response = response
            .error_for_status()?
            .json()
            .await
            .with_context(|| context("decoding"))?;
        Ok(response)
    }

    const BASE: &str = "https://api.guildwars2.com";
    /// <https://wiki.guildwars2.com/wiki/API:2#Schemas>
    ///
    /// <https://api.guildwars2.com/v2.json?v=latest>
    pub const V2_SCHEMA_VERSION: &'static str = "2025-08-29T01:00:00.000Z";
    /// Query argument `?v=`[Self::V2_SCHEMA_VERSION]
    pub const V2_SCHEMA_KEY: &'static str = "v";
    /// [Self::V2_SCHEMA_KEY] recommended instead due to
    /// [header being ignored sometimes?](https://github.com/gw2-api/issues/issues/106)
    pub const V2_SCHEMA_HEADER: header::HeaderName = header::HeaderName::from_static("x-schema-version");

    pub fn base_url_for<T: Endpoint>(&self) -> Url {
        let url = self
            .base_url
            .join(T::URL)
            .with_context(|| format!("url join for endpoint {}, this should never fail!", T::URL));

        match url {
            Ok(url) => url,
            Err(e) => {
                log::error!("{e:#}");
                // return something nonsensical, we're off the rails here
                self.base_url.clone()
            },
        }
    }

    pub fn setup_request_for<T: Endpoint>(&self, req: &mut Request) -> anyhow::Result<()> {
        req.url_mut()
            .query_pairs_mut()
            .append_pair(Self::V2_SCHEMA_KEY, T::VERSION);
        self.set_headers_for::<T>(req.headers_mut())?;
        Ok(())
    }
    pub fn set_headers_for<T: Endpoint>(&self, headers: &mut header::HeaderMap) -> anyhow::Result<()> {
        match (T::AUTHENTICATED, &self.authorization) {
            (true, None) => anyhow::bail!("API token required for {}", T::URL),
            (true, Some(token)) => {
                headers.insert(header::AUTHORIZATION, token.clone());
            },
            (false, _) => (),
        }
        if let (true, Some(locale)) = (T::LOCALE, &self.locale) {
            headers.insert(header::ACCEPT_LANGUAGE, locale.clone());
        }
        Ok(())
    }
}
