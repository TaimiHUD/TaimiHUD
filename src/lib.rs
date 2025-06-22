mod controller;
mod render;
mod settings;
mod timer;
mod util;

#[cfg(feature = "markers")]
mod marker;

#[cfg(feature = "space")]
mod space;

//use i18n_embed_fl::fl;
#[cfg(feature = "space")]
use {
    space::{engine::SpaceEvent, resources::Texture, Engine},
    std::{
        cell::RefCell,
        path::PathBuf,
        sync::atomic::{AtomicBool, Ordering},
    },
};
use {
    crate::{
        controller::{Controller, ControllerEvent},
        render::{RenderEvent, RenderState},
        settings::SettingsLock,
    },
    arcdps::{extras::UserInfoOwned, AgentOwned},
    controller::SquadState,
    i18n_embed::{
        fluent::{fluent_language_loader, FluentLanguageLoader},
        DefaultLocalizer, LanguageLoader, RustEmbedNotifyAssets,
    },
    marker::format::MarkerType,
    nexus::{
        event::{
            arc::{CombatData, ACCOUNT_NAME, COMBAT_LOCAL},
            event_consume,
            extras::{SquadUpdate, EXTRAS_SQUAD_UPDATE},
            Event, MumbleIdentityUpdate, MUMBLE_IDENTITY_UPDATED,
        }, gui::{register_render, render, RenderType}, keybind::{keybind_handler, register_keybind_with_string}, localization::translate, paths::get_addon_dir, quick_access::{add_quick_access, add_quick_access_context_menu}, rtapi::{
            event::{
                RTAPI_GROUP_MEMBER_JOINED, RTAPI_GROUP_MEMBER_LEFT, RTAPI_GROUP_MEMBER_UPDATE,
            },
            GroupMember, GroupMemberOwned,
        }, texture::{load_texture_from_memory, Texture as NexusTexture}, texture_receive, AddonFlags, UpdateProvider
    },
    rust_embed::RustEmbed,
    settings::SourcesFile,
    std::{
        collections::HashMap,
        ffi::{c_char, CStr},
        ptr,
        sync::{Arc, LazyLock, Mutex, OnceLock, RwLock},
        thread::{self, JoinHandle},
    },
    tokio::sync::mpsc::{channel, Sender},
    unic_langid_impl::LanguageIdentifier,
};

type Revertible = Box<dyn FnOnce() + Send + 'static>;

// https://github.com/kellpossible/cargo-i18n/blob/95634c35eb68643d4a08ff4cd17406645e428576/i18n-embed/examples/library-fluent/src/lib.rs
#[derive(RustEmbed)]
#[folder = "i18n/"]
pub struct LocalizationsEmbed;

