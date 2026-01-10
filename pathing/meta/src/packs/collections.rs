use {
    crate::{
        map::MapID,
        packs::{
            id::{MarkerIndex, MarkerIndexVariant},
            CategoryIndex,
            CategoryPath,
            MapIndex,
            PackIndex,
            PackPath,
            PackRegistryNs,
            PoiPath,
            TrailPath,
        },
    },
    bitvec::vec::BitVec,
    core::iter,
    std::{
        collections::{btree_set, BTreeSet},
        mem,
    },
    taimi_hoard::{
        iters::{IterExt, LazyMapFn},
        loc::{indexed, LocationGet, Locator},
    },
};

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
    pub fn insert_index<C: Into<CategoryIndex>>(&mut self, index: C) -> bool {
        self.0.insert(index.into())
    }
    /// false indicates the value was already present
    pub fn insert<N>(&mut self, path: CategoryPath<N>) -> bool {
        self.insert_index(path.path)
    }
    pub fn remove_index<C: Into<CategoryIndex>>(&mut self, index: C) -> bool {
        self.0.remove(&index.into())
    }
    pub fn remove<N>(&mut self, path: CategoryPath<N>) -> bool {
        self.remove_index(path.path)
    }
    pub fn contains_index<C: Into<CategoryIndex>>(&self, index: C) -> bool {
        self.0.contains(&index.into())
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

    pub fn iter<'a>(&'a self) -> <&'a Self as IntoIterator>::IntoIter {
        IntoIterator::into_iter(self)
    }
    pub fn paths<'a>(&'a self) -> impl Iterator<Item = CategoryPath> + Clone + 'a {
        self.iter().map(CategoryPath::with_path)
    }
    pub fn into_paths(self) -> impl Iterator<Item = CategoryPath> {
        self.into_iter().map(CategoryPath::with_path)
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

impl FromIterator<CategoryIndex> for CategorySet {
    fn from_iter<I: IntoIterator<Item = CategoryIndex>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}
impl FromIterator<Option<CategoryIndex>> for CategorySet {
    fn from_iter<I: IntoIterator<Item = Option<CategoryIndex>>>(iter: I) -> Self {
        Self(iter.into_iter().filter_map(|c| c).collect())
    }
}
impl Extend<CategoryIndex> for CategorySet {
    fn extend<I: IntoIterator<Item = CategoryIndex>>(&mut self, iter: I) {
        self.0.extend(iter)
    }
}
impl<N> Extend<Locator<N, CategoryIndex>> for CategorySet {
    #[inline]
    fn extend<I: IntoIterator<Item = Locator<N, CategoryIndex>>>(&mut self, iter: I) {
        self.extend(iter.into_iter().map(Locator::into_path))
    }
}
impl Extend<Option<CategoryIndex>> for CategorySet {
    fn extend<I: IntoIterator<Item = Option<CategoryIndex>>>(&mut self, iter: I) {
        self.0.extend(iter.into_iter().filter_map(|c| c))
    }
}

impl<N> LocationGet<N, CategoryIndex> for CategorySet {
    type LookupGet = ();

    fn lookup_get(&self, loc: &Locator<N, CategoryIndex>) -> Option<Self::LookupGet> {
        self.contains_index(loc.path).then_some(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct MarkerSet {
    pub pois: BTreeSet<PoiPath>,
    pub trails: BTreeSet<TrailPath>,
}
impl MarkerSet {
    pub fn contains<I>(&self, marker: I) -> bool
    where
        I: Into<MarkerIndex>,
    {
        match marker.into().variant() {
            MarkerIndexVariant::Poi(poi) => self.pois.contains(&Locator::with_path(poi)),
            MarkerIndexVariant::Trail(trail) | MarkerIndexVariant::TrailSection(trail, ..) =>
                self.trails.contains(&Locator::with_path(trail)),
            _ => false,
        }
    }
    pub fn insert<I>(&mut self, marker: I) -> bool
    where
        I: Into<MarkerIndex>,
    {
        match marker.into().variant() {
            MarkerIndexVariant::Poi(poi) => self.pois.insert(Locator::with_path(poi)),
            MarkerIndexVariant::Trail(trail) | MarkerIndexVariant::TrailSection(trail, ..) =>
                self.trails.insert(Locator::with_path(trail)),
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackSet(BitVec);
impl PackSet {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn insert_index<I: Into<PackIndex>>(&mut self, index: I) -> bool {
        let index = index.into() as usize;
        if let Some(mut b) = self.0.get_mut(index) {
            return mem::replace(&mut *b, true)
        }

        self.0.resize(index, false);
        self.0.push(true);
        false
    }
    pub fn remove_index<I: Into<PackIndex>>(&mut self, index: I) -> bool {
        let index = index.into() as usize;
        self.0
            .get_mut(index)
            .map(|mut b| mem::replace(&mut *b, false))
            .unwrap_or(false)
    }
    pub fn insert(&mut self, path: PackPath) -> bool {
        self.insert_index(path.path)
    }
    pub fn remove(&mut self, path: PackPath) -> bool {
        self.remove_index(path.path)
    }

    #[inline]
    pub fn contains_index<I: Into<PackIndex>>(&self, index: I) -> bool {
        let index = index.into() as usize;
        self.0.get(index).map(|b| *b).unwrap_or(false)
    }
    pub fn contains(&self, path: PackPath) -> bool {
        self.contains_index(path.path)
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.not_any()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.0.count_ones()
    }

    /// TODO: what's the wrapper for this called...
    pub fn iter(&self) -> impl Iterator<Item = PackPath> + '_ {
        self.0
            .iter_ones()
            .lazy_map(|idx| PackPath::with_path(idx as PackIndex))
    }
    /// TODO: still use iter_ones approach ugh
    #[cfg(todo)]
    pub fn into_iter(self) -> impl Iterator<Item = PackPath> {}
}
fn filter_pack_item<N, P>((path, present): (Locator<N, P>, bool)) -> Option<Locator<N, P>> {
    present.then_some(path)
}
fn map_pack_index(index: usize) -> PackPath {
    PackPath::with_path(index as PackIndex)
}
impl IntoIterator for PackSet {
    type Item = PackPath;
    type IntoIter = iter::FilterMap<
        indexed::LocatorEnumerateAsRel<PackRegistryNs, PackIndex, bitvec::vec::IntoIter>,
        fn((PackPath, bool)) -> Option<PackPath>,
    >;
    fn into_iter(self) -> Self::IntoIter {
        indexed::LocatorRelIter0::enumerate(Default::default(), self.0.into_iter())
            .filter_map(filter_pack_item)
    }
}
impl<'a> IntoIterator for &'a PackSet {
    type Item = PackPath;
    type IntoIter =
        LazyMapFn<bitvec::slice::IterOnes<'a, usize, bitvec::order::Lsb0>, fn(usize) -> PackPath>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_ones().lazy_map(map_pack_index)
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
impl From<PackPath> for PackSet {
    fn from(path: PackPath) -> Self {
        let mut packs = Self::default();
        packs.0.reserve_exact(path.path as usize + 1);
        packs.insert_index(path.path);
        packs
    }
}
