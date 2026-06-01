use {
    crate::exports::{
        self,
        runtime::{self as rt, RuntimeResult},
    },
    taimi_ui::im::colours::{ImColourContainer, ImColourIndex},
};

/// TODO: implement runtime alert
/// push to controller alert queue or something?
pub fn send_alert<U>(ui: &U, message: &str) -> RuntimeResult<()>
where
    U: ?Sized + ImColourContainer<LogWarningColour>,
{
    #[cfg(feature = "extension-nexus")]
    if let Some(res) = exports::nexus::send_alert(message)? {
        return Ok(res)
    }

    #[cfg(feature = "extension-arcdps")]
    if let Some(res) = exports::arcdps::send_alert(ui, message)? {
        return Ok(res)
    }

    Err(rt::RT_UNAVAILABLE)
}

pub struct LogWarningColour;
#[cfg(taimi_imgui = "180")]
impl ImColourContainer<LogWarningColour> for taimi_ui::im::im180::Ui<'_> {
    fn lookup_style_colour(&self, _: LogWarningColour) -> taimi_ui::im::colours::ImColour {
        self.lookup_style_colour(ImColourIndex::NavCursor)
    }
}
#[cfg(taimi_imgui = "192")]
impl ImColourContainer<LogWarningColour> for taimi_ui::im::im192::Ui<'_> {
    fn lookup_style_colour(&self, _: LogWarningColour) -> taimi_ui::im::colours::ImColour {
        self.lookup_style_colour(ImColourIndex::NavCursor)
    }
}
