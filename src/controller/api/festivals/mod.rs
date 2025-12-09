use {super::ApiController, crate::controller::Controller};

pub use self::{schedule::FestivalWindow, state::FestivalState};

mod schedule;
mod state;

impl ApiController {
    pub fn active_festivals() -> Option<FestivalState> {
        Controller::with_sender(|s| s.api.as_ref().map(|a| a.festivals.borrow().clone())).flatten()
    }
}
