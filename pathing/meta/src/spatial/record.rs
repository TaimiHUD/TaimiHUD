use {
    core::{fmt, marker::PhantomData, mem, ptr::NonNull},
    taimi_hoard::{iters::IterExt, lazyfmt},
};

pub type FrameRecordOf<T> = FrameRecord<4, T>;
pub const fn n_len(n: usize) -> usize {
    1 << n
}
pub type FrameId = u32;
#[derive(Clone, Hash)]
pub struct FrameRecord<const N: usize, T: FrameRecordEntry> {
    pub data: [T; N],
    pub position: FrameId,
}

impl<const N: usize, T: FrameRecordEntry> FrameRecord<N, T> {
    pub const EMPTY: Self = match N {
        n if !n.is_power_of_two() => panic!("^2 please"),
        _ => Self {
            data: [const { T::EMPTY }; N],
            position: 0,
        },
    };

    pub fn set_at(&mut self, position: FrameId, v: T) {
        if self.position_in_range(position) {
            *self.get_wrapped_mut(position) = v;
        } else {
            self.advance_to(position);
            *self.front_mut() = v;
        }
    }
    pub fn advance_to(&mut self, position: FrameId) {
        let amt = position.wrapping_sub(self.position);
        let tail = match () {
            #[cfg(todo = "unnecessary")]
            _ => self.iter_data_mut().rev(),
            _ => self.iter_data_to_mut(self.position.wrapping_add(1)),
        };
        for gone in tail.take(amt as usize) {
            gone.clobber();
        }
        self.position = position;
    }
    pub fn push(&mut self, v: T) {
        let next = self.position.wrapping_add(1);
        self.position = next;
        *self.get_wrapped_mut(next) = v;
    }
    pub fn remove_front(&mut self) {
        self.front_mut().clobber();
        self.position = self.position.wrapping_sub(1);
    }
    pub fn remove_back(&mut self) {
        self.back_mut().clobber();
    }
    pub fn get_at(&self, position: FrameId) -> Option<&T> {
        self.position_in_range(position)
            .then(|| self.get_wrapped(position).get_opt())
            .flatten()
    }
    pub fn get_wrapped(&self, position: FrameId) -> &T {
        let start = position as usize % Self::N_LEN;
        unsafe { self.data.get_unchecked(start) }
    }
    pub fn get_at_mut(&mut self, position: FrameId) -> Option<&mut T> {
        self.position_in_range(position)
            .then(|| self.get_wrapped_mut(position).get_opt_mut())
            .flatten()
    }
    pub fn get_wrapped_mut(&mut self, position: FrameId) -> &mut T {
        let start = position as usize % Self::N_LEN;
        unsafe { self.data.get_unchecked_mut(start) }
    }
    #[inline]
    pub fn front(&self) -> &T {
        self.get_wrapped(self.position)
    }
    #[inline]
    pub fn front_mut(&mut self) -> &mut T {
        self.get_wrapped_mut(self.position)
    }
    pub fn back(&self) -> &T {
        self.get_wrapped(self.position.wrapping_add(1))
    }
    pub fn back_mut(&mut self) -> &mut T {
        self.get_wrapped_mut(self.position.wrapping_add(1))
    }
    pub fn position_in_range(&self, position: FrameId) -> bool {
        position.wrapping_sub(self.back_position()) < Self::N_LEN as FrameId
    }
    #[cfg(todo)]
    pub fn position_range(&self) -> ops::RangeToInclusive<FrameId> {
        self.back_position()..=self.position
    }
    pub fn front_position(&self) -> FrameId {
        self.position
    }
    pub fn next_position(&self) -> FrameId {
        self.position.wrapping_add(1)
    }
    const N_LEN_MINUS_1: usize = Self::N_LEN - 1;
    const N_LEN: usize = N;
    pub fn back_position(&self) -> FrameId {
        self.position.wrapping_sub(Self::N_LEN_MINUS_1 as FrameId)
    }
    pub fn iter_populated(&self) -> impl DoubleEndedIterator<Item = (FrameId, &T)> {
        self.iter_all().filter(|(_, v)| !v.is_empty())
    }
    pub fn iter_all(&self) -> impl DoubleEndedIterator<Item = (FrameId, &T)> + ExactSizeIterator {
        self.iter_positions().zip(self.iter_data())
    }
    pub fn iter_all_mut(
        &mut self,
    ) -> impl DoubleEndedIterator<Item = (FrameId, &mut T)> + ExactSizeIterator {
        self.iter_positions().zip(self.iter_data_mut())
    }
    pub fn iter_data(&self) -> impl DoubleEndedIterator<Item = &T> + ExactSizeIterator {
        self.iter_data_from(self.position)
    }
    pub fn iter_data_mut(&mut self) -> impl DoubleEndedIterator<Item = &mut T> + ExactSizeIterator {
        self.iter_data_from_mut(self.position)
    }
    pub fn iter_data_to(&self, end: FrameId) -> impl DoubleEndedIterator<Item = &T> + ExactSizeIterator {
        (0u32..Self::N_LEN as u32).lazy_map(move |i| unsafe {
            self.data
                .get_unchecked(end.wrapping_add(i) as usize % Self::N_LEN)
        })
    }
    #[cfg(todo)]
    pub fn iter_data_to(&self, end: FrameId) -> impl Iterator<Item = &T> {
        let end = end as usize % N;
        let (past, future) = unsafe { self.data.split_at_unchecked(end) };
        future.into_iter().chain(past)
    }
    fn iter_data_from(&self, position: FrameId) -> impl DoubleEndedIterator<Item = &T> + ExactSizeIterator {
        self.iter_data_to(position.wrapping_add(1)).rev()
    }
    fn iter_data_to_mut(
        &mut self,
        end: FrameId,
    ) -> impl DoubleEndedIterator<Item = &mut T> + ExactSizeIterator {
        FrameIterMut::new_to(&mut self.data, end)
    }
    pub fn iter_data_from_mut(
        &mut self,
        position: FrameId,
    ) -> impl DoubleEndedIterator<Item = &mut T> + ExactSizeIterator {
        self.iter_data_to_mut(position.wrapping_add(1)).rev()
    }
    pub fn iter_positions(&self) -> impl DoubleEndedIterator<Item = FrameId> + ExactSizeIterator {
        FramePosIter::new_to::<N>(self.position)
    }
}
impl<const N: usize, T: FrameRecordEntry> Default for FrameRecord<N, T> {
    fn default() -> Self {
        Self::EMPTY
    }
}
impl<const N: usize, T: FrameRecordEntry + fmt::Debug> fmt::Debug for FrameRecord<N, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("FrameRecord")
            .field(&lazyfmt::MaybeFmt::new(|f| {
                let mut sep = None;
                f.write_str("[")?;
                for (i, item) in self.iter_populated() {
                    if let Some(sep) = sep {
                        f.write_str(sep)?;
                    }
                    fmt::Debug::fmt(&(i, item), f)?;
                    // f.alternate() matter at all or no?
                    sep = Some(", ");
                }
                f.write_str("]")
            }))
            .finish()
    }
}

