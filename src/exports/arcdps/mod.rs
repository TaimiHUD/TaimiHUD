#[cfg(feature = "extension-arcdps-extern")]
use dpsapi::api::ApiExports as _;
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
            runtime::{
                self as rt,
                alert::LogWarningColour,
                bindings::TaimiControls,
                log::DeferredLogger,
                RuntimeResult,
            },
        },
        marker::format::MarkerType,
        render::{
            element::im::{DrawContextInput, ImDrawWindow, UiContextCell},
            i18n::LanguageIdentifier,
            machine::RenderMachine,
            RenderState,
        },
        settings::{
            state::{AddonHostName, BootstrapState},
            ArcSettings,
        },
    },
    anyhow::Context,
    arcdps::extras::{ExtrasVersion, UserInfoIter},
    arcloader_mumblelink::gw2_mumble::{LinkedMem, MumbleLink},
    dpsapi::{
        api::header::ExtensionLoadResult,
        combat::{CombatArgs, CombatEvent},
    },
    log::Level,
    std::{
        ffi::{c_void, CStr, CString, OsStr},
        fmt::{self, Write},
        mem,
        ops,
        panic,
        path::PathBuf,
        ptr::{self, NonNull},
        sync::{
            atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering},
            Mutex,
        },
        thread,
        time::Duration,
    },
    taimi_ui::im::colours::ImColourContainer,
    windows::Win32::{
        Foundation::{HMODULE, HWND},
        UI::{Input::KeyboardAndMouse, WindowsAndMessaging},
    },
};

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
pub(crate) fn check_for_nexus() -> bool {
    check_for_nexus_bridge() || crate::exports::nexus::datalink::check_for_nexus_link()
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

    let res = match crate::pre_init_for(AddonHostName::ArcDPS) {
        Ok(true) => Ok(()),
        Ok(false) => {
            disable();
            Ok(())
        },
        Err(e) => {
            disable();
            Err(e)
        },
    };

    let res = res
        .and_then(|()| crate::init())
        .and_then(|()| crate::load_arcdps());

    if res.is_err() {
        disable();
    }

    #[cfg(feature = "extension-arcdps-extras")]
    if res.is_ok() && !extras_available() && unofficial_extras::extras_resubscribe() {
        EXTRAS_AVAILABLE.store(true, Ordering::Relaxed);
    }

    crate::post_init_for(AddonHostName::ArcDPS, res.is_ok());

    res.map_err(Into::into)
}

fn release() {
    log::trace!("arcdps release");
    let _ = MUMBLE_LINK.lock().unwrap_or_else(|e| e.into_inner()).take();

    let unloading = crate::pre_uninit_for(AddonHostName::ArcDPS);
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
        crate::post_uninit_for(AddonHostName::ArcDPS);
    }
}

pub struct ExitHandle {
    own_handle: HMODULE,
    arc_loaded: bool,
    ref_count: u8,
}

impl ExitHandle {
    const HMODULE_ERR: &'static str = "my HMODULE";
    pub unsafe fn try_exit() -> RuntimeResult<Option<Self>> {
        let is_loaded = loaded();
        let needs_own_handle = is_loaded || EXTRA_HANDLES.load(Ordering::SeqCst) > 0;
        let mut own_handle = needs_own_handle
            .then(||
            // pre-incrementing ref is important, because arcdps will FreeLibrary upon
            // removal, but we're kinda still alive here!
            Self::own_handle(true).ok_or(Self::HMODULE_ERR))
            .transpose()?;
        if let Some(own_handle) = &mut own_handle {
            if is_loaded {
                match unsafe { unload_self() } {
                    Ok(Some(handle)) if handle.is_invalid() => (),
                    Ok(Some(handle)) if handle.0 != own_handle.own_handle.0 => {
                        log::error!(
                            "removed HMODULE({:p}) mismatches, {:p} expected",
                            handle.0,
                            own_handle.own_handle.0
                        );
                        return Err(Self::HMODULE_ERR)
                    },
                    Ok(Some(_)) => {
                        own_handle.arc_loaded = true;
                    },
                    Err(..) | Ok(None) => (),
                }
            }
            own_handle.commit_ref_count();
        }
        Ok(match own_handle {
            Some(handle) if handle.arc_loaded || handle.ref_count > 1 => Some(handle),
            _ => {
                // just had to check first...
                // NOTE this will drop and free library ref created above
                None
            },
        })
    }

    #[cfg(todo)]
    pub fn free_and_pray(self) {
        use windows::Win32::System::LibraryLoader::FreeLibraryAndExitThread;

        unsafe { FreeLibraryAndExitThread(self.own_handle, 0) };
    }

