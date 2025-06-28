use {
    arcdps::{
        extras::{Control, ExtrasAddonInfo, Key, KeybindChange, UserInfoIter},
        Language,
    },
    arcloader_mumblelink::{
        gw2_mumble::{LinkedMem, MumbleLink, MumblePtr},
        identity::MumbleIdentity,
    },
    crate::{
        exports::{self, runtime::{self as rt, imgui::{self, Ui}, keyboard::KeyInput, mouse::MouseInput, KeyState, RuntimeResult}},
        game_language_id,
        marker::format::MarkerType,
        render::RenderState,
        settings::{ArcSettings, ArcVk, GitHubSource},
    },
    dpsapi::combat::{CombatArgs, CombatEvent},
    log::Level,
    nexus::{data_link::NexusLink, rtapi::RealTimeApi},
    std::{
        cell::RefCell,
        collections::BTreeMap,
        ffi::{c_void, CStr, OsStr},
        fmt::{self, Write},
        ops,
        path::PathBuf,
        ptr::{self, NonNull},
        sync::{atomic::{AtomicBool, AtomicI32, AtomicPtr, Ordering}, Mutex, RwLock},
        time::Duration,
    },
    windows::Win32::{
        Foundation::HMODULE,
        UI::{
            WindowsAndMessaging,
            Input::KeyboardAndMouse,
        },
    },
};
#[cfg(feature = "extension-arcdps-extern")]
use dpsapi::api::ApiExports as _;

#[cfg(feature = "extension-arcdps-extern")]
pub(crate) mod r#extern;
#[cfg(feature = "extension-arcdps-codegen")]
pub(crate) mod cb;

pub const SIG: u32 = exports::SIG as u32;

pub fn gh_repo_src() -> GitHubSource {
    GitHubSource {
        owner: "TaimiHUD".into(),
        repository: "TaimiHUD".into(),
        description: None,
    }
}

static RUNTIME_AVAILABLE: AtomicBool = AtomicBool::new(false);
static RUNTIME_LOADED: AtomicBool = AtomicBool::new(false);
fn early_init() {
    RUNTIME_AVAILABLE.store(true, Ordering::Relaxed);

    match MumbleLink::new() {
        Ok(ml) => {
            log::debug!("MumbleLink initialized");
            let ptr = ml.as_ptr();
            *MUMBLE_LINK.lock().expect("MumbleLink poisoned") = Some(ml);
            MUMBLE_LINK_PTR.store(ptr as *mut _, Ordering::Relaxed);
        },
        Err(e) => {
            log::error!("MumbleLink failed to initialize: {e}");
        },
    }
}

#[cfg(feature = "extension-nexus")]
fn check_for_nexus() -> bool {
    const NEXUS_BRIDGE_SIG: u32 = -0x127e89di32 as u32;

    #[allow(unreachable_patterns)]
    match () {
        #[cfg(feature = "extension-arcdps-codegen")]
        () if cb::available() && arcdps::exports::has_list_extension() => return cb::has_extension::<NEXUS_BRIDGE_SIG>(),
        #[cfg(feature = "extension-arcdps-extern")]
        () => match r#extern::arc_args() {
            Some(arc) => {
                let mut has_nexus = false;
                let res = arc.module.extension_list(|exp| if exp.sig().map(|s| s.get()).unwrap_or_default() == NEXUS_BRIDGE_SIG {
                    has_nexus = true;
                });
                if res.is_ok() {
                    return has_nexus
                }
            },
            None => (),
        },
        _ => (),
    }

    // TODO: we could fall back to check for ArcDPS.dll in the process, but...

    false
}

fn pre_init() {
    RUNTIME_LOADED.store(true, Ordering::Relaxed);
    let _ = rt::log::TaimiLog::setup();
}

fn init() -> Result<(), &'static str> {
    early_init();

    #[cfg(feature = "extension-nexus")]
    if rt::nexus_available() {
        log::info!("already loaded by nexus");
        disable();
    } else if check_for_nexus() {
        log::info!("nexus detected");
    }

    let res = crate::init()
        .and_then(|()| crate::load_arcdps());

    if res.is_err() {
        RUNTIME_AVAILABLE.store(false, Ordering::SeqCst);
    }

    res.map_err(Into::into)
}

