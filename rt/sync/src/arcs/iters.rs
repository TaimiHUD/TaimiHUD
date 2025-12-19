#![cfg(todo)]
//! bleh

use std::{
    cell::UnsafeCell,
    pin::Pin,
    sync::Arc,
    ops,
    slice::SliceIndex,
    mem,
    fmt,
    iter,
};

/// ehhh bleh this is not okay though :<
pub struct ArcIter<T: ?Sized + 'static> where
    for<'a> &'a T: IntoIterator,
{
    iter: UnsafeCell<<&'static T as IntoIterator>::IntoIter>,
    inner: Pin<Arc<T>>,
}
impl<T: ?Sized + 'static> ArcIter<T> where
    for<'a> &'a T: IntoIterator,
{
    pub fn new(inner: Arc<T>) -> Self where
        T: Unpin,
    {
        Self::new_pinned(Pin::new(inner))
    }
    pub fn new_pinned(inner: Pin<Arc<T>>) -> Self {
        let iter = UnsafeCell::new(unsafe {
            IntoIterator::into_iter(&*(&*inner as *const T))
        });
        Self { iter, inner }
    }

    pub fn inner(&self) -> &Pin<Arc<T>> {
        &self.inner
    }
    pub unsafe fn into_inner(self) -> Arc<T> {
        let Self { iter, inner } = self;
        drop(iter);
        Pin::into_inner_unchecked(inner)
    }
    pub unsafe fn inner_mut(&mut self) -> &mut Pin<Arc<T>> {
        &mut self.inner
    }
    pub unsafe fn inner_iter<'a>(&'a self) -> &'a <&'a T as IntoIterator>::IntoIter {
        let iter = self.iter.get() as *mut <&'a T as IntoIterator>::IntoIter;
        &*(iter as *const _)
    }
    pub unsafe fn inner_iter_mut<'a>(&'a mut self) -> &'a mut <&'a T as IntoIterator>::IntoIter {
        let iter = self.iter.get() as *mut <&'a T as IntoIterator>::IntoIter;
        &mut *iter
    }
}
impl<T: ?Sized + 'static, I> fmt::Debug for ArcIter<T> where
    for<'a> &'a T: IntoIterator<IntoIter = I>,
    I: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("ArcIter")
            .field(&(&*self.inner as *const T))
            .field(&self.iter)
            .finish()
    }
}
impl<T: ?Sized + 'static, I> Iterator for ArcIter<T> where
    for<'a> &'a T: IntoIterator<Item = I>,
    I: 'static,
{
    type Item = I;
    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            self.inner_iter_mut().next()
        }
    }

    fn nth(&mut self, nth: usize) -> Option<Self::Item> {
        unsafe {
            self.inner_iter_mut().nth(nth)
        }
    }
}
impl<T: ?Sized + 'static, I> Clone for ArcIter<T> where
    for<'a> &'a T: IntoIterator<IntoIter = I>,
    I: Clone,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            iter: UnsafeCell::new(unsafe { self.inner_iter().clone() }),
        }
    }
}
unsafe impl<T: ?Sized + 'static, I> Send for ArcIter<T> where
    for<'a> &'a T: IntoIterator<IntoIter = I>,
    Arc<T>: Send,
    I: Send,
{}
unsafe impl<T: ?Sized + 'static, I> Sync for ArcIter<T> where
    for<'a> &'a T: IntoIterator<IntoIter = I>,
    Arc<T>: Sync,
    I: Sync,
{}
impl<T: ?Sized + 'static, I> Unpin for ArcIter<T> where
    for<'a> &'a T: IntoIterator<IntoIter = I>,
    I: Unpin,
{}

