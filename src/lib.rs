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
use space::{engine::SpaceEvent, Engine};
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
        texture::Texture as NexusTexture,
    },
    relative_path::RelativePathBuf,
    rust_embed::RustEmbed,
    settings::SourcesFile,
    std::{
        collections::HashMap,
        ffi::{c_char, CStr},
        path::PathBuf,
        ptr,
        sync::{Arc, LazyLock, Mutex, OnceLock, RwLock},
        thread::{self, JoinHandle},
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
    },
    gui::{register_render, render, RenderType},
    keybind::{keybind_handler, register_keybind_with_string},
    quick_access::{add_quick_access, add_quick_access_context_menu},
    rtapi::{
        event::{
            RTAPI_GROUP_MEMBER_JOINED, RTAPI_GROUP_MEMBER_LEFT, RTAPI_GROUP_MEMBER_UPDATE,
        },
    },
    AddonFlags, UpdateProvider,
};

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
}

#[cfg(feature = "texture-loader")]
static TEXTURES: OnceLock<RwLock<HashMap<PathBuf, Arc<resources::Texture>>>> = OnceLock::new();
static IMGUI_TEXTURES: OnceLock<RwLock<HashMap<String, Arc<NexusTexture>>>> = OnceLock::new();
static CONTROLLER_SENDER: OnceLock<Sender<ControllerEvent>> = OnceLock::new();
static RENDER_SENDER: OnceLock<Sender<RenderEvent>> = OnceLock::new();
static ACCOUNT_NAME_CELL: OnceLock<String> = OnceLock::new();

#[cfg(feature = "space")]
static SPACE_SENDER: OnceLock<Sender<SpaceEvent>> = OnceLock::new();

static CONTROLLER_THREAD: OnceLock<JoinHandle<()>> = OnceLock::new();

#[cfg(feature = "extension-nexus")]
nexus::export! {
    name: "TaimiHUD",
    signature: exports::nexus::SIG,
    load: exports::nexus::cb_load,
    unload: exports::nexus::cb_unload,
    flags: AddonFlags::None,
    provider: UpdateProvider::GitHub,
    update_link: exports::gh_repo_url!(),
    log_filter: "debug"
}

#[cfg(feature = "extension-arcdps-codegen")]
arcdps::export! {
    name: "TaimiHUD",
    sig: exports::arcdps::SIG,
    init: exports::arcdps::cb_init,
    release: exports::arcdps::cb_release,
    imgui: exports::arcdps::cb_imgui,
    options_end: exports::arcdps::cb_options_end,
    wnd_filter: exports::arcdps::cb_wnd_filter,
    combat_local: exports::arcdps::cb_combat_local,
    update_url: exports::arcdps::cb_update_url,
    extras_init: exports::arcdps::cb_extras_init,
    extras_language_changed: exports::arcdps::cb_extras_language,
    extras_keybind_changed: exports::arcdps::cb_extras_keybind,
    extras_squad_update: exports::arcdps::cb_extras_squad_update,
}

static RENDER_STATE: OnceLock<Mutex<RenderState>> = OnceLock::new();

static SOURCES: OnceLock<Arc<RwLock<SourcesFile>>> = OnceLock::new();
static SETTINGS: OnceLock<SettingsLock> = OnceLock::new();
#[cfg(feature = "space")]
use std::cell::{Cell, RefCell};
#[cfg(feature = "space")]
thread_local! {
    static ENGINE_INITIALIZED: Cell<bool> = const { Cell::new(false) };
    static ENGINE: RefCell<Option<Engine>> = panic!("!");
}

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

fn init() -> Result<(), &'static str> {
    let _ = IMGUI_TEXTURES.set(RwLock::new(HashMap::new()));
    #[cfg(feature = "space")]
    let _ = TEXTURES.set(RwLock::new(HashMap::new()));
    // Say hi to the world :o
    let name = env!("CARGO_PKG_NAME");
    let authors = env!("CARGO_PKG_AUTHORS");
    log::info!("Loading {name} by {authors}");

    // Set up the thread
    let addon_dir = rt::addon_dir()?;

    rt::reload_language()?;

    let (controller_sender, controller_receiver) = channel::<ControllerEvent>(32);
    let (render_sender, render_receiver) = channel::<RenderEvent>(32);

    let _ = CONTROLLER_SENDER.set(controller_sender);
    let _ = RENDER_SENDER.set(render_sender.clone());

    let controller_handler =
        thread::spawn(|| Controller::load(controller_receiver, render_sender, addon_dir));

    // muh queues
    let _ = CONTROLLER_THREAD.set(controller_handler);
    let _ = RENDER_STATE.set(Mutex::new(RenderState::new(render_receiver)));

    Ok(())
}

