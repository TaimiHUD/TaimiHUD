mod controller;
mod exports;
mod render;
pub mod resources;
mod settings;
mod timer;
mod util;

#[cfg(feature = "markers")]
mod marker;

#[cfg(feature = "space")]
mod space;

//use i18n_embed_fl::fl;
#[cfg(feature = "space")]
use space::engine::{Engine, SpaceEvent};
use {
    crate::{
        controller::{Controller, ControllerEvent},
        exports::runtime as rt,
        render::{RenderEvent, RenderState},
        settings::SettingsLock,
    },
    arcdps::{extras::UserInfo, AgentOwned, Language},
    controller::SquadState,
    i18n_embed::{
        fluent::{fluent_language_loader, FluentLanguageLoader},
        DefaultLocalizer, LanguageLoader, RustEmbedNotifyAssets,
    },
    marker::format::MarkerType,
    nexus::{
        event::{
            arc::CombatData,
            extras::SquadUpdate,
            MumbleIdentityUpdate,
        },
        rtapi::{
            GroupMember, GroupMemberOwned,
        },
    },
    relative_path::RelativePathBuf,
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
    tokio::sync::mpsc::{channel, Sender},
    unic_langid_impl::LanguageIdentifier,
};
#[cfg(feature = "extension-nexus")]
use nexus::{
    event::{
        arc::{ACCOUNT_NAME, COMBAT_LOCAL},
        event_consume,
        extras::EXTRAS_SQUAD_UPDATE,
        Event, MUMBLE_IDENTITY_UPDATED,
        WINDOW_RESIZED,
    },
    texture::{load_texture_from_memory, texture_receive, Texture as NexusTexture},
    gui::{register_render, render, RenderType},
    keybind::{keybind_handler, register_keybind_with_string},
    quick_access::{add_quick_access, add_quick_access_context_menu},
    rtapi::event::{
        RTAPI_GROUP_MEMBER_JOINED, RTAPI_GROUP_MEMBER_LEFT, RTAPI_GROUP_MEMBER_UPDATE,
    },
    wnd_proc::register_wnd_proc,
    AddonFlags, UpdateProvider,
};
#[cfg(feature = "goggles")]
use crate::space::goggles;

type Revertible = Box<dyn FnOnce() + Send + 'static>;

// https://github.com/kellpossible/cargo-i18n/blob/95634c35eb68643d4a08ff4cd17406645e428576/i18n-embed/examples/library-fluent/src/lib.rs
#[derive(RustEmbed)]
#[folder = "i18n/"]
pub struct LocalizationsEmbed;

pub static LOCALIZATIONS: LazyLock<RustEmbedNotifyAssets<LocalizationsEmbed>> =
    LazyLock::new(|| {
        RustEmbedNotifyAssets::new(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("i18n/"),
        )
    });

static LANGUAGE_LOADER: LazyLock<FluentLanguageLoader> = LazyLock::new(|| {
    let loader: FluentLanguageLoader = fluent_language_loader!();
    loader
        .load_available_languages(&*LOCALIZATIONS)
        .expect("Error while loading fallback language");
    loader.set_use_isolating(false);

    loader
});

#[macro_export]
macro_rules! fl {
    ($message_id:literal) => {{
        i18n_embed_fl::fl!($crate::LANGUAGE_LOADER, $message_id)
    }};

    ($message_id:literal, $($args:expr),*) => {{
        i18n_embed_fl::fl!($crate::LANGUAGE_LOADER, $message_id, $($args), *)
    }};
}

pub fn localizer() -> DefaultLocalizer<'static> {
    DefaultLocalizer::new(&*LANGUAGE_LOADER, &*LOCALIZATIONS)
}

