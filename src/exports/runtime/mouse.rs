use {
    anyhow::Context,
    crate::exports::runtime::{self as rt, RuntimeResult},
    taimi_input::win::mouse,
};
#[cfg(feature = "markers")]
use taimi_meta::coords::ScreenPoint;

pub use taimi_input::win::mouse::*;

#[cfg(feature = "markers")]
#[cfg(todo = "unused")]
pub fn mouse_position_on_screen(point: glam::Vec2) -> MousePosition {
    use crate::render::machine::RenderMachine;

    let display_size = match RenderMachine::shared_map_state().blocking_lock().calibration.display_size {
        sz if sz.width == 0.0 => {
            log::error!("screen size missing");
            return mouse_position_from_screen(point.as_())
        },
        sz => sz,
    };
    MousePosition {
        x: (point.x * display_size.width).round() as i32,
        y: (point.y * display_size.height).round() as i32,
    }
}

#[cfg(feature = "markers")]
pub fn mouse_position_from_screen(point: ScreenPoint) -> MousePosition {
    MousePosition {
        x: point.x.round() as i32,
        y: point.y.round() as i32,
    }
}

pub fn send_mouse(input: MouseInput, prior: Option<MouseInput>) -> RuntimeResult<()> {
    let hwnd = rt::window_handle()?;
    let res = mouse::send_mouse(hwnd, input, prior)
        .context("sending mouse");
    res.map_err(|e| {
        log::warn!("{e:#}");
        "Failed to send mouse"
    })
}

pub fn send_input<I: Into<MouseInput>>(input: I) -> RuntimeResult<()> {
    let hwnd = rt::window_handle()?;
    let res = mouse::send_input(hwnd, input)
        .context("sending mouse input");
    res.map_err(|e| {
        log::warn!("{e:#}");
        "Failed to send mouse input"
    })
}
