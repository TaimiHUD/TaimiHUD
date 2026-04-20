#[cfg(any(feature = "markers", feature = "space"))]
pub use arcloader_mumblelink::identity::NexusIdentityUpdate as MumbleIdentityUpdate;
#[cfg(any(feature = "markers", feature = "space"))]
use {
    crate::exports::runtime::bindings::{ControlsReceiver, CONTROLS},
    arcloader_mumblelink::identity::{MumbleIdentityTracker, NexusIdentityShare},
    std::borrow::Cow,
    taimi_meta::{
        coords::SignObtainer,
        map::{Map, MapCache},
        ui::{MapOpen, UiMap},
    },
};
#[cfg(feature = "space")]
use {
    crate::{
        controller::pathing::shared::{PathingEnables, PathingShared},
        render::element::pack::PackElements,
        space::engine::Engine,
    },
    std::{ops::Range, sync::Arc},
    taimi_meta::map::MapProjectionDepth,
};
use {
    crate::{
        controller::Controller,
        exports::runtime::{self as rt, imgui, statistics::MetricsSwitch},
        render::RenderState,
        settings::Settings,
    },
    anyhow::Context,
    core::num::NonZero,
    glamour::{Angle, Point3, Size2, Vector2, Vector3},
    std::time::{Duration, Instant},
    taimi_meta::{
        coords::{LocalSpace, ScreenSpace},
        map::MapID,
        ui::{
            gameplay::{GameplayState, GameplayTransition},
            MapCalibration,
            UiState,
        },
    },
};

#[cfg(feature = "extension-nexus")]
pub use self::rtapi::RenderStateRtapi;
#[cfg(feature = "space")]
pub use self::space::CameraState;
pub use self::{
    diag::{frame_log, FrameLog, FrameState},
    mumblelink::{MumblelinkTick, MumblelinkFrames},
    tasks::{RenderTask, RenderTaskPriority, RenderTaskQueue},
};

mod diag;
mod map;
#[cfg(feature = "markers")]
mod markers;
mod mumblelink;
#[cfg(feature = "extension-nexus")]
mod rtapi;
#[cfg(feature = "space")]
mod space;
mod tasks;
mod ui;

pub struct RenderMachine {
    #[cfg(any(feature = "markers", feature = "space"))]
    pub identity: NexusIdentityShare,
    #[cfg(any(feature = "markers", feature = "space"))]
    pub identity_changes: MumbleIdentityTracker,
    pub identity_users: RenderUsers,
    #[cfg(any(feature = "markers", feature = "space"))]
    pub map: UiMap,
    pub map_users: RenderUsers,
    #[cfg(any(feature = "markers", feature = "space"))]
    pub map_info: Option<Cow<'static, Map>>,
    #[cfg(any(feature = "markers", feature = "space"))]
    pub map_open: bool,
    #[cfg(any(feature = "markers", feature = "space"))]
    pub map_open_timestamp: Option<Instant>,
    #[cfg(any(feature = "markers", feature = "space"))]
    pub map_sign: SignObtainer,
    #[cfg(any(feature = "markers", feature = "space"))]
    pub map_hidden: bool,
    #[cfg(any(feature = "markers", feature = "space"))]
    pub controls: ControlsReceiver,
    #[cfg(feature = "space")]
    pub depth_range: Option<Range<f32>>,
    #[cfg(feature = "space")]
    pub map_depth: Option<MapProjectionDepth>,
    #[cfg(feature = "space")]
    pub map_depth_guess: Option<MapProjectionDepth>,
    #[cfg(feature = "space")]
    fov: Vector2<Angle>,
    #[cfg(feature = "space")]
    pub fov2_tan: Angle,
    #[cfg(feature = "goggles")]
    pub goggles: crate::space::goggles::GogglesState,
    #[cfg(feature = "extension-nexus")]
    pub rtapi: Option<rt::RealTimeApi>,
    #[cfg(feature = "extension-nexus")]
    pub rtapi_state: RenderStateRtapi,
    #[cfg(feature = "extension-nexus")]
    pub rtapi_users: RenderUsers,
    pub mumblelink_frames: MumblelinkFrames,
    pub mumblelink_frame: u32,
    pub mumblelink_frame_player: Option<(u32, Instant)>,
    pub mumblelink_frame_skip: u32,
    pub mumblelink_map: MapID,
    pub mumblelink_state: UiState,
    pub mumblelink_player: RenderPositioning,
    #[cfg(feature = "space")]
    pub camera: CameraState,
    #[cfg(feature = "space")]
    pub mumblelink_camera: RenderPosition,
    #[cfg(feature = "space")]
    pub mumblelink_camera_frame: u32,
    #[cfg(feature = "space")]
    pub mumblelink_camera_prev: RenderPositioning,
    #[cfg(feature = "space")]
    pub mumblelink_camera_prev_frame: u32,
    pub mumblelink_users: RenderUsers,
    pub frame_duration: Option<Duration>,
    pub gameplay: GameplayState,
    #[cfg(not(any(feature = "markers", feature = "space")))]
    pub display_size: Size2<ScreenSpace>,
    #[cfg(feature = "paths")]
    pub pathing: Option<Arc<PathingShared>>,
    #[cfg(feature = "paths")]
    pub pack_ui_state: PackElements,
    pub metrics_switch: MetricsSwitch,
    pub metrics_checkpoint: Option<Instant>,
    pub metrics_checkpoint_render: Option<Instant>,
    pub metrics_checkpoint_ui: Option<Instant>,
}

