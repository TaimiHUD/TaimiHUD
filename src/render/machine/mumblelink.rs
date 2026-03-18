#[cfg(any(feature = "markers", feature = "space"))]
pub use nexus::event::MumbleIdentityUpdate;
#[cfg(any(feature = "markers", feature = "space"))]
use {
    crate::render::machine::RenderPosition,
    taimi_meta::ui::{realign_fov, MapOpen},
};
use {
    crate::{
        controller::{timers::TimersController, Controller, ControllerEvent},
        exports::runtime::{self as rt, MumblePtr},
        render::machine::{RenderMachine, RenderPositioning, RenderUsers},
        MarkersController,
    },
    core::{num::NonZero, ptr},
    glamour::{Point3, Vector2, Vector3},
    std::time::{Duration, Instant},
    taimi_meta::{
        coords::{LocalSpace, SignObtainer},
        ui::{gameplay::GameplayState, UiState},
    },
};

impl RenderMachine {
    pub const LOCAL_UP: Vector3<LocalSpace> = Vector3::Y;
    pub const LOCAL_FORWARD: Vector3<LocalSpace> = Vector3::Z;

    pub fn last_ui_tick(&self) -> MumblelinkTick {
        match &self.mumblelink_frame_player {
            &Some((player_tick, ..)) if player_tick == self.mumblelink_frame =>
                MumblelinkTick::with_player_tick(player_tick),
            _ => MumblelinkTick::Ui(self.mumblelink_frame),
        }
    }

    pub fn ui_tick(&self) -> Option<MumblelinkTick> {
        if self.mumblelink_frame == 0 || self.mumblelink_frame_skip > 0 {
            return None
        }

        Some(self.last_ui_tick())
    }

    pub fn get_player_mumblelink(&self) -> Option<RenderPositioning> {
        match self.mumblelink_player.1.x.is_infinite()
            || rt::vec_eq(self.mumblelink_player.1, Vector3::ZERO)
        {
            true => None,
            false => Some(self.mumblelink_player),
        }
    }

    #[cfg(feature = "space")]
    pub fn get_camera_mumblelink(&self) -> Option<RenderPosition> {
        match self.mumblelink_camera.1.x.is_infinite()
            || rt::vec_eq(self.mumblelink_camera.1, Vector3::ZERO)
        {
            true => None,
            false => Some(match (self.mumblelink_camera_frame.wrapping_sub(self.mumblelink_camera_prev_frame), self.mumblelink_frame_skip) {
                (jump @ 2..=3, skip) | (jump @ 1..=4, skip @ 1..=3) => {
                    let (pos0, front0) = self.mumblelink_camera_prev;
                    let (pos1, front1, up) = self.mumblelink_camera;
                    let interp_ahead = Self::CAMERA_SMOOTHING_PER_FRAME * 0.5;
                    //let interp_ahead = 0.38f32;
                    let interp_ahead = 0.28f32;
                    let jump_back = interp_ahead;
                    let skip_ahead = interp_ahead * skip as f32;
                    let skip_ahead = interp_ahead * (skip as f32 + /*0.75f32*/ 0.5f32)/*.min(0.45f32)*/ - jump/*.saturating_sub(1)*/ as f32 * jump_back;
                    let scale = 1.0f32 / jump as f32;
                    let scale_ahead = skip_ahead * scale;

                    /*let prev_skip = jump/*.saturating_sub(1)*/ as f32 + skip.saturating_sub(1) as f32;
                    let target = (1.0f32 - 0.5f32 * scale + prev_skip * scale * 0.1f32).min(1.0f32);
                    let factor = target + scale_ahead;*/
                    let factor = 1.0f32 + scale_ahead;
                    // jump=1 => target=prev+1.0 or pos0+1.0 or pos1+0
                    // skip=1 => target=prev+0.5 or pos0+1.5 or pos1+0.5
                    // jump=2 => target=prev+0.2 or pos0+0.7 or pos1-0.3
                    // jump=2 skip=1 => target=prev+0.5 or pos0+2.5 or pos1+0.5/2
                    //let factor = target - jump.saturating_sub(1) as f32 * Self::CAMERA_SMOOTHING_PER_FRAME;
                    let pos = pos0.lerp(pos1, factor);
                    let front = front0.slerp(front1, factor);
                    (pos, front.normalize(), up)
                },
                _ => self.mumblelink_camera,
            })
        }
    }
    #[cfg(feature = "space")]
    pub(super) fn lastminute_mumblelink_update(&mut self) {
        let Ok(ml) = rt::mumble_link_ptr() else { return };
        let tick = ml.read_ui_tick();
        if tick == 0 || tick == self.mumblelink_frame {
            return
        }
        self.mumblelink_camera = Self::read_camera_mumblelink(ml);
        self.mumblelink_camera_frame = tick;
        // TODO: bleh...
        self.mumblelink_frame_skip = 0;
    }