pub static LOCALIZATIONS: LazyLock<RustEmbedNotifyAssets<LocalizationsEmbed>> =
    LazyLock::new(|| {
        RustEmbedNotifyAssets::new(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("i18n/"),
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

#[cfg(feature = "space")]
static TEXTURES: OnceLock<RwLock<HashMap<PathBuf, Arc<Texture>>>> = OnceLock::new();
static IMGUI_TEXTURES: OnceLock<RwLock<HashMap<String, Arc<NexusTexture>>>> = OnceLock::new();
static CONTROLLER_SENDER: RwLock<Option<Sender<ControllerEvent>>> = RwLock::new(None);
static RENDER_SENDER: RwLock<Option<Sender<RenderEvent>>> = RwLock::new(None);
static RENDER_CALLBACK: Mutex<Option<Revertible>> = Mutex::new(None);
static ACCOUNT_NAME_CELL: OnceLock<String> = OnceLock::new();

#[cfg(feature = "space")]
static SPACE_SENDER: RwLock<Option<Sender<SpaceEvent>>> = RwLock::new(None);

static CONTROLLER_THREAD: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);

nexus::export! {
    name: "TaimiHUD",
    signature: -0x7331BABD, // raidcore addon id or NEGATIVE random unique signature
    load,
    unload,
    flags: AddonFlags::None,
    provider: UpdateProvider::GitHub,
    update_link: "https://github.com/TaimiHUD/TaimiHUD",
    log_filter: "debug"
}

static RENDER_STATE: Mutex<Option<RenderState>> = Mutex::new(None);

static SOURCES: OnceLock<Arc<RwLock<SourcesFile>>> = OnceLock::new();
static SETTINGS: OnceLock<SettingsLock> = OnceLock::new();
#[cfg(feature = "space")]
static ENGINE_INITIALIZED: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "space")]
pub fn engine_initialized() -> bool {
    ENGINE_INITIALIZED.load(Ordering::SeqCst)
}
#[cfg(feature = "space")]
thread_local! {
    static ENGINE: RefCell<Option<Result<Engine, ()>>> = RefCell::new(None);
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

fn load() {
    let _ = IMGUI_TEXTURES.set(RwLock::new(HashMap::new()));
    #[cfg(feature = "space")]
    let _ = TEXTURES.set(RwLock::new(HashMap::new()));
    // Say hi to the world :o
    let name = env!("CARGO_PKG_NAME");
    let authors = env!("CARGO_PKG_AUTHORS");
    log::info!("Loading {name} by {authors}");

    // Set up the thread
    let addon_dir = get_addon_dir("Taimi").expect("Invalid addon dir");

    reload_language();

    let (controller_sender, controller_receiver) = channel::<ControllerEvent>(32);
    let (render_sender, render_receiver) = channel::<RenderEvent>(32);

    let controller_handler = {
        let render_sender = render_sender.clone();
        thread::spawn(move || Controller::load(controller_receiver, render_sender, addon_dir))
    };

    // muh queues
    *CONTROLLER_THREAD.lock().unwrap() = Some(controller_handler);
    *CONTROLLER_SENDER.write().unwrap() = Some(controller_sender);

    *RENDER_STATE.lock().unwrap() = Some(RenderState::new(render_receiver));
    *RENDER_SENDER.write().unwrap() = Some(render_sender);

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

    // Handle window toggling with keybind and button
    let main_window_keybind_handler = keybind_handler!(|_id, is_release| {
        if !is_release {
            Controller::try_send(ControllerEvent::WindowState(WINDOW_PRIMARY.into(), None));
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
            Controller::try_send(ControllerEvent::WindowState(WINDOW_MARKERS.into(), None));
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
            Controller::try_send(ControllerEvent::WindowState(WINDOW_TIMERS.into(), None));
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
            Controller::try_send(ControllerEvent::WindowState(WINDOW_PATHING.into(), None));
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

    let receive_texture =
        texture_receive!(|id: &str, _texture: Option<&NexusTexture>| log::info!("texture {id} loaded"));

    load_texture_from_memory("TAIMI_ICON", taimi_icon, Some(receive_texture));
    load_texture_from_memory("TAIMI_ICON_HOVER", taimi_hover_icon, Some(receive_texture));
    load_texture_from_memory("TAIMI_MARKERS_ICON", markers_icon, Some(receive_texture));
    load_texture_from_memory("TAIMI_MARKERS_ICON_HOVER", markers_hover_icon, Some(receive_texture));

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
                Controller::try_send(ControllerEvent::WindowState(WINDOW_TIMERS.into(), None));
            }
            #[cfg(feature = "space")]
            if ui.button(fl!("pathing-render-toggle")) {
                Engine::try_send(SpaceEvent::PathingToggle);
            }
            #[cfg(feature = "space")]
            if ui.button(fl!("pathing-window")) {
                Controller::try_send(ControllerEvent::WindowState(WINDOW_PATHING.into(), None));
            }
            #[cfg(feature = "markers")]
            if ui.button(fl!("marker-window")) {
                Controller::try_send(ControllerEvent::WindowState(WINDOW_MARKERS.into(), None));
            }
            if ui.button(fl!("primary-window")) {
                Controller::try_send(ControllerEvent::WindowState(WINDOW_PRIMARY.into(), None));
            }
        }),
    )
    .revert_on_unload();

    ACCOUNT_NAME
        .subscribe(event_consume!(<c_char> |name| {
            if let Some(name) = name {
                let name = unsafe {CStr::from_ptr(name as *const c_char)};
                let name = name.to_string_lossy().to_string();
                log::info!("Received account name: {name:?}");
                match ACCOUNT_NAME_CELL.set(name) {
                    Ok(_) => (),
                    Err(err) => log::error!("Error with account name cell: {err}"),
                }
            }
        }))
        .revert_on_unload();

    let combat_callback = event_consume!(|cdata: Option<&CombatData>| {
        if let Some(combat_data) = cdata {
            if let Some(evt) = combat_data.event() {
                if let Some(agt) = combat_data.src() {
                    let agt = AgentOwned::from(unsafe { ptr::read(agt) });
                    Controller::try_send(ControllerEvent::CombatEvent {
                        src: agt,
                        evt: evt.clone(),
                    });
                }
            }
        }
    });
    COMBAT_LOCAL.subscribe(combat_callback).revert_on_unload();

    // MumbleLink Identity
    MUMBLE_IDENTITY_UPDATED
        .subscribe(event_consume!(<MumbleIdentityUpdate> |mumble_identity| {
            match mumble_identity {
                None => (),
                Some(ident) => {
                    let copied_identity = ident.clone();
                    Controller::try_send(ControllerEvent::MumbleIdentityUpdated(copied_identity));
                },
            }
        }))
        .revert_on_unload();

    RTAPI_GROUP_MEMBER_LEFT.subscribe(
        event_consume!(
            <GroupMember> | group_member | {
                if let Some(group_member) = group_member {
                    let group_member: GroupMemberOwned = group_member.into();
                    Controller::try_send(ControllerEvent::RTAPISquadUpdate(SquadState::Left, group_member));
                }
            }
        )
    ).revert_on_unload();

    RTAPI_GROUP_MEMBER_JOINED.subscribe(
        event_consume!(
            <GroupMember> | group_member | {
                if let Some(group_member) = group_member {
                    let group_member: GroupMemberOwned = group_member.into();
                    Controller::try_send(ControllerEvent::RTAPISquadUpdate(SquadState::Joined, group_member));
                }
            }
        )
    ).revert_on_unload();

    RTAPI_GROUP_MEMBER_UPDATE.subscribe(
        event_consume!(
            <GroupMember> | group_member | {
                if let Some(group_member) = group_member {
                    let group_member: GroupMemberOwned = group_member.into();
                    Controller::try_send(ControllerEvent::RTAPISquadUpdate(SquadState::Update, group_member));
                }
            }
        )
    ).revert_on_unload();

    EXTRAS_SQUAD_UPDATE.subscribe(
        event_consume!(
            <SquadUpdate> | update | {
            if let Some(update) = update {
                let update: Vec<UserInfoOwned> = update.iter().map(|x| unsafe { ptr::read(x) }.to_owned()).collect();
                    Controller::try_send(ControllerEvent::ExtrasSquadUpdate(update));
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
                reload_language();
            }
        ))
        .revert_on_unload();
}

fn detect_language() -> String {
    let index_to_check = "KB_CHANGELOG";
    let language = match &translate(index_to_check).expect("Couldn't translate string")[..] {
        "Registro de Alterações" => "pt-br",
        "更新日志" => "cn",
        "Seznam změn" => "cz",
        "Änderungsprotokoll" => "de",
        "Changelog" => "en",
        "Notas del parche" => "es",
        "Journal des modifications" => "fr",
        "Registro modifiche" => "it",
        "Lista zmian" => "pl",
        "Список изменений" => "ru",
        _ => "en",
    };
    language.to_string()
}

fn reload_language() {
    let detected_language = detect_language();
    log::info!("Detected language {detected_language} for internationalization");
    let detected_language_identifier: LanguageIdentifier = detected_language
        .parse()
        .expect("Cannot parse detected language");
    let get_language = vec![detected_language_identifier];
    i18n_embed::select(&*LANGUAGE_LOADER, &*LOCALIZATIONS, get_language.as_slice())
        .expect("Couldn't load language!");
    (&*LANGUAGE_LOADER).set_use_isolating(false);
}

#[cfg(feature = "space")]
fn render_space(ui: &nexus::imgui::Ui) {
    let enabled = SETTINGS.get()
        .and_then(|settings| settings.try_read().ok())
        .map(|settings| settings.enable_katrender)
        .unwrap_or(false);
    if enabled && RenderState::is_running() {
        if !ENGINE_INITIALIZED.load(Ordering::Acquire) {
            let (space_sender, space_receiver) = channel::<SpaceEvent>(32);
            *SPACE_SENDER.write().unwrap() = Some(space_sender);
            let drawstate_inner = Engine::initialise(ui, space_receiver);
            if let Err(error) = &drawstate_inner {
                log::error!("DrawState setup failed: {error:?}");
            };
            ENGINE.set(Some(drawstate_inner.map_err(drop)));
            ENGINE_INITIALIZED.store(true, Ordering::Release);
        }
        ENGINE.with_borrow_mut(|ds_op| {
            if let Some(Ok(ds)) = ds_op {
                if let Err(error) = ds.render(ui) {
                    log::error!("Engine error: {error}");
                }
            }
        });
    }
}

fn unload() {
    log::info!("Unloading addon");

    let controller_handle = CONTROLLER_THREAD.lock().unwrap().take();
    let controller_quit = CONTROLLER_SENDER.write().unwrap().take()
        .map(|sender| sender.try_send(ControllerEvent::Quit));

    {
        let render_sender = RENDER_SENDER.write().unwrap().take();
        if RenderState::is_render_thread() {
            let _state = RenderState::lock().take();
            drop(_state);
            unload_render();
        } else if let Some(sender) = render_sender {
            match sender.try_send(RenderEvent::Quit) {
                Ok(()) => {
                    log::debug!("TODO: wait for renderer shutdown");
                    std::thread::sleep(std::time::Duration::from_millis(67));
                    #[cfg(feature = "space")] {
                        // just to be safe? idk
                        std::thread::sleep(std::time::Duration::from_millis(1500));
                    }
                },
                _ => {
                    // clean up what we can if possible
                    unload_render_background();
                },
            }
        }
    }

    match controller_quit {
        Some(Ok(())) => match controller_handle {
            Some(handle) => {
                log::info!("Waiting for controller shutdown...");
                if let Err(e) = handle.join() {
                    log_join_error("controller", e);
                }
            },
            None => {
                log::warn!("Controller unavailable?");
            },
        },
        Some(Err(..)) => {
            log::warn!("Failed to signal controller quit");
        },
        None => (),
    }

    if let Some(revert_render) = RENDER_CALLBACK.lock().unwrap().take() {
        revert_render();
    }
}

