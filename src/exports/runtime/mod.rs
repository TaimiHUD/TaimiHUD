use std::{ffi::CStr, mem, path::{Path, PathBuf}, ptr::NonNull, sync::Mutex, time::Duration};
use ::log::info;
use nexus::{data_link::{mumble::MumblePtr, NexusLink}, rtapi::RealTimeApi};
use crate::{exports, load_language, marker::format::MarkerType};
use windows::Win32::{
    Foundation::{HWND, LPARAM, WPARAM},
    UI::{
        WindowsAndMessaging,
        Input::KeyboardAndMouse,
    },
};
#[cfg(feature = "texture-loader")]
use crate::TEXTURES;

pub mod keyboard;
pub mod log;
pub mod mouse;
pub mod textures;
pub use {
    nexus::imgui,
    self::{
        mouse::MousePosition,
        keyboard::KeyState,
        textures::TextureLoader,
    },
};

pub type RuntimeError = &'static str;
pub type RuntimeResult<T = ()> = Result<T, RuntimeError>;
pub const RT_UNAVAILABLE: RuntimeError = "extension runtime unavailable";

pub const CRATE_NAME: &'static str = env!("CARGO_PKG_NAME");
pub const CRATE_VERSION: &'static str = env!("CARGO_PKG_VERSION");
pub const NAME: &'static str = "TaimiHUD";
pub const NAME_C: &'static CStr = unsafe {
    CStr::from_bytes_with_nul_unchecked(b"TaimiHUD\0")
};

pub static LOADER_LOCK: Mutex<bool> = Mutex::new(false);

pub fn nexus_available() -> bool {
    match () {
        #[cfg(feature = "extension-nexus")]
        () => exports::nexus::available(),
        #[cfg(not(feature = "extension-nexus"))]
        _ => false,
    }
}

#[cfg(todo)]
pub fn arcdps_available() -> bool {
    match () {
        #[cfg(feature = "extension-arcdps")]
        () => exports::arcdps::available(),
        #[cfg(not(feature = "extension-arcdps"))]
        _ => false,
    }
}

pub fn addon_dir() -> RuntimeResult<PathBuf> {
    #[cfg(feature = "extension-nexus")]
    if let Some(path) = exports::nexus::addon_dir()? {
        return Ok(path)
    }

    #[cfg(feature = "extension-arcdps")]
    if let Some(path) = exports::arcdps::addon_dir()? {
        return Ok(path)
    }

    Err(RT_UNAVAILABLE)
}

pub fn detect_language() -> RuntimeResult<String> {
    #[cfg(feature = "extension-nexus")]
    if let Some(lang) = exports::nexus::detect_language()? {
        return Ok(lang)
    }

    #[cfg(feature = "extension-arcdps")]
    if let Some(lang) = exports::arcdps::detect_language()? {
        return Ok(lang)
    }

    Err(RT_UNAVAILABLE)
}

pub fn reload_language() -> RuntimeResult {
    let language = detect_language()?;
    info!("Detected language {language} for internationalization");

    load_language(&language)
}

pub fn mumble_link_ptr() -> RuntimeResult<MumblePtr> {
    #[cfg(feature = "extension-nexus")]
    if let Some(ml) = exports::nexus::mumble_link_ptr()? {
        return Ok(ml)
    }

    #[cfg(feature = "extension-arcdps")]
    if let Some(ml) = exports::arcdps::mumble_link_ptr()? {
        return Ok(unsafe {
            mem::transmute(ml)
        })
    }

    Err(RT_UNAVAILABLE)
}

pub fn nexus_link_ptr() -> RuntimeResult<NonNull<NexusLink>> {
    #[cfg(feature = "extension-nexus")]
    if let Some(nl) = exports::nexus::nexus_link_ptr()? {
        return Ok(nl)
    }

    #[cfg(feature = "extension-arcdps")]
    if let Some(nl) = exports::arcdps::nexus_link_ptr()? {
        return Ok(nl)
    }

    Err(RT_UNAVAILABLE)
}

pub fn read_nexus_link() -> RuntimeResult<NexusLink> {
    nexus_link_ptr()
        .map(|p| unsafe { p.read_volatile() })
}

pub fn is_ingame() -> RuntimeResult<bool> {
    if let Ok(nexus_link) = read_nexus_link() {
        return Ok(nexus_link.is_gameplay);
    }

    #[cfg(feature = "extension-arcdps")]
    if let Some(ingame) = exports::arcdps::is_ingame() {
        return Ok(ingame)
    }

    Err(RT_UNAVAILABLE)
}

pub fn rtapi() -> RuntimeResult<Option<RealTimeApi>> {
    #[cfg(feature = "extension-nexus")]
    if let Some(rtapi) = exports::nexus::rtapi()? {
        return Ok(Some(rtapi))
    }

    #[cfg(feature = "extension-arcdps")]
    if let Some(rtapi) = exports::arcdps::rtapi()? {
        return Ok(Some(rtapi))
    }

    Err(RT_UNAVAILABLE)
}

pub async fn press_marker_bind(marker: MarkerType, target: bool, down: bool, position: Option<MousePosition>) -> RuntimeResult<()> {
    #[cfg(feature = "extension-nexus")]
    if let Some(res) = exports::nexus::press_marker_bind(marker, target, down, position).await? {
        return Ok(res)
    }

    #[cfg(feature = "extension-arcdps")]
    if let Some(res) = exports::arcdps::press_marker_bind(marker, target, down, position).await? {
        return Ok(res)
    }

    Err(RT_UNAVAILABLE)
}