fn release() {
    log::trace!("arcdps release");
    MUMBLE_LINK_PTR.store(ptr::null_mut(), Ordering::SeqCst);
    let _ml = MUMBLE_LINK.lock()
        .unwrap_or_else(|e| e.into_inner())
        .take();

    if available() && !rt::nexus_available() {
        crate::unload();
    }

    RUNTIME_AVAILABLE.store(false, Ordering::SeqCst);
    RUNTIME_LOADED.store(false, Ordering::SeqCst);
    EXTRAS_AVAILABLE.store(false, Ordering::SeqCst);
}

static IS_INGAME: AtomicBool = AtomicBool::new(false);

pub fn is_ingame() -> Option<bool> {
    if !available() {
        return None
    }

    Some(IS_INGAME.load(Ordering::Relaxed))
}

static MUMBLE_LINK: Mutex<Option<MumbleLink>> = Mutex::new(None);
static MUMBLE_LINK_PTR: AtomicPtr<LinkedMem> = AtomicPtr::new(ptr::null_mut());

fn mumble_ptr() -> Option<MumblePtr> {
    NonNull::new(MUMBLE_LINK_PTR.load(Ordering::Relaxed))
        .and_then(|mem| unsafe { MumblePtr::new(mem.as_ptr()) })
}

thread_local! {
    static MUMBLE_IDENTITY: RefCell<MumbleIdentity> = RefCell::new(MumbleIdentity::new());
}

fn update_mumble_link() {
    let ml = match mumble_ptr() {
        Some(ml) => ml,
        None => return,
    };

    let update = MUMBLE_IDENTITY.with_borrow_mut(|identity| {
        match identity.update(&ml) {
            true => Some((*identity.identity).clone()),
            false => None,
        }
    });

    if let Some(update) = update {
        crate::receive_mumble_identity(update);
    }
}

#[cfg(todo)]
pub unsafe fn imgui_ui<'u>() -> Option<ManuallyDrop<Ui<'u>>> {
    match () {
        #[cfg(feature = "extension-arcdps-extern")]
        () => r#extern::arc_imgui_ui(),
        #[cfg(feature = "extension-arcdps-codegen")]
        () => arcdps::__macro::ui(),
    }
}

fn imgui(ui: &Ui, not_charsel_loading: bool, _hide: u32) {
    let ingame = not_charsel_loading;
    IS_INGAME.store(ingame, Ordering::Relaxed);

    if !available() { return }

    update_mumble_link();

    #[cfg(feature = "space")] {
        crate::render_space(ui);
    }

    RenderState::render_ui(ui);
}

