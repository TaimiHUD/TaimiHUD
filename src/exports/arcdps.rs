use std::{borrow::Cow, cell::RefCell, collections::BTreeMap, ffi::OsStr, num::NonZeroU64, path::{Path, PathBuf}, ptr::{self, NonNull}, sync::{atomic::{AtomicBool, AtomicI32, AtomicPtr, Ordering}, Mutex, RwLock}, time::Duration};
use arcdps::{exports as arc, extras::{Control, ExtrasAddonInfo, KeybindChange, UserInfoIter}, imgui, Language};
use dpsapi::combat::{CombatArgs, CombatEvent};
use arcloader_mumblelink::{gw2_mumble::{LinkedMem, MumbleLink, MumblePtr}, identity::MumbleIdentity};
use nexus::{data_link::NexusLink, rtapi::RealTimeApi};
use crate::{
    control_window, exports::{self, runtime::RuntimeResult}, game_language_id, load_arcdps, load_language, marker::format::MarkerType, receive_account_name, receive_evtc_local, receive_mumble_identity, render_overlay, settings::GitHubSource, unload, WINDOW_PRIMARY, WINDOW_TIMERS
};

pub const SIG: u32 = exports::SIG as u32;

pub fn gh_repo_src() -> GitHubSource {
    GitHubSource {
        owner: "kittywitch".into(),
        repository: "TaimiHUD".into(),
        description: None,
    }
}

static RUNTIME_AVAILABLE: AtomicBool = AtomicBool::new(false);
pub(crate) fn pre_init() {
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

#[cfg(feature = "extension-arcdps-codegen")]
pub(crate) fn cb_init() -> Result<(), String> {
    pre_init();

    let res = load_arcdps();
    if res.is_err() {
        RUNTIME_AVAILABLE.store(false, Ordering::SeqCst);
    }

    res.map_err(Into::into)
}

pub(crate) fn cb_release() {
    RUNTIME_AVAILABLE.store(false, Ordering::SeqCst);
    EXTRAS_AVAILABLE.store(false, Ordering::SeqCst);
    MUMBLE_LINK_PTR.store(ptr::null_mut(), Ordering::SeqCst);
    let _ml = MUMBLE_LINK.lock()
        .unwrap_or_else(|e| e.into_inner())
        .take();

    unload();
}

static IS_INGAME: AtomicBool = AtomicBool::new(false);

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
        receive_mumble_identity(update);
    }
}

#[cfg(feature = "extension-arcdps-codegen")]
pub(crate) fn cb_imgui(ui: &imgui::Ui, ingame: bool) {
    IS_INGAME.store(ingame, Ordering::Relaxed);

    update_mumble_link();

    #[cfg(feature = "space")] {
        crate::render_space(ui);
    }

    render_overlay(ui);
}

