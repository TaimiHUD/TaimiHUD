use {
    crate::{
        map::MapID,
        packs::{
            id::{MarkerIndex, MarkerPath, PackMarkerNs},
            CategoryIndex,
            CategoryPath,
            MapIndex,
            PackIndex,
            PackPath,
            PackPoiNs,
            PackRegistryNs,
            PackTrailNs,
            PoiIndex,
            PoiPath,
            TrailIndex,
            TrailPath,
        },
    },
    core::iter,
    num_traits::AsPrimitive,
    std::collections::{btree_set, BTreeSet},
    taimi_hoard::{
        collections::TaimiSet,
        flags::set::{self as bitset, BitSet},
        iters::IterExt as _,
        loc::{LocationGet, Locator, NamespacePivotFrom, NamespaceTryConvTo},
    },
};

pub use super::visible::VisibilityFlagSet;

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MapSet(BTreeSet<MapID>);
#[cfg(todo)]
pub struct MapSet(BitVec);

impl MapSet {
    #[inline]
    pub fn insert<M: Into<MapID>>(&mut self, map: M) -> bool {
        self.0.insert(map.into())
    }
    #[inline]
    pub fn remove<M: Into<MapID>>(&mut self, map: M) -> bool {
        self.0.remove(&map.into())
    }

    #[inline]
    pub fn contains<M: Into<MapID>>(&self, map: M) -> bool {
        self.0.contains(&map.into())
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl FromIterator<MapID> for MapSet {
    fn from_iter<I: IntoIterator<Item = MapID>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

impl<N> LocationGet<N, MapIndex> for MapSet {
    type LookupGet = ();

    fn lookup_get(&self, loc: &Locator<N, MapIndex>) -> Option<Self::LookupGet> {
        self.contains(loc.path).then_some(())
    }
}
impl<N> LocationGet<N, MapID> for MapSet {
    type LookupGet = ();

    fn lookup_get(&self, loc: &Locator<N, MapID>) -> Option<Self::LookupGet> {
        self.contains(loc.path).then_some(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CategorySet(BTreeSet<CategoryIndex>);

impl CategorySet {
    pub fn empty() -> Self {
        Self::default()
    }
    /// TODO: switch to something that doesn't use so much pointer indirection...
    pub fn with_capacity(cap: usize) -> Self {
        let _ = cap;
        Self::empty()
    }

    #[inline]
    pub fn insert_index<C: AsPrimitive<CategoryIndex>>(&mut self, index: C) -> bool {
        self.0.insert(index.as_())
    }
    /// false indicates the value was already present
    pub fn insert<N>(&mut self, path: CategoryPath<N>) -> bool {
        self.insert_index(path.path)
    }
    pub fn remove_index<C: AsPrimitive<CategoryIndex>>(&mut self, index: C) -> bool {
        self.0.remove(&index.as_())
    }
    pub fn remove<N>(&mut self, path: CategoryPath<N>) -> bool {
        self.remove_index(path.path)
    }
    pub fn contains_index<C: AsPrimitive<CategoryIndex>>(&self, index: C) -> bool {
        self.0.contains(&index.as_())
    }
    pub fn contains<N>(&self, path: CategoryPath<N>) -> bool {
        self.contains_index(path.path)
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn iter<'a>(&'a self) -> <&'a Self as IntoIterator>::IntoIter {
        IntoIterator::into_iter(self)
    }
    pub fn into_iter<T>(self) -> impl DoubleEndedIterator<Item = T>
    where
        T: Copy + 'static,
        CategoryIndex: AsPrimitive<T>,
    {
        IntoIterator::into_iter(self).lazy_map(AsPrimitive::as_)
    }
    pub fn paths<'a>(&'a self) -> impl DoubleEndedIterator<Item = CategoryPath> + Clone + 'a {
        self.iter().lazy_map(CategoryPath::with_path)
    }
    #[inline]
    pub fn into_paths(self) -> impl DoubleEndedIterator<Item = CategoryPath> {
        self.into_iter::<CategoryPath>()
    }

    pub fn into_index_boxed(self) -> Box<[CategoryIndex]> {
        self.0.into_iter().collect()
    }
}

impl IntoIterator for CategorySet {
    type Item = CategoryIndex;
    type IntoIter = btree_set::IntoIter<CategoryIndex>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}
impl<'a> IntoIterator for &'a CategorySet {
    type Item = CategoryIndex;
    type IntoIter = iter::Copied<btree_set::Iter<'a, CategoryIndex>>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter().copied()
    }
}

impl<T: AsPrimitive<CategoryIndex>> FromIterator<T> for CategorySet {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let iter = iter.into_iter().map(AsPrimitive::as_);
        Self(iter.collect())
    }
}
impl<T: AsPrimitive<CategoryIndex>> Extend<T> for CategorySet {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        self.0.extend(iter.into_iter().map(AsPrimitive::as_))
    }
}

