use std::{borrow::Borrow, fmt, iter, mem};
use uuid::Uuid;
use {
    taimi_hoard::loc::{locator_ns, Locator, NamespacePivotFrom, NamespaceTryConvTo},
    crate::packs::{
        CategoryIndex, CategoryPath, MapIndex, PackIndex, PackMapPath, PackPath, PackRegistryNs, PoiIndex, PoiPath, TrailIndex, TrailPath, TrailSectionIndex, TrailSectionPath, SectionOfTrail,
        PackPoiNs, PackTrailNs,
    },
};
use uuid::Uuid as Guid;
#[cfg(todo)]
use taimi_pack::attributes::keys::Guid;

locator_ns! {
    pub struct PackMarkerNs;
    impl LocatorNamespace {
        index PackMarkerIndex = MarkerIndex;
        pub path MarkerPath;
        fn fmt(&self, f) {
            f.write_str("pack/marker")
        }
    }
}
pub type MarkerIndexNamespace = u32;
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct MarkerIndex {
    pub index: u32,
}
impl MarkerIndex {
    pub const UNK: Self = Self::new_invalid(Self::NS_UNK);
    pub const INDEX_MASK: u32 = 0x1fff_ffff;
    pub const INDEX_INVALID: u32 = Self::INDEX_MASK;
    pub const INDEX_MAX: u32 = Self::INDEX_MASK - 1;
    pub const INDEX_MAX_POI: u32 = Self::max_index_from(PoiIndex::MAX as u32);
    pub const INDEX_MAX_CAT: u32 = Self::max_index_from(CategoryIndex::MAX as u32);
    pub const INDEX_MAX_TRAIL: u32 = Self::max_index_from(TrailIndex::MAX as u32);
    pub const EXTRA_INVALID_TRAIL: u32 = (Self::INDEX_MASK >> Self::EXTRA_MASK_TRAIL.trailing_zeros());
    pub const EXTRA_MAX_TRAIL: u32 = Self::EXTRA_INVALID_TRAIL - 1;
    pub const EXTRA_MASK_TRAIL: u32 = Self::INDEX_MASK & (!Self::INDEX_MAX_TRAIL);
    pub const EXTRA_SHIFT_TRAIL: u32 = Self::INDEX_MAX_TRAIL.trailing_ones();
    pub const NS_MASK: MarkerIndexNamespace = 0xe000_0000;
    pub const NS_UNK: MarkerIndexNamespace = 0x0000_0000;
    pub const NS_POI: MarkerIndexNamespace = 0x2000_0000;
    pub const NS_TRAIL: MarkerIndexNamespace = 0x4000_0000;
    pub const NS_CAT: MarkerIndexNamespace = 0x6000_0000;

    #[inline(always)]
    pub const fn repr(self) -> u32 {
        self.index
    }
    #[inline(always)]
    pub const fn from_repr(index: u32) -> Self {
        Self { index }
    }
    pub const fn with_parts(ns: MarkerIndexNamespace, index: u32) -> Self {
        Self::from_repr(ns | index)
    }
    pub const fn new_invalid(ns: MarkerIndexNamespace) -> Self {
        Self::with_parts(ns, Self::INDEX_INVALID)
    }
    #[inline(always)]
    pub fn new<I: Into<u32>>(ns: MarkerIndexNamespace, index: I) -> Self {
        Self::with_parts(ns, index.into())
    }
    pub const fn with_poi(poi: PoiIndex) -> Self {
        Self::with_parts(Self::NS_POI, poi as u32)
    }
    pub const fn with_trail(trail: TrailIndex) -> Self {
        Self::with_parts(Self::NS_TRAIL, trail as u32)
    }
    pub const fn with_category(cat: CategoryIndex) -> Self {
        Self::with_parts(Self::NS_CAT, cat as u32)
    }
    pub fn with_trail_section(trail: TrailIndex, section: TrailSectionIndex) -> Self {
        let section = ((section as u32) + 1).min(Self::EXTRA_MAX_TRAIL);
        let extra = Self::new_extra_for(Self::NS_TRAIL, section as u32);
        Self::with_parts(Self::NS_TRAIL, trail as u32 | extra)
    }

