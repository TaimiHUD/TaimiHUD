#[cfg(feature = "space")]
use {
    crate::{controller::pathing::PathingEvent, space::Engine},
    taimi_meta::ui::MapContext,
};
use {
    crate::{
        controller::timers::{TimersController, TimersEvent},
        exports::{
            self,
            runtime::{self as rt, imgui, RuntimeResult},
        },
        marker::format::MarkerType,
        render::{machine::RenderMachine, RenderState},
        settings::{
            state::{AddonHostName, BootstrapState},
            ArcSettings,
        },
        with_i18n,
    },
    anyhow::Context,
    arcdps::extras::{ExtrasVersion, UserInfoIter},
    arcloader_mumblelink::gw2_mumble::{LinkedMem, MumbleLink},
    dpsapi::combat::{CombatArgs, CombatEvent},
    log::Level,
    std::{
        ffi::{c_void, CStr, CString, OsStr},
        fmt::{self, Write},
        ops,
        panic,
        path::PathBuf,
        ptr::{self, NonNull},
        sync::{
            atomic::{AtomicBool, AtomicPtr, Ordering},
            Mutex,
        },
        thread,
        time::Duration,
    },
    windows::Win32::{
        Foundation::{HMODULE, HWND},
        UI::{Input::KeyboardAndMouse, WindowsAndMessaging},
    },
};
#[cfg(feature = "extension-arcdps-extern")]
use {dpsapi::api::ApiExports as _, std::mem::transmute};

#[cfg(feature = "extension-arcdps-codegen")]
pub(crate) mod cb;
#[cfg(feature = "extension-arcdps-extern")]
pub(crate) mod r#extern;
#[cfg(feature = "extension-arcdps-extras")]
pub(crate) mod unofficial_extras;

pub const SIG: u32 = exports::SIG as u32;