    pub fn free_and_exit(mut self) -> ! {
        use windows::Win32::System::{LibraryLoader::FreeLibraryAndExitThread, Threading::ExitThread};

        let code = 0;

        let mut last_ref = self.take_one_ref();
        if self.free_refs().is_err() {
            // oh no...
            last_ref = None;
        }
        unsafe {
            match last_ref {
                Some(()) => FreeLibraryAndExitThread(self.own_handle, code),
                None => ExitThread(code),
            }
        }
    }

    fn take_one_ref(&mut self) -> Option<()> {
        self.ref_count.checked_sub(1).map(|rem| {
            self.ref_count = rem;
            ()
        })
    }
    pub fn free_refs(&mut self) -> Result<(), ()> {
        use windows::Win32::Foundation::FreeLibrary;

        let amt = mem::replace(&mut self.ref_count, 0);
        for _ in 0..amt {
            let res = unsafe { FreeLibrary(self.own_handle) };
            if res.is_err() {
                // oh no...
                return Err(())
            }
        }
        Ok(())
    }

    pub fn spawn_free(self) {
        let _ = thread::spawn(move || -> ! { self.free_blocking() });
    }
    pub fn prepare_for_free(&mut self) {
        rt::log::TaimiLog::logger().flush_all();
        self.commit_ref_count();
        let wait = self.wait();
        if wait.as_millis() > 0 {
            thread::sleep(wait);
        }
        self.commit_ref_count();
        log::info!("goodbye x{}", self.ref_count);
        rt::log::TaimiLog::logger().close();
        // TODO: synchronize with main thread and controller too...
    }
    pub fn free_blocking(mut self) -> ! {
        self.prepare_for_free();
        self.free_and_exit();
    }

    fn commit_ref_count(&mut self) {
        let count_extra = EXTRA_HANDLES.swap(0, Ordering::SeqCst);
        self.ref_count = self.ref_count.saturating_add(count_extra as _);
    }

    fn wait(&self) -> Duration {
        match self {
            Self { ref_count: 0, .. } => Duration::from_millis(0),
            Self { arc_loaded: false, .. } => Duration::from_millis(54),
            Self { arc_loaded: true, .. } => Duration::from_millis(80),
        }
    }

    pub fn own_handle(inc_ref: bool) -> Option<Self> {
        use windows::{
            core::PCSTR,
            Win32::System::LibraryLoader::{self as ll, GetModuleHandleExA},
        };
        let mut own_handle = HMODULE::default();
        let res = unsafe {
            let context_msg = "GetModuleHandleExA on exit";
            // a static str like this should be in our module's .text/rodata probably sure
            let sentinel = context_msg.as_ptr();
            let flag_ref = (!inc_ref)
                .then_some(ll::GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT)
                .unwrap_or_default();
            let flags = flag_ref | ll::GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS;
            GetModuleHandleExA(flags, PCSTR(sentinel as *const _), &mut own_handle).context(context_msg)
        };
        rt::log::error_ok(res).and_then(|()| match own_handle.is_invalid() {
            true => None,
            false => Some(Self {
                own_handle,
                ref_count: inc_ref.then_some(1).unwrap_or(0),
                arc_loaded: false,
            }),
        })
    }
}

unsafe impl Send for ExitHandle {}
impl Drop for ExitHandle {
    fn drop(&mut self) {
        if let Err(()) = self.free_refs() {
            log::error!("own FreeLibrary failed???");
        }
    }
}

/// This may block!
#[cfg(todo = "unused")]
pub fn exit() -> RuntimeResult<()> {
    let exit = unsafe { pExitHandle::try_exit() };
    let exit = match exit? {
        None => return Err("arcdps is unaware of us, maybe not loaded?"),
        Some(h) => h,
    };

    // TODO: stash this away in a static to be spawned *after* release has been called?
    exit.spawn_free();

    Ok(())
}