fn imgui_options_tab(ui: &Ui) {
    ui.text("WORK IN PROGRESS");

    ui.checkbox("Check for updates", &mut false);

    thread_local! {
        static BINDING_BUFFERS: std::cell::RefCell<std::collections::HashMap<&'static str, String>> = Default::default();
    }

    fn keybind_ui<F: FnOnce(&ArcVk)>(ui: &Ui, vk: &'static ArcVk, action: Option<F>) {
        let _id_token = ui.push_id(vk.id);
        let name = vk.get_name();
        match action {
            Some(action) => if ui.button(name) {
                action(vk)
            },
            None => ui.text(name),
        }
        ui.same_line();

        let default_vk = vk.vkeycode_default();
        let default_vsc = default_vk.and_then(rt::keyboard::scan_code);
        let default_name = default_vsc.and_then(|vsc| rt::keyboard::key_name(vsc).ok());

        let changed = BINDING_BUFFERS.with_borrow_mut(|b| {
            let binding_buffer = b.entry(vk.id);
            let is_fresh = matches!(binding_buffer, std::collections::hash_map::Entry::Vacant(..));
            let binding_buffer = binding_buffer.or_default();
            if is_fresh {
                if let Some(current_vk) = vk.get_setting_vkeycode() {
                    use std::fmt::Write;

                    let current_name = rt::keyboard::scan_code(current_vk).and_then(|vsc| rt::keyboard::key_name(vsc).ok());
                    let _ = if let Some(name) = current_name {
                        write!(binding_buffer, "{name}")
                    } else {
                        write!(binding_buffer, "{}", current_vk.0)
                    };
                }
            }
            let input = ui.input_text("Keybind", binding_buffer)
                .auto_select_all(true)
                .always_insert_mode(true)
                .enter_returns_true(true)
                .no_undo_redo(true)
                .no_horizontal_scroll(true);
            let changed = match (default_name, default_vk) {
                (Some(name), _) => input.hint(name.to_string()),
                (None, Some(vk)) => input.hint(format!("{}", vk.0)),
                (None, None) => input.hint("unbound by default".into()),
            }.build();

            match changed {
                false => None,
                true => match binding_buffer.parse::<u16>() {
                    Ok(new) => {
                        log::debug!("updating {} keybind to: {new:#x}", vk.id);
                        Some(KeyboardAndMouse::VIRTUAL_KEY(new))
                    },
                    Err(_) => {
                        log::warn!("TODO: update {} keybind to: {binding_buffer:?}", vk.id);
                        None
                    },
                },
            }
        });

        if let Some(new) = changed {
            if let Err(e) = vk.set_vkeycode(new) {
                log::error!("saving keybind {} failed: {}", vk.id, e);
            }
        }
    }

    ui.new_line();
    for &binding in ArcSettings::VK_WINDOWS {
        keybind_ui(ui, binding, Some(|vk: &ArcVk| crate::control_window(vk.id, None)));
    }
    #[cfg(feature = "space")]
    if crate::engine_initialized() {
        keybind_ui(ui, &ArcSettings::VK_RENDER_TOGGLE_PATHING, Some(|_vk: &ArcVk| crate::Engine::try_send(crate::SpaceEvent::PathingToggle)));
    }
    for binding in &ArcSettings::VK_TIMER_TRIGGERS {
        keybind_ui(ui, binding, Some(|vk: &ArcVk| crate::Controller::try_send(crate::ControllerEvent::TimerKeyTrigger(vk.id.into(), false))));
    }
    let all_windows = [
        crate::WINDOW_PRIMARY,
        crate::WINDOW_TIMERS,
        #[cfg(feature = "markers")]
        crate::WINDOW_MARKERS,
    ];
    for window in all_windows {
        let _id = ui.push_id(window);
        let singular = window.strip_suffix("s");
        let name = crate::LANGUAGE_LOADER.get(&format!("{}-window-toggle", singular.unwrap_or(window)));
        if ui.button(name) {
            crate::control_window(window, None);
        }
        ui.same_line();
        ui.text("Keybind: ");
        ui.same_line();
        ui.text_disabled("ALT+SHIFT+");
        ui.same_line();
        if ui.button("BIND") {
            log::warn!("TODO: keybind settings");
        }
        if window == crate::WINDOW_PRIMARY {
            let desc = crate::LANGUAGE_LOADER.get(&format!("{window}-window-toggle-text"));
            ui.text_disabled(desc);
        }
        ui.separator();
    }

    let selected_language = game_language()
        .map(game_language_id)
        .unwrap_or("");
    if let Some(languages) = ui.begin_combo("Language", selected_language) {
        let mut new_language = None;
        for l in crate::LANGUAGES_GAME {
            let id = game_language_id(l);
            let selected = imgui::Selectable::new(id)
                .selected(selected_language == id)
                .build(ui);
            if selected {
                new_language = Some(Ok(l));
            }
        }
        for id in crate::LANGUAGES_EXTRA {
            let selected = imgui::Selectable::new(id)
                .selected(selected_language == id)
                .build(ui);
            if selected {
                new_language = Some(Err(id));
            }
        }
        languages.end();

        if let Some(new_language) = new_language {
            log::warn!("TODO: language selection");
        }
    }
}

fn imgui_options_windows(ui: &Ui, window_name: Option<&str>) -> bool {
    let hide_checkbox = false;
    hide_checkbox
}

