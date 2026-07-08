#[cfg(all(any(feature = "markers", feature = "space"), not(feature = "extension-nexus")))]
pub use arcloader_mumblelink::gw2_mumble::Identity as MumbleIdentityUpdate;
#[cfg(all(any(feature = "markers", feature = "space"), feature = "extension-nexus"))]
pub use arcloader_mumblelink::identity::{
    NexusIdentityShare as MumbleIdentityShare,
    NexusIdentityUpdate as MumbleIdentityUpdate,
};
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
#[cfg(feature = "space")]
use {
    taimi_hoard::time::duration_from_secs_f32,
    taimi_meta::spatial::record::{FrameRecord, FrameRecordEntry},
};

#[cfg(feature = "goggles2-project")]
use crate::settings::goggles::GogglesEnables;

#[cfg(all(any(feature = "markers", feature = "space"), not(feature = "extension-nexus")))]
pub(crate) type MumbleIdentityShare = Option<MumbleIdentityUpdate>;

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
    pub fn get_camera_mumblelink_verbatim(&self) -> Option<RenderPosition> {
        match self.mumblelink_camera.1.x.is_infinite()
            || rt::vec_eq(self.mumblelink_camera.1, Vector3::ZERO)
        {
            true => None,
            false => Some(self.mumblelink_camera),
        }
    }
    #[cfg(feature = "space")]
    #[cfg(deleteme)]
    pub fn get_camera_mumblelink(&self) -> Option<RenderPosition> {
        let mumblelink_camera = self.get_camera_mumblelink_verbatim()?;
        Some(
            match (
                self.mumblelink_camera_frame
                    .wrapping_sub(self.mumblelink_camera_prev_frame),
                self.mumblelink_frame_skip,
            ) {
                (jump @ 2..=3, skip) | (jump @ 1..=4, skip @ 1..=3) => {
                    let (pos0, front0) = self.mumblelink_camera_prev;
                    let (pos1, front1, up) = mumblelink_camera;
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
                _ => mumblelink_camera,
            },
        )
    }
    #[cfg(feature = "space")]
    pub(crate) fn lastminute_mumblelink_update(&mut self) {
        use taimi_meta::spatial::record::frame_is_lt;

        #[cfg(deleteme)]
        let ml = rt::mumble_link_ptr().ok().map(|ml| {
            let tick = ml.read_ui_tick();
            (ml, tick, tick != self.mumblelink_frame)
        });
        #[cfg(deleteme)]
        if let Some((_, tick, _)) = ml {
            #[cfg(deleteme)]
            let tick = tick.wrapping_sub(1);
            if frame_is_lt(self.mumblelink_frames.times.position, tick) {
                self.mumblelink_frames.times.advance_to(tick);
            }
        }
        #[cfg(deleteme)]
        let mut camera = None;
        #[cfg(feature = "extension-nexus")]
        #[cfg(deleteme)]
        if let Some(rtapi) = self
            .rtapi
            .as_ref()
            .and_then(|rtapi| rtapi.is_active().then_some(rtapi))
        {
            use taimi_hoard::vec::vec32_eq;

            let rt_tick = match ml {
                #[cfg(todo)]
                Some((_, tick, true)) => tick.wrapping_sub(1),
                #[cfg(todo)]
                _ => self
                    .mumblelink_frame
                    .wrapping_add(self.mumblelink_frame_skip.min(1)),
                _ => self.mumblelink_frames.latest_render_tick(),
            };
            let rt_tick_missed = || self.camera.mumblelink.get_at(rt_tick).is_none();
            if self.rtapi_state.has_camera() && rt_tick_missed() {
                let rtapi_prev = self.rtapi_state.camera;
                let (camera_rtapi, _fov) = super::RenderStateRtapi::read_camera(rtapi);
                if !vec32_eq(camera_rtapi.0, rtapi_prev.0) && !vec32_eq(camera_rtapi.1, rtapi_prev.1) {
                    let matches_ml = ml.map(|(ml, ..)| {
                        vec32_eq(camera.insert(Self::read_camera_mumblelink(ml)).0, camera_rtapi.0)
                    });
                    if matches_ml != Some(true) {
                        self.camera
                            .record_mumblelink(rt_tick, (camera_rtapi.0, camera_rtapi.1, Vector3::ZERO));
                    }
                }
            }
        }
        #[cfg(deleteme)]
        let ml = ml.and_then(|(ml, tick, new_tick)| new_tick.then_some((ml, tick)));
        let tryagain = self.mumblelink_frames.is_early();
        #[cfg(feature = "goggles2-project")]
        let tryagain = tryagain || self.goggles.active.contains(GogglesEnables::PROJECT_ENABLE);
        if tryagain {
            let _new_tick = self.post_render_mumblelink_check();
            #[cfg(todo)]
            if _new_tick.is_some() && self.goggles.project_is_projecting() {
                resyncidk();
            }
        }
        #[cfg(feature = "goggles2-camera")]
        #[cfg(deleteme)]
        {
            self.update_camera_goggles2();
        }
        #[cfg(deleteme)]
        let needs_ml_cam = ml.is_some()
            && self
                .camera
                .mumblelink
                .get_at(self.mumblelink_frames.latest_render_tick())
                .is_none();
        #[cfg(deleteme)]
        if let Some((ml, tick)) = needs_ml_cam.then_some(ml).flatten() {
            let camera = camera.unwrap_or_else(|| Self::read_camera_mumblelink(ml));
            self.camera.record_mumblelink(tick, camera);
            #[cfg(deleteme)]
            {
                self.mumblelink_camera = Self::read_camera_mumblelink(ml);
                self.mumblelink_camera_frame = tick;
                // TODO: bleh...
                self.mumblelink_frame_skip = 0;
            }
        }
    }
    #[cfg(feature = "space")]
    pub(super) fn post_render_mumblelink_check(&mut self) -> Option<u32> {
        let ml = rt::mumble_link_ptr().ok().and_then(|ml| {
            let tick = ml.read_ui_tick();
            (tick != self.mumblelink_frame).then_some((ml, tick))
        });
        let Some((ml, tick)) = ml else { return None };
        self.mumblelink_frames
            .record_lastminute_tick_at(tick, Instant::now());
        let camera = Self::read_camera_mumblelink(ml);
        self.camera.resync_with_frames(&self.mumblelink_frames);
        self.camera.record_mumblelink(tick, camera);
        self.map.update_from_mumblelink(ml);

        Some(tick)
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
            #[cfg(feature = "extension-nexus")]
            let update = match self.identity.update(&mut self.identity_changes, &ml) {
                Some(true) if !self.identity.is_empty() => Some(&self.identity.identity),
                _ => None,
            };
            #[cfg(not(feature = "extension-nexus"))]
            let update = match self.identity_changes.update(&ml) {
                Some(Some((_, identity))) => Some(&*self.identity.insert(identity)),
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
        let id_update = match (identity, &mut self.map.calibration) {
            _ if self.map_users.is_empty() => false,
            #[cfg(feature = "extension-nexus")]
            (Some(id), calib) => calib.update_from_mumblelink_identity_nexus(id),
            #[cfg(not(feature = "extension-nexus"))]
            (Some(id), calib) => calib.update_from_mumblelink_identity(id),
            _ => false,
        };
        #[cfg(feature = "space")]
        let new_fov = match (identity, self.map_users.contains(RenderUsers::SPACE)) {
            (Some(update), true) => {
                let fov_y = realign_fov(update.fov);
                #[cfg(todo)]
                let changed = fov_y.to_bits() != self.fov.y.to_bits();
                Some(Vector2::ZERO.with_y(fov_y))
            },
            _ => None,
        };

        #[cfg(any(feature = "markers", feature = "space"))]
        if !self.map_users.is_empty() {
            self.map.update_from_mumblelink(ml);

            if id_update {
                self.act_map_recalibrate(false);
            }
        }
        #[cfg(feature = "space")]
        if let Some(fov) = new_fov {
            self.set_fov(fov);
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
            #[cfg(todo)]
            false if self.mumblelink_frame_skip > 0 => false,
            false if rt::vec_eq(playpos, Point3::INFINITY) => {
                log::debug!("ML lost playpos?");
                true
            },
            eq => !eq,
        };
        if playpos_ticked {
            self.mumblelink_player = (playpos, front);
            self.mumblelink_frames.record_latest_player_tick();
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
            self.camera.record_mumblelink(self.mumblelink_frame, camera);
            let camera_dirty = !rt::vec_eq(self.mumblelink_camera.0, camera.0)
                || !rt::vec_eq(self.mumblelink_camera.1, camera.1);
            if self.mumblelink_camera_frame == self.mumblelink_frame && false {
                // lastminute updated, ignore
            } else
            /*if camera_dirty || playpos_ticked*/
            {
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
    pub(crate) fn read_camera_mumblelink(ml: MumblePtr) -> RenderPosition {
        unsafe {
            let camera = &raw const (*ml.as_ptr()).camera;
            let camera_pos = Point3::from_array(ptr::read_volatile(&raw const (*camera).position));
            let camera_front = Vector3::from_array(ptr::read_volatile(&raw const (*camera).front));
            let camera_up = Vector3::from_array(ptr::read_volatile(&raw const (*camera).top));
            if !crate::built_info::IS_TAGGED_VERSION {
                if !rt::vec_eq(camera_up, Vector3::ZERO) {
                    log::info!(
                        "Whoa, MumbleLink actually populates camera_up ({camera_up:?})? Unthinkable!"
                    );
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

    pub fn get_latest_space_timestamp(&self) -> Option<Instant> {
        let ts = match &self.mumblelink_frames {
            #[cfg(feature = "space")]
            frames => frames
                .renders
                .get_at(frames.latest_render_tick())
                .copied()
                .flatten(),
            #[cfg(not(feature = "space"))]
            frames => frames.get_latest_render_timestamp(),
        };
        #[cfg(feature = "goggles")]
        let ts = ts.or_else(|| self.goggles.latest_frame_timestamp());
        ts
    }
    pub fn latest_space_timestamp(&self) -> Instant {
        self.get_latest_space_timestamp()
            .unwrap_or_else(|| Instant::now())
    }
}

#[derive(Debug, Clone, Default)]
pub struct MumblelinkFrames {
    #[cfg(not(feature = "space"))]
    pub latest_tick: u32,
    #[cfg(not(feature = "space"))]
    pub latest_tick_timestamp: Option<Instant>,
    /// game ui ticks
    pub times: FrameRecord<8, Option<Instant>>,
    /// missing multiple ui ticks typically indicates a loading screen
    pub ui_skip: u32,
    /// addon update or present ticks, expected to desync from game update loop
    pub renders: FrameRecord<8, Option<Instant>>,
    pub render_offset: u32,
    #[cfg(feature = "space")]
    pub render_offset_space: u32,
    pub latest_render_tick: u32,
    pub player_frame: Option<(u32, Instant)>,
}
impl MumblelinkFrames {
    pub fn latest_tick(&self) -> Option<(u32, &Instant)> {
        match () {
            #[cfg(feature = "space")]
            _ => self.iter_ticks().next(),
            #[cfg(not(feature = "space"))]
            _ => self
                .latest_tick_timestamp
                .as_ref()
                .map(|i| (i, &self.latest_tick)),
        }
    }
    pub fn record_render_tick_at(&mut self, when: Instant) {
        self.renders.push(Some(when));
    }
    /// whether we appear to be sampling mumblelink data "early" relative to game logic frames
    ///
    /// frames are identified by monotonic uitick increments, and at render time we cannot tell
    /// in isolation whether the data we've just read corresponds to the upcoming frame ("late" read)
    /// or the prior frame - which (if using nexus render callbacks) is what we're drawing over
    /// at framebuffer present time
    ///
    /// being early is a disadvantage since it means we have no headroom, so we
    /// keep a gap at the front of our ring buffer that can hopefully be filled at some point if we're lucky!
    pub(super) fn is_early(&self) -> bool {
        self.times.front().is_empty()
    }
    pub(super) fn mark_early(&mut self) {
        self.times.push(None);
        //self.render_offset = self.render_offset.wrapping_add(1);
        self.resync();
    }
    #[cfg(todo)]
    pub(super) fn mark_late(&mut self) {
        self.times.rewind_by(1);
    }
    pub(super) fn resync(&mut self) {
        let render = match () {
            #[cfg(feature = "space")]
            _ => self.latest_space_tick(),
            #[cfg(not(feature = "space"))]
            _ => self.latest_render_tick(),
        };
        self.render_offset = self.last_ui_tick().wrapping_sub(render);
    }
    pub(super) fn render_to_uitick(&self, render_tick: u32) -> u32 {
        render_tick.wrapping_add(self.render_offset)
    }
    pub(super) fn uitick_to_render(&self, ui_tick: u32) -> u32 {
        ui_tick.wrapping_sub(self.render_offset)
    }
    pub fn record_tick_at(&mut self, tick: u32, when: Instant) {
        let prev_tick = self
            .times
            .iter_populated()
            .next()
            .map(|(i, ..)| i)
            .unwrap_or(self.times.position);
        let mut remain_early = self.ui_skip > Self::UI_TICK_OVERDUE;
        let became_late = match tick.wrapping_sub(prev_tick) {
            _ if !self.is_early() => false,
            0 => {
                remain_early = true;
                false
            },
            _ if self.ui_skip > 0 => false,
            1..=3 => true,
            _ => false,
        };
        let resync_after_skip = self.ui_skip > 0;
        self.ui_skip = 0;
        self.times.set_at(tick, Some(when));
        if remain_early {
            // maintain that gap!
            self.times.push(None);
        }
        if resync_after_skip || became_late {
            self.resync();
        }
    }
    pub fn record_lastminute_tick_at(&mut self, tick: u32, when: Instant) {
        return;
        let was_early = self.is_early();
        self.times.set_at(tick, Some(when));
        if was_early && self.ui_skip == 0 {
            self.resync()
        }
    }
    #[cfg(not(feature = "space"))]
    pub fn record_tick_now(&mut self, tick: u32, when: Instant) {
        self.latest_tick = tick;
        self.latest_tick_timestamp = when;
    }
    pub fn record_latest_player_tick(&mut self) {
        self.player_frame = self.latest_tick().map(|(i, when)| (i, *when));
    }
    pub fn record_missed_tick(&mut self) {
        if !self.is_early() {
            self.times.push(None);
        } else {
            self.ui_skip = self.ui_skip.saturating_add(1);
        }
    }
    pub fn record_missed_tick_at(&mut self, tick: u32) {
        self.times.advance_to(tick);
    }
    pub fn iter_ticks(&self) -> impl DoubleEndedIterator<Item = (u32, &Instant)> {
        self.times
            .iter_populated()
            .filter_map(|(tick, ts)| ts.as_ref().map(|ts| (tick, ts)))
    }
    pub fn timestamp_at(&self, tick: u32) -> Option<&Instant> {
        self.times.get_at(tick).and_then(|ts| ts.as_ref())
    }
    pub fn duration_at(&self, tick: Option<u32>) -> Option<f32> {
        let (tick, ts) = tick
            .and_then(|tick| self.timestamp_at(tick).map(|ts| (tick, ts)))
            .or_else(|| self.iter_ticks().next())?;
        let (oldest, past) = match self.iter_ticks().next_back() {
            #[cfg(debug_assertions)]
            oldest => oldest.unwrap(),
            #[cfg(todo)]
            oldest => oldest.unwrap_unchecked(),
            oldest => oldest.unwrap_or((tick, ts)),
        };
        match ts.saturating_duration_since(*past) {
            Duration::ZERO => {
                // bleh
                None
            },
            delta => Some(delta.as_secs_f32() / (tick - oldest) as f32),
        }
    }
    pub fn last_seen_tick(&self) -> Option<(u32, &Instant)> {
        self.latest_tick()
            .or(self.player_frame.as_ref().map(|(ptick, pts)| (*ptick, pts)))
    }
    /// ideal headroom of 1 frame - see [Self::is_early]
    pub fn last_ui_tick(&self) -> u32 {
        self.times.position.wrapping_sub(1)
    }
    #[cfg(feature = "space")]
    pub fn latest_space_tick(&self) -> u32 {
        self.latest_render_tick().wrapping_add(self.render_offset_space)
    }
    pub fn latest_render_tick(&self) -> u32 {
        self.renders.position
    }
    pub fn get_latest_render_timestamp(&self) -> Option<&Instant> {
        self.renders.front().as_ref()
    }
    pub fn latest_render_timestamp(&self) -> &Instant {
        match self.renders.front().as_ref() {
            #[cfg(debug_assertions)]
            ts => ts.unwrap(),
            #[cfg(not(debug_assertions))]
            ts => unsafe { ts.unwrap_unchecked() },
        }
    }
    pub fn is_tick_overdue(&self) -> bool {
        let Some((tick, ts)) = self.last_seen_tick() else { return false };

        match self
            .render_to_uitick(self.latest_render_tick())
            .wrapping_sub(tick)
        {
            missed if missed < Self::UI_TICK_OVERDUE => false,
            _ if self.latest_render_timestamp().saturating_duration_since(*ts)
                < Self::UI_TICK_OVERDUE_DURATION =>
                false,
            _ => true,
        }
    }
    pub(super) fn is_ui_tick_overdue(&self) -> bool {
        self.ui_skip >= Self::UI_TICK_OVERDUE
    }
    /// min missed uitick frames
    const UI_TICK_OVERDUE: u32 = match () {
        #[cfg(todo)]
        _ => 9,
        _ => 5,
    };
    /// min time to wait
    const UI_TICK_OVERDUE_DURATION: Duration = duration_from_secs_f32(0.3f32);

    pub fn player_tick_overdue(&self) -> Option<f32> {
        let (_, ref ts_prev) = self.player_frame.as_ref()?;
        let (_, ts) = self.latest_tick()?;
        let elapsed = ts.saturating_duration_since(*ts_prev);
        match elapsed.as_secs_f32() - Self::PLAYER_TICK_DURATION {
            overdue if overdue >= Self::PLAYER_TICK_DURATION_OVERDUE_BY => Some(overdue),
            _ => None,
        }
    }
    pub fn next_player_deadline(&self) -> Option<(u32, Instant)> {
        let &(tick, ref ts) = self.player_frame.as_ref()?;
        let deadline = match ts.checked_add(Duration::from_secs_f32(Self::PLAYER_TICK_DURATION)) {
            #[cfg(debug_assertions)]
            d => d.unwrap(),
            d => d.unwrap_or(*ts),
        };
        let estimate = self
            .duration_at(None)
            .map(|frame_time| {
                tick.wrapping_add(((Self::PLAYER_TICK_DURATION / frame_time).ceil() as u32).max(1))
            })
            .unwrap_or_else(|| tick.wrapping_add(Self::PLAYER_TICK_DEADLINE));
        Some((estimate, deadline))
    }
    const PLAYER_TICK_DEADLINE: u32 = 3;
    const PLAYER_TICK_RATE: u32 = 25;
    const PLAYER_TICK_DURATION: f32 = (Self::PLAYER_TICK_RATE as f32).recip();
    const PLAYER_TICK_DURATION_OVERDUE: f32 = Self::PLAYER_TICK_DURATION * 2.2;
    const PLAYER_TICK_DURATION_OVERDUE_BY: f32 =
        Self::PLAYER_TICK_DURATION_OVERDUE - Self::PLAYER_TICK_DURATION;
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