pub mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));

    pub const IS_TAGGED_VERSION: bool = check_is_release();

    /// Official tagged release build
    pub fn is_release() -> bool {
        #[allow(unreachable_patterns)]
        match IS_TAGGED_VERSION {
            // never allow debug builds to be marked as a release
            #[cfg(debug_assertions)]
            true => false,
            true if git_release() == Some(crate::exports::runtime::CRATE_VERSION) =>
                true,
            _ => false,
        }
    }

    /// Ok(tag) or Err(branch)
    pub fn git_ref_name() -> Result<&'static str, &'static str> {
        match git_tag_name() {
            Some(tag) => Ok(tag),
            None => Err(git_branch_name()
                .or(GIT_HEAD_REF)
                .unwrap_or("HEAD")
            ),
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

    use crate::exports::runtime::update::{GIT_REF_BRANCH_PREFIX, GIT_REF_RELEASE_PREFIX, GIT_REF_TAG_PREFIX};
    const TAG_STR: [u8; 3] = *b"tag";
    const fn check_is_release() -> bool {
        let head = match GIT_HEAD_REF {
            Some(head) if head.len() >= GIT_REF_RELEASE_PREFIX.len() => head,
            _ => return false,
        }.as_bytes();
        let refs = "refs/".len();
        let tag = refs + "tags/".len();
        match [head[refs], head[refs + 1], head[refs + 2]] {
            self::TAG_STR if head[tag] == b'v' => (),
            _ => return false,
        }

        let prefix_matches = has_prefix(head, GIT_REF_RELEASE_PREFIX.len(), crate::exports::runtime::CRATE_VERSION.as_bytes(), 0);

        match prefix_matches {
            false => panic!("release version mismatch"),
            true => true,
        }
    }
    const fn has_prefix(s: &[u8], off: usize, prefix: &[u8], poff: usize) -> bool {
        if s.len() <= off || prefix.len() < poff {
            return false
        } else if prefix.len() == poff {
            return true
        }

        match s[off] == prefix[poff] {
            false => false,
            true => has_prefix(s, off + 1, prefix, poff + 1),
        }
    }
}

static TEXTURES: LazyLock<rt::TextureLoader> = LazyLock::new(|| rt::TextureLoader::new());
static CONTROLLER_SENDER: RwLock<Option<Sender<ControllerEvent>>> = RwLock::new(None);
static RENDER_SENDER: RwLock<Option<Sender<RenderEvent>>> = RwLock::new(None);
#[cfg(feature = "extension-nexus")]
static RENDER_CALLBACK: Mutex<Option<Revertible>> = Mutex::new(None);
static ACCOUNT_NAME_CELL: OnceLock<String> = OnceLock::new();

#[cfg(feature = "space")]
static SPACE_SENDER: RwLock<Option<Sender<SpaceEvent>>> = RwLock::new(None);

static CONTROLLER_THREAD: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);

#[cfg(feature = "extension-nexus")]
nexus::export! {
    name: "TaimiHUD",
    signature: exports::nexus::SIG,
    load: exports::nexus::cb_load,
    unload: exports::nexus::cb_unload,
    flags: AddonFlags::None,
    provider: if built_info::IS_TAGGED_VERSION { UpdateProvider::GitHub } else { UpdateProvider::Manual },
    update_link: exports::gh_repo_url!(),
}

#[cfg(feature = "extension-arcdps-codegen")]
arcdps::export! {
    name: "TaimiHUD",
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
    raw_extras_init: exports::arcdps::unofficial_extras::extras_init_raw,
    /*raw_extras_init: exports::arcdps::cb::extras_init,
    extras_language_changed: exports::arcdps::cb::extras_language,
    extras_keybind_changed: exports::arcdps::cb::extras_keybind,
    extras_squad_update: exports::arcdps::cb::extras_squad_update,*/
}

static RENDER_STATE: Mutex<Option<RenderState>> = Mutex::new(None);
static RENDER_UNLOAD: Condvar = Condvar::new();

static SOURCES: OnceLock<Arc<RwLock<SourcesFile>>> = OnceLock::new();
static SETTINGS: OnceLock<SettingsLock> = OnceLock::new();
#[cfg(feature = "space")]
static ENGINE: Mutex<Option<Result<Engine, ()>>> = Mutex::new(None);
#[cfg(feature = "space")]
pub fn engine_mut<R, F: FnOnce(&mut Engine) -> R>(f: F) -> Option<R> {
    if !RenderState::is_render_thread() || !RenderState::is_running() {
        return None
    }

    if let Ok(Some(Ok(engine))) = ENGINE.lock().as_mut().map(|e| &mut **e) {
        Some(f(engine))
    } else {
        None
    }
}
#[cfg(feature = "space")]
pub fn engine_ref<R, F: FnOnce(&Engine) -> R>(f: F) -> Option<R> {
    //engine_mut(|e| f(e))
    if let Ok(Some(Ok(engine))) = ENGINE.try_lock().as_ref().map(|e| &**e) {
        Some(f(engine))
    } else {
        None
    }
}

