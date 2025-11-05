use {
    crate::{exports::runtime as rt, settings::SettingsLock, Interruption},
    anyhow::Context,
    taimi_api_client::{client::ApiClient, festivals::FestivalCategory},
    taimi_pack::attributes::Festival,
    tokio::sync::mpsc::Receiver,
};

pub type Gw2ApiKey = String;
pub enum ApiMessage {
    Exit(Interruption),
}

pub struct ApiController {
    client: Option<ApiClient>,
    rx: Receiver<ApiMessage>,
    settings: SettingsLock,
}

impl ApiController {
    pub fn new(rx: Receiver<ApiMessage>, settings: SettingsLock) -> Self {
        Self { rx, settings, client: None }
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        log::warn!("TODO: ApiController");
        while let Some(e) = self.rx.recv().await {
            match e {
                ApiMessage::Exit(_reason) => break,
            }
        }
        Ok(())
    }

    pub fn build_client(api_key: Option<Gw2ApiKey>) -> anyhow::Result<ApiClient> {
        ApiClient::new().context("initializing API client")
    }

    pub fn setup(&mut self, api_key: Option<Gw2ApiKey>) {
        if let Some(..) = api_key {
            log::info!("API key was provided, authenticated endpoints will be available.");
        } else {
            log::warn!("No API key provided, only unauthenticated endpoints will be available.");
        }
        if let Some(client) = rt::log::error_ok(Self::build_client(api_key)) {
            self.client.insert(client);
        }
    }

    pub fn current_festival(&self) -> Option<Festival> {
        todo!()
    }
}
