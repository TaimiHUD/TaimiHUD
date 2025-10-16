use anyhow::Context;
use std::{
    borrow::Cow,
    ffi::CStr,
    mem,
    ops,
    path::{Path, PathBuf},
    ptr::{self, NonNull},
    sync::{
        atomic::{AtomicBool, AtomicPtr, Ordering},
        Mutex, Once, OnceLock,
    },
    time::Duration,
};
use ::log::info;
use crate::{exports, load_language, marker::format::MarkerType, notify_quit};
use windows::Win32::{
    Foundation::HWND,
    UI::{
        WindowsAndMessaging,
        Input::KeyboardAndMouse,
    },
};
#[cfg(feature = "texture-loader")]
use crate::TEXTURES;

pub mod keyboard;
pub mod log;
pub mod mouse;
pub mod textures;
pub mod update;
pub use {
    nexus::imgui,
    self::{
        mouse::MousePosition,
        textures::TextureLoader,
    },
    taimi_meta::coords::vec_eq,
    taimi_input::win::keyboard::KeyState,
};

#[cfg(feature = "extension-arcdps")]
pub use arcloader_mumblelink::gw2_mumble::{LinkedMem as MumbleLink, MumblePtr, UiState};
#[cfg(not(feature = "extension-arcdps"))]
pub use nexus::data_link::mumble::{MumbleLink, MumblePtr, UiState};
#[cfg(feature = "extension-nexus")]
pub use nexus::{data_link::NexusLink, rtapi::RealTimeApi};
#[cfg(not(feature = "extension-nexus"))]
pub type NexusLink = ();
#[cfg(not(feature = "extension-nexus"))]
pub type RealTimeApi = ();
#[cfg(any(feature = "space", feature = "texture-loader"))]
pub use taimi_d3d::{
    device::SwapChain0 as SwapChain,
    dx11::device::Device0 as Device,
};

pub type RuntimeError = &'static str;
pub type RuntimeResult<T = ()> = Result<T, RuntimeError>;
pub const RT_UNAVAILABLE: RuntimeError = "extension runtime unavailable";

pub const CRATE_NAME: &'static str = env!("CARGO_PKG_NAME");
pub const CRATE_VERSION: &'static str = env!("CARGO_PKG_VERSION");
pub const NAME: &'static str = "TaimiHUD";
pub const NAME_C: &'static CStr = unsafe {
    CStr::from_bytes_with_nul_unchecked(b"TaimiHUD\0")
};

pub static LOADER_LOCK: Mutex<bool> = Mutex::new(false);

pub fn nexus_available() -> bool {
    match () {
        #[cfg(feature = "extension-nexus")]
        () => exports::nexus::available(),
        #[cfg(not(feature = "extension-nexus"))]
        _ => false,
    }
}

#[cfg(todo)]
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

static ADDON_DIR: OnceLock<Cow<'static, Path>> = OnceLock::new();

pub fn addon_dir() -> &'static Path {
    if let Some(path) = ADDON_DIR.get() {
        return path
    }

    match try_addon_dir() {
        Ok(path) =>
            ADDON_DIR.get_or_init(|| path.into()),
        Err(e) => {
            let warn_once = {
                static WARN_ONCE: Once = Once::new();
                let mut first_time = false;
                WARN_ONCE.call_once(|| first_time = true);
                first_time
            };
            if warn_once {
                // beware, logging can recurse into here to determine log file path
                ::log::warn!("falling back to default addon dir due to error: {e}");
            }
            addon_dir_fallback()
        },
    }
}

pub struct AddonDir;
impl ops::Deref for AddonDir {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        addon_dir()
    }
}

pub fn detect_language() -> RuntimeResult<String> {
    #[cfg(feature = "extension-nexus")]
    if let Some(lang) = exports::nexus::detect_language()? {
        return Ok(lang)
    }

    #[cfg(feature = "extension-arcdps")]
    if let Some(lang) = exports::arcdps::detect_language()? {
        return Ok(lang)
    }

    Err(RT_UNAVAILABLE)
}

pub fn reload_language() -> RuntimeResult {
    let language = detect_language()?;
    info!("Detected language {language} for internationalization");

    load_language(&language)
}