pub const WINDOW_PRIMARY: &'static str = "primary";
pub const WINDOW_TIMERS: &'static str = "timers";
pub const WINDOW_MARKERS: &'static str = "markers";
pub const WINDOW_PATHING: &'static str = "pathing";

fn marker_icon_data(marker_type: MarkerType) -> Option<Vec<u8>> {
    let arrow = include_bytes!("../icons/markers/cmdrArrow.png");
    let circle = include_bytes!("../icons/markers/cmdrCircle.png");
    let cross = include_bytes!("../icons/markers/cmdrCross.png");
    let heart = include_bytes!("../icons/markers/cmdrHeart.png");
    let spiral = include_bytes!("../icons/markers/cmdrSpiral.png");
    let square = include_bytes!("../icons/markers/cmdrSquare.png");
    let star = include_bytes!("../icons/markers/cmdrStar.png");
    let triangle = include_bytes!("../icons/markers/cmdrTriangle.png");
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
    let _ = rt::log::TaimiLog::setup();

    // XXX: could consider calling this from a DllMain (or CRT TLS hook fn?),
    // but defining explicit entry points is kinder anyway...
    // but contention over who "owns" the global panic hook or logger will only
    // ever matter if we switch to dynamic linking std anyway, so...
}

fn init() -> Result<(), &'static str> {
    crate_init();

    let mut loaded = match rt::LOADER_LOCK.lock() {
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
    // Say hi to the world :o
    let name = rt::CRATE_NAME;
    let version = rt::CRATE_VERSION;
    let authors = env!("CARGO_PKG_AUTHORS");
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
        (Err(branch), None) =>
            log::info!("Development build of {branch}"),
    }

    // Set up the thread
    let addon_dir = &*ADDON_DIR;

    rt::reload_language()?;

    #[cfg(feature = "texture-loader")]
    if let Err(e) = TEXTURES.setup() {
            if !rt::nexus_available() {
                return Err(e)
            }
            log::error!("{e}");
    } else {
        if let Err(e) = TEXTURES.wait_for_startup() {
            log::error!("{e}");
            if !rt::nexus_available() {
                return Err("texture loader didn't start")
            }
        }
    }

    let (controller_sender, controller_receiver) = channel::<ControllerEvent>(32);
    let (render_sender, render_receiver) = channel::<RenderEvent>(32);

    let controller_handler = {
        let render_sender = render_sender.clone();
        thread::spawn(move || Controller::load(controller_receiver, render_sender, addon_dir.to_owned()))
    };

    // muh queues
    *CONTROLLER_THREAD.lock().unwrap() = Some(controller_handler);
    *CONTROLLER_SENDER.write().unwrap() = Some(controller_sender);

    *RENDER_STATE.lock().unwrap() = Some(RenderState::new(render_receiver));
    *RENDER_SENDER.write().unwrap() = Some(render_sender);

    *loaded = true;
    Ok(())
}

