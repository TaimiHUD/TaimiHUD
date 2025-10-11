use {
    crate::controller::{Controller, ControllerEvent},
    std::time::Instant,
    super::{RenderMachine, RenderUsers},
    taimi_meta::ui::{
        MapContext, MapOpen,
    },
};
#[cfg(feature = "space")]
use crate::space::engine::{Engine, SpaceEvent};

impl RenderMachine {
    pub fn get_map_open(&self) -> MapOpen {
        match MapOpen::with_open(self.map_open) {
            #[cfg(todo = "unnecessary")]
            mut open => {
                let mut open = self.get_map_open_state();
                if let (Some(ts), Some(elapsed)) = (self.map_open_timestamp, open.elapsed_mut()) {
                    *elapsed = ts.elapsed().as_secs_f32();
                    open.cap(false);
                }
                open
            },
            open => {
                let open = match self.map_open_timestamp {
                    Some(ts) =>
                        open.while_elapsed(ts.elapsed().as_secs_f32()),
                    None =>
                        open,
                };
                match self.map_open {
                    true => open.cap(false),
                    false => open,
                }
            },
        }
    }

    pub fn get_map_open_state(&self) -> MapOpen {
        let open = MapOpen::with_open(self.map_open);
        match &self.map_open_timestamp {
            Some(..) =>
                open.while_elapsed(0.0),
            None =>
                open,
        }
    }

    pub fn set_map_open(&mut self, open: MapOpen) -> bool {
        if open.shape() != self.get_map_open_state().shape() {
            self.map_open = open.is_open();
            self.map_open_timestamp = open.elapsed().map(|e| match Instant::now() {
                #[cfg(todo = "unnecessary")]
                now => now.checked_sub(e).unwrap_or(now),
                now => now - e,
            });
            log::warn!("MAP OPEN CHANGED TO: {open:?}");
            true
        } else {
            #[cfg(todo = "unnecessary")]
            if let Some(ts) = self.map_open_timestamp {
                if ts.elapsed() >= MapOpen::max_duration(false) {
                    self.map_open_timestamp = None;
                }
            }
            false
        }
    }

    pub fn map_open(&mut self) -> MapOpen {
        let open = self.get_map_open();
        if !open.is_anim() {
            self.map_open_timestamp = None;
        }
        open
    }

    #[cfg(any(feature = "markers", feature = "space"))]
    pub fn act_map_open(&mut self) {
        #[cfg(feature = "markers")]
        if self.map_users.contains(RenderUsers::MARKERS) {
            Controller::try_send(ControllerEvent::UiMapOpened(self.get_map_open_state()));
        }
    }

    #[cfg(any(feature = "markers", feature = "space"))]
    pub fn is_map_visible(&self) -> Option<MapContext> {
        self.is_ingame().map(|_|
            self.get_map_open_state()
                // TODO: .primary_context() if no anims enabled
                .visible_context()
        )
    }

    #[cfg(any(feature = "markers", feature = "space"))]
    pub fn act_map_recalibrate(&self, display_changed: bool) {
        #[cfg(feature = "space")]
        if display_changed && self.mumblelink_users.contains(RenderUsers::SPACE) {
            Engine::try_send(SpaceEvent::UiResize(self.display_size()));
        }
        Controller::try_send(ControllerEvent::UiResize(self.map.calibration.clone()));
    }
}
