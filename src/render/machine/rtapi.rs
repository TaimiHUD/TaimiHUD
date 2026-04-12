#[cfg(feature = "space")]
use taimi_hoard::vec::vec32_eq;
use {
    crate::{
        exports::runtime as rt,
        render::machine::{MumblelinkTick, RenderMachine, RenderPositioning},
    },
    anyhow::Context,
    core::{mem, ptr},
    glamour::{Point3, Vector3},
    nexus::rtapi::GameState,
    taimi_meta::{coords::LocalSpace, ui::GameplayState},
};

pub struct RenderStateRtapi {
    #[cfg(feature = "space")]
    pub camera: RenderPositioning,
    #[cfg(feature = "space")]
    pub camera_fov_y: f32,
    #[cfg(feature = "space")]
    pub camera_tick: u32,
    pub player: RenderPositioning,
    pub gameplay: u32,
    pub prev_map_id: u32,
    pub gameplay_count: u32,
}

/// shhh [rt::RealTimeApi] is fine to share tbh
unsafe impl Send for RenderMachine {}

impl RenderMachine {
    pub fn rtapi_wanted(&self) -> bool {
        !self.rtapi_users.is_empty()
    }

    pub fn rtapi_setup(&mut self) {
        let rtapi = Self::rtapi_open();
        #[cfg(feature = "extension-nexus")]
        match &rtapi {
            Ok(Some(rtapi)) if !rtapi.is_active() => log::info!("RTAPI unavailable"),
            Ok(Some(..)) => log::info!("Using RTAPI as perspective data source"),
            Err(e) => {
                // TODO: listen for events in case it gets loaded later or something
                log::debug!("{e:#}");
            },
            _ => (),
        }
        self.rtapi = rtapi.ok().flatten();
    }

    pub fn rtapi_init(&mut self) -> anyhow::Result<bool> {
        if self.rtapi.is_none() {
            self.rtapi = Self::rtapi_open()?;
        }

        let active = self.rtapi.as_ref().map(|rtapi| rtapi.is_active());

        Ok(active.unwrap_or(false))
    }

    pub fn rtapi_open() -> anyhow::Result<Option<rt::RealTimeApi>> {
        let rtapi = rt::rtapi()
            .map_err(anyhow::Error::msg)
            .context("RTAPI unavailable");
        if let Ok(Some(rtapi)) = &rtapi {
            let game_lang = unsafe { *&raw const (*rtapi.as_ptr()).language };
            rt::notify_game_language(game_lang as _);
        }
        match rtapi {
            #[cfg(not(feature = "extension-nexus"))]
            Err(_) => Ok(None),
            res => res,
        }
    }

    pub fn rtapi_camera_sync(&mut self) {
        let render_tick = self.mumblelink_frames.latest_render_tick();
        let render_ui_tick = self.mumblelink_frames.render_to_uitick(render_tick);
        let upcoming_ui_tick = render_ui_tick.wrapping_add(1);

        if !self.rtapi_state.has_camera()
            || self.rtapi_state.camera_tick == render_tick
            || self.mumblelink_frames.ui_skip > 0
        {
            return
        }
        // though RTAPI patches ML to provide similar data, it will not be bit-identical!
        let cmp_pos = |lhs: Point3<LocalSpace>, rhs: glam::Vec3A| {
            glam::Vec3A::from(lhs.to_vector()).abs_diff_eq(rhs, 5e-5)
        };
        let cmp_dir =
            |lhs: Vector3<LocalSpace>, rhs: glam::Vec3A| glam::Vec3A::from(lhs).abs_diff_eq(rhs, 2e-5);

        let matches_upcoming = self
            .camera
            .mumblelink
            .get_at(upcoming_ui_tick)
            .map(|cam| cmp_pos(self.rtapi_state.camera.0, cam.pos));
        let Some(matches_upcoming) = matches_upcoming else { return };
        let render_pos = self.camera.mumblelink.get_at(render_ui_tick);
        let matches_render = || {
            render_pos.map(|cam| {
                cmp_pos(self.rtapi_state.camera.0, cam.pos) && cmp_dir(self.rtapi_state.camera.1, cam.front)
            })
        };
        // if just got new ml update that != rtapi... if prior frame == rtapi then mark early. if prior missing then fill in!
        if matches_upcoming {
            // late or early in both cases!
            #[cfg(todo)]
            if self.mumblelink_frames.is_early() && matches_render() == Some(false) {
                self.mumblelink_frames.mark_late();
                self.camera.resync_with_frames(&self.mumblelink_frames);
            }
        } else {
            // late ml but early rt
            if render_pos.is_none() {
                #[cfg(taimi_debug)]
                log::debug!(
                    "RTAPI(resync) {render_pos:?} matches_render={:?}",
                    matches_render()
                );
                return;
                self.camera.record_mumblelink(
                    render_ui_tick,
                    (
                        self.rtapi_state.camera.0,
                        self.rtapi_state.camera.1,
                        Vector3::ZERO,
                    ),
                );
            } else if !self.mumblelink_frames.is_early() && matches_render() == Some(true) {
                #[cfg(taimi_debug)]
                log::debug!("RTAPI(resync) {render_pos:?}");
                return;
                self.mumblelink_frames.mark_early();
                self.camera.resync_with_frames(&self.mumblelink_frames);
            }
        }

        self.rtapi_state.camera_tick = render_tick;
    }
}

