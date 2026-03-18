#[cfg(any(feature = "markers", feature = "space"))]
use {
    crate::exports::runtime::bindings::{ControlsReceiver, CONTROLS},
    arcloader_mumblelink::identity::MumbleIdentity,
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
    std::time::Instant,
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

#[cfg(any(feature = "markers", feature = "space"))]
pub use self::mumblelink::MumbleIdentityUpdate;
#[cfg(feature = "extension-nexus")]
pub use self::rtapi::RenderStateRtapi;
#[cfg(feature = "space")]
pub use self::space::GogglesState;
pub use self::{
    diag::{frame_log, FrameLog, FrameState},
    mumblelink::MumblelinkTick,
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

pub struct RenderMachine {
    #[cfg(any(feature = "markers", feature = "space"))]
    pub identity: MumbleIdentity,
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
    fov: Vector2<Angle>,
    #[cfg(feature = "space")]
    pub fov2_tan: Angle,
    #[cfg(feature = "goggles")]
    pub goggles: GogglesState,
    #[cfg(feature = "extension-nexus")]
    pub rtapi: Option<rt::RealTimeApi>,
    #[cfg(feature = "extension-nexus")]
    pub rtapi_state: RenderStateRtapi,
    #[cfg(feature = "extension-nexus")]
    pub rtapi_users: RenderUsers,
    pub mumblelink_frame: u32,
    pub mumblelink_frame_player: Option<(u32, Instant)>,
    pub mumblelink_frame_skip: u32,
    pub mumblelink_map: MapID,
    pub mumblelink_state: UiState,
    pub mumblelink_player: RenderPositioning,
    #[cfg(feature = "space")]
    pub mumblelink_camera: RenderPosition,
    #[cfg(feature = "space")]
    pub mumblelink_camera_frame: u32,
    #[cfg(feature = "space")]
    pub mumblelink_camera_prev: RenderPositioning,
    #[cfg(feature = "space")]
    pub mumblelink_camera_prev_frame: u32,
    pub mumblelink_users: RenderUsers,
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
            identity: MumbleIdentity::new(),
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
            goggles: GogglesState::default(),
            #[cfg(feature = "space")]
            depth_range: None,
            #[cfg(feature = "extension-nexus")]
            rtapi: None,
            #[cfg(feature = "extension-nexus")]
            rtapi_state: RenderStateRtapi::new(),
            #[cfg(feature = "extension-nexus")]
            rtapi_users: Self::USERS,
            mumblelink_frame: 0,
            mumblelink_frame_player: None,
            mumblelink_frame_skip: 0,
            mumblelink_map: 0,
            mumblelink_state: UiState::empty(),
            mumblelink_player: Self::POSITIONING_EMPTY,
            #[cfg(feature = "space")]
            mumblelink_camera: Self::POSITION_EMPTY,
            #[cfg(feature = "space")]
            mumblelink_camera_frame: 0,
            #[cfg(feature = "space")]
            mumblelink_camera_prev: Self::POSITIONING_EMPTY,
            #[cfg(feature = "space")]
            mumblelink_camera_prev_frame: 0,
            mumblelink_users: Self::USERS,
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
            use taimi_meta::coords::MapLocalScale;
            self.map_info = match gameplay.latest_map() {
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
                        #[cfg(feature = "space")]
                        {
                            let continent = map.continent_rect().size();
                            let rect = map.map_rect();
                            let map_extents = rect.size().to_vector() + rect.center().to_vector();
                            let map_aspect = map_extents.x / map_extents.y;
                            let far = continent.height * 12.0 * map_aspect.min(Self::DEPTH_ASPECT_MAX) * MapLocalScale::METRES_PER_INCH;
                            let near = far * Self::DEPTH_NEAR_MULT;
                            self.depth_range = Some(near..far);
                        }
                        Some(map)
                    },
                    None => {
                        #[cfg(feature = "space")]
                        {
                            self.depth_range = None;
                        }
                        None
                    },
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

    pub fn turn_ui_entry(ui: &imgui::Ui) {
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

        let mut state = RenderState::lock();
        if let Some(state) = state.as_mut() {
            state.machine.turn_render_pre();
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

    pub const TEXTURE_LOGO_LINES_KEY: &'static str = "taimihud_lines256";
    pub const TEXTURE_LOGO_LINES_BIN: &'static [u8] =
        include_bytes!("../../../data/textures/logotype-lines-256.png");
    pub const TEXTURE_LOGO_KEY: &'static str = "taimihud_glow256";
    pub const TEXTURE_LOGO_BIN: &'static [u8] =
        include_bytes!("../../../data/textures/logotype-glow-256.png");

    pub fn turn_render_pre(&mut self) {
        self.metrics_pre();
        self.metrics_pre_render();

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
        #[cfg(feature = "goggles")]
        {
            let visible = self.gameplay.gameplay_map().is_some();
            self.goggles.act_pre_render(visible);
            if true {
                let size = self.display_size_ref();
                crate::space::goggles::FerretResource::set_display_size(size.to_raw());
            }
        }
    }

    pub fn turn_render(&mut self, _render_slot: RenderSlot<'_>) {
        #[cfg(any(feature = "markers", feature = "space"))]
        let controls_changed = self.controls.update().map(|(&state, changes)| (state, changes));

        let (ml, mut frameskip_gameplay, frame_skip) = self.next_mumblelink_frame();

        let mumble_gameplay = ml.and_then(|ml|
            self.act_mumblelink_tick(ml)
        );
        self.mumblelink_frame_skip = frame_skip;

        let ui_tick = self.ui_tick();

        let mut gameplay_change = None;
        #[cfg(feature = "extension-nexus")]
        if let Some(rtapi) = self
            .rtapi
            .as_ref()
            .and_then(|rtapi| rtapi.is_active().then_some(rtapi))
        {
            let rtapi_camera = ui_tick.is_none() || !self.rtapi_users.is_empty();
            let rtapi_gameplay = self.rtapi_state.update(rtapi, ui_tick, rtapi_camera);
            if let Some(rtapi_gameplay) = rtapi_gameplay {
                gameplay_change = Some(rtapi_gameplay);
            } else {
                frameskip_gameplay = None;
            }
        } else {
            self.rtapi_state.set_inactive();
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
        #[cfg(feature = "goggles2-camera")]
        if self.goggles.camera_enabled {
            let cam = match self.get_camera_mumblelink() {
                #[cfg(feature = "extension-nexus")]
                _ if self.rtapi_state.is_cutscene() => Err(true),
                #[cfg(feature = "extension-nexus")]
                #[cfg(todo = "unnecessary")]
                _ if self.rtapi_state.is_intermission() => Err(false),
                _ if self.gameplay.gameplay_map().is_none() => Err(false),
                cam => Ok(cam),
            };
            match cam {
                Ok(cam) => {
                    let persp = (
                        self.get_fov().y,
                        self.aspect_ratio().unwrap_or(Self::DEFAULT_ASPECT_RATIO),
                    );
                    let ml_frame = self.mumblelink_frame.wrapping_add(self.mumblelink_frame_skip);
                    let update_mod: u32 = match self.mumblelink_state {
                        s if !s.contains(UiState::WINDOW_FOCUS) =>
                            0x80,
                        s if s.contains(UiState::MAP_OPEN) => match self.get_map_open_state() {
                            MapOpen::Closed | MapOpen::Closing { .. } =>
                                0x08,
                            _ => 0x80,
                        },
                        _ => 0x02,
                    };
                    let update_mask = update_mod - 1;
                    let update = (ml_frame & update_mask) as u8;
                    self.goggles.camera_setup(cam, persp, update);
                },
                Err(intermission) =>
                    self.goggles.camera_pause(intermission),
            }
        }
        #[cfg(feature = "goggles")]
        {
            self.goggles.act_render_post();
        }
        if frame_log!(::is_enabled()) {
            if let Some((pos, dir)) = self.get_player_pos() {
                frame_log!(;
                    "player @ {pos:?} front={dir:?}"
                );
            }
            if let Some((pos, dir, up)) = self.get_camera_mumblelink() {
                frame_log!(;
                    "camera @ {pos:?} dir={dir:?} up={up:?}"
                );
            }
        }
    }

    pub fn turn_ui(&mut self, ui: &imgui::Ui) {
        self.metrics_pre_ui();

        let prev_display_size = *self.display_size_ref();
        let display_size = self.display_size_mut();
        *display_size = Size2::from_array(ui.io().display_size);
        if !rt::vec_eq(*display_size, prev_display_size) {
            self.act_display_size();
        }
    }
    pub fn post_ui(&mut self) {
        self.metrics_post_ui();
    }

    #[inline]
    fn is_cutscene(&self) -> bool {
        #[cfg(feature = "extension-nexus")]
        if self.rtapi_state.is_active() {
            return self.rtapi_state.is_cutscene();
        }
        #[cfg(feature = "goggles2-camera")]
        if self.gameplay.gameplay_map().is_none() && self.goggles.camera_enabled && self.goggles.is_camera_moving() { return true }
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
            if !self.goggles.has_camera() { return true }
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
