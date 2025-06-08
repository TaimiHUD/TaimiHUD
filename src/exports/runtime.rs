use std::{path::{Path, PathBuf}, ptr::NonNull};
use nexus::{data_link::{mumble::MumblePtr, NexusLink}, rtapi::RealTimeApi};
use crate::{exports, load_language, marker::format::MarkerType};

pub type RuntimeError = &'static str;
pub type RuntimeResult<T = ()> = Result<T, RuntimeError>;
pub const RT_UNAVAILABLE: RuntimeError = "extension runtime unavailable";

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
    log::info!("Detected language {language} for internationalization");

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

#[cfg(feature = "space")]
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
pub fn d3d11_device() -> RuntimeResult<Option<windows::Win32::Graphics::Direct3D11::ID3D11Device>> {
    #[cfg(feature = "extension-nexus")]
    if let Some(device) = exports::nexus::d3d11_device()? {
        return Ok(Some(device))
    }

    #[cfg(feature = "extension-arcdps")]
    if let Some(device) = exports::arcdps::d3d11_device()? {
        return Ok(Some(device))
    }

    Err(RT_UNAVAILABLE)
}

pub fn texture_schedule_path(key: &str, path: &Path) -> RuntimeResult<()> {
    #[cfg(feature = "extension-nexus")]
    if let Some(res) = exports::nexus::texture_schedule_path(key, path)? {
        return Ok(res)
    }

    #[cfg(feature = "extension-arcdps")]
    if let Some(res) = exports::arcdps::texture_schedule_path(key, path)? {
        return Ok(res)
    }

    Err(RT_UNAVAILABLE)
}

pub fn texture_schedule_bytes(key: &str, bytes: Vec<u8>) -> RuntimeResult<()> {
    #[cfg(feature = "extension-nexus")]
    if let Some(res) = exports::nexus::texture_schedule_bytes(key, &bytes)? {
        return Ok(res)
    }

    #[cfg(feature = "extension-arcdps")]
    if let Some(res) = exports::arcdps::texture_schedule_bytes(key, bytes)? {
        return Ok(res)
    }

    Err(RT_UNAVAILABLE)
}
