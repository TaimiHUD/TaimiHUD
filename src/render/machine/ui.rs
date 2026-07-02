use {
    super::RenderMachine,
    crate::{
        render::element::im::{UiContextStorage, DrawContextInput},
        settings::state::AddonHostName,
    },
    core::num::NonZero,
    std::sync::{atomic::{AtomicU32, Ordering}, LazyLock, RwLock},
};

impl RenderMachine {
    #[inline(always)]
    pub fn ui_shared_context() -> &'static RwLock<UiContextStorage> {
        static UI_CONTEXT: LazyLock<RwLock<UiContextStorage>> = LazyLock::new(Default::default);
        &UI_CONTEXT
    }
    #[inline]
    pub fn ui_read_context() -> UiContextStorage {
        (*Self::ui_shared_context()
            .read()
            .unwrap_or_else(|e| e.into_inner()))
        .clone()
    }

    const UI_REPORT_ORD_PUBLISH: Ordering = Ordering::Relaxed;
    const UI_REPORT_ORD_READ: Ordering = Ordering::Relaxed;
    pub(super) fn ui_publish_report(report: &UiStateReport) {
        Self::ui_published_captured_keyboard().store(InterfaceParty::opt_repr(report.captured_keyboard), Self::UI_REPORT_ORD_PUBLISH);
    }
    #[inline]
    pub fn ui_read_captured_keyboard() -> Option<InterfaceParty> {
        InterfaceParty::from_id(Self::ui_published_captured_keyboard().load(Self::UI_REPORT_ORD_READ))
    }
    #[inline]
    fn ui_published_captured_keyboard() -> &'static AtomicU32 {
        static CAPTURED_KEYBOARD: AtomicU32 = AtomicU32::new(InterfaceParty::REPR_NONE);
        &CAPTURED_KEYBOARD
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct InterfaceParty {
    pub id: NonZero<u32>,
}
impl InterfaceParty {
    /// game unfocused
    pub const OS: Self = Self::with_id_const(1);
    pub const GAME: Self = Self::with_id_const(2);
    pub const UNKNOWN: Self = Self::with_id_const(3);
    pub const MODAL: Self = Self::with_id_const(4);
    pub const TAIMI: Self = Self::with_id_const(match () {
        #[cfg(todo)]
        _ => crate::exports::SIG as _,
        _ => 7,
    });
    pub const NEXUS: Self = Self::with_id_const(8);
    pub const ARCDPS: Self = Self::with_id_const(9);

    #[inline(always)]
    pub const fn with_id(id: NonZero<u32>) -> Self {
        Self { id }
    }
    pub const fn with_id_const(id: u32) -> Self {
        match Self::from_id(id) {
            Some(id) => id,
            None => panic!("id=0"),
        }
    }
    #[inline]
    pub const fn from_id(id: u32) -> Option<Self> {
        match NonZero::new(id) {
            Some(id) => Some(Self::with_id(id)),
            None => None,
        }
    }

    pub const REPR_NONE: u32 = 0;

    #[inline(always)]
    pub const fn to_repr(self) -> u32 {
        self.id.get()
    }

    #[inline]
    pub const fn opt_repr(v: Option<Self>) -> u32 {
        match v {
            Some(v) => v.to_repr(),
            None => Self::REPR_NONE,
        }
    }

    pub(super) fn for_ui_context(context: DrawContextInput<'_>) -> Self {
        match context.container.viewport.host {
            AddonHostName::ArcDPS => Self::ARCDPS,
            AddonHostName::Nexus => Self::NEXUS,
            #[cfg(todo)]
            _ => Self::UNKNOWN,
            _ => Self::TAIMI,
        }
    }
}
impl Default for InterfaceParty {
    fn default() -> Self {
        match () {
            #[cfg(todo)]
            _ => Self::GAME,
            _ => Self::UNKNOWN,
        }
    }
}
impl From<u32> for InterfaceParty {
    fn from(id: u32) -> Self {
        Self::from_id(id).unwrap_or(Self::UNKNOWN)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct UiStateReport {
    pub captured_keyboard: Option<InterfaceParty>,
    #[cfg(todo = "unused")]
    pub captured_mouse: Option<InterfaceParty>,
}
