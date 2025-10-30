pub use taimi_input::win::keyboard::*;
use {
    crate::exports::runtime::{self as rt, RuntimeResult},
    anyhow::Context,
    taimi_input::win::keyboard,
};

pub fn do_key_combo<R, F: FnOnce() -> Result<R, rt::RuntimeError>>(
    f: F,
    input: KeyInput,
) -> RuntimeResult<R> {
    let hwnd = rt::window_handle()?;
    keyboard::do_key_combo(hwnd, || f().map_err(anyhow::Error::msg), input)
        .with_context(|| format!("Sending key combo {input:?}"))
        .map_err(|e| {
            log::warn!("{e:#}");
            "Failed to send key combo"
        })
}
pub fn send_key_combo(input: KeyInput) -> RuntimeResult<()> {
    let hwnd = rt::window_handle()?;
    keyboard::send_key_combo(hwnd, input)
        .with_context(|| format!("Sending key combo {input:?}"))
        .map_err(|e| {
            log::warn!("{e:#}");
            "Failed to send key combo"
        })
}

pub fn send_key(input: KeyInput) -> RuntimeResult<()> {
    let (msg, w, l) = input.to_event();
    unsafe { rt::window_message(msg, w, l) }
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

pub async fn press_bind(
    control: rt::bindings::Control,
    down: bool,
    position: Option<rt::MousePosition>,
) -> RuntimeResult<Option<()>> {
    use crate::{
        exports::runtime::mouse::MouseInput,
        settings::{state::SaveState, InvokeMethod, Settings},
    };

    let method = Settings::async_read()
        .await
        .ok()
        .and_then(|s| s.arc().gamebind_invoke)
        .unwrap_or_default();

    let (vk, mut mods) =
        SaveState::read_with(|s| s.game_binds.get(control, None)).ok_or("unknown keybind")?;

    match rt::bindings::GameBinds::vk_is_button(vk) {
        false => {
            if let Some(position) = position {
                // move the mouse into position first...
                match method {
                    InvokeMethod::Message =>
                        rt::mouse::send_mouse(MouseInput::with_position(position), None)?,
                    InvokeMethod::Input | _ => rt::mouse::send_input(MouseInput::with_position(position))?,
                }
            }
            let input = KeyInput::new(vk, mods, down);
            match method {
                InvokeMethod::Message => rt::keyboard::send_key_combo(input),
                InvokeMethod::Input | _ => rt::keyboard::send_key_input(input),
            }
        },
        true => {
            let button = KeyState::try_from_vk(vk)
                .context("Unsupported mouse key")
                .map_err(|e| {
                    log::warn!("{e:#}");
                    "Unsupported mouse key"
                })?;
            let pos = match position {
                Some(p) => p,
                None => rt::window_mouse_position()?,
            };
            let input = MouseInput::new(pos, button | mods, Some(down));
            let prior = match (method, position) {
                // ensure the mouse is moved if a position was explicitly requested
                (InvokeMethod::Input, Some(..)) => Some(MouseInput::new(
                    rt::MousePosition::EMPTY,
                    input.button_before(),
                    None,
                )),
                _ => None,
            };
            let mouse_mods = match mods.take(MouseInput::EVENT_MODS) {
                mouse_mods if !mods.is_empty() => {
                    // can't eliminate the need to simulate modifier key presses, so just move all mods to that
                    mods.insert(mouse_mods);
                    KeyState::EMPTY
                },
                mouse_mods => mouse_mods,
            };

            let invoke = || match (method, mouse_mods.is_empty()) {
                (InvokeMethod::Input, true) /*if position.is_none()*/ => rt::mouse::send_input(input),
                _ => rt::mouse::send_mouse(input, prior),
            };
            match mods.is_empty() {
                true => invoke(),
                false => rt::keyboard::do_key_combo(invoke, KeyInput::empty_with_mods(mods, down)),
            }
        },
    }
    .map(Some)
}
#[cfg(feature = "markers")]
pub async fn press_marker_bind(
    marker: crate::marker::format::MarkerType,
    target: bool,
    down: bool,
    position: Option<rt::MousePosition>,
) -> RuntimeResult<Option<()>> {
    let control = match target {
        true => marker.control_object(),
        false => marker.control_location(),
    };
    press_bind(control.into(), down, position).await
}
