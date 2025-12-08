use {
    crate::{controller::pathing::PathingEvent, render::machine::MumbleIdentityUpdate},
    taimi_meta::ui::GameplayState,
    tokio::sync::{mpsc, watch},
};

pub use self::festival::FestivalState;

mod festival;

#[derive(Debug, Clone)]
pub struct PathingSender {
    pub command: mpsc::Sender<PathingEvent>,
    pub festivals: watch::Sender<FestivalState>,
}
impl PathingSender {
    pub fn new(
        gameplay: watch::Receiver<GameplayState>,
        mumble_identity: watch::Receiver<Option<MumbleIdentityUpdate>>,
        festivals: watch::Sender<FestivalState>,
    ) -> (Self, PathingReceiver) {
        let (command, command_rx) = mpsc::channel(48);
        let sender = Self { command, festivals };
        let rx = PathingReceiver {
            command: command_rx,
            festivals: sender.festivals.subscribe(),
            gameplay,
            mumble_identity,
        };

        (sender, rx)
    }
}

pub struct PathingReceiver {
    pub command: mpsc::Receiver<PathingEvent>,
    pub festivals: watch::Receiver<FestivalState>,
    pub gameplay: watch::Receiver<GameplayState>,
    pub mumble_identity: watch::Receiver<Option<MumbleIdentityUpdate>>,
}