    pub fn act_mumblelink_tick(&mut self, ml: MumblePtr) -> Option<GameplayState> {
        //log::warn!("ml tick {}", ml.read_ui_tick());
        let (ui_state, map_id) = match () {
            #[cfg(todo = "unnecessary")]
            _ => unsafe {
                let context = &raw const (*ml.as_ptr()).context;
                let map_id = ptr::read_volatile(&raw const (*context).map_id);
                let ui_state = ptr::read_volatile(&raw const (*context).ui_state as *const UiState);
                let map_id = UiState::from(ml.read_ui_state());
            },
            _ => (UiState::from(ml.read_ui_state()), ml.read_map_id()),
        };
        let ui_state_changes = self.mumblelink_state ^ ui_state;
        self.mumblelink_state = ui_state;

        #[cfg(any(feature = "markers", feature = "space"))]
        let identity = if !self.identity_users.is_empty() || !self.map_users.is_empty() {
            let update = match self.identity.update(&mut self.identity_changes, &ml) {
                Some(true) if !self.identity.is_empty() => Some(&self.identity.identity),
                _ => None,
            };

            #[cfg(any(feature = "markers", feature = "space"))]
            if let Some(update) = update.cloned() {
                Controller::with_sender(|s| {
                    if let Some(tx) = s.mumble_identity.as_ref() {
                        tx.send_replace(Some(update));
                    }
                });
            }

            update
        } else {
            None
        };

        #[cfg(any(feature = "markers", feature = "space"))]
        if !self.map_users.is_empty() {
            let id_update = if let Some(identity) = identity {
                self.map
                    .calibration
                    .update_from_mumblelink_identity_nexus(identity)
            } else {
                false
            };
            self.map.update_from_mumblelink(ml);

            if id_update {
                self.act_map_recalibrate(false);
            }
        }

        #[cfg(any(feature = "markers", feature = "space"))]
        if let (Some(update), true) = (identity, self.map_users.contains(RenderUsers::SPACE)) {
            let fov_y = realign_fov(update.fov);
            #[cfg(todo)]
            let changed = fov_y.to_bits() != self.fov.y.to_bits();
            self.set_fov(Vector2::ZERO.with_y(fov_y));
        }

        if !self.mumblelink_users.is_empty() {
            //log::warn!("TODO: act_mumblelink_tick");
        }

        #[cfg(any(feature = "markers", feature = "space"))]
        if ui_state_changes.contains(UiState::MapOpen) {
            let open = MapOpen::with_state_event(ui_state);
            if self.set_map_open(open) {
                self.act_map_open();
            }
        }

        let avatar = unsafe { &raw const (*ml.as_ptr()).avatar };
        let front = Vector3::from_array(unsafe { ptr::read_volatile(&raw const (*avatar).front) });
        let playpos = match rt::vec_eq(front, Vector3::ZERO) {
            true => Point3::INFINITY,
            false => Point3::from_array(unsafe { ptr::read_volatile(&raw const (*avatar).position) }),
        };
        let playpos_ticked = match rt::vec_eq(self.mumblelink_player.0, playpos) {
            // meaningless unless UI tick signifies we're actually ingame...
            false if self.mumblelink_frame_skip > 0 => false,
            false if rt::vec_eq(playpos, Point3::INFINITY) => {
                log::debug!("ML lost playpos?");
                true
            },
            eq => !eq,
        };
        if playpos_ticked {
            self.mumblelink_player = (playpos, front);
            self.mumblelink_frame_player = Some((self.mumblelink_frame, Instant::now()));
        }

        if playpos_ticked {
            if !crate::built_info::IS_TAGGED_VERSION {
                let up = Vector3::<LocalSpace>::from_array(unsafe {
                    ptr::read_volatile(&raw const (*avatar).top)
                });
                if !rt::vec_eq(up, Vector3::ZERO) {
                    log::info!("Whoa, MumbleLink actually populates player_up ({up:?})? Unthinkable!");
                }
            }
            #[cfg(feature = "space")]
            {
                self.map_sign.update(playpos, self.map.player_pos);
                if self.map_info.is_none() {
                    self.map
                        .calibration
                        .set_offset(self.map_sign.bounds.center(), self.map_sign.global.center());
                }
                match (self.map_sign.get_scale(), &self.map_info) {
                    (Some(scale), None) => {
                        self.map.calibration.local_space =
                            SignObtainer::is_significant(scale).then_some(scale);
                    },
                    _ => (),
                }
            }
        }

        let _camera_update = if !self.mumblelink_users.is_empty() {
            let camera = Self::read_camera_mumblelink(ml);
            let camera_dirty = !rt::vec_eq(self.mumblelink_camera.0, camera.0)
                || !rt::vec_eq(self.mumblelink_camera.1, camera.1);
            if self.mumblelink_camera_frame == self.mumblelink_frame {
                // lastminute updated, ignore
            } else /*if camera_dirty || playpos_ticked*/ {
                self.mumblelink_camera_prev = (self.mumblelink_camera.0, self.mumblelink_camera.1);
                self.mumblelink_camera_prev_frame = self.mumblelink_camera_frame;
            }
            self.mumblelink_camera_frame = self.mumblelink_frame;
            self.mumblelink_camera = camera;
            camera_dirty.then_some(())
        } else {
            None
        };

        let map_id_update = match map_id {
            0 => None,
            map_id
                if playpos_ticked
                    && (self.mumblelink_map != map_id
                        || matches!(self.gameplay, GameplayState::Intermission { initial: false, .. })) =>
                Some(self.mumblelink_map),
            _ => None,
        };
        if let Some(map_id) = map_id_update {
            self.mumblelink_map = map_id;
        }

        let tick_notable = ui_state_changes
            .intersects(MarkersController::MARKERS_NOTABLE_STATE | TimersController::TIMERS_NOTABLE_STATE)
            || map_id_update.is_some();
        if tick_notable || playpos_ticked {
            Controller::try_send(ControllerEvent::UiTick(self.last_ui_tick()));
        }

        let gameplay_update = match map_id_update {
            None => None,
            Some(..) if map_id == 0 => None,
            Some(_prev_map_id) => Some(GameplayState::new_ingame(map_id)),
        };

        gameplay_update
    }

