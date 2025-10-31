use {
    crate::{
        exports,
        load_language,
        marker::format::MarkerType,
        notify_quit,
        settings::state::BootstrapState,
    },
    ::log::info,
    anyhow::Context,
    rand::{rng, seq::SliceRandom},
    std::{
        borrow::Cow,
        ffi::CStr,
        fs,
        mem,
        ops,
        path::{Path, PathBuf},
        ptr::{self, NonNull},
        sync::{
            atomic::{AtomicBool, AtomicI32, AtomicPtr, Ordering},
            Mutex,
            Once,
            RwLock,
        },
        time::Duration,
    },
    windows::Win32::{
        Foundation::HWND,
        UI::{Input::KeyboardAndMouse, WindowsAndMessaging},
    },
};

#[cfg(feature = "texture-loader")]
use crate::TEXTURES;

pub mod alert;
#[cfg(feature = "allocator")]
pub mod allocator;
pub mod bindings;
pub mod keyboard;
pub mod log;
pub mod mouse;
pub mod statistics;
pub mod textures;
pub mod update;
pub mod watched;
#[cfg(feature = "extension-arcdps")]
pub use arcloader_mumblelink::gw2_mumble::{LinkedMem as MumbleLink, MumblePtr, UiState};
#[cfg(not(feature = "extension-arcdps"))]
pub use nexus::data_link::mumble::{MumbleLink, MumblePtr, UiState};
#[cfg(feature = "extension-nexus")]
pub use nexus::{data_link::NexusLink, rtapi::RealTimeApi};
pub use {
    self::{
        alert::send_alert,
        mouse::MousePosition,
        statistics::Counter,
        textures::TextureLoader,
        watched::Watched,
    },
    arcdps::Language as GameLanguage,
    nexus::imgui,
    taimi_meta::coords::vec_eq,
    unic_langid_impl::subtags::Language,
};
#[cfg(not(feature = "extension-nexus"))]
pub type NexusLink = ();
#[cfg(not(feature = "extension-nexus"))]
pub type RealTimeApi = ();
#[cfg(any(feature = "space", feature = "texture-loader"))]
pub use taimi_d3d::{device::SwapChain0 as SwapChain, dx11::device::Device0 as Device};

pub type RuntimeError = &'static str;
pub type RuntimeResult<T = ()> = Result<T, RuntimeError>;
pub const RT_UNAVAILABLE: RuntimeError = "extension runtime unavailable";

pub const CRATE_NAME: &'static str = env!("CARGO_PKG_NAME");
pub const CRATE_VERSION: &'static str = match option_env!("ADDON_VERSION") {
    Some(v) => v,
    None => env!("CARGO_PKG_VERSION"),
};
pub const NAME: &'static str = "TaimiHUD";
pub const NAME_C: &'static CStr = unsafe { CStr::from_bytes_with_nul_unchecked(b"TaimiHUD\0") };
pub fn crate_authors() -> String {
    let mut rng = rng();
    let sep = match () {
        #[cfg(feature = "extension-nexus-codegen")]
        _ => ", ",
        #[cfg(not(feature = "extension-nexus-codegen"))]
        _ => ":",
    };
    let mut authors: Vec<_> = env!("CARGO_PKG_AUTHORS").split(sep).collect();
    authors.shuffle(&mut rng);
    authors.join(", ")
}

pub static LOADER_LOCK: Mutex<bool> = Mutex::new(false);

#[inline]
pub fn nexus_available() -> bool {
    match () {
        #[cfg(feature = "extension-nexus")]
        () => exports::nexus::available(),
        #[cfg(not(feature = "extension-nexus"))]
        _ => false,
    }
}

#[inline]
pub fn arcdps_available() -> bool {
    match () {
        #[cfg(feature = "extension-arcdps")]
        () => exports::arcdps::available(),
        #[cfg(not(feature = "extension-arcdps"))]
        _ => false,
    }
}

