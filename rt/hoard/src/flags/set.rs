#[cfg(feature = "serde")]
use serde::{ser, de};
use crate::loc::Locator;
use bitvec::{order::{BitOrder, Lsb0}, slice::BitSlice, vec::BitVec, view::BitView, ptr::{self as bitptr, BitRef}, store::BitStore};
use core::{ops, hash, marker::PhantomData, mem};
use num_traits::AsPrimitive;

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
pub struct BitSet<V: ?Sized = BitVec, T: BitStore = usize, O: BitOrder = Lsb0> {
    pub _bits: PhantomData<bitptr::BitPtr<bitptr::Mut, T, O>>,
    pub flags: V,
}
impl<V, T: BitStore, O: BitOrder> BitSet<V, T, O> {
    #[inline]
    pub const fn new(flags: V) -> Self {
        Self {
            flags,
            _bits: PhantomData,
        }
    }
    #[inline]
    pub fn into_flags(self) -> V {
        self.flags
    }
}
impl<V: ?Sized, T: BitStore, O: BitOrder> BitSet<V, T, O> {
    #[inline]
    pub const fn from_ref(flags: &V) -> &Self {
        unsafe {
            mem::transmute(flags)
        }
    }
    #[inline]
    pub fn from_mut(flags: &mut V) -> &mut Self {
        unsafe {
            mem::transmute(flags)
        }
    }
    #[inline]
    pub const fn flags_ref(&self) -> &V {
        &self.flags
    }
    #[inline]
    pub fn flags_mut(&mut self) -> &mut V {
        &mut self.flags
    }
    const UNREASONABLE_LEN: usize = u32::MAX as usize - 1;
    #[inline]
    fn check_offset(offset: usize) -> Option<usize> {
        if offset >= Self::UNREASONABLE_LEN {
            // TODO: debug_assertions
            log::error!("flagset[{offset:#x}] must be a bug!");
            None
        } else {
            Some(offset)
        }
    }
}
impl<T: BitStore, O: BitOrder> BitSet<BitVec<T, O>, T, O> {
    #[inline]
    pub fn extend_to<L: AsPrimitive<usize>>(&mut self, min_size: L, fill: bool) {
        self.extend_to_size(min_size.as_(), fill)
    }
    #[inline]
    pub fn extend_for<L: AsPrimitive<usize>>(&mut self, offset: L, fill: bool) -> BitRef<'_, bitptr::Mut, T, O> {
        self.extend_for_offset(offset.as_(), fill)
    }
    #[inline]
    pub fn insert_at<L: AsPrimitive<usize>>(&mut self, offset: L) -> bool {
        let offset = offset.as_();
        let Some(offset) = Self::check_offset(offset) else { return false };
        self.insert_at_offset(offset)
    }

    pub fn extend_to_size(&mut self, min_size: usize, fill: bool) {
        if self.flags.len() < min_size {
            self.flags.resize(min_size, fill);
        }
    }
    pub fn extend_for_offset(&mut self, offset: usize, fill: bool) -> BitRef<'_, bitptr::Mut, T, O> {
        if self.flags.len() <= offset {
            self.flags.resize(offset + 1, fill);
        }
        unsafe {
            self.flags.get_unchecked_mut(offset)
        }
    }
    pub fn insert_at_offset(&mut self, offset: usize) -> bool {
        let mut dest = self.extend_for_offset(offset, false);
        mem::replace(&mut *dest, true)
    }
}
impl<V: ?Sized, T: BitStore, O: BitOrder> BitSet<V, T, O> where
    V: AsRef<BitSlice<T, O>>,
{
    #[inline]
    pub fn contains<L: AsPrimitive<usize>>(&self, offset: L) -> bool {
        matches!(self.get_at(offset), Some(true))
    }
    #[inline]
    pub fn get_at<L: AsPrimitive<usize>>(&self, offset: L) -> Option<bool> {
        let offset = offset.as_();
        let Some(offset) = Self::check_offset(offset) else { return None };
        self.get_at_offset(offset)
    }
    #[inline]
    pub fn get_ref_at<L: AsPrimitive<usize>>(&self, offset: L) -> Option<BitRef<'_, bitptr::Const, T, O>> {
        let offset = offset.as_();
        let Some(offset) = Self::check_offset(offset) else { return None };
        self.get_ref_at_offset(offset)
    }

    #[inline]
    pub fn get_at_offset(&self, offset: usize) -> Option<bool> {
        self.get_ref_at_offset(offset).map(|b| *b)
    }
    #[inline]
    pub fn get_ref_at_offset(&self, offset: usize) -> Option<BitRef<'_, bitptr::Const, T, O>> {
        self.flags.as_ref().get(offset)
    }
}
impl<V: ?Sized, T: BitStore, O: BitOrder> BitSet<V, T, O> where
    V: AsMut<BitSlice<T, O>>,
{
    #[inline]
    pub fn get_mut_at<L: AsPrimitive<usize>>(&mut self, offset: L) -> Option<BitRef<'_, bitptr::Mut, T, O>> {
        let offset = offset.as_();
        let Some(offset) = Self::check_offset(offset) else { return None };
        self.get_mut_at_offset(offset)
    }
    #[inline]
    pub fn get_mut_at_offset(&mut self, offset: usize) -> Option<BitRef<'_, bitptr::Mut, T, O>> {
        self.flags.as_mut().get_mut(offset)
    }
}
impl<V: ?Sized, T: BitStore, O: BitOrder> ops::Deref for BitSet<V, T, O> {
    type Target = V;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.flags
    }
}
impl<V: ?Sized, T: BitStore, O: BitOrder> ops::DerefMut for BitSet<V, T, O> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.flags
    }
}
impl<V: ?Sized, T: BitStore, O: BitOrder, U: ?Sized> AsRef<U> for BitSet<V, T, O> where
    V: AsRef<U>,
{
    #[inline]
    fn as_ref(&self) -> &U {
        self.flags.as_ref()
    }
}
impl<V: ?Sized, T: BitStore, O: BitOrder, U: ?Sized> AsMut<U> for BitSet<V, T, O> where
    V: AsMut<U>,
{
    #[inline]
    fn as_mut(&mut self) -> &mut U {
        self.flags.as_mut()
    }
}
unsafe impl<V: ?Sized + Send, T: BitStore, O: BitOrder> Send for BitSet<V, T, O> {}
unsafe impl<V: ?Sized + Sync, T: BitStore, O: BitOrder> Sync for BitSet<V, T, O> {}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct FlagSet<F: BitFlagForSet, V = BitVec<<F as BitFlagForSet>::Repr>> {
    pub flags: BitSet<V, <F as BitFlagForSet>::Repr>,
    pub _values: PhantomData<[F]>,
}

impl<F: BitFlagForSet, V> FlagSet<F, V> {
    #[inline]
    pub const fn new(flags: V) -> Self {
        Self {
            flags: BitSet::new(flags),
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

#[cfg(feature = "serde")]
impl<F: BitFlagForSet, V> ser::Serialize for FlagSet<F, V> where
    V: ser::Serialize,
{
    fn serialize<S: ser::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.flags.serialize(serializer)
    }
}
#[cfg(feature = "serde")]
impl<'de, F: BitFlagForSet, V> de::Deserialize<'de> for FlagSet<F, V> where
    V: de::Deserialize<'de>,
{
    fn deserialize<D: de::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        V::deserialize(deserializer).map(Self::new)
    }
}
