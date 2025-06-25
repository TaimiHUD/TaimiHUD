use std::{ffi::CStr, path::{Path, PathBuf}, ptr::NonNull, sync::Mutex};
use ::log::info;
use nexus::{data_link::{mumble::MumblePtr, NexusLink}, rtapi::RealTimeApi};
use crate::{exports, load_language, marker::format::MarkerType};

pub mod log;
pub mod textures;
pub use self::textures::TextureLoader;

pub use nexus::imgui;

pub type RuntimeError = &'static str;
pub type RuntimeResult<T = ()> = Result<T, RuntimeError>;
pub const RT_UNAVAILABLE: RuntimeError = "extension runtime unavailable";

pub const CRATE_NAME: &'static str = env!("CARGO_PKG_NAME");
pub const CRATE_VERSION: &'static str = env!("CARGO_PKG_VERSION");
#[cfg(todo)]
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
            core::mem::transmute(ml)
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

pub fn invoke_marker_bind(marker: MarkerType, target: bool, duration_ms: i32) -> RuntimeResult<()> {
    #[cfg(feature = "extension-nexus")]
    if let Some(res) = exports::nexus::invoke_marker_bind(marker, target, duration_ms)? {
        return Ok(res)
    }

    #[cfg(feature = "extension-arcdps")]
    if let Some(res) = exports::arcdps::invoke_marker_bind(marker, target, duration_ms)? {
        return Ok(res)
    }

    Err(RT_UNAVAILABLE)
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

pub fn texture_schedule_path(key: &str, path: &Path) -> RuntimeResult<()> {
    #[cfg(feature = "extension-nexus")]
    if let Some(res) = exports::nexus::texture_schedule_path(key, path)? {
        return Ok(res)
    }

    Err(RT_UNAVAILABLE)
}

pub fn texture_schedule_bytes(key: &str, bytes: Vec<u8>) -> RuntimeResult<()> {
    #[cfg(feature = "extension-nexus")]
    if let Some(res) = exports::nexus::texture_schedule_bytes(key, &bytes)? {
        return Ok(res)
    }

    Err(RT_UNAVAILABLE)
}