pub type RenderPositioning<S = LocalSpace> = (Point3<S>, Vector3<S>);
pub type RenderPosition<S = LocalSpace> = (Point3<S>, Vector3<S>, Vector3<S>);

impl RenderMachine {
    /// TODO
    pub const USERS: RenderUsers = RenderUsers::all();

    pub const POSITIONING_EMPTY: RenderPositioning = (Point3::INFINITY, Vector3::INFINITY);
    #[cfg(feature = "space")]
    pub const POSITION_EMPTY: RenderPosition = (Point3::INFINITY, Vector3::INFINITY, Vector3::INFINITY);

    pub fn new() -> Self {
        Self {
            #[cfg(any(feature = "markers", feature = "space"))]
            identity: NexusIdentityShare::EMPTY,
            #[cfg(any(feature = "markers", feature = "space"))]
            identity_changes: MumbleIdentityTracker::new(),
            identity_users: Self::USERS,
            #[cfg(any(feature = "markers", feature = "space"))]
            map: {
                let mut map = UiMap::DEFAULT;
                map.calibration.display_size = Size2::ZERO;
                map
            },
            map_users: Self::USERS,
            #[cfg(any(feature = "markers", feature = "space"))]
            map_info: None,
            #[cfg(any(feature = "markers", feature = "space"))]
            map_open: MapOpen::DEFAULT.is_open(),
            #[cfg(any(feature = "markers", feature = "space"))]
            map_open_timestamp: None,
            #[cfg(any(feature = "markers", feature = "space"))]
            map_sign: SignObtainer::DEFAULT,
            #[cfg(any(feature = "markers", feature = "space"))]
            map_hidden: false,
            #[cfg(any(feature = "markers", feature = "space"))]
            controls: CONTROLS.subscribe_controls(),
            #[cfg(feature = "space")]
            fov: Vector2::ZERO,
            #[cfg(feature = "space")]
            fov2_tan: Self::DEFAULT_FOV2_TAN,
            #[cfg(feature = "goggles")]
            goggles: Default::default(),
            #[cfg(feature = "space")]
            depth_range: None,
            #[cfg(feature = "space")]
            map_depth: None,
            #[cfg(feature = "space")]
            map_depth_guess: None,
            #[cfg(feature = "extension-nexus")]
            rtapi: None,
            #[cfg(feature = "extension-nexus")]
            rtapi_state: RenderStateRtapi::new(),
            #[cfg(feature = "extension-nexus")]
            rtapi_users: Self::USERS,
            mumblelink_frames: MumblelinkFrames::default(),
            mumblelink_frame: 0,
            mumblelink_frame_player: None,
            mumblelink_frame_skip: 0,
            mumblelink_map: 0,
            mumblelink_state: UiState::empty(),
            mumblelink_player: Self::POSITIONING_EMPTY,
            #[cfg(feature = "space")]
            camera: CameraState::default(),
            #[cfg(feature = "space")]
            mumblelink_camera: Self::POSITION_EMPTY,
            #[cfg(feature = "space")]
            mumblelink_camera_frame: 0,
            #[cfg(feature = "space")]
            mumblelink_camera_prev: Self::POSITIONING_EMPTY,
            #[cfg(feature = "space")]
            mumblelink_camera_prev_frame: 0,
            mumblelink_users: Self::USERS,
            frame_duration: None,
            gameplay: GameplayState::INITIAL,
            #[cfg(not(any(feature = "markers", feature = "space")))]
            display_size: Size2::ZERO,
            #[cfg(feature = "paths")]
            pathing: None,
            #[cfg(feature = "paths")]
            pack_ui_state: PackElements::default(),
            metrics_switch: Default::default(),
            metrics_checkpoint: Default::default(),
            metrics_checkpoint_render: Default::default(),
            metrics_checkpoint_ui: Default::default(),
        }
    }