#[cfg(feature = "extension-nexus")]
fn load_nexus() {
    // Rendering setup
    let taimi_window = render!(|ui| {
        RenderState::render_ui(ui);
    });
    let render_callback = register_render(RenderType::Render, taimi_window);
    *RENDER_CALLBACK.lock().unwrap() = Some(Box::new(render_callback.into_inner()));

    #[cfg(feature = "space")]
    let space_render = render!(|ui| render_space(ui));
    #[cfg(feature = "space")]
    register_render(RenderType::Render, space_render).revert_on_unload();

    register_wnd_proc(exports::nexus::wnd).revert_on_unload();

    // Handle window toggling with keybind and button
    let main_window_keybind_handler = keybind_handler!(|_id, is_release| {
        if !is_release {
            control_window(WINDOW_PRIMARY, None);
        }
    });

    register_keybind_with_string(
        fl!("primary-window-toggle"),
        main_window_keybind_handler,
        "ALT+SHIFT+M",
    )
    .revert_on_unload();

    // Handle window toggling with keybind and button
    #[cfg(feature = "markers")]
    let marker_window_keybind_handler = keybind_handler!(|_id, is_release| {
        if !is_release {
            control_window(WINDOW_MARKERS, None);
        }
    });

    #[cfg(feature = "markers")]
    register_keybind_with_string(
        fl!("marker-window-toggle"),
        marker_window_keybind_handler,
        "ALT+SHIFT+L",
    )
    .revert_on_unload();

    // Handle window toggling with keybind and button
    let timer_window_keybind_handler = keybind_handler!(|_id, is_release| {
        if !is_release {
            control_window(WINDOW_TIMERS, None);
        }
    });

    register_keybind_with_string(
        fl!("timer-window-toggle"),
        timer_window_keybind_handler,
        "ALT+SHIFT+K",
    )
    .revert_on_unload();

    // Handle window toggling with keybind and button
    #[cfg(feature = "space")]
    let pathing_window_keybind_handler = keybind_handler!(|_id, is_release| {
        if !is_release {
            control_window(WINDOW_PATHING, None);
        }
    });

    #[cfg(feature = "space")]
    register_keybind_with_string(
        fl!("pathing-window-toggle"),
        pathing_window_keybind_handler,
        "ALT+SHIFT+N",
    )
    .revert_on_unload();

    let pathing_render_keybind_handler = keybind_handler!(|_id, is_release| {
        if !is_release {
            Engine::try_send(SpaceEvent::PathingToggle);
        }
    });

    register_keybind_with_string(
        fl!("pathing-render-toggle"),
        pathing_render_keybind_handler,
        "ALT+SHIFT+N",
    )
    .revert_on_unload();

    let event_trigger_keybind_handler = keybind_handler!(|id, is_release| {
        Controller::try_send(ControllerEvent::TimerKeyTrigger(id.to_string(), is_release));
    });

    for i in 0..5 {
        register_keybind_with_string(
            fl!("timer-key-trigger", id = format!("{}", i)),
            event_trigger_keybind_handler,
            "",
        )
        .revert_on_unload();
    }

    // Disused currently, icon loading for quick access
    /*
    load_texture_from_file("Taimi_ICON", addon_dir.join("icon.png"), Some(receive_texture));
    load_texture_from_file(
        "Taimi_ICON_HOVER",
        addon_dir.join("icon_hover.png"),
        Some(receive_texture),
    );
    */

    let taimi_icon = include_bytes!("../icons/taimi.png");
    let taimi_hover_icon = include_bytes!("../icons/taimi-hover.png");
    let markers_icon = include_bytes!("../icons/markers.png");
    let markers_hover_icon = include_bytes!("../icons/markers-hover.png");
    let timers_icon = include_bytes!("../icons/timers.png");
    let timers_hover_icon = include_bytes!("../icons/timers-hover.png");
    let pathing_icon = include_bytes!("../icons/pathing.png");
    let pathing_hover_icon = include_bytes!("../icons/pathing-hover.png");
    let pathing_toggle_icon = include_bytes!("../icons/pathing-toggle.png");
    let pathing_toggle_hover_icon = include_bytes!("../icons/pathing-toggle-hover.png");

    let receive_texture =
        texture_receive!(|id: &str, _texture: Option<&NexusTexture>| log::info!("texture {id} loaded"));

    load_texture_from_memory("TAIMI_ICON", taimi_icon, Some(receive_texture));
    load_texture_from_memory("TAIMI_ICON_HOVER", taimi_hover_icon, Some(receive_texture));
    load_texture_from_memory("TAIMI_MARKERS_ICON", markers_icon, Some(receive_texture));
    load_texture_from_memory("TAIMI_MARKERS_ICON_HOVER", markers_hover_icon, Some(receive_texture));
    load_texture_from_memory("TAIMI_TIMERS_ICON", timers_icon, Some(receive_texture));
    load_texture_from_memory("TAIMI_TIMERS_ICON_HOVER", timers_hover_icon, Some(receive_texture));
    load_texture_from_memory("TAIMI_PATHING_ICON", pathing_icon, Some(receive_texture));
    load_texture_from_memory("TAIMI_PATHING_ICON_HOVER", pathing_hover_icon, Some(receive_texture));
    load_texture_from_memory("TAIMI_PATHING_RENDER_ICON", pathing_toggle_icon, Some(receive_texture));
    load_texture_from_memory("TAIMI_PATHING_RENDER_ICON_HOVER", pathing_toggle_hover_icon, Some(receive_texture));

    let same_identifier = "TAIMI_BUTTON";

    add_quick_access(
        same_identifier,
        "TAIMI_ICON",
        "TAIMI_ICON_HOVER",
        fl!("primary-window-toggle"),
        fl!("primary-window-toggle-text"),
    )
    .revert_on_unload();
    add_quick_access(
        "TAIMI_PATHING_BUTTON",
        "TAIMI_PATHING_ICON",
        "TAIMI_PATHING_ICON_HOVER",
        fl!("pathing-window-toggle"),
        fl!("pathing-window-toggle"),
    )
    .revert_on_unload();
    add_quick_access(
        "TAIMI_PATHING_RENDER_BUTTON",
        "TAIMI_PATHING_RENDER_ICON",
        "TAIMI_PATHING_RENDER_ICON_HOVER",
        fl!("pathing-render-toggle"),
        fl!("pathing-render-toggle"),
    )
    .revert_on_unload();
    add_quick_access(
        "TAIMI_TIMER_BUTTON",
        "TAIMI_TIMERS_ICON",
        "TAIMI_TIMERS_ICON_HOVER",
        fl!("timer-window-toggle"),
        fl!("timer-window-toggle"),
    )
    .revert_on_unload();
    add_quick_access(
        "TAIMI_MARKERS_BUTTON",
        "TAIMI_MARKERS_ICON",
        "TAIMI_MARKERS_ICON_HOVER",
        fl!("marker-window-toggle"),
        fl!("marker-window-toggle"),
    )
    .revert_on_unload();

    add_quick_access_context_menu(
        "TAIMI_MENU",
        Some(same_identifier), // maybe some day
        //None::<&str>,
        render!(|ui| {
            if ui.button(fl!("timer-window")) {
                control_window(WINDOW_TIMERS, None);
            }
            #[cfg(feature = "space")]
            if ui.button(fl!("pathing-render-toggle")) {
                Engine::try_send(SpaceEvent::PathingToggle);
            }
            #[cfg(feature = "space")]
            if ui.button(fl!("pathing-window")) {
                control_window(WINDOW_PATHING, None);
            }
            #[cfg(feature = "markers")]
            if ui.button(fl!("marker-window")) {
                control_window(WINDOW_MARKERS, None);
            }
            if ui.button(fl!("primary-window")) {
                control_window(WINDOW_PRIMARY, None);
            }
        }),
    )
    .revert_on_unload();

    ACCOUNT_NAME
        .subscribe(event_consume!(<c_char> |name| {
            if let Some(name) = name {
                let name = unsafe {CStr::from_ptr(name as *const c_char)};
                receive_account_name(name.to_string_lossy());
            }
        }))
        .revert_on_unload();

    let combat_callback = event_consume!(|cdata: Option<&CombatData>| {
        if let Some(combat_data) = cdata {
            receive_evtc_local(combat_data);
        }
    });
    COMBAT_LOCAL.subscribe(combat_callback).revert_on_unload();

    // MumbleLink Identity
    MUMBLE_IDENTITY_UPDATED
        .subscribe(event_consume!(<MumbleIdentityUpdate> |mumble_identity| {
            if let Some(mumble_identity) = mumble_identity {
                receive_mumble_identity(mumble_identity.clone());
            }
        }))
        .revert_on_unload();

    RTAPI_GROUP_MEMBER_LEFT.subscribe(
        event_consume!(
            <GroupMember> | group_member | {
                if let Some(group_member) = group_member {
                    receive_group_update(SquadState::Left, group_member);
                }
            }
        )
    ).revert_on_unload();

    RTAPI_GROUP_MEMBER_JOINED.subscribe(
        event_consume!(
            <GroupMember> | group_member | {
                if let Some(group_member) = group_member {
                    receive_group_update(SquadState::Joined, group_member);
                }
            }
        )
    ).revert_on_unload();

    RTAPI_GROUP_MEMBER_UPDATE.subscribe(
        event_consume!(
            <GroupMember> | group_member | {
                if let Some(group_member) = group_member {
                    receive_group_update(SquadState::Update, group_member);
                }
            }
        )
    ).revert_on_unload();

    EXTRAS_SQUAD_UPDATE.subscribe(
        event_consume!(
            <SquadUpdate> | update | {
                if let Some(update) = update {
                    receive_squad_update(update.iter().map(|p| unsafe {
                        // convert reference from disjoint arcdps-rs crate versions
                        mem::transmute::<_, &UserInfo>(p)
                    }));
                }
            }
        )
    ).revert_on_unload();

    pub const EV_LANGUAGE_CHANGED: Event<()> = unsafe { Event::new("EV_LANGUAGE_CHANGED") };

    // I don't want to store the localization data in either Nexus or communicate it with Nexus,
    // because this would mean entirely being beholden to Nexus as the addon's loader for the
    // rest of all time.
    EV_LANGUAGE_CHANGED
        .subscribe(event_consume!(
            <()> |_| {
                let res = rt::reload_language();
                if let Err(e) = res {
                    log::warn!("failed to load language: {e}");
                }
            }
        ))
        .revert_on_unload();

    WINDOW_RESIZED.subscribe(event_consume!(<()> |_| {
        reload_render(true);
    })).revert_on_unload();
}