    pub const fn namespace(self) -> MarkerIndexNamespace {
        self.repr() & Self::NS_MASK
    }
    pub const fn index(self) -> u32 {
        self.repr() & Self::INDEX_MASK
    }

    pub fn variant(self) -> MarkerIndexVariant {
        MarkerIndexVariant::from_index(self)
    }

    /// Dead space
    pub const fn index_extra(self) -> u32 {
        match self.namespace() {
            Self::NS_TRAIL => self.repr() & Self::EXTRA_MASK_TRAIL,
            _ => 0,
        }
    }
    pub const fn extra(self) -> u32 {
        match self.namespace() {
            Self::NS_TRAIL => (self.repr() & Self::EXTRA_MASK_TRAIL) >> Self::EXTRA_SHIFT_TRAIL,
            _ => 0,
        }
    }
    pub fn trail_section_unchecked(&self) -> TrailSectionIndex {
        let extra = (self.repr() & Self::EXTRA_MASK_TRAIL) >> Self::EXTRA_SHIFT_TRAIL;
        let section = extra as TrailSectionIndex;
        section.wrapping_sub(1)
    }

    pub const fn new_extra_for(ns: MarkerIndexNamespace, v: u32) -> u32 {
        match ns {
            Self::NS_TRAIL => v << Self::EXTRA_SHIFT_TRAIL,
            _ => 0,
        }
    }
    pub const fn try_index_from(index: u32) -> Option<u32> {
        match index {
            index @ 0..=Self::INDEX_MAX =>
                Some(index),
            _ => None,
        }
    }
    const fn max_index_from(index: u32) -> u32 {
        match Self::try_index_from(index) {
            Some(i) => i,
            None => Self::INDEX_MAX,
        }
    }
}
impl From<PoiPath> for MarkerIndex {
    fn from(i: PoiPath) -> Self {
        Self::with_poi(i.path)
    }
}
impl From<CategoryPath> for MarkerIndex {
    fn from(i: CategoryPath) -> Self {
        Self::with_category(i.path)
    }
}
impl From<TrailPath> for MarkerIndex {
    fn from(i: TrailPath) -> Self {
        Self::with_trail(i.path)
    }
}
impl From<TrailSectionPath<TrailPath>> for MarkerIndex {
    fn from(i: TrailSectionPath<TrailPath>) -> Self {
        let trail = i.root.path;
        let section = i.path;
        Self::with_trail_section(trail, section)
    }
}
impl<N> From<MarkerPath<N>> for MarkerIndex {
    fn from(i: MarkerPath<N>) -> Self {
        i.path
    }
}
impl fmt::Display for MarkerIndex {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.variant() {
            MarkerIndexVariant::Trail(i) => {
                let p: TrailPath = TrailPath::with_path(i);
                fmt::Display::fmt(&p, f)
            },
            MarkerIndexVariant::TrailSection(i, section) => {
                let t: TrailPath = TrailPath::with_path(i);
                let s: TrailSectionPath = TrailSectionPath::with_path(section);
                let p = t.rel(s.path);
                fmt::Display::fmt(&p, f)
            },
            MarkerIndexVariant::Poi(i) => {
                let p: PoiPath = PoiPath::with_path(i);
                fmt::Display::fmt(&p, f)
            },
            MarkerIndexVariant::Category(i) => {
                let p: CategoryPath = CategoryPath::with_path(i);
                fmt::Display::fmt(&p, f)
            },
            _ => fmt::Display::fmt(&self.index, f),
        }
    }
}
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MarkerIndexVariant {
    Trail(TrailIndex),
    TrailSection(TrailIndex, TrailSectionIndex),
    Poi(PoiIndex),
    Category(CategoryIndex),
    Invalid(MarkerIndexNamespace),
    Unknown(MarkerIndex),
}
impl MarkerIndexVariant {
    pub fn from_index(index: MarkerIndex) -> Self {
        match (index.namespace(), index.index()) {
            (MarkerIndex::NS_POI, poi @ 0..=MarkerIndex::INDEX_MAX_POI) =>
                Self::Poi(poi as PoiIndex),
            (MarkerIndex::NS_CAT, cat @ 0..=MarkerIndex::INDEX_MAX_CAT) =>
                Self::Category(cat as CategoryIndex),
            (MarkerIndex::NS_TRAIL, trail @ 0..=MarkerIndex::INDEX_MAX_TRAIL) =>
                Self::Trail(trail as TrailIndex),
            (MarkerIndex::NS_TRAIL, trail) => {
                let trail = (trail & !MarkerIndex::EXTRA_MASK_TRAIL) as TrailIndex;
                Self::TrailSection(trail, index.trail_section_unchecked())
            },
            (ns, MarkerIndex::INDEX_INVALID) =>
                Self::Invalid(ns),
            _ =>
                Self::Unknown(index),
        }
    }
}
impl From<TrailPath> for MarkerIndexVariant {
    fn from(i: TrailPath) -> Self {
        Self::Trail(i.path)
    }
}
impl From<PoiPath> for MarkerIndexVariant {
    fn from(i: PoiPath) -> Self {
        Self::Poi(i.path)
    }
}
impl From<CategoryPath> for MarkerIndexVariant {
    fn from(i: CategoryPath) -> Self {
        Self::Category(i.path)
    }
}
impl<N> From<MarkerPath<N>> for MarkerIndexVariant {
    fn from(i: MarkerPath<N>) -> Self {
        Self::from_index(i.path)
    }
}
impl From<MarkerIndex> for MarkerIndexVariant {
    fn from(i: MarkerIndex) -> Self {
        Self::from_index(i)
    }
}