    pub fn reset_display_size(&mut self) {
        *self.display_size_mut() = Size2::ZERO;
    }

    pub fn display_size_mut(&mut self) -> &mut Size2<ScreenSpace> {
        match () {
            #[cfg(any(feature = "markers", feature = "space"))]
            _ => &mut self.map.calibration.display_size,
            #[cfg(not(any(feature = "markers", feature = "space")))]
            _ => &mut self.display_size,
        }
    }

    pub fn display_size_ref(&self) -> &Size2<ScreenSpace> {
        match () {
            #[cfg(any(feature = "markers", feature = "space"))]
            _ => &self.map.calibration.display_size,
            #[cfg(not(any(feature = "markers", feature = "space")))]
            _ => &self.display_size,
        }
    }

    pub fn display_size(&self) -> Option<Size2<ScreenSpace>> {
        let display_size = *self.display_size_ref();
        (!rt::vec_eq(display_size, Size2::ZERO)).then_some(display_size)
    }

    pub const DEFAULT_ASPECT_RATIO: f32 = 16.0f32 / 9.0f32;
    pub fn aspect_ratio(&self) -> Option<f32> {
        let sz = self.display_size_ref();
        match sz.width / sz.height {
            ratio if ratio.is_infinite() || ratio.is_nan() => None,
            ratio => Some(ratio),
        }
    }
    #[cfg(feature = "space")]
    #[inline]
    pub fn get_aspect_ratio(&self) -> f32 {
        self.aspect_ratio().unwrap_or(Self::DEFAULT_ASPECT_RATIO)
    }

    pub fn get_player_pos(&self) -> Option<RenderPositioning<LocalSpace>> {
        match self.mumblelink_player.0.x.is_infinite() {
            #[cfg(feature = "extension-nexus")]
            _ if !self.rtapi_state.player.0.x.is_infinite() => Some(self.rtapi_state.player),
            false => Some(self.mumblelink_player),
            true => None,
        }
    }

    pub fn get_player(&mut self) -> RenderPosition<LocalSpace> {
        // TODO: cache direct ptr each frame
        let (pos, front) = self
            .get_player_pos()
            .map(|(pos, front)| (pos, front))
            .unwrap_or((Point3::ZERO, Vector3::ZERO));
        (pos, front.normalize_or(Self::LOCAL_FORWARD), Self::LOCAL_UP)
    }

