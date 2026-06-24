use {
    crate::controller::script::event,
    core::{fmt, mem},
    taimi_hoard::{
        lazyfmt,
        loc::{locator_ns, Locator, NamespacePivotFrom, NamespaceTryConvTo},
    },
    num_traits::AsPrimitive,
};
#[cfg(feature = "paths")]
use {
    crate::controller::pathing::registry::{PackRegistryNs, PackPath, PackIndex},
    taimi_meta::packs::id::{MarkerIndex, MarkerId},
};
#[cfg(not(feature = "paths"))]
type PackIndex = u16;

pub use taimi_meta::packs::id::{
    MarkerPath as EventArgPath,
    MarkerIndex as ScriptEventArg,
    MarkerId as ScriptEventId,
};

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct ScriptIndex {
    pub index: u32,
}
pub type ScriptIndexNamespace = u32;
impl ScriptIndex {
    pub const UNK: Self = Self::new(Self::NS_MISC, 0);
    pub const GLOBAL: Self = Self::WILDCARD_MISC;
    pub const GLOBAL_PATH: &'static ScriptBroadcastPath = ScriptBroadcastPath::marker_static();
    pub const WILDCARD_MISC: Self = Self::wildcard_with_namespace(Self::NS_MISC);
    pub const WILDCARD_PLUG: Self = Self::wildcard_with_namespace(Self::NS_PLUG);
    pub const WILDCARD_PACK: Self = Self::wildcard_with_namespace(Self::NS_PACK);
    pub const NS_MISC: ScriptIndexNamespace = 0x00000000;
    pub const NS_PLUG: ScriptIndexNamespace = 0x20000000;
    pub const NS_PACK: ScriptIndexNamespace = 0x40000000;
    pub const NS_PACK_LUA: ScriptIndexNamespace = Self::NS_PACK;
    pub const MASK_NS: ScriptIndexNamespace = 0xe0000000;
    pub const MASK_INDEX: u32 = 0x000fffff;
    pub const MASK_UNASSIGNED: u32 = 0x1ff00000;
    pub const SHIFT_NS: u32 = Self::MASK_NS.leading_zeros();
    pub const SHIFT_UNASSIGNED: u32 = Self::MASK_UNASSIGNED.leading_zeros();
    pub const SHIFT_INDEX: u32 = match Self::MASK_INDEX {
        #[cfg(todo = "unnecessary")]
        mask => mask.leading_zeros(),
        _ => 0,
    };

    #[inline(always)]
    pub const fn from_repr(index: u32) -> Self {
        Self { index }
    }
    #[inline(always)]
    pub const fn new(ns: ScriptIndexNamespace, index: u32) -> Self {
        Self::from_repr(ns | index)
    }
    #[inline(always)]
    pub const fn with_plug_index(index: PlugIndex) -> Self {
        Self::from_repr(Self::NS_PLUG | index as u32)
    }
    #[inline(always)]
    pub const fn with_pack_index(index: PackScriptIndex) -> Self {
        Self::from_repr(Self::NS_PACK | index as u32)
    }
    #[inline(always)]
    pub const fn with_pack(path: PackScriptPath) -> Self {
        Self::with_pack_index(path.path)
    }
    #[inline(always)]
    #[cfg(feature = "paths")]
    pub const fn for_pack(path: PackPath) -> Self {
        Self::with_pack(PackScriptPath::new_path(path.path))
    }
    #[inline(always)]
    pub const fn with_plug(path: PlugPath) -> Self {
        Self::with_plug_index(path.path)
    }
    pub const fn wildcard_with_namespace(ns: u32) -> Self {
        Self::new(Self::NS_MISC, 1 + ns >> Self::SHIFT_NS)
    }
    #[inline(always)]
    pub const fn namespace(self) -> u32 {
        self.index & Self::MASK_NS
    }
    #[inline(always)]
    pub const fn namespace_index(self) -> u32 {
        self.namespace() >> Self::SHIFT_NS
    }
    #[inline(always)]
    pub const fn index(self) -> u32 {
        self.index & Self::MASK_INDEX
    }
    pub const fn get_wildcard_namespace(self) -> u32 {
        (self.index() << Self::SHIFT_NS) - 1
    }
    #[inline(always)]
    pub fn get_plug_index(self) -> PlugPath {
        PlugPath::new_path(self.index() as PlugIndex)
    }
    #[inline(always)]
    pub fn get_pack_index(self) -> PackScriptPath {
        PackScriptPath::new_path(self.index() as PackScriptIndex)
    }