pub(crate) fn cb_options_end(ui: &imgui::Ui) {
    ui.text("WORK IN PROGRESS");

    ui.checkbox("Check for updates", &mut false);

    ui.new_line();
    let all_windows = [
        WINDOW_PRIMARY,
        WINDOW_TIMERS,
        #[cfg(feature = "markers")]
        crate::WINDOW_MARKERS,
    ];
    for window in all_windows {
        let _id = ui.push_id(window);
        let singular = window.strip_suffix("s");
        let name = crate::LANGUAGE_LOADER.get(&format!("{}-window-toggle", singular.unwrap_or(window)));
        if ui.button(name) {
            control_window(window, None);
        }
        ui.same_line();
        ui.text("Keybind: ");
        ui.same_line();
        ui.text_disabled("ALT+SHIFT+");
        ui.same_line();
        if ui.button("BIND") {
            log::warn!("TODO: keybind settings");
        }
        if window == WINDOW_PRIMARY {
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

#[cfg(feature = "extension-arcdps-codegen")]
pub(crate) fn cb_wnd_filter(keycode: usize, key_down: bool, prev_key_down: bool) -> bool {
    true
}

const UPDATE_CHECK_TIMEOUT: Duration = Duration::from_secs(4);

#[cfg(feature = "extension-arcdps-codegen")]
pub(crate) fn cb_update_url() -> Option<String> {
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

pub fn update_allowed() -> bool {
    // TODO: setting somewhere!
    true
}

#[cfg(feature = "extension-arcdps-codegen")]
pub(crate) fn cb_combat_local(
    ev: Option<&arcdps::Event>,
    src: Option<&arcdps::Agent>,
    dst: Option<&arcdps::Agent>,
    skill_name: Option<&'static str>,
    id: u64,
    revision: u64,
) {
    let skill_name = match skill_name {
        // if one strongly suspects the str wasn't reallocated
        // then you could do an out-of-bounds check, but also...
        // just don't, it's unused anyway
        _ => Default::default(),
    };
    let event = CombatArgs {
        ev: ev
            .map(|e| Cow::Borrowed(e.as_ref())),
        src: src
            .map(|a| Cow::Borrowed(a.as_ref())),
        dst: dst
            .map(|a| Cow::Borrowed(a.as_ref())),
        skill_name,
        id: NonZeroU64::new(id),
        revision,
    };
    match event.event() {
        Some(CombatEvent::Skill(..)) =>
            event.borrow_imp(receive_evtc_local),
        Some(CombatEvent::Agent(agent)) if agent.is_self().get() => {
            if let Some(name) = agent.account_names() {
                receive_account_name(name.to_string_lossy());
            }
        },
        None => {
            log::warn!("unrecognized cbtevent {event:?}");
        },
        _ => (),
    }
}

static EXTRAS_AVAILABLE: AtomicBool = AtomicBool::new(false);

pub(crate) fn cb_extras_init(info: ExtrasAddonInfo, account_name: Option<&str>) {
    EXTRAS_AVAILABLE.store(true, Ordering::Relaxed);

    log::debug!("arcdps_extras initialized: {info:?}");

    if let Some(name) = account_name {
        receive_account_name(name);
    }
}

static GAME_LANGUAGE: AtomicI32 = AtomicI32::new(Language::English as i32);

pub fn game_language() -> Option<Language> {
    let id = GAME_LANGUAGE.load(Ordering::Relaxed);
    Language::try_from(id).ok()
}

pub(crate) fn cb_extras_language(language: Language) {
    let id = language.into();
    let prev = GAME_LANGUAGE.swap(id, Ordering::Relaxed);
    if prev != id {
        let res = load_language(game_language_id(language));
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

pub(crate) fn cb_extras_keybind(changed: KeybindChange) {
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

#[cfg(feature = "extension-arcdps-codegen")]
pub(crate) fn cb_extras_squad_update(members: UserInfoIter) {
    use crate::receive_squad_update;

    receive_squad_update(members)
}

pub fn available() -> bool {
    RUNTIME_AVAILABLE.load(Ordering::Relaxed)
}

pub fn extras_available() -> bool {
    EXTRAS_AVAILABLE.load(Ordering::Relaxed)
}

const NO_EXPORT: &'static str = "arcdps export missing";

pub fn addon_dir() -> RuntimeResult<Option<PathBuf>> {
    if !available() {
        return Ok(None)
    }

    if !arc::has_e0_config_path() {
        return Err(NO_EXPORT)
    }
    let mut path = arcdps::exports::config_path()
        .ok_or("Unknown arcdps config dir")?;
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

pub fn invoke_marker_bind(marker: MarkerType, target: bool, duration_ms: i32) -> RuntimeResult<Option<()>> {
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

    let KeybindChange { key, mod_shift, mod_alt, mod_ctrl, .. } = binding;

    Err("TODO: invoke marker bind")
}

#[cfg(any(feature = "space", feature = "texture-loader"))]
pub fn d3d11_device() -> RuntimeResult<Option<windows::Win32::Graphics::Direct3D11::ID3D11Device>> {
    if !available() {
        return Ok(None)
    }

    let device = arcdps::d3d11_device().cloned();

    Ok(unsafe {
        core::mem::transmute(device)
    })
}

#[cfg(feature = "space")]
pub fn dxgi_swap_chain() -> RuntimeResult<Option<windows::Win32::Graphics::Dxgi::IDXGISwapChain>> {
    if !available() {
        return Ok(None)
    }

    let swap_chain = arcdps::dxgi_swap_chain().cloned();

    Ok(unsafe {
        core::mem::transmute(swap_chain)
    })
}

pub fn texture_schedule_path(key: &str, path: &Path) -> RuntimeResult<Option<()>> {
    if !available() {
        return Ok(None)
    }

    Err("texture loading unimplemented")
}

pub fn texture_schedule_bytes(key: &str, data: Vec<u8>) -> RuntimeResult<Option<()>> {
    if !available() {
        return Ok(None)
    }

    Err("texture loading unimplemented")
}
