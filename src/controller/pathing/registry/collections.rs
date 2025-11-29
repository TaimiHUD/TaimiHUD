use crate::controller::pathing::registry::{CategoryPath, CategoryIndex, MapIndex};
use crate::exports::runtime::locator::{Locator, LocationGet};
use taimi_meta::map::MapID;
use bitvec::{order::Lsb0, slice::BitSlice, vec::BitVec, view::BitView, store::BitStore};
use std::collections::{btree_set, BTreeSet};
use std::{iter, ops, hash, marker::PhantomData};

/// A poor man's LRU cache
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RecentlyUsed {
    pub generation: u32,
}

impl RecentlyUsed {
    pub const DEFAULT: Self = Self {
        generation: 0,
    };

    pub fn mark_used(&mut self) {
        self.generation = 0;
    }

    pub fn mark_unused(&mut self) {
        self.generation = self.generation.saturating_add(1);
    }

    pub fn is_elderly(&self, threshold: u32) -> bool {
        self.generation > threshold
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MapSet(BTreeSet<MapID>);
#[cfg(todo)]
pub struct MapSet(BitVec);

impl MapSet {
    pub fn contains<M: Into<MapID>>(&self, map: M) -> bool {
        self.0.contains(&map.into())
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

pub trait BitFlagForSet: Copy + Clone + Default {
    type Repr: Copy + Clone + Default + PartialEq + Eq + PartialOrd + Ord + hash::Hash + BitStore;
    const BIT_WIDTH: usize;

    fn as_bits(&self) -> &Self::Repr;
    fn as_bits_mut(&mut self) -> &mut Self::Repr;
    fn as_bitslice(&self) -> &BitSlice<Self::Repr, Lsb0> {
        &self.as_bits().view_bits()[..Self::BIT_WIDTH]
    }
    fn as_bitslice_mut(&mut self) -> &mut BitSlice<Self::Repr, Lsb0> {
        &mut self.as_bits_mut().view_bits_mut()[..Self::BIT_WIDTH]
    }

    fn range_for(index: usize) -> ops::Range<usize> {
        let start = index * Self::BIT_WIDTH;
        let end = start + Self::BIT_WIDTH;
        start..end
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct FlagSet<F: BitFlagForSet, V = BitVec<<F as BitFlagForSet>::Repr>> {
    pub flags: V,
    pub _values: PhantomData<[F]>,
}

impl<F: BitFlagForSet, V> FlagSet<F, V> {
    pub const fn new(flags: V) -> Self {
        Self {
            flags,
            _values: PhantomData,
        }
    }
}

impl<F: BitFlagForSet> FlagSet<F> {
    pub fn extend_to(&mut self, min_len: usize, fill: bool) {
        let min_size = min_len * F::BIT_WIDTH;
        if self.flags.len() < min_size {
            self.flags.resize(min_size, fill);
        }
    }

    pub fn extend_for<N, L: Into<u32>>(&mut self, path: Locator<N, L>, fill: bool) {
        let idx = path.path.into();
        if idx >= u32::MAX {
            // TODO: debug_assertions
            log::error!("flagset[{idx:#x}] must be a bug!");
            return
        }
        self.extend_to(idx as usize + 1, fill)
    }
}

impl<F: BitFlagForSet, V> FlagSet<F, V> where
    V: AsRef<BitSlice<F::Repr, Lsb0>>,
{
    pub fn get(&self, index: usize) -> Option<F> {
        let range = F::range_for(index);
        let flags = self.flags.as_ref();
        flags.as_ref().get(range).map(|flags| {
            let mut out = F::default();
            out.as_bitslice_mut().copy_from_bitslice(flags);
            out
        })
    }
    pub fn get_for<N, L: Into<u32>>(&self, path: Locator<N, L>) -> Option<F> {
        let idx = path.path.into() as usize;
        self.get(idx)
    }
}

impl<F: BitFlagForSet, V> FlagSet<F, V> where
    V: AsMut<BitSlice<F::Repr, Lsb0>>,
{
    pub fn set(&mut self, index: usize, value: F) -> Result<(), ()> {
        let value = value.as_bitslice();
        let range = F::range_for(index);
        let flags = self.flags.as_mut();
        if flags.len() >= range.end {
            Err(())
        } else {
            unsafe {
                flags.as_mut().get_unchecked_mut(range).copy_from_bitslice(value)
            }
            Ok(())
        }
    }
    pub fn set_at<N, L: Into<usize>>(&mut self, path: Locator<N, L>, value: F) -> Result<(), ()> {
        self.set(path.path.into(), value)
    }
}

impl<F: BitFlagForSet> Extend<F> for FlagSet<F> {
    fn extend<I: IntoIterator<Item = F>>(&mut self, iter: I) {
        let iter = iter.into_iter();
        let (min, max) = iter.size_hint();
        if let Some(max) = max {
            self.flags.reserve_exact(max);
        } else {
            self.flags.reserve(min);
        }
        for flag in iter {
            self.flags.extend_from_bitslice(flag.as_bitslice());
        }
    }
}
impl<F: BitFlagForSet> FromIterator<F> for FlagSet<F> {
    fn from_iter<I: IntoIterator<Item = F>>(iter: I) -> Self {
        let mut set = Self::default();
        set.extend(iter);
        set
    }
}
