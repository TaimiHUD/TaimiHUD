use crate::collections::traits::TaimiCollection;
use num_traits::AsPrimitive;
use core::ops;
use std::sync::Arc;

pub trait TaimiSeqKey: AsPrimitive<usize> + Copy + 'static + PartialOrd + PartialEq {
    #[inline(always)]
    fn to_seq_index(self) -> usize {
        self.as_()
    }
    #[inline(always)]
    unsafe fn from_seq_index_unchecked(index: usize) -> Self {
        Self::try_from_seq_index(index).unwrap_unchecked()
    }
    fn try_from_seq_index(index: usize) -> Option<Self>;
}
impl<T> TaimiSeqKey for T where
    T: PartialEq + PartialOrd + AsPrimitive<usize> + Copy + 'static,
    usize: AsPrimitive<T>,
{
    #[inline(always)]
    unsafe fn from_seq_index_unchecked(index: usize) -> Self {
        index.as_()
    }
    #[inline(always)]
    fn try_from_seq_index(index: usize) -> Option<Self> {
        Some(index.as_())
    }
}
pub trait TaimiSeq<V: ?Sized, K: TaimiSeqKey = usize>: TaimiCollection {
    #[inline]
    fn seq_range(&self) -> Option<ops::RangeInclusive<K>> {
        self.seq_max()
            .map(|max| unsafe {
                let min = self.seq_min();
                debug_assert!(min.is_some());
                min.unwrap_unchecked()..=max
            })
    }
    fn seq_min(&self) -> Option<K>;
    fn seq_max(&self) -> Option<K>;

    #[inline(always)]
    fn seq_len(&self) -> usize {
        self.collection_len()
    }
}
pub trait TaimiSeqMut<V: ?Sized, K: TaimiSeqKey = usize>: TaimiSeq<V, K> {
}
pub trait TaimiSeqGet<V, K: TaimiSeqKey = usize>: TaimiSeq<V, K> {
    fn seq_get(self, index: K) -> Option<V>;
}
pub trait TaimiSeqIndex<V: ?Sized, K: TaimiSeqKey = usize>: TaimiSeq<V, K> {
    fn seq_index<'a>(&'a self, index: K) -> Option<&'a V> {
        match self.seq_range() {
            Some(range) if range.contains(&index) => Some(unsafe {
                self.seq_index_unchecked(index)
            }),
            _ => None,
        }
    }
    unsafe fn seq_index_unchecked<'a>(&'a self, index: K) -> &'a V;
}
pub trait TaimiSeqIndexMut<V, K: TaimiSeqKey = usize>: TaimiSeqMut<V, K> {
    fn seq_index_mut<'a>(&'a mut self, index: K) -> Option<&'a mut V> {
        match self.seq_range() {
            Some(range) if range.contains(&index) => Some(unsafe {
                self.seq_index_mut_unchecked(index)
            }),
            _ => None,
        }
    }
    unsafe fn seq_index_mut_unchecked<'a>(&'a mut self, index: K) -> &'a mut V;
}
pub trait TaimiSeqStorage: TaimiSeq<<Self as TaimiSeqStorage>::Value, <Self as TaimiSeqStorage>::Index> {
    type Index: TaimiSeqKey;
    type Value;
}

impl<T> TaimiSeqStorage for [T] {
    type Index = usize;
    type Value = T;
}
impl<T> TaimiSeq<T> for [T] {
    #[inline(always)]
    fn seq_range(&self) -> Option<ops::RangeInclusive<usize>> {
        self.seq_max()
            .map(|max| 0..=max)
    }
    #[inline(always)]
    fn seq_min(&self) -> Option<usize> {
        (!self.is_empty()).then_some(0)
    }
    #[inline(always)]
    fn seq_max(&self) -> Option<usize> {
        self.len().checked_sub(1)
    }
}
impl<T> TaimiSeqIndex<T> for [T] {
    #[inline(always)]
    fn seq_index<'a>(&'a self, index: usize) -> Option<&'a T> {
        self.get(index)
    }
    #[inline(always)]
    unsafe fn seq_index_unchecked<'a>(&'a self, index: usize) -> &'a T {
        self.get_unchecked(index)
    }
}
impl<T> TaimiSeqMut<T> for [T] {}
impl<T> TaimiSeqIndexMut<T> for [T] {
    #[inline(always)]
    fn seq_index_mut<'a>(&'a mut self, index: usize) -> Option<&'a mut T> {
        self.get_mut(index)
    }
    #[inline(always)]
    unsafe fn seq_index_mut_unchecked<'a>(&'a mut self, index: usize) -> &'a mut T {
        self.get_unchecked_mut(index)
    }
}

impl<T: ?Sized + TaimiSeqStorage> TaimiSeqStorage for Arc<T> where
    Self: TaimiCollection,
{
    type Index = T::Index;
    type Value = T::Value;
}
impl<V: ?Sized, K: TaimiSeqKey, T: ?Sized + TaimiSeq<V, K>> TaimiSeq<V, K> for Arc<T> where
    Self: TaimiCollection,
{
    #[inline(always)]
    fn seq_range(&self) -> Option<ops::RangeInclusive<K>> {
        <T as TaimiSeq<V, K>>::seq_range(self)
    }
    #[inline(always)]
    fn seq_min(&self) -> Option<K> {
        <T as TaimiSeq<V, K>>::seq_min(self)
    }
    #[inline(always)]
    fn seq_max(&self) -> Option<K> {
        <T as TaimiSeq<V, K>>::seq_max(self)
    }
}
impl<V: ?Sized, K: TaimiSeqKey, T: ?Sized + TaimiSeqIndex<V, K>> TaimiSeqIndex<V, K> for Arc<T> where
    Self: TaimiCollection,
{
    #[inline(always)]
    fn seq_index<'a>(&'a self, index: K) -> Option<&'a V> {
        <T as TaimiSeqIndex<V, K>>::seq_index(self, index)
    }
    #[inline(always)]
    unsafe fn seq_index_unchecked<'a>(&'a self, index: K) -> &'a V {
        <T as TaimiSeqIndex<V, K>>::seq_index_unchecked(self, index)
    }
}
impl<'a, V, K: TaimiSeqKey, T: ?Sized> TaimiSeqGet<V, K> for &'a Arc<T> where
    &'a T: TaimiSeqGet<V, K>,
    Self: TaimiSeq<V, K>,
{
    #[inline(always)]
    fn seq_get(self, index: K) -> Option<V> {
        <&'a T as TaimiSeqGet<V, K>>::seq_get(self, index)
    }
}
