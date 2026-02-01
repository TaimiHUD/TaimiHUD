use std::collections::VecDeque;
use std::sync::Arc;
use core::cmp::Ordering;
use core::{mem, num::NonZero};

#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct FlatSet<T> {
    storage: VecDeque<T>,
}
impl<T> FlatSet<T> {
    #[inline]
    pub const fn new() -> Self {
        unsafe {
            Self::with_storage_sorted(VecDeque::new())
        }
    }
    /// TODO: whether sortedness is "trusted" is in the air atm
    #[inline(always)]
    pub const unsafe fn with_storage_sorted(storage: VecDeque<T>) -> Self {
        Self {
            storage,
        }
    }

    #[inline]
    pub fn with_capacity(amt: usize) -> Self {
        unsafe {
            Self::with_storage_sorted(VecDeque::with_capacity(amt))
        }
    }
    #[inline]
    pub fn with_item(value: T) -> Self {
        unsafe {
            Self::with_storage_sorted(vec![value].into())
        }
    }
    pub fn with_item_opt(value: Option<T>) -> Self {
        value.map(Self::with_item).unwrap_or_else(Self::new)
    }

    #[inline]
    pub fn into_vec(self) -> Vec<T> { self.storage.into() }
    pub fn into_arc(self) -> Arc<[T]> {
        match self.storage {
            #[cfg(todo = "unnecessary")]
            storage => Vec::from(storage).into(),
            storage => storage.into_iter().collect(),
        }
    }
    pub fn into_boxed_slice(self) -> Box<[T]> {
        match self.storage {
            #[cfg(todo = "unnecessary")]
            storage => Vec::from(storage).into_boxed_slice(),
            storage => storage.into_iter().collect(),
        }
    }

    #[inline]
    pub fn into_storage(self) -> VecDeque<T> { self.storage }
    #[inline]
    pub fn storage(&self) -> &VecDeque<T> { &self.storage }
    #[inline]
    pub fn iter(&self) -> <&VecDeque<T> as IntoIterator>::IntoIter {
        self.storage.iter()
    }
    /// modifications that may change sort order are a bad idea
    #[inline(always)]
    pub unsafe fn storage_mut(&mut self) -> &mut VecDeque<T> { &mut self.storage }
    /// beware [Self::storage_mut]
    #[inline(always)]
    pub unsafe fn iter_mut(&mut self) -> <&mut VecDeque<T> as IntoIterator>::IntoIter {
        self.storage.iter_mut()
    }
    /// beware [Self::storage_mut]
    #[inline(always)]
    pub unsafe fn retain_mut<F: FnMut(&mut T) -> bool>(&mut self, f: F) {
        self.storage.retain_mut(f)
    }
    /// beware [Self::storage_mut]
    #[inline(always)]
    pub unsafe fn at_mut(&mut self, index: usize) -> Option<&mut T> {
        self.storage.get_mut(index)
    }
    #[inline]
    pub unsafe fn collect_sorted<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self::with_storage_sorted(iter.into_iter().collect())
    }
}
impl<T> FlatSet<T> where
    T: Ord,
{
    pub fn try_with_storage(storage: &mut VecDeque<T>) -> Result<Self, UnsortedError> {
        UnsortedError::check_iter(storage.iter())
            .map(move |_| unsafe {
                Self::with_storage_sorted(mem::take(storage))
            })
    }
    pub fn from_vec<V>(vec: V) -> Self where
        V: Into<VecDeque<T>>,
    {
        let mut storage = vec.into();
        match Self::try_with_storage(&mut storage) {
            Ok(set) => set,
            Err(until) => {
                let mut remaining = storage.split_off(until.index());
                let mut set = unsafe {
                    Self::with_storage_sorted(storage)
                };
                set.append_from_vecdeque(&mut remaining);
                set
            },
        }
    }
    #[inline]
    pub unsafe fn from_vec_sorted<V>(vec: V) -> Self where
        V: Into<VecDeque<T>>,
    {
        Self::with_storage_sorted(vec.into())
    }
    #[inline]
    pub fn merge_from(&mut self, other: &mut Self) {
        self.append_from_vecdeque(&mut other.storage)
    }
    /// TODO: should this sort ahead of time and/or look ahead to reserve space for multiple items when inserting?
    pub fn append_from_vecdeque(&mut self, vec: &mut VecDeque<T>) {
        self.extend(vec.drain(..))
    }
    pub fn try_insert(&mut self, value: T) -> Result<(), T> {
        let (index, slot) = self.binary_search_for_insert(|s| s.cmp(&value));
        Ok(match index {
            _ if slot.is_eq() => return Err(value),
            None => match slot {
                Ordering::Less => self.storage.push_back(value),
                _ => self.storage.push_front(value),
            },
            #[cfg(debug_assertions)]
            _ if slot.is_lt() => unreachable!(),
            Some(index) => self.storage.insert(index.get(), value),
        })
    }
    pub fn insert(&mut self, value: T) -> bool {
        self.try_insert(value).is_ok()
    }
}
impl<T> FlatSet<T> {
    #[inline(always)]
    pub fn at(&self, index: usize) -> Option<&T> {
        self.storage.get(index)
    }
    /// TODO: index properly I guess
    #[inline(always)]
    pub unsafe fn at_unchecked(&self, index: usize) -> &T {
        let value = self.storage.get(index);
        debug_assert!(value.is_some());
        value.unwrap_unchecked()
    }
    /// TODO: index properly I guess
    #[inline(always)]
    pub unsafe fn at_mut_unchecked(&mut self, index: usize) -> &mut T {
        let value = self.storage.get_mut(index);
        debug_assert!(value.is_some());
        value.unwrap_unchecked()
    }