static RUNTIME_AVAILABLE: AtomicBool = AtomicBool::new(false);
static RUNTIME_LOADED: AtomicBool = AtomicBool::new(false);
fn early_init() {
    RUNTIME_AVAILABLE.store(true, Ordering::Relaxed);

    match MumbleLink::new() {
        Ok(ml) => {
            log::debug!("MumbleLink initialized");
            match MUMBLE_LINK.lock() {
                Ok(mut lock) => *lock = Some(ml),
                Err(..) => log::error!("MumbleLink poisoned"),
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
    let res = unsafe { OpenFileMappingA(FILE_MAP_READ.0, false, PCSTR(object_name.as_ptr() as *const _)) };
    match res {
        Ok(handle) => {
            let cleanup = unsafe { CloseHandle(handle) };
            if let Err(e) = cleanup {
                log::warn!("Failed to clean up mapped handle after checking for NexusLink: {e}");
            }
            true
        },
        Err(_e) => {
            // TODO: does it matter what error code we expect, ERROR_OBJECT_NOT_FOUND?
            #[cfg(debug_assertions)]
            {
                log::debug!("NexusLink({object_name}) unavailable: {_e}");
            }
            false
        },
    }
}

#[cfg(feature = "extension-nexus")]
pub(crate) fn check_for_nexus() -> bool {
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
                let res = arc.module.extension_list(|exp| {
                    if exp.sig().map(|s| s.get()).unwrap_or_default() == sig {
                        has_ext = true;
                    }
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

    let res = crate::init().and_then(|()| crate::load_arcdps());

    if res.is_err() {
        RUNTIME_AVAILABLE.store(false, Ordering::SeqCst);
    }

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
    let _ = MUMBLE_LINK.lock().unwrap_or_else(|e| e.into_inner()).take();

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
                Some(handle) if !handle.is_invalid() => Some(handle),
                _ => None,
            },
            false => None,
        };

        Ok(handle.map(|own_handle| Self { own_handle }))
    }

    #[cfg(todo)]
    pub fn free_and_pray(self) {
        use windows::Win32::System::LibraryLoader::FreeLibraryAndExitThread;

        unsafe { FreeLibraryAndExitThread(self.own_handle, 0) };
    }

    pub fn free_and_exit(self) -> ! {
        use windows::Win32::System::LibraryLoader::FreeLibraryAndExitThread;

        log::info!("goodbye");
        unsafe { FreeLibraryAndExitThread(self.own_handle, 0) };
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

    if !available {
        return
    }

    RenderMachine::turn_ui_entry(ui);

    #[cfg(feature = "space")]
    RenderMachine::turn_render_entry();

    RenderState::render_ui(ui);
}

fn imgui_options_tab(ui: &imgui::Ui) {
    if RenderState::is_running() {
        let mut state = RenderState::lock();
        if let Some(ref mut state) = *state {
            state.primary_window.arc_tab.ui_options(ui);
        }
    }
}

fn imgui_options_windows(ui: &imgui::Ui, window_name: Option<&str>) -> bool {
    let hide_checkbox = false;
    if window_name.is_some() || !RenderState::is_running() {
        return hide_checkbox
    }

    let mut settings = match crate::SETTINGS.get().and_then(|s| s.try_write().ok()) {
        Some(s) => s,
        None => return hide_checkbox,
    };
    for &binding in ArcSettings::VK_WINDOWS {
        let Some(window) = binding.window_name() else { continue };
        let window_id = format!("{window}-window");
        let Some(state) = settings.get_window_state_mut(window) else { continue };
        if with_i18n!(&window_id, |msg| ui.checkbox(&msg, state)) {
            // just mutating settings is enough?
        }
    }

    hide_checkbox
}

/// Filtered means we only receive input events if the configured
/// [modifier keys](ui_modifiers) are being held down..?
fn wnd_filter(_hwnd: *mut c_void, msg: u32, w: usize, l: isize) -> u32 {
    if !available() {
        return msg
    }

    match msg {
        WindowsAndMessaging::WM_KEYDOWN
        | WindowsAndMessaging::WM_SYSKEYDOWN
        | WindowsAndMessaging::WM_KEYUP
        | WindowsAndMessaging::WM_SYSKEYUP => {
            // no such thing as a duplicate keyup event, but just in case...
            let prev_down = l & (1 << 30) != 0;
            let repeat = l & 0xff;

            // NOTE: modifiers may be released prior to key release, so this needs to
            // trigger on press to be reliable
            // (resolving this likely requires switching to the non-filtered callback)
            let is_up = matches!(
                msg,
                WindowsAndMessaging::WM_KEYUP | WindowsAndMessaging::WM_SYSKEYUP
            );
            let is_trigger = !is_up && repeat == 1;
            let is_release = is_up && prev_down;
            let settings = crate::SETTINGS.get().and_then(|s| s.try_read().ok());
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
                    core::num::NonZeroU16::new(sc)
                        .and_then(rt::keyboard::scan_code_key)
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
                        PathingEvent::VISIBLE_TOGGLE_SPACE.try_send();
                    }
                }
                if arc.binding_matches(&ArcSettings::VK_RENDER_TOGGLE_PATHING_MINIMAP, vk) {
                    bound = true;
                    if is_trigger {
                        PathingEvent::visible_toggle(MapContext::Minimap).try_send();
                    }
                }
                if arc.binding_matches(&ArcSettings::VK_RENDER_TOGGLE_PATHING_MAP, vk) {
                    bound = true;
                    if is_trigger {
                        PathingEvent::visible_toggle(MapContext::Global).try_send();
                    }
                }
            }

            #[cfg(feature = "timers")]
            for binding in &ArcSettings::VK_TIMER_TRIGGERS {
                if arc.binding_matches(binding, vk) {
                    bound = true;
                    if is_release == is_up {
                        TimersController::try_send(TimersEvent::TimerKeyTrigger(
                            binding.id.into(),
                            is_release,
                        ));
                    }
                }
            }
            #[cfg(feature = "timers")]
            if arc.binding_matches(&ArcSettings::VK_TIMER_RESET, vk) {
                TimersController::try_send(TimersEvent::TimerReset);
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
    if !available() {
        return msg
    }

    // ignore duplicates since arcdps proxies these from nexus
    #[cfg(feature = "extension-nexus")]
    if rt::nexus_available() {
        return msg
    }

    match msg {
        _ if !available() => (),
        WindowsAndMessaging::WM_SIZE
            if matches!(
                w as u32,
                WindowsAndMessaging::SIZE_RESTORED | WindowsAndMessaging::SIZE_MAXIMIZED
            ) =>
        {
            // TODO: does DPI scaling mess with this?
            let w = l as u16;
            let h = (l as u32) >> 16;
            let newsize = [w as f32, h as f32];
            crate::resize_render(Some(newsize));
        },
        _ => (),
    }

    rt::handle_wnd_event(HWND(hwnd), msg, w, l)
}

const UPDATE_CHECK_TIMEOUT: Duration = Duration::from_secs(4);
fn get_update_url() -> Option<String> {
    if !loaded() {
        // this may be called prior to init, so ensure logging is present
        crate::crate_init();
    }

    match BootstrapState::read_with(|state| state.update_host_preference()) {
        Some(AddonHostName::ArcDPS) => (),
        Some(pref) => {
            log::info!("skipping get_update_url, {pref} is preferred");
            return None
        },
        None => {
            log::info!("skipping get_update_url, updates disabled");
            return None
        },
    }

    match panic::catch_unwind(|| update_url()) {
        Ok(url) => url,
        Err(e) => {
            crate::log_any_error("get_update_url", &e);
            None
        },
    }
}

/// TODO: is this worthwhile to avoid or no?
const UPDATE_INDIRECT: bool = false;
async fn release_update_url(release: rt::update::ResolvedVersion) -> anyhow::Result<Option<url::Url>> {
    let auth = rt::update::Updater::notify_latest(&release)?;
    match auth {
        true => release.dll_url(UPDATE_INDIRECT).await.map(Some),
        false => Ok(None),
    }
}
pub(crate) fn update_url() -> Option<String> {
    let authorized = rt::update::Updater::get_preference();
    if authorized.will_authorize() == Some(false) {
        log::info!("Auto-update disabled");
        return None
    }

    let res =
        rt::update::ResolvedVersion::latest_release_standalone(UPDATE_CHECK_TIMEOUT, release_update_url)
            .context("Update check failed");
    match res {
        Err(e) => {
            log::warn!("{e:#}");
            None
        },
        Ok(None) => None,
        Ok(Some(dll_url)) => Some(dll_url.as_str().into()),
    }
}

fn combat_local(event: CombatArgs) {
    if !available() {
        return
    }

    match event.event() {
        Some(CombatEvent::Skill(..)) => event.borrow_imp(crate::receive_evtc_local),
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

#[cfg(feature = "extension-arcdps-extras")]
fn extras_squad_update(members: UserInfoIter) {
    if !available() {
        return
    }

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
        () => Some(HMODULE(unsafe { arcdps::exports::raw::free_extension(SIG).0 })),
        #[cfg(feature = "extension-arcdps-extern")]
        () => r#extern::arc_args().and_then(|arc| {
            unsafe { arc.module.arc_extension_remove2(Some(r#extern::ARC_SIG)) }
                .ok()
                .map(|module| HMODULE(module.0))
        }),
    }
    .ok_or(NO_EXPORT)
    .map(Some)
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
    }
    .ok_or(NO_EXPORT)
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

pub fn log_write_record_buffer(
    w: &mut rt::log::LogBuffer,
    record: &log::Record,
) -> Result<ops::Range<usize>, fmt::Error> {
    let colour = match record.level() {
        _ if !log_window_filter(record.metadata()) => None,
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

    log_window_message(message).map(Some)
}

fn log_window_message(message: &CStr) -> RuntimeResult<()> {
    match () {
        #[cfg(feature = "extension-arcdps-codegen")]
        () if !arcdps::exports::has_e8_log_window() => None,
        #[cfg(feature = "extension-arcdps-codegen")]
        () => Some(unsafe { arcdps::exports::raw::e8_log_window(message.as_ptr()) }),
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
                p => unsafe { transmute(p) },
            }
            .map(|e| unsafe { e(Some(message.into())) })
        }),
    }
    .ok_or(NO_EXPORT)
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
        () => Some(unsafe { arcdps::exports::raw::e3_log_file(message.as_ptr()) }),
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
                p => unsafe { transmute(p) },
            }
            .map(|e| unsafe { e(Some(message.into())) })
        }),
    }
    .ok_or(NO_EXPORT)
    .map(Some)
}