impl<T> TaimiSet<T> for CategorySet
where
    T: Copy + AsPrimitive<CategoryIndex>,
{
    #[inline]
    fn set_contains(&self, index: &T) -> bool {
        self.contains_index(index.as_())
    }
}
impl<N> LocationGet<N, CategoryIndex> for CategorySet {
    type LookupGet = ();

    fn lookup_get(&self, loc: &Locator<N, CategoryIndex>) -> Option<Self::LookupGet> {
        self.contains_index(loc.path).then_some(())
    }
}

#[derive(Debug, Clone)]
pub struct MarkerSet<PN = PackPoiNs, TN = PackTrailNs> {
    pub pois: BTreeSet<PoiPath<PN>>,
    pub trails: BTreeSet<TrailPath<TN>>,
}
impl<PN, TN> MarkerSet<PN, TN>
where
    PN: Ord + Default,
    TN: Ord + Default,
{
    /// TODO: doesn't check ns :<
    #[inline]
    pub fn contains_marker_unchecked<N, I>(&self, path: Locator<N, I>) -> bool
    where
        PackMarkerNs: NamespacePivotFrom<N, I, NsPivotFromPath = MarkerIndex>,
    {
        let path = PackMarkerNs::loc_pivot_from(path);
        self.contains_index(path.path)
    }
    pub fn contains_index<I>(&self, marker: I) -> bool
    where
        I: Into<MarkerIndex>,
        PN: Default,
        TN: Default,
    {
        let marker = marker.into();
        match marker.namespace() {
            MarkerIndex::NS_POI => {
                let path: PoiPath<PN> = Locator::with_path(marker.index_poi_unchecked());
                self.contains_poi(&path)
            },
            MarkerIndex::NS_TRAIL => {
                let path: TrailPath<TN> = Locator::with_path(marker.trail_index_unchecked());
                self.contains_trail(&path)
            },
            _ => false,
        }
    }
    pub fn insert_index<I>(&mut self, marker: I) -> bool
    where
        I: Into<MarkerIndex>,
    {
        let marker = marker.into();
        match marker.namespace() {
            MarkerIndex::NS_POI => {
                let path: PoiPath<PN> = Locator::with_path(marker.index_poi_unchecked());
                self.insert_poi(path)
            },
            MarkerIndex::NS_TRAIL => {
                let path: TrailPath<TN> = Locator::with_path(marker.trail_index_unchecked());
                self.insert_trail(path)
            },
            _ => false,
        }
    }
}
impl<PN, TN> MarkerSet<PN, TN>
where
    PN: Ord,
    TN: Ord,
{
    #[inline]
    pub fn contains_path<N, I>(&self, path: Locator<N, I>) -> bool
    where
        N: NamespaceTryConvTo<I, PoiPath<PN>> + Clone,
        N: NamespaceTryConvTo<I, TrailPath<TN>>,
        I: Clone,
    {
        if let Some(poi) = <N as NamespaceTryConvTo<I, PoiPath<PN>>>::try_conv_to(path.clone()) {
            self.pois.contains(&poi)
        } else if let Some(trail) = <N as NamespaceTryConvTo<I, TrailPath<TN>>>::try_conv_to(path) {
            self.trails.contains(&trail)
        } else {
            false
        }
    }
    #[inline]
    pub fn insert_path<N, I>(&mut self, path: Locator<N, I>) -> bool
    where
        N: NamespaceTryConvTo<I, PoiPath<PN>> + Clone,
        N: NamespaceTryConvTo<I, TrailPath<TN>>,
        I: Clone,
    {
        if let Some(poi) = <N as NamespaceTryConvTo<I, PoiPath<PN>>>::try_conv_to(path.clone()) {
            self.pois.insert(poi)
        } else if let Some(trail) = <N as NamespaceTryConvTo<I, TrailPath<TN>>>::try_conv_to(path) {
            self.trails.insert(trail)
        } else {
            false
        }
    }
}
impl<PN, TN> MarkerSet<PN, TN>
where
    PN: Ord,
{
    #[inline]
    pub fn contains_poi(&self, path: &PoiPath<PN>) -> bool {
        self.pois.contains(path)
    }
    #[inline]
    pub fn insert_poi(&mut self, path: PoiPath<PN>) -> bool {
        self.pois.insert(path)
    }
}
impl<PN, TN> MarkerSet<PN, TN>
where
    TN: Ord,
{
    #[inline]
    pub fn contains_trail(&self, path: &TrailPath<TN>) -> bool {
        self.trails.contains(path)
    }
    #[inline]
    pub fn insert_trail(&mut self, path: TrailPath<TN>) -> bool {
        self.trails.insert(path)
    }
}
impl<PN, TN, N, I> TaimiSet<Locator<N, I>> for MarkerSet<PN, TN>
where
    PN: Ord,
    TN: Ord,
    N: NamespaceTryConvTo<I, PoiPath<PN>> + Clone,
    N: NamespaceTryConvTo<I, TrailPath<TN>>,
    I: Clone,
{
    #[inline]
    fn set_contains(&self, path: &Locator<N, I>) -> bool {
        self.contains_path(path.clone())
    }
}