/// Filtered means we only receive input events if the configured
/// [modifier keys](ui_modifiers) are being held down..?
fn wnd_filter(_hwnd: *mut c_void, msg: u32, w: usize, l: isize) -> u32 {
    if !available() { return msg }

    match msg {
        WindowsAndMessaging::WM_KEYDOWN | WindowsAndMessaging::WM_KEYUP => {
            // no such thing as a duplicate keyup event, but just in case...
            let prev_down = l & (1 << 30) != 0;

            let is_up = msg == WindowsAndMessaging::WM_KEYUP;
            let is_release = is_up && prev_down;
            let settings = crate::SETTINGS.get()
                .and_then(|s| s.try_read().ok());
            let arc = match settings.as_ref().map(|s| s.arc()) {
                Some(arc) => arc,
                _ => {
                    log::trace!("key pressed while settings unavailable");
                    return msg
                },
            };

            let vk = KeyboardAndMouse::VIRTUAL_KEY(w as u16);
            let mut bound = false;

            for &binding in ArcSettings::VK_WINDOWS {
                if arc.binding_matches(binding, vk) {
                    bound = true;
                    if is_release {
                        crate::control_window(binding.id, None)
                    }
                }
            }

            #[cfg(feature = "space")]
            if crate::engine_initialized() && arc.binding_matches(&ArcSettings::VK_RENDER_TOGGLE_PATHING, vk) {
                bound = true;
                if is_release {
                    crate::Engine::try_send(crate::SpaceEvent::PathingToggle);
                }
            }

            for binding in &ArcSettings::VK_TIMER_TRIGGERS {
                if arc.binding_matches(binding, vk) {
                    bound = true;
                    if is_release == is_up {
                        crate::Controller::try_send(crate::ControllerEvent::TimerKeyTrigger(binding.id.into(), is_release));
                    }
                }
            }

            match bound {
                true => {
                    // tell game to ignore our keybind
                    0
                },
                false => msg,
            }
        },
        _ => msg,
    }
}

const UPDATE_CHECK_TIMEOUT: Duration = Duration::from_secs(4);

fn update_url() -> Option<String> {
    use tokio::{runtime, time::timeout};

    if !update_allowed() {
        log::debug!("skipping update check");
        return None
    }

    let src = gh_repo_src();
    log::info!("checking for updates at {}...", src);

    let runner = runtime::Builder::new_current_thread()
        .enable_all()
        .build();
    let runner = match runner {
        Ok(r) => r,
        Err(e) => {
            log::warn!("Failed to start update check: {e}");
            return None
        },
    };

    let release = runner.block_on(async move {
        let check = src.latest_release();
        timeout(UPDATE_CHECK_TIMEOUT, check).await
    });
    let release = match release {
        Ok(Ok(release)) => {
            let built_ver = crate::built_info::GIT_HEAD_REF.and_then(|r| r.strip_prefix("refs/tags/v"));
            match release.tag_name.strip_prefix("v") {
                None => {
                    log::info!("Latest version {} unrecognized", release.tag_name);
                    return None
                },
                Some(remote_ver) if remote_ver == env!("CARGO_PKG_VERSION") || Some(remote_ver) == built_ver => {
                    log::info!("{} is up-to-date!", release.name.as_ref().unwrap_or(&release.tag_name));
                    return None
                },
                Some(..) => (),
            }
            log::info!("Latest version is {}", release.name.as_ref().unwrap_or(&release.tag_name));
            let is_dev_build = match built_ver {
                #[cfg(not(debug_assertions))]
                Some(..) => false,
                _ => true,
            };
            if release.prerelease {
                log::info!("Skipping update to pre-release");
                return None
            } else if is_dev_build {
                log::info!("Refusing to update development build");
                return None
            }
            release
        },
        Ok(Err(e)) => {
            log::warn!("Failed to check for update: {e}");
            return None
        },
        Err(e) => {
            log::warn!("{e} while checking for updates");
            return None
        },
    };

    let dll_asset = release.assets.into_iter()
        .find(|a| a.name.ends_with(".dll") /*&& a.state == "uploaded"*/);

    match dll_asset {
        // asset.url can also work as long as Content-Type is set correctly...
        Some(asset) => asset.browser_download_url.map(Into::into),
        None => None,
    }
}

fn update_allowed() -> bool {
    #[cfg(feature = "extension-nexus")]
    if exports::nexus::available() {
        return false
    }

    // TODO: setting somewhere!
    true
}

fn combat_local(event: CombatArgs) {
    if !available() { return }

    match event.event() {
        Some(CombatEvent::Skill(..)) =>
            event.borrow_imp(crate::receive_evtc_local),
        Some(CombatEvent::Agent(agent)) if agent.is_self().get() => {
            if let Some(name) = agent.account_names() {
                crate::receive_account_name(name.to_string_lossy());
            }
        },
        None => {
            log::warn!("unrecognized cbtevent {event:?}");
        },
        _ => (),
    }
}

static EXTRAS_AVAILABLE: AtomicBool = AtomicBool::new(false);

fn extras_init(info: ExtrasAddonInfo) {
    EXTRAS_AVAILABLE.store(true, Ordering::Relaxed);

    log::debug!("arcdps_extras initialized: {info:?}");
}

