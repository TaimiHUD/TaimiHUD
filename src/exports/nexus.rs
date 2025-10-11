use {
    anyhow::anyhow,
    arcdps::Language,
    std::{
        ffi::CStr,
        path::{Path, PathBuf},
        ptr::{self, NonNull},
        sync::atomic::{AtomicBool, Ordering},
        time::Duration,
    },
    nexus::{
        data_link::{get_mumble_link_ptr, get_nexus_link, mumble::MumbleLink, NexusLink},
        gamebind,
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
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};

/// raidcore addon id or NEGATIVE random unique signature
pub const SIG: i32 = -exports::SIG;

static RUNTIME_AVAILABLE: AtomicBool = AtomicBool::new(false);

pub(crate) fn pre_init() {
    RUNTIME_AVAILABLE.store(true, Ordering::Relaxed);
    crate::crate_init();
}

pub(crate) fn cb_load() {
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
    let own_handle = match exports::arcdps::ExitHandle::try_exit() {
        Err(e) => {
            log::warn!("failed to request unload from arcdps: {e}");
            None
        },
        Ok(exit) => {
            if exit.is_some() {
                log::info!("scheduling DLL exit after unload...");
            }
            exit
        },
    };

    if available() {
        unload();
    }

    #[cfg(feature = "extension-arcdps")]
    if let Some(handle) = own_handle {
        handle.spawn_free();
    } else {
        rt::log::TaimiLog::logger().close();
    }

    RUNTIME_AVAILABLE.store(false, Ordering::SeqCst);

    #[cfg(not(feature = "extension-arcdps"))]
    rt::log::TaimiLog::logger().close();
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

pub fn mumble_link_ptr() -> RuntimeResult<Option<NonNull<MumbleLink>>> {
    if !available() {
        return Ok(None)
    }

    let ml = get_mumble_link_ptr();
    NonNull::new(ml as *mut _)
        .ok_or("MumbleLink unavailable")
        .map(Some)
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

const MOUSE_MOVE_DELAY: Duration = Duration::from_millis(60); // 50 too low?
pub async fn press_marker_bind(marker: MarkerType, target: bool, down: bool, position: Option<rt::MousePosition>) -> RuntimeResult<Option<()>> {
    if !available() {
        return Ok(None)
    }

    if let Some(position) = position {
        match rt::mouse::send_input(position) {
            Ok(()) =>
                // wait for nexus to get the event, ugh
                tokio::time::sleep(MOUSE_MOVE_DELAY).await,
            Err(e) => {
                log::error!("Failed to adjust mouse position for marker placement: {e}");
                return Err("Marker mouse move failed")
            },
        }
    }

    let bind = match target {
        true => marker.to_set_agent_gamebind(),
        false => marker.to_place_world_gamebind(),
    };
    Ok(Some(match down {
        true => gamebind::press_gamebind(bind),
        false => gamebind::release_gamebind(bind),
    }))
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

/// just a utility that calls dxgi_swap_chain().GetDevice()...
#[cfg(todo = "unnecessary")]
#[cfg(any(feature = "space", feature = "texture-loader"))]
pub fn d3d11_device() -> RuntimeResult<Option<rt::Device>> {
    if !available() {
        return Ok(None)
    }

    let api = AddonApi::get();
    Ok(api.get_d3d11_device().map(Into::into))
}

#[cfg(any(feature = "space", feature = "texture-loader"))]
pub fn dxgi_swap_chain() -> RuntimeResult<Option<rt::SwapChain>> {
    use windows::Win32::Graphics::Dxgi::IDXGISwapChain;

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

    Ok(swap_chain.clone().map(Into::into))
}

pub extern "C-unwind" fn wnd(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> u32 {
    match rt::handle_wnd_event(hwnd, msg, w.0, l.0) {
        m if m == msg =>
            msg,
        _ => 0,
    }
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