pub struct ArcSliceIter<T: ?Sized, I: ?Sized = ops::RangeFrom<usize>> {
    pub inner: Arc<T>,
    pub idx: I,
}
impl<T: ?Sized, I> ArcSliceIter<T, I> {
    pub fn with_parts(inner: Arc<T>, idx: I) -> Self {
        Self { inner, idx }
    }
    pub fn new(inner: Arc<T>) -> Self where
        I: Default,
    {
        Self::with_parts(inner, I::default())
    }
}
impl<T: ?Sized, U> ArcSliceIter<T, ops::Range<usize>> where
    T: ops::Index<ops::RangeFull, Output = [U]>,
{
    pub fn new_iter(inner: Arc<T>) -> Self {
        let len = inner[..].len();
        Self::new_to(inner, len)
    }
    pub fn new_to(inner: Arc<T>, end: usize) -> Self {
        Self::with_parts(inner, 0..end)
    }
}
impl<T: ?Sized> ArcSliceIter<T, ops::RangeFrom<usize>> {
    pub fn new_from(inner: Arc<T>, start: usize) -> Self {
        Self::with_parts(inner, start..)
    }
}
impl<T: ?Sized> ArcSliceIter<T, ops::RangeInclusive<usize>> {
    pub fn new_at(inner: Arc<T>, at: usize) -> Self {
        Self::with_parts(inner, at..=at)
    }
}
impl<'a, T: ?Sized, U, I> ArcSliceIter<T, I> where
    T: ops::Index<ops::RangeFull, Output = [U]>,
    I: Clone + Iterator,
    <I as Iterator>::Item: SliceIndex<[U]>,
    U: 'a,
{
    pub fn peek_ref(&'a self) -> Option<&'a <<I as Iterator>::Item as SliceIndex<[U]>>::Output> {
        self.idx.clone().next().and_then(|idx|
            self.inner[..].get(idx)
        )
    }
    pub fn peek_arc(&self) -> Option<ArcSliceAt<T, I>> where
        Self: Clone,
    {
        ArcSliceAt::with_inner(self.clone())
    }
}
impl<'a, T: ?Sized, I: ?Sized, U> ArcSliceIter<T, I> where
    T: ops::Index<ops::RangeFull, Output = [U]>,
    I: Clone + Iterator,
    <I as Iterator>::Item: SliceIndex<[U]>,
    U: 'a,
    // TODO: clone not technically necessary...
{
    pub fn next_ref(&'a mut self) -> Option<&'a <<I as Iterator>::Item as SliceIndex<[U]>>::Output> {
        let prev = self.idx.clone();
        let idx = self.idx.next()?;
        let next = self.inner[..].get(idx);
        if next.is_none() {
            self.idx = prev;
        }
        next
    }
    pub fn next_arc(&'a mut self) -> Option<ArcSliceAt<T, I>> where
        Self: Clone,
    {
        let next = ArcSliceAt::with_inner(self.clone());
        if next.is_some() {
            let _ = self.idx.next();
        }
        next
    }
    pub fn iter_arc(&'a self) -> impl Iterator<Item = ArcSliceAt<T, ops::RangeInclusive<usize>>> + 'static where
        I: Iterator<Item = usize> + 'static,
    {
        self.clone().into_iter_arc()
    }
}
impl<'a, T: ?Sized, I: ?Sized, U> ArcSliceIter<T, I> where
    T: ops::Index<ops::RangeFull, Output = [U]>,
    I: Clone + Iterator,
    <I as Iterator>::Item: SliceIndex<[U]>,
{
    pub fn into_iter_arc(mut self) -> impl Iterator<Item = ArcSliceAt<T, ops::RangeInclusive<usize>>> + 'static where
        I: Iterator<Item = usize> + 'static,
    {
        iter::from_fn(move || {
            let idx = self.idx.next()?;
            ArcSliceAt::new(self.inner.clone(), idx..=idx)
        })
    }
}
impl<'a, T: ?Sized, I: ?Sized> ArcSliceIter<T, I> where
    &'a T: IntoIterator + 'a,
{
    fn iter_at(&'a self) -> <&'a T as IntoIterator>::IntoIter {
        IntoIterator::into_iter(&*self.inner)
    }
}
impl<'a, T: ?Sized, I> ArcSliceIter<T, I> where
    &'a T: IntoIterator + 'a,
    I: Clone + Iterator<Item = usize>,
{
    pub fn peek_at(&'a self) -> Option<<&'a T as IntoIterator>::Item> {
        self.idx.clone().next().and_then(|idx|
            self.iter_at().nth(idx)
        )
    }
    pub fn next_at(&'a mut self) -> Option<<&'a T as IntoIterator>::Item> {
        let prev = self.idx.clone();
        let idx = self.idx.next()?;
        let next = IntoIterator::into_iter(&*self.inner).nth(idx);
        if next.is_none() {
            self.idx = prev;
        }
        next
    }
}
#[cfg(todo)]
impl<T: ?Sized, U> Iterator for ArcSliceIter<T, usize> where
    T: ops::Index<ops::RangeFull, Output = [U]>,
    usize: SliceIndex<[U]>,
    <usize as SliceIndex<[U]>>::Output: Clone,
{
    type Item = <usize as SliceIndex<[U]>>::Output;
    fn next(&mut self) -> Option<Self::Item> {
        self.next_ref().cloned()
    }
}
impl<T: ?Sized, I, II> Iterator for ArcSliceIter<T, I> where
    for<'a> &'a T: IntoIterator<Item = II>,
    I: Clone + Iterator<Item = usize>,
{
    type Item = II;
    fn next(&mut self) -> Option<Self::Item> {
        self.next_at()
    }

    fn nth(&mut self, nth: usize) -> Option<Self::Item> {
        let prev = self.idx.clone();
        let idx = self.idx.nth(nth)?;
        match IntoIterator::into_iter(&*self.inner).nth(idx) {
            None => {
                self.idx = prev;
                None
            },
            item => item,
        }
    }
}
impl<T: ?Sized, I, II> ExactSizeIterator for ArcSliceIter<T, I> where
    for<'a> &'a T: IntoIterator<IntoIter = II>,
    II: ExactSizeIterator,
    I: ExactSizeIterator,
    Self: Iterator,
{
    fn len(&self) -> usize {
        self.iter_at().len().min(self.idx.len())
    }
}
impl<T: ?Sized, I> DoubleEndedIterator for ArcSliceIter<T, I> where
    for<'a> &'a T: IntoIterator<Item = <Self as Iterator>::Item>,
    I: DoubleEndedIterator<Item = usize>,
    Self: Iterator,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        let nth = self.idx.next_back()?;
        self.iter_at().nth(nth)
    }
}
impl<T: ?Sized, I: Clone> Clone for ArcSliceIter<T, I> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            idx: self.idx.clone(),
        }
    }
}
impl<T> fmt::Debug for ArcSliceIter<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("ArcSliceIter")
            .field(&Arc::as_ptr(&self.inner))
            .field(&self.idx)
            .finish()
    }
}

