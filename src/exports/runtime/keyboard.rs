use {
    anyhow::Context,
    crate::exports::runtime::{self as rt, RuntimeResult},
    taimi_input::win::keyboard,
};

pub use taimi_input::win::keyboard::*;

pub fn do_key_combo<R, F: FnOnce() -> Result<R, rt::RuntimeError>>(f: F, input: KeyInput) -> RuntimeResult<R> {
    let hwnd = rt::window_handle()?;
    keyboard::do_key_combo(hwnd, || f().map_err(anyhow::Error::msg), input)
        .with_context(|| format!("Sending key combo {input:?}"))
        .map_err(|e| {
            log::warn!("{e:#}");
            "Failed to send key combo"
        })
}

pub fn send_key(input: KeyInput) -> RuntimeResult<()> {
    let (msg, w, l) = input.to_event();
    unsafe {
        rt::window_message(msg, w, l)
    }
}

pub fn send_key_input<I: Into<KeyInput>>(input: I) -> RuntimeResult<()> {
    let hwnd = rt::window_handle()?;
    let input = input.into();
    keyboard::send_key_input(hwnd, input)
        .with_context(|| format!("Sending key input {input:?}"))
        .map_err(|e| {
            log::warn!("{e:#}");
            "Failed to send key input"
        })
}