pub fn detect_language() -> RuntimeResult<Option<String>> {
    if !available() {
        return Ok(None)
    }

    // unimplemented...
    Ok(None)
}

pub fn mumble_link_ptr() -> RuntimeResult<Option<NonNull<LinkedMem>>> {
    if !available() {
        return Ok(None)
    }

    MUMBLE_LINK
        .lock()
        .map_err(|_| "MumbleLink poisoned")
        .and_then(|ml| {
            ml.as_ref()
                .map(|ml| ml.as_non_null())
                .ok_or("MumbleLink unavailable")
        })
        .map(Some)
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

pub async fn press_marker_bind(
    marker: MarkerType,
    target: bool,
    down: bool,
    position: Option<rt::MousePosition>,
) -> RuntimeResult<Option<()>> {
    if !available() {
        return Ok(None)
    }

    rt::keyboard::press_marker_bind(marker, target, down, position).await
}

pub fn send_alert(ui: &imgui::Ui, message: &str) -> RuntimeResult<Option<()>> {
    if !available() {
        return Ok(None)
    }

    let [r, g, b, _] = ui.style_color(imgui::StyleColor::NavHighlight);
    let (r, g, b) = ((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8);
    let msg = format!("TaimiHUD Alert: <c=#{r:02x}{g:02x}{b:02x}>{message}</c>");
    let msg = unsafe { CString::from_vec_unchecked(msg.into_bytes()) };

    log_window_message(&msg).map(Some)
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
        true if !arcdps::exports::has_e7_ui_modifiers() => None,
        #[cfg(feature = "extension-arcdps-codegen")]
        true if arcdps::exports::has_e7_ui_modifiers() => Some(arcdps::exports::modifiers().into()),
        #[cfg(feature = "extension-arcdps-extern")]
        true => r#extern::arc_args()
            .and_then(|arc| arc.module.arc_ui_modifiers().ok())
            .map(Into::into),
        _ => None,
    }
    .unwrap_or(ModifierKeys::ARC_DEFAULT)
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
    }
    .map(Into::into))
}