    fn next_mumblelink_tick(&mut self) -> rt::RuntimeResult<Option<MumblePtr>> {
        let ml = rt::mumble_link_ptr()?;
        let tick = NonZero::<u32>::new(ml.read_ui_tick());
        Ok(tick.and_then(move |tick| {
            let prev = self.mumblelink_frame;
            self.mumblelink_frame = tick.get();
            (prev != self.mumblelink_frame).then_some(ml)
        }))
    }

    #[cfg(feature = "space")]
    fn read_camera_mumblelink(ml: MumblePtr) -> RenderPosition {
        unsafe {
            let camera = &raw const (*ml.as_ptr()).camera;
            let camera_pos = Point3::from_array(ptr::read_volatile(&raw const (*camera).position));
            let camera_front = Vector3::from_array(ptr::read_volatile(&raw const (*camera).front));
            let camera_up = Vector3::from_array(ptr::read_volatile(&raw const (*camera).top));
            if !crate::built_info::IS_TAGGED_VERSION {
                if !rt::vec_eq(camera_up, Vector3::ZERO) {
                    log::info!("Whoa, MumbleLink actually populates camera_up ({camera_up:?})? Unthinkable!");
                }
            }
            (camera_pos, camera_front, camera_up)
        }
    }

    /// Amount of skipped UI frames that would indicate we're likely in a loading
    /// screen
    const MUMBLELINK_PLAYER_FPS: u32 = 25;
    const MUMBLELINK_PLAYER_TICK_LOADING: u32 = 3;
    const MUMBLELINK_PLAYER_LOADING: Duration = Duration::from_millis(
        Self::MUMBLELINK_PLAYER_TICK.as_millis() as u64 * Self::MUMBLELINK_PLAYER_TICK_LOADING as u64,
    );
    const MUMBLELINK_PLAYER_TICK: Duration =
        Duration::from_millis(1000 / Self::MUMBLELINK_PLAYER_FPS as u64);
    const MUMBLELINK_SKIP_LOADING: u32 = {
        let target_fps = 55;
        target_fps * Self::MUMBLELINK_PLAYER_TICK_LOADING / Self::MUMBLELINK_PLAYER_FPS
    };

