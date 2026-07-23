#![allow(unused_variables)]

use {
    crate::Result,
    arcffi::cstr::Str0,
    core::ptr::NonNull,
    std::path::{Path, PathBuf},
};

pub type Nop = ();
#[inline(always)]
pub const fn nop() -> &'static Nop {
    &()
}

fn pending<T: Sync + Send + 'static>() -> crate::DynStreamOf<T> {
    Box::into_pin(Box::new(futures_util::stream::pending::<T>()) as Box<_>)
}
fn unimpl() -> anyhow::Error {
    anyhow::Error::msg("unimplemented")
}

impl crate::HostedBy for Nop {
    fn available(&self) -> bool {
        false
    }
}
impl crate::HostedLogs for Nop {
    fn log_filter_meta(&self, metadata: &log::Metadata<'_>) -> bool {
        false
    }
    fn log_record_c(&self, record: &log::Record<'_>, message: Option<&Str0>) -> bool {
        false
    }
    fn log_wants_message(&self) -> crate::logs::LogMessageStyle {
        Default::default()
    }
}
impl crate::HostedStorageDir for Nop {
    fn init_addon_dir(&self) -> Result<PathBuf> {
        Err(unimpl())
    }
    fn game_dir(&self) -> Result<&Path> {
        Err(unimpl())
    }
}
unsafe impl crate::HostedGameInfo for Nop {
    fn mumblelink_ptr(&self) -> Option<NonNull<()>> {
        None
    }
    fn game_language_id(&self) -> crate::GameLanguageId {
        crate::GameLanguageId::UNKNOWN
    }
    fn is_ingame(&self) -> Option<bool> {
        None
    }
}
unsafe impl crate::HostedAddonApi for Nop {
    fn addonapi_version(&self) -> Option<u8> {
        None
    }
    fn addonapi_ptr(&self, version: u8) -> Option<NonNull<()>> {
        None
    }
    fn nexuslink_ptr(&self) -> Option<NonNull<()>> {
        None
    }
    fn rtapi_ptr(&self) -> Option<NonNull<()>> {
        None
    }
}

impl crate::HostedGameInvoke for Nop {
    fn press_gamebind(
        &self,
        gamebind: crate::GameControlIndex,
        down: bool,
        position: Option<crate::MousePosition>,
    ) -> Result<()> {
        Ok(())
    }
}
impl crate::HostedKeybinds for Nop {
    fn register_bind(&self, ident: &Str0, default: Option<crate::KeyState>) -> Result<crate::KeybindId> {
        Ok(0)
    }
    fn update_bind(&self, id: crate::KeybindId, new_value: Option<crate::KeyState>) -> Result<()> {
        Ok(())
    }
    fn async_binds(&self) -> crate::DynStreamOf<(crate::KeybindId, Option<bool>)> {
        pending()
    }
}
unsafe impl crate::HostedGameWindow for Nop {
    fn game_window_handle(&self) -> Option<NonNull<()>> {
        None
    }
    fn register_key_interest(&self, key: u16) {}
    fn deregister_key_interest(&self, key: u16) {}
    fn async_keys(&self) -> crate::DynStreamOf<(u16, bool, crate::ModState)> {
        pending()
    }
    fn async_window_events(&self) -> crate::DynStreamOf<crate::WindowEvent> {
        pending()
    }
}
impl crate::HostedGameSettings for Nop {
    fn lookup_gamebind(
        &self,
        gamebind: crate::GameControlIndex,
        slot: Option<crate::GameControlSlot>,
    ) -> Option<crate::KeyState> {
        None
    }
}
impl crate::HostedEvtc for Nop {
    /// TODO: `'static` is a mood killer
    fn async_combat_events(&self) -> crate::DynStreamOf<crate::CombatEvent> {
        pending()
    }
}