static GAME_LANGUAGE: AtomicI32 = AtomicI32::new(Language::English as i32);

pub fn game_language() -> Option<Language> {
    let id = GAME_LANGUAGE.load(Ordering::Relaxed);
    Language::try_from(id).ok()
}

fn extras_language(language: Language) {
    if !available() { return }

    let id = language.into();
    let prev = GAME_LANGUAGE.swap(id, Ordering::Relaxed);
    if prev != id {
        let res = crate::load_language(game_language_id(language));
        if let Err(e) = res {
            log::warn!("Failed to change language to {language:?}: {e}");
        }
    }
}

const INTERESTING_BINDS: [Control; 18] = [
    MarkerType::Arrow.control_location(), MarkerType::Arrow.control_object(),
    MarkerType::Circle.control_location(), MarkerType::Circle.control_object(),
    MarkerType::Heart.control_location(), MarkerType::Heart.control_object(),
    MarkerType::Square.control_location(), MarkerType::Square.control_object(),
    MarkerType::Star.control_location(), MarkerType::Star.control_object(),

    MarkerType::Spiral.control_location(), MarkerType::Spiral.control_object(),
    MarkerType::Triangle.control_location(), MarkerType::Triangle.control_object(),
    MarkerType::Cross.control_location(), MarkerType::Cross.control_object(),
    MarkerType::ClearMarkers.control_location(), MarkerType::ClearMarkers.control_object(),
];

static KEYBINDS: RwLock<BTreeMap<Control, KeybindChange>> = RwLock::new(BTreeMap::new());

fn extras_keybind(changed: KeybindChange) {
    if !available() { return }

    if !INTERESTING_BINDS.contains(&changed.control) {
        return
    }

    let mut kb = match KEYBINDS.write() {
        Ok(kb) => kb,
        Err(_) => {
            log::warn!("Keybinds poisoned?");
            return
        },
    };
    kb.insert(changed.control, changed);
}

fn extras_squad_update(members: UserInfoIter) {
    if !available() { return }

    crate::receive_squad_update(members)
}

pub fn loaded() -> bool {
    RUNTIME_LOADED.load(Ordering::Relaxed)
}

pub fn available() -> bool {
    RUNTIME_AVAILABLE.load(Ordering::Relaxed)
}

pub fn disable() {
    RUNTIME_AVAILABLE.store(false, Ordering::SeqCst)
}

pub fn unload_self() -> RuntimeResult<Option<HMODULE>> {
    if !loaded() {
        return Ok(None)
    }

    match () {
        #[cfg(feature = "extension-arcdps-codegen")]
        () if !arcdps::exports::has_free_extension() => None,
        #[cfg(feature = "extension-arcdps-codegen")]
        () => Some(HMODULE(unsafe {
            arcdps::exports::raw::free_extension(SIG).0
        })),
        #[cfg(feature = "extension-arcdps-extern")]
        () => r#extern::arc_args().and_then(|arc| unsafe {
            arc.module.arc_extension_remove2(Some(r#extern::ARC_SIG))
        }.ok().map(|module| HMODULE(module.0))),
    }.ok_or(NO_EXPORT).map(Some)
}

#[cfg(todo)]
pub fn extras_available() -> bool {
    EXTRAS_AVAILABLE.load(Ordering::Relaxed)
}

const NO_EXPORT: &'static str = "arcdps export missing";

pub fn addon_dir() -> RuntimeResult<Option<PathBuf>> {
    if !available() {
        return Ok(None)
    }

    let mut path = match () {
        #[cfg(feature = "extension-arcdps-codegen")]
        () if !arcdps::exports::has_e0_config_path() => None,
        #[cfg(feature = "extension-arcdps-codegen")]
        () => arcdps::exports::config_path(),
        #[cfg(feature = "extension-arcdps-extern")]
        () => r#extern::arc_args().and_then(|arc| arc.module.get_ini_path().ok()),
    }.ok_or(NO_EXPORT)?;
    // remove ini leaf from path...
    if !path.pop() {
        return Err("Incomplete config path")
    }

    let in_addons = path.file_name() == Some(OsStr::new("arcdps"))
        || path.parent().and_then(|p| p.file_name()) == Some(OsStr::new("addons"));
    if in_addons {
        path.pop();
    }

    path.push(exports::ADDON_DIR_NAME);
    Ok(Some(path))
}

