use {
    crate::{
        exports::runtime as rt,
        render::machine::{MumblelinkTick, RenderMachine},
    },
    anyhow::Context,
    core::{mem, ptr},
    glamour::{Point3, Vector3},
    nexus::rtapi::GameState,
    taimi_meta::{coords::LocalSpace, ui::GameplayState},
};

pub struct RenderStateRtapi {
    #[cfg(feature = "space")]
    pub camera: (Point3<LocalSpace>, Vector3<LocalSpace>),
    pub player: (Point3<LocalSpace>, Vector3<LocalSpace>),
    pub gameplay: u32,
    pub prev_map_id: u32,
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
}

impl RenderStateRtapi {
    pub const fn new() -> Self {
        Self {
            gameplay: GameState::CharacterSelection as u32,
            player: RenderMachine::POSITIONING_EMPTY,
            #[cfg(feature = "space")]
            camera: RenderMachine::POSITIONING_EMPTY,
            prev_map_id: 0,
        }
    }

    const GAMEPLAY_INGAME: u32 = GameState::Gameplay as _;
    const GAMEPLAY_LOADING: u32 = GameState::LoadingScreen as _;
    /// Vista viewing...
    const GAMEPLAY_CINEMATIC: u32 = GameState::Cinematic as _;

    pub fn update(
        &mut self,
        rtapi: &rt::RealTimeApi,
        ui_tick: Option<MumblelinkTick>,
        camera_wanted: bool,
    ) -> Option<GameplayState> {
        let prev_gameplay = self.gameplay;
        self.gameplay = unsafe { ptr::read_volatile(&raw const (*rtapi.as_ptr()).game_state) };
        let map_id = unsafe { ptr::read_volatile(&raw const (*rtapi.as_ptr()).map_id) };
        let prev_map_id = mem::replace(&mut self.prev_map_id, map_id);
        let gameplay_update = match self.gameplay {
            Self::GAMEPLAY_LOADING if map_id != prev_map_id => Some(
                GameplayState::new_loading(map_id, prev_map_id)
            ),
            Self::GAMEPLAY_INGAME if map_id != prev_map_id || prev_gameplay != Self::GAMEPLAY_INGAME =>
                Some(GameplayState::new_ingame(map_id)),
            state if state == prev_gameplay => None,
            Self::GAMEPLAY_LOADING => Some(
                GameplayState::new_loading(Default::default(), map_id)
            ),
            Self::GAMEPLAY_CINEMATIC => Some(GameplayState::new_loading(map_id, map_id)),
            state => GameState::try_from(state).ok().map(GameplayState::from),
        };

        let rtapi_ingame = self.gameplay == Self::GAMEPLAY_INGAME;
        let rtapi_camera = camera_wanted;
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
            .unwrap_or((Point3::INFINITY, Vector3::INFINITY));

        #[cfg(feature = "space")]
        #[cfg(todo)]
        let mut camera_fov = None;
        #[cfg(feature = "space")]
        let camera = {
            #[cfg(todo)]
            let needs_fov = self.fov_y().is_none();
            let needs_fov = false;
            let camera = (rtapi_ingame && (rtapi_camera || needs_fov)).then(|| {
                (
                    Point3::from_array(unsafe {
                        ptr::read_volatile(&raw const (*rtapi.as_ptr()).camera_position)
                    }),
                    Vector3::from_array(unsafe {
                        ptr::read_volatile(&raw const (*rtapi.as_ptr()).camera_facing)
                    }),
                    unsafe { ptr::read_volatile(&raw const (*rtapi.as_ptr()).camera_fov) },
                )
            });
            if let Some((camera_pos, camera_front, _fov)) = camera {
                #[cfg(todo)]
                if camera_fov.to_bits() != 0 {
                    camera_fov = Some(Vector2::ZERO.with_y(_fov));
                }
                match rt::vec_eq(camera_front, Vector3::ZERO) {
                    true => None,
                    false => Some((camera_pos, camera_front)),
                }
            } else {
                None
            }
        }
        .unwrap_or((Point3::INFINITY, Vector3::INFINITY));
        #[cfg(feature = "space")]
        {
            self.camera = camera;
        }

        // TODO: camera_fov
        gameplay_update
    }
}