static MUMBLE_LINK_PTR: AtomicPtr<MumbleLink> = AtomicPtr::new(ptr::dangling_mut());
pub fn mumble_link_ptr() -> RuntimeResult<MumblePtr> {
    match NonNull::new(MUMBLE_LINK_PTR.load(Ordering::Relaxed)) {
        Some(ml) if ml == NonNull::dangling() =>
            (),
        Some(ml) => return Ok(unsafe {
            mem::transmute::<_, MumblePtr>(ml)
        }),
        None => return Err(RT_UNAVAILABLE),
    }

    let (ptr, res) = match get_mumble_link_ptr() {
        Err(e) => (ptr::null_mut(), Err(e)),
        Ok(Some(ml)) => (ml.cast().as_ptr(), Ok(unsafe {
            mem::transmute::<_, MumblePtr>(ml)
        })),
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
    nexus_link_ptr()
        .map(|p| unsafe { p.read_volatile() })
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

pub async fn press_marker_bind(marker: MarkerType, target: bool, down: bool, position: Option<MousePosition>) -> RuntimeResult<()> {
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

pub async fn invoke_marker_bind(marker: MarkerType, target: bool, duration: Duration, position: Option<MousePosition>) -> RuntimeResult<()> {
    if let Ok(false) = mumble_link_ptr().map(|ml| ml.read_ui_state().contains(UiState::GAME_HAS_FOCUS)) {
        return Err("Game unfocused")
    }

    press_marker_bind(marker, target, true, position).await?;

    tokio::time::sleep(duration).await;

    #[cfg(feature = "extension-nexus")]
    let position = match exports::nexus::available() {
        false => position,
        true => None,
    };

    press_marker_bind(marker, target, false, position).await
}

/// TODO: push to controller alert queue or something...
pub fn send_alert(ui: &imgui::Ui, message: &str) -> RuntimeResult<()> {
    #[cfg(feature = "extension-nexus")]
    if let Some(res) = exports::nexus::send_alert(ui, message)? {
        return Ok(res)
    }

    #[cfg(feature = "extension-arcdps")]
    if let Some(res) = exports::arcdps::send_alert(ui, message)? {
        return Ok(res)
    }

    Err(RT_UNAVAILABLE)
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
        .transpose().unwrap_or(Err(RT_UNAVAILABLE))
        .map_err(anyhow::Error::msg)
        .context("DXGI swap chain unavailable");

    sc.and_then(|sc| sc.get_device11()
        .map(|d| (d, sc))
        .context("D3D11 device unavailable")
    )
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
    } else { res };

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
    } else { res };

    #[cfg(feature = "extension-nexus")]
    if let Some(res) = exports::nexus::texture_schedule_bytes(key, &bytes)? {
        return Ok(res)
    }

    Err(res)
}

pub fn window_handle() -> RuntimeResult<HWND> {
    let sc = dxgi_swap_chain()?
        .ok_or("swap chain unavailable")?;

    let desc = sc.get_desc0()
        .map_err(|_| "swap chain descriptor missing")?;

    match desc.OutputWindow.is_invalid() {
        false => Ok(desc.OutputWindow),
        true => Err("no window handle associated with swap chain"),
    }
}

pub fn window_dpi() -> RuntimeResult<u32> {
    let hwnd = window_handle()?;
    taimi_input::win::mouse::window_dpi(hwnd)
        .map_err(|_| RT_UNAVAILABLE)
}

pub fn screen_mouse_position() -> RuntimeResult<MousePosition> {
    taimi_input::win::mouse::screen_position()
        .map_err(|e| {
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

pub fn window_send_inputs<I: Into<KeyboardAndMouse::INPUT>>(inputs: impl IntoIterator<Item = I>) -> RuntimeResult<()> {
    let hwnd = match window_handle() {
        Ok(wnd) => wnd,
        Err(_e) => {
            ::log::debug!("TODO: fall back for SendInput?");
            Default::default()
        },
    };

    let res = taimi_input::win::window_send_inputs(hwnd, inputs)
        .context("failed to send window inputs");
    if let Err(e) = res {
        ::log::warn!("{e:#}");
        return Err("SendInput failed")
    }

    Ok(())
}

pub fn handle_wnd_event(_hwnd: HWND, msg: u32, _w: usize, _l: isize) -> u32 {
    match msg {
        WindowsAndMessaging::WM_DESTROY | WindowsAndMessaging::WM_QUIT | WindowsAndMessaging::WM_CLOSE => {
            // nexus will unload you immediately after, and need to make a point not to take too long waiting for a render cb that won't come
            notify_quit();
        },
        _ => (),
    }

    msg
}
