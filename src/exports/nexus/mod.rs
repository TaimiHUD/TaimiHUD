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
        marker::format::MarkerType,
        render::{
            i18n::{self, new_lang_id, with_i18n, LanguageIdentifier},
            machine::RenderMachine,
            RenderState,
        },
        settings::{state::AddonHostName, IconStyle},
        unload,
        TEXTURES,
    },
    anyhow::anyhow,
    nexus::{
        addon::{AddonFlags, UpdateProvider},
        alert,
        data_link::{get_mumble_link_ptr, get_nexus_link, mumble::MumbleLink, NexusLink},
        gamebind,
        gui::RenderType,
        localization::translate,
        paths,
        rtapi::RealTimeApi,
        texture::{load_texture_from_file, load_texture_from_memory, RawTextureReceiveCallback, Texture},
        AddonApi,
    },
    std::{
        borrow::Cow,
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
pub use self::cb::with_ui;
#[cfg(feature = "extension-nexus-extern")]
pub use self::r#extern::with_ui;

#[cfg(feature = "extension-nexus-codegen")]
pub(crate) mod cb;
#[allow(dead_code)]
#[cfg(feature = "extension-arcdps")]
pub mod datalink;
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
    let res = match crate::pre_init_for(AddonHostName::Nexus) {
        Ok(true) => Ok(()),
        Ok(false) => {
            unsafe {
                load_fallback();
            }
            #[cfg(todo)]
            disable();
            Ok(())
        },
        Err(e) => Err(e),
    };

    let res = res.and_then(|()| crate::init());
    match res {
        #[cfg(feature = "extension-nexus-extern")]
        Ok(()) if r#extern::is_disabled() => {
            log::info!("skipping disabled nexus init");
            unsafe {
                load_fallback();
            }
        },
        Ok(()) => unsafe { load_nexus() },
        Err(e) => {
            #[cfg(not(panic = "unwind"))]
            log::error!("nexus load failed: {e}");
            disable();
        },
    }
    let success = match &res {
        #[cfg(all(todo, panic = "unwind"))]
        Err(..) => {
            // until we figure out a way to consistently trigger unload in nexus,
            // it can never actually "fail" init
            false
        },
        _ => true,
    };
    crate::post_init_for(AddonHostName::Nexus, success);
    #[cfg(panic = "unwind")]
    if let Err(e) = res {
        panic!("{e}")
    }
}

unsafe fn load_nexus() {
    let Some(aapi) = addon_api() else { return };
    #[cfg(feature = "space")]
    (aapi.renderer.register)(RenderType::PreRender, unsafe_render_pre);
    (aapi.renderer.register)(RenderType::Render, unsafe_render);
    (aapi.renderer.register)(RenderType::OptionsRender, unsafe_options);
    (aapi.wnd_proc.register)(wnd);

    // TODO: migrate the rest here too...
    crate::load_nexus();
}
unsafe fn load_fallback() {
    let Some(aapi) = addon_api() else { return };

    if !aapi.imgui_context.is_null() {
        (aapi.renderer.register)(RenderType::OptionsRender, unsafe_options_fallback);
    }
}
extern "C-unwind" fn unsafe_render_pre() {
    unsafe {
        let Some(render_ready) = RenderState::pre_render(AddonHostName::Nexus) else {
            return
        };
        RenderMachine::turn_render_entry();
        if !render_ready {
            RenderState::render_setup();
        }
    }
}
extern "C-unwind" fn unsafe_render() {
    unsafe {
        #[cfg(not(feature = "space"))]
        unsafe_render_pre();
        if RenderState::is_host(AddonHostName::Nexus) != Some(true) {
            return
        }
        let frame = RenderMachine::ui_read_context().to_frame_storage();
        with_ui(|ui| {
            RenderMachine::turn_ui_entry(ui);
            RenderState::render_ui(ui, frame.as_ref());
        });
    }
}
extern "C-unwind" fn unsafe_options() {
    unsafe {
        let mut running = loaded() && RenderState::is_running();
        if running {
            let frame = RenderMachine::ui_read_context().to_frame_storage();
            with_ui(|ui| {
                running &= RenderState::render_options(ui, frame.as_ref(), AddonHostName::Nexus);
            });
        }
        if !running {
            unsafe_options_fallback();
        }
    }
}
extern "C-unwind" fn unsafe_options_fallback() {
    unsafe {
        let frame = RenderMachine::ui_read_context().to_frame_storage();
        with_ui(|ui| RenderState::render_options_fallback(ui, frame.as_ref(), AddonHostName::Nexus));
    }
}

pub(self) fn uninit() {
    let unloading = crate::pre_uninit_for(AddonHostName::Nexus);
    if unloading {
        unload();
    }
    uninit_cleanup();

    RUNTIME_AVAILABLE.store(false, Ordering::SeqCst);
    RUNTIME_LOADED.store(false, Ordering::SeqCst);

    if unloading {
        crate::post_uninit_for(AddonHostName::Nexus);
    } else {
        RenderState::try_send(crate::render::RenderEvent::RefreshHost);
    }
}
pub fn enter() -> RuntimeResult<()> {
    if available() {
        return Ok(())
    }

    log::debug!("TODO: enter");

    Ok(())
}

