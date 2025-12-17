mod controller;
mod exports;
mod render;
pub mod resources;
mod settings;
mod timer;

#[cfg(feature = "markers")]
mod marker;

#[cfg(feature = "space")]
mod space;

//use i18n_embed_fl::fl;
#[cfg(feature = "extension-nexus")]
use {
    crate::exports::runtime::bindings::TaimiControls,
    nexus::{
        event::{
            arc::{ACCOUNT_NAME, COMBAT_LOCAL},
            event_consume,
            extras::EXTRAS_SQUAD_UPDATE,
            Event,
            MUMBLE_IDENTITY_UPDATED,
            WINDOW_RESIZED,
        },
        rtapi::{
            event::{RTAPI_GROUP_MEMBER_JOINED, RTAPI_GROUP_MEMBER_LEFT, RTAPI_GROUP_MEMBER_UPDATE},
            GroupMember,
            GroupMemberOwned,
        },
    },
    tokio::sync::watch,
};
use {
    crate::{
        controller::{
            markers::{MarkersController, MarkersEvent},
            Controller,
            ControllerEvent,
            ControllerSender,
        },
        exports::runtime as rt,
        render::{i18n, RenderEvent, RenderState},
        settings::{
            state::{AddonHostName, BootstrapState},
            SettingsLock,
        },
    },
    anyhow::Context,
    arcdps::{extras::UserInfo, AgentOwned},
    controller::markers::SquadState,
    marker::format::MarkerType,
    nexus::event::{arc::CombatData, extras::SquadUpdate, MumbleIdentityUpdate},
    relative_path::RelativePath,
    rust_embed::RustEmbed,
    settings::SourcesFile,
    std::{
        ffi::{c_char, CStr},
        mem,
        panic,
        path::PathBuf,
        ptr,
        sync::{Arc, Condvar, LazyLock, Mutex, OnceLock, RwLock},
        thread::{self, JoinHandle},
        time::Duration,
    },
    tokio::sync::mpsc,
};

#[cfg(feature = "space")]
use crate::space::engine::SpaceEvent;
#[cfg(feature = "goggles")]
use crate::space::goggles;

type Revertible = Box<dyn FnOnce() + Send + 'static>;

// https://github.com/kellpossible/cargo-i18n/blob/95634c35eb68643d4a08ff4cd17406645e428576/i18n-embed/examples/library-fluent/src/lib.rs
#[derive(RustEmbed)]
#[folder = "i18n/"]
pub(crate) struct LocalizationsEmbed;

pub mod built_info {
    #[cfg(feature = "built-info")]
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
    #[cfg(not(feature = "built-info"))]
    include!("./built.rs");

    pub const IS_TAGGED_VERSION: bool = option_env!("ADDON_VERSION_RELEASE").is_some();
    /// Broken because nexus makes dumb assumptions...
    pub const IS_TAGGED_RELEASE_OR_RC: bool = match option_env!("ADDON_VERSION_RELEASE") {
        Some(r) if r.len() == 0 => false,
        None => false,
        Some(..) => true,
    };
    #[cfg(todo)]
    pub const IS_TAGGED_RELEASE_OR_RC: bool = IS_TAGGED_RELEASE;
    pub const IS_TAGGED_RELEASE: bool = match option_env!("ADDON_VERSION_RELEASE") {
        Some(r) if r.len() == 1 && r.as_bytes()[0] == b'z' => true,
        _ => false,
    };

    /// Official tagged release build
    pub fn is_release() -> bool {
        #[allow(unreachable_patterns)]
        match IS_TAGGED_VERSION {
            // never allow debug builds to be marked as a release
            #[cfg(debug_assertions)]
            true => false,
            true if git_release() == Some(crate::exports::runtime::CRATE_VERSION) => true,
            _ => false,
        }
    }

    /// Ok(tag) or Err(branch)
    pub fn git_ref_name() -> Result<&'static str, &'static str> {
        match git_tag_name() {
            Some(tag) => Ok(tag),
            None => Err(git_branch_name().or(GIT_HEAD_REF).unwrap_or("HEAD")),
        }
    }

    pub fn git_tag_name() -> Option<&'static str> {
        GIT_HEAD_REF.and_then(|head| head.strip_prefix(GIT_REF_TAG_PREFIX))
    }

    pub fn git_branch_name() -> Option<&'static str> {
        GIT_HEAD_REF.and_then(|head| head.strip_prefix(GIT_REF_BRANCH_PREFIX))
    }

    pub fn git_release() -> Option<&'static str> {
        GIT_HEAD_REF.and_then(|head| head.strip_prefix(GIT_REF_RELEASE_PREFIX))
    }

    use crate::exports::runtime::update::{
        GIT_REF_BRANCH_PREFIX,
        GIT_REF_RELEASE_PREFIX,
        GIT_REF_TAG_PREFIX,
    };
}