#[cfg(feature = "extension-arcdps")]
fn load_arcdps() -> Result<(), &'static str> {
    Ok(())
}

pub const LANGUAGES_GAME: [Language; 5] = [
    Language::English,
    Language::French,
    Language::German ,
    Language::Spanish,
    Language::Chinese,
];
pub const LANGUAGES_EXTRA: [&'static str; 5] = [
    "cz",
    "it",
    "pl",
    "pt-br",
    "ru",
];

pub fn game_language_id(lang: Language) -> &'static str {
    match lang {
        Language::English => "en",
        Language::French => "fr",
        Language::German => "de",
        Language::Spanish => "es",
        Language::Chinese => "cn",
    }
}

fn load_language(detected_language: &str) -> rt::RuntimeResult {
    let detected_language_identifier: LanguageIdentifier = detected_language
        .parse()
        .map_err(|_| "Cannot parse detected language")?;
    let get_language = vec![detected_language_identifier];
    i18n_embed::select(&*LANGUAGE_LOADER, &*LOCALIZATIONS, get_language.as_slice())
        .map_err(|_| "Couldn't load language!")?;
    (&*LANGUAGE_LOADER).set_use_isolating(false);
    Ok(())
}

pub const ADDON_DIR: rt::AddonDir = rt::AddonDir;
pub static TIMERS_DIR: LazyLock<PathBuf> =
    LazyLock::new(|| ADDON_DIR.join("timers"));

