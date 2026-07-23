//! generic interfaces for functionality provided by the runtime environment
//! of various loaders we can run under

use {
    anyhow::Result,
    arcffi::cstr::Str0,
    core::{pin::Pin, ptr::NonNull},
    futures_core::stream::Stream,
    std::path::{Path, PathBuf},
};

pub mod logs;
mod nop;

pub use self::{
    logs::HostedLogs,
    nop::{nop, Nop},
};

pub type DynStreamOf<T> = Pin<Box<dyn Stream<Item = T> + Sync + Send + 'static>>;

pub trait HostedBy {
    fn available(&self) -> bool;
    #[cfg(todo)]
    fn variant_name(&self) -> &'static str;
    #[cfg(todo)]
    fn request_unload(&self) -> bool;
    #[cfg(todo)]
    fn async_host_events(&self) -> DynStreamOf<HostedEvent>;
}
#[cfg(todo)]
pub enum HostedEvent {
    Unload,
}
pub trait HostedStorageDir {
    fn init_addon_dir(&self) -> Result<PathBuf>;
    fn game_dir(&self) -> Result<&Path>;
}
pub unsafe trait HostedGameInfo {
    fn mumblelink_ptr(&self) -> Option<NonNull<()>>;
    fn game_language_id(&self) -> GameLanguageId;
    #[cfg(todo)]
    fn async_mumblelink_ticks(&self) -> DynStreamOf<(u32, Option<bool>)>;
    #[deprecated]
    fn is_ingame(&self) -> Option<bool>;
}

/// TODO: move to `taimi_meta` or out of addonapi crate or w/e
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GameLanguageId(pub u8);
impl GameLanguageId {
    pub const UNKNOWN: Self = Self(u8::MAX);
    #[inline]
    pub const fn known(self) -> Option<Self> {
        match self {
            Self::UNKNOWN => None,
            #[cfg(todo)]
            0..=5 => Some(self),
            _ => Some(self),
        }
    }
}

/// nexus
pub unsafe trait HostedAddonApi {
    fn addonapi_version(&self) -> Option<u8>;
    fn addonapi_ptr(&self, version: u8) -> Option<NonNull<()>>;
    fn nexuslink_ptr(&self) -> Option<NonNull<()>>;
    fn rtapi_ptr(&self) -> Option<NonNull<()>>;
}

pub type MousePosition = glamour::Point2<i32>;
pub type GameControlIndex = u8;
pub type GameControlSlot = u8;

pub trait HostedGameInvoke {
    fn press_gamebind(
        &self,
        gamebind: GameControlIndex,
        down: bool,
        position: Option<MousePosition>,
    ) -> Result<()>;
    /// TODO: `'static` is a mood killer
    #[cfg(todo)]
    fn press_async(
        &self,
        gamebind: GameControlIndex,
        down: bool,
        position: Option<MousePosition>,
    ) -> Box<dyn Future<Output = ()> + Send>;
    /// TODO: ugly, but is provided by nexus...
    #[cfg(todo)]
    fn invoke_async(
        &self,
        gamebind: GameControlIndex,
        down: bool,
        position: Option<MousePosition>,
        duration: Duration,
    ) -> Box<dyn Future<Output = ()> + Send>;
}

pub type ModState = u8;
pub type KeyState = (u16, ModState);
pub type KeybindId = usize;
pub trait HostedKeybinds {
    fn register_bind(&self, ident: &Str0, default: Option<KeyState>) -> Result<KeybindId>;
    fn update_bind(&self, id: KeybindId, new_value: Option<KeyState>) -> Result<()>;
    #[cfg(todo)]
    fn deregister_bind(&self, id: KeybindId) -> Result<()>;
    /// TODO: `'static` is a mood killer
    fn async_binds(&self) -> DynStreamOf<(KeybindId, Option<bool>)>;
}
pub enum WindowEvent {
    /// `WM_CLOSE`
    Close,
    /// `WM_QUIT`
    Quit,
    #[cfg(todo)]
    Focus(bool),
    #[cfg(todo)]
    Key(u16, bool, ModState),
}
pub unsafe trait HostedGameWindow {
    fn game_window_handle(&self) -> Option<NonNull<()>>;
    fn register_key_interest(&self, key: u16);
    fn deregister_key_interest(&self, key: u16);
    fn async_keys(&self) -> DynStreamOf<(u16, bool, ModState)>;
    fn async_window_events(&self) -> DynStreamOf<WindowEvent>;
    /// no ownership transfer
    ///
    /// TODO: render callbacks can just pass this along instead
    #[deprecated]
    fn dxgi_swap_chain(&self) -> Option<NonNull<()>> {
        None
    }
}
pub trait HostedGameSettings {
    fn lookup_gamebind(
        &self,
        gamebind: GameControlIndex,
        slot: Option<GameControlSlot>,
    ) -> Option<KeyState>;
    #[cfg(todo)]
    fn async_gamebind_changes(&self) -> DynStreamOf<(GameControlIndex, GameControlSlot)>;
    #[cfg(todo)]
    fn async_language_changes(&self) -> DynStreamOf<GameLanguageId>;
}
/// TODO
pub type CombatEvent = Box<[u8]>;
pub trait HostedEvtc {
    /// TODO: `'static` is a mood killer
    fn async_combat_events(&self) -> DynStreamOf<CombatEvent>;
}