static TEXTURES: LazyLock<rt::TextureLoader> = LazyLock::new(|| rt::TextureLoader::new());
static CONTROLLER_SENDER: RwLock<ControllerSender> = RwLock::new(ControllerSender::EMPTY);
#[cfg(feature = "extension-nexus")]
static QUICK_ACCESS_STATE: LazyLock<watch::Sender<TaimiControls>> =
    LazyLock::new(|| watch::Sender::new(TaimiControls::empty()));
static RENDER_SENDER: RwLock<Option<mpsc::Sender<RenderEvent>>> = RwLock::new(None);
static ACCOUNT_NAME_CELL: OnceLock<String> = OnceLock::new();

#[cfg(feature = "space")]
static SPACE_SENDER: RwLock<Option<mpsc::Sender<SpaceEvent>>> = RwLock::new(None);

static CONTROLLER_THREAD: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);

#[cfg(feature = "extension-nexus-codegen")]
nexus::export! {
    name: exports::addon_title!(),
    signature: exports::nexus::cb::SIG,
    load: exports::nexus::cb::load,
    unload: exports::nexus::cb::unload,
    flags: exports::nexus::cb::FLAGS,
    provider: exports::nexus::cb::UPDATE_PROVIDER,
    update_link: exports::nexus::cb::update_url!(),
    // TODO: author: env!("ADDON_AUTHOR")
}

#[cfg(feature = "extension-arcdps-codegen")]
arcdps::export! {
    name: exports::addon_title!(),
    sig: exports::arcdps::SIG,
    init: exports::arcdps::cb::init,
    release: exports::arcdps::cb::release,
    imgui: exports::arcdps::cb::imgui,
    options_end: exports::arcdps::cb::options_end,
    options_windows: exports::arcdps::cb::options_windows,
    wnd_filter: exports::arcdps::cb::wnd_filter,
    raw_wnd_nofilter: exports::arcdps::cb::wnd_raw,
    combat_local: exports::arcdps::cb::combat_local,
    update_url: exports::arcdps::cb::update_url,
    raw_extras_init: exports::arcdps::cb::extras_init_raw,
}

static RENDER_STATE: Mutex<Option<RenderState>> = Mutex::new(None);
static RENDER_UNLOAD: Condvar = Condvar::new();

static SOURCES: RwLock<SourcesFile> = RwLock::new(SourcesFile::EMPTY);
static SETTINGS: OnceLock<SettingsLock> = OnceLock::new();

pub const WINDOW_PRIMARY: &'static str = "primary";
pub const WINDOW_TIMERS: &'static str = "timers";
pub const WINDOW_MARKERS: &'static str = "markers";
pub const WINDOW_PATHING: &'static str = "pathing";

fn marker_icon_data(marker_type: MarkerType) -> Option<Vec<u8>> {
    let arrow = include_bytes!("../data/icons/markers/cmdrArrow.png");
    let circle = include_bytes!("../data/icons/markers/cmdrCircle.png");
    let cross = include_bytes!("../data/icons/markers/cmdrCross.png");
    let heart = include_bytes!("../data/icons/markers/cmdrHeart.png");
    let spiral = include_bytes!("../data/icons/markers/cmdrSpiral.png");
    let square = include_bytes!("../data/icons/markers/cmdrSquare.png");
    let star = include_bytes!("../data/icons/markers/cmdrStar.png");
    let triangle = include_bytes!("../data/icons/markers/cmdrTriangle.png");
    use MarkerType::*;
    match marker_type {
        Arrow => Some(Vec::from(arrow)),
        Circle => Some(Vec::from(circle)),
        Cross => Some(Vec::from(cross)),
        Heart => Some(Vec::from(heart)),
        Spiral => Some(Vec::from(spiral)),
        Square => Some(Vec::from(square)),
        Star => Some(Vec::from(star)),
        Triangle => Some(Vec::from(triangle)),
        Blank => None,
        ClearMarkers => None,
    }
}

fn crate_init() {
    setup_panic_hook();
    rt::try_init_addon_dir(false, || rt::try_addon_dir().ok());
    let _ = rt::log::TaimiLog::setup();

    // XXX: could consider calling this from a DllMain (or CRT TLS hook fn?),
    // but defining, self.rt_sender.clone() explicit entry points is kinder anyway...
    // but contention over who "owns" the global panic hook or logger will only
    // ever matter if we switch to dynamic linking std anyway, so...
}

