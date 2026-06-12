#[cfg(todo)]
use arcffi::wide::Utf16Display;
use {
    crate::exports::runtime::{self as rt, MumblePtr, UiState},
    std::{borrow::Cow, num::NonZero},
    taimi_meta::ui::mumblelink::gw2_mumble::Identity as MumbleIdentity,
    taimi_pack::script::{
        pathing::{InstanceVec3, ScriptApiMumble},
        value::{Size2U, Vec2, Vec3},
        Result,
        ScriptError,
    },
};

#[derive(Debug, Clone)]
pub struct ScriptHostMumbleLink {}
impl ScriptHostMumbleLink {
    pub fn new() -> Self {
        Self {}
    }
    #[inline(always)]
    pub fn with_ml_ptr<R, F>(&self, f: F) -> Option<R>
    where
        F: FnOnce(MumblePtr) -> R,
    {
        rt::mumble_link_ptr().ok().map(f)
    }
    /// XXX: avoid using this if a fallback can be sensibly returned
    pub fn try_with_ml_ptr<R, F>(&self, f: F) -> Result<R>
    where
        F: FnOnce(MumblePtr) -> R,
    {
        self.with_ml_ptr(f)
            .ok_or_else(|| ScriptError::msg("MumbleLink unavailable"))
    }

    #[inline]
    fn read_ui_state(&self) -> UiState {
        self.with_ml_ptr(|ml| UiState::from(ml.read_ui_state()))
            .unwrap_or(UiState::empty())
    }
    #[inline]
    fn ui_has_state(&self, flag: UiState) -> Result<bool> {
        Ok(self.read_ui_state().contains(flag))
    }

    /// TODO
    fn is_on_map(&self) -> bool {
        self.with_ml_ptr(|ml| ml.read_map_id() != 0).unwrap_or(false)
    }

    /// TODO
    fn with_identity<R, F>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&MumbleIdentity) -> R,
    {
        self.with_ml_ptr(|ml| ml.parse_identity().ok().map(|id| f(&id)))
            .flatten()
    }
    fn try_with_identity<R, F>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&MumbleIdentity) -> R,
    {
        self.with_identity(f)
            .ok_or_else(|| ScriptError::msg("TODO: fix ml identity"))
    }
}

impl ScriptApiMumble for ScriptHostMumbleLink {
    /// TODO: switch to state tracked by RenderMachine, and detect this better
    fn is_available(&self) -> bool {
        matches!(self.with_ml_ptr(drop), Some(()))
    }
    fn ui_tick(&self) -> Result<u32> {
        self.try_with_ml_ptr(|ml| ml.read_ui_tick())
    }
    #[cfg(todo)]
    fn since_ui_tick(&self) -> Result<TimeSpan> {}
    #[cfg(todo)]
    fn ticks_since_ui_tick(&self) -> Result<TickSpan> {}

    fn game_build(&self) -> Result<u32> {
        self.try_with_ml_ptr(|ml| ml.read_build_id())
    }
    fn game_focused(&self) -> Result<bool> {
        self.ui_has_state(UiState::GAME_HAS_FOCUS)
    }

    fn camera_forward(&self) -> Result<Vec3> {
        self.try_with_ml_ptr(|ml| {
            InstanceVec3::new_vec3(unsafe { *&raw const (*ml.as_ptr()).camera.front })
        })
    }
    fn camera_position(&self) -> Result<Vec3> {
        self.try_with_ml_ptr(|ml| {
            InstanceVec3::new_vec3(unsafe { *&raw const (*ml.as_ptr()).camera.position })
        })
    }

    fn player_mount(&self) -> Result<Option<NonZero<u32>>> {
        self.try_with_ml_ptr(|ml| NonZero::new(ml.read_mount_index() as _))
    }
    fn player_forward(&self) -> Result<Vec3> {
        self.try_with_ml_ptr(|ml| {
            InstanceVec3::new_vec3(unsafe { *&raw const (*ml.as_ptr()).avatar.front })
        })
    }
    fn player_position(&self) -> Result<Vec3> {
        self.try_with_ml_ptr(|ml| {
            InstanceVec3::new_vec3(unsafe { *&raw const (*ml.as_ptr()).avatar.position })
        })
    }
    fn is_in_combat(&self) -> Result<bool> {
        self.ui_has_state(UiState::IS_IN_COMBAT)
    }