pub fn try_addon_dir() -> RuntimeResult<PathBuf> {
    #[cfg(feature = "extension-nexus")]
    if let Some(path) = exports::nexus::addon_dir()? {
        return Ok(path)
    }

    #[cfg(feature = "extension-arcdps")]
    if let Some(path) = exports::arcdps::addon_dir()? {
        return Ok(path)
    }

    Err(RT_UNAVAILABLE)
}

pub fn addon_dir_fallback() -> &'static Path {
    Path::new("addons/Taimi")
}

pub(crate) static ADDON_DIR: RwLock<Option<&'static Path>> = RwLock::new(None);

pub fn addon_dir() -> &'static Path {
    if let Ok(Some(path)) = ADDON_DIR.read().map(|p| *p) {
        return path
    }

    let fallback = addon_dir_fallback();
    let saved = BootstrapState::read_with(|state| {
        // as long as what we saved still seems valid, use it...
        let addon_dir = state
            .addon_dir
            .as_ref()
            .and_then(|addon_dir| fs::metadata(Path::new(addon_dir)).is_ok().then_some(addon_dir));
        match addon_dir {
            Some(addon_dir) => try_init_addon_dir(false, move || Some(addon_dir.into())),
            None => fallback,
        }
    });
    if saved as *const Path as *const () != fallback as *const Path as *const () {
        // if it didn't fall back due to mutex contention...
        return saved
    }

    match try_addon_dir() {
        Ok(path) => init_addon_dir(path),
        Err(e) => {
            static WARN_ONCE: Once = Once::new();
            let mut warn_once = false;
            WARN_ONCE.call_once(|| warn_once = true);
            if warn_once {
                // beware, logging can recurse into here to determine log file path
                ::log::warn!(logger: log::DeferredLogger::BEST_EFFORT, "falling back to default addon dir: {e}\n{}", std::backtrace::Backtrace::capture());
            }
            saved
        },
    }
}
pub(crate) fn try_init_addon_dir<F: FnOnce() -> Option<PathBuf>>(
    blocking: bool,
    addon_dir: F,
) -> &'static Path {
    let path = match blocking {
        true => ADDON_DIR.write().map_err(drop),
        false => ADDON_DIR.try_write().map_err(drop),
    };
    let Ok(mut path) = path else { return addon_dir_fallback() };
    if let Some(path) = *path {
        return path
    }

    if let Some(addon_dir) = addon_dir() {
        *path.get_or_insert_with(|| &*Box::leak(addon_dir.into_boxed_path()))
    } else {
        addon_dir_fallback()
    }
}
pub(crate) fn init_addon_dir<D: Into<Cow<'static, Path>> + AsRef<Path>>(addon_dir: D) -> &'static Path {
    if let Ok(mut path) = ADDON_DIR.write() {
        if let Some(path) = *path {
            if path == addon_dir.as_ref() {
                return path
            }
        }
        let addon_dir = match addon_dir.into() {
            Cow::Borrowed(p) => p,
            Cow::Owned(p) => &*Box::leak(p.into_boxed_path()),
        };
        *path.insert(addon_dir)
    } else {
        addon_dir_fallback()
    }
}

pub struct AddonDir;
impl ops::Deref for AddonDir {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        addon_dir()
    }
}

pub fn detect_language() -> RuntimeResult<Cow<'static, str>> {
    #[cfg(feature = "extension-nexus")]
    if let Some(lang) = exports::nexus::detect_language()? {
        return Ok(lang.into())
    }

    #[cfg(feature = "extension-arcdps")]
    if let Some(lang) = exports::arcdps::detect_language()? {
        return Ok(lang.into())
    }

    game_language()
        .map(crate::game_language_id)
        .map(Cow::Borrowed)
        .ok_or(RT_UNAVAILABLE)
}

pub fn reload_language() -> RuntimeResult {
    let saved = BootstrapState::read_with(|state: &BootstrapState| {
        state.language.as_ref().and_then(|l| l.parse::<Language>().ok())
    });
    let language;
    let language = match &saved {
        Some(l) => l.as_str(),
        _ => {
            language = detect_language()?;
            info!("Detected language {language} for internationalization");
            &language
        },
    };

    load_language(language)
}

