use {
    anyhow::anyhow,
    arcdps::Language,
    std::{
        ffi::CStr,
        fmt,
        path::{Path, PathBuf},
        ptr::{self, NonNull},
        sync::atomic::{AtomicBool, Ordering},
    },
    nexus::{
        data_link::{get_mumble_link, get_nexus_link, mumble::MumblePtr, NexusLink},
        gamebind::invoke_gamebind_async,
        localization::translate,
        paths,
        rtapi::RealTimeApi,
        texture::{load_texture_from_file, load_texture_from_memory, Texture, RawTextureReceiveCallback},
        AddonApi,
    },
    crate::{
        exports::{self, runtime::{self as rt, RuntimeResult}},
        game_language_id as lang_id,
        marker::format::MarkerType,
        unload,
        TEXTURES,
    },
};
#[cfg(feature = "space")]
use windows::Win32::Graphics::Dxgi::IDXGISwapChain;

/// raidcore addon id or NEGATIVE random unique signature
pub const SIG: i32 = -exports::SIG;

static RUNTIME_AVAILABLE: AtomicBool = AtomicBool::new(false);

pub(crate) fn pre_init() {
    RUNTIME_AVAILABLE.store(true, Ordering::Relaxed);
}

pub(crate) fn cb_load() {
    let _ = rt::log::TaimiLog::setup();

    pre_init();

    #[cfg(feature = "extension-arcdps")]
    if exports::arcdps::available() {
        log::info!("switching over from arcdps to nexus...");
        exports::arcdps::disable();
    }

    crate::init().expect("load failed");
    crate::load_nexus()
}

pub(crate) fn cb_unload() {
    #[cfg(feature = "extension-arcdps")]
    let own_handle = match exports::arcdps::loaded() {
        true => match exports::arcdps::unload_self() {
            Err(e) => {
                log::warn!("failed to request unload from arcdps: {e}");
                None
            },
            Ok(Some(handle)) if !handle.is_invalid() => {
                log::info!("scheduling DLL exit after unload...");
                Some(handle)
            },
            _ => None,
        },
        false => None,
    };

    if available() {
        unload();
    }

    #[cfg(feature = "extension-arcdps")]
    if let Some(handle) = own_handle {
        use std::{thread, time::Duration};
        use windows::Win32::{
            Foundation::HMODULE,
            System::LibraryLoader::FreeLibraryAndExitThread,
        };

        let handle = handle.0 as usize;
        let _ = thread::spawn(move || -> ! {
            thread::sleep(Duration::from_millis(1200));
            log::info!("goodbye");
            unsafe {
                FreeLibraryAndExitThread(HMODULE(handle as *mut _), 0)
            };
        });
    }

    RUNTIME_AVAILABLE.store(false, Ordering::SeqCst);
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

pub fn log_write_record<W: fmt::Write>(w: &mut W, record: &log::Record) -> fmt::Result {
    let target_written = rt::log::write_metadata_prefix(w, record.metadata(), true)?;
    rt::log::write_record_prefix(w, record, target_written)?;
    rt::log::write_record_body(w, record)?;

    Ok(())
}

pub fn log_record_buffer(buffer: &mut rt::log::LogBuffer, record: &log::Record) -> RuntimeResult<Option<()>> {
    if !available() {
        return Ok(None)
    }

    log_write_record(buffer, record)
        .map_err(|_| rt::log::RT_FORMAT_ERROR)?;

    log(record.metadata(), buffer.terminate())
}

pub fn log(metadata: &log::Metadata, message: &CStr) -> RuntimeResult<Option<()>> {
    if !available() {
        return Ok(None)
    }

    let level = rt::log::nexus_log_level(metadata.level());
    let channel = rt::NAME_C.as_ptr();

    unsafe {
        (AddonApi::get().log)(level, channel, message.as_ptr());
    }

    Ok(Some(()))
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
        &*(ptr::addr_of!(api.swap_chain) as *const Option<IDXGISwapChain>)
    };
    if swap_chain.is_none() {
        return Err("DXGI swap chain unavailable")
    }

    Ok(swap_chain.clone())
}

fn nexus_texture_ok(texture: Option<&Texture>) -> anyhow::Result<Texture> {
    use windows::core::IUnknown;

    match texture {
        Some(texture) => {
            let srv = unsafe {
                &*(ptr::addr_of!(texture.resource) as *const Option<IUnknown>)
            };
            match srv.is_some() {
                true => Ok(texture.clone()),
                false => Err(anyhow!("nexus produced an empty SRV")),
            }
        },
        _ => {
            Err(anyhow!("nexus could not load the texture"))
        },
    }
}

static IMGUI_TEXTURE_CALLBACK: RawTextureReceiveCallback = nexus::texture_receive!(|id, texture| {
    TEXTURES.report_load(id, nexus_texture_ok(texture));
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
