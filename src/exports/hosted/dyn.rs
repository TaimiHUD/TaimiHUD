use {
    crate::exports::runtime as rt,
    arcffi::UnsaferCell,
    core::{mem, ptr},
    taimi_hosted::*,
};

#[derive(Clone)]
pub struct HostedProviderDyn<'h> {
    pub host: &'h dyn HostedBy,
    pub storage: &'h dyn HostedStorageDir,
    pub logs: &'h dyn HostedLogs,
    pub game_info: &'h dyn HostedGameInfo,
    #[cfg(feature = "extension-nexus")]
    pub addonapi: &'h dyn HostedAddonApi,
    pub game_invoke: &'h dyn HostedGameInvoke,
    pub keybinds: &'h dyn HostedKeybinds,
    pub game_window: &'h dyn HostedGameWindow,
    pub game_settings: &'h dyn HostedGameSettings,
    pub game_combat: &'h dyn HostedEvtc,
}

impl<'h> HostedProviderDyn<'h> {
    #[inline(always)]
    pub unsafe fn immortal(self) -> HostedProviderDyn<'static> {
        mem::transmute(self)
    }
    pub unsafe fn immortalize_globally(self) {
        ptr::write(HostedProviderDyn::singleton_storage().get(), self.immortal());
    }
}
impl HostedProviderDyn<'static> {
    pub const NOP: Self = Self {
        host: unsafe {
            // `&()` seems unlikely to be guaranteed to be unique across codegen units even if ZST?
            &*ptr::dangling::<()>() as &_
        },
        storage: &(),
        logs: &(),
        game_info: &(),
        #[cfg(feature = "extension-nexus")]
        addonapi: &(),
        game_invoke: &(),
        keybinds: &(),
        game_window: &(),
        game_settings: &(),
        game_combat: &(),
    };

    #[inline(always)]
    pub fn is_nop(&self) -> bool {
        ptr::addr_eq(self.host, Self::NOP.host)
    }

    #[inline(always)]
    fn singleton_storage() -> &'static UnsaferCell<Self> {
        static HOSTED_PROVIDER: UnsaferCell<HostedProviderDyn<'static>> =
            unsafe { UnsaferCell::new(HostedProviderDyn::NOP) };
        &HOSTED_PROVIDER
    }
    #[inline(always)]
    pub fn singleton() -> &'static Self {
        unsafe { Self::singleton_storage().as_ref_unchecked() }
    }
    /// consider whether it needs clearing if you can guarantee nothing is accessing the singleton?
    pub unsafe fn nop_globally() {
        ptr::write(Self::singleton_storage().get(), Self::NOP)
    }
}
#[inline(always)]
pub(crate) fn singleton() -> &'static HostedProviderDyn<'static> {
    HostedProviderDyn::singleton()
}

/// TODO
#[cfg(feature = "markers")]
pub(crate) fn press_marker_bind(
    marker: crate::marker::format::MarkerType,
    target: bool,
    down: bool,
    position: Option<rt::MousePosition>,
) -> rt::RuntimeResult<Option<()>> {
    if !singleton().host.available() {
        return Ok(None)
    }
    let control = rt::keyboard::control_for_marker(marker, target);
    log::debug!("TODO: press_marker_bind");
    rt::keyboard::press_bind(control, down, position)
}
