use {
    arcdps::{
        extras::{Control, ExtrasVersion, Key, KeybindChange, UserInfoIter},
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
        settings::{ArcSettings, ArcUpdatePreference, ArcVk, GitHubSource, GitHubLatestRelease, Settings},
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
        panic,
        path::PathBuf,
        ptr::{self, NonNull},
        sync::{atomic::{AtomicBool, AtomicI32, AtomicPtr, Ordering}, Mutex, RwLock},
        time::Duration,
    },
    windows::Win32::{
        Foundation::{HMODULE, HWND},
        UI::{
            WindowsAndMessaging,
            Input::KeyboardAndMouse,
        },
    },
};
#[cfg(feature = "space")]
use crate::space::engine::{Engine, SpaceEvent};
#[cfg(feature = "extension-arcdps-extern")]
use dpsapi::api::ApiExports as _;

#[cfg(feature = "extension-arcdps-extern")]
pub(crate) mod r#extern;
#[cfg(feature = "extension-arcdps-codegen")]
pub(crate) mod cb;
pub(crate) mod unofficial_extras;

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
fn check_for_nexus_bridge() -> bool {
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

#[cfg(feature = "extension-nexus")]
fn check_for_nexus_link() -> bool {
    use windows::{
        core::PCSTR,
        Win32::{
            Foundation::CloseHandle,
            System::Memory::{OpenFileMappingA, FILE_MAP_READ},
        },
    };

    let object_name = {
        //let process_id = windows::Win32::System::Threading::GetCurrentProcessId();
        let process_id = std::process::id();
        format!("DL_NEXUS_LINK_{process_id}\0")
    };
    let res = unsafe {
        OpenFileMappingA(FILE_MAP_READ.0, false, PCSTR(object_name.as_ptr() as *const _))
    };
    match res {
        Ok(handle) => {
            let cleanup = unsafe {
                CloseHandle(handle)
            };
            if let Err(e) = cleanup {
                log::warn!("Failed to clean up mapped handle after checking for NexusLink: {e}");
            }
            true
        },
        Err(_e) => {
            // TODO: does it matter what error code we expect, ERROR_OBJECT_NOT_FOUND?
            #[cfg(debug_assertions)] {
                log::debug!("NexusLink({object_name}) unavailable: {_e}");
            }
            false
        },
    }
}

#[cfg(feature = "extension-nexus")]
fn check_for_nexus() -> bool {
    check_for_nexus_bridge() || check_for_nexus_link()
}

fn pre_init() {
    RUNTIME_LOADED.store(true, Ordering::Relaxed);
    crate::crate_init();
}

fn init() -> Result<(), &'static str> {
    early_init();

    #[cfg(feature = "extension-nexus")]
    if rt::nexus_available() {
        log::info!("already loaded by nexus");
        disable();
        init_continue_with_nexus()?;
    } else if check_for_nexus() {
        log::info!("nexus detected");
        init_continue_with_nexus()?;
    }

    let res = crate::init()
        .and_then(|()| crate::load_arcdps());

    if res.is_err() {
        RUNTIME_AVAILABLE.store(false, Ordering::SeqCst);
    }

    res.map_err(Into::into)
}

