#[cfg(feature = "serde")]
use serde::{de, ser};
use {
    crate::{
        collections::TaimiSet,
        flags::{
            bitidx::BitIdx,
            bitptr,
            bitslice,
            BitAddr,
            BitOrder,
            BitRef,
            BitSlice,
            BitStore,
            BitVec,
            BitView,
            BitsLsb,
            BitsNative,
        },
        iters::IterExt as _,
        loc::{LocationGet, LocationMut, LocationRef, Locator},
    },
    core::{hash, marker::PhantomData, mem, ops},
    num_traits::AsPrimitive,
};
pub type BitsOrder = BitsLsb;

pub trait BitFlagForSet: Copy + Clone + Default {
    type Repr: Copy + Clone + Default + PartialEq + Eq + PartialOrd + Ord + hash::Hash + BitStore;
    const BIT_WIDTH: usize;

    fn as_bits(&self) -> &Self::Repr;
    fn as_bits_mut(&mut self) -> &mut Self::Repr;
    fn as_bitslice(&self) -> &BitSlice<Self::Repr, BitsOrder> {
        &self.as_bits().view_bits()[..Self::BIT_WIDTH]
    }
    fn as_bitslice_mut(&mut self) -> &mut BitSlice<Self::Repr, BitsOrder> {
        &mut self.as_bits_mut().view_bits_mut()[..Self::BIT_WIDTH]
    }

    fn range_for(index: usize) -> ops::Range<usize> {
        let start = index * Self::BIT_WIDTH;
        let end = start + Self::BIT_WIDTH;
        start..end
    }

    /// TODO: variants to populate from any shape of bitslice
    unsafe fn from_bitslice_unchecked(bits: &BitSlice<Self::Repr, BitsOrder>) -> Self {
        let mut out = Self::default();
        // not unsafe yet but w/e
        out.as_bitslice_mut().copy_from_bitslice(bits);
        out
    }
}
impl BitFlagForSet for bool {
    type Repr = u8;
    const BIT_WIDTH: usize = 1;

    fn as_bits(&self) -> &Self::Repr {
        todo!()
    }
    fn as_bits_mut(&mut self) -> &mut Self::Repr {
        todo!()
    }
    fn as_bitslice(&self) -> &BitSlice<Self::Repr, BitsLsb> {
        unsafe {
            let idx = BitIdx::new_unchecked(0);
            let addr = BitAddr::from(self).cast::<u8>();
            &*bitptr::bitslice_from_raw_parts(bitptr::BitPtr::new_unchecked(addr, idx), 1)
        }
    }
    fn as_bitslice_mut(&mut self) -> &mut BitSlice<Self::Repr, BitsLsb> {
        unsafe {
            let idx = BitIdx::new_unchecked(0);
            let addr = BitAddr::from(self).cast::<u8>();
            &mut *bitptr::bitslice_from_raw_parts_mut(bitptr::BitPtr::new_unchecked(addr, idx), 1)
        }
    }

    fn range_for(start: usize) -> ops::Range<usize> {
        let end = start + 1;
        start..end
    }