static GAME_LANGUAGE: AtomicI32 = AtomicI32::new(i32::MIN);
pub fn game_language() -> Option<GameLanguage> {
    let id = GAME_LANGUAGE.load(Ordering::Relaxed);
    GameLanguage::try_from(id).ok()
}

pub fn notify_game_language(language: GameLanguage) {
    let id = language.into();
    let prev = GAME_LANGUAGE.swap(id, Ordering::Relaxed);
    if prev != id {
        let res = if BootstrapState::read_with(|state| state.language.is_none()) {
            reload_language()
                .map_err(anyhow::Error::msg)
                .with_context(|| format!("Failed to reload language"))
        } else {
            Ok(())
        };
        if let Err(e) = res {
            ::log::warn!("{e:#}");
        }
    }
}

static MUMBLE_LINK_PTR: AtomicPtr<MumbleLink> = AtomicPtr::new(ptr::dangling_mut());
pub fn mumble_link_ptr() -> RuntimeResult<MumblePtr> {
    match NonNull::new(MUMBLE_LINK_PTR.load(Ordering::Relaxed)) {
        Some(ml) if ml == NonNull::dangling() => (),
        Some(ml) => return Ok(unsafe { mem::transmute::<_, MumblePtr>(ml) }),
        None => return Err(RT_UNAVAILABLE),
    }

    let (ptr, res) = match get_mumble_link_ptr() {
        Err(e) => (ptr::null_mut(), Err(e)),
        Ok(Some(ml)) => (
            ml.cast().as_ptr(),
            Ok(unsafe { mem::transmute::<_, MumblePtr>(ml) }),
        ),
        Ok(None) => return Err(RT_UNAVAILABLE),
    };
    MUMBLE_LINK_PTR.store(ptr, Ordering::Relaxed);
    res
}

pub fn get_mumble_link_ptr() -> RuntimeResult<Option<NonNull<MumbleLink>>> {
    #[cfg(feature = "extension-nexus")]
    if let Some(ml) = exports::nexus::mumble_link_ptr()? {
        return Ok(Some(ml.cast()))
    }

    #[cfg(feature = "extension-arcdps")]
    if let Some(ml) = exports::arcdps::mumble_link_ptr()? {
        return Ok(Some(ml.cast()))
    }

    Ok(None)
}

pub fn nexus_link_ptr() -> RuntimeResult<NonNull<NexusLink>> {
    #[cfg(feature = "extension-nexus")]
    if let Some(nl) = exports::nexus::nexus_link_ptr()? {
        return Ok(nl)
    }

    #[cfg(feature = "extension-arcdps")]
    if let Some(nl) = exports::arcdps::nexus_link_ptr()? {
        return Ok(nl)
    }

    Err(RT_UNAVAILABLE)
}

pub fn read_nexus_link() -> RuntimeResult<NexusLink> {
    nexus_link_ptr().map(|p| unsafe { p.read_volatile() })
}

pub fn is_ingame() -> RuntimeResult<bool> {
    if let Ok(nexus_link) = nexus_link_ptr() {
        #[cfg(feature = "extension-nexus")]
        return Ok(unsafe {
            let is_gameplay = ptr::addr_of!((*nexus_link.as_ptr()).is_gameplay);
            is_gameplay.read_volatile()
        });
    }

    #[cfg(feature = "extension-arcdps")]
    if let Some(ingame) = exports::arcdps::is_ingame() {
        return Ok(ingame)
    }

    // TODO: fall back to mumblelink

    Err(RT_UNAVAILABLE)
}

static EXIT: AtomicBool = AtomicBool::new(false);
pub fn is_shutdown() -> bool {
    EXIT.load(Ordering::Relaxed)
}
pub fn notify_shutdown() {
    EXIT.store(true, Ordering::Relaxed);
}

pub fn rtapi() -> RuntimeResult<Option<RealTimeApi>> {
    #[cfg(feature = "extension-nexus")]
    if let Some(rtapi) = exports::nexus::rtapi()? {
        return Ok(Some(rtapi))
    }

    #[cfg(feature = "extension-arcdps")]
    if let Some(rtapi) = exports::arcdps::rtapi()? {
        return Ok(Some(rtapi))
    }

    Err(RT_UNAVAILABLE)
}

