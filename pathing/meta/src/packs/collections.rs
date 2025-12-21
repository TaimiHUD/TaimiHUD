use taimi_hoard::loc::{Locator, LocationGet};
use crate::{
    packs::{
        id::{MarkerIndex, MarkerIndexVariant},
        CategoryPath, CategoryIndex, MapIndex, PoiPath, TrailPath,
    },
    map::MapID,
};
use std::collections::{btree_set, BTreeSet};
use core::iter;

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
    pub fn contains<I>(&self, marker: I) -> bool where
        I: Into<MarkerIndex>,
    {
        match marker.into().variant() {
            MarkerIndexVariant::Poi(poi) => self.pois.contains(&Locator::with_path(poi)),
            MarkerIndexVariant::Trail(trail) | MarkerIndexVariant::TrailSection(trail, ..) => self.trails.contains(&Locator::with_path(trail)),
            _ => false,
        }
    }
    pub fn insert<I>(&mut self, marker: I) -> bool where
        I: Into<MarkerIndex>,
    {
        match marker.into().variant() {
            MarkerIndexVariant::Poi(poi) => self.pois.insert(Locator::with_path(poi)),
            MarkerIndexVariant::Trail(trail) | MarkerIndexVariant::TrailSection(trail, ..) => self.trails.insert(Locator::with_path(trail)),
            _ => false,
        }
    }
}