#[cfg(feature = "extension-nexus")]
fn load_nexus() {
    init().expect("load failed");

    // Rendering setup
    let taimi_window = render!(|ui| render_overlay(ui));
    register_render(RenderType::Render, taimi_window).revert_on_unload();

    #[cfg(feature = "space")]
    let space_render = render!(|ui| render_space(ui));
    #[cfg(feature = "space")]
    register_render(RenderType::Render, space_render).revert_on_unload();

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
    let marker_window_keybind_handler = keybind_handler!(|_id, is_release| {
        if !is_release {
            let sender = CONTROLLER_SENDER.get().unwrap();
            let _ = sender.try_send(ControllerEvent::WindowState("markers".to_string(), None));
        }
    });

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
    let pathing_window_keybind_handler = keybind_handler!(|_id, is_release| {
        if !is_release {
            control_window(WINDOW_PATHING, None);
        }
    });

    register_keybind_with_string(
        fl!("pathing-window-toggle"),
        pathing_window_keybind_handler,
        "ALT+SHIFT+N",
    )
    .revert_on_unload();

    let event_trigger_keybind_handler = keybind_handler!(|id, is_release| {
        let sender = CONTROLLER_SENDER.get().unwrap();
        let _ = sender.try_send(ControllerEvent::TimerKeyTrigger(id.to_string(), is_release));
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
    let receive_texture =
        texture_receive!(|id: &str, _texture: Option<&Texture>| log::info!("texture {id} loaded"));
    load_texture_from_file("Taimi_ICON", addon_dir.join("icon.png"), Some(receive_texture));
    load_texture_from_file(
        "Taimi_ICON_HOVER",
        addon_dir.join("icon_hover.png"),
        Some(receive_texture),
    );
    */

    let same_identifier = "TAIMI_BUTTON";

    add_quick_access(
        same_identifier,
        "TAIMI_ICON",
        "TAIMI_ICON_HOVER",
        fl!("primary-window-toggle"),
        fl!("primary-window-toggle-text"),
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
                    receive_squad_update(update.iter());
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
}

#[cfg(feature = "extension-arcdps")]
fn load_arcdps() -> Result<(), &'static str> {
    init()?;

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

pub static ADDON_DIR: LazyLock<PathBuf> =
    LazyLock::new(|| rt::addon_dir()
        .unwrap_or_else(|_| PathBuf::from("Taimi"))
    );
pub static TIMERS_DIR: LazyLock<PathBuf> =
    LazyLock::new(|| ADDON_DIR.join("timers"));

const WINDOW_PRIMARY: &'static str = "primary";
const WINDOW_TIMERS: &'static str = "timers";
#[cfg(feature = "space")]
const WINDOW_PATHING: &'static str = "pathing";
#[cfg(feature = "markers")]
const WINDOW_MARKERS: &'static str = "markers";

fn control_window(window: impl Into<String>, state: Option<bool>) {
    let window = window.into();
    let sender = CONTROLLER_SENDER.get().unwrap();
    let event = ControllerEvent::WindowState(window, state);
    let _ = sender.try_send(event);
}

fn receive_account_name<N: AsRef<str> + Into<String>>(account_name: N) {
    let account_name_ref = account_name.as_ref();
    let name = match account_name_ref.strip_prefix(":") {
        Some(name) => name,
        None => account_name_ref,
    };
    match ACCOUNT_NAME_CELL.get() {
        // ignore duplicates
        Some(prev) if prev == name =>
            return,
        _ => (),
    }
    log::info!("Received account name: {name:?}");
    let name_owned = match account_name_ref.as_ptr() != name.as_ptr() {
        // if the prefix was stripped, reallocate
        true => name.into(),
        false => account_name.into(),
    };
    match ACCOUNT_NAME_CELL.set(name_owned) {
        Ok(_) => (),
        Err(name) if Some(&name) != ACCOUNT_NAME_CELL.get() => {
            log::error!("Account name inconsistent")
        },
        Err(..) => (),
    }
}

fn receive_mumble_identity(id: MumbleIdentityUpdate) {
    let sender = CONTROLLER_SENDER.get().unwrap();
    let event = ControllerEvent::MumbleIdentityUpdated(id);
    let _event_send = sender.try_send(event);
}

fn receive_evtc_local(combat_data: &CombatData) {
    let (evt, src) = match (combat_data.event(), combat_data.src()) {
        (Some(evt), Some(src)) => (evt, src),
        _ => return,
    };

    let sender = CONTROLLER_SENDER.get().unwrap();
    let src = AgentOwned::from(unsafe { ptr::read(src) });
    let event = ControllerEvent::CombatEvent {
        src,
        evt: evt.clone(),
    };
    let _event_send = sender.try_send(event);
}

fn receive_group_update(state: SquadState, group_member: &GroupMember) {
    let sender = CONTROLLER_SENDER.get().unwrap();
    let group_member: GroupMemberOwned = group_member.into();
    let event = ControllerEvent::RTAPISquadUpdate(state, group_member);
    let _event_send = sender.try_send(event);
}

fn receive_squad_update<'u>(update: impl IntoIterator<Item = &'u UserInfo>) {
    let update: Vec<_> = update.into_iter()
        .map(|x| unsafe { ptr::read(x) }.into())
        .collect();
    let sender = CONTROLLER_SENDER.get().unwrap();
    let event = ControllerEvent::ExtrasSquadUpdate(update);
    let _event_send = sender.try_send(event);
}

fn render_overlay(ui: &nexus::imgui::Ui) {
    let mut state = RenderState::lock();
    state.draw(ui);
    drop(state);
}

#[cfg(feature = "space")]
fn render_space(ui: &nexus::imgui::Ui) {
    if let Some(settings) = SETTINGS.get().and_then(|settings| settings.try_read().ok()) {
        if settings.enable_katrender {
            if !ENGINE_INITIALIZED.get() {
                let (space_sender, space_receiver) = channel::<SpaceEvent>(32);
                let _ = SPACE_SENDER.set(space_sender);
                let drawstate_inner = Engine::initialise(ui, space_receiver);
                if let Err(error) = &drawstate_inner {
                    log::error!("DrawState setup failed: {error:?}");
                };
                ENGINE.set(drawstate_inner.ok());
                ENGINE_INITIALIZED.set(true);
            }
            ENGINE.with_borrow_mut(|ds_op| {
                if let Some(ds) = ds_op {
                    if let Err(error) = ds.render(ui) {
                        log::error!("Engine error: {error}");
                    }
                }
            });
        }
    }
}

fn load_texture_bytes<K, B>(key: K, bytes: B) where
    K: AsRef<str> + Into<String>,
    B: AsRef<[u8]> + Into<Vec<u8>>,
{
    #[cfg(feature = "texture-loader")]
    match rt::d3d11_device() {
        Ok(Some(d3d11)) => {
            match resources::Texture::new_bytes(&d3d11, bytes.as_ref(), key.as_ref()) {
                Ok(texture) => {
                    let mut gooey_lock = IMGUI_TEXTURES.get().unwrap().write().unwrap();
                    if let Some(texture) = texture.to_nexus() {
                        gooey_lock.entry(key.into())
                            .or_insert(Arc::new(texture));
                    }
                    return
                },
                Err(e) => {
                    log::warn!(target:"texture-loader", "failed to load {}: {e}", key.as_ref());
                },
            }
        },
        Err(e) => {
            log::info!(target:"texture-loader", "D3D11 unavailable? {e}");
        },
        _ => (),
    }

    texture_schedule_bytes(key, bytes)
}

fn load_texture_path(rel: RelativePathBuf, path: PathBuf) {
    // TODO: if load fails, mark it in hashmap to avoid repeately attempting load
    // (regardless of load method, resources::texture or nexus or otherwise)

    #[cfg(feature = "texture-loader")]
    match rt::d3d11_device() {
        Ok(Some(d3d11)) => {
            if let Some(base) = path.parent() {
                let abs = rel.to_path(base);
                match resources::Texture::new_path(&d3d11, &abs) {
                    Ok(texture) => {
                        let mut gooey_lock = IMGUI_TEXTURES.get().unwrap().write().unwrap();
                        if let Some(texture) = texture.to_nexus() {
                            gooey_lock.entry(rel.into())
                                .or_insert(Arc::new(texture));
                        }
                        return
                    },
                    Err(e) => {
                        log::warn!(target:"texture-loader", "failed to load {abs:?}: {e}");
                    },
                }
            }
        },
        Err(e) => {
            log::info!(target:"texture-loader", "D3D11 unavailable? {e}");
        },
        _ => (),
    }

    texture_schedule_path(rel, path)
}

fn texture_schedule_bytes<K, B>(key: K, bytes: B) where
    K: AsRef<str> + Into<String>,
    B: AsRef<[u8]> + Into<Vec<u8>>,
{
    let sender = CONTROLLER_SENDER.get().unwrap();
    let event = ControllerEvent::LoadTextureIntegrated(
            key.into(),
            bytes.into(),
    );
    let _res = sender.try_send(event);
}

fn texture_schedule_path(rel: RelativePathBuf, path: PathBuf) {
    let sender = CONTROLLER_SENDER.get().unwrap();
    let event = ControllerEvent::LoadTexture(
            rel,
            path,
    );
    let _res = sender.try_send(event);
}

fn unload() {
    log::info!("Unloading addon");
    #[cfg(feature = "space")]
    let _ = ENGINE.set(None);
    #[cfg(feature = "space")]
    let _ = TEXTURES.set(Default::default());
    let _ = IMGUI_TEXTURES.set(Default::default());
    /*ENGINE.with_borrow_mut(|e| {
        //#[cfg(todo)]
        //e.cleanup();
    });*/
    let sender = CONTROLLER_SENDER.get().unwrap();
    let event_send = sender.try_send(ControllerEvent::Quit);
    drop(event_send);
}