#[inline]
pub fn frame_is_lt(lhs: FrameId, rhs: FrameId) -> bool {
    lhs.wrapping_sub(rhs) & 0x80000000u32 != 0
}
#[inline]
pub fn frame_is_gt(lhs: FrameId, rhs: FrameId) -> bool {
    rhs.wrapping_sub(lhs) & 0x80000000u32 != 0
}

pub trait FrameRecordEntry: Sized {
    const EMPTY: Self;
    fn is_empty(&self) -> bool;
    fn clobber(&mut self) {
        *self = Self::EMPTY;
    }
    #[inline]
    fn get_opt(&self) -> Option<&Self> {
        (!self.is_empty()).then_some(self)
    }
    #[inline]
    fn get_opt_mut(&mut self) -> Option<&mut Self> {
        (!self.is_empty()).then_some(self)
    }
}
impl<T> FrameRecordEntry for Option<T> {
    const EMPTY: Self = None;
    #[inline]
    fn is_empty(&self) -> bool {
        self.is_none()
    }
    fn clobber(&mut self) {
        *self = None;
    }
    #[inline]
    fn get_opt(&self) -> Option<&Self> {
        match self {
            None => None,
            v => Some(v),
        }
    }
}
impl FrameRecordEntry for glam::Mat4 {
    const EMPTY: Self = glam::Mat4::NAN;
    #[inline]
    fn is_empty(&self) -> bool {
        self.x_axis.is_empty()
    }
    fn clobber(&mut self) {
        self.x_axis.clobber();
    }
}
impl FrameRecordEntry for glam::Mat3 {
    const EMPTY: Self = glam::Mat3::NAN;
    #[inline]
    fn is_empty(&self) -> bool {
        self.x_axis.is_empty()
    }
    fn clobber(&mut self) {
        self.x_axis.clobber()
    }
}
impl FrameRecordEntry for glam::Mat3A {
    const EMPTY: Self = glam::Mat3A::NAN;
    #[inline]
    fn is_empty(&self) -> bool {
        self.x_axis.is_empty()
    }
    fn clobber(&mut self) {
        self.x_axis.clobber()
    }
}
impl FrameRecordEntry for glam::Vec3A {
    const EMPTY: Self = glam::Vec3A::NAN;
    #[inline]
    fn is_empty(&self) -> bool {
        self.x.to_bits() == Self::EMPTY.x.to_bits()
    }
    fn clobber(&mut self) {
        self.x = Self::EMPTY.x;
    }
}
impl FrameRecordEntry for glam::Vec3 {
    const EMPTY: Self = glam::Vec3::NAN;
    #[inline]
    fn is_empty(&self) -> bool {
        self.x.to_bits() == Self::EMPTY.x.to_bits()
    }
    fn clobber(&mut self) {
        self.x = Self::EMPTY.x;
    }
}
impl FrameRecordEntry for glam::Vec4 {
    const EMPTY: Self = glam::Vec4::NAN;
    #[inline]
    fn is_empty(&self) -> bool {
        self.x.to_bits() == Self::EMPTY.x.to_bits()
    }
    fn clobber(&mut self) {
        self.x = Self::EMPTY.x;
    }
}
impl<T: glamour::FloatScalar> FrameRecordEntry for glamour::Matrix4<T> {
    const EMPTY: Self = glamour::Matrix4::NAN;
    #[inline]
    fn is_empty(&self) -> bool {
        self.x_axis.x.is_nan()
    }
    fn clobber(&mut self) {
        self.x_axis.x = Self::EMPTY.x_axis.x;
    }
}
impl<T: glamour::Unit> FrameRecordEntry for glamour::Vector3<T>
where
    T::Scalar: glamour::FloatScalar,
{
    const EMPTY: Self = glamour::Vector3::NAN;
    #[inline]
    fn is_empty(&self) -> bool {
        use num_traits::Float;
        self.x.is_nan()
    }
    fn clobber(&mut self) {
        self.x = Self::EMPTY.x;
    }
}

