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
        gui::{register_render, RenderType},
        on_unload,
        rtapi::{
            event::{RTAPI_GROUP_MEMBER_JOINED, RTAPI_GROUP_MEMBER_LEFT, RTAPI_GROUP_MEMBER_UPDATE},
            GroupMember,
            GroupMemberOwned,
        },
        wnd_proc::register_wnd_proc,
        AddonFlags,
        UpdateProvider,
    },
    tokio::sync::watch,
};
use {
    crate::{
        controller::{
            markers::{MarkersController, MarkersEvent},
            Controller,
            ControllerEvent,
        },
        exports::runtime as rt,
        render::{machine::RenderMachine, RenderEvent, RenderState},
        settings::{state::BootstrapState, SettingsLock},
    },
    anyhow::Context,
    arcdps::{extras::UserInfo, AgentOwned, Language},
    controller::markers::SquadState,
    i18n_embed::{
        fluent::{fluent_language_loader, FluentLanguageLoader},
        DefaultLocalizer,
        LanguageLoader,
        RustEmbedNotifyAssets,
    },
    marker::format::MarkerType,
    nexus::event::{arc::CombatData, extras::SquadUpdate, MumbleIdentityUpdate},
    relative_path::RelativePathBuf,
    rust_embed::RustEmbed,
    settings::SourcesFile,
    std::{
        borrow::Cow,
        collections::BTreeSet,
        ffi::{c_char, CStr},
        mem,
        panic,
        path::PathBuf,
        ptr,
        slice,
        sync::{Arc, Condvar, LazyLock, Mutex, OnceLock, RwLock},
        thread::{self, JoinHandle},
        time::Duration,
    },
    tokio::sync::mpsc::{channel, Sender},
    unic_langid_impl::LanguageIdentifier,
};

#[cfg(feature = "space")]
use crate::space::engine::SpaceEvent;
#[cfg(feature = "goggles")]
use crate::space::goggles;

type Revertible = Box<dyn FnOnce() + Send + 'static>;

// https://github.com/kellpossible/cargo-i18n/blob/95634c35eb68643d4a08ff4cd17406645e428576/i18n-embed/examples/library-fluent/src/lib.rs
#[derive(RustEmbed)]
#[folder = "i18n/"]
pub struct LocalizationsEmbed;

pub static LOCALIZATIONS: LazyLock<RustEmbedNotifyAssets<LocalizationsEmbed>> =
    LazyLock::new(|| RustEmbedNotifyAssets::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("i18n/")));

static LANGUAGE_LOADER: LazyLock<FluentLanguageLoader> = LazyLock::new(|| {
    let assets = &*LOCALIZATIONS;
    let loader: FluentLanguageLoader = fluent_language_loader!();
    loader
        .load_available_languages(assets)
        .expect("Error while loading fallback language");
    let res =
        BootstrapState::read_with(
            |state| match state.language.as_ref().and_then(|l| l.parse().ok()) {
                Some(language) if loader.current_language() != language =>
                    i18n_embed::select(&loader, assets, slice::from_ref(&language))
                        .with_context(|| format!("Failed to select language {language}"))
                        .map(drop),
                _ => Ok(()),
            },
        );
    if let Err(e) = res {
        log::warn!(logger: rt::log::DeferredLogger::BEST_EFFORT, "{e:#}");
    }
    language_loader_setup(&loader);
    loader
});
fn language_loader_setup(loader: &FluentLanguageLoader) {
    loader.set_use_isolating(false);
    #[cfg(todo)]
    loader.with_bundles_mut(|b| {
        // might be needed for fluent 0.17..
        let res = b.add_builtins().context("Failed to add i18n/fluent builtins");
        if let Err(e) = res {
            log::warn!("{e:#}");
        }
    });
}

#[macro_export]
macro_rules! fl {
    ($message_id:literal) => {{
        i18n_embed_fl::fl!($crate::LANGUAGE_LOADER, $message_id)
    }};

    ($message_id:literal, $($args:expr),*) => {{
        i18n_embed_fl::fl!($crate::LANGUAGE_LOADER, $message_id, $($args), *)
    }};
}