impl NamespacePivotFrom<PackPoiNs, PoiIndex> for PackMarkerNs {
    type NsPivotFromPath = MarkerIndex;
    fn loc_pivot_from(path: PoiPath) -> Locator<Self, Self::NsPivotFromPath> {
        Locator::with_path(MarkerIndex::with_poi(path.path))
    }
}
impl NamespacePivotFrom<PackTrailNs, TrailIndex> for PackMarkerNs {
    type NsPivotFromPath = MarkerIndex;
    fn loc_pivot_from(path: TrailPath) -> Locator<Self, Self::NsPivotFromPath> {
        Locator::with_path(MarkerIndex::with_trail(path.path))
    }
}
impl NamespacePivotFrom<TrailPath, TrailSectionPath> for PackMarkerNs {
    type NsPivotFromPath = MarkerIndex;
    fn loc_pivot_from(path: SectionOfTrail) -> Locator<Self, Self::NsPivotFromPath> {
        Locator::with_path(MarkerIndex::with_trail_section(path.root.path, path.path.path))
    }
}
impl NamespaceTryConvTo<MarkerIndex, TrailPath> for PackMarkerNs {
    fn try_conv_to(path: Locator<Self, MarkerIndex>) -> Option<TrailPath> {
        match path.path.variant() {
            | MarkerIndexVariant::Trail(i)
            | MarkerIndexVariant::TrailSection(i, _)
                => Some(TrailPath::with_path(i)),
            _ => None,
        }
    }
}
impl NamespaceTryConvTo<MarkerIndex, TrailSectionPath> for PackMarkerNs {
    fn try_conv_to(path: Locator<Self, MarkerIndex>) -> Option<TrailSectionPath> {
        match path.path.variant() {
            MarkerIndexVariant::TrailSection(_, section)
                => Some(TrailSectionPath::with_path(section)),
            _ => None,
        }
    }
}
impl NamespaceTryConvTo<MarkerIndex, SectionOfTrail> for PackMarkerNs {
    fn try_conv_to(path: Locator<Self, MarkerIndex>) -> Option<SectionOfTrail> {
        match path.path.variant() {
            MarkerIndexVariant::TrailSection(i, section)
                => Some(TrailPath::with_path(i).rel(TrailSectionPath::with_path(section))),
            _ => None,
        }
    }
}
impl NamespaceTryConvTo<MarkerIndex, Locator<TrailPath, Option<TrailSectionPath>>> for PackMarkerNs {
    fn try_conv_to(path: Locator<Self, MarkerIndex>) -> Option<Locator<TrailPath, Option<TrailSectionPath>>> {
        match path.path.variant() {
            MarkerIndexVariant::Trail(i) =>
                Some(TrailPath::with_path(i).rel(None)),
            MarkerIndexVariant::TrailSection(i, section)
                => Some(TrailPath::with_path(i).rel(Some(TrailSectionPath::with_path(section)))),
            _ => None,
        }
    }
}
impl NamespaceTryConvTo<MarkerIndex, PoiPath> for PackMarkerNs {
    fn try_conv_to(path: Locator<Self, MarkerIndex>) -> Option<PoiPath> {
        match path.path.variant() {
            MarkerIndexVariant::Poi(i) =>
                Some(PoiPath::with_path(i)),
            _ => None,
        }
    }
}
impl NamespaceTryConvTo<MarkerIndex, CategoryPath> for PackMarkerNs {
    fn try_conv_to(path: Locator<Self, MarkerIndex>) -> Option<CategoryPath> {
        match path.path.variant() {
            MarkerIndexVariant::Category(i) =>
                Some(CategoryPath::with_path(i)),
            _ => None,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct MarkerId {
    pub uuid: Uuid,
}

impl MarkerId {
    pub const EMPTY: Self = Self::with_uuid(Uuid::nil());
}

impl MarkerId {
    /// index(d1) represents a [MarkerIndex]
    pub const NS0_MARKER: u8 = 1;

    /// [PackRegistryNs]
    pub const NS1_REGISTRY: u8 = 1;
    /// [PackNs]
    pub const NS1_PACK: u8 = 2;
    pub const NS1_PACK_MAP: u8 = 3;

    pub const fn with_uuid(uuid: Uuid) -> Self {
        Self { uuid }
    }
    pub const fn from_uuid_ref(uuid: &Uuid) -> &Self {
        unsafe {
            mem::transmute(uuid)
        }
    }
    #[cfg(todo)]
    pub const fn with_guid(guid: Guid) -> Self {}
    #[cfg(todo)]
    pub const fn from_guid_ref(guid: &Guid) -> &Self {}

    /// any data present in `d3 & 0xf000` and `d4[0] & 0xc0` will be cleared
    ///
    /// See also: [Uuid::from_fields]
    pub fn with_uuidv8_fields(d1: u32, d2: u16, d3: u16, d4: &[u8; 8]) -> Self {
        let bytes = uuid::Builder::from_fields(d1, d2, d3, d4).into_uuid().into_bytes();
        let uuid = uuid::Builder::from_custom_bytes(bytes).into_uuid();
        Self::with_uuid(uuid)
    }

    /// ns0=2bit, ns1=4bit, index2=12bit, index3=56bit
    pub fn with_parts(ns0: u8, ns1: u8, index0: u32, index1: u16, index2: u16, index3: u64) -> Self {
        let d4 = {
            let mut d4 = (index3 << 8).to_le_bytes();
            let [out_ns01, ..] = &mut d4;
            *out_ns01 = (ns0 << 4) | ns1;
            d4
        };
        Self::with_uuidv8_fields(
            index0,
            index1,
            index2,
            &d4,
        )
    }
    pub fn for_marker<N: MarkerId1, P: Into<MarkerIndex>>(marker: Locator<N, P>) -> Self {
        let path = marker.path.into();
        let index0 = path.repr();
        let index1 = marker.root.index1();
        let index2 = marker.root.index2();
        let ns0 = Self::NS0_MARKER;
        let ns1 = marker.root.ns1();
        Self::with_parts(ns0, ns1, index0, index1, index2, 0)
    }

    pub fn ns01_8(&self) -> u8 {
        self.uuid.as_bytes()[8]
    }
    pub fn ns01(&self) -> (u8, u8) {
        let ns = self.ns01_8() & 0x3f;
        let ns0 = ns >> 4;
        let ns1 = ns & 0x0f;
        (ns0, ns1)
    }
    pub fn index0(&self) -> u32 {
        let bytes = self.uuid.as_bytes();
        let index0 = unsafe {
            &*(bytes as *const [u8; 16] as *const [u8; 4])
        };
        u32::from_ne_bytes(*index0)
            .swap_bytes()
    }
    pub fn index12(&self) -> (u16, u16) {
        let bytes = self.uuid.as_bytes();
        let (index1, index2) = unsafe {
            let bytes = bytes as *const [u8; 16] as *const [u8; 2];
            (
                &*bytes.add(2),
                &*bytes.add(3),
            )
        };
        let index1 = u16::from_ne_bytes(*index1).swap_bytes();
        let index2 = u16::from_ne_bytes(*index2).swap_bytes();
        (index1, index2)
    }
    pub fn index3(&self) -> u64 {
        let bytes = self.uuid.as_bytes();
        let index3 = unsafe {
            &*(bytes as *const [u8; 16] as *const [u8; 8]).add(1)
        };
        u64::from_ne_bytes(*index3)
            .swap_bytes() >> 8
    }

    pub fn marker_index(&self) -> Option<MarkerIndex> {
        match self.ns01() {
            (Self::NS0_MARKER, _ns1) =>
                Some(MarkerIndex::from_repr(self.index0())),
            _ => None,
        }
    }
    pub fn marker_path<N: FromMarkerId1>(&self) -> Option<MarkerPath<N>> {
        match self.ns01() {
            (_ns0, ns1) if ns1 == N::NS1 => self.marker_index().map(|path| {
                let (index1, index2) = self.index12();
                let root = N::from_index12(index1, index2);
                MarkerPath::with_parts(root, path)
            }),
            _ => None,
        }
    }

    fn new_uuidv3_namespace<'n, 'b, B: IntoIterator<Item = &'b [u8]>>(ns: &'n Uuid, bytes: B) -> Uuid where
        'n: 'b,
    {
        let bytes = iter::once(&ns.as_bytes()[..])
            .chain(bytes);
        let hash = hash_bytes_md5(bytes);
        let uuid = uuid::Builder::from_md5_bytes(hash).into_uuid();
        uuid
    }
    #[cfg(feature = "sha1_smol")]
    fn new_uuidv5_namespace<'n, 'b, B: IntoIterator<Item = &'b [u8]>>(ns: &'n Uuid, bytes: B) -> Uuid where
        'n: 'b,
    {
        let bytes = iter::once(&ns.as_bytes()[..])
            .chain(bytes);
        let hash = hash_bytes_sha1(bytes);
        let uuid = uuid::Builder::from_sha1_bytes(hash).into_uuid();
        uuid
    }
    fn new_uuid_namespace<'n, 'b, B: IntoIterator<Item = &'b [u8]>>(ns: &'n Uuid, bytes: B) -> Uuid where
        'n: 'b,
    {
        match ns {
            #[cfg(feature = "sha1_smol")]
            ns => Self::new_uuidv5_namespace(ns, bytes),
            #[cfg(not(feature = "sha1_smol"))]
            ns => Self::new_uuidv3_namespace(ns, bytes),
        }
    }

