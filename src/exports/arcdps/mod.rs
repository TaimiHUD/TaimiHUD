use {
    anyhow::Context,
    arcdps::{
        extras::{Control, ExtrasVersion, Key, KeybindChange, UserInfoIter},
        Language,
    },
    arcloader_mumblelink::gw2_mumble::{LinkedMem, MumbleLink},
    crate::{
        exports::{self, runtime::{self as rt, imgui, keyboard::KeyInput, mouse::MouseInput, KeyState, RuntimeResult}},
        game_language_id,
        marker::format::MarkerType,
        render::{machine::RenderMachine, RenderState},
        settings::{ArcSettings, ArcUpdatePreference, GitHubSource, GitHubLatestRelease, Settings},
    },
    dpsapi::combat::{CombatArgs, CombatEvent},
    log::Level,
    std::{
        collections::{
            btree_map,
            BTreeMap,
        },
        ffi::{c_void, CStr, OsStr},
        fmt::{self, Write},
        ops,
        panic,
        path::PathBuf,
        ptr::{self, NonNull},
        sync::{atomic::{AtomicBool, AtomicI32, AtomicU64, AtomicPtr, Ordering}, Mutex, RwLock},
        thread,
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
use {
    crate::space::engine::{Engine, SpaceEvent},
    taimi_meta::ui::MapContext,
};
#[cfg(feature = "extension-arcdps-extern")]
use {
    dpsapi::api::ApiExports as _,
    std::mem::transmute,
};

#[cfg(feature = "extension-arcdps-extern")]
pub(crate) mod r#extern;
#[cfg(feature = "extension-arcdps-codegen")]
pub(crate) mod cb;
#[cfg(feature = "extension-arcdps-extras")]
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
            match MUMBLE_LINK.lock() {
                Ok(mut lock) =>
                    *lock = Some(ml),
                Err(..) =>
                    log::error!("MumbleLink poisoned"),
            }
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
    if let Some(has_nexus) = has_extension(NEXUS_BRIDGE_SIG) {
        return has_nexus
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

#[allow(unreachable_patterns)]
pub(crate) fn has_extension(sig: u32) -> Option<bool> {
    match () {
        #[cfg(feature = "extension-arcdps-codegen")]
        () if cb::available() && arcdps::exports::has_list_extension() => Some(cb::has_extension(sig)),
        #[cfg(feature = "extension-arcdps-extern")]
        () => match r#extern::arc_args() {
            Some(arc) => {
                let mut has_ext = false;
                let res = arc.module.extension_list(|exp| if exp.sig().map(|s| s.get()).unwrap_or_default() == sig {
                    has_ext = true;
                });
                res.ok().map(|_| has_ext)
            },
            None => None,
        },
        _ => None,
    }
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

    let mut keybinds = KEYBINDS.write().unwrap_or_else(|e| e.into_inner());
    macro_rules! default_keybind {
        () => {};
        ($control:ident => KeyCode::$key:ident, $mods:expr; $($rest:tt)*) => {
            if !keybinds.contains_key(&Control::$control) {
                keybinds.insert(Control::$control, KeybindChange {
                    control: Control::$control,
                    index: i32::MAX,
                    mod_alt: $mods.contains(KeyState::ALT),
                    mod_ctrl: $mods.contains(KeyState::CTRL),
                    mod_shift: $mods.contains(KeyState::SHIFT),
                    key: Key::Key(arcdps::extras::KeyCode::$key)
                });
            }
            default_keybind! {
                $($rest)*
            }
        };
    }

    #[cfg(feature = "markers")]
    default_keybind! {
        Squad_Location_Arrow => KeyCode::Number1, KeyState::ALT;
        Squad_Location_Circle => KeyCode::Number2, KeyState::ALT;
        Squad_Location_Square => KeyCode::Number3, KeyState::ALT;
        Squad_Location_Heart => KeyCode::Number4, KeyState::ALT;
        Squad_Location_Star => KeyCode::Number5, KeyState::ALT;
        Squad_Location_Spiral => KeyCode::Number6, KeyState::ALT;
        Squad_Location_Triangle => KeyCode::Number7, KeyState::ALT;
        Squad_Location_X => KeyCode::Number8, KeyState::ALT;
        Squad_ClearAllLocationMarkers => KeyCode::Number9, KeyState::ALT;
        Miscellaneous_Interact => KeyCode::F, KeyState::empty();
        // TODO: settarget, setpersonaltarget, setjadebotwaypoint
        // TODO: setpersonalwaypoint, draw-on-map
    }
    // TODO: restore from stashed memory, since we won't be told a second time!
    drop(keybinds);

    #[cfg(feature = "extension-arcdps-extras")]
    if !extras_available() && unofficial_extras::extras_resubscribe() {
        // TODO: extras_reinit() and stash info?
        EXTRAS_AVAILABLE.store(true, Ordering::Relaxed);
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
    let _ = MUMBLE_LINK.lock()
        .unwrap_or_else(|e| e.into_inner())
        .take();

    let unloading = available() && !rt::nexus_available();
    if unloading {
        crate::unload();
    }

    #[cfg(feature = "extension-arcdps-extras")]
    if extras_available() {
        log::trace!("extras release");
        unsafe {
            unofficial_extras::extras_release();
        }
    }

    RUNTIME_AVAILABLE.store(false, Ordering::SeqCst);
    RUNTIME_LOADED.store(false, Ordering::SeqCst);
    EXTRAS_AVAILABLE.store(false, Ordering::SeqCst);

    if unloading {
        // TODO: avoid if exit is in-flight?
        rt::log::TaimiLog::logger().close();
    }
}

pub struct ExitHandle {
    own_handle: HMODULE,
}

impl ExitHandle {
    pub fn try_exit() -> RuntimeResult<Option<Self>> {
        let handle = match loaded() {
            true => match unload_self()? {
                Some(handle) if !handle.is_invalid() => {
                    Some(handle)
                },
                _ => None,
            },
            false => None,
        };

        Ok(handle.map(|own_handle| Self {
            own_handle,
        }))
    }

    #[cfg(todo)]
    pub fn free_and_pray(self) {
        use windows::Win32::System::LibraryLoader::FreeLibraryAndExitThread;

        unsafe {
            FreeLibraryAndExitThread(self.own_handle, 0)
        };
    }

    pub fn free_and_exit(self) -> ! {
        use windows::Win32::System::LibraryLoader::FreeLibraryAndExitThread;

        log::info!("goodbye");
        unsafe {
            FreeLibraryAndExitThread(self.own_handle, 0)
        };
    }

    pub fn spawn_free(self) {
        let _ = thread::spawn(move || -> ! {
            thread::sleep(Duration::from_millis(400));
            rt::log::TaimiLog::logger().close();
            self.free_and_exit();
        });
    }
}

unsafe impl Send for ExitHandle {}

/// This may block!
pub fn exit() -> RuntimeResult<()> {
    let exit = match ExitHandle::try_exit()? {
        None => return Err("arcdps is unaware of us, maybe not loaded?"),
        Some(h) => h,
    };

    // TODO: stash this away in a static to be spawned *after* release has been called?
    exit.spawn_free();

    Ok(())
}

static IS_INGAME: AtomicBool = AtomicBool::new(false);

pub fn is_ingame() -> Option<bool> {
    if !available() {
        return None
    }

    Some(IS_INGAME.load(Ordering::Relaxed))
}

static MUMBLE_LINK: Mutex<Option<MumbleLink>> = Mutex::new(None);

#[cfg(todo)]
pub unsafe fn imgui_ui<'u>() -> Option<ManuallyDrop<imgui::Ui<'u>>> {
    match () {
        #[cfg(feature = "extension-arcdps-extern")]
        () => r#extern::arc_imgui_ui(),
        #[cfg(feature = "extension-arcdps-codegen")]
        () => arcdps::__macro::ui(),
    }
}

fn imgui(ui: &imgui::Ui, not_charsel_loading: bool, _hide: u32) {
    let available = available();

    IS_INGAME.store(not_charsel_loading, Ordering::Relaxed);

    if !available { return }

    RenderMachine::turn_ui_entry(ui);

    #[cfg(feature = "space")]
    RenderMachine::turn_render_entry();

    RenderState::render_ui(ui);
}

fn imgui_options_tab(ui: &imgui::Ui) {
    if RenderState::is_running() {
        let mut state = RenderState::lock();
        if let Some(ref mut state) = *state {
            state.arc.ui_options(ui);
        }
    }
}

fn imgui_options_windows(_ui: &imgui::Ui, _window_name: Option<&str>) -> bool {
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
            if Engine::is_available() {
                if arc.binding_matches(&ArcSettings::VK_RENDER_TOGGLE_PATHING, vk) {
                    bound = true;
                    if is_trigger {
                        Engine::try_send(SpaceEvent::PathingToggle);
                    }
                }
                if arc.binding_matches(&ArcSettings::VK_RENDER_TOGGLE_PATHING_MINIMAP, vk) {
                    bound = true;
                    if is_trigger {
                        Engine::try_send(SpaceEvent::MapToggle(MapContext::Minimap));
                    }
                }
                if arc.binding_matches(&ArcSettings::VK_RENDER_TOGGLE_PATHING_MAP, vk) {
                    bound = true;
                    if is_trigger {
                        Engine::try_send(SpaceEvent::MapToggle(MapContext::Global));
                    }
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
    #[cfg(todo)]
    if !available() { return msg }

    match msg {
        WindowsAndMessaging::WM_KEYDOWN | WindowsAndMessaging::WM_SYSKEYDOWN
        | WindowsAndMessaging::WM_KEYUP | WindowsAndMessaging::WM_SYSKEYUP if KeyIntercept::intercept_ready() => {
            // TODO: let repeat = l & 0xff; ignore non-zero?

            let is_up = matches!(msg, WindowsAndMessaging::WM_KEYUP | WindowsAndMessaging::WM_SYSKEYUP);

            KeyIntercept::intercept_report(KeyInput {
                vk: KeyboardAndMouse::VIRTUAL_KEY(w as u16),
                down: !is_up,
                // TODO?
                mods: KeyState::empty(),
            });

            return 0;
        },
        _ => (),
    }

    // ignore duplicates since arcdps proxies these from nexus
    #[cfg(feature = "extension-nexus")]
    if rt::nexus_available() { return msg }

    rt::handle_wnd_event(HWND(hwnd), msg, w, l)
}

pub enum KeyIntercept {
    Pending,
    Intercepted {
        key: KeyInput,
    },
}

static KEY_INTERCEPT: AtomicU64 = AtomicU64::new(KeyIntercept::NONE);
impl KeyIntercept {
    const NONE: u64 = 0;
    const PENDING: u64 = u64::MAX;
    const DOWN: u64 = 0x1_00000000_0000;

    pub fn raw(&self) -> u64 {
        match self {
            Self::Pending => Self::PENDING,
            Self::Intercepted { key } => {
                let vk = key.vk.0 as u64;
                let mods = (key.mods.bits() as u64) << 16;
                let down = match key.down {
                    true => Self::DOWN,
                    false => 0,
                };
                vk as u64 | mods | down
            },
        }
    }

    pub fn from_raw(raw: u64) -> Option<Self> {
        Some(match raw {
            0 => return None,
            Self::PENDING => Self::Pending,
            raw => Self::Intercepted {
                key: KeyInput {
                    vk: KeyboardAndMouse::VIRTUAL_KEY(raw as u16),
                    mods: KeyState::from_bits_retain((raw >> 16) as u32),
                    down: raw & Self::DOWN != 0,
                },
            },
        })
    }

    pub fn intercept_restart() {
        KEY_INTERCEPT.store(Self::PENDING, Ordering::SeqCst);
    }

    pub fn intercept_take() -> Option<Self> {
        let mut raw = KEY_INTERCEPT.load(Ordering::SeqCst);
        loop {
            let int = match Self::from_raw(raw) {
                res @ (None | Some(Self::Pending)) => return res,
                int => int,
            };
            match KEY_INTERCEPT.compare_exchange_weak(raw, Self::NONE, Ordering::SeqCst, Ordering::SeqCst) {
                Ok(..) => break int,
                Err(current) => {
                    raw = current;
                },
            }
        }
    }

    pub fn intercept_ready() -> bool {
        KEY_INTERCEPT.load(Ordering::Relaxed) == Self::PENDING
    }

    #[cfg(todo)]
    pub fn intercept_read() -> Option<Self> {
        Self::from_raw(KEY_INTERCEPT.load(Ordering::Relaxed))
    }

    pub fn intercept_report(key: KeyInput) {
        let int = Self::Intercepted {
            key,
        };
        KEY_INTERCEPT.store(int.raw(), Ordering::SeqCst);
    }
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

pub(crate) fn update_url() -> Option<String> {
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

pub(crate) fn update_preference() -> anyhow::Result<ArcUpdatePreference> {
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

pub(crate) fn default_update_preference() -> ArcUpdatePreference {
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

#[cfg(feature = "extension-arcdps-extras")]
fn extras_init(info: ExtrasVersion) {
    EXTRAS_AVAILABLE.store(true, Ordering::Relaxed);

    log::debug!("arcdps_extras initialized: {info:?}");
}

static GAME_LANGUAGE: AtomicI32 = AtomicI32::new(Language::English as i32);

pub fn game_language() -> Option<Language> {
    let id = GAME_LANGUAGE.load(Ordering::Relaxed);
    Language::try_from(id).ok()
}

#[cfg(feature = "extension-arcdps-extras")]
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

const INTERESTING_BINDS: [Control; 19] = [
    MarkerType::Arrow.control_location(), MarkerType::Arrow.control_object(),
    MarkerType::Circle.control_location(), MarkerType::Circle.control_object(),
    MarkerType::Heart.control_location(), MarkerType::Heart.control_object(),
    MarkerType::Square.control_location(), MarkerType::Square.control_object(),
    MarkerType::Star.control_location(), MarkerType::Star.control_object(),

    MarkerType::Spiral.control_location(), MarkerType::Spiral.control_object(),
    MarkerType::Triangle.control_location(), MarkerType::Triangle.control_object(),
    MarkerType::Cross.control_location(), MarkerType::Cross.control_object(),
    MarkerType::ClearMarkers.control_location(), MarkerType::ClearMarkers.control_object(),

    Control::Miscellaneous_Interact,
];

static KEYBINDS: RwLock<BTreeMap<Control, KeybindChange>> = RwLock::new(BTreeMap::new());

#[cfg(feature = "extension-arcdps-extras")]
fn extras_keybind(changed: KeybindChange) {
    if !loaded() { return }

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

    let unbound = matches!(&changed.key, Key::Unknown(0));
    match kb.entry(changed.control.clone()) {
        // maybe we should store the unbound state idk
        btree_map::Entry::Vacant(..) if unbound => (),
        btree_map::Entry::Vacant(e) => {
            e.insert(changed);
        },
        btree_map::Entry::Occupied(e) if e.get().index < changed.index => {
            log::trace!("Keeping {:?}; higher prio than {changed:?}", e.get());
        },
        btree_map::Entry::Occupied(mut e) => {
            log::trace!("Overwrite {:?}; lower prio than {changed:?}", e.get());
            if unbound {
                e.remove();
            } else {
                e.insert(changed);
            }
        },
    }
}

#[cfg(feature = "extension-arcdps-extras")]
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
        _ if metadata.target().starts_with("taimi_pack::") =>
            // avoid visual spam since most packs are full of missing or broken data...
            false,
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
        () => r#extern::arc_args().and_then(|arc| {
            static LOG_WINDOW_EXPORT: AtomicPtr<()> = AtomicPtr::new(ptr::null_mut());

            match LOG_WINDOW_EXPORT.load(Ordering::Relaxed) {
                p if p.is_null() => {
                    let export = arc.module.lookup_e8();
                    if let Some(export) = export {
                        LOG_WINDOW_EXPORT.store(export as usize as *mut _, Ordering::Relaxed)
                    }
                    export
                },
                p => unsafe {
                    transmute(p)
                },
            }.map(|e| unsafe {
                e(Some(message.into()))
            })
        }),
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
        () => r#extern::arc_args().and_then(|arc| {
            static LOG_EXPORT: AtomicPtr<()> = AtomicPtr::new(ptr::null_mut());

            match LOG_EXPORT.load(Ordering::Relaxed) {
                p if p.is_null() => {
                    let export = arc.module.lookup_e3();
                    if let Some(export) = export {
                        LOG_EXPORT.store(export as usize as *mut _, Ordering::Relaxed)
                    }
                    export
                },
                p => unsafe {
                    transmute(p)
                },
            }.map(|e| unsafe {
                e(Some(message.into()))
            })
        }),
    }.ok_or(NO_EXPORT).map(Some)
}

pub fn detect_language() -> RuntimeResult<Option<String>> {
    if !available() {
        return Ok(None)
    }

    let language = game_language().map(game_language_id);
    Ok(language.map(Into::into))
}

pub fn mumble_link_ptr() -> RuntimeResult<Option<NonNull<LinkedMem>>> {
    if !available() {
        return Ok(None)
    }

    MUMBLE_LINK.lock()
        .map_err(|_| "MumbleLink poisoned")
        .and_then(|ml| ml.as_ref()
            .map(|ml| ml.as_non_null())
            .ok_or("MumbleLink unavailable")
        ).map(Some)
}

pub fn nexus_link_ptr() -> RuntimeResult<Option<NonNull<rt::NexusLink>>> {
    if !available() {
        return Ok(None)
    }

    Err("NexusLink unavailable")
}

pub fn rtapi() -> RuntimeResult<Option<rt::RealTimeApi>> {
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
                //rt::mouse::send_mouse(MouseInput::with_position(position), None)?;
                rt::mouse::send_input(MouseInput::with_position(position))?;
            }
            let mut input = KeyInput::empty_with_mods(mods, down);
            input.vk = KeyInput::from(keycode).vk;
            rt::keyboard::send_key_input(input)
            //rt::keyboard::send_key_combo(input)
        },
        Key::Mouse(button) => {
            let button = KeyState::try_from(button)
                .context("Unsupported mouse key")
                .map_err(|e| {
                    log::warn!("{e:#}");
                    "Unsupported mouse key"
                })?;
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
        Key::Unknown(key) => {
            log::error!("cannot invoke keycode {key}");
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
pub fn dxgi_swap_chain() -> RuntimeResult<Option<rt::SwapChain>> {
    if !available() {
        return Ok(None)
    }

    Ok(match () {
        #[cfg(feature = "extension-arcdps-extern")]
        () => r#extern::dxgi_swap_chain().map(|sc| sc.to_owned()),
        #[cfg(feature = "extension-arcdps-codegen")]
        () => cb::dxgi_swap_chain(),
    }.map(Into::into))
}