impl RenderStateRtapi {
    pub const fn new() -> Self {
        Self {
            gameplay: Self::GAMEPLAY_NONE,
            player: RenderMachine::POSITIONING_EMPTY,
            #[cfg(feature = "space")]
            camera: RenderMachine::POSITIONING_EMPTY,
            #[cfg(feature = "space")]
            camera_fov_y: 0.0f32,
            #[cfg(feature = "space")]
            camera_tick: 0,
            prev_map_id: 0,
            gameplay_count: 0,
        }
    }

    const GAMEPLAY_INGAME: u32 = GameState::Gameplay as _;
    const GAMEPLAY_LOADING: u32 = GameState::LoadingScreen as _;
    /// Vista viewing...
    const GAMEPLAY_CINEMATIC: u32 = GameState::Cinematic as _;
    const GAMEPLAY_CHARSEL: u32 = GameState::CharacterSelection as _;
    const GAMEPLAY_NONE: u32 = u32::MAX - 1;

    pub fn update(
        &mut self,
        rtapi: &rt::RealTimeApi,
        ui_tick: Option<MumblelinkTick>,
        camera_wanted: bool,
    ) -> Option<GameplayState> {
        let prev_gameplay = mem::replace(&mut self.gameplay, unsafe {
            ptr::read_volatile(&raw const (*rtapi.as_ptr()).game_state)
        });
        let gameplay_delay = match self.gameplay {
            Self::GAMEPLAY_LOADING => {
                // game "loads" for a handful of frames prior to starting a cutscene such as a vista...
                // unfortunately I think this means we can't tell if loading actually means map load or not? :<
                Self::COUNT_UNCERTAIN_LOADING
            },
            _ => Self::COUNT_FRESH,
        };
        if self.gameplay != prev_gameplay {
            self.gameplay_count = match prev_gameplay {
                Self::GAMEPLAY_NONE => Self::COUNT_AWHILE,
                prev if self.gameplay == Self::GAMEPLAY_LOADING && prev != Self::GAMEPLAY_INGAME =>
                    Self::COUNT_UNCERTAIN_LOADING,
                _ => Self::COUNT_FRESH,
            }
        } else {
            self.gameplay_count = self.gameplay_count.saturating_add(1);
        }
        let gameplay_counts = self.gameplay_count >= gameplay_delay;
        let map_id = unsafe { ptr::read_volatile(&raw const (*rtapi.as_ptr()).map_id) };
        let prev_map_id = mem::replace(&mut self.prev_map_id, map_id);
        let gameplay_update = match self.gameplay {
            Self::GAMEPLAY_LOADING if map_id != prev_map_id =>
                Some(GameplayState::new_loading(map_id, prev_map_id)),
            Self::GAMEPLAY_INGAME if map_id != prev_map_id || prev_gameplay != Self::GAMEPLAY_INGAME =>
                Some(GameplayState::new_ingame(map_id)),
            _ if !gameplay_counts || self.gameplay_count >= Self::COUNT_EMITTED => None,
            Self::GAMEPLAY_LOADING => Some(GameplayState::new_loading(Default::default(), map_id)),
            Self::GAMEPLAY_CINEMATIC => Some(GameplayState::new_loading(map_id, map_id)),
            state => GameState::try_from(state).ok().map(GameplayState::from),
        };
        if gameplay_update.is_some() {
            // .max(self.gameplay_count)?
            self.gameplay_count = Self::COUNT_EMITTED;
        }

        let rtapi_ingame = self.gameplay == Self::GAMEPLAY_INGAME;
        #[cfg(todo)]
        let rtapi_camera = camera_wanted;
        let rtapi_camera = rtapi_ingame;
        let player_tick = ui_tick.map(|tick| tick.is_player()).unwrap_or(false);

        self.player = (rtapi_ingame && (rtapi_camera || !player_tick))
            .then(|| {
                (
                    Point3::from_array(unsafe {
                        ptr::read_volatile(&raw const (*rtapi.as_ptr()).character_position)
                    }),
                    Vector3::from_array(unsafe {
                        ptr::read_volatile(&raw const (*rtapi.as_ptr()).character_facing)
                    }),
                )
            })
            .and_then(|(pos, front)| match rt::vec_eq(front, Vector3::ZERO) {
                true => None,
                false => Some((pos, front)),
            })
            .unwrap_or(RenderMachine::POSITIONING_EMPTY);

        #[cfg(feature = "space")]
        {
            #[cfg(todo)]
            let needs_fov = self.fov_y().is_none();
            let needs_fov = false;
            if rtapi_ingame && (rtapi_camera || needs_fov) {
                if self.update_camera(rtapi) {
                    self.mark_camera_unchanged()
                }
            } else {
                self.clear_camera();
            }
        }

        gameplay_update
    }

