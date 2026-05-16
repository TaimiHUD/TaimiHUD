use core::{borrow::BorrowMut, ops};

/// something that can be reverted to a prior or default state for use in `dyn` contexts
///
/// see also: [CheckpointRewind]
pub trait Rewind {
    fn rewind(&mut self);
}

impl dyn Rewind {
    #[inline]
    pub fn impl_default_rewind<T>(this: &mut T)
    where
        T: Default,
    {
        *this = T::default();
    }
}

/// a convenient dyn alias for [Rewind] + [Iterator]
pub trait RewindableIterator: Iterator + Rewind {}
impl<T: ?Sized> RewindableIterator for T where T: Iterator + Rewind {}
/// see [RewindableIterator]
pub trait RewindableSizeIterator: RewindableIterator + ExactSizeIterator {}
#[cfg(todo = "unnecessary")]
pub trait RewindableSizeIterator: ExactSizeIterator + Rewind {}
impl<T: ?Sized> RewindableSizeIterator for T where T: ExactSizeIterator + Rewind {}
#[cfg(todo)]
pub trait RewindableDoubleEndedIterator: RewindableIterator + DoubleEndedIterator {}
#[cfg(todo)]
impl<T: ?Sized> RewindableDoubleEndedIterator for T where T: DoubleEndedIterator + Rewind {}

/// unfortunately of limited use if you can't impl additional traits for it...
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CheckpointRewind<C, T = C> {
    pub inner: T,
    pub checkpoint: C,
}

impl<C, T> CheckpointRewind<C, T> {
    #[inline]
    pub const fn new_with_checkpoint(inner: T, checkpoint: C) -> Self {
        Self { inner, checkpoint }
    }
    #[inline]
    pub fn new(inner: T) -> Self
    where
        T: Clone,
        C: From<T>,
    {
        Self {
            checkpoint: inner.clone().into(),
            inner,
        }
    }

    #[inline]
    pub fn into_inner(self) -> T {
        self.inner
    }
}
impl<C, T> CheckpointRewind<C, T>
where
    T: BorrowMut<C>,
    C: Clone,
{
    #[inline]
    pub fn new_snapshot(mut inner: T) -> Self {
        let checkpoint = inner.borrow_mut().clone();
        Self::new_with_checkpoint(inner, checkpoint)
    }
}
impl<'a, C> CheckpointRewind<C, &'a mut C>
where
    C: Clone,
{
    #[inline]
    pub fn new_snapshot_mut(inner: &'a mut C) -> Self {
        Self::new_snapshot(inner)
    }
}
impl<C, T> Rewind for CheckpointRewind<C, T>
where
    T: BorrowMut<C>,
    C: Clone,
{
    fn rewind(&mut self) {
        *self.inner.borrow_mut() = self.checkpoint.clone();
    }
}
impl<C, T> ops::Deref for CheckpointRewind<C, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl<T: Clone> ops::DerefMut for CheckpointRewind<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
impl<C, T> From<T> for CheckpointRewind<C, T>
where
    T: Clone,
    C: From<T>,
{
    #[inline]
    fn from(inner: T) -> Self {
        Self::new(inner)
    }
}

impl<C, T: Clone, U: ?Sized> AsRef<U> for CheckpointRewind<C, T>
where
    T: AsRef<U>,
{
    #[inline]
    fn as_ref(&self) -> &U {
        self.inner.as_ref()
    }
}
impl<C, T: Clone, U: ?Sized> AsMut<U> for CheckpointRewind<C, T>
where
    T: AsMut<U>,
{
    #[inline]
    fn as_mut(&mut self) -> &mut U {
        self.inner.as_mut()
    }
}
impl<C, T> Iterator for CheckpointRewind<C, T>
where
    T: Iterator,
{
    type Item = T::Item;
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.inner.nth(n)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
    fn count(self) -> usize
    where
        Self: Sized,
    {
        self.inner.count()
    }
    fn last(self) -> Option<Self::Item>
    where
        Self: Sized,
    {
        self.inner.last()
    }
}
impl<C, T> ExactSizeIterator for CheckpointRewind<C, T>
where
    T: ExactSizeIterator,
{
    fn len(&self) -> usize {
        self.inner.len()
    }
}
impl<C, T> DoubleEndedIterator for CheckpointRewind<C, T>
where
    T: DoubleEndedIterator,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner.next_back()
    }
    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        self.inner.nth_back(n)
    }
}