fn unload_render() {
    log::info!("Renderer unloading");
    debug_assert!(RenderState::is_render_thread());

    #[cfg(feature = "space")]
    if engine_initialized() {
        log::debug!("unloading space engine");
        let _ = ENGINE.try_with(|e| if let Some(Ok(mut engine)) = e.borrow_mut().take() {
            log::debug!("engine.cleanup()");
            engine.cleanup();
            /*log::debug!("skipping engine drop()");
            std::mem::forget(engine);*/
        });
        ENGINE_INITIALIZED.store(false, Ordering::SeqCst);
    }
}

/// A limited form of [unload_render()] that should try its best,
/// but isn't able to touch render TLS or single-threaded interfaces
fn unload_render_background() {
    log::warn!("Unloading render state from a background thread");

    let _state = RENDER_STATE.lock().unwrap().take();

    #[cfg(feature = "space")]
    {
        ENGINE_INITIALIZED.store(false, Ordering::SeqCst);
    }

    return
}

fn log_join_error(name: &str, e: Box<dyn std::any::Any + Send>) {
    let msg = if let Some(m) = e.downcast_ref::<&'static str>() {
        *m
    } else if let Some(m) = e.downcast_ref::<String>() {
        &m[..]
    } else {
        log::error!("{name} thread panicked");
        return
    };
    log::error!("{name} thread panicked: {msg}");
}