    pub fn act_gameplay_transition(&mut self, trans: GameplayTransition) {
        if trans.was_initial() {
            self.act_gameplay_initial();
        }
        let gameplay = self.gameplay.clone();
        #[cfg(any(feature = "markers", feature = "space"))]
        {
            #[cfg(feature = "goggles")]
            let map_left = match trans {
                GameplayTransition::Map { prev_map_id: Some(prev), .. } if Some(prev) == gameplay.gameplay_map() => false,
                #[cfg(not(feature = "goggles2-project"))]
                GameplayTransition::Intermission { .. } if !self.goggles.active.is_empty() => false,
                #[cfg(todo)]
                GameplayTransition::Intermission { .. } if !self.is_cutscene() && !self.is_ingame_paused() => true,
                _ => true,
            };
            #[cfg(feature = "goggles2-camera")]
            if let (true, Some(map_id)) = (map_left, trans.prev_map_id()) {
                let farz =
                    self.goggles.camera.perspective_farz()
                        .map(|farz| (map_id, farz));
                log::debug!("saving? {:?}", self.goggles.camera.perspective_params());
                let settings = farz.is_some().then(||
                    crate::SETTINGS.get().and_then(|s| s.try_write().ok())
                ).flatten();
                if let (Some((map_id, farz)), Some(mut settings)) = (farz, settings) {
                    let dirty = if let Some(pathing) = settings.pathing.as_mut() {
                        pathing.space.goggles.set_map_proj_seen_depth(map_id.get(), farz)
                    } else { false };
                    if dirty {
                        settings.mark_dirty();
                        #[cfg(taimi_debug)]
                        if let Some((h, _, near, far)) = self.goggles.camera.perspective_params() {
                            log::error!("ON.mapid={map_id:04} FOUND NEW PERSP.far = {far:?} ({near}..{far}) h={h:?}")
                        }
                    }
                }
            }
            #[cfg(feature = "goggles")]
            if map_left {
                self.goggles.act_map_exit(gameplay, trans);
            }
            let map_id = gameplay.latest_map();
            #[cfg(feature = "goggles")]
            if let Some(map_id) = map_id {
                let map_entered = match trans {
                    GameplayTransition::Map { prev_map_id: Some(prev), .. } => Some(prev != map_id),
                    GameplayTransition::Loaded { prev_map_id: Some(prev), .. } => Some(prev != map_id),
                    GameplayTransition::Intermission { .. } => None,
                    #[cfg(todo)]
                    GameplayTransition::Map { prev_map_id: None, next_map_id } => true,
                    _ => Some(false),
                };
                if let Some(hard) = map_entered {
                    self.goggles.act_map_enter(hard);
                }
            }
            #[cfg(feature = "space")]
            if let Some(map_id) = map_id {
                self.map_depth = MapCache.lookup_map_projection(map_id.get()).map(|proj| proj.depth.clone());
                self.map_depth_guess = None;
            }
            self.map_info = match map_id {
                None => {
                    if self.map_hidden {
                        log::info!("UI toggle escape hatch - resetting hidden state due to loading screen");
                        self.map_hidden = false;
                    }
                    None
                },
                Some(map_id) => match MapCache.lookup_map(map_id.get()) {
                    Some(map) => {
                        if matches!(trans, GameplayTransition::Loaded { .. }) {
                            self.map.calibration.local_space = None;
                            self.map.calibration.local_offset = None;
                        }
                        self.map.calibration.update_from_map(&map);
                        #[cfg(todo)]
                        #[cfg(feature = "space")]
                        if self.map_depth.is_none() {
                            use taimi_meta::coords::MapLocalScale;
                            let continent = map.continent_rect().size();
                            let rect = map.map_rect();
                            let map_extents = rect.size().to_vector() + rect.center().to_vector();
                            let map_aspect = map_extents.x / map_extents.y;
                            let far = continent.height * 12.0 * map_aspect.min(Self::DEPTH_ASPECT_MAX) * MapLocalScale::METRES_PER_INCH;
                            self.map_depth_guess = MapProjectionDepth::with_far_in(far);
                        }
                        Some(map)
                    },
                    None => None,
                },
            };
        }
        Controller::with_sender(|sender| {
            if let Some(tx) = &sender.gameplay {
                tx.send_replace(gameplay);
            }
        });
    }

