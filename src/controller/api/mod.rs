use {
    crate::{settings::SettingsLock, Interruption},
    tokio::sync::mpsc::Receiver,
};

pub enum ApiMessage {
    Exit(Interruption),
}

pub struct ApiController {
    rx: Receiver<ApiMessage>,
    settings: SettingsLock,
}

impl ApiController {
    pub fn new(rx: Receiver<ApiMessage>, settings: SettingsLock) -> Self {
        Self { rx, settings }
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
}
