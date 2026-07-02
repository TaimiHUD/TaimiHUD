use {
    core::fmt,
    std::borrow::Cow,
    strum::{EnumCount, FromRepr, VariantArray},
    taimi_pack::script::{pathing::ScriptApiEvent, Result as ScriptResult},
};
#[cfg(feature = "scripts-lua")]
use {
    mlua::{FromLua, IntoLua, Lua, Result as LuaResult, Value as LuaValue},
    taimi_pack::script::{format_err, lua},
};
pub type SignalId = u32;

#[derive(Debug, Copy, Clone)]
pub struct ScriptHostEvent {}

impl ScriptHostEvent {
    pub fn new() -> Self {
        Self {}
    }
}
impl ScriptApiEvent for ScriptHostEvent {
    fn notifcation_mask(&self, id: SignalId) -> ScriptResult<()> {
        Ok(log::debug!("TODO: unsubscribe from {id}"))
    }
    fn notifcation_unmask(&self, id: SignalId) -> ScriptResult<()> {
        Ok(log::debug!("TODO: subscribe to {id}"))
    }

    fn all_notifications(&self) -> Self::SignalNames {
        Box::new(ScriptNotification::all_key_values().map(|(k, v)| (Cow::Borrowed(k), v))) as Box<_>
    }
    fn all_signals(&self) -> Self::SignalNames {
        Box::new(ScriptSignal::all_key_values().map(|(k, v)| (Cow::Borrowed(k), v))) as Box<_>
    }
    type SignalNames = Box<dyn Iterator<Item = (Cow<'static, str>, SignalId)>>;
}

#[derive(Debug)]
pub struct NotifyScript {
    pub id: SignalId,
    pub receiver: Option<EventTarget>,
    pub args: UntypedArgs,
}

#[derive(Debug)]
pub struct EventTarget {
    pub receiver: String,
    pub user_args: UntypedArgs,
}
pub enum UntypedArgs {
    Empty,
    Int(isize),
    Bool(bool),
    #[cfg(feature = "scripts-lua")]
    Lua(Box<dyn lua::IntoLuaMultiMut + Send>),
    #[cfg(todo)]
    #[cfg(feature = "paths-lua")]
    LuaSrc(String),
}
impl fmt::Debug for UntypedArgs {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut f = f.debug_tuple("UntypedArgs");
        match self {
            Self::Empty => f.field(&()),
            Self::Int(v) => f.field(&v),
            Self::Bool(v) => f.field(&v),
            #[cfg(feature = "scripts-lua")]
            Self::Lua(..) => f.field(&"LuaValue"),
            #[cfg(todo)]
            #[cfg(feature = "scripts-lua")]
            Self::LuaSrc(src) => f.field(src),
        }
        .finish()
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScriptNotificationCategory {
    Lifecycle,
    Interaction,
    Introspection,
    #[cfg(feature = "paths-lua")]
    Pathing,
    #[cfg(feature = "paths-lua")]
    PathingMarker,
    #[cfg(feature = "extension-nexus")]
    NexusCallback,
}
#[derive(
    Debug,
    Copy,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    FromRepr,
    EnumCount,
    VariantArray,
    strum::IntoStaticStr,
)]
#[repr(u32)]
pub enum ScriptNotification {
    Exit = 1,
    Nop,
    MenuClick,
    GameplayKeybind,
    #[cfg(feature = "scripts-lua")]
    DebugWatchExport,
    #[cfg(feature = "paths-lua")]
    PathingTick,
    #[cfg(feature = "paths-lua")]
    PathingTickMarker,
    #[cfg(feature = "paths-lua")]
    PathingLoadMarker,
    #[cfg(feature = "paths-lua")]
    PathingFilterMarker,
    #[cfg(feature = "paths-lua")]
    PathingTrigger,
    #[cfg(feature = "paths-lua")]
    PathingFocus,
    #[cfg(feature = "paths-lua")]
    PathingUnfocus,
    #[cfg(feature = "paths-lua")]
    PathingMapExit,
    #[cfg(feature = "extension-nexus")]
    NexusEvent,
    #[cfg(feature = "extension-nexus")]
    NexusTextureLoad,
    #[cfg(feature = "extension-nexus")]
    NexusFontLoad,
    #[cfg(feature = "extension-nexus")]
    NexusRenderOverlayPre,
    #[cfg(feature = "extension-nexus")]
    NexusRenderOverlay,
    #[cfg(feature = "extension-nexus")]
    NexusRenderOverlayPost,
    #[cfg(feature = "extension-nexus")]
    NexusRenderOptions,
    #[cfg(feature = "extension-nexus")]
    NexusRenderQuickAccess,
    #[cfg(feature = "extension-nexus")]
    NexusWndProc,
    #[cfg(feature = "extension-nexus")]
    NexusInputBindPress,
    #[cfg(feature = "extension-nexus")]
    NexusQuickAccessShortcut,
}
impl ScriptNotification {
    #[inline]
    pub fn to_repr(self) -> SignalId {
        self as _
    }
    #[inline]
    pub fn name(self) -> &'static str {
        self.into()
    }

    pub fn all_key_values() -> impl Iterator<Item = (&'static str, SignalId)> {
        Self::VARIANTS.iter().map(|v| (v.name(), v.to_repr()))
    }

    pub fn category(self) -> ScriptNotificationCategory {
        match self {
            Self::Exit | Self::Nop => ScriptNotificationCategory::Lifecycle,
            Self::MenuClick | Self::GameplayKeybind => ScriptNotificationCategory::Interaction,
            #[cfg(feature = "scripts-lua")]
            Self::DebugWatchExport => ScriptNotificationCategory::Introspection,
            #[cfg(feature = "paths-lua")]
            Self::PathingTick => ScriptNotificationCategory::Pathing,
            #[cfg(feature = "paths-lua")]
            | Self::PathingTickMarker
            | Self::PathingLoadMarker
            | Self::PathingFilterMarker
            | Self::PathingTrigger
            | Self::PathingFocus
            | Self::PathingUnfocus => ScriptNotificationCategory::PathingMarker,
            #[cfg(feature = "paths-lua")]
            Self::PathingMapExit => ScriptNotificationCategory::Lifecycle,
            #[cfg(feature = "extension-nexus")]
            | Self::NexusEvent
            | Self::NexusTextureLoad
            | Self::NexusFontLoad
            | Self::NexusRenderOverlayPre
            | Self::NexusRenderOverlay
            | Self::NexusRenderOverlayPost
            | Self::NexusRenderOptions
            | Self::NexusRenderQuickAccess
            | Self::NexusWndProc
            | Self::NexusInputBindPress
            | Self::NexusQuickAccessShortcut => ScriptNotificationCategory::NexusCallback,
        }
    }
}
#[cfg(feature = "scripts-lua")]
impl IntoLua for ScriptNotification {
    fn into_lua(self, _lua: &Lua) -> LuaResult<LuaValue> {
        Ok(LuaValue::Integer(self.to_repr() as _))
    }
}
#[cfg(feature = "scripts-lua")]
impl FromLua for ScriptNotification {
    fn from_lua(value: LuaValue, lua: &Lua) -> LuaResult<Self> {
        FromLua::from_lua(value, lua).and_then(|v| {
            Self::from_repr(v)
                .ok_or_else(|| lua::to_lua_error(format_err!("unrecognized notification {v}")))
        })
    }
}
#[derive(
    Debug,
    Copy,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    FromRepr,
    EnumCount,
    VariantArray,
    strum::IntoStaticStr,
)]
#[repr(u32)]
pub enum ScriptSignal {
    Started = ScriptNotification::COUNT as SignalId + 1,
    Pending,
    Ended,
    /// request immediate resume (or signalled oob) since there may be something to yield
    Resume,
    #[cfg(todo = "unnecessary")]
    RegisterTick,
    /// decision to filter marker as a response to [ScriptNotification::PathingFilterMarker]
    #[cfg(todo = "unnecessary")]
    PathingHideMarker,
    /// decision to unfilter marker as a response to [ScriptNotification::PathingFilterMarker]
    #[cfg(todo = "unnecessary")]
    PathingShowMarker,
}
impl ScriptSignal {
    #[inline]
    pub fn to_repr(self) -> u32 {
        self as _
    }
    #[inline]
    pub fn name(self) -> &'static str {
        self.into()
    }

    pub fn all_key_values() -> impl Iterator<Item = (&'static str, u32)> {
        Self::VARIANTS.iter().map(|v| (v.name(), v.to_repr()))
    }
}
#[cfg(feature = "scripts-lua")]
impl IntoLua for ScriptSignal {
    fn into_lua(self, _lua: &Lua) -> LuaResult<LuaValue> {
        Ok(LuaValue::Integer(self.to_repr() as _))
    }
}
#[cfg(feature = "scripts-lua")]
impl FromLua for ScriptSignal {
    fn from_lua(value: LuaValue, lua: &Lua) -> LuaResult<Self> {
        FromLua::from_lua(value, lua).and_then(|v| {
            Self::from_repr(v).ok_or_else(|| lua::to_lua_error(format_err!("unrecognized signal {v}")))
        })
    }
}
