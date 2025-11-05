use {
    crate::{EndpointWithId, Gw2ApiKey, Gw2BulkEndpoint, Gw2Endpoint, IdRange, Language},
    anyhow::anyhow,
    reqwest::{Client as HttpClient, ClientBuilder},
    serde::de::DeserializeOwned,
    std::{collections::HashMap, sync::Arc},
    tokio::sync::Semaphore,
};

pub struct ApiClient {
    client: HttpClient,
    api_key: Option<Gw2ApiKey>,
    semaphore: Arc<Semaphore>,
}

pub fn new_client() -> ClientBuilder {
    let name = "taimihud-api";
    let version = "0.0.1";
    let user_agent = format!("{name}/{version}");
    HttpClient::builder().user_agent(user_agent)
}

pub fn build_client() -> anyhow::Result<HttpClient> {
    new_client().build().map_err(Into::into)
}

pub enum ApiRequest {
    Ids,
    Single(u32),
    Multi(Vec<u32>),
    All,
}

impl ApiClient {
    const BASE: &str = "https://api.guildwars2.com";
    const SIMULTANEOUS_LIMIT: usize = 200;

    pub fn new() -> anyhow::Result<Self> {
        let client = build_client()?;
        let semaphore = Arc::new(Semaphore::new(Self::SIMULTANEOUS_LIMIT));
        Ok(Self { client, api_key: None, semaphore })
    }

    pub fn set_api_key(&mut self, api_key: Gw2ApiKey) {
        self.api_key = Some(api_key);
    }

    pub async fn request_bulk<T: Gw2BulkEndpoint>(&self, ids: impl IdRange) -> anyhow::Result<Vec<T>> {
        let mut params = HashMap::new();
        params.insert("v", T::VERSION.to_string());
        if T::AUTHENTICATED {
            let api_key = self
                .api_key
                .as_ref()
                .ok_or_else(|| anyhow!("Authenticated request required but no API key given"))?;
            params.insert("access_token", api_key.to_string());
        }
        if T::LOCALE {
            // TODO: detection of language or passing it up
            params.insert("lang", Language::En.to_string());
        }
        if let Some(k) = ids.id_key() {
            params.insert(k, ids.id_string_value());
        }
        let url = format!("{}/{}", Self::BASE, T::URL);
        let request = self.client.get(url).query(&params);
        log::debug!("{:?}", request);
        let response = request.send().await?.error_for_status()?.json().await?;
        Ok(response)
    }
    pub async fn request<T: Gw2Endpoint>(&self, ids: impl IdRange) -> anyhow::Result<T> {
        let mut params = HashMap::new();
        params.insert("v", T::VERSION.to_string());
        if T::AUTHENTICATED {
            let api_key = self
                .api_key
                .as_ref()
                .ok_or_else(|| anyhow!("Authenticated request required but no API key given"))?;
            params.insert("access_token", api_key.to_string());
        }
        if T::LOCALE {
            // TODO: detection of language or passing it up
            params.insert("lang", Language::En.to_string());
        }
        if let Some(k) = ids.id_key() {
            params.insert(k, ids.id_string_value());
        }
        let url = format!("{}/{}", Self::BASE, T::URL);
        let request = self.client.get(url).query(&params);
        log::debug!("{:?}", request);
        let response = request.send().await?.error_for_status()?.json().await?;
        Ok(response)
    }
    pub async fn request_ids<T: Gw2Endpoint + EndpointWithId>(&self) -> anyhow::Result<Vec<T::IdType>>
    where
        <T as EndpointWithId>::IdType: DeserializeOwned,
    {
        let mut params = HashMap::new();
        params.insert("v", T::VERSION.to_string());
        if T::AUTHENTICATED {
            let api_key = self
                .api_key
                .as_ref()
                .ok_or_else(|| anyhow!("Authenticated request required but no API key given"))?;
            params.insert("access_token", api_key.to_string());
        }
        if T::LOCALE {
            // TODO: detection of language or passing it up
            params.insert("lang", Language::En.to_string());
        }
        let url = format!("{}/{}", Self::BASE, T::URL);
        let request = self.client.get(url).query(&params);
        let response = request.send().await?.error_for_status()?.json().await?;
        Ok(response)
    }
}