pub(crate) fn pre_init_for(host: AddonHostName) -> Result<bool, &'static str> {
    match host.is_preferred_host() {
        Ok(()) => (),
        Err(pref) =>
            return {
                let (loud, desc) = match host.is_active() {
                    true => (true, "already loaded by"),
                    false => (false, "we like"),
                };
                log::info!("ghosting {host}, {desc} {pref} more");
                match loud {
                    #[cfg(todo)]
                    true => Err(pref.name()),
                    true => Err("disabled via boot.json"),
                    false => Ok(false),
                }
            },
    }

    for other in AddonHostName::HOST_PRIORITY.iter().filter(|&h| *h != host) {
        if !other.is_active() {
            continue
        }
        let take_over = match (host, other) {
            #[cfg(feature = "extension-nexus-extern")]
            (AddonHostName::Nexus, _) if exports::nexus::r#extern::is_disabled() => false,
            _ => host.is_explicit_preferred_host().is_ok(),
        };
        match other {
            _ if !take_over => (),
            #[cfg(feature = "extension-arcdps")]
            AddonHostName::ArcDPS => {
                log::info!("switching over from arcdps to {host}");
                exports::arcdps::disable();
                // XXX: we should have arcdps quit here...
            },
            #[cfg(feature = "extension-nexus")]
            AddonHostName::Nexus => {
                log::info!("switching over from nexus to {host}");
                exports::nexus::disable();
            },
            _ => (),
        }
    }

    Ok(true)
}
/// `success` indicates that it will remain loaded after returning from init
///
/// nexus export for example cannot request itself to be unloaded, so will
/// always "succeed"
pub(crate) fn post_init_for(host: AddonHostName, success: bool) {
    #[cfg(todo)]
    let is_primary = success && _host.is_active();

    RenderState::try_send(RenderEvent::RefreshHost);

    if success {
        let effective_host = match host.is_active() {
            false if BootstrapState::current_addon_host().is_some() => AddonHostName::All,
            _ => host,
        };
        BootstrapState::try_write_with(|s| s.try_init_latest_host(effective_host));
    }
}
pub(crate) fn pre_uninit_for(host: AddonHostName) -> bool {
    if host.is_explicit_preferred_host().is_ok() {
        return true
    }

    let mut other_hosts = AddonHostName::HOST_PRIORITY.iter().filter(|&h| *h != host);
    other_hosts.all(|other| !other.is_active())
}
pub(crate) fn post_uninit_for(host: AddonHostName) {
    match rt::is_shutdown() {
        Some(Interruption::GameQuit | Interruption::Abort) => {
            // too much cleanup on exit is a bad idea unfortunately
            // (partly because nexus unload is unsynchronized+racy, therefore urgent)
            return
        },
        _ => (),
    }
    let mut pending_cleanup = false;
    let other_hosts = AddonHostName::HOST_PRIORITY
        .iter()
        .filter(|&h| *h != host)
        .filter(|other| other.is_loaded());
    for other in other_hosts {
        match other {
            #[cfg(feature = "extension-arcdps")]
            AddonHostName::ArcDPS => {
                let exit = unsafe { exports::arcdps::ExitHandle::try_exit() };
                let exit = match exit {
                    Err(e) => {
                        log::warn!("failed to request unload from arcdps: {e}");
                        continue
                    },
                    Ok(exit) => exit,
                };
                if let Some(exit) = exit {
                    pending_cleanup = true;
                    exit.spawn_free();
                }
            },
            _ => (),
        }
    }

    if !pending_cleanup {
        rt::log::TaimiLog::logger().close();
    }

    RenderState::try_send(RenderEvent::RefreshHost);
}

fn init() -> Result<(), &'static str> {
    crate_init();

    let loaded = rt::LOADER_LOCK.lock();
    let mut loaded = match loaded {
        Ok(loaded) if *loaded => {
            log::info!("already loaded, skipping init");
            return Ok(())
        },
        Ok(loaded) => loaded,
        Err(..) => {
            let msg = "loader poisoned";
            log::error!("{msg}");
            return Err(msg)
        },
    };
    if let Ok(addon_dir) = rt::try_addon_dir() {
        BootstrapState::init_addon_dir(&addon_dir);
        rt::init_addon_dir(addon_dir);
    }
    // Say hi to the world :o
    let name = rt::CRATE_NAME;
    let version = rt::CRATE_VERSION;
    let authors = rt::crate_authors();
    log::info!("Loading {name} {version} by {authors}");
    match (built_info::git_ref_name(), built_info::GIT_COMMIT_HASH_SHORT) {
        (Ok(_), _) if built_info::is_release() => (),
        (Ok(tag), commit) => {
            let commit = commit.unwrap_or("HEAD");
            log::info!("Release build {tag}({commit})");
        },
        (Err(branch), Some(commit)) => {
            let platform = built_info::CI_PLATFORM.unwrap_or("unknown");
            log::info!("Development build of {branch}({commit}) on {platform}");
        },
        (Err(branch), None) => log::info!("Development build of {branch}"),
    }

    // Set up the thread
    rt::reset_shutdown();
    let addon_dir = &*ADDON_DIR;

    let lang_config = BootstrapState::read_with(|state| state.language_id());
    let lang_explicit = lang_config.is_some();
    let fallback = i18n::fallback_language();
    let language = lang_config
        .or(rt::detect_language().ok())
        .unwrap_or(fallback.clone());
    if !lang_explicit && &language != fallback {
        log::info!("Loading detected language {language} for internationalization...");
    }
    if let Err(e) = i18n::load_language(&language) {
        log::debug!("Failed language setup: {e:#}");
    }

    #[cfg(feature = "texture-loader")]
    if let Err(e) = TEXTURES.setup() {
        if !rt::nexus_available() {
            return Err(e)
        }
        log::error!("{e:#}");
    } else {
        if let Err(e) = TEXTURES.wait_for_startup() {
            log::error!("{e:#}");
            if !rt::nexus_available() {
                return Err("texture loader didn't start")
            }
        }
    }

    let (controller_sender, controller_receiver) = ControllerSender::new();
    let (render_sender, render_receiver) = mpsc::channel::<RenderEvent>(48);

    let mut render_state = RENDER_STATE.lock().unwrap();

    let controller_handler = {
        let render_sender = render_sender.clone();
        thread::spawn(move || Controller::load(controller_receiver, render_sender, addon_dir.to_owned()))
    };

    // muh queues
    *CONTROLLER_THREAD.lock().unwrap() = Some(controller_handler);
    *CONTROLLER_SENDER.write().unwrap() = controller_sender;

    *render_state = Some(RenderState::new(render_receiver));
    *RENDER_SENDER.write().unwrap() = Some(render_sender);

    log::logger().flush();

    *loaded = true;
    Ok(())
}