struct FramePosIter(FrameId, FrameId);
impl FramePosIter {
    fn new_to<const N: usize>(
        position: FrameId,
    ) -> impl DoubleEndedIterator<Item = FrameId> + ExactSizeIterator {
        Self(position.wrapping_sub(N as FrameId), position).rev()
    }
    fn new_from<const N: usize>(position: FrameId) -> Self {
        Self(position.wrapping_sub(N as FrameId + 1), position.wrapping_sub(1))
    }
}
impl ExactSizeIterator for FramePosIter {
    #[inline]
    fn len(&self) -> usize {
        self.1.wrapping_sub(self.0) as usize
    }
}
impl Iterator for FramePosIter {
    type Item = FrameId;
    fn next(&mut self) -> Option<Self::Item> {
        if self.0 == self.1 {
            return None
        }
        let next_start = self.0.wrapping_add(1);
        self.0 = next_start;
        Some(next_start)
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
    #[inline]
    fn count(self) -> usize {
        self.len()
    }
    #[inline]
    fn last(mut self) -> Option<Self::Item> {
        self.next_back()
    }
}
impl DoubleEndedIterator for FramePosIter {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.0 == self.1 {
            return None
        }
        let next_end = self.1.wrapping_sub(1);
        Some(mem::replace(&mut self.1, next_end))
    }
}