    pub(crate) fn next_mumblelink_frame(&mut self) -> (Option<MumblePtr>, Option<GameplayState>, u32) {
        let mut gameplay_change = None;
        let prev_frame = self.mumblelink_frame;
        let mut frameskip = self.mumblelink_frame_skip;
        let ml = match self.next_mumblelink_tick() {
            Ok(Some(ml)) => {
                frameskip = match prev_frame {
                    // on early load, we don't know if this "new" frame is stale or not...
                    0 => 1,
                    _ => 0,
                };
                Some(ml)
            },
            Ok(None) if self.mumblelink_frame == 0 => {
                // pre-loading
                if !self.gameplay.is_initial() {
                    log::warn!(
                        "MumbleLink went back in time from {:?}? Weird, this is probably a bug!",
                        self.gameplay
                    );
                    self.gameplay = GameplayState::INITIAL;
                }
                None
            },
            Ok(None) => {
                let frame_player = (frameskip >= Self::MUMBLELINK_SKIP_LOADING)
                    .then_some(self.mumblelink_frame_player)
                    .flatten();
                let probably_loading = frame_player
                    .map(|(_tick, when)| when.elapsed() > Self::MUMBLELINK_PLAYER_LOADING)
                    .unwrap_or(false);
                if probably_loading {
                    let prev_map_id = self.gameplay.latest_map().map(|m| m.get());
                    // we don't know what map we're loading in to,
                    // but have no good way to indicate that uncertainty other than to convey
                    // we're likely returning to the same map
                    // (this contrasts with rtapi which can at least discern pauses in some cases like cinematics)
                    let map_id = None;
                    #[cfg(todo)]
                    let map_id = NonZero::new(rt::mumble_link_ptr().ok().and_then(|ml| ml.read_map_id()));
                    let map_id = map_id.or(prev_map_id);
                    gameplay_change = Some(GameplayState::new_loading(
                        map_id.unwrap_or_default(),
                        prev_map_id.unwrap_or_default(),
                    ));
                }
                frameskip = frameskip.saturating_add(1);
                None
            },
            Err(..) => None,
        };
        (ml, gameplay_change, frameskip)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MumblelinkTick {
    Ui(u32),
    Player {
        ui_tick: u32,
        #[cfg(todo)]
        interim_ui_ticks: u32,
    },
}

impl MumblelinkTick {
    pub const fn with_ui_tick(ui_tick: u32) -> Self {
        Self::Ui(ui_tick)
    }

    pub const fn with_player_tick(ui_tick: u32) -> Self {
        MumblelinkTick::Player { ui_tick }
    }

    pub const fn ui_tick(&self) -> u32 {
        match *self {
            Self::Ui(ui_tick) => ui_tick,
            Self::Player { ui_tick, .. } => ui_tick,
        }
    }

    pub fn is_player(&self) -> bool {
        matches!(self, Self::Player { .. })
    }
}

impl From<u32> for MumblelinkTick {
    fn from(tick: u32) -> Self {
        Self::with_ui_tick(tick)
    }
}