    pub fn new_for_parent(category: &MarkerId, group: Option<&MarkerId>, marker: MarkerIndex) -> Uuid {
        Self::new_uuid_namespace(&NAMESPACE_PACK_PARENT, [
            &category.as_bytes()[..],
            group.unwrap_or(&MarkerId::EMPTY).as_bytes(),
            &marker.repr().to_be_bytes(),
        ])
    }

    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.uuid.as_bytes()
    }

    pub fn variant(&self) -> IdVariant {
        IdVariant::from_uuid(self.as_ref())
    }
}
impl<N: MarkerId1> From<MarkerPath<N>> for MarkerId {
    fn from(marker: MarkerPath<N>) -> Self {
        Self::for_marker(marker)
    }
}
#[cfg(todo)]
impl From<Guid> for MarkerId {
    fn from(id: Guid) -> Self {
        Self::with_guid(id)
    }
}
impl From<Uuid> for MarkerId {
    fn from(id: Uuid) -> Self {
        Self::with_uuid(id)
    }
}
#[cfg(todo)]
impl From<MarkerId> for Guid {
    fn from(id: MarkerId) -> Self {
        id.guid
    }
}
impl From<MarkerId> for Uuid {
    fn from(id: MarkerId) -> Self {
        id.uuid
    }
}
#[cfg(todo)]
impl AsRef<Guid> for MarkerId {
    fn as_ref(&self) -> &Guid {
        &self.uuid
    }
}
impl AsRef<Uuid> for MarkerId {
    fn as_ref(&self) -> &Uuid {
        &self.uuid
    }
}
#[cfg(todo)]
impl Borrow<Guid> for MarkerId {
    fn borrow(&self) -> &Guid {
        self.as_ref()
    }
}
impl Borrow<Uuid> for MarkerId {
    fn borrow(&self) -> &Uuid {
        self.as_ref()
    }
}
impl AsRef<MarkerId> for MarkerId {
    fn as_ref(&self) -> &MarkerId {
        self
    }
}
impl AsRef<MarkerId> for Guid {
    fn as_ref(&self) -> &MarkerId {
        MarkerId::from_uuid_ref(self)
    }
}
impl fmt::Display for MarkerId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.variant() {
            IdVariant::MarkerRegistered(p) => fmt::Display::fmt(&p, f),
            IdVariant::MarkerLoaded(p) => fmt::Display::fmt(&p, f),
            IdVariant::MarkerUnscoped(p) => fmt::Display::fmt(&p, f),
            _ => fmt::Display::fmt(&self.uuid, f),
        }
    }
}