pub async fn invoke_marker_bind(marker: MarkerType, target: bool, duration: Duration, position: Option<MousePosition>) -> RuntimeResult<()> {
    press_marker_bind(marker, target, true, position).await?;

    tokio::time::sleep(duration).await;

    #[cfg(feature = "extension-nexus")]
    let position = match exports::nexus::available() {
        false => position,
        true => None,
    };

    press_marker_bind(marker, target, false, position).await
}

#[cfg(any(feature = "space", feature = "texture-loader"))]
pub fn dxgi_swap_chain() -> RuntimeResult<Option<windows::Win32::Graphics::Dxgi::IDXGISwapChain>> {
    #[cfg(feature = "extension-nexus")]
    if let Some(swap_chain) = exports::nexus::dxgi_swap_chain()? {
        return Ok(Some(swap_chain))
    }

    #[cfg(feature = "extension-arcdps")]
    if let Some(swap_chain) = exports::arcdps::dxgi_swap_chain()? {
        return Ok(Some(swap_chain))
    }

    Err(RT_UNAVAILABLE)
}

#[cfg(any(feature = "space", feature = "texture-loader"))]
pub fn d3d11_device() -> anyhow::Result<windows::Win32::Graphics::Direct3D11::ID3D11Device> {
    #[cfg(feature = "extension-nexus")]
    if let Ok(Some(device)) = exports::nexus::d3d11_device() {
        return Ok(device)
    }

    let res = match dxgi_swap_chain() {
        Ok(Some(swap_chain)) => unsafe {
            let device = swap_chain.GetDevice()?;
            Ok(Some(device))
        },
        Ok(None) => Ok(None),
        Err(msg) => Err(msg),
    }.transpose().unwrap_or(Err(RT_UNAVAILABLE));

    res.map_err(|msg| anyhow::anyhow!("d3d11 device unavailable: {msg}"))
}

pub async fn texture_schedule_path(key: &str, path: &Path) -> RuntimeResult<()> {
    let res = RT_UNAVAILABLE;

    #[cfg(feature = "texture-loader")]
    let res = if TEXTURES.is_available() {
        match TEXTURES.request_load_file(key, path).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                let msg = "Texture load failure";
                ::log::error!("{msg}: {e}");
                msg
            },
        }
    } else { res };

    #[cfg(feature = "extension-nexus")]
    if let Some(res) = exports::nexus::texture_schedule_path(key, path)? {
        return Ok(res)
    }

    Err(res)
}

pub async fn texture_schedule_bytes(key: &str, bytes: Vec<u8>) -> RuntimeResult<()> {
    let res = RT_UNAVAILABLE;

    #[cfg(feature = "texture-loader")]
    let res = if TEXTURES.is_available() {
        let bytes = match bytes {
            #[cfg(feature = "extension-nexus")]
            ref b => &b[..],
            #[cfg(not(feature = "extension-nexus"))]
            b => b,
        };
        match TEXTURES.request_load_bytes(key, bytes).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                let msg = "Texture load failure";
                ::log::error!("{msg}: {e}");
                msg
            },
        }
    } else { res };

    #[cfg(feature = "extension-nexus")]
    if let Some(res) = exports::nexus::texture_schedule_bytes(key, &bytes)? {
        return Ok(res)
    }

    Err(res)
}

pub fn window_handle() -> RuntimeResult<HWND> {
    let sc = dxgi_swap_chain()?
        .ok_or("swap chain unavailable")?;

    let desc = unsafe {
        sc.GetDesc()
    }.map_err(|_| "swap chain descriptor missing")?;

    match desc.OutputWindow.is_invalid() {
        false => Ok(desc.OutputWindow),
        true => Err("no window handle associated with swap chain"),
    }
}

pub fn screen_mouse_position() -> RuntimeResult<MousePosition> {
    mouse::screen_position()
}

#[cfg(todo)]
pub fn window_mouse_position() -> RuntimeResult<MousePosition> {
    // TODO: maybe from imgui? or wndproc?
    mouse::screen_position()
        .and_then(|pos| pos.to_window())
}

pub unsafe fn window_message(msg: u32, w: usize, l: isize) -> RuntimeResult<()> {
    let hwnd = window_handle()?;

    if let Err(e) = WindowsAndMessaging::PostMessageA(Some(hwnd), msg, WPARAM(w), LPARAM(l)) {
        ::log::warn!("failed to send message to {hwnd:?}: {e}");
        return Err("PostMessageA failed")
    }

    Ok(())
}

pub fn window_send_inputs<I: Into<KeyboardAndMouse::INPUT>>(inputs: impl IntoIterator<Item = I>) -> RuntimeResult<()> {
    let inputs: Vec<_> = inputs.into_iter().map(I::into).collect();
    let res = unsafe {
        KeyboardAndMouse::SendInput(&inputs[..], mem::size_of::<KeyboardAndMouse::INPUT>() as _)
    };
    match res {
        0 => {
            let msg = "SendInput Failed";
            ::log::error!("{msg}: {}", windows::core::Error::from_win32());
            Err(msg)
        },
        _ => Ok(()),
    }
}