struct FrameIterMut<'a, const N: usize, T> {
    data: NonNull<[T; N]>,
    pos: FramePosIter,
    _data: PhantomData<&'a mut [T; N]>,
}
impl<'a, const N: usize, T> FrameIterMut<'a, N, T> {
    fn new_to(data: &'a mut [T; N], end: FrameId) -> Self {
        Self {
            #[cfg(todo)]
            data: NonNull::from_mut(data),
            data: unsafe { NonNull::new_unchecked(data as *mut _) },
            pos: FramePosIter::new_from::<N>(end),
            _data: PhantomData,
        }
    }
    #[inline]
    fn start_ptr(&self) -> *mut T {
        self.data.as_ptr().cast()
    }
    #[inline]
    unsafe fn ptr_at(&self, position: FrameId) -> *mut T {
        self.start_ptr().add((position as usize) % N)
    }
}
impl<'a, const N: usize, T> ExactSizeIterator for FrameIterMut<'a, N, T> {
    #[inline]
    fn len(&self) -> usize {
        self.pos.len()
    }
}
impl<'a, const N: usize, T> Iterator for FrameIterMut<'a, N, T> {
    type Item = &'a mut T;
    fn next(&mut self) -> Option<Self::Item> {
        self.pos
            .next()
            .map(|position| unsafe { &mut *self.ptr_at(position) })
    }
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.pos
            .nth(n)
            .map(|position| unsafe { &mut *self.ptr_at(position) })
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.pos.size_hint()
    }
    #[inline]
    fn count(self) -> usize {
        self.pos.count()
    }
    #[inline]
    fn last(mut self) -> Option<Self::Item> {
        self.next_back()
    }
}
impl<'a, const N: usize, T> DoubleEndedIterator for FrameIterMut<'a, N, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.pos
            .next_back()
            .map(|position| unsafe { &mut *self.ptr_at(position) })
    }
    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        self.pos
            .nth_back(n)
            .map(|position| unsafe { &mut *self.ptr_at(position) })
    }
}

#[test]
fn frame_pos_iter() {
    const N: usize = n_len(2);
    let none = None::<bool>;
    let some = Some(true);
    let mut data = FrameRecord::<{ N }, Option<bool>>::EMPTY;
    data.advance_to(2);
    data.push(some);
    data.advance_to(5);
    data.set_at(4, some);
    let expected = [(5u32, &none), (4, &some), (3, &some), (2, &none)];
    let all_positions = {
        let mut iter_pos = data.iter_positions();
        core::iter::from_fn(|| iter_pos.next()).collect::<Vec<_>>()
    };
    assert_eq!(&all_positions[..], &[5u32, 4, 3, 2]);
    let all = data.iter_all().collect::<Vec<_>>();
    assert_eq!(&all[..], &expected[..]);

    data.advance_to(1u32);
    let expected = [
        (1u32, &none),
        (0, &none),
        (u32::MAX, &none),
        (u32::MAX - 1, &none),
    ];
    let all = data.iter_all().collect::<Vec<_>>();
    assert_eq!(&all[..], &expected[..]);

    data.advance_to(u32::MAX);
    data.push(some);
    let expected = [
        (0, &some),
        (u32::MAX, &none),
        (u32::MAX - 1, &none),
        (u32::MAX - 2, &none),
    ];
    let all = data.iter_all().collect::<Vec<_>>();
    assert_eq!(&all[..], &expected[..]);
    let all_mut = data.iter_all_mut().map(|(i, v)| (i, &*v)).collect::<Vec<_>>();
    assert_eq!(&all_mut[..], &expected[..]);

    let mut iter_mut = data.iter_data_mut();
    let iter_mut_count = core::iter::from_fn(|| iter_mut.next().map(drop));
    assert_eq!(iter_mut_count.count(), N);

    for lhs in [0, u32::MAX / 4, u32::MAX / 2, u32::MAX] {
        for (l, r) in [(0, 9), (2, 9), (-2, 9), (-2, 0), (-2, -1), (1, 2)] {
            let rhs = lhs.wrapping_add_signed(r);
            let lhs = lhs.wrapping_add_signed(l);
            assert!(frame_is_lt(lhs, rhs));
            assert!(!frame_is_lt(rhs, lhs));
            assert!(frame_is_gt(rhs, lhs));
            assert!(!frame_is_gt(lhs, rhs));
        }
        assert!(!frame_is_lt(lhs, lhs));
        assert!(!frame_is_gt(lhs, lhs));
    }
}