fn add_self() -> RuntimeResult<bool> {
    let Some(own_handle) = ExitHandle::own_handle(true) else {
        return Err(ExitHandle::HMODULE_ERR)
    };
    let mut prev_host = None;
    BootstrapState::try_write_with(|s| {
        // temporary override :3
        prev_host = Some((s.addon_host_preference, s.get_update_host_preference()));
        s.addon_host_preference = Some(AddonHostName::All);
        s.set_update_host_preference(Some(None));
        false
    });
    let res = match () {
        #[cfg(feature = "extension-arcdps-codegen")]
        () if !arcdps::exports::has_add_extension() => None,
        #[cfg(feature = "extension-arcdps-codegen")]
        () => Some(ExtensionLoadResult::from(unsafe {
            arcdps::exports::raw::add_extension(own_handle.own_handle) as u32
        })),
        #[cfg(feature = "extension-arcdps-extern")]
        () => r#extern::arc_args()
            .and_then(|arc| unsafe { arc.module.arc_extension_add2(own_handle.own_handle.into()) }.ok()),
    }
    .ok_or(NO_EXPORT);
    if let Some((prev_host, prev_updater)) = prev_host {
        BootstrapState::try_write_with(|s| {
            // restore override
            s.addon_host_preference = prev_host;
            s.set_update_host_preference(prev_updater);
            false
        });
    }

    match res?.ok() {
        Ok(_res) => {
            // arc leaks a handle here iirc x.x
            // own_handle.ref_count += 1;
            // in case we're wrong+crash, delay reclaiming this handle until later
            EXTRA_HANDLES.fetch_add(1, Ordering::Relaxed);
            Ok(true)
        },
        // duplicate sig means we're probably already loaded
        Err(ExtensionLoadResult::ALREADY_LOADED) => Ok(false),
        Err(e) => {
            log::warn!("addextension2 failed with {e}");
            Err("addextension2")
        },
    }
}
pub fn enter() -> RuntimeResult<()> {
    let res = match loaded() {
        true if available() => return Ok(()),
        true => Some(false),
        false => None,
    };
    log::info!("arc (re?)entry");
    let res = match res {
        None => add_self()?,
        Some(r) => r,
    };
    if res {
        log::debug!("guess we're in");
    } else {
        log::info!("unhiding from arc");
    };
    if !res || !available() {
        RUNTIME_LOADED.store(true, Ordering::SeqCst);
        RUNTIME_AVAILABLE.store(true, Ordering::Relaxed);
    }
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

#[allow(unreachable_patterns)]
#[cfg(todo)]
pub fn imgui_context_ptr() -> Option<NonNull<()>> {
    match () {
        #[cfg(feature = "extension-arcdps-codegen")]
        _ if loaded() => {
            // TODO
            None
        },
        #[cfg(feature = "extension-arcdps-extern")]
        _ => arcffi::nn::nonnull_opt_cast(r#extern::arc_imgui_context_ptr()),
        _ => None,
    }
}

fn imgui_present<'ui>(
    imgui: Option<(&'ui mut UiContextCell, DrawContextInput<'ui>)>,
    not_charsel_loading: bool,
    _hide: u32,
) {
    let available = available();

    IS_INGAME.store(not_charsel_loading, Ordering::Relaxed);

    if !available {
        return
    }

    if let Some(render_ready) = RenderState::pre_render(AddonHostName::ArcDPS) {
        RenderMachine::turn_render_entry();

        if !render_ready {
            RenderState::render_setup();
        }
        if let Some((imgui, context)) = imgui {
            let ui = imgui.bound_ui();

            imgui_draw_present(ui, context)
        }
    }
    RenderState::post_render(AddonHostName::ArcDPS);
}
fn imgui_draw_present<'ui, U>(ui: &'ui mut U, context: DrawContextInput<'ui>)
where
    U: ?Sized + ImDrawWindow<'ui>,
{
    RenderMachine::turn_ui_entry(&mut *ui);

    RenderState::render_ui(ui, context);
}

fn imgui_draw_options_tab<'ui, U>(ui: &mut U, context: DrawContextInput<'ui>)
where
    U: ?Sized + ImDrawWindow<'ui>,
{
    let mut running = available() && RenderState::is_running();

    if running {
        if !RenderState::render_options_arc(ui, context, AddonHostName::ArcDPS) {
            running = false;
        }
    }
    if !running {
        RenderState::render_options_fallback(ui, context, AddonHostName::ArcDPS)
    }
}