/// revert nexus handles if any were registered
///
/// must remain idempotent if used multiple times during shutdown
pub fn uninit_cleanup() {
    if let Some(aapi) = addon_api() {
        unsafe {
            (aapi.renderer.deregister)(unsafe_options);
            (aapi.renderer.deregister)(unsafe_options_fallback);
            (aapi.renderer.deregister)(unsafe_render);
            (aapi.renderer.deregister)(unsafe_render_pre);
            (aapi.wnd_proc.deregister)(wnd);
        }
    }

    quick_access_remove_all();
    unregister_keybinds();
}

pub fn loaded() -> bool {
    match RUNTIME_LOADED.load(Ordering::SeqCst) {
        #[cfg(feature = "extension-nexus-extern")]
        true if r#extern::addon_api().is_none() || r#extern::requested_api() == 0 => false,
        loaded => loaded,
    }
}
pub fn available() -> bool {
    RUNTIME_AVAILABLE.load(Ordering::SeqCst)
}
pub fn disable() {
    RUNTIME_AVAILABLE.store(false, Ordering::SeqCst)
}

pub fn addon_dir() -> RuntimeResult<Option<PathBuf>> {
    if !available() {
        return Ok(None)
    }

    paths::get_addon_dir(exports::ADDON_DIR_NAME)
        .ok_or("Invalid addon dir")
        .map(Some)
}

