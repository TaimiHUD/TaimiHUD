use {anyhow::Context, core::mem};

pub mod keyboard;
pub mod mouse;

pub use {
    self::{keyboard::KeyState, mouse::MousePosition},
    windows::{
        core::Error as WinError,
        Win32::{
            Foundation::{HWND, LPARAM, WPARAM},
            UI::{Input::KeyboardAndMouse, WindowsAndMessaging},
        },
    },
};

pub unsafe fn window_message(hwnd: HWND, msg: u32, w: usize, l: isize) -> anyhow::Result<()> {
    WindowsAndMessaging::PostMessageA(Some(hwnd), msg, WPARAM(w), LPARAM(l)).context("PostMessageA")
}

pub fn window_send_inputs<I: Into<KeyboardAndMouse::INPUT>>(
    hwnd: HWND,
    inputs: impl IntoIterator<Item = I>,
) -> anyhow::Result<()> {
    // TODO: bail out if window isn't focused or something
    let inputs: Vec<_> = inputs.into_iter().map(I::into).collect();
    let res =
        unsafe { KeyboardAndMouse::SendInput(&inputs[..], mem::size_of::<KeyboardAndMouse::INPUT>() as _) };
    match res {
        0 => Err(WinError::from_win32()),
        _ => Ok(()),
    }
    .context("SendInput")
}