    /// ```
    /// # let (idx, other) = (ScriptIndex::UNK, ScriptIndex::UNK);
    /// let (lhs, rhs) = idx.matcher(other);
    /// if lhs.matches(rhs) {
    ///   // ...
    /// }
    /// ```
    pub fn matcher(self, other: Self) -> (Self, Self) {
        match (self, other) {
            (l, r) if r.namespace() == Self::NS_MISC => (r, l),
            v => v,
        }
    }
    /// [Self::matcher] first
    pub fn matches(self, rhs: Self) -> bool {
        match self {
            Self::GLOBAL => true,
            #[cfg(todo = "unnecessary")]
            Self::UNK => false,
            l => {
                let rns = rhs.namespace();
                match l.namespace() {
                    Self::NS_MISC if l.get_wildcard_namespace() == rns =>
                        true,
                    lns => lns == rns && l.index() == rhs.index(),
                }
            },
            #[cfg(todo)]
            Self::WILDCARD_PACK if rhs.namespace() == Self::NS_PACK => (),
            #[cfg(todo)]
            Self::WILDCARD_PLUG if rhs.namespace() == Self::NS_PLUG => (),
            #[cfg(todo)]
            l => l.repr() == rhs.repr(),
        }
    }
    /// [Self::matcher] first
    pub fn matches_namespace(self, rhs: Self) -> bool {
        let rns = rhs.namespace();
        match self {
            Self::GLOBAL => true,
            Self::WILDCARD_PLUG => rns == Self::NS_PLUG,
            Self::WILDCARD_PACK => rns == Self::NS_PACK,
            l => l.namespace() == rns,
        }
    }
    pub fn is_wildcard(self) -> bool {
        matches!(self, Self::WILDCARD_MISC | Self::WILDCARD_PLUG | Self::WILDCARD_PACK)
    }

    #[inline(always)]
    pub const fn is_empty(self) -> bool {
        matches!(self, Self::UNK)
    }
    #[inline(always)]
    pub const fn or_empty(self) -> Option<Self> {
        match self.is_empty() {
            true => None,
            false => Some(self),
        }
    }