    /// First map load we've seen!
    ///
    /// TODO: inconsistent (maybe with rtapi)?
    pub fn act_gameplay_initial(&mut self) {
        log::debug!("loading initial keybinds");
        rt::bindings::populate_bind_controls();
    }
    pub fn act_setup(&mut self) {
        self.metrics_init();

        log::debug!("loading initial keybinds");
        rt::bindings::populate_bind_controls();
    }

    pub fn act_display_size(&mut self) {
        #[cfg(any(feature = "markers", feature = "space"))]
        if let Some(_size) = self.display_size() {
            self.set_fov(self.fov.with_x(0.0));
            let dpi = Settings::read_with_blocking(|s| s.dpi_scaling.clone())
                .ok()
                .flatten();
            self.map.calibration.dpi = dpi.unwrap_or_else(|| match rt::window_dpi() {
                Ok(dpi) => dpi as f32,
                Err(e) => {
                    log::warn!("Maps and markers may be incorrect, could not determine DPI due to: {e}");
                    MapCalibration::DPI_REFERENCE
                },
            });
            log::trace!("resize to: {_size:?} @ {}", self.map.calibration.dpi);
            self.act_map_recalibrate(true);
        }
    }

    pub fn turn_ui_entry<'ui, U>(ui: &mut U)
    where
        U: ?Sized + super::element::im::ImDrawWindow<'ui>,
    {
        if !RenderState::is_running() {
            return
        }
        let mut state = RenderState::lock();
        if let Some(state) = state.as_mut() {
            state.machine.turn_ui(ui);
        }
    }

    pub fn turn_render_entry() {
        if !RenderState::is_running() {
            return
        }

        let now = Instant::now();
        let mut state = RenderState::lock();
        if let Some(state) = state.as_mut() {
            let render_timestamp = match () {
                #[cfg(feature = "goggles")]
                _ => state.machine.goggles.latest_frame_timestamp(),
                #[cfg(not(feature = "goggles"))]
                _ => None::<Instant>,
            };
            state.machine.frame_duration = None;
            let prev = state.machine.mumblelink_frames.get_latest_render_timestamp().copied();
            let render_timestamp = render_timestamp.or_else(|| {
                let frametime = *state.machine.frame_duration.insert(now.saturating_duration_since(prev?));
                let midpoint = (frametime <= Self::FRAMETIME_MAX_REASONABLE)
                    .then_some(frametime / 3);
                midpoint.and_then(|mid|
                    now.checked_sub(mid)
                )
            });
            let wants_frame_duration = state.machine.frame_duration.is_none() && state.machine.metrics_switch.contains(MetricsSwitch::COLLECT);
            if wants_frame_duration {
                if let (Some(prev), Some(render)) = (prev, render_timestamp) {
                    state.machine.frame_duration = render.checked_duration_since(prev);
                }
            }
            state.machine.turn_render_pre(now, render_timestamp);
            state.pre_render_ui();

            Self::run_tasks(state);

            Self::poll_runtime(state);

            let render_slot = (match () {
                #[cfg(feature = "space")]
                () => &mut state.engine,
                #[cfg(not(feature = "space"))]
                () => (),
            },);
            state.machine.turn_render(render_slot);
        }
    }
    const FRAMETIME_MAX_REASONABLE: Duration = Duration::from_millis(0x80);

    pub const TEXTURE_LOGO_LINES_KEY: &'static str = "taimihud_lines256";
    pub const TEXTURE_LOGO_LINES_BIN: &'static [u8] =
        include_bytes!("../../../data/textures/logotype-lines-256.png");
    pub const TEXTURE_LOGO_KEY: &'static str = "taimihud_glow256";
    pub const TEXTURE_LOGO_BIN: &'static [u8] =
        include_bytes!("../../../data/textures/logotype-glow-256.png");