#[cfg(feature = "extension-nexus")]
fn load_nexus() {
    use crate::exports::nexus::register_keybind;

    // Handle window toggling with keybind and button
    register_keybind(
        TaimiControls::WINDOW_PRIMARY,
        c"primary-window-toggle",
        c"ALT+SHIFT+M",
    );

    // Handle window toggling with keybind and button
    #[cfg(feature = "markers")]
    register_keybind(
        TaimiControls::WINDOW_MARKERS,
        c"marker-window-toggle",
        c"ALT+SHIFT+L",
    );

    // Handle window toggling with keybind and button
    #[cfg(feature = "timers")]
    register_keybind(
        TaimiControls::WINDOW_TIMERS,
        c"timer-window-toggle",
        c"ALT+SHIFT+K",
    );

    #[cfg(feature = "space")]
    {
        register_keybind(
            TaimiControls::WINDOW_PATHING,
            c"pathing-window-toggle",
            c"ALT+SHIFT+N",
        );
        register_keybind(TaimiControls::PATHING_SPACE, c"pathing-render-toggle", c"(null)");
        register_keybind(
            TaimiControls::PATHING_MINIMAP,
            c"pathing-render-minimap-toggle",
            c"ALT+SHIFT+F1",
        );
        register_keybind(
            TaimiControls::PATHING_MAP,
            c"pathing-render-map-toggle",
            c"ALT+SHIFT+F2",
        );
    }

    #[cfg(feature = "timers")]
    for control in TaimiControls::TIMER_TRIGGERS {
        use std::ffi::CString;

        let id = control.index() - TaimiControls::TIMER_TRIGGER_0.index();
        let id = format!("timer-key-trigger-{id}");
        register_keybind(control, CString::new(id).unwrap_or_default(), c"(null)");
    }
    #[cfg(feature = "timers")]
    register_keybind(TaimiControls::TIMER_RESET, c"timer-key-reset", c"(null)");

    register_keybind(TaimiControls::MENU_PRIMARY, c"context-menu-primary", c"(null)");

    const REQUEST_ACCOUNT_NAME: &'static str = "EV_REQUEST_ACCOUNT_NAME";
    ACCOUNT_NAME
        .subscribe(event_consume!(<c_char> |name| {
            if let Some(name) = name {
                let name = unsafe {CStr::from_ptr(name as *const c_char)};
                receive_account_name(name.to_string_lossy());
            }
        }))
        .revert_on_unload();
    nexus::event::event_raise_notification(REQUEST_ACCOUNT_NAME);

    let combat_callback = event_consume!(|cdata: Option<&CombatData>| {
        if let Some(combat_data) = cdata {
            receive_evtc_local(combat_data);
        }
    });
    COMBAT_LOCAL.subscribe(combat_callback).revert_on_unload();

    // MumbleLink Identity
    #[cfg(any(feature = "markers", feature = "space"))]
    MUMBLE_IDENTITY_UPDATED
        .subscribe(event_consume!(<MumbleIdentityUpdate> |mumble_identity| {
            if let Some(update) = mumble_identity.cloned() {
                Controller::with_sender(|s| if let Some(tx) = s.mumble_identity.as_ref() {
                    tx.send_replace(Some(update));
                });
            }
        }))
        .revert_on_unload();

    #[cfg(feature = "markers")]
    RTAPI_GROUP_MEMBER_LEFT
        .subscribe(event_consume!(
            <GroupMember> | group_member | {
                if let Some(group_member) = group_member {
                    receive_group_update(SquadState::Left, group_member);
                }
            }
        ))
        .revert_on_unload();

    #[cfg(feature = "markers")]
    RTAPI_GROUP_MEMBER_JOINED
        .subscribe(event_consume!(
            <GroupMember> | group_member | {
                if let Some(group_member) = group_member {
                    receive_group_update(SquadState::Joined, group_member);
                }
            }
        ))
        .revert_on_unload();

    #[cfg(feature = "markers")]
    RTAPI_GROUP_MEMBER_UPDATE
        .subscribe(event_consume!(
            <GroupMember> | group_member | {
                if let Some(group_member) = group_member {
                    receive_group_update(SquadState::Update, group_member);
                }
            }
        ))
        .revert_on_unload();

    EXTRAS_SQUAD_UPDATE
        .subscribe(event_consume!(
            <SquadUpdate> | update | {
                if let Some(update) = update {
                    receive_squad_update(update.iter().map(|p| unsafe {
                        // convert reference from disjoint arcdps-rs crate versions
                        mem::transmute::<_, &UserInfo>(p)
                    }));
                }
            }
        ))
        .revert_on_unload();

    nexus::event::extras::KEYBIND_CHANGED
        .subscribe({
            let cb = event_consume!(
                <arcdps::extras::keybinds::RawKeybindChange> | keybind | {
                    if let Some(keybind) = keybind {
                        let keybind = taimi_input::win::keyboard::keybind_change_from_raw(keybind);
                        rt::bindings::process_key_bound(keybind);
                    }
                }
            );
            unsafe {
                // crate versions strike again...
                mem::transmute(cb as unsafe extern "C-unwind" fn(_))
            }
        })
        .revert_on_unload();
    nexus::event::extras::LANGUAGE_CHANGED
        .subscribe({
            let cb = event_consume!(
                <arcdps::Language> | language | {
                    if let Some(language) = language {
                        rt::notify_game_language(*language as i32)
                    }
                }
            );
            unsafe {
                // crate versions strike again...
                mem::transmute(cb as unsafe extern "C-unwind" fn(_))
            }
        })
        .revert_on_unload();

    pub const EV_LANGUAGE_CHANGED: Event<()> = unsafe { Event::new("EV_LANGUAGE_CHANGED") };

    // I don't want to store the localization data in either Nexus or communicate it with Nexus,
    // because this would mean entirely being beholden to Nexus as the addon's loader for the
    // rest of all time.
    EV_LANGUAGE_CHANGED
        .subscribe(event_consume!(
            <()> |_| {
                let res = rt::auto_reload_language()
                    .map_err(anyhow::Error::msg)
                    .context("failed to load language");
                if let Err(e) = res {
                    log::info!("{e:#}");
                }
            }
        ))
        .revert_on_unload();

    WINDOW_RESIZED
        .subscribe(event_consume!(<()> |_| {
            resize_render(None);
        }))
        .revert_on_unload();
}

