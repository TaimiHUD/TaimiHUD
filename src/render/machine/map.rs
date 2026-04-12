use {
    super::{RenderMachine, RenderUsers},
    crate::controller::markers::{MarkersController, MarkersEvent},
    std::time::Instant,
    taimi_meta::ui::{MapContext, MapOpen},
};

#[cfg(any(feature = "markers", feature = "space"))]
use crate::exports::runtime::bindings::{GameControl, GameControls};
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
                    Some(ts) => {
                        let when = self.latest_space_timestamp();
                        open.while_elapsed(when.saturating_duration_since(ts).as_secs_f32())
                    },
                    None => open,
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
            Some(..) => open.while_elapsed(0.0),
            None => open,
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
        if self.map_open && self.map_hidden {
            // TODO: move this to require pressing UI toggle while world map is open maybe?
            log::info!("UI toggle escape hatch - resetting hidden state due to world map");
            self.map_hidden = false;
        }

        #[cfg(feature = "markers")]
        if self.map_users.contains(RenderUsers::MARKERS) {
            MarkersController::try_send(MarkersEvent::UiMapOpened(self.get_map_open_state()));
        }
    }
    #[cfg(any(feature = "markers", feature = "space"))]
    pub fn is_ui_hidden(&self) -> bool {
        match self.map_hidden {
            #[cfg(feature = "space")]
            false if self.is_cutscene() => true,
            h => h,
        }
    }

    #[cfg(any(feature = "markers", feature = "space"))]
    pub fn is_map_visible(&self) -> Option<MapContext> {
        self.is_ingame().and_then(|_| {
            let context = self
                .get_map_open_state()
                // TODO: .primary_context() if no anims enabled
                .visible_context();
            match (context, self.is_ui_hidden()) {
                (MapContext::Minimap, true) => None,
                (context, _) => Some(context),
            }
        })
    }

    #[cfg(any(feature = "markers", feature = "space"))]
    pub fn act_map_recalibrate(&self, display_changed: bool) {
        #[cfg(feature = "space")]
        if display_changed && self.mumblelink_users.contains(RenderUsers::SPACE) {
            Engine::try_send(SpaceEvent::UiResize(self.display_size()));
        }
        MarkersController::try_send(MarkersEvent::UiResize(self.map.calibration.clone()));
    }

    #[cfg(any(feature = "markers", feature = "space"))]
    pub fn act_controls_changed(&mut self, controls_state: GameControls, controls_changed: GameControls) {
        let pressed = controls_state & controls_changed;
        if controls_changed.contains(GameControl::Map_OpenClose) {
            self.act_press_map_toggle(controls_state.contains(GameControl::Map_OpenClose));
        }
        if pressed.contains(GameControl::UI_ShowHideUI) {
            self.map_hidden ^= true;
        }
    }

    #[cfg(any(feature = "markers", feature = "space"))]
    pub fn act_press_map_toggle(&mut self, down: bool) {
        if !down {
            // ignore release event
            return
        }

        let changed = match self.get_map_open_state() {
            MapOpen::Open => self.set_map_open(MapOpen::Closing { elapsed: 0.0 }),
            // TODO: reconsider in case we're wrong?
            MapOpen::Opening { elapsed } if elapsed > 0.5 =>
                self.set_map_open(MapOpen::Closing { elapsed: 0.0 }),
            _ => false,
        };
        if changed {
            self.act_map_open();
        }
    }
}
