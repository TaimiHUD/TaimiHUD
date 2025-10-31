use crate::exports::{
    self,
    runtime::{self as rt, imgui, RuntimeResult},
};

/// TODO: implement runtime alert
/// push to controller alert queue or something?
pub fn send_alert(ui: &imgui::Ui, message: &str) -> RuntimeResult<()> {
    #[cfg(feature = "extension-nexus")]
    if let Some(res) = exports::nexus::send_alert(ui, message)? {
        return Ok(res)
    }

    #[cfg(feature = "extension-arcdps")]
    if let Some(res) = exports::arcdps::send_alert(ui, message)? {
        return Ok(res)
    }

    Err(rt::RT_UNAVAILABLE)
}