    /// TODO: fn find()?
    pub fn index_of<Q>(&mut self, value: &Q) -> Option<usize> where
        T: PartialOrd<Q> + PartialEq<Q>,
    {
        let index = self.storage.partition_point(
            move |s| s < value
        );
        match self.at(index) {
            Some(item) if item == value => Some(index),
            _ => None
        }
    }
    /// TODO: reimpl? also does upstream impl do an extra unnecessary check on back.first()?
    pub fn binary_search_by<F>(&self, f: F) -> (Option<NonZero<usize>>, Ordering) where
        F: FnMut(&T) -> Ordering,
    {
        match self.storage.binary_search_by(f) {
            Err(i) if i == self.storage.len() =>
                (None, Ordering::Less),
            cmp @ (Ok(i) | Err(i)) =>
                (NonZero::new(i), match cmp {
                    Ok(..) => Ordering::Equal,
                    Err(..) => Ordering::Greater,
                }),
        }
    }
    /// TODO: reimpl?
    /// TODO: `Result<Result<bool, NonZero<usize>>, ()>`?
    fn binary_search_for_insert<F>(&self, mut f: F) -> (Option<NonZero<usize>>, Ordering) where
        F: FnMut(&T) -> Ordering,
    {
        match self.storage.back().map(&mut f) {
            Some(Ordering::Greater) => self.binary_search_by(f),
            Some(Ordering::Less) | None => (None, Ordering::Less),
            Some(cmp @ Ordering::Equal) => {
                let last_index = unsafe {
                    NonZero::new_unchecked(self.storage.len().unchecked_sub(1))
                };
                (Some(last_index), cmp)
            },
        }
    }
    /// TODO: Borrow<Q>?
    #[inline]
    pub fn contains<Q>(&mut self, value: &Q) -> bool where
        T: PartialOrd<Q> + PartialEq<Q>,
    {
        self.index_of(value).is_some()
    }

    #[inline(always)]
    pub fn retain<F: FnMut(&T) -> bool>(&mut self, f: F) {
        self.storage.retain(f)
    }
    #[inline(always)]
    pub fn remove_at(&mut self, index: usize) -> Option<T> {
        self.storage.remove(index)
    }
    pub fn remove<Q>(&mut self, value: &Q) -> Option<T> where
        T: PartialOrd<Q> + PartialEq<Q>,
    {
        self.index_of(value).and_then(|index|
            self.remove_at(index)
        )
    }
}
impl<T> Extend<T> for FlatSet<T> where
    T: Ord,
{
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for value in iter {
            self.insert(value);
        }
    }
}
impl<T> FromIterator<T> for FlatSet<T> where
    T: Ord,
{
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let iter = iter.into_iter();
        let mut set = Self::with_capacity(iter.size_hint().1.unwrap_or(0));
        set.extend(iter);
        set
    }
}
impl<T> IntoIterator for FlatSet<T> {
    type Item = T;
    type IntoIter = <VecDeque<T> as IntoIterator>::IntoIter;
    fn into_iter(self) -> Self::IntoIter {
        self.storage.into_iter()
    }
}
impl<'a, T> IntoIterator for &'a FlatSet<T> {
    type Item = &'a T;
    type IntoIter = <&'a VecDeque<T> as IntoIterator>::IntoIter;
    fn into_iter(self) -> Self::IntoIter {
        self.storage.iter()
    }
}
impl<T> From<std::collections::BTreeSet<T>> for FlatSet<T> {
    #[inline]
    fn from(set: std::collections::BTreeSet<T>) -> Self {
        unsafe {
            Self::collect_sorted(set)
        }
    }
}
impl<T, V> From<std::collections::BTreeMap<T, V>> for FlatSet<T> {
    #[inline]
    fn from(set: std::collections::BTreeMap<T, V>) -> Self {
        unsafe {
            Self::collect_sorted(set.into_keys())
        }
    }
}
impl<T: Clone> From<&'_ std::collections::BTreeSet<T>> for FlatSet<T> {
    #[inline]
    fn from(set: &std::collections::BTreeSet<T>) -> Self {
        unsafe {
            Self::collect_sorted(set.iter().cloned())
        }
    }
}
impl<T: Clone, V> From<&'_ std::collections::BTreeMap<T, V>> for FlatSet<T> {
    #[inline]
    fn from(set: &std::collections::BTreeMap<T, V>) -> Self {
        unsafe {
            Self::collect_sorted(set.keys().cloned())
        }
    }
}
impl<T> Default for FlatSet<T> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

pub struct UnsortedError {
    pub index: NonZero<usize>,
}
impl UnsortedError {
    #[inline(always)]
    pub const fn with_index(index: NonZero<usize>) -> Self { Self { index } }

    pub fn check_iter<I: IntoIterator>(iter: I) -> Result<usize, Self> where
        I::Item: PartialOrd,
    {
        Self::check_with(iter, |prev, _i, next| *prev <= *next)
    }
    pub fn check_with<I: IntoIterator, C>(iter: I, mut cmp: C) -> Result<usize, Self> where
        C: FnMut(&mut I::Item, usize, &mut I::Item) -> bool,
    {
        let mut iter = iter.into_iter().enumerate();
        let mut prev_i = 0usize;
        let mut prev = match iter.next() {
            Some((_i, prev)) => prev,
            None => return Ok(prev_i),
        };
        prev_i = 1;
        for (i, mut next) in iter {
            prev_i = i;
            if !cmp(&mut prev, prev_i, &mut next) {
                return Err(Self::with_index(unsafe { NonZero::new_unchecked(prev_i) }))
            }
        }
        Ok(prev_i)
    }

    pub fn index(&self) -> usize {
        self.index.get()
    }
    pub fn prior_index(&self) -> usize {
        unsafe {
            self.index.get().unchecked_sub(1)
        }
    }
}