    unsafe fn from_bitslice_unchecked(bits: &BitSlice<Self::Repr, BitsLsb>) -> Self {
        *bits.get_unchecked(0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct BitSet<V: ?Sized = BitVec, T: BitStore = usize, O: BitOrder = BitsNative> {
    pub _bits: PhantomData<bitptr::BitPtr<bitptr::Mut, T, O>>,
    pub flags: V,
}
impl BitSet {
    #[inline]
    pub fn new_vec() -> Self {
        Self::new(BitVec::new())
    }
}
impl<V, T: BitStore, O: BitOrder> BitSet<V, T, O> {
    #[inline]
    pub const fn new(flags: V) -> Self {
        Self { flags, _bits: PhantomData }
    }
    #[inline]
    pub fn into_flags(self) -> V {
        self.flags
    }
}
impl<V: ?Sized, T: BitStore, O: BitOrder> BitSet<V, T, O> {
    #[inline]
    pub const fn from_ref(flags: &V) -> &Self {
        unsafe { mem::transmute(flags) }
    }
    #[inline]
    pub fn from_mut(flags: &mut V) -> &mut Self {
        unsafe { mem::transmute(flags) }
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

    /// shadow surprising deref behaviour
    #[doc(hidden)]
    pub fn remove(&mut self) {}
    /// shadow surprising deref behaviour
    #[doc(hidden)]
    pub fn insert(&mut self) {}
    /// shadow surprising deref behaviour
    #[doc(hidden)]
    pub fn set(&mut self) {}
    #[doc(hidden)]
    pub fn len(&mut self) {}
}
impl<T: BitStore, O: BitOrder> BitSet<BitVec<T, O>, T, O> {
    #[inline]
    pub fn extend_to<L: AsPrimitive<usize>>(&mut self, min_size: L, fill: bool) {
        self.extend_to_size(min_size.as_(), fill)
    }
    #[inline]
    pub fn extend_for<L: AsPrimitive<usize>>(
        &mut self,
        offset: L,
        fill: bool,
    ) -> BitRef<'_, bitptr::Mut, T, O> {
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
        unsafe { self.flags.get_unchecked_mut(offset) }
    }
    pub fn insert_at_offset(&mut self, offset: usize) -> bool {
        let mut dest = self.extend_for_offset(offset, false);
        mem::replace(&mut *dest, true)
    }
    /// like [self.insert_at()] unless present is false, which will instead [self.remove_at()]
    #[inline]
    pub fn insert_at_if<L: AsPrimitive<usize>>(&mut self, offset: L, present: bool) -> bool {
        let offset = offset.as_();
        let Some(offset) = Self::check_offset(offset) else { return false };
        self.insert_at_offset_if(offset, present)
    }
    pub fn insert_at_offset_if(&mut self, offset: usize, present: bool) -> bool {
        match present {
            true => self.insert_at_offset(offset),
            false => self.remove_at_offset(offset).unwrap_or(false),
        }
    }
}
impl<V: ?Sized, T: BitStore, O: BitOrder> BitSet<V, T, O>
where
    V: AsRef<BitSlice<T, O>>,
{
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.as_bitslice().not_any()
    }
    #[inline]
    pub fn count(&self) -> usize {
        self.as_bitslice().count_ones()
    }
    /// allocated capacity, effectively highest index+1
    ///
    /// (not necessarily max insert index though, since no eager truncation occurs)
    #[inline]
    pub fn end_len(&self) -> usize {
        self.as_bitslice().len()
    }

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
        #[cfg(todo = "unnecessary")]
        let Some(offset) = Self::check_offset(offset) else {
            return None
        };
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
    #[inline]
    pub fn slice_ref_at_offset(&self, offset: usize) -> Option<&BitSlice<T, O>> {
        self.get_ref_at_offset(offset)
            .map(|p| unsafe { &*bitptr::bitslice_from_raw_parts(p.into_bitptr(), 1) })
    }
    #[inline]
    pub fn as_bitslice(&self) -> &BitSlice<T, O> {
        self.as_ref()
    }

    pub fn iter_of<L>(&self) -> impl Iterator<Item = L> + '_
    where
        L: Copy + 'static,
        usize: AsPrimitive<L>,
    {
        self.as_bitslice().iter_ones().lazy_map(|i| i.as_())
    }
}
impl<V: ?Sized, T: BitStore, O: BitOrder> BitSet<V, T, O>
where
    V: AsMut<BitSlice<T, O>>,
{
    #[inline]
    pub fn remove_at<L: AsPrimitive<usize>>(&mut self, offset: L) -> Option<bool> {
        self.remove_at_offset(offset.as_())
    }
    pub fn remove_at_offset(&mut self, offset: usize) -> Option<bool> {
        self.get_mut_at_offset(offset)
            .map(|mut b| mem::replace(&mut *b, false))
    }

    #[inline]
    pub fn get_mut_at<L: AsPrimitive<usize>>(
        &mut self,
        offset: L,
    ) -> Option<BitRef<'_, bitptr::Mut, T, O>> {
        let offset = offset.as_();
        let Some(offset) = Self::check_offset(offset) else { return None };
        self.get_mut_at_offset(offset)
    }
    #[inline]
    pub fn get_mut_at_offset(&mut self, offset: usize) -> Option<BitRef<'_, bitptr::Mut, T, O>> {
        self.flags.as_mut().get_mut(offset)
    }
    #[inline]
    pub fn slice_mut_at_offset(&mut self, offset: usize) -> Option<&mut BitSlice<T, O>> {
        self.get_mut_at(offset)
            .map(|p| unsafe { &mut *bitptr::bitslice_from_raw_parts_mut(p.into_bitptr(), 1) })
    }
    #[inline]
    pub fn as_bitslice_mut(&mut self) -> &mut BitSlice<T, O> {
        self.as_mut()
    }
}
impl<L, V: ?Sized, T: BitStore, O: BitOrder> TaimiSet<L> for BitSet<V, T, O>
where
    V: AsRef<BitSlice<T, O>>,
    L: AsPrimitive<usize>,
{
    #[inline]
    fn set_contains(&self, offset: &L) -> bool {
        self.contains(*offset)
    }
}
/// TODO: consider whether `None` is meaningful (length) vs `Some(false)`
impl<N, L, V: ?Sized, T: BitStore, O: BitOrder> LocationGet<N, L> for BitSet<V, T, O>
where
    V: AsRef<BitSlice<T, O>>,
    L: AsPrimitive<usize>,
{
    type LookupGet = bool;
    #[inline]
    fn lookup_get(&self, loc: &Locator<N, L>) -> Option<Self::LookupGet> {
        Some(self.contains(loc.path))
    }
}
impl<N, L, V: ?Sized, T: BitStore, O: BitOrder> LocationRef<N, L> for BitSet<V, T, O>
where
    V: AsRef<BitSlice<T, O>>,
    L: AsPrimitive<usize>,
{
    type LookupRef = BitSlice<T, O>;
    #[inline]
    fn lookup_ref(&self, loc: &Locator<N, L>) -> Option<&Self::LookupRef> {
        self.slice_ref_at_offset(loc.path.as_())
    }
}
impl<N, L, V: ?Sized, T: BitStore, O: BitOrder> LocationMut<N, L> for BitSet<V, T, O>
where
    V: AsRef<BitSlice<T, O>> + AsMut<BitSlice<T, O>>,
    L: AsPrimitive<usize>,
{
    #[inline]
    fn lookup_mut(&mut self, loc: &Locator<N, L>) -> Option<&mut Self::LookupRef> {
        self.slice_mut_at_offset(loc.path.as_())
    }
}
impl<T: BitStore, O: BitOrder, L: AsPrimitive<usize>> Extend<L> for BitSet<BitVec<T, O>, T, O> {
    fn extend<I: IntoIterator<Item = L>>(&mut self, iter: I) {
        let iter = iter.into_iter();
        let (min, max) = iter.size_hint();
        if let Some(max) = max {
            self.reserve_exact(max);
        } else {
            self.reserve(min);
        }
        for offset in iter {
            self.insert_at(offset);
        }
    }
}
impl<T: BitStore, O: BitOrder, L: AsPrimitive<usize>> FromIterator<L> for BitSet<BitVec<T, O>, T, O> {
    fn from_iter<I: IntoIterator<Item = L>>(iter: I) -> Self {
        let mut set = Self::default();
        set.extend(iter);
        set
    }
}
impl<V: Default, T: BitStore, O: BitOrder> Default for BitSet<V, T, O> {
    #[inline]
    fn default() -> Self {
        Self::new(V::default())
    }
}
/// TODO: this may be more confusing and trouble than it's worth?
impl<V: ?Sized, T: BitStore, O: BitOrder> ops::Deref for BitSet<V, T, O> {
    type Target = V;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.flags
    }
}
/// TODO: this may be more confusing and trouble than it's worth?
impl<V: ?Sized, T: BitStore, O: BitOrder> ops::DerefMut for BitSet<V, T, O> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.flags
    }
}
impl<V: ?Sized, T: BitStore, O: BitOrder, U: ?Sized> AsRef<U> for BitSet<V, T, O>
where
    V: AsRef<U>,
{
    #[inline]
    fn as_ref(&self) -> &U {
        self.flags.as_ref()
    }
}
impl<V: ?Sized, T: BitStore, O: BitOrder, U: ?Sized> AsMut<U> for BitSet<V, T, O>
where
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
    #[inline]
    pub fn with_capacity(amt: usize) -> Self {
        let len = F::range_for(amt).start;
        let mut flags = BitVec::new();
        flags.reserve_exact(len);
        Self::new(flags)
    }
    pub fn with_len(amt: usize, fill: bool) -> Self {
        let len = F::range_for(amt).start;
        let mut flags = BitVec::new();
        flags.reserve_exact(len);
        flags.resize(len, fill);
        Self::new(flags)
    }

    pub fn extend_to(&mut self, min_len: usize, fill: bool) {
        let min_size = min_len * F::BIT_WIDTH;
        if self.flags.flags.len() < min_size {
            self.flags.flags.resize(min_size, fill);
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

    pub fn push(&mut self, flags: &F) {
        self.flags.flags.extend_from_bitslice(flags.as_bitslice());
    }
}

impl<F: BitFlagForSet, V> FlagSet<F, V>
where
    V: AsRef<BitSlice<F::Repr, BitsOrder>>,
{
    #[inline]
    pub fn get(&self, index: usize) -> Option<F> {
        self.get_ref(index)
            .map(|flags| unsafe { F::from_bitslice_unchecked(flags) })
    }
    pub fn get_ref(&self, index: usize) -> Option<&BitSlice<F::Repr, BitsOrder>> {
        let range = F::range_for(index);
        self.as_bitslice().get(range)
    }
    pub fn get_for<L: AsPrimitive<usize>>(&self, path: L) -> Option<F> {
        self.get(path.as_())
    }

    #[inline]
    pub fn as_bitslice(&self) -> &BitSlice<F::Repr, BitsOrder> {
        self.flags.as_ref()
    }

    #[inline]
    pub fn iter_chunks(&self) -> bitslice::ChunksExact<'_, F::Repr, BitsOrder> {
        self.as_bitslice().chunks_exact(F::BIT_WIDTH)
    }

    #[inline]
    pub fn iter<'a>(&'a self) -> <&'a Self as IntoIterator>::IntoIter {
        IntoIterator::into_iter(self)
    }
}