    #[inline(always)]
    pub const fn to_path(self) -> ScriptPath {
        ScriptPath::new_path(self)
    }
}
impl From<ScriptIndex> for u32 {
    #[inline(always)]
    fn from(v: ScriptIndex) -> Self { v.index }
}
impl<T> AsPrimitive<T> for ScriptIndex where
    T: Copy + 'static,
    u32: AsPrimitive<T>,
{
    #[inline(always)]
    fn as_(self) -> T { self.index.as_() }
}
impl fmt::Display for ScriptIndex {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.namespace() {
            Self::NS_PLUG => {
                let p: PlugPath = PlugPath::new_path(self.index() as PlugIndex);
                fmt::Display::fmt(&p, f)
            },
            #[cfg(all(feature = "paths", todo = "unnecessary"))]
            Self::NS_PACK => {
                let p: PackPath = PackPath::new_path(self.index() as _);
                fmt::Display::fmt(&p, f)
            },
            Self::NS_PACK_LUA => {
                let p: PackPath = PackPath::new_path(self.index() as PackIndex);
                fmt::Display::fmt(&p, f)
            },
            Self::NS_MISC => match *self {
                Self::UNK => f.write_str("<unk>"),
                Self::GLOBAL => fmt::Display::fmt(&Self::GLOBAL_PATH.root, f),
                _ => fmt::Display::fmt(&self.index, f),
            },
            ns => {
                let p = Locator::with_parts(lazyfmt::fmt_fn(|f| write!(f, "{ns:X}")), self.index());
                fmt::Display::fmt(&p, f)
            },
        }
    }
}
locator_ns! {
    pub struct LoadedScriptNs;
    impl LocatorNamespace {
        pub index LoadedScriptIndex = ScriptIndex;
        pub path LoadedScriptPath;
        fn fmt(&self, f) {
            f.write_str("script")
        }
    }
}
pub type ScriptPath = LoadedScriptPath;
impl LoadedScriptNs {
    pub fn ns1_for(idx: ScriptIndex) -> u8 {
        match idx.namespace() {
            ScriptIndex::NS_PACK => MarkerId::NS1_PACK,
            _ => 0,
        }
    }
    pub fn ns0_for(idx: ScriptIndex) -> u8 {
        match idx.namespace() {
            ScriptIndex::NS_PACK => MarkerId::NS0_MARKER,
            _ => 0,
        }
    }
    pub fn index1_for(idx: ScriptIndex) -> u16 {
        match idx.namespace() {
            ScriptIndex::NS_MISC => idx.index() as _,
            _ => 0,
        }
    }
    pub fn index2_for(idx: ScriptIndex) -> u16 {
        match idx.namespace() {
            ScriptIndex::NS_MISC => u16::MAX,
            _ => idx.index() as _,
        }
    }
    const BITS_INDEX3: u32 = 56;
    const BITS_NAMESPACE: u32 = mem::size_of::<ScriptIndexNamespace>() as u32 * 8 - ScriptIndex::SHIFT_NS - ScriptIndex::MASK_NS.leading_zeros();
    const SHIFT_NAMESPACE_INDEX3: u32 = Self::BITS_INDEX3 - ScriptIndex::SHIFT_NS - Self::BITS_NAMESPACE;
    pub fn index3_for(idx: ScriptIndex, event: ScriptEventPath) -> u64 {
        let ns = (idx.namespace() as u64) << Self::SHIFT_NAMESPACE_INDEX3;
        let event_id = event.path as u64;
        ns | event_id
    }
    pub fn marker_notif_to_id(script: PackScriptPath, event: ScriptEventPath, arg: ScriptEventArg) -> ScriptEventId {
        Self::notif_to_id(script.pivot_from(), event, arg)
    }
    pub fn notif_to_id(script: ScriptPath, event: ScriptEventPath, arg: ScriptEventArg) -> ScriptEventId {
        let ns0 = Self::ns0_for(script.path);
        let ns1 = Self::ns1_for(script.path);
        let index0 = arg.repr();
        let index1 = Self::index1_for(script.path);
        let index2 = Self::index2_for(script.path);
        let index3 = Self::index3_for(script.path, event);
        MarkerId::with_parts(ns0, ns1, index0, index1, index2, index3)
    }
    pub fn id_to_notif(id: &ScriptEventId) -> (ScriptPath, ScriptEventIndex, ScriptEventArg) {
        let (_ns0, ns1) = id.ns01();
        let index3 = id.index3();
        let ev = index3 as ScriptEventIndex;
        let arg = Self::arg_from_repr(id.index0());
        match ns1 {
            MarkerId::NS1_PACK => return (id.get_marker_pack_path().pivot_from(), ev, arg),
            _ => (),
        }
        let ns = (index3 >> Self::SHIFT_NAMESPACE_INDEX3) as ScriptIndexNamespace & ScriptIndex::MASK_NS;
        let path = match ns {
            ns @ ScriptIndex::NS_MISC => ScriptPath::new_path(ScriptIndex::new(ns, id.index1() as _)),
            ns => ScriptPath::new_path(ScriptIndex::new(ns, id.index2() as _)),
        };
        (path, ev, arg)
    }
    #[inline]
    pub fn id_to_notif_arg(id: &ScriptEventId) -> ScriptEventArg {
        MarkerIndex::from_repr(id.index0())
    }
    pub fn id_to_notif_marker(id: &ScriptEventId) -> ScriptEventArg {
        let (_ns0, ns1) = id.ns01();
        match ns1 {
            MarkerId::NS1_PACK => Self::id_to_notif_arg(id),
            _ => MarkerIndex::UNK,
        }
    }
    pub fn id_to_notif_event(id: &ScriptEventId) -> ScriptEventIndex {
        id.index3() as ScriptEventIndex
    }
    pub const ARG_UNK: ScriptEventArg = MarkerIndex::UNK;
    /// TODO?
    pub const ARG_WILD: ScriptEventArg = Self::ARG_UNK;
    const ARG_UNK_REPR: u32 = Self::ARG_UNK.repr();