    fn map_id(&self) -> Result<u32> {
        self.try_with_ml_ptr(|ml| ml.read_map_id())
    }
    fn map_is_competitive(&self) -> Result<bool> {
        self.ui_has_state(UiState::IS_IN_COMPETITIVE_MODE)
    }
    fn map_type(&self) -> Result<Option<u32>> {
        if !self.is_on_map() {
            return Ok(None)
        }
        self.try_with_ml_ptr(|ml| Some(ml.read_map_type()))
    }

    fn compass_rotation(&self) -> Result<f32> {
        self.try_with_ml_ptr(|ml| ml.read_compass_rotation())
    }
    fn compass_size(&self) -> Result<Size2U> {
        self.try_with_ml_ptr(|ml| {
            let [w, h] = ml.read_compass_dimensions();
            [w as u32, h as u32].into()
        })
    }
    fn is_compass_rotation_enabled(&self) -> Result<bool> {
        self.ui_has_state(UiState::DOES_COMPASS_HAVE_ROTATION_ENABLED)
    }
    fn is_compass_top_right(&self) -> Result<bool> {
        self.ui_has_state(UiState::IS_COMPASS_TOP_RIGHT)
    }
    fn is_map_open(&self) -> Result<bool> {
        self.ui_has_state(UiState::IS_MAP_OPEN)
    }
    fn is_text_input_focused(&self) -> Result<bool> {
        self.ui_has_state(UiState::TEXTBOX_HAS_FOCUS)
    }
    fn map_centre(&self) -> Result<Vec2> {
        self.try_with_ml_ptr(|ml| ml.read_map_center().into())
    }
    fn map_position(&self) -> Result<Vec2> {
        self.try_with_ml_ptr(|ml| ml.read_player_position().into())
    }
    fn map_scale(&self) -> Result<f32> {
        self.try_with_ml_ptr(|ml| ml.read_map_scale())
    }

    /// TODO: get from RenderMachine
    #[cfg(todo)]
    fn camera_clip_planes(&self) -> Result<ops::Range<f32>> {}
    /// TODO: get from Controller
    #[cfg(todo)]
    fn is_lieutenant(&self) -> Result<bool> {}

    // identity
    fn camera_fov(&self) -> Result<f32> {
        self.try_with_identity(|id| id.fov)
    }
    fn ui_size(&self) -> Result<u32> {
        self.try_with_identity(|id| id.ui_scale as _)
    }
    fn player_race(&self) -> Result<u32> {
        self.try_with_identity(|id| id.race as _)
    }
    fn player_profession(&self) -> Result<u32> {
        self.try_with_identity(|id| id.profession as _)
    }
    fn player_spec(&self) -> Result<u32> {
        self.try_with_identity(|id| id.spec)
    }
    fn player_team_colour_id(&self) -> Result<u32> {
        self.try_with_identity(|id| id.team_color_id)
    }
    fn character_name(&self) -> Result<Cow<'_, str>> {
        self.try_with_identity(|id| id.name.clone().into())
    }
    fn is_commander(&self) -> Result<bool> {
        self.try_with_identity(|id| id.commander)
    }

    /// TODO: update arcffi!
    ///
    /// TODO: double-check this?
    #[cfg(todo)]
    fn map_name(&self) -> Result<Option<Cow<'_, str>>> {
        if !self.is_on_map() {
            return Ok(None)
        }
        self.try_with_ml_ptr(|ml| unsafe {
            let name_bytes = &*&raw const (*ml.as_ptr()).name;
            let len = name_bytes.iter().position(|c| c == 0).unwrap_or(name_bytes.len());
            let name = name_bytes.get_unchecked(..len);
            Some(Cow::Owned(Utf16Display::from_data(name).to_string()))
        });
    }
}