fn control_window(window: impl Into<String>, state: Option<bool>) {
    let window = window.into();
    let event = ControllerEvent::WindowState(window, state);
    Controller::try_send(event);
}

fn receive_account_name<N: AsRef<str> + Into<String>>(account_name: N) {
    let account_name_ref = account_name.as_ref();
    let name = match account_name_ref.strip_prefix(":") {
        Some(name) => name,
        None => account_name_ref,
    };
    if name.is_empty() {
        return
    }
    match ACCOUNT_NAME_CELL.get() {
        // ignore duplicates
        Some(prev) if prev == name =>
            return,
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
                log::error!("Account name {name:?} inconsistent with previously recorded value {:?}", prev.map(|s| &s[..]).unwrap_or(""))
            }
        },
    }
}

fn receive_mumble_identity(id: MumbleIdentityUpdate) {
    Controller::try_send(ControllerEvent::MumbleIdentityUpdated(id));
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

    let event = ControllerEvent::CombatEvent {
        src,
        evt,
    };
    Controller::try_send(event);
}

fn receive_group_update(state: SquadState, group_member: &GroupMember) {
    let group_member: GroupMemberOwned = group_member.into();
    let event = ControllerEvent::RTAPISquadUpdate(state, group_member);
    Controller::try_send(event);
}

fn receive_squad_update<'u>(update: impl IntoIterator<Item = &'u UserInfo>) {
    let update: Vec<_> = update.into_iter()
        .map(|x| unsafe { ptr::read(x) }.into())
        .collect();
    let event = ControllerEvent::ExtrasSquadUpdate(update);
    Controller::try_send(event);
}