pub trait MarkerId1 {
    fn ns1(&self) -> u8;
    fn index1(&self) -> u16;
    fn index2(&self) -> u16;
}
pub trait FromMarkerId1 {
    const NS1: u8;
    fn from_index12(index1: u16, index2: u16) -> Self;
}
impl MarkerId1 for PackMarkerNs {
    fn ns1(&self) -> u8 { MarkerId::NS1_REGISTRY }
    fn index1(&self) -> u16 { 0 }
    fn index2(&self) -> u16 { 0 }
}
impl FromMarkerId1 for PackMarkerNs {
    const NS1: u8 = MarkerId::NS1_REGISTRY;
    fn from_index12(_index1: u16, _index2: u16) -> Self {
        debug_assert_eq!(_index1, 0);
        debug_assert_eq!(_index2, 0);
        Self
    }
}
impl MarkerId1 for PackRegistryNs {
    fn ns1(&self) -> u8 { MarkerId::NS1_REGISTRY }
    fn index1(&self) -> u16 { 0 }
    fn index2(&self) -> u16 { 0 }
}
impl FromMarkerId1 for PackRegistryNs {
    const NS1: u8 = MarkerId::NS1_REGISTRY;
    fn from_index12(_index1: u16, _index2: u16) -> Self {
        debug_assert_eq!(_index1, 0);
        debug_assert_eq!(_index2, 0);
        Self
    }
}
impl MarkerId1 for PackPath {
    fn ns1(&self) -> u8 { MarkerId::NS1_PACK }
    fn index1(&self) -> u16 { 0 }
    fn index2(&self) -> u16 { self.path as u16 }
}
impl FromMarkerId1 for PackPath {
    const NS1: u8 = MarkerId::NS1_PACK;
    fn from_index12(_index1: u16, index2: u16) -> Self {
        Self::with_path(index2 as PackIndex)
    }
}
impl MarkerId1 for PackMapPath {
    fn ns1(&self) -> u8 { MarkerId::NS1_PACK_MAP }
    fn index1(&self) -> u16 { self.path.get() as u16 }
    fn index2(&self) -> u16 { self.root.index2() }
}
impl FromMarkerId1 for PackMapPath {
    const NS1: u8 = MarkerId::NS1_PACK_MAP;
    fn from_index12(index1: u16, index2: u16) -> Self {
        let pack = PackPath::from_index12(0, index2);
        let map = match MapIndex::new(index1 as _) {
            Some(map) => map,
            None => {
                log::error!("invalid map index 0");
                MapIndex::MAX
            },
        };

        Self::with_parts(pack, map)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IdVariant {
    /// [Uuid::nil()]
    Empty,
    /// Likely a malformed [group](Self::Group)
    Unknown,
    /// random UUIDv4
    Group,
    /// UUIDv5 via [NAMESPACE_PACK_CATEGORY], [NAMESPACE_PACK_PARENT], etc
    PackRef,
    /// [UUIDv8](MarkerId)
    MarkerRegistered(MarkerPath<PackPath>),
    /// [UUIDv8](MarkerId)
    MarkerLoaded(MarkerPath<PackMapPath>),
    /// [UUIDv8](MarkerId)
    MarkerUnscoped(MarkerPath),
}
impl IdVariant {
    pub fn from_uuid(uuid: &Uuid) -> Self {
        match (uuid.get_variant(), uuid.get_version_num()) {
            (uuid::Variant::NCS, 0) =>
                Self::Empty,
            (uuid::Variant::RFC4122, 4) =>
                Self::Group,
            (uuid::Variant::RFC4122, 5) =>
                Self::PackRef,
            (uuid::Variant::RFC4122, 8) => {
                let id = MarkerId::from_uuid_ref(uuid);
                let ns01 = id.ns01();
                match ns01.1 {
                    MarkerId::NS1_REGISTRY => id.marker_path().map(Self::MarkerUnscoped),
                    MarkerId::NS1_PACK => id.marker_path().map(Self::MarkerRegistered),
                    MarkerId::NS1_PACK_MAP => id.marker_path().map(Self::MarkerLoaded),
                    _ => None,
                }.unwrap_or(Self::Unknown)
            },
            _ => Self::Unknown,
        }
    }
}

pub const NAMESPACE_PACK_CATEGORY: Uuid = match Uuid::try_parse_ascii(b"3102bb82-0ae3-4433-853a-dc53265bfb8a") {
    Ok(uuid) => uuid,
    Err(..) => unreachable!(),
};
pub const NAMESPACE_PACK_PARENT: Uuid = match Uuid::try_parse_ascii(b"5a4d8880-7dc2-4e91-b1b9-843f6ba34f1f") {
    Ok(uuid) => uuid,
    Err(..) => unreachable!(),
};

/// TODO: wincrypt/ncrypt implementation?
#[cfg(feature = "sha1_smol")]
pub fn hash_bytes_sha1<'a, B: IntoIterator<Item = &'a [u8]>>(bytes: B) -> [u8; 16] {
    let mut hasher = sha1_smol::Sha1::new();
    for b in bytes {
        hasher.update(b);
    }
    let hash: [u8; 20] = hasher.digest().bytes();
    unsafe {
        // truncate...
        *(&hash as *const [u8; 20] as *const [u8; 16])
    }
}
/// TODO: wincrypt/ncrypt implementation?
pub fn hash_bytes_md5<'a, B: IntoIterator<Item = &'a [u8]>>(bytes: B) -> [u8; 16] {
    use md5::{Digest, Md5};

    let mut hasher = Md5::new();
    for b in bytes {
        hasher.update(b);
    }
    hasher.finalize().into()
}