    #[inline(always)]
    pub fn arg_is_empty(arg: ScriptEventArg) -> bool {
        match arg {
            a => matches!(a, Self::ARG_UNK),
            #[cfg(todo = "unnecessary")]
            a => Self::arg32_is_empty(a.repr()),
        }
    }
    #[inline]
    pub fn arg32_is_empty(arg: u32) -> bool {
        matches!(arg, 0 | Self::ARG_UNK_REPR)
    }
    pub fn arg_from_repr(arg: u32) -> ScriptEventArg {
        match arg {
            0 => Self::ARG_UNK,
            a => MarkerIndex::from_repr(a),
        }
    }
}
#[cfg(feature = "paths")]
impl NamespacePivotFrom<PackRegistryNs, PackIndex> for LoadedScriptNs {
    type NsPivotFromPath = ScriptIndex;
    #[inline]
    fn loc_pivot_from(path: PackPath) -> Locator<Self, Self::NsPivotFromPath> {
        Locator::new_path(ScriptIndex::for_pack(path))
    }
}
#[cfg(feature = "paths")]
impl NamespaceTryConvTo<ScriptIndex, PackPath> for LoadedScriptNs {
    fn try_conv_to(
        path: Locator<Self, ScriptIndex>,
    ) -> Option<PackPath> {
        match path.path.namespace() {
            ScriptIndex::NS_PACK => Some(PackPath::new_path(path.path.index() as PackScriptIndex)),
            _ => None,
        }
    }
}
impl NamespaceTryConvTo<ScriptIndex, PackScriptPath> for LoadedScriptNs {
    fn try_conv_to(
        path: Locator<Self, ScriptIndex>,
    ) -> Option<PackScriptPath> {
        match path.path.namespace() {
            ScriptIndex::NS_PACK => Some(PackScriptPath::new_path(path.path.index() as PackScriptIndex)),
            _ => None,
        }
    }
}
impl NamespaceTryConvTo<ScriptIndex, PlugPath> for LoadedScriptNs {
    fn try_conv_to(
        path: Locator<Self, ScriptIndex>,
    ) -> Option<PlugPath> {
        match path.path.namespace() {
            ScriptIndex::NS_PLUG => Some(PlugPath::new_path(path.path.index() as PlugIndex)),
            _ => None,
        }
    }
}

locator_ns! {
    pub struct ScriptBroadcastNs;
    impl LocatorNamespace {
        pub index ScriptBroadcast = ();
        pub path ScriptBroadcastPath;
        fn fmt(&self, f) {
            f.write_str("all")
        }
    }
}
impl NamespacePivotFrom<ScriptBroadcastNs, ScriptBroadcast> for LoadedScriptNs {
    type NsPivotFromPath = ScriptIndex;
    #[inline]
    fn loc_pivot_from(_: ScriptBroadcastPath) -> Locator<Self, Self::NsPivotFromPath> {
        Locator::new_path(ScriptIndex::GLOBAL)
    }
}

locator_ns! {
    /// think AUTOINCREMENT
    pub struct PlugScriptNs;
    impl LocatorNamespace {
        pub index PlugIndex = u16;
        pub path PlugPath;
        fn fmt(&self, f) {
            f.write_str("plug")
        }
    }
}
impl NamespacePivotFrom<PlugScriptNs, PlugIndex> for LoadedScriptNs {
    type NsPivotFromPath = ScriptIndex;
    #[inline]
    fn loc_pivot_from(path: PlugPath) -> Locator<Self, Self::NsPivotFromPath> {
        Locator::new_path(ScriptIndex::with_plug(path))
    }
}