fn log_window_filter(metadata: &log::Metadata) -> bool {
    match metadata.level() {
        _ if !loaded() => false,
        #[cfg(not(debug_assertions))]
        #[cfg(feature = "extension-nexus")]
        Level::Trace | Level::Debug | Level::Info if !available() && exports::nexus::available() => false,
        #[cfg(not(debug_assertions))]
        Level::Trace | Level::Debug => false,
        _ => true,
    }
}

pub fn log_write_record_buffer(w: &mut rt::log::LogBuffer, record: &log::Record) -> Result<ops::Range<usize>, fmt::Error> {
    let colour = match record.level() {
        _ if !log_window_filter(record.metadata()) =>
            None,
        Level::Error => Some("#ff0000"),
        Level::Warn => Some("#ffa0a0"),
        Level::Debug => Some("#80a0a0"),
        Level::Trace => Some("#a0a080"),
        _ => None,
    };

    let window_start = w.len();
    let start = match colour {
        Some(colour) => {
            write!(w, "<c={colour}>")?;
            w.len()
        },
        None => window_start,
    };
    log_write_record(w, record)?;
    let end = w.len();

    if let Some(..) = colour {
        write!(w, "</c>")?;
    }

    Ok(start..end)
}

pub fn log_write_record<W: fmt::Write>(w: &mut W, record: &log::Record) -> fmt::Result {
    rt::log::write_record(w, record)
}

pub fn log_window(metadata: &log::Metadata, message: &CStr) -> RuntimeResult<Option<()>> {
    if !loaded() {
        return Ok(None)
    }

    if !log_window_filter(metadata) {
        return Ok(Some(()))
    }

    match () {
        #[cfg(feature = "extension-arcdps-codegen")]
        () if !arcdps::exports::has_e8_log_window() => None,
        #[cfg(feature = "extension-arcdps-codegen")]
        () => Some(unsafe {
            arcdps::exports::raw::e8_log_window(message.as_ptr())
        }),
        #[cfg(feature = "extension-arcdps-extern")]
        () => r#extern::arc_args().and_then(|arc| arc.module.arc_log_window(message.as_ref()).ok()),
    }.ok_or(NO_EXPORT).map(Some)
}

pub fn log(_metadata: &log::Metadata, message: &CStr) -> RuntimeResult<Option<()>> {
    if !loaded() {
        return Ok(None)
    }

    match () {
        #[cfg(feature = "extension-arcdps-codegen")]
        () if !arcdps::exports::has_e3_log_file() => None,
        #[cfg(feature = "extension-arcdps-codegen")]
        () => Some(unsafe {
            arcdps::exports::raw::e3_log_file(message.as_ptr())
        }),
        #[cfg(feature = "extension-arcdps-extern")]
        () => r#extern::arc_args().and_then(|arc| arc.module.arc_log(message.as_ref()).ok()),
    }.ok_or(NO_EXPORT).map(Some)
}

pub fn detect_language() -> RuntimeResult<Option<String>> {
    if !available() {
        return Ok(None)
    }

    let language = game_language().map(game_language_id);
    Ok(language.map(Into::into))
}

pub fn mumble_link_ptr() -> RuntimeResult<Option<MumblePtr>> {
    if !available() {
        return Ok(None)
    }

    match mumble_ptr() {
        Some(ml) => Ok(Some(ml)),
        None => Err("MumbleLink unavailable"),
    }
}

pub fn nexus_link_ptr() -> RuntimeResult<Option<NonNull<NexusLink>>> {
    if !available() {
        return Ok(None)
    }

    Err("NexusLink unavailable")
}

pub fn rtapi() -> RuntimeResult<Option<RealTimeApi>> {
    if !available() {
        return Ok(None)
    }

    Err("RTAPI unsupported")
}

