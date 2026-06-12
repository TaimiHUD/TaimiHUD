use {
    crate::{
        controller::{api::FestivalState, pathing::PathingEvent},
        render::machine::MumbleIdentityUpdate,
    },
    taimi_meta::ui::GameplayState,
    tokio::sync::{mpsc, watch},
};

#[derive(Debug, Clone)]
pub struct PathingSender {
    pub command: mpsc::Sender<PathingEvent>,
    pub enables: watch::Sender<PathingEnables>,
}
impl PathingSender {
    pub fn new(
        gameplay: watch::Receiver<GameplayState>,
        mumble_identity: watch::Receiver<Option<MumbleIdentityUpdate>>,
        festivals: &watch::Sender<FestivalState>,
    ) -> (Self, PathingReceiver) {
        let (command, command_rx) = mpsc::channel(48);
        let sender = Self {
            command,
            enables: watch::Sender::new(PathingEnables::empty()),
        };
        let rx = PathingReceiver {
            command: command_rx,
            festivals: festivals.subscribe(),
            gameplay,
            mumble_identity,
            enables: sender.enables.clone(),
        };

        (sender, rx)
    }
}

pub struct PathingReceiver {
    pub command: mpsc::Receiver<PathingEvent>,
    pub enables: watch::Sender<PathingEnables>,
    pub festivals: watch::Receiver<FestivalState>,
    pub gameplay: watch::Receiver<GameplayState>,
    pub mumble_identity: watch::Receiver<Option<MumbleIdentityUpdate>>,
}

bitflags::bitflags! {
    #[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct PathingEnables: u8 {
        const KATRENDER = 0x01;
        const API_BYPASS = 0x02;
        #[cfg(feature = "paths-lua")]
        const SCRIPTING_LUA = 0x04;
        #[cfg(feature = "paths-lua")]
        const SCRIPTING_UNSECURED = 0x08;
    }
}