locator_ns! {
    pub struct PackScriptNs;
    impl LocatorNamespace {
        pub index PackScriptIndex = PackIndex;
        pub path PackScriptPath;
        fn fmt(&self, f) {
            f.write_str("pack.lua")
        }
    }
}
#[cfg(feature = "paths")]
impl PartialEq<PackRegistryNs> for PackScriptNs {
    #[inline(always)]
    fn eq(&self, _: &PackRegistryNs) -> bool { true }
}
#[cfg(feature = "paths")]
impl PartialEq<PackScriptNs> for PackRegistryNs {
    #[inline(always)]
    fn eq(&self, _: &PackScriptNs) -> bool { true }
}
#[cfg(feature = "paths")]
impl NamespacePivotFrom<PackRegistryNs, PackIndex> for PackScriptNs {
    type NsPivotFromPath = PackScriptIndex;
    #[inline]
    fn loc_pivot_from(path: PackPath) -> Locator<Self, Self::NsPivotFromPath> {
        Locator::new_path(path.path)
    }
}
#[cfg(feature = "paths")]
impl NamespacePivotFrom<PackScriptNs, PackScriptIndex> for PackRegistryNs {
    type NsPivotFromPath = PackIndex;
    #[inline]
    fn loc_pivot_from(path: PackScriptPath) -> Locator<Self, Self::NsPivotFromPath> {
        Locator::new_path(path.path)
    }
}
impl NamespacePivotFrom<PackScriptNs, PackScriptIndex> for LoadedScriptNs {
    type NsPivotFromPath = ScriptIndex;
    #[inline]
    fn loc_pivot_from(path: PackScriptPath) -> Locator<Self, Self::NsPivotFromPath> {
        Locator::new_path(ScriptIndex::with_pack(path))
    }
}
locator_ns! {
    pub struct ScriptEventNs;
    impl LocatorNamespace {
        //index ScriptEventIndex = event::SignalId;
        pub index ScriptEventIndex = u16;
        pub path ScriptEventPath;
        fn fmt(&self, f) {
            f.write_str("signal")
        }
    }
}
impl NamespacePivotFrom<ScriptEventNs, event::ScriptNotification> for ScriptEventNs {
    type NsPivotFromPath = ScriptEventIndex;
    #[inline]
    fn loc_pivot_from(path: ScriptNotificationPath) -> Locator<Self, Self::NsPivotFromPath> {
        Locator::new_path(path.path.to_repr() as ScriptEventIndex)
    }
}
impl NamespacePivotFrom<ScriptEventNs, event::ScriptSignal> for ScriptEventNs {
    type NsPivotFromPath = ScriptEventIndex;
    #[inline]
    fn loc_pivot_from(path: ScriptSignalPath) -> Locator<Self, Self::NsPivotFromPath> {
        Locator::new_path(path.path.to_repr() as ScriptEventIndex)
    }
}
pub type ScriptNotificationPath = Locator<ScriptEventNs, event::ScriptNotification>;
pub type ScriptSignalPath = Locator<ScriptEventNs, event::ScriptSignal>;
pub type MarkerNotificationPath<P = EventArgPath> = Locator<ScriptNotificationPath, P>;
pub type PackScriptMarkerEventPath = Locator<ScriptEventPath, EventArgPath>;
pub type NotificationDestination<R = ScriptPath, E = ScriptEventIndex> = Locator<R, ScriptEventPath<E>>;
pub type MarkerDestination<R = ScriptPath, E = event::ScriptNotification, T = EventArgPath> = Locator<R, Locator<ScriptEventPath<E>, T>>;

#[cfg(todo)]
impl DebugPathDisplayOrWhateverItsCalled for ScriptPath {}