pub fn resize_render(newsize: Option<[f32; 2]>) {
    match RENDER_STATE.try_lock() {
        Ok(mut state) =>
            if let Some(ref mut state) = *state {
                // TODO: do this on most reloads (move reload/resize to method on RenderState or machine)
                match newsize {
                    Some(newsize) if newsize == state.machine.display_size_ref().to_array() => {
                        log::trace!("Ignoring redundant resize to {newsize:?}");
                        return
                    },
                    Some(newsize) => {
                        log::debug!("Resizing to {newsize:?}");
                        //*state.machine.display_size_mut() = newsize.into();
                        state.machine.reset_display_size();
                    },
                    None => {
                        state.machine.reset_display_size();
                    },
                }
                state.reload(true);
            },
        _ => {
            RenderState::try_send(RenderEvent::Reload);
        },
    }
}

#[cfg(feature = "extension-arcdps")]
fn load_arcdps() -> Result<(), &'static str> {
    Ok(())
}

pub const ADDON_DIR: rt::AddonDir = rt::AddonDir;

fn control_window(window: impl Into<String>, state: Option<bool>) {
    let window = window.into();
    let event = ControllerEvent::WindowState(window, state);
    Controller::try_send(event);
}

fn receive_account_name<N: AsRef<str> + Into<String>>(account_name: N) {
    let account_name_ref = account_name.as_ref();
    let name = match account_name_canon(account_name_ref) {
        Some(n) => n,
        None => return,
    };
    match ACCOUNT_NAME_CELL.get() {
        // ignore duplicates
        Some(prev) if prev == name => return,
        _ => (),
    }
    //log::info!("Received account name: {name:?}");
    let name_owned = match account_name_ref.as_ptr() != name.as_ptr() {
        // if the prefix was stripped, reallocate
        true => name.into(),
        false => account_name.into(),
    };
    match ACCOUNT_NAME_CELL.set(name_owned) {
        Ok(_) => (),
        Err(name) => {
            let prev = ACCOUNT_NAME_CELL.get();
            if Some(&name) != prev {
                log::error!(
                    "Account name {name:?} inconsistent with previously recorded value {:?}",
                    prev.map(|s| &s[..]).unwrap_or("")
                )
            }
        },
    }
}

pub fn account_name_canon<N: ?Sized + AsRef<str>>(account_name: &N) -> Option<&str> {
    let account_name = account_name.as_ref();
    let name = match account_name.strip_prefix(":") {
        Some(name) => name,
        None => account_name,
    };
    match name.is_empty() {
        true => None,
        false => Some(name),
    }
}