impl<F: BitFlagForSet, V> FlagSet<F, V>
where
    V: AsMut<BitSlice<F::Repr, BitsOrder>>,
{
    /// TODO: copy unchecked?
    pub fn set(&mut self, index: usize, value: F) -> Result<(), ()> {
        self.get_mut(index)
            .map(|flags| {
                let value = value.as_bitslice();
                flags.copy_from_bitslice(value)
            })
            .ok_or(())
    }
    pub fn set_at<L: AsPrimitive<usize>>(&mut self, path: L, value: F) -> Result<(), ()> {
        self.set(path.as_(), value)
    }
    pub fn get_mut(&mut self, index: usize) -> Option<&mut BitSlice<F::Repr, BitsOrder>> {
        let range = F::range_for(index);
        self.as_bitslice_mut().get_mut(range)
    }

    #[inline]
    pub fn as_bitslice_mut(&mut self) -> &mut BitSlice<F::Repr, BitsOrder> {
        self.flags.as_mut()
    }
    #[inline]
    pub fn iter_chunks_mut(&mut self) -> bitslice::ChunksExactMut<'_, F::Repr, BitsOrder> {
        self.as_bitslice_mut().chunks_exact_mut(F::BIT_WIDTH)
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
            self.push(&flag);
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
impl<'a, N, L, F: BitFlagForSet, V> LocationGet<N, L> for FlagSet<F, V>
where
    V: AsRef<BitSlice<F::Repr, BitsOrder>>,
    L: AsPrimitive<usize>,
{
    type LookupGet = F;
    #[inline]
    fn lookup_get(&self, loc: &Locator<N, L>) -> Option<Self::LookupGet> {
        self.get(loc.path.as_())
    }
}
impl<'a, N, L, F: BitFlagForSet, V> LocationRef<N, L> for FlagSet<F, V>
where
    V: AsRef<BitSlice<F::Repr, BitsOrder>>,
    L: AsPrimitive<usize>,
{
    type LookupRef = BitSlice<F::Repr, BitsOrder>;
    #[inline]
    fn lookup_ref(&self, loc: &Locator<N, L>) -> Option<&Self::LookupRef> {
        self.get_ref(loc.path.as_())
    }
}
impl<'a, N, L, F: BitFlagForSet, V> LocationMut<N, L> for FlagSet<F, V>
where
    V: AsRef<BitSlice<F::Repr, BitsOrder>> + AsMut<BitSlice<F::Repr, BitsOrder>>,
    L: AsPrimitive<usize>,
{
    #[inline]
    fn lookup_mut(&mut self, loc: &Locator<N, L>) -> Option<&mut Self::LookupRef> {
        self.get_mut(loc.path.as_())
    }
}
fn bitslice_chunk_into_exact_unchecked<'a, F: BitFlagForSet>(bits: &'a BitSlice<F::Repr, BitsOrder>) -> F {
    unsafe { F::from_bitslice_unchecked(bits) }
}
impl<'a, F: BitFlagForSet, V> IntoIterator for &'a FlagSet<F, V>
where
    V: AsRef<BitSlice<F::Repr, BitsOrder>>,
{
    type IntoIter = crate::iters::LazyMapFn<
        bitslice::ChunksExact<'a, F::Repr, BitsOrder>,
        fn(&'a BitSlice<F::Repr, BitsOrder>) -> F,
    >;
    type Item = F;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter_chunks()
            .lazy_map(bitslice_chunk_into_exact_unchecked::<F>)
    }
}

