use std::{path::{Path, PathBuf}, ptr::NonNull, sync::{atomic::{AtomicBool, Ordering}, Arc}};
use arcdps::Language;
use nexus::{data_link::{get_mumble_link, get_nexus_link, mumble::MumblePtr, NexusLink}, gamebind::invoke_gamebind_async, localization::translate, paths, rtapi::RealTimeApi, texture::{load_texture_from_file, load_texture_from_memory, RawTextureReceiveCallback}};
use crate::{exports::{self, runtime::RuntimeResult}, game_language_id as lang_id, load_nexus, marker::format::MarkerType, unload};
#[cfg(any(feature = "space", feature = "texture-loader"))]
use nexus::AddonApi;
#[cfg(feature = "space")]
use windows::Win32::Graphics::Dxgi::IDXGISwapChain;

/// raidcore addon id or NEGATIVE random unique signature
pub const SIG: i32 = -exports::SIG;

static RUNTIME_AVAILABLE: AtomicBool = AtomicBool::new(false);

pub(crate) fn pre_init() {
    RUNTIME_AVAILABLE.store(true, Ordering::Relaxed);
}

pub(crate) fn cb_load() {
    pre_init();
    load_nexus();
}

pub(crate) fn cb_unload() {
    RUNTIME_AVAILABLE.store(false, Ordering::SeqCst);
    unload();
}

pub fn available() -> bool {
    RUNTIME_AVAILABLE.load(Ordering::SeqCst)
}

pub fn addon_dir() -> RuntimeResult<Option<PathBuf>> {
    if !available() {
        return Ok(None)
    }

    paths::get_addon_dir(exports::ADDON_DIR_NAME)
        .ok_or("Invalid addon dir")
        .map(Some)
}

pub fn detect_language() -> RuntimeResult<Option<String>> {
    if !available() {
        return Ok(None)
    }

    let index_to_check = "KB_CHANGELOG";
    let translated = translate(index_to_check)
        .ok_or("Couldn't translate string")?;
    let language = match &translated[..] {
        "Registro de Alterações" => "pt-br",
        "更新日志" => lang_id(Language::Chinese),
        "Seznam změn" => "cz",
        "Änderungsprotokoll" => lang_id(Language::German),
        "Changelog" => lang_id(Language::English),
        "Notas del parche" => lang_id(Language::Spanish),
        "Journal des modifications" => lang_id(Language::French),
        "Registro modifiche" => "it",
        "Lista zmian" => "pl",
        "Список изменений" => "ru",
        _ => lang_id(Language::English),
    };
    Ok(Some(language.into()))
}

pub fn mumble_link_ptr() -> RuntimeResult<Option<MumblePtr>> {
    if !available() {
        return Ok(None)
    }

    match get_mumble_link() {
        Some(ml) => Ok(Some(ml)),
        None => Err("MumbleLink unavailable"),
    }
}

pub fn rtapi() -> RuntimeResult<Option<RealTimeApi>> {
    if !available() {
        return Ok(None)
    }

    Ok(RealTimeApi::get())
}

pub fn nexus_link_ptr() -> RuntimeResult<Option<NonNull<NexusLink>>> {
    if !available() {
        return Ok(None)
    }

    Ok(NonNull::new(get_nexus_link() as *mut NexusLink))
}

pub fn invoke_marker_bind(marker: MarkerType, target: bool, duration_ms: i32) -> RuntimeResult<Option<()>> {
    if !available() {
        return Ok(None)
    }

    let bind = match target {
        true => marker.to_set_agent_gamebind(),
        false => marker.to_place_world_gamebind(),
    };
    Ok(Some(invoke_gamebind_async(bind, duration_ms)))
}

#[cfg(any(feature = "space", feature = "texture-loader"))]
pub fn d3d11_device() -> RuntimeResult<Option<windows::Win32::Graphics::Direct3D11::ID3D11Device>> {
    if !available() {
        return Ok(None)
    }

    let api = AddonApi::get();
    Ok(api.get_d3d11_device())
}

#[cfg(feature = "space")]
pub fn dxgi_swap_chain() -> RuntimeResult<Option<IDXGISwapChain>> {
    if !available() {
        return Ok(None)
    }

    let api: &'static AddonApi = AddonApi::get();

    let swap_chain = unsafe {
        *(std::ptr::addr_of!(api.swap_chain) as *const *mut IDXGISwapChain)
    };
    if swap_chain.is_null() {
        return Err("DXGI swap chain unavailable")
    }

    Ok(Some(unsafe {
        (*swap_chain).clone()
    }))
}

static IMGUI_TEXTURE_CALLBACK: RawTextureReceiveCallback = nexus::texture_receive!(|id, texture| {
    let texture = match texture {
        Some(t) => {
            log::info!("Texture {id} loaded.");
            t
        },
        None => {
            log::warn!("Texture {id} failed to load");
            return
        },
    };

    let gooey = crate::IMGUI_TEXTURES.get().unwrap();
    let mut gooey_lock = gooey.write().unwrap();
    gooey_lock
        .entry(id.into())
        .or_insert(Arc::new(texture.clone()));
});

pub fn texture_schedule_path(key: &str, path: &Path) -> RuntimeResult<Option<()>> {
    if !available() {
        return Ok(None)
    }

    Ok(Some(load_texture_from_file(key, path, Some(IMGUI_TEXTURE_CALLBACK))))
}

pub fn texture_schedule_bytes(key: &str, data: &[u8]) -> RuntimeResult<Option<()>> {
    if !available() {
        return Ok(None)
    }

    Ok(Some(load_texture_from_memory(key, data, Some(IMGUI_TEXTURE_CALLBACK))))
}