fn receive_evtc_local(combat_data: &CombatData) {
    let (evt, src) = match (combat_data.event(), combat_data.src()) {
        (Some(evt), Some(src)) => (evt, src),
        _ => return,
    };
    let (evt, src) = unsafe {
        // convert references from disjoint arcdps-rs crate versions
        (
            mem::transmute::<_, &arcdps::evtc::Event>(evt).clone(),
            AgentOwned::from(ptr::read(src as *const _ as *const arcdps::evtc::Agent)),
        )
    };

    let event = ControllerEvent::CombatEvent { src, evt };
    Controller::try_send(event);
}

#[cfg(all(feature = "markers", feature = "extension-nexus"))]
fn receive_group_update(state: SquadState, group_member: &GroupMember) {
    let group_member: GroupMemberOwned = group_member.into();
    let event = MarkersEvent::RTAPISquadUpdate(state, group_member);
    MarkersController::try_send(event);
}

fn receive_squad_update<'u>(update: impl IntoIterator<Item = &'u UserInfo>) {
    let update: Vec<_> = update
        .into_iter()
        .map(|x| unsafe { ptr::read(x) }.into())
        .collect();
    let event = MarkersEvent::ExtrasSquadUpdate(update);
    MarkersController::try_send(event);
}

fn process_textures() {
    #[cfg(feature = "texture-loader")]
    let res = TEXTURES
        .try_responses(|mut responses| -> anyhow::Result<()> {
            use {
                anyhow::{anyhow, Context},
                rt::textures::{Texture, TextureResponse},
            };
            let mut device = None;

            loop {
                let response = match responses.try_recv() {
                    Err(mpsc::error::TryRecvError::Empty) => break,
                    Err(mpsc::error::TryRecvError::Disconnected) =>
                        Err(anyhow!("texture loader shut down?")),
                    Ok(response) => Ok(response),
                }?;
                match response {
                    TextureResponse::Decoded {
                        key,
                        format,
                        pixels,
                        stride,
                        dimensions,
                    } => {
                        let device = match &mut device {
                            Some(d) => d,
                            device => {
                                let (d3d11, _) =
                                    rt::d3d11_device().context("d3d11 device required to load textures")?;
                                device.insert(d3d11)
                            },
                        };
                        let texture =
                            unsafe { Texture::new_raw(device, &pixels, dimensions, stride, format) };
                        TEXTURES.report_load(key, texture);
                    },
                    TextureResponse::DecodeFailed { key, error } => {
                        log::error!("texture {key} failed to decode: {error:#}");
                        TEXTURES.report_failure(key);
                    },
                    TextureResponse::LoopExit { id } => {
                        log::warn!("texture loader {id:?} exited?");
                    },
                    TextureResponse::LoopEnter { id } => {
                        log::info!("texture loader {id:?} started");
                    },
                }
            }

            Ok(())
        })
        .and_then(|res| res.transpose());
    #[cfg(feature = "texture-loader")]
    match res {
        Err(e) => {
            log::error!("texture processing error: {e:#}");
        },
        Ok(..) => (),
    }
}

fn texture_schedule_bytes<K, B>(key: K, bytes: B)
where
    K: Into<rt::textures::TextureKey>,
    B: Into<Vec<u8>>,
{
    let event = ControllerEvent::LoadTextureIntegrated(key.into(), bytes.into());
    Controller::try_send(event);
}
fn texture_schedule_file<K, P>(key: K, path: P)
where
    K: Into<rt::textures::TextureKey>,
    P: Into<PathBuf>,
{
    let event = ControllerEvent::LoadTexture(key.into(), path.into());
    Controller::try_send(event);
}

fn texture_schedule_path<R, P>(rel: R, path: P)
where
    R: AsRef<RelativePath>,
    P: Into<PathBuf>,
{
    texture_schedule_file(rel.as_ref().as_str(), path)
}

/// it's a bad idea to take too long to unload on quit due to issues on nexus,
/// so instead we perform slow/blocking shutdown in wndproc :<
fn notify_quit() {
    // if !RenderState::is_running() { return }

    log::info!("Preparing for game exit");
    let int = rt::notify_shutdown(Interruption::GameQuit);

    Controller::send_exit(int);

    TEXTURES.quit();

    #[cfg(feature = "goggles")]
    if let Err(e) = goggles::shutdown().context("Goggles shutdown failed") {
        log::error!("{e:#}");
    }

    #[cfg(feature = "space")]
    let _ = SPACE_SENDER.write().unwrap().take();

    if RenderState::is_render_thread() {
        let state = RenderState::lock().take();
        if let Some(state) = state {
            state.unload();
        }
    } else {
        // can't do much more than just shut down our queues...
        let render_sender = RENDER_SENDER.write().unwrap().take();
        if let Some(sender) = render_sender {
            // this seems futile and unlikely to reach the other side but we can try anyway
            let _ = sender.try_send(RenderEvent::Quit(int));
        }
    }

    if rt::nexus_available() {
        // this can take some time :<
        let textures_shutdown = TEXTURES
            .wait_for_shutdown()
            .context("failed to shut down texture loader");
        rt::log::error_ok(textures_shutdown);
    }
}