pub fn detect_language() -> RuntimeResult<Option<LanguageIdentifier>> {
    if !available() {
        return Ok(None)
    }

    let index_to_check = "KB_CHANGELOG";
    let translated = translate(index_to_check).ok_or("Couldn't translate string")?;
    Ok(Some(match &translated[..] {
        "Registro de Alterações" => LANG_PT,
        "更新日志" => i18n::LANG_ZH,
        "Seznam změn" => LANG_CZ,
        "Änderungsprotokoll" => i18n::LANG_DE,
        "Changelog" => i18n::LANG_EN,
        "Notas del parche" => i18n::LANG_ES,
        "Journal des modifications" => i18n::LANG_FR,
        "Registro modifiche" => LANG_IT,
        "Lista zmian" => LANG_PL,
        "Список изменений" => LANG_RU,
        msg => {
            log::info!("unrecognized language: {msg:?}");
            return Ok(None)
        },
    }))
}
const LANG_CZ: LanguageIdentifier = new_lang_id!(cz-*-);
const LANG_IT: LanguageIdentifier = new_lang_id!(it-*-);
const LANG_PL: LanguageIdentifier = new_lang_id!(pl-*-);
const LANG_PT: LanguageIdentifier = new_lang_id!(pt-BR-);
const LANG_RU: LanguageIdentifier = new_lang_id!(ru-*-);
/// locales offered by nexus
/// <https://github.com/RaidcoreGG/Nexus-Translations>
pub static LANGUAGES_EXTRA: [LanguageIdentifier; 5] = [LANG_CZ, LANG_IT, LANG_PL, LANG_PT, LANG_RU];
/// nexus uses region instead of lang code here for some reason, so adjust..
const ZH_CN: i18n::unic_subtags::Region = new_lang_id!(Region: "CN");
/// swappy [ZH_CN]
const CN_ZH: i18n::unic_subtags::Language = new_lang_id!(Language: "cn");
/// throw out the extra tags like region and substitute in [CN_ZH]
fn nexus_language_id(lang: &i18n::LanguageIdentifier) -> Cow<'_, str> {
    match *lang {
        i18n::LanguageIdentifier { region: Some(self::ZH_CN), .. } => Cow::Borrowed(CN_ZH.as_str()),
        // except for this one for some reason..?
        self::LANG_PT => i18n::language_to_string(lang),
        _ => Cow::Borrowed(lang.language.as_str()),
    }
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
    if !loaded() {
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

pub fn send_alert(message: &str) -> RuntimeResult<Option<()>> {
    if !available() {
        return Ok(None)
    }

    alert::send_alert(message);
    Ok(Some(()))
}

pub fn is_nexus_updater() -> bool {
    match () {
        #[cfg(feature = "extension-nexus-extern")]
        _ => is_provider_nexus(&r#extern::requested_provider()),
        #[cfg(feature = "extension-nexus-codegen")]
        _ => is_provider_nexus(&UPDATE_PROVIDER),
    }
}

pub fn addon_api() -> Option<&'static AddonApi> {
    match () {
        #[cfg(feature = "extension-nexus-codegen")]
        _ if !available() => return None,
        #[cfg(feature = "extension-nexus-codegen")]
        _ => Some(AddonApi::get()),
        #[cfg(feature = "extension-nexus-extern")]
        _ => r#extern::addon_api(),
    }
}

pub fn log(metadata: &log::Metadata, message: &CStr) -> RuntimeResult<Option<()>> {
    #[cfg(todo = "unnecessary")]
    if !loaded() {
        return Ok(None)
    }
    let Some(aapi) = addon_api() else { return Ok(None) };

    let level = rt::log::nexus_log_level(metadata.level());
    let channel = rt::NAME_C.as_ptr();

    unsafe {
        (aapi.log)(level, channel, message.as_ptr());
    }

    Ok(Some(()))
}

pub async fn perform_update(release: &rt::update::ResolvedVersion) -> RuntimeResult<Option<()>> {
    #[cfg(todo = "unnecessary")]
    if !loaded() {
        return Ok(None)
    }
    let Some(aapi) = addon_api() else { return Ok(None) };

    let dll_url = release.dll_url(false).await.map_err(|e| {
        log::error!("{e:#}");
        "DLL URL missing"
    })?;
    match () {
        #[cfg(todo = "unnecessary")]
        () => nexus::updater::request_update(SIG, dll_url.as_str()),
        () => {
            let url = String::from(dll_url);
            unsafe {
                let url_c = CString::from_vec_unchecked(url.into());
                (aapi.request_update)(SIG, url_c.as_ptr())
            }
        },
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

    let swap_chain = unsafe { &*(ptr::addr_of!(api.swap_chain) as *const Option<IDXGISwapChain>) };
    if swap_chain.is_none() {
        return Err("DXGI swap chain unavailable")
    }

    Ok(swap_chain.clone().map(Into::into))
}

pub extern "C-unwind" fn wnd(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> u32 {
    if !available() {
        return msg
    }

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
    let texture = nexus_texture_ok(texture).map(rt::textures::NexusTexture::from_nexus);
    TEXTURES.report_load(id, texture);
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
        let language = i18n::current_language();
        let language = nexus_language_id(&language);
        if let Some(timer_trigger) = id.strip_prefix("timer-key-trigger-") {
            // ew special cased...
            set_translation(
                id,
                &language,
                fl!("timer-key-trigger", id = timer_trigger).into_string(),
            )
        } else {
            with_i18n(id, |msg| set_translation(id, &language, msg));
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
    if let Some(aapi) = addon_api() {
        for kb in keybinds.keys() {
            unsafe { (aapi.input_binds.deregister)(kb.as_ptr()) }
        }
    }
    keybinds.clear();
}

pub fn quick_access_add(icon: TaimiControls, state_on: TaimiControls, style: IconStyle) {
    use nexus::quick_access::{add_quick_access, add_quick_access_context_menu};

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
        extern "C-unwind" fn unsafe_render_popup_timers() {
            unsafe {
                let _ = with_ui(|ui| RenderState::render_context_popup(ui, TaimiControls::WINDOW_TIMERS));
            }
        }
        extern "C-unwind" fn unsafe_render_popup_markers() {
            unsafe {
                let _ = with_ui(|ui| RenderState::render_context_popup(ui, TaimiControls::WINDOW_MARKERS));
            }
        }
        extern "C-unwind" fn unsafe_render_popup_pathing() {
            unsafe {
                let _ = with_ui(|ui| RenderState::render_context_popup(ui, TaimiControls::WINDOW_PATHING));
            }
        }
        extern "C-unwind" fn unsafe_render_popup_primary() {
            unsafe {
                let _ = with_ui(|ui| RenderState::render_context_popup(ui, TaimiControls::WINDOW_PRIMARY));
            }
        }
        let menu_id = IconStyle::control_menu_id(identifier);
        let callback = match icon {
            #[cfg(feature = "timers")]
            TaimiControls::WINDOW_TIMERS => unsafe_render_popup_timers,
            #[cfg(feature = "markers")]
            TaimiControls::WINDOW_MARKERS => unsafe_render_popup_markers,
            #[cfg(feature = "space")]
            TaimiControls::WINDOW_PATHING
            | TaimiControls::PATHING_SPACE
            | TaimiControls::PATHING_MINIMAP
            | TaimiControls::PATHING_MAP => unsafe_render_popup_pathing,
            _ => unsafe_render_popup_primary,
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
    let Some(aapi) = addon_api() else { return };
    let Some(identifier) = IconStyle::control_id(icon) else { return };
    if IconStyle::control_has_menu(icon) {
        let menu_id = IconStyle::control_menu_id(identifier);
        unsafe {
            let menu_id = CString::from_vec_unchecked(menu_id.into());
            (aapi.quick_access.remove_context_menu)(menu_id.as_ptr())
        }
    }
    let button_id = IconStyle::control_button_id(identifier);
    unsafe {
        let button_id = CString::from_vec_unchecked(button_id.into());
        (aapi.quick_access.remove)(button_id.as_ptr())
    }
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

fn is_provider_nexus(provider: &UpdateProvider) -> bool {
    matches!(
        provider,
        UpdateProvider::Direct | UpdateProvider::GitHub | UpdateProvider::Raidcore
    )
}