impl<PN, TN> MarkerSet<PN, TN> {
    #[inline]
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn iter_index<N>(&self) -> impl DoubleEndedIterator<Item = MarkerIndex> + '_ {
        let pois = self.pois.iter().lazy_map(|poi| MarkerIndex::with_poi(poi.path));
        let trails = self
            .trails
            .iter()
            .lazy_map(|trail| MarkerIndex::with_trail(trail.path));
        pois.chain(trails)
    }

    pub fn iter_paths<'a, N>(
        &'a self,
    ) -> impl DoubleEndedIterator<Item = Locator<N, <N as NamespacePivotFrom<PN, PoiIndex>>::NsPivotFromPath>> + 'a
    where
        N: NamespacePivotFrom<PN, PoiIndex> + 'a,
        N: NamespacePivotFrom<
            TN,
            TrailIndex,
            NsPivotFromPath = <N as NamespacePivotFrom<PN, PoiIndex>>::NsPivotFromPath,
        >,
        PN: Clone,
        TN: Clone,
    {
        let pois = self.pois.iter().lazy_clone().lazy_map(N::loc_pivot_from);
        let trails = self.trails.iter().lazy_clone().lazy_map(N::loc_pivot_from);
        pois.chain(trails)
    }
}
impl MarkerSet {
    #[inline]
    pub fn iter_markers(&self) -> impl DoubleEndedIterator<Item = MarkerPath> + '_ {
        self.iter_paths::<PackMarkerNs>()
    }
}
impl<PN, TN> Default for MarkerSet<PN, TN> {
    fn default() -> Self {
        Self {
            pois: Default::default(),
            trails: Default::default(),
        }
    }
}
impl<PN, TN, M> FromIterator<M> for MarkerSet<PN, TN>
where
    Self: Extend<M>,
{
    fn from_iter<I: IntoIterator<Item = M>>(iter: I) -> Self {
        let mut set = Self::default();
        set.extend(iter);
        set
    }
}
impl<PN, TN, N, P> Extend<Locator<N, P>> for MarkerSet<PN, TN>
where
    N: NamespaceTryConvTo<P, PoiPath<PN>> + Clone,
    N: NamespaceTryConvTo<P, TrailPath<TN>>,
    P: Clone,
    PN: Ord,
    TN: Ord,
{
    fn extend<I: IntoIterator<Item = Locator<N, P>>>(&mut self, iter: I) {
        for path in iter {
            self.insert_path(path);
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackSet(BitSet);
impl PackSet {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn insert_index<I: AsPrimitive<usize>>(&mut self, index: I) -> bool {
        self.0.insert_at(index)
    }
    pub fn remove_index<I: AsPrimitive<usize>>(&mut self, index: I) -> bool {
        self.0.remove_at(index).unwrap_or(false)
    }
    pub fn insert(&mut self, path: PackPath) -> bool {
        self.insert_index(path.path)
    }
    pub fn remove(&mut self, path: PackPath) -> bool {
        self.remove_index(path.path)
    }

    #[inline]
    pub fn contains_index<I: AsPrimitive<usize>>(&self, index: I) -> bool {
        self.0.contains(index)
    }
    pub fn contains(&self, path: PackPath) -> bool {
        self.contains_index(path.path)
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.0.count()
    }

    pub fn iter<N: Default + Copy + 'static>(&self) -> bitset::BitSetIterOf<'_, PackPath<N>> {
        self.0.iter_of::<PackPath<N>>()
    }
    pub fn into_iter<N: Default + Clone>(self) -> bitset::BitSetVecIter<N, PackIndex> {
        self.0.bitvec_iter_of()
    }
}
impl IntoIterator for PackSet {
    type Item = PackPath;
    type IntoIter = bitset::BitSetVecIter<PackRegistryNs, PackIndex>;
    fn into_iter(self) -> Self::IntoIter {
        self.into_iter()
    }
}
impl<'a> IntoIterator for &'a PackSet {
    type Item = PackPath;
    type IntoIter = bitset::BitSetIterOf<'a, PackPath>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl FromIterator<PackIndex> for PackSet {
    fn from_iter<I: IntoIterator<Item = PackIndex>>(iter: I) -> Self {
        let mut set = Self::default();
        set.extend(iter);
        set
    }
}
impl FromIterator<Option<PackIndex>> for PackSet {
    fn from_iter<I: IntoIterator<Item = Option<PackIndex>>>(iter: I) -> Self {
        let mut set = Self::default();
        set.extend(iter);
        set
    }
}
impl<N> FromIterator<Locator<N, PackIndex>> for PackSet {
    fn from_iter<I: IntoIterator<Item = Locator<N, PackIndex>>>(iter: I) -> Self {
        let mut set = Self::default();
        set.extend(iter);
        set
    }
}
impl<N> FromIterator<Option<Locator<N, PackIndex>>> for PackSet {
    fn from_iter<I: IntoIterator<Item = Option<Locator<N, PackIndex>>>>(iter: I) -> Self {
        Self::from_iter(iter.into_iter().flatten())
    }
}
impl Extend<PackIndex> for PackSet {
    fn extend<I: IntoIterator<Item = PackIndex>>(&mut self, iter: I) {
        for index in iter {
            self.insert_index(index);
        }
    }
}
impl<N> Extend<Locator<N, PackIndex>> for PackSet {
    #[inline]
    fn extend<I: IntoIterator<Item = Locator<N, PackIndex>>>(&mut self, iter: I) {
        self.extend(iter.into_iter().map(Locator::into_path))
    }
}
impl Extend<Option<PackIndex>> for PackSet {
    fn extend<I: IntoIterator<Item = Option<PackIndex>>>(&mut self, iter: I) {
        for index in iter {
            if let Some(index) = index {
                self.insert_index(index);
            }
        }
    }
}
impl<T> TaimiSet<T> for PackSet
where
    T: Copy + AsPrimitive<PackIndex>,
{
    #[inline]
    fn set_contains(&self, index: &T) -> bool {
        self.contains_index(index.as_())
    }
}
impl From<PackPath> for PackSet {
    fn from(path: PackPath) -> Self {
        let mut packs = Self::default();
        packs.0.reserve_exact(path.path as usize + 1);
        packs.insert_index(path.path);
        packs
    }
}