#[derive(Debug, Default, Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Interruption {
    /// We don't have much time!
    GameQuit = 1,
    /// Addon unload or exit requested
    Shutdown,
    /// Individualized unload message intended for a specific component,
    /// take time to cleanly shutdown
    Temporary,
    /// idk quit asap
    Abort,
    /// Likely due to channel drop or associated component shutdown
    #[default]
    Unspecified,
}

impl Interruption {
    pub const NONE: u8 = 0;
    pub const UNSPECIFIED: u8 = Self::Unspecified.repr();
    pub const REPR_MIN: u8 = Self::GameQuit.repr();
    pub const REPR_MAX: u8 = Self::UNSPECIFIED;

    pub const fn from_bool(value: bool) -> Option<Self> {
        match value {
            true => Some(Interruption::Unspecified),
            false => None,
        }
    }
    pub const fn from_repr(value: u8) -> Option<Self> {
        match value {
            Self::NONE | Self::REPR_MIN..=Self::REPR_MAX => unsafe { Self::from_repr_unchecked(value) },
            _ => None,
        }
    }

    pub const unsafe fn with_repr_unchecked(value: u8) -> Self {
        mem::transmute(value)
    }
    pub const unsafe fn from_repr_unchecked(value: u8) -> Option<Self> {
        mem::transmute(value)
    }
    pub const fn repr(self) -> u8 {
        self as _
    }

    pub fn is_urgent(&self) -> bool {
        matches!(self, Interruption::Abort | Interruption::GameQuit)
    }

    /// Drain remaining queue for termination signal, then close the channel -
    /// discard and ignore everything
    ///
    /// consider a timeout when using this...
    pub async fn drain_signals_async<I: InterruptionSignal>(
        rx: &mut mpsc::Receiver<I>,
    ) -> Option<Interruption> {
        while let Some(e) = rx.recv().await {
            if let Some(reason) = e.interrupted() {
                rx.close();
                return Some(reason)
            }
            // discard and ignore everything else
        }
        None
    }

    /// non-async variant of [Self::drain_signals_async]
    pub fn try_drain_signals<I: InterruptionSignal>(rx: &mut mpsc::Receiver<I>) -> Option<Interruption> {
        let mut int = None;
        while let Ok(e) = rx.try_recv() {
            int = e.interrupted();
            if int.is_some() {
                break
            }
        }
        rx.close();
        int
    }
}

impl From<Interruption> for u8 {
    fn from(value: Interruption) -> Self {
        value.repr()
    }
}
pub trait InterruptionSignal {
    fn interrupted(&self) -> Option<Interruption>;
}
impl InterruptionSignal for Interruption {
    fn interrupted(&self) -> Option<Interruption> {
        Some(*self)
    }
}
impl InterruptionSignal for u8 {
    fn interrupted(&self) -> Option<Interruption> {
        Interruption::from_repr(*self)
    }
}
impl InterruptionSignal for bool {
    fn interrupted(&self) -> Option<Interruption> {
        Interruption::from_bool(*self)
    }
}
impl InterruptionSignal for Option<()> {
    fn interrupted(&self) -> Option<Interruption> {
        self.map(|()| Interruption::Unspecified)
    }
}

fn unload() {
    log::debug!("Shutdown requested...");
    let mut loaded = match rt::LOADER_LOCK.lock() {
        Ok(loaded) if !*loaded => {
            log::warn!("not loaded, skipping unload");
            return
        },
        Ok(loaded) => loaded,
        Err(..) => {
            let msg = "loader poisoned";
            log::error!("{msg}");
            return
        },
    };
    BootstrapState::try_write_with(|s| s.try_update_latest_host(None));

    log::info!("Unloading addon");
    let reason = rt::notify_shutdown(Interruption::Shutdown);

    TEXTURES.quit();

    #[cfg(feature = "goggles")]
    if let Err(e) = goggles::shutdown().context("Goggles shutdown failed") {
        log::error!("{e:#}");
    }

    let controller_handle = CONTROLLER_THREAD.lock().unwrap().take();
    let controller_quit = Controller::send_exit(reason);

    let confirm_render_unload = {
        #[cfg(feature = "space")]
        let _space = SPACE_SENDER.write().unwrap().take();
        let mut render_sender = RENDER_SENDER.write().unwrap();
        let mut render_state = RenderState::lock();

        let render_quit = match RenderState::is_render_thread() {
            true => {
                if let Some(state) = render_state.take() {
                    state.unload();
                }
                None
            },
            _ => render_sender
                .as_ref()
                .map(|sender| sender.try_send(RenderEvent::Quit(reason))),
        };
        let _ = render_sender.take();

        log::logger().flush();
        match render_quit {
            _ if matches!(reason, Interruption::GameQuit) || render_state.is_none() => {
                // it's already gone, nothing more to do here
                false
            },
            Some(Ok(())) => {
                let unload_timeout = match () {
                    #[cfg(feature = "space")]
                    () if _space.is_some() =>
                    // give it time to do more shutdown if needed...
                        Duration::from_millis(1500),
                    _ => Duration::from_millis(67),
                };
                let timeout =
                    RENDER_UNLOAD.wait_timeout_while(render_state, unload_timeout, |state| state.is_some());
                let (mut render_state, timeout) = timeout.unwrap_or_else(|e| e.into_inner());
                if timeout.timed_out() {
                    log::warn!("timed out waiting for render quit");
                }
                let _ = render_state.take();
                timeout.timed_out()
            },
            Some(Err(..)) | None => {
                // clean up what we can if possible
                // anything special needed when game shutting down? if controller_quit.is_none() && controller_handle.is_some() {}
                log::info!("discarding render state");
                let _ = render_state.take();
                true
            },
        }
    };

    if let Err(e) = TEXTURES
        .wait_for_shutdown()
        .context("failed to shut down texture loader")
    {
        log::error!("{e:#}");
    }

    match controller_quit {
        Interruption::Unspecified => log::warn!("Failed to signal controller quit"),
        Interruption::Abort => (),
        #[cfg(feature = "extension-nexus")]
        Interruption::GameQuit if rt::nexus_available() => {
            log::info!("not bothering to wait for controller");
        },
        _ => match controller_handle {
            Some(handle) => {
                log::info!("Waiting for controller shutdown...");
                log::logger().flush();
                if let Err(e) = handle.join() {
                    log_any_error("controller thread", &e);
                }
            },
            None => {
                log::warn!("Controller unavailable?");
            },
        },
    }

    #[cfg(feature = "extension-nexus")]
    exports::nexus::uninit_cleanup();

    if confirm_render_unload {
        unload_render_background();
    }

    *loaded = false;

    #[cfg(todo = "unnecessary")]
    #[cfg(not(debug_assertions))]
    {
        drop(panic::take_hook());
    }

    log::debug!("Unload complete");
    log::logger().flush();
}