static I18N_WARN_ONCE: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());
pub fn with_i18n<R, F>(message_id: &str, f: F) -> R
where
    F: FnOnce(Cow<str>) -> R,
{
    use {core::cell::RefCell, fluent_syntax::ast::PatternElement};

    // XXX: why does this not take an FnOnce...
    let mut f = RefCell::new(Some(f));
    let res = LANGUAGE_LOADER.with_fluent_message(message_id, |m| {
        let msg = m.value().and_then(|m| match &m.elements[..] {
            &[PatternElement::TextElement { value: one }] => Some(one),
            _ => None,
        })?;
        let f = f.try_borrow_mut().ok()?.take()?;
        Some(f(Cow::Borrowed(msg)))
    });
    match (res, f.get_mut().take()) {
        (Some(Some(r)), _) => r,
        (Some(None), Some(f)) => f(LANGUAGE_LOADER.get(message_id).into()),
        (None, Some(f)) => {
            let warn_once = match I18N_WARN_ONCE.try_lock() {
                Ok(once) if once.contains(message_id) => false,
                Ok(mut once) => {
                    once.insert(message_id.into());
                    true
                },
                Err(..) => false,
            };
            if warn_once {
                log::debug!("missing i18n message {message_id}");
            }
            f(Cow::Borrowed(message_id))
        },
        (_, None) => unreachable!("with_message calls once"),
    }
}
#[macro_export]
macro_rules! with_i18n {
    ($message_id:literal, $closure:expr) => {
        {
            'with_i18n_: {
                #![allow(unreachable_code)]
                let res = $crate::with_i18n($message_id, $closure);
                break 'with_i18n_ res;
                // still check ID at compile time...
                #[cfg(debug_assertions)]
                let _ = $crate::fl!($message_id);
            }
        }
    };
    ($message_id:expr, $closure:expr) => {
        {
            $crate::with_i18n($message_id, $closure)
        }
    };
}

pub fn localizer() -> DefaultLocalizer<'static> {
    DefaultLocalizer::new(&*LANGUAGE_LOADER, &*LOCALIZATIONS)
}

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
        Some(r) if r.len() == 1 && r.as_bytes()[0] == b'1' => true,
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
static CONTROLLER_SENDER: RwLock<Option<Sender<ControllerEvent>>> = RwLock::new(None);
#[cfg(feature = "extension-nexus")]
static QUICK_ACCESS_STATE: LazyLock<watch::Sender<TaimiControls>> =
    LazyLock::new(|| watch::Sender::new(TaimiControls::empty()));
static RENDER_SENDER: RwLock<Option<Sender<RenderEvent>>> = RwLock::new(None);
#[cfg(feature = "extension-nexus")]
static RENDER_CALLBACK: Mutex<Option<Revertible>> = Mutex::new(None);
#[cfg(all(feature = "extension-nexus", feature = "space"))]
static RENDER_CALLBACK_PRE: Mutex<Option<Revertible>> = Mutex::new(None);
static ACCOUNT_NAME_CELL: OnceLock<String> = OnceLock::new();

#[cfg(feature = "space")]
static SPACE_SENDER: RwLock<Option<Sender<SpaceEvent>>> = RwLock::new(None);

static CONTROLLER_THREAD: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);