fn process_textures() {
    #[cfg(feature = "texture-loader")]
    let res = TEXTURES.try_responses(|mut responses| -> anyhow::Result<()> {
        use {
            anyhow::{anyhow, Context},
            rt::textures::{TextureResponse, Texture},
            tokio::sync::mpsc::error::TryRecvError,
        };
        let mut device = None;

        loop {
            let response = match responses.try_recv() {
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => Err(anyhow!("texture loader shut down?")),
                Ok(response) => Ok(response),
            }?;
            match response {
                TextureResponse::Decoded { key, format, pixels, stride, dimensions } => {
                    let device = match &mut device {
                        Some(d) => d,
                        device => {
                            let d3d11 = rt::d3d11_device()
                                .context("d3d11 device required to load textures")?;
                            device.insert(d3d11)
                        },
                    };
                    let texture = unsafe {
                        Texture::new_raw(device, &pixels, dimensions, stride, format)
                    };
                    TEXTURES.report_load(key, texture);
                },
                TextureResponse::DecodeFailed { key, error } => {
                    log::error!("texture {key} failed to decode: {error}");
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
    }).and_then(|res| res.transpose());
    #[cfg(feature = "texture-loader")]
    match res {
        Err(e) => {
            log::error!("texture processing error: {e}");
        },
        Ok(..) => (),
    }
}

fn texture_schedule_bytes<K, B>(key: K, bytes: B) where
    K: Into<String>,
    B: Into<Vec<u8>>,
{
    let event = ControllerEvent::LoadTextureIntegrated(
        key.into(),
        bytes.into(),
    );
    Controller::try_send(event);
}

fn texture_schedule_path<R, P>(rel: R, path: P) where
    R: Into<RelativePathBuf>,
    P: Into<PathBuf>,
{
    let event = ControllerEvent::LoadTexture(
        rel.into(),
        path.into(),
    );
    Controller::try_send(event);
}

#[cfg(feature = "space")]
fn render_space(ui: &nexus::imgui::Ui) {
    let enabled = SETTINGS.get()
        .and_then(|settings| settings.try_read().ok())
        .map(|settings| settings.enable_katrender)
        .unwrap_or(false);
    if !enabled || !RenderState::is_running() {
        return
    }
    let mut engine = match ENGINE.try_lock() {
        // if early game loading or charsel, delay init
        Ok(e) if e.is_none() && !rt::is_ingame().unwrap_or(false) => return,
        Ok(e) => e,
        _ => return,
    };
    let engine = engine.get_or_insert_with(|| {
            let (space_sender, space_receiver) = channel::<SpaceEvent>(32);
            *SPACE_SENDER.write().unwrap() = Some(space_sender);
            let drawstate_inner = Engine::initialise(ui, space_receiver);
            if let Err(error) = &drawstate_inner {
                log::error!("DrawState setup failed: {error:?}");
            };
            drawstate_inner.map_err(drop)
    });
    let ds = match engine.as_mut() {
        Ok(e) => e,
        Err(..) => return,
    };
                #[cfg(feature = "goggles")]
                if goggles::has_classification(goggles::LensClass::Space) == Some(false) {
                    goggles::classify_space_lens(ds);
                }
                if let Err(error) = ds.render(ui) {
                    log::error!("Engine error: {error}");
                }
}

fn notify_quit() {
    // if !RenderState::is_running() { return }

    log::info!("Preparing for game exit");

    let mut controller_sender = CONTROLLER_SENDER.write().unwrap();
    let controller_quit = controller_sender.as_ref()
        .map(|sender| sender.try_send(ControllerEvent::Quit));
    if let Some(Ok(())) = controller_quit {
        *controller_sender = None;
    }

    TEXTURES.quit();

    #[cfg(feature = "goggles")]
    if let Err(e) = goggles::shutdown() {
        log::error!("Goggles shutdown failed: {e}");
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
            let _ = sender.try_send(RenderEvent::Quit);
        }
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

    log::info!("Unloading addon");

    TEXTURES.quit();

    #[cfg(feature = "goggles")]
    if let Err(e) = goggles::shutdown() {
        log::error!("Goggles shutdown failed: {e}");
    }

    let controller_handle = CONTROLLER_THREAD.lock().unwrap().take();
    let controller_quit = CONTROLLER_SENDER.write().unwrap().take()
        .map(|sender| sender.try_send(ControllerEvent::Quit));

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
            _ => render_sender.as_ref().map(|sender| sender.try_send(RenderEvent::Quit)),
        };
        let _ = render_sender.take();

        match render_quit {
            _ if render_state.is_none() => {
                // it's already gone, nothing more to do here
                false
            },
            Some(Ok(())) => {
                let unload_timeout = match () {
                    #[cfg(feature = "space")]
                    () if _space.is_some() =>
                        // give it time to do more shutdown if needed...
                        Duration::from_millis(1500),
                    _ =>
                        Duration::from_millis(67),
                };
                let timeout = RENDER_UNLOAD.wait_timeout_while(render_state, unload_timeout, |state| state.is_some());
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

    if let Err(e) = TEXTURES.wait_for_shutdown() {
        log::error!("failed to shut down texture loader: {e}");
    }

    match controller_quit {
        Some(Ok(())) | None => match controller_handle {
            Some(handle) => {
                log::info!("Waiting for controller shutdown...");
                if let Err(e) = handle.join() {
                    log_any_error("controller thread", &e);
                }
            },
            None => {
                log::warn!("Controller unavailable?");
            },
        },
        Some(Err(..)) => {
            log::warn!("Failed to signal controller quit");
        },
    }

    #[cfg(feature = "extension-nexus")]
    if let Some(revert_render) = RENDER_CALLBACK.lock().unwrap().take() {
        revert_render();
    }

    if confirm_render_unload {
        unload_render_background();
    }

    *loaded = false;

    #[cfg(not(debug_assertions))] {
        drop(panic::take_hook());
    }

    log::debug!("Unload complete");
}

fn unload_render() {
    log::info!("Renderer unloading");
    debug_assert!(RenderState::is_render_thread());

    TEXTURES.cleanup(true);

    #[cfg(feature = "space")]
    if let Some(Ok(mut engine)) = ENGINE.lock().unwrap().take() {
        log::debug!("unloading space engine");
        engine.cleanup();
    }

    log::debug!("render unload complete");
    RENDER_UNLOAD.notify_all();
}

/// A limited form of [unload_render()] that should try its best,
/// but isn't able to touch render TLS or single-threaded interfaces
fn unload_render_background() {
    log::warn!("Unloading render state from a background thread");

    #[cfg(feature = "space")]
    if let Some(Ok(engine)) = ENGINE.lock().unwrap().take() {
        log::debug!("skipping engine drop()");
        mem::forget(engine);
    }

    let _state = RENDER_STATE.lock().unwrap().take();

    TEXTURES.cleanup(false);
    RENDER_UNLOAD.notify_all();
}

fn reload_render(superficial: bool) {
    log::info!("{} renderer...", if superficial { "reloading" } else { "reinit" });

    #[cfg(feature = "goggles")]
    let _ = goggles::shutdown();

    #[cfg(feature = "space")]
    if let Some(Ok(mut engine)) = ENGINE.lock().unwrap().take() {
        log::debug!("reloading space engine");
        if RenderState::is_render_thread() {
            engine.cleanup();
        } else {
            mem::forget(engine);
            log::warn!("TODO: reloading outside of render thread");
        }
        // ... and let it reinit on its own next render frame
    }

    if !superficial {
        // probably no need to reload textures/etc unless we've lost the entire d3d device or something?
        TEXTURES.cleanup(RenderState::is_render_thread());
    }
}

fn with_any_error<R, F: FnOnce(&str) -> R>(e: &dyn std::any::Any, f: F) -> R {
    let msg = if let Some(m) = e.downcast_ref::<&str>() {
        *m
    } else if let Some(m) = e.downcast_ref::<String>() {
        &m[..]
    } else {
        "unknown error"
    };
    f(msg)
}

fn log_any_error(name: &str, e: &dyn std::any::Any) {
    with_any_error(e, move |e| log::error!("{name} panicked: {e}"))
}

fn panic_hook(info: &panic::PanicHookInfo) {
    log_any_error(rt::NAME, info.payload());
    if let Some(location) = info.location() {
        log::error!("Panic occurred in {} at {location}", rt::CRATE_NAME);
    }
}

fn setup_panic_hook() {
    panic::set_hook(Box::new(panic_hook))
}