    #[cfg(feature = "goggles2-camera")]
    pub fn is_intermission(&self) -> bool {
        match self.gameplay {
            Self::GAMEPLAY_LOADING => true,
            Self::GAMEPLAY_CHARSEL if self.prev_map_id != 0 => true,
            _ => false,
        }
    }
    pub fn is_cutscene(&self) -> bool {
        if self.is_loading_uncertain() {
            return true
        }
        self.gameplay == Self::GAMEPLAY_CINEMATIC /* && self.is_ingame()*/
    }
    #[cfg(todo)]
    pub fn is_ingame(&self) -> bool {
        !self.player.0.x.is_infinite()
    }
    #[cfg(feature = "space")]
    pub fn has_camera(&self) -> bool {
        !self.camera.0.x.is_infinite()
    }
    #[cfg(feature = "space")]
    pub fn camera_fov_y(&self) -> Option<f32> {
        (self.camera_fov_y.to_bits() != 0.0f32.to_bits()).then_some(self.camera_fov_y)
    }
    /// 3 or 4 might be enough, but bleh...
    const COUNT_UNCERTAIN_LOADING: u32 = 6;
    const COUNT_AWHILE: u32 = 0x400;
    const COUNT_EMITTED: u32 = 0x800;
    const COUNT_FRESH: u32 = 0;
    pub fn is_loading_uncertain(&self) -> bool {
        self.gameplay == Self::GAMEPLAY_LOADING && self.gameplay_count < Self::COUNT_UNCERTAIN_LOADING
    }

    pub fn is_active(&self) -> bool {
        self.gameplay != Self::GAMEPLAY_NONE
    }
    pub fn set_inactive(&mut self) {
        self.gameplay = Self::GAMEPLAY_NONE;
        self.player = RenderMachine::POSITIONING_EMPTY;
        #[cfg(feature = "space")]
        {
            self.camera = RenderMachine::POSITIONING_EMPTY;
            self.camera_fov_y = 0.0f32;
        }
    }

    #[cfg(feature = "space")]
    pub fn read_camera(rtapi: &rt::RealTimeApi) -> (RenderPositioning, f32) {
        let timeofday = unsafe { ptr::read_volatile(&raw const (*rtapi.as_ptr()).time_of_day) };
        let (mut pos, mut front, fov) = {
            (
                Point3::from_array(unsafe {
                    ptr::read_volatile(&raw const (*rtapi.as_ptr()).camera_position)
                }),
                Vector3::from_array(unsafe {
                    ptr::read_volatile(&raw const (*rtapi.as_ptr()).camera_facing)
                }),
                unsafe { ptr::read_volatile(&raw const (*rtapi.as_ptr()).camera_fov) },
            )
        };
        for _ in 0..=2 {
            std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);

            let (pos_reread, front_reread) = (
                Point3::from_array(unsafe {
                    ptr::read_volatile(&raw const (*rtapi.as_ptr()).camera_position)
                }),
                Vector3::from_array(unsafe {
                    ptr::read_volatile(&raw const (*rtapi.as_ptr()).camera_facing)
                }),
            );
            if !vec32_eq(pos, pos_reread) || !vec32_eq(front, front_reread) {
                let timeofday_reread =
                    unsafe { ptr::read_volatile(&raw const (*rtapi.as_ptr()).time_of_day) };
                pos = pos_reread;
                front = front_reread;
                log::error!("RTAPI INCOMPLETE READ??? @ToD={timeofday_reread} (prev {timeofday})");
            } else {
                break
            }
        }
        ((pos, front), fov)
    }
    #[cfg(feature = "space")]
    pub fn update_camera(&mut self, rtapi: &rt::RealTimeApi) -> bool {
        let ((camera_pos, camera_front), fov) = Self::read_camera(rtapi);
        let camera = match rt::vec_eq(camera_front, Vector3::ZERO) {
            true => None,
            false => Some((camera_pos, camera_front)),
        };
        let changed = if let Some((pos, front)) = &camera {
            if vec32_eq(*pos, self.camera.0) && vec32_eq(*front, self.camera.1) {
                return false
            }
            true
        } else {
            false
        };
        self.camera = camera.unwrap_or(RenderMachine::POSITIONING_EMPTY);
        self.camera_fov_y = fov;
        changed
    }
    #[cfg(feature = "space")]
    pub fn mark_camera_unchanged(&mut self) {
        self.camera_tick = self.camera_tick.wrapping_add(1);
    }
    #[cfg(feature = "space")]
    pub fn clear_camera(&mut self) {
        self.camera = RenderMachine::POSITIONING_EMPTY;
        self.camera_fov_y = 0.0f32;
    }
}