#[repr(transparent)]
pub struct ArcSliceAt<T: ?Sized, I: ?Sized> {
    inner: ArcSliceIter<T, I>,
}
impl<T: ?Sized, I: ?Sized> ArcSliceAt<T, I> {
    pub unsafe fn with_inner_ref_unchecked(inner: &ArcSliceIter<T, I>) -> &Self {
        mem::transmute(inner)
    }
    pub unsafe fn with_inner_mut_unchecked(inner: &mut ArcSliceIter<T, I>) -> &mut Self {
        mem::transmute(inner)
    }

    pub fn inner_ref(&self) -> &ArcSliceIter<T, I> {
        &self.inner
    }
    pub unsafe fn inner_mut(&mut self) -> &mut ArcSliceIter<T, I> {
        &mut self.inner
    }
}
impl<T: ?Sized, I> ArcSliceAt<T, I> {
    pub unsafe fn with_inner_unchecked(inner: ArcSliceIter<T, I>) -> Self {
        Self { inner }
    }
    pub unsafe fn new_unchecked(inner: Arc<T>, idx: I) -> Self {
        let inner = ArcSliceIter::with_parts(inner, idx);
        Self::with_inner_unchecked(inner)
    }

    pub fn into_inner(self) -> ArcSliceIter<T, I> {
        self.inner
    }
}
impl<T: ?Sized, I: ?Sized, U> ArcSliceAt<T, I> where
    T: ops::Index<ops::RangeFull, Output = [U]>,
    I: Clone + Iterator,
    <I as Iterator>::Item: SliceIndex<[U]>,
{
    pub fn with_inner(inner: ArcSliceIter<T, I>) -> Option<Self> where
        I: Sized,
    {
        let _ = inner.peek_ref().map(drop)?;
        Some(unsafe {
            Self::with_inner_unchecked(inner)
        })
    }
    pub fn with_inner_ref(inner: &ArcSliceIter<T, I>) -> Option<&Self> {
        let _ = inner.peek_ref().map(drop)?;
        Some(unsafe {
            Self::with_inner_ref_unchecked(inner)
        })
    }
    pub fn new(inner: Arc<T>, idx: I) -> Option<Self> where
        I: Sized,
    {
        Self::with_inner(ArcSliceIter::with_parts(inner, idx))
    }

    pub fn get_nth(&self, nth: usize) -> Option<ArcSliceAt<T, <I as Iterator>::Item>> where
        I: Iterator,
        <I as Iterator>::Item: Clone + SliceIndex<[U]>,
    {
        let i = self.inner.idx.clone().nth(nth)?;
        let _ = self.inner.inner[..].get(i.clone()).map(drop)?;
        Some(unsafe {
            ArcSliceAt::new_unchecked(self.inner.inner.clone(), i)
        })
    }
    pub unsafe fn get_nth_unchecked(&self, nth: usize) -> ArcSliceAt<T, <I as Iterator>::Item> where
        I: Iterator,
    {
        let i = self.inner.idx.clone().nth(nth).unwrap_unchecked();
        ArcSliceAt::new_unchecked(self.inner.inner.clone(), i)
    }
}
impl<T: ?Sized, U> ArcSliceAt<T, ops::RangeInclusive<usize>> where
    T: ops::Index<ops::RangeFull, Output = [U]>,
    <ops::RangeInclusive<usize> as Iterator>::Item: SliceIndex<[U]>,
{
    pub fn new_at(inner: Arc<T>, at: usize) -> Option<Self> {
        Self::new(inner, at..=at)
    }
}
impl<'a, T: ?Sized, I: ?Sized, U> ArcSliceAt<T, I> where
    T: ops::Index<ops::RangeFull, Output = [U]>,
    I: Clone + Iterator,
    <I as Iterator>::Item: SliceIndex<[U]>,
    U: 'a,
{
    pub fn at_ref(&'a self) -> &'a <<I as Iterator>::Item as SliceIndex<[U]>>::Output {
        unsafe {
            self.inner.peek_ref().unwrap_unchecked()
        }
    }
    pub fn nth_ref(&'a self, nth: usize) -> Option<&'a <<I as Iterator>::Item as SliceIndex<[U]>>::Output> where
        I: Iterator,
        <I as Iterator>::Item: SliceIndex<[U]>,
    {
        let i = self.inner.idx.clone().nth(nth)?;
        self.inner.inner[..].get(i)
    }
}
impl<T: ?Sized, I, U> ops::Deref for ArcSliceAt<T, I> where
    T: ops::Index<ops::RangeFull, Output = [U]>,
    I: Clone + Iterator,
    <I as Iterator>::Item: SliceIndex<[U]>,
    // :<
    U: 'static,
{
    type Target = <<I as Iterator>::Item as SliceIndex<[U]>>::Output;

    fn deref(&self) -> &Self::Target {
        self.at_ref()
    }
}
impl<T: ?Sized, I, U> AsRef<<<I as Iterator>::Item as SliceIndex<[U]>>::Output> for ArcSliceAt<T, I> where
    T: ops::Index<ops::RangeFull, Output = [U]>,
    I: Clone + Iterator,
    <I as Iterator>::Item: SliceIndex<[U]>,
    // :<
    U: 'static,
{
    fn as_ref(&self) -> &<<I as Iterator>::Item as SliceIndex<[U]>>::Output {
        self.at_ref()
    }
}
impl<T: ?Sized, I> Clone for ArcSliceAt<T, I> where
    ArcSliceIter<T, I>: Clone,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}