fn imgui_draw_options_windows<'ui, U>(
    ui: &mut U,
    context: DrawContextInput<'ui>,
    window_name: Option<&str>,
) -> bool
where
    U: ?Sized + ImDrawWindow<'ui>,
{
    use crate::render::element::prelude::*;
    let hide_checkbox = false;
    if window_name.is_some() || !RenderState::is_running() || !available() {
        return hide_checkbox
    }

    let mut settings = match crate::SETTINGS.get().and_then(|s| s.try_write().ok()) {
        Some(s) => s,
        None => return hide_checkbox,
    };
    let mut context_menu = None;
    for &binding in ArcSettings::VK_WINDOWS {
        let Some(window) = binding.window_name() else { continue };
        let window_id = format!("{window}-window");
        let Some(mut state) = settings.get_window_state(window) else { continue };
        if with_i18n!(&window_id, |msg| ui.checkbox(&msg, &mut state)) {
            let _ = settings.update_window_state(window, state);
        }
        if ui.is_item_right_clicked() {
            context_menu = Some(binding.control().unwrap_or(TaimiControls::WINDOW_PRIMARY));
        }
    }
    drop(settings);

    if let Some(menu) = context_menu {
        RenderState::open_context_menu(ui, menu);
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
            let mut control = None;

            for &binding in ArcSettings::VK_CONTEXT_MENUS {
                if arc.binding_matches(binding, vk) {
                    control = control.or(binding.control());
                }
            }

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

            if let Some(control) = control {
                use crate::exports::runtime::bindings::CONTROLS;

                bound = true;
                if is_trigger {
                    CONTROLS.notify_press(control.to_vk_dummy(), control)
                } else if is_release {
                    CONTROLS.notify_release(control.to_vk_dummy())
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

    match AddonHostName::ArcDPS.is_preferred_update_host() {
        Ok(())
            if AddonHostName::HOST_PRIORITY
                .iter()
                .any(|&h| h != AddonHostName::ArcDPS && h.is_loaded()) =>
        {
            log::warn!("skipping get_update_url, we're not alone");
            return None
        },
        Ok(()) => (),
        Err(pref) => {
            match pref {
                Some(pref) =>
                    log::info!(logger: DeferredLogger::BEST_EFFORT, "skipping get_update_url, {pref} is preferred"),
                None =>
                    log::info!(logger: DeferredLogger::BEST_EFFORT, "skipping get_update_url, updates disabled"),
            }
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
        log::info!(logger: DeferredLogger::BEST_EFFORT, "Auto-update disabled");
        return None
    }

    rt::log::TaimiLog::logger().ensure_available("arcdps get_update_url");

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
static EXTRA_HANDLES: AtomicUsize = AtomicUsize::new(0);

#[cfg(feature = "extension-arcdps-extras")]
fn extras_init(info: ExtrasVersion) {
    EXTRAS_AVAILABLE.store(true, Ordering::Relaxed);
    // UE leaks a handle (but not if we've recovered our callbacks after a reload!)
    EXTRA_HANDLES.fetch_add(1, Ordering::Relaxed);

    log::trace!("arcdps_extras initialized: {info:?}");
}

#[cfg(feature = "extension-arcdps-extras")]
fn extras_squad_update(members: UserInfoIter) {
    if !available() {
        return
    }

    crate::receive_squad_update(members)
}

pub fn loaded() -> bool {
    RUNTIME_LOADED.load(Ordering::SeqCst)
}

pub fn available() -> bool {
    RUNTIME_AVAILABLE.load(Ordering::SeqCst)
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

unsafe fn unload_self() -> RuntimeResult<Option<HMODULE>> {
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
                p => unsafe { mem::transmute(p) },
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
                p => unsafe { mem::transmute(p) },
            }
            .map(|e| unsafe { e(Some(message.into())) })
        }),
    }
    .ok_or(NO_EXPORT)
    .map(Some)
}

pub fn detect_language() -> RuntimeResult<Option<LanguageIdentifier>> {
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

pub fn send_alert<U: ?Sized + ImColourContainer<LogWarningColour>>(
    ui: &U,
    message: &str,
) -> RuntimeResult<Option<()>> {
    if !available() {
        return Ok(None)
    }

    let c = ui.lookup_style_colour(LogWarningColour).truncate() * 255.0f32;
    let (r, g, b) = (c.x as u8, c.y as u8, c.z as u8);
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
pub fn with_dxgi_swap_chain<R, F: FnOnce(&rt::SwapChain) -> R>(f: F) -> Option<R> {
    if !available() {
        return None
    }

    match () {
        #[cfg(feature = "extension-arcdps-extern")]
        () => r#extern::dxgi_swap_chain().map(f),
        #[cfg(feature = "extension-arcdps-codegen")]
        () => cb::dxgi_swap_chain().map(|sc|
            f(&sc)
        ),
    }
}
#[cfg(any(feature = "space", feature = "texture-loader"))]
pub fn dxgi_swap_chain() -> RuntimeResult<Option<rt::SwapChain>> {
    if !available() {
        return Ok(None)
    }

    Ok(match () {
        #[cfg(feature = "extension-arcdps-extern")]
        () => r#extern::dxgi_swap_chain().map(|sc| sc.clone()),
        #[cfg(feature = "extension-arcdps-codegen")]
        () => cb::dxgi_swap_chain(),
    }
    .map(Into::into))
}
