#[cfg(feature = "serde")]
use serde::{ser, de};
use crate::loc::Locator;
use bitvec::{order::Lsb0, slice::BitSlice, vec::BitVec, view::BitView, store::BitStore};
use core::{ops, hash, marker::PhantomData};

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
