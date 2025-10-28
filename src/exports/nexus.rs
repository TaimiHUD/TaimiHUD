use {
    crate::{
        exports::{
            self,
            runtime::{
                self as rt,
                bindings::{TaimiControls, CONTROLS},
                RuntimeResult,
            },
        },
        game_language_id as lang_id,
        marker::format::MarkerType,
        unload,
        with_i18n,
        TEXTURES,
    },
    anyhow::anyhow,
    arcdps::Language,
    nexus::{
        alert,
        data_link::{get_mumble_link_ptr, get_nexus_link, mumble::MumbleLink, NexusLink},
        gamebind,
        localization::translate,
        paths,
        rtapi::RealTimeApi,
        texture::{
            load_texture_from_file,
            load_texture_from_memory,
            RawTextureReceiveCallback,
            Texture,
        },
        AddonApi,
    },
    std::{
        collections::BTreeMap,
        ffi::{c_char, CStr, CString},
        path::{Path, PathBuf},
        ptr::{self, NonNull},
        sync::{
            atomic::{AtomicBool, Ordering},
            RwLock,
        },
        time::Duration,
    },
    windows::Win32::Foundation::{HWND, LPARAM, WPARAM},
};

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
    let translated = translate(index_to_check).ok_or("Couldn't translate string")?;
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
pub async fn press_marker_bind(
    marker: MarkerType,
    target: bool,
    down: bool,
    position: Option<rt::MousePosition>,
) -> RuntimeResult<Option<()>> {
    use crate::settings::{InvokeMethod, Settings};

    if !available() {
        return Ok(None)
    }

    let method = Settings::async_read()
        .await
        .ok()
        .and_then(|s| s.arc().gamebind_invoke)
        .unwrap_or(InvokeMethod::Nexus);
    if method != InvokeMethod::Nexus {
        return rt::keyboard::press_marker_bind(marker, target, down, position).await
    }

    if let Some(position) = position {
        match rt::mouse::send_input(position) {
            Ok(()) =>
            // wait for nexus to get the event, ugh
            {
                tokio::time::sleep(MOUSE_MOVE_DELAY).await
            },
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

pub fn send_alert(_ui: &rt::imgui::Ui, message: &str) -> RuntimeResult<Option<()>> {
    if !available() {
        return Ok(None)
    }

    alert::send_alert(message);
    Ok(Some(()))
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

pub fn perform_update(release: &rt::update::ResolvedVersion) -> RuntimeResult<Option<()>> {
    if !available() {
        return Ok(None)
    }

    let dll_url = release.dll_url().map_err(|_| "DLL URL missing")?;
    nexus::updater::request_update(SIG, dll_url.as_str());

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

    let swap_chain = unsafe { &*(ptr::addr_of!(api.swap_chain) as *const Option<IDXGISwapChain>) };
    if swap_chain.is_none() {
        return Err("DXGI swap chain unavailable")
    }

    Ok(swap_chain.clone().map(Into::into))
}

pub extern "C-unwind" fn wnd(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> u32 {
    match rt::handle_wnd_event(hwnd, msg, w.0, l.0) {
        m if m == msg => msg,
        _ => 0,
    }
}

fn nexus_texture_ok(texture: Option<&Texture>) -> anyhow::Result<Texture> {
    use windows::core::IUnknown;

    match texture {
        Some(texture) => {
            let srv = unsafe { &*(ptr::addr_of!(texture.resource) as *const Option<IUnknown>) };
            match srv.is_some() {
                true => Ok(texture.clone()),
                false => Err(anyhow!("nexus produced an empty SRV")),
            }
        },
        _ => Err(anyhow!("nexus could not load the texture")),
    }
}

static IMGUI_TEXTURE_CALLBACK: RawTextureReceiveCallback =
    nexus::texture_receive!(|id, texture| {
        TEXTURES.report_load(id, nexus_texture_ok(texture));
    });

pub fn texture_schedule_path(key: &str, path: &Path) -> RuntimeResult<Option<()>> {
    if !available() {
        return Ok(None)
    }

    Ok(Some(load_texture_from_file(
        key,
        path,
        Some(IMGUI_TEXTURE_CALLBACK),
    )))
}

pub fn texture_schedule_bytes(key: &str, data: &[u8]) -> RuntimeResult<Option<()>> {
    if !available() {
        return Ok(None)
    }

    Ok(Some(load_texture_from_memory(
        key,
        data,
        Some(IMGUI_TEXTURE_CALLBACK),
    )))
}

static KEYBIND_IDS: RwLock<BTreeMap<CString, TaimiControls>> = RwLock::new(BTreeMap::new());
extern "C-unwind" fn unsafe_keybind_cb(identifier: *const c_char, is_release: bool) {
    let id = unsafe { CStr::from_ptr(identifier as *const _) };
    let kb = KEYBIND_IDS.read().ok().and_then(|kb| kb.get(id).copied());
    match kb {
        Some(control) => match is_release {
            true => CONTROLS.notify_release(control.to_vk_dummy()),
            false => CONTROLS.notify_press(control.to_vk_dummy(), control),
        },
        None => {
            log::warn!("unexpected nexus event(release={is_release:?}) for keybind {id:?}");
        },
    }
}

pub fn register_keybind<I: Into<CString>>(control: TaimiControls, id: I, default_keybind: &CStr) {
    use {crate::fl, i18n_embed::LanguageLoader, nexus::localization::set_translation};

    let id = id.into();
    if let Ok(id) = id.to_str() {
        let language = crate::LANGUAGE_LOADER.current_language().language;
        if let Some(timer_trigger) = id.strip_prefix("timer-key-trigger-") {
            // ew special cased...
            set_translation(
                id,
                language.as_str(),
                fl!("timer-key-trigger", id = timer_trigger),
            )
        } else {
            with_i18n(id, |msg| set_translation(id, language.as_str(), msg));
        }
    }
    let id = if let Ok(mut keybinds) = KEYBIND_IDS.write() {
        // UNSAFE: but probably fine so I won't get into the nuance :3
        let ptr = id.as_ptr();
        keybinds.insert(id, control);
        ptr
    } else {
        log::error!("keybind map poisoned?");
        return
    };
    unsafe {
        (AddonApi::get().input_binds.register_with_string)(
            id,
            unsafe_keybind_cb,
            default_keybind.as_ptr(),
        );
    }
}

pub fn unregister_keybinds() {
    let Ok(mut keybinds) = KEYBIND_IDS.write() else {
        log::error!("keybind map poisoned?");
        return
    };
    for kb in keybinds.keys() {
        unsafe { (AddonApi::get().input_binds.deregister)(kb.as_ptr()) }
    }
    keybinds.clear();
}

pub fn quick_access_add(icon: TaimiControls) {
    use nexus::quick_access::{add_quick_access, add_quick_access_context_menu};

    let Some((identifier, (neutral, neutral_png), (hover, hover_png), keybind)) =
        quick_access_button_id(icon)
    else {
        return
    };

    load_texture_from_memory(neutral, neutral_png, None);
    load_texture_from_memory(hover, hover_png, None);
    let tooltip_id = match icon {
        TaimiControls::WINDOW_PRIMARY => "primary-window-toggle-text",
        _ => keybind,
    };
    with_i18n(tooltip_id, |tooltip_text| {
        add_quick_access(identifier, neutral, hover, keybind, tooltip_text).leak();
    });

    if let TaimiControls::WINDOW_PRIMARY = icon {
        use crate::{control_window, fl};

        add_quick_access_context_menu(
            "TAIMI_MENU",
            Some(identifier), // maybe some day
            //None::<&str>,
            nexus::render!(|ui| {
                #[cfg(feature = "timers")]
                if ui.button(fl!("timer-window")) {
                    control_window(crate::WINDOW_TIMERS, None);
                }
                #[cfg(feature = "space")]
                {
                    use {crate::controller::pathing::PathingEvent, taimi_meta::ui::MapContext};
                    if ui.button(fl!("pathing-render-toggle")) {
                        PathingEvent::VISIBLE_TOGGLE_SPACE.try_send();
                    }
                    if ui.button(fl!("pathing-render-minimap-toggle")) {
                        PathingEvent::visible_toggle(MapContext::Minimap).try_send();
                    }
                    if ui.button(fl!("pathing-render-map-toggle")) {
                        PathingEvent::visible_toggle(MapContext::Global).try_send();
                    }
                    if ui.button(fl!("pathing-window")) {
                        control_window(crate::WINDOW_PATHING, None);
                    }
                }
                #[cfg(feature = "markers")]
                if ui.button(fl!("marker-window")) {
                    control_window(crate::WINDOW_MARKERS, None);
                }
                if ui.button(fl!("primary-window")) {
                    control_window(crate::WINDOW_PRIMARY, None);
                }
            }),
        )
        .leak();
    }
}

pub fn quick_access_remove_all() {
    // TODO: filter by visible icons or don't bother?
    let icons = TaimiControls::QUICK_ACCESS_ICONS;
    for icon in icons {
        quick_access_remove(icon);
    }
}

pub fn quick_access_remove(icon: TaimiControls) {
    use nexus::quick_access::{remove_quick_access, remove_quick_access_context_menu};

    let Some((identifier, ..)) = quick_access_button_id(icon) else {
        return
    };
    if let TaimiControls::WINDOW_PRIMARY = icon {
        remove_quick_access_context_menu("TAIMI_MENU");
    }
    remove_quick_access(identifier);
}

/// ("BUTTON", "ICON", "HOVER", "keybind")
pub(crate) fn quick_access_button_id(
    icon: TaimiControls,
) -> Option<(
    &'static str,
    (&'static str, &'static [u8]),
    (&'static str, &'static [u8]),
    &'static str,
)> {
    Some(match icon {
        TaimiControls::WINDOW_PRIMARY => (
            "TAIMI_BUTTON",
            ("TAIMI_ICON", include_bytes!("../../icons/taimi.png")),
            (
                "TAIMI_ICON_HOVER",
                include_bytes!("../../icons/taimi-hover.png"),
            ),
            "primary-window-toggle",
        ),
        #[cfg(feature = "markers")]
        TaimiControls::WINDOW_MARKERS => (
            "TAIMI_MARKERS_BUTTON",
            (
                "TAIMI_MARKERS_ICON",
                include_bytes!("../../icons/markers.png"),
            ),
            (
                "TAIMI_MARKERS_ICON_HOVER",
                include_bytes!("../../icons/markers-hover.png"),
            ),
            "marker-window-toggle",
        ),
        #[cfg(feature = "timers")]
        TaimiControls::WINDOW_TIMERS => (
            "TAIMI_TIMER_BUTTON",
            (
                "TAIMI_TIMERS_ICON",
                include_bytes!("../../icons/timers.png"),
            ),
            (
                "TAIMI_TIMERS_ICON_HOVER",
                include_bytes!("../../icons/timers-hover.png"),
            ),
            "timer-window-toggle",
        ),
        #[cfg(feature = "space")]
        TaimiControls::WINDOW_PATHING => (
            "TAIMI_PATHING_BUTTON",
            (
                "TAIMI_PATHING_ICON",
                include_bytes!("../../icons/pathing.png"),
            ),
            (
                "TAIMI_PATHING_ICON_HOVER",
                include_bytes!("../../icons/pathing-hover.png"),
            ),
            "pathing-window-toggle",
        ),
        #[cfg(feature = "space")]
        TaimiControls::PATHING_SPACE
        | TaimiControls::PATHING_MINIMAP
        | TaimiControls::PATHING_MAP => (
            match icon {
                TaimiControls::PATHING_MINIMAP => "TAIMI_PATHING_RENDER_MINIMAP_BUTTON",
                TaimiControls::PATHING_MAP => "TAIMI_PATHING_RENDER_MAP_BUTTON",
                _ => "TAIMI_PATHING_RENDER_BUTTON",
            },
            (
                "TAIMI_PATHING_RENDER_ICON",
                include_bytes!("../../icons/pathing-toggle.png"),
            ),
            (
                "TAIMI_PATHING_RENDER_ICON_HOVER",
                include_bytes!("../../icons/pathing-toggle-hover.png"),
            ),
            match icon {
                TaimiControls::PATHING_MINIMAP => "pathing-render-minimap-toggle",
                TaimiControls::PATHING_MAP => "pathing-render-map-toggle",
                _ => "pathing-render-toggle",
            },
        ),
        icon => {
            log::warn!("unrecognized quick access icon {icon:?}");
            return None
        },
    })
}