    pub fn turn_render_pre(&mut self, now: Instant, render_timestamp: Option<Instant>) {
        self.metrics_pre();
        self.metrics_pre_render(&now);
        self.mumblelink_frames.record_render_tick_at(render_timestamp.unwrap_or(now));

        #[cfg(feature = "paths")]
        if self.pathing.is_none() {
            Controller::with_sender(|s| {
                if let Some(pathing) = &s.pathing {
                    self.pathing = Some(pathing.shared.clone());
                }
            });
        }
        #[cfg(feature = "paths")]
        #[cfg(todo)]
        if let Some(pathing) = &self.pathing {
            if !self.pack_map.is_watching() {
                self.pack_map.restart_watching(&pathing.gameplay);
            }
            let _ = self.pack_map.try_read_mut();
        }
        #[cfg(feature = "space")]
        {
            self.pre_render_space();
        }
    }

    pub fn turn_render(&mut self, _render_slot: RenderSlot<'_>) {
        #[cfg(any(feature = "markers", feature = "space"))]
        let controls_changed = self.controls.update().map(|(&state, changes)| (state, changes));

        let (ml, mut frameskip_gameplay, frame_skip) = self.next_mumblelink_frame();

        if ml.is_none() {
            self.mumblelink_frames.record_missed_tick();
        }
        let mumble_gameplay = ml.and_then(|ml| {
            let now = *self.mumblelink_frames.latest_render_timestamp();
            self.mumblelink_frames.record_tick_at(self.mumblelink_frame, now);
            self.act_mumblelink_tick(ml)
        });
        #[cfg(feature = "space")]
        {
            self.camera.resync_with_frames(&self.mumblelink_frames);
        }
        self.mumblelink_frame_skip = frame_skip;

        #[cfg(feature = "goggles2-camera")]
        {
            self.goggles_update_camera(true);
        }

        let ui_tick = self.ui_tick();

        let mut gameplay_change = None;
        #[cfg(feature = "extension-nexus")]
        let rtapi = self
            .rtapi
            .as_ref()
            .and_then(|rtapi| rtapi.is_active().then_some(rtapi));
        #[cfg(feature = "extension-nexus")]
        if let Some(rtapi) = rtapi {
            let rtapi_camera = ui_tick.is_none() || !self.rtapi_users.is_empty();
            let rtapi_gameplay = self.rtapi_state.update(rtapi, ui_tick, rtapi_camera);
            if let Some(rtapi_gameplay) = rtapi_gameplay {
                gameplay_change = Some(rtapi_gameplay);
            } else {
                frameskip_gameplay = None;
            }
            self.camera.mumblelink_confident = true;
            self.rtapi_camera_sync();
        } else {
            self.rtapi_state.set_inactive();
            self.camera.mumblelink_confident = false;
        }

        let gameplay_change = gameplay_change
            .or(mumble_gameplay)
            .or(frameskip_gameplay);
        let gameplay_transition = gameplay_change.and_then(|gameplay| match gameplay {
            GameplayState::Intermission {
                next_map_id: map_id @ Some(..),
                prev_map_id,
                ..
            } if map_id == prev_map_id => self.gameplay.commit_intermission(),
            GameplayState::Intermission { next_map_id: map_id, .. } => {
                //self.map.calibration.clear_map();
                self.gameplay.commit_loading(map_id)
            },
            GameplayState::Gameplay { map_id } => {
                if let Some(_map_id) = map_id {
                    // TODO: only if prev map was different!
                    //self.map.calibration.clear_map();
                    self.map_sign.clear();
                }
                self.gameplay
                    .commit_ingame(map_id.map(|id| id.get()).unwrap_or_default())
            },
        });

        if let Some(trans) = gameplay_transition {
            self.act_gameplay_transition(trans);
        }

        #[cfg(any(feature = "markers", feature = "space"))]
        if let Some((controls_state, controls_changed)) = controls_changed {
            self.act_controls_changed(controls_state, controls_changed);
        }

        if let Some(_ui_tick) = ui_tick {
            self.publish_map_state();
        }

        #[cfg(feature = "space")]
        if self.mumblelink_users.contains(RenderUsers::SPACE)
            && self.display_size().is_some()
            && RenderState::is_running()
        {
            let (engine_slot,) = _render_slot;
            // TODO: !game_is_shutting_down
            let mut init = false;
            let res = Engine::init_mut(self, engine_slot, |e, machine| {
                init = true;
                let res = e.render(machine).context("Space engine render");
                if res.is_err() {
                    log::info!("Stopping render for now, resize game to retry (Alt+Enter) and consider reporting the error");
                    e.stop();
                }
                res
            }).context("Initializing space engine");
            if let Err(e) = &res {
                log::error!("{e:#}");
            }
            let signal_engine = |active: bool| {
                Controller::with_sender(|s| {
                    if let Some(s) = s.pathing.as_ref() {
                        s.enables
                            .send_modify(|enables| enables.set(PathingEnables::ENGINE, active))
                    }
                })
            };
            match res {
                Err(e) if init => {
                    signal_engine(false);
                },
                Ok(true) => {
                    signal_engine(true);
                },
                _ => (),
            }
        }
        self.post_render();
    }
    fn post_render(&mut self) {
        self.metrics_post_render();
        self.act_frame_log();
        #[cfg(feature = "goggles")]
        {
            self.goggles_post_render();
        }
        if frame_log!(::is_enabled()) {
            if let Some((pos, dir)) = self.get_player_pos() {
                frame_log!(;
                    "player @ {pos:?} front={dir:?}"
                );
            }
            if let Some((pos, dir, up)) = self.get_camera_mumblelink_verbatim() {
                frame_log!(;
                    "camera @ {pos:?} dir={dir:?} up={up:?}"
                );
            }
        }
    }