pub async fn press_marker_bind(
    marker: MarkerType,
    target: bool,
    down: bool,
    position: Option<MousePosition>,
) -> RuntimeResult<()> {
    #[cfg(feature = "extension-nexus")]
    if let Some(res) = exports::nexus::press_marker_bind(marker, target, down, position).await? {
        return Ok(res)
    }

    #[cfg(feature = "extension-arcdps")]
    if let Some(res) = exports::arcdps::press_marker_bind(marker, target, down, position).await? {
        return Ok(res)
    }

    Err(RT_UNAVAILABLE)
}

pub async fn invoke_marker_bind(
    marker: MarkerType,
    target: bool,
    duration: Duration,
    position: Option<MousePosition>,
) -> RuntimeResult<()> {
    use crate::settings::{InvokeMethod, Settings};
    if let Ok(false) = mumble_link_ptr().map(|ml| ml.read_ui_state().contains(UiState::GAME_HAS_FOCUS)) {
        return Err("Game unfocused")
    }

    press_marker_bind(marker, target, true, position).await?;

    let method = Settings::async_read()
        .await
        .ok()
        .and_then(|s| s.arc().gamebind_invoke)
        .unwrap_or(match nexus_available() {
            #[cfg(feature = "extension-nexus")]
            true => InvokeMethod::Nexus,
            _ => InvokeMethod::default(),
        });

    match method {
        InvokeMethod::Message => (),
        InvokeMethod::Input | InvokeMethod::Nexus => tokio::time::sleep(duration).await,
    }

    #[cfg(feature = "extension-nexus")]
    let position = match method {
        InvokeMethod::Nexus => None,
        _ => position,
    };

    press_marker_bind(marker, target, false, position).await
}

#[cfg(any(feature = "space", feature = "texture-loader"))]
pub fn dxgi_swap_chain() -> RuntimeResult<Option<SwapChain>> {
    #[cfg(feature = "extension-nexus")]
    if let Some(swap_chain) = exports::nexus::dxgi_swap_chain()? {
        return Ok(Some(swap_chain))
    }

    #[cfg(feature = "extension-arcdps")]
    if let Some(swap_chain) = exports::arcdps::dxgi_swap_chain()? {
        return Ok(Some(swap_chain))
    }

    Err(RT_UNAVAILABLE)
}

#[cfg(any(feature = "space", feature = "texture-loader"))]
pub fn d3d11_device() -> anyhow::Result<(Device, SwapChain)> {
    #[cfg(feature = "extension-nexus")]
    #[cfg(todo = "unnecessary")]
    if let Ok(Some(device)) = exports::nexus::d3d11_device() {
        return Ok(device)
    }

    let sc = dxgi_swap_chain()
        .transpose()
        .unwrap_or(Err(RT_UNAVAILABLE))
        .map_err(anyhow::Error::msg)
        .context("DXGI swap chain unavailable");

    sc.and_then(|sc| {
        sc.get_device11()
            .map(|d| (d, sc))
            .context("D3D11 device unavailable")
    })
}

pub async fn texture_schedule_path(key: &str, path: &Path) -> RuntimeResult<()> {
    let res = RT_UNAVAILABLE;

    #[cfg(feature = "texture-loader")]
    let res = if TEXTURES.is_available() {
        match TEXTURES.request_load_file(key, path).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                let msg = "Texture load failure";
                ::log::error!("{msg}: {e}");
                msg
            },
        }
    } else {
        res
    };

    #[cfg(feature = "extension-nexus")]
    if let Some(res) = exports::nexus::texture_schedule_path(key, path)? {
        return Ok(res)
    }

    Err(res)
}