fn unload_render() {
    log::info!("Renderer unloading");
    debug_assert!(RenderState::is_render_thread());

    TEXTURES.cleanup(true);

    log::debug!("render unload complete");
    RENDER_UNLOAD.notify_all();
}

/// A limited form of [unload_render()] that should try its best,
/// but isn't able to touch render TLS or single-threaded interfaces
fn unload_render_background() {
    log::warn!("Unloading render state from a background thread");

    if let Some(state) = RENDER_STATE.lock().unwrap().take() {
        state.cleanup_background();
    }

    TEXTURES.cleanup(false);
    RENDER_UNLOAD.notify_all();

    log::logger().flush();
}

fn with_any_error<R, F: FnOnce(&str) -> R>(e: &dyn std::any::Any, f: F) -> R {
    let buf;
    let msg = if let Some(m) = e.downcast_ref::<&str>() {
        *m
    } else if let Some(m) = e.downcast_ref::<String>() {
        &m[..]
    } else if let Some(m) = e.downcast_ref::<Box<str>>() {
        &m[..]
    } else if let Some(m) = e.downcast_ref::<Arc<str>>() {
        &m[..]
    } else if let Some(m) = e.downcast_ref::<anyhow::Error>() {
        buf = m.to_string();
        &buf[..]
    } else {
        "unknown error"
    };
    f(msg)
}

pub(crate) fn log_any_error(name: impl std::fmt::Display, e: &dyn std::any::Any) {
    log_any_error_dyn(&name, e)
}
pub(crate) fn log_any_error_dyn(name: &dyn std::fmt::Display, e: &dyn std::any::Any) {
    with_any_error(e, move |e| log::error!("{name} panicked: {e}"))
}
pub(crate) fn log_join_error(name: &str, e: tokio::task::JoinError) {
    let _ = with_join_error(name, e, |m| log::error!("{m}"));
}
pub(crate) fn with_join_error<R, F: FnOnce(&dyn std::fmt::Display) -> R>(
    name: &str,
    e: tokio::task::JoinError,
    f: F,
) -> Option<R> {
    if e.is_cancelled() {
        log::debug!("{name} task cancelled: {e:#}");
        None
    } else {
        Some(match e.try_into_panic() {
            Ok(e) => with_any_error(&e, move |e| f(&format_args!("{name} task panicked: {e}"))),
            Err(e) => with_any_error(&e, move |e| f(&format_args!("{name} task failed: {e:#}"))),
        })
    }
}

#[track_caller]
fn panic_hook(info: &panic::PanicHookInfo) {
    use std::backtrace::{Backtrace, BacktraceStatus};

    log_any_error(rt::NAME, info.payload());
    let backtrace = match built_info::IS_TAGGED_RELEASE {
        true => Backtrace::capture(),
        false => Backtrace::force_capture(),
    };
    if let Some(location) = info.location() {
        log::error!("Panic occurred in {} at {location}", rt::CRATE_NAME);
    }
    if backtrace.status() != BacktraceStatus::Disabled {
        if built_info::IS_TAGGED_RELEASE_OR_RC || built_info::CI_PLATFORM.is_some() {
            log::error!("{backtrace}");
        } else {
            log::error!("{backtrace:#}");
        }
    }
    rt::log::TaimiLog::logger().flush_all();
}

fn setup_panic_hook() {
    panic::set_hook(Box::new(panic_hook))
}