#[cfg(feature = "extension-nexus-codegen")]
nexus::export! {
    name: exports::addon_title!(),
    signature: exports::nexus::SIG,
    load: exports::nexus::cb_load,
    unload: exports::nexus::cb_unload,
    flags: AddonFlags::None,
    provider: if built_info::IS_TAGGED_RELEASE_OR_RC { UpdateProvider::GitHub } else { UpdateProvider::Manual },
    update_link: exports::gh_repo_url!(),
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
    let addon_dir = &*ADDON_DIR;

    if let Err(e) = rt::reload_language() {
        log::debug!("No language detected at init: {e}");
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

    let (controller_sender, controller_receiver) = channel::<ControllerEvent>(64);
    let (render_sender, render_receiver) = channel::<RenderEvent>(48);

    let mut render_state = RENDER_STATE.lock().unwrap();

    let controller_handler = {
        let render_sender = render_sender.clone();
        thread::spawn(move || Controller::load(controller_receiver, render_sender, addon_dir.to_owned()))
    };

    // muh queues
    *CONTROLLER_THREAD.lock().unwrap() = Some(controller_handler);
    *CONTROLLER_SENDER.write().unwrap() = Some(controller_sender);

    *render_state = Some(RenderState::new(render_receiver));
    *RENDER_SENDER.write().unwrap() = Some(render_sender);

    log::logger().flush();

    *loaded = true;
    Ok(())
}

#[cfg(feature = "extension-nexus")]
fn load_nexus() {
    use crate::exports::nexus::{quick_access_remove_all, register_keybind, unregister_keybinds};

    // Rendering setup

    let taimi_window = nexus::gui::render!(|ui| {
        #[cfg(not(feature = "space"))]
        if !RenderState::pre_render() {
            RenderState::render_setup(ui);
        }
        RenderMachine::turn_ui_entry(ui);
        RenderState::render_ui(ui);
    });
    let render_callback = register_render(RenderType::Render, taimi_window);
    *RENDER_CALLBACK.lock().unwrap() = Some(Box::new(render_callback.into_inner()));

    #[cfg(feature = "space")]
    {
        extern "C-unwind" fn nexus_pre_render() {
            let render_ready = RenderState::pre_render();
            RenderMachine::turn_render_entry();
            if !render_ready {
                #[cfg(feature = "extension-nexus-codegen")]
                let ui = unsafe { nexus::ui() };
                RenderState::render_setup(ui);
            }
        }
        let render_callback_pre = register_render(RenderType::PreRender, nexus_pre_render);
        *RENDER_CALLBACK_PRE.lock().unwrap() = Some(Box::new(render_callback_pre.into_inner()));
    }

    let taimi_settings = nexus::gui::render!(|ui| {
        RenderState::render_options(ui);
    });
    register_render(RenderType::OptionsRender, taimi_settings).revert_on_unload();

    register_wnd_proc(exports::nexus::wnd).revert_on_unload();

    on_unload(unregister_keybinds);
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

    // Disused currently, icon loading for quick access
    /*
    load_texture_from_file("Taimi_ICON", addon_dir.join("icon.png"), Some(receive_texture));
    load_texture_from_file(
        "Taimi_ICON_HOVER",
        addon_dir.join("icon_hover.png"),
        Some(receive_texture),
    );
    */

    on_unload(quick_access_remove_all);

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
    #[cfg(feature = "markers")]
    MUMBLE_IDENTITY_UPDATED
        .subscribe(event_consume!(<MumbleIdentityUpdate> |mumble_identity| {
            if let Some(mumble_identity) = mumble_identity {
                MarkersController::receive_mumble_identity(mumble_identity);
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
                        rt::notify_game_language(*language)
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
                let res = rt::reload_language()
                    .map_err(anyhow::Error::msg)
                    .context("failed to load language");
                if let Err(e) = res {
                    log::warn!("{e:#}");
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

pub const LANGUAGES_GAME: [Language; 5] = [
    Language::English,
    Language::French,
    Language::German,
    Language::Spanish,
    Language::Chinese,
];
pub const LANGUAGES_EXTRA: [&'static str; 5] = ["cz", "it", "pl", "pt-br", "ru"];

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
    if LANGUAGE_LOADER.current_language().language.as_str() == detected_language {
        return Ok(())
    }

    let detected_language_identifier: LanguageIdentifier = detected_language
        .parse()
        .map_err(|_| "Cannot parse detected language")?;
    let get_language = vec![detected_language_identifier];
    // TODO: this may happen twice at startup and can be skipped if no change detected?
    i18n_embed::select(&*LANGUAGE_LOADER, &*LOCALIZATIONS, get_language.as_slice())
        .map_err(|_| "Couldn't load language!")?;
    language_loader_setup(&LANGUAGE_LOADER);
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
    K: Into<String>,
    B: Into<Vec<u8>>,
{
    let event = ControllerEvent::LoadTextureIntegrated(key.into(), bytes.into());
    Controller::try_send(event);
}

fn texture_schedule_path<R, P>(rel: R, path: P)
where
    R: Into<RelativePathBuf>,
    P: Into<PathBuf>,
{
    let event = ControllerEvent::LoadTexture(rel.into(), path.into());
    Controller::try_send(event);
}

fn notify_quit() {
    // if !RenderState::is_running() { return }

    log::info!("Preparing for game exit");
    rt::notify_shutdown();

    let mut controller_sender = CONTROLLER_SENDER.write().unwrap();
    let controller_quit = controller_sender
        .as_ref()
        .map(|sender| sender.try_send(ControllerEvent::Quit));
    if let Some(Ok(())) = controller_quit {
        *controller_sender = None;
    }

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
    if let Some(host) = BootstrapState::current_addon_host() {
        BootstrapState::write_with(|state| {
            state.latest_addon_host = Some(host);
        });
    }

    log::info!("Unloading addon");

    TEXTURES.quit();

    #[cfg(feature = "goggles")]
    if let Err(e) = goggles::shutdown().context("Goggles shutdown failed") {
        log::error!("{e:#}");
    }

    let controller_handle = CONTROLLER_THREAD.lock().unwrap().take();
    let controller_quit = CONTROLLER_SENDER
        .write()
        .unwrap()
        .take()
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
            _ => render_sender
                .as_ref()
                .map(|sender| sender.try_send(RenderEvent::Quit)),
        };
        let _ = render_sender.take();

        log::logger().flush();
        match render_quit {
            _ if rt::is_shutdown() || render_state.is_none() => {
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
        Some(Ok(())) | None => match controller_handle {
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
        Some(Err(..)) => {
            log::warn!("Failed to signal controller quit");
        },
    }

    #[cfg(feature = "extension-nexus")]
    if let Some(revert_render) = RENDER_CALLBACK.lock().unwrap().take() {
        revert_render();
    }
    #[cfg(all(feature = "extension-nexus", feature = "space"))]
    if let Some(revert_render) = RENDER_CALLBACK_PRE.lock().unwrap().take() {
        revert_render();
    }

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
    use std::sync::Arc;

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

fn panic_hook(info: &panic::PanicHookInfo) {
    log_any_error(rt::NAME, info.payload());
    if let Some(location) = info.location() {
        log::error!("Panic occurred in {} at {location}", rt::CRATE_NAME);
    }
}

fn setup_panic_hook() {
    panic::set_hook(Box::new(panic_hook))
}