    pub fn turn_ui<'ui, U>(&mut self, ui: &mut U)
    where
        U: ?Sized + super::element::im::ImDrawWindow<'ui>,
    {
        self.metrics_pre_ui();

        let prev_display_size = *self.display_size_ref();
        let display_size = self.display_size_mut();
        *display_size = ui.im_io_display_size().0.cast();
        if !rt::vec_eq(*display_size, prev_display_size) {
            self.act_display_size();
        }
    }
    pub fn post_ui(&mut self) {
        self.metrics_post_ui();
    }
    /// TODO: move to actual post-render (nexus callback)?
    pub fn post_render_late(&mut self, _render_slot: RenderSlot<'_>) {
        #[cfg(feature = "space")]
        if let (Some(Ok(engine)),) = _render_slot {
            engine.drawing.end_frame();
        }
        #[cfg(feature = "goggles")]
        {
            self.goggles_post_render_late(_render_slot);
        }
        self.post_render_mumblelink_check();
    }

    #[inline]
    pub(crate) fn is_cutscene(&self) -> bool {
        #[cfg(feature = "extension-nexus")]
        if self.rtapi_state.is_active() {
            return self.rtapi_state.is_cutscene();
        }
        #[cfg(feature = "goggles2-camera")]
        if self.gameplay.gameplay_map().is_none() && self.goggles.camera.camera_enabled && self.is_camera_moving() { return true }
        false
    }
    #[inline]
    pub fn is_ingame(&self) -> Option<NonZero<MapID>> {
        match self.gameplay {
            GameplayState::Intermission { initial: false, next_map_id: Some(next), .. } if self.is_cutscene() =>
                Some(next),
            gameplay => gameplay.gameplay_map(),
        }
    }
    #[inline]
    pub(crate) fn is_ingame_paused(&self) -> bool {
        #[cfg(feature = "extension-nexus")]
        if self.rtapi_state.is_loading_uncertain() { return true }
        if self.is_cutscene() {
            #[cfg(feature = "goggles2-camera")]
            if !self.goggles.camera.has_camera() { return true }
        }
        false
    }
}

bitflags::bitflags! {
    #[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
    pub struct RenderUsers: u16 {
        #[cfg(feature = "extension-arcdps")]
        const ARC = 0x01;
        #[cfg(feature = "space")]
        const SPACE = 0x02;
        #[cfg(feature = "markers")]
        const MARKERS = 0x04;
    }
}

#[cfg(feature = "space")]
pub type RenderSlot<'r> = (&'r mut Option<anyhow::Result<crate::space::engine::Engine>>,);
#[cfg(not(feature = "space"))]
pub type RenderSlot<'r> = ();