pub async fn texture_schedule_bytes(key: &str, bytes: Vec<u8>) -> RuntimeResult<()> {
    let res = RT_UNAVAILABLE;

    #[cfg(feature = "texture-loader")]
    let res = if TEXTURES.is_available() {
        let bytes = match bytes {
            #[cfg(feature = "extension-nexus")]
            ref b => &b[..],
            #[cfg(not(feature = "extension-nexus"))]
            b => b,
        };
        match TEXTURES.request_load_bytes(key, bytes).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                let msg = "Texture load failure";
                ::log::error!("{msg}: {e}");
                msg
            },
        }
    } else {
        res
    };

    #[cfg(feature = "extension-nexus")]
    if let Some(res) = exports::nexus::texture_schedule_bytes(key, &bytes)? {
        return Ok(res)
    }

    Err(res)
}

pub fn window_handle() -> RuntimeResult<HWND> {
    let sc = dxgi_swap_chain()?.ok_or("swap chain unavailable")?;

    let desc = sc.get_desc0().map_err(|_| "swap chain descriptor missing")?;

    match desc.OutputWindow.is_invalid() {
        false => Ok(desc.OutputWindow),
        true => Err("no window handle associated with swap chain"),
    }
}

pub fn window_dpi() -> RuntimeResult<u32> {
    let hwnd = window_handle()?;
    taimi_input::win::mouse::window_dpi(hwnd).map_err(|_| RT_UNAVAILABLE)
}

pub fn screen_mouse_position() -> RuntimeResult<MousePosition> {
    taimi_input::win::mouse::screen_position().map_err(|e| {
        ::log::warn!("{e:#}");
        "Screen position of mouse not found"
    })
}

pub fn window_mouse_position() -> RuntimeResult<MousePosition> {
    let hwnd = window_handle()?;
    taimi_input::win::mouse::screen_position()
        .and_then(|pos| pos.to_window(hwnd))
        .map_err(|e| {
            ::log::warn!("{e:#}");
            "Window position of mouse not found"
        })
}

pub unsafe fn window_message(msg: u32, w: usize, l: isize) -> RuntimeResult<()> {
    let hwnd = window_handle()?;

    let res = taimi_input::win::window_message(hwnd, msg, w, l)
        .with_context(|| format!("failed to send window message {msg:#06x}({w:#010x}, {l:010x})"));
    if let Err(e) = res {
        ::log::warn!("{e:#}");
        return Err("PostMessageA failed")
    }

    Ok(())
}

pub fn window_send_inputs<I: Into<KeyboardAndMouse::INPUT>>(
    inputs: impl IntoIterator<Item = I>,
) -> RuntimeResult<()> {
    let hwnd = match window_handle() {
        Ok(wnd) => wnd,
        Err(_e) => {
            ::log::debug!("TODO: fall back for SendInput?");
            Default::default()
        },
    };

    let res = taimi_input::win::window_send_inputs(hwnd, inputs).context("failed to send window inputs");
    if let Err(e) = res {
        ::log::warn!("{e:#}");
        return Err("SendInput failed")
    }

    Ok(())
}

pub fn handle_wnd_event(_hwnd: HWND, msg: u32, w: usize, l: isize) -> u32 {
    match msg {
        WindowsAndMessaging::WM_KEYDOWN
        | WindowsAndMessaging::WM_SYSKEYDOWN
        | WindowsAndMessaging::WM_KEYUP
        | WindowsAndMessaging::WM_SYSKEYUP => return bindings::process_key_event(msg, w, l),
        WindowsAndMessaging::WM_LBUTTONUP
        | WindowsAndMessaging::WM_LBUTTONDOWN
        | WindowsAndMessaging::WM_RBUTTONUP
        | WindowsAndMessaging::WM_RBUTTONDOWN
        | WindowsAndMessaging::WM_MBUTTONUP
        | WindowsAndMessaging::WM_MBUTTONDOWN
        | WindowsAndMessaging::WM_XBUTTONUP
        | WindowsAndMessaging::WM_XBUTTONDOWN => return bindings::process_button_event(msg, w, l),
        WindowsAndMessaging::WM_DESTROY | WindowsAndMessaging::WM_QUIT | WindowsAndMessaging::WM_CLOSE => {
            // nexus will unload you immediately after, and need to make a point not to take too long waiting for a render cb that won't come
            notify_quit();
        },
        _ => (),
    }

    msg
}