pub async fn press_marker_bind(marker: MarkerType, target: bool, down: bool, position: Option<rt::MousePosition>) -> RuntimeResult<Option<()>> {
    if !available() {
        return Ok(None)
    }

    let control = match target {
        true => marker.control_object(),
        false => marker.control_location(),
    };

    let binding = {
        let kb = KEYBINDS.read()
            .map_err(|_| "keybinds poisoned")?;
        kb.get(&control).cloned()
    }.ok_or("unknown keybind")?;

    let mut mods = KeyState::from(&binding);
    match binding.key {
        Key::Key(keycode) => {
            if let Some(position) = position {
                // move the mouse into position first...
                rt::mouse::send_mouse(MouseInput::with_position(position), None)?;
            }
            let mut input = KeyInput::empty_with_mods(mods, down);
            input.vk = KeyInput::from(keycode).vk;
            //rt::keyboard::send_key_input(input)
            rt::keyboard::send_key_combo(input)
        },
        Key::Mouse(button) => {
            let button = KeyState::try_from(button)?;
            let pos = match position {
                Some(p) => p,
                None => rt::screen_mouse_position()?,
            };
            let input = MouseInput::new(pos, button | mods, Some(down));
            let prior = match position {
                // ensure the mouse is moved if a position was explicitly requested
                Some(..) => Some(MouseInput::new(rt::MousePosition::EMPTY, input.button_before(), None)),
                _ => None,
            };
            let mouse_mods = match mods.take(MouseInput::EVENT_MODS) {
                mouse_mods if !mods.is_empty() => {
                    // can't eliminate the need to simulate modifier key presses, so just move all mods to that
                    mods.insert(mouse_mods);
                    KeyState::EMPTY
                },
                mouse_mods =>
                    mouse_mods,
            };

            let invoke = || match mouse_mods.is_empty() {
                true /*if position.is_none()*/ => rt::mouse::send_input(input),
                _ => rt::mouse::send_mouse(input, prior),
            };
            match mods.is_empty() {
                true => invoke(),
                false => rt::keyboard::do_key_combo(invoke, KeyInput::empty_with_mods(mods, down)),
            }
        },
        Key::Unknown(..) => {
            Err("unrecognized bind")
        },
    }.map(Some)
}

#[cfg(todo)]
#[derive(Debug, Copy, Clone)]
pub struct ModifierKeys {
    mod1: KeyInput,
    mod2: KeyInput,
    modmulti: KeyInput,
}

#[cfg(todo)]
impl ModifierKeys {
    pub const ARC_DEFAULT: Self = Self {
        mod1: KeyInput::vk_down(KeyboardAndMouse::VK_SHIFT),
        mod2: KeyInput::vk_down(KeyboardAndMouse::VK_MENU),
        modmulti: KeyInput::vk_down(KeyboardAndMouse::VK_SHIFT),
    };
}

#[cfg(todo)]
impl From<u64> for ModifierKeys {
    fn from(ui_mods: u64) -> Self {
        Self {
            mod1: KeyInput::from(ui_mods as u16),
            mod2: KeyInput::from((ui_mods >> 16) as u16),
            modmulti: KeyInput::from((ui_mods >> 32) as u16),
        }
    }
}

#[cfg(todo)]
#[cfg(feature = "extension-arcdps-codegen")]
impl From<arcdps::exports::Modifiers> for ModifierKeys {
    fn from(ui_mods: arcdps::exports::Modifiers) -> Self {
        Self {
            mod1: KeyInput::from(ui_mods.modifier1),
            mod2: KeyInput::from(ui_mods.modifier2),
            modmulti: KeyInput::from(ui_mods.modifier_multi),
        }
    }
}

#[cfg(todo)]
pub fn ui_modifiers() -> ModifierKeys {
    match available() {
        #[cfg(feature = "extension-arcdps-codegen")]
        true if !arcdps::exports::has_e7_ui_modifiers() =>
            None,
        #[cfg(feature = "extension-arcdps-codegen")]
        true if arcdps::exports::has_e7_ui_modifiers() =>
            Some(arcdps::exports::modifiers().into()),
        #[cfg(feature = "extension-arcdps-extern")]
        true => r#extern::arc_args().and_then(|arc| arc.module.arc_ui_modifiers().ok())
            .map(Into::into),
        _ => None,
    }.unwrap_or(ModifierKeys::ARC_DEFAULT)
}

#[cfg(any(feature = "space", feature = "texture-loader"))]
pub fn dxgi_swap_chain() -> RuntimeResult<Option<windows::Win32::Graphics::Dxgi::IDXGISwapChain>> {
    if !available() {
        return Ok(None)
    }

    Ok(match () {
        #[cfg(feature = "extension-arcdps-extern")]
        () => r#extern::dxgi_swap_chain().map(|sc| sc.to_owned()),
        #[cfg(feature = "extension-arcdps-codegen")]
        () => cb::dxgi_swap_chain(),
    })
}
