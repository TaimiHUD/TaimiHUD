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
        settings::IconStyle,
        unload,
        with_i18n,
        TEXTURES,
    },
    anyhow::anyhow,
    arcdps::Language,
    nexus::{
        addon::{AddonFlags, UpdateProvider},
        alert,
        data_link::{get_mumble_link_ptr, get_nexus_link, mumble::MumbleLink, NexusLink},
        gamebind,
        localization::translate,
        paths,
        rtapi::RealTimeApi,
        texture::{load_texture_from_file, load_texture_from_memory, RawTextureReceiveCallback, Texture},
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

#[cfg(feature = "extension-nexus-codegen")]
pub(crate) mod cb;
#[cfg(feature = "extension-nexus-extern")]
pub(crate) mod r#extern;

/// raidcore addon id or NEGATIVE random unique signature
pub const SIG: i32 = -exports::SIG;
#[allow(unreachable_patterns)]
pub const UPDATE_PROVIDER: UpdateProvider = match () {
    #[cfg(taimi_update = "github")]
    () => UpdateProvider::GitHub,
    #[cfg(taimi_update = "direct")]
    () => UpdateProvider::Direct,
    #[cfg(taimi_update = "manual")]
    () => UpdateProvider::Manual,
    _ => UpdateProvider::None,
};
pub const FLAGS: AddonFlags = AddonFlags::None;

static RUNTIME_AVAILABLE: AtomicBool = AtomicBool::new(false);
static RUNTIME_LOADED: AtomicBool = AtomicBool::new(false);

pub(crate) fn pre_init() {
    RUNTIME_LOADED.store(true, Ordering::Relaxed);
    crate::crate_init();
}

pub(self) fn init() {
    RUNTIME_AVAILABLE.store(true, Ordering::Relaxed);
    #[cfg(feature = "extension-arcdps")]
    if exports::arcdps::available() {
        log::info!("switching over from arcdps to nexus...");
        exports::arcdps::disable();
    }

    crate::init().expect("load failed");
    crate::load_nexus()
}

pub(self) fn uninit() {
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
    RUNTIME_LOADED.store(false, Ordering::SeqCst);

    #[cfg(not(feature = "extension-arcdps"))]
    rt::log::TaimiLog::logger().close();
}

pub fn loaded() -> bool {
    RUNTIME_LOADED.load(Ordering::SeqCst)
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

pub fn send_alert(_ui: &rt::imgui::Ui, message: &str) -> RuntimeResult<Option<()>> {
    if !available() {
        return Ok(None)
    }

    alert::send_alert(message);
    Ok(Some(()))
}

#[allow(unreachable_patterns)]
pub fn imgui_context_ptr() -> Option<NonNull<rt::imgui::sys::ImGuiContext>> {
    match () {
        #[cfg(feature = "extension-nexus-extern")]
        _ => r#extern::imgui_context_ptr(),
        #[cfg(feature = "extension-nexus-codegen")]
        _ => NonNull::new(cb::addon_api().imgui_ctx),
    }
}
pub unsafe fn with_ui<R, F: FnOnce(&rt::imgui::Ui) -> R>(f: F) -> R {
    #[cfg(feature = "extension-nexus-extern")]
    let ui = {
        r#extern::new_imgui_frame();
        r#extern::imgui_ui()
    };
    #[cfg(feature = "extension-nexus-codegen")]
    let ui = {
        cb::new_imgui_frame();
        nexus::ui()
    };
    f(ui)
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

pub async fn perform_update(release: &rt::update::ResolvedVersion) -> RuntimeResult<Option<()>> {
    if !available() {
        return Ok(None)
    }

    let dll_url = release.dll_url(false).await.map_err(|e| {
        log::error!("{e:#}");
        "DLL URL missing"
    })?;
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

static IMGUI_TEXTURE_CALLBACK: RawTextureReceiveCallback = nexus::texture_receive!(|id, texture| {
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
        (AddonApi::get().input_binds.register_with_string)(id, unsafe_keybind_cb, default_keybind.as_ptr());
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

pub fn quick_access_add(icon: TaimiControls, state_on: TaimiControls, style: IconStyle) {
    use {
        crate::render::RenderState,
        nexus::quick_access::{add_quick_access, add_quick_access_context_menu},
    };

    let Some(identifier) = IconStyle::control_id(icon) else {
        log::warn!("no button for icon {:#010x}", icon.bits());
        return
    };
    let Some(keybind) = IconStyle::keybind_id(icon) else {
        log::error!("no keybind for icon {:#010x}", icon.bits());
        return
    };
    let tooltip_id = IconStyle::tooltip_id(icon).unwrap_or(keybind);
    let button_id = IconStyle::control_button_id(identifier);

    let on_off = match state_on.intersects(icon) {
        state if style.icon_has_state(icon) => state,
        _ => IconStyle::NEUTRAL_ON_OFF,
    };

    let texture_neutral = style
        .texture_id(icon, on_off, false)
        .or_else(|| IconStyle::default().texture_id(icon, on_off, false));
    let Some(texture_neutral) = texture_neutral else {
        log::error!("no texture for icon {:#010x}", icon.bits());
        return
    };
    if let Some(data) = style.data_for(icon, on_off, false) {
        load_texture_from_memory(&texture_neutral, data, None);
    }
    let texture_hover = style.texture_id(icon, on_off, true);
    let texture_hover = match &texture_hover {
        Some(id) if style.icon_has_hover(icon) => match style.data_for(icon, on_off, true) {
            Some(data) => {
                load_texture_from_memory(id, data, None);
                Some(&id[..])
            },
            _ => None,
        },
        _ => None,
    }
    .unwrap_or(&texture_neutral[..]);
    with_i18n(tooltip_id, |tooltip_text| {
        add_quick_access(&button_id, &texture_neutral, texture_hover, keybind, tooltip_text).leak();
    });

    if IconStyle::control_has_menu(icon) {
        let menu_id = IconStyle::control_menu_id(identifier);
        let callback = match icon {
            #[cfg(feature = "timers")]
            TaimiControls::WINDOW_TIMERS =>
                nexus::render!(|ui| RenderState::render_context_popup(ui, TaimiControls::WINDOW_TIMERS)),
            #[cfg(feature = "markers")]
            TaimiControls::WINDOW_MARKERS =>
                nexus::render!(|ui| RenderState::render_context_popup(ui, TaimiControls::WINDOW_MARKERS)),
            #[cfg(feature = "space")]
            TaimiControls::WINDOW_PATHING
            | TaimiControls::PATHING_SPACE
            | TaimiControls::PATHING_MINIMAP
            | TaimiControls::PATHING_MAP =>
                nexus::render!(|ui| RenderState::render_context_popup(ui, TaimiControls::WINDOW_PATHING)),
            _ => nexus::render!(|ui| RenderState::render_context_popup(ui, TaimiControls::WINDOW_PRIMARY)),
        };
        add_quick_access_context_menu(menu_id, Some(button_id), callback).leak()
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

    let Some(identifier) = IconStyle::control_id(icon) else { return };
    if IconStyle::control_has_menu(icon) {
        let menu_id = IconStyle::control_menu_id(identifier);
        remove_quick_access_context_menu(menu_id);
    }
    let button_id = IconStyle::control_button_id(identifier);
    remove_quick_access(button_id);
}

pub fn quick_access_init(icons: TaimiControls, style: IconStyle, state_on: TaimiControls) {
    let quick_access_icons_visible = TaimiControls::QUICK_ACCESS_ICONS
        .into_iter()
        .filter(|&icon| icons.intersects(icon));
    for icon in quick_access_icons_visible {
        quick_access_add(icon, state_on, style);
    }
}

impl IconStyle {
    pub fn data_for(self, icon: TaimiControls, on_off: bool, hover: bool) -> Option<&'static [u8]> {
        Some(match (self, Self::canon_icon(icon), hover, on_off) {
            (Self::Plain, TaimiControls::WINDOW_PRIMARY, false, _) =>
                include_bytes!("../../../data/icons/plain/taimi.png"),
            (Self::Scanlines1, TaimiControls::WINDOW_PRIMARY, false, _) =>
                include_bytes!("../../../data/icons/scanlines-1/taimi.png"),
            (Self::Plain, TaimiControls::WINDOW_PRIMARY, true, _) =>
                include_bytes!("../../../data/icons/plain/taimi-hover.png"),
            (Self::Scanlines1, TaimiControls::WINDOW_PRIMARY, true, _) =>
                include_bytes!("../../../data/icons/scanlines-1/taimi-hover.png"),

            #[cfg(feature = "markers")]
            (Self::Plain, TaimiControls::WINDOW_MARKERS, false, _) =>
                include_bytes!("../../../data/icons/plain/markers.png"),
            #[cfg(feature = "markers")]
            (Self::Scanlines1, TaimiControls::WINDOW_MARKERS, false, _) =>
                include_bytes!("../../../data/icons/scanlines-1/markers.png"),
            #[cfg(feature = "markers")]
            (Self::Plain, TaimiControls::WINDOW_MARKERS, true, _) =>
                include_bytes!("../../../data/icons/plain/markers-hover.png"),
            #[cfg(feature = "markers")]
            (Self::Scanlines1, TaimiControls::WINDOW_MARKERS, true, _) =>
                include_bytes!("../../../data/icons/scanlines-1/markers-hover.png"),

            #[cfg(feature = "timers")]
            (Self::Plain, TaimiControls::WINDOW_TIMERS, false, _) =>
                include_bytes!("../../../data/icons/plain/timers.png"),
            #[cfg(feature = "timers")]
            (Self::Scanlines1, TaimiControls::WINDOW_TIMERS, false, _) =>
                include_bytes!("../../../data/icons/scanlines-1/timers.png"),
            #[cfg(feature = "timers")]
            (Self::Plain, TaimiControls::WINDOW_TIMERS, true, _) =>
                include_bytes!("../../../data/icons/plain/timers-hover.png"),
            #[cfg(feature = "timers")]
            (Self::Scanlines1, TaimiControls::WINDOW_TIMERS, true, _) =>
                include_bytes!("../../../data/icons/scanlines-1/timers-hover.png"),

            #[cfg(feature = "space")]
            (Self::Plain, TaimiControls::WINDOW_PATHING, false, _) =>
                include_bytes!("../../../data/icons/plain/pathing.png"),
            #[cfg(feature = "space")]
            (Self::Scanlines1, TaimiControls::WINDOW_PATHING, false, _) =>
                include_bytes!("../../../data/icons/scanlines-1/pathing.png"),
            #[cfg(feature = "space")]
            (Self::Plain, TaimiControls::WINDOW_PATHING, true, _) =>
                include_bytes!("../../../data/icons/plain/pathing-hover.png"),
            #[cfg(feature = "space")]
            (Self::Scanlines1, TaimiControls::WINDOW_PATHING, true, _) =>
                include_bytes!("../../../data/icons/scanlines-1/pathing-hover.png"),

            #[cfg(feature = "space")]
            (Self::Plain, TaimiControls::PATHING_SPACE, false, true) =>
                include_bytes!("../../../data/icons/plain/pathingtoggle-on.png"),
            #[cfg(feature = "space")]
            (Self::Scanlines1, TaimiControls::PATHING_SPACE, false, true) =>
                include_bytes!("../../../data/icons/scanlines-1/pathingtoggle-on.png"),
            #[cfg(feature = "space")]
            (Self::Plain, TaimiControls::PATHING_SPACE, true, true) =>
                include_bytes!("../../../data/icons/plain/pathingtoggle-on-hover.png"),
            #[cfg(feature = "space")]
            (Self::Scanlines1, TaimiControls::PATHING_SPACE, true, true) =>
                include_bytes!("../../../data/icons/scanlines-1/pathingtoggle-on-hover.png"),
            #[cfg(feature = "space")]
            (Self::Plain, TaimiControls::PATHING_SPACE, false, false) =>
                include_bytes!("../../../data/icons/plain/pathingtoggle-off.png"),
            #[cfg(feature = "space")]
            (Self::Scanlines1, TaimiControls::PATHING_SPACE, false, false) =>
                include_bytes!("../../../data/icons/scanlines-1/pathingtoggle-off.png"),
            #[cfg(feature = "space")]
            (Self::Plain, TaimiControls::PATHING_SPACE, true, false) =>
                include_bytes!("../../../data/icons/plain/pathingtoggle-off-hover.png"),
            #[cfg(feature = "space")]
            (Self::Scanlines1, TaimiControls::PATHING_SPACE, true, false) =>
                include_bytes!("../../../data/icons/scanlines-1/pathingtoggle-off-hover.png"),
            _ => {
                log::warn!("unrecognized quick access icon {:#010x}", icon.bits());
                return None
            },
        })
    }
    pub fn control_button_id(control_id: &str) -> String {
        format!("{control_id}_BUTTON")
    }
    pub fn control_menu_id(control_id: &str) -> String {
        format!("{control_id}_MENU")
    }
    pub fn texture_id(self, icon: TaimiControls, on_off: bool, hover: bool) -> Option<String> {
        let icon_id = Self::icon_id(icon)?;
        let state = match self.icon_has_state(icon) {
            true if !on_off => "_OFF",
            _ => "",
        };
        let hover = match self.icon_has_hover(icon) {
            true if hover => "_HOVER",
            _ => "",
        };
        let suffix = self.suffix_upper();
        Some(format!("{icon_id}{state}{hover}{suffix}"))
    }
    pub fn control_id(icon: TaimiControls) -> Option<&'static str> {
        Some(match icon {
            TaimiControls::WINDOW_PRIMARY => "TAIMI",
            #[cfg(feature = "markers")]
            TaimiControls::WINDOW_MARKERS => "TAIMI_MARKERS",
            #[cfg(feature = "timers")]
            TaimiControls::WINDOW_TIMERS => "TAIMI_TIMERS",
            #[cfg(feature = "space")]
            TaimiControls::WINDOW_PATHING => "TAIMI_PATHING",
            #[cfg(feature = "space")]
            TaimiControls::PATHING_SPACE => "TAIMI_PATHING_RENDER",
            #[cfg(feature = "space")]
            TaimiControls::PATHING_MINIMAP => "TAIMI_PATHING_RENDER_MINIMAP",
            #[cfg(feature = "space")]
            TaimiControls::PATHING_MAP => "TAIMI_PATHING_RENDER_MAP",
            _ => return None,
        })
    }
    pub fn icon_id(icon: TaimiControls) -> Option<&'static str> {
        Self::control_id(Self::canon_icon(icon))
    }
    pub fn icon_has_state(self, icon: TaimiControls) -> bool {
        match icon {
            TaimiControls::PATHING_SPACE | TaimiControls::PATHING_MAP | TaimiControls::PATHING_MINIMAP =>
                true,
            _ => false,
        }
    }
    #[inline(always)]
    pub fn icon_has_hover(self, _icon: TaimiControls) -> bool {
        true
    }
    #[inline(always)]
    pub fn control_has_menu(_icon: TaimiControls) -> bool {
        true
    }
    pub fn suffix_upper(self) -> &'static str {
        match self {
            Self::Plain => "",
            Self::Scanlines1 => "_SCAN1",
        }
    }
    pub fn canon_icon(icon: TaimiControls) -> TaimiControls {
        match icon {
            icon if icon.intersects(TaimiControls::PATHING_TOGGLES) => TaimiControls::PATHING_SPACE,
            icon => icon,
        }
    }
    pub fn keybind_id(icon: TaimiControls) -> Option<&'static str> {
        Some(match icon {
            TaimiControls::WINDOW_PRIMARY => "primary-window-toggle",
            #[cfg(feature = "markers")]
            TaimiControls::WINDOW_MARKERS => "marker-window-toggle",
            #[cfg(feature = "timers")]
            TaimiControls::WINDOW_TIMERS => "timer-window-toggle",
            #[cfg(feature = "space")]
            TaimiControls::WINDOW_PATHING => "pathing-window-toggle",
            #[cfg(feature = "space")]
            TaimiControls::PATHING_MINIMAP => "pathing-render-minimap-toggle",
            #[cfg(feature = "space")]
            TaimiControls::PATHING_MAP => "pathing-render-map-toggle",
            #[cfg(feature = "space")]
            TaimiControls::PATHING_SPACE => "pathing-render-toggle",
            _ => return None,
        })
    }
    pub fn tooltip_id(icon: TaimiControls) -> Option<&'static str> {
        match icon {
            TaimiControls::WINDOW_PRIMARY => Some("primary-window-toggle-text"),
            _ => None,
        }
    }
}
