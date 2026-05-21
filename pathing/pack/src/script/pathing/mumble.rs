use {
    crate::script::{
        script_unimpl,
        value::{Size2U, TickSpan, TimeSpan, Vec2, Vec3},
        Result,
    },
    core::{num::NonZero, ops},
    std::borrow::Cow,
};

pub trait ScriptApiMumble {
    fn is_available(&self) -> bool {
        false
    }
    fn ui_tick(&self) -> Result<u32> {
        script_unimpl!()
    }
    fn since_ui_tick(&self) -> Result<TimeSpan> {
        script_unimpl!()
    }
    fn ticks_since_ui_tick(&self) -> Result<TickSpan> {
        script_unimpl!()
    }

    fn game_build(&self) -> Result<u32> {
        script_unimpl!()
    }
    fn game_focused(&self) -> Result<bool> {
        script_unimpl!()
    }

    fn camera_clip_planes(&self) -> Result<ops::Range<f32>> {
        script_unimpl!()
    }
    fn camera_fov(&self) -> Result<f32> {
        script_unimpl!()
    }
    fn camera_forward(&self) -> Result<Vec3> {
        script_unimpl!()
    }
    fn camera_position(&self) -> Result<Vec3> {
        script_unimpl!()
    }

    fn player_mount(&self) -> Result<Option<NonZero<u32>>> {
        script_unimpl!()
    }
    fn player_forward(&self) -> Result<Vec3> {
        script_unimpl!()
    }
    fn player_position(&self) -> Result<Vec3> {
        script_unimpl!()
    }
    fn player_race(&self) -> Result<u32> {
        script_unimpl!()
    }
    fn player_profession(&self) -> Result<u32> {
        script_unimpl!()
    }
    fn player_spec(&self) -> Result<u32> {
        script_unimpl!()
    }
    fn player_team_colour_id(&self) -> Result<u32> {
        script_unimpl!()
    }
    fn character_name(&self) -> Result<Cow<'_, str>> {
        script_unimpl!()
    }
    fn is_in_combat(&self) -> Result<bool> {
        script_unimpl!()
    }
    fn is_commander(&self) -> Result<bool> {
        script_unimpl!()
    }
    fn is_lieutenant(&self) -> Result<bool> {
        script_unimpl!()
    }

    fn map_id(&self) -> Result<u32> {
        script_unimpl!()
    }
    fn map_is_competitive(&self) -> Result<bool> {
        script_unimpl!()
    }
    fn map_type(&self) -> Result<Option<u32>> {
        script_unimpl!()
    }
    fn map_name(&self) -> Result<Option<Cow<'_, str>>> {
        script_unimpl!()
    }

    fn compass_rotation(&self) -> Result<f32> {
        script_unimpl!()
    }
    fn compass_size(&self) -> Result<Size2U> {
        script_unimpl!()
    }
    fn is_compass_rotation_enabled(&self) -> Result<bool> {
        script_unimpl!()
    }
    fn is_compass_top_right(&self) -> Result<bool> {
        script_unimpl!()
    }
    fn is_map_open(&self) -> Result<bool> {
        script_unimpl!()
    }
    fn is_text_input_focused(&self) -> Result<bool> {
        script_unimpl!()
    }
    fn map_centre(&self) -> Result<Vec2> {
        script_unimpl!()
    }
    fn map_position(&self) -> Result<Vec2> {
        script_unimpl!()
    }
    fn map_scale(&self) -> Result<f32> {
        script_unimpl!()
    }
    fn ui_size(&self) -> Result<u32> {
        script_unimpl!()
    }
}