#[cfg(todo)]
mod flag_set_iter {
    pub struct FlagSetIter<
        'a,
        F: BitFlagForSet,
        M = bitptr::Const,
        T = <F as BitFlagForSet>::Repr,
        O = BitsOrder,
    >
    where
        F: BitFlagForSet,
        M: BitMutability,
        T: BitStore + 'a,
        O: BitOrder,
    {
        range: BitPtrRange<M, T, O>,
        bits: PhantomData<&'a [F]>,
    }
    impl<'a, F, M, T, O> FlagSetIter<'a, F, M, T, O>
    where
        F: BitFlagForSet,
        M: BitMutability,
        T: BitStore + 'a,
        O: BitOrder,
    {
        #[inline]
        pub const unsafe fn new_unchecked(range: BitPtrRange<M, T, O>) -> Self {
            Self { range, bits: PhantomData }
        }
        pub fn bit_ptr(&self) -> &BitRange<M, T, O> {
            &self.range
        }
        pub unsafe fn bit_ptr_mut(&mut self) -> &mut BitRange<M, T, O> {
            &mut self.range
        }
    }
    impl<'a, F, M, T, O> Clone for FlagSetIter<'a, F, bitptr::Const, T, O>
    where
        F: BitFlagForSet,
        T: BitStore + 'a,
        O: BitOrder,
    {
        #[inline]
        fn clone(&self) -> Self {
            unsafe { Self::new_unchecked(self.range) }
        }
    }
    impl<'a, F, M, T, O> Send for FlagSetIter<'a, F, M, T, O>
    where
        F: BitFlagForSet + Send,
        M: BitMutability,
        T: BitStore + 'a,
        O: BitOrder,
        &'a mut BitSlice<T, O>: Send,
    {
    }
    impl<'a, F, M, T, O> Sync for FlagSetIter<'a, F, M, T, O>
    where
        F: BitFlagForSet + Sync,
        M: BitMutability,
        T: BitStore + 'a,
        O: BitOrder,
        BitSlice<T, O>: Sync,
    {
    }
    impl<'a, F, M, T, O> Iterator for FlagSetIter<'a, F, M, T, O>
    where
        F: BitFlagForSet + Sync,
        M: BitMutability,
        T: BitStore + 'a,
        O: BitOrder,
        BitSlice<T, O>: Sync,
    {
        type Item = &BitSlice<T, O>;
        fn next(&mut self) -> Option<Self::Item> {}

        fn nth(&mut self, n: usize) -> Option<Self::Item> {}
    }
}

#[cfg(feature = "serde")]
impl<F: BitFlagForSet, V> ser::Serialize for FlagSet<F, V>
where
    V: ser::Serialize,
{
    fn serialize<S: ser::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.flags.serialize(serializer)
    }
}
#[cfg(feature = "serde")]
impl<'de, F: BitFlagForSet, V> de::Deserialize<'de> for FlagSet<F, V>
where
    V: de::Deserialize<'de>,
{
    fn deserialize<D: de::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        V::deserialize(deserializer).map(Self::new)
    }
}