/// Returns an empty error to abort init if we'd prefer nexus instead
#[cfg(feature = "extension-nexus")]
fn init_continue_with_nexus() -> Result<(), &'static str> {
    //log::trace!("TODO: option to select between arcdps and nexus");
    //Err("")
    Ok(())
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

    if let Ok(pref) = update_preference() {
        let mut index = ArcUpdatePreference::OPTIONS.iter().position(|opt| opt == &pref.as_option())
            .unwrap_or(0);
        let auto_update = ui.combo("Auto-update", &mut index, &ArcUpdatePreference::OPTIONS, |option| {
            option.as_str().into()
        });
        let mut new_pref = None;
        if auto_update {
            new_pref = ArcUpdatePreference::OPTIONS.get(index).cloned();
        }
        if ui.button("Check now") {
            log::debug!("TODO: update check");
            let _ = update_url();
        }
        let blanket_auth = pref.blanket_authorization();
        let mut authorized = blanket_auth.unwrap_or(false);
        let auth_toggled = Settings::try_read().and_then(|s| s.arc().update_remote_version.as_ref().map(|latest| {
            ui.same_line();
            if latest == rt::CRATE_VERSION {
                ui.text("Up-to-date");
                None
            } else if blanket_auth.is_none() {
                authorized = pref.authorizes_version(latest).unwrap_or(false);
                ui.checkbox(format!("Allow update to {latest}"), &mut authorized)
                    .then(|| latest.clone())
            } else {
                ui.text("Update available: {latest}");
                None
            }
        })).flatten();
        if auth_toggled.is_some() || new_pref.is_some() {
            if let Some(mut settings) = crate::SETTINGS.get().map(|s| s.blocking_write()) {
                let arc = settings.arc_mut();
                let pref = match new_pref {
                    Some(pref) =>
                        arc.update_preference.insert(pref),
                    None =>
                        arc.update_preference.get_or_insert_with(|| default_update_preference()),
                };
                if let Some(latest) = auth_toggled {
                    pref.authorize_update(latest, authorized);
                }
            }
        }
    }

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
        let default_name = default_vk.and_then(|vk| rt::keyboard::vk_name(vk).ok());

        let changed = BINDING_BUFFERS.with_borrow_mut(|b| {
            let binding_buffer = b.entry(vk.id);
            let is_fresh = matches!(binding_buffer, std::collections::hash_map::Entry::Vacant(..));
            let binding_buffer = binding_buffer.or_default();
            if is_fresh {
                if let Some(current_vk) = vk.get_setting_vkeycode() {
                    use std::fmt::Write;

                    let current_name = rt::keyboard::vk_name(current_vk);
                    let _ = if let Ok(name) = current_name {
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
        keybind_ui(ui, binding, Some(|vk: &ArcVk| if let Some(window) = vk.window_name() {
            crate::control_window(window, None);
        }));
    }
    #[cfg(feature = "space")]
    if Engine::is_available() {
        ui.separator();
        keybind_ui(ui, &ArcSettings::VK_RENDER_TOGGLE_PATHING, Some(|_vk: &ArcVk| Engine::try_send(SpaceEvent::PathingToggle)));
    }
    ui.separator();
    for binding in &ArcSettings::VK_TIMER_TRIGGERS {
        keybind_ui(ui, binding, Some(|vk: &ArcVk| crate::Controller::try_send(crate::ControllerEvent::TimerKeyTrigger(vk.id.into(), false))));
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
        WindowsAndMessaging::WM_KEYDOWN | WindowsAndMessaging::WM_SYSKEYDOWN
        | WindowsAndMessaging::WM_KEYUP | WindowsAndMessaging::WM_SYSKEYUP => {
            // no such thing as a duplicate keyup event, but just in case...
            let prev_down = l & (1 << 30) != 0;
            let repeat = l & 0xff;

            // NOTE: modifiers may be released prior to key release, so this needs to
            // trigger on press to be reliable
            // (resolving this likely requires switching to the non-filtered callback)
            let is_up = matches!(msg, WindowsAndMessaging::WM_KEYUP | WindowsAndMessaging::WM_SYSKEYUP);
            let is_trigger = !is_up && repeat == 1;
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

            let vk = match w as u16 {
                #[cfg(todo)]
                0 => {
                    let sc = ((l >> 16) & 0xff) as u16;
                    core::num::NonZeroU16::new(sc).and_then(rt::keyboard::scan_code_key)
                        .unwrap_or(KeyboardAndMouse::VIRTUAL_KEY(sc))
                },
                w => KeyboardAndMouse::VIRTUAL_KEY(w as u16),
            };
            let mut bound = false;

            for &binding in ArcSettings::VK_WINDOWS {
                if arc.binding_matches(binding, vk) {
                    bound = true;
                    if is_trigger {
                        if let Some(window) = binding.window_name() {
                            crate::control_window(window, None)
                        }
                    }
                }
            }

            #[cfg(feature = "space")]
            if Engine::is_available() && arc.binding_matches(&ArcSettings::VK_RENDER_TOGGLE_PATHING, vk) {
                bound = true;
                if is_trigger {
                    Engine::try_send(SpaceEvent::PathingToggle);
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

fn wnd(hwnd: *mut c_void, msg: u32, w: usize, l: isize) -> u32 {
    // ignore duplicates since arcdps proxies these from nexus
    #[cfg(feature = "extension-nexus")]
    if rt::nexus_available() { return msg }

    #[cfg(todo)]
    if !available() { return msg }

    rt::handle_wnd_event(HWND(hwnd), msg, w, l)
}

const UPDATE_CHECK_TIMEOUT: Duration = Duration::from_secs(4);
fn get_update_url() -> Option<String> {
    if !loaded() {
        // this may be called prior to init, so ensure logging is present
        crate::crate_init();
    }

    #[cfg(feature = "extension-nexus")]
    if rt::nexus_available() || check_for_nexus() {
        log::info!("skipping get_update_url, nexus is available");
        return None
    }

    match panic::catch_unwind(|| update_url()) {
        Ok(url) => url,
        Err(e) => {
            crate::log_any_error("get_update_url", &e);
            None
        },
    }
}

fn update_url() -> Option<String> {
    let authorized = match update_preference() {
        Err(e) => {
            log::info!("Skipping update check: {e}");
            return None
        },
        Ok(ArcUpdatePreference::Never) => {
            log::info!("Auto-update disabled");
            return None
        },
        Ok(ArcUpdatePreference::Always) => {
            Some(Ok(None))
        },
        Ok(ArcUpdatePreference::Ask { authorized }) => {
            authorized.map(|a| a.map(Some))
        },
        Ok(ArcUpdatePreference::Once { authorized }) => {
            Some(Ok(Some(authorized)))
        },
    };

    let release = match rt::update::latest_release_blocking(&gh_repo_src(), UPDATE_CHECK_TIMEOUT) {
        Err(e) => {
            log::warn!("Update check failed: {e}");
            return None
        },
        Ok(release) => release,
    };
    log::info!("Latest version is {}", release.name.as_ref().unwrap_or(&release.tag_name));
    let res = rt::update::release_dll_url(&release)
        .and_then(|dll| release_is_update(&release).map(|rv|
            (rv, dll)
        ));
    let (release_version, dll_url) = match res {
        Err(e) => {
            log::warn!("Invalid update found: {e}");
            return None
        },
        Ok((None, ..)) => return None,
        Ok((Some(rv), url)) => (rv, url),
    };

    match release_is_allowed(release_version, &authorized) {
        None => {
            log::info!("Update requires user authorization");
            mark_update_outdated(Some(release_version.into()));
            None
        },
        Some(false) => {
            log::info!("Update blacklisted, skipping");
            None
        },
        Some(true) => Some(dll_url.as_str().into()),
    }
}

pub fn release_is_update(release: &GitHubLatestRelease) -> anyhow::Result<Option<&str>> {
    let release_version = rt::update::release_version(release)?;
    // TODO: this is a mess without proper semver
    if release_version == rt::CRATE_VERSION || Some(&release.tag_name[..]) == crate::built_info::git_tag_name() {
        log::info!("Up-to-date with latest version {}!", release.name.as_ref().unwrap_or(&release.tag_name));
        return Ok(None)
    }
    let is_dev_build = match crate::built_info::git_release() {
        #[cfg(not(debug_assertions))]
        Some(..) => false,
        _ => true,
    };
    if release.prerelease {
        log::info!("Skipping update to pre-release");
        return Ok(None)
    } else if is_dev_build {
        log::info!("Refusing to update development build");
        return Ok(None)
    }
    Ok(Some(release_version))
}

pub fn release_is_allowed(release_version: &str, authorized: &Option<Result<Option<String>, String>>) -> Option<bool> {
    match authorized {
        Some(Err(unauthorized)) if unauthorized == release_version => {
            log::info!("Update blacklisted, skipping");
            Some(false)
        },
        Some(Err(..)) =>
            None,
        Some(Ok(None)) =>
            Some(true),
        Some(Ok(Some(authorized))) if authorized == release_version =>
            Some(true),
        Some(Ok(Some(..))) | None =>
            None,
    }
}

fn mark_update_outdated(latest: Option<String>) {
    log::debug!("Recording latest available update: {latest:?}");
    let mut settings = match crate::SETTINGS.get() {
        Some(settings) => settings.blocking_write(),
        None => {
            log::warn!("Settings unavailable to record update status");
            return
        },
    };
    if latest.is_none() && settings.arc.is_none() {
        // nothing to do...
        return
    }
    let arc = settings.arc_mut();
    let updated_pref = match arc.update_preference {
        Some(ArcUpdatePreference::Ask { authorized: Some(..) }) =>
            Some(ArcUpdatePreference::ASK),
        Some(ArcUpdatePreference::Once { .. }) =>
            Some(ArcUpdatePreference::Never),
        _ => None,
    };
    if let Some(pref) = updated_pref {
        arc.update_preference = Some(pref);
    }
    arc.update_remote_version = latest;
    // TODO: schedule save
}

fn update_preference() -> anyhow::Result<ArcUpdatePreference> {
    let mut outdated = false;
    let pref = Settings::read_with_blocking(|settings| {
        let arc = settings.arc();
        match arc.update_preference.as_ref() {
            Some(ArcUpdatePreference::Ask { authorized: Some(Ok(version) | Err(version)) }) if version == rt::CRATE_VERSION => {
                outdated = true;
                ArcUpdatePreference::ASK
            },
            Some(ArcUpdatePreference::Once { authorized }) if authorized == rt::CRATE_VERSION => {
                outdated = true;
                ArcUpdatePreference::Never
            },
            Some(pref) => pref.clone(),
            None => default_update_preference(),
        }
    });
    if outdated {
        mark_update_outdated(None);
    }
    pref
}

fn default_update_preference() -> ArcUpdatePreference {
    #[cfg(feature = "extension-nexus")]
    if exports::nexus::available() {
        return ArcUpdatePreference::Never
    }

    match crate::built_info::is_release() {
        #[cfg(todo)]
        true => ArcUpdatePreference::ASK,
        _ => ArcUpdatePreference::Never,
    }
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

fn extras_init(info: ExtrasVersion) {
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

pub fn exports_present() -> bool {
    #[cfg(feature = "extension-arcdps-codegen")]
    if cb::available() {
        return true
    }
    #[cfg(feature = "extension-arcdps-extern")]
    if r#extern::arc_args().is_some() {
        return true
    }
    loaded()
}

pub fn disable() {
    RUNTIME_AVAILABLE.store(false, Ordering::SeqCst)
}

pub fn disable_load() {
    RUNTIME_LOADED.store(false, Ordering::SeqCst);
    disable();
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
    let path = match () {
        #[cfg(feature = "extension-arcdps-codegen")]
        () if !cb::available() || !arcdps::exports::has_e0_config_path() => None,
        #[cfg(feature = "extension-arcdps-codegen")]
        () => arcdps::exports::config_path(),
        #[cfg(feature = "extension-arcdps-extern")]
        () => r#extern::arc_args().and_then(|arc| arc.module.get_ini_path().ok()),
    }.ok_or(NO_EXPORT)
    .and_then(|mut path| match path.pop() {
        // remove ini leaf from path...
        true => Ok(path),
        false => Err("Incomplete config path"),
    });

    let mut path = match path {
        Ok(path) => path,
        // we tried but aren't actually loaded, so let the caller move on to nexus or whatever
        Err(..) if !available() => return Ok(None),
        Err(e) => return Err(e),
    };

    let in_addons = path.file_name() == Some(OsStr::new("arcdps"))
        || path.parent().and_then(|p| p.file_name()) == Some(OsStr::new("addons"));
    if in_addons {
        path.pop();
    }

    path.push(exports::ADDON_DIR_NAME);
    Ok(Some(path))
}

pub fn log_window_filter(metadata: &log::Metadata) -> bool {
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
    rt::log::write_record(w, record, false)?;
    let end = w.len();

    if let Some(..) = colour {
        write!(w, "</c>")?;
    }

    Ok(start..end)
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
    if !exports_present() {
        return Ok(None)
    }
    #[cfg(feature = "extension-nexus")]
    if exports::nexus::available() && !loaded() {
        return Ok(None)
    }

    match () {
        #[cfg(feature = "extension-arcdps-codegen")]
        () if !cb::available() || !arcdps::exports::has_e3_log_file() => None,
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
