use {
    crate::iters::tree,
    core::{fmt, iter, mem},
    std::{borrow::Cow, slice},
};

/// unlike [iter::Map], [self.map] is not guaranteed to run in sequence
/// for all items, such as when [Iterator::nth] is used
#[derive(Copy, Clone)]
pub struct LazyMapFn<I, F> {
    pub map: F,
    pub iter: I,
}

impl<I, F> LazyMapFn<I, F> {
    #[inline]
    pub const fn new(map: F, iter: I) -> Self {
        Self { map, iter }
    }

    pub fn new_default(map: F) -> Self
    where
        I: Default,
    {
        Self::new(map, I::default())
    }

    pub fn into_inner(self) -> I {
        self.iter
    }

    pub fn map_iter<T, M: FnOnce(I) -> T>(self, f: M) -> LazyMapFn<T, F> {
        let Self { map, iter } = self;
        LazyMapFn::new(map, f(iter))
    }
}
impl<I: Iterator, F> LazyMapFn<I, F> {
    pub fn map<T, R, M: FnMut(R) -> T>(self, mut f: M) -> LazyMapFn<I, impl FnMut(I::Item) -> T>
    where
        F: FnMut(I::Item) -> R,
    {
        let Self { mut map, iter } = self;
        LazyMapFn::new(move |i| f(map(i)), iter)
    }

    /// it uses `nth` and not `advance_by` so it's fine, idk why I was worried
    #[cfg(todo = "unnecessary")]
    pub fn skip(self, amt: usize) -> LazyMapFn<iter::Skip<I>, F> {
        self.map_iter(|i| i.skip(amt))
    }

    pub fn advance_by_lazy(&mut self, amt: usize) {
        let _ = self.iter.by_ref().take(amt).count();
    }
}
impl<I: Iterator, F, R> LazyMapFn<I, F>
where
    F: FnMut(I::Item) -> R,
{
    /// `into_strict`?
    pub fn into_std(self) -> iter::Map<I, F> {
        let Self { iter, map } = self;
        iter.map(map)
    }
}
impl<'a, T, F, R> LazyMapFn<slice::Iter<'a, T>, F>
where
    F: FnMut(&'a T) -> R,
{
    /// non-consuming peek
    #[inline(always)]
    pub unsafe fn index_unchecked(&mut self, index: usize) -> R {
        let item = self.iter.as_slice().get_unchecked(index);
        (&mut self.map)(item)
    }
}
impl<'a, T, F, R> LazyMapFn<slice::IterMut<'a, T>, F>
where
    F: FnMut(&'a mut T) -> R,
{
    #[inline(always)]
    pub unsafe fn into_index_mut_unchecked(self, index: usize) -> R {
        let Self { iter, mut map } = self;
        (map)(iter.into_slice().get_unchecked_mut(index))
    }
    /// non-consuming peek, but prefer [Self::into_index_mut_unchecked] if possible
    ///
    /// TODO: unstable..
    #[inline(always)]
    pub unsafe fn index_mut_unchecked<'b>(&'b mut self, index: usize) -> R
    where
        R: 'b,
    {
        let slice: *mut [T] = {
            let slice = mem::replace(&mut self.iter, [].iter_mut()).into_slice();
            slice as *mut [T]
        };
        self.iter = <[T]>::iter_mut(mem::transmute(slice));
        // now it's nice and pointery again maybe? this would be easier but useless if R weren't allowed to borrow...
        (&mut self.map)(&mut *(slice as *mut T).add(index))
    }
    #[cfg(todo)]
    pub unsafe fn index_mut_unchecked(&mut self, index: usize) -> R {
        let item = self.iter.as_mut_slice().get_unchecked_mut(index);
        (&mut self.map)(item)
    }
}
impl<I: Iterator, F, R> Iterator for LazyMapFn<I, F>
where
    F: FnMut(I::Item) -> R,
{
    type Item = R;

    #[inline]
    fn next(&mut self) -> Option<R> {
        self.iter.next().map(&mut self.map)
    }

    #[inline]
    fn nth(&mut self, n: usize) -> Option<R> {
        self.iter.nth(n).map(&mut self.map)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }

    #[inline]
    fn last(self) -> Option<R> {
        let Self { map, iter } = self;
        iter.last().map(map)
    }

    #[inline]
    fn count(self) -> usize {
        self.iter.count()
    }
}
impl<I: ExactSizeIterator, F> ExactSizeIterator for LazyMapFn<I, F>
where
    Self: Iterator,
{
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}
impl<I: iter::FusedIterator, F> iter::FusedIterator for LazyMapFn<I, F> where Self: Iterator {}
impl<I: DoubleEndedIterator, F, R> DoubleEndedIterator for LazyMapFn<I, F>
where
    F: FnMut(I::Item) -> R,
{
    #[inline]
    fn next_back(&mut self) -> Option<R> {
        self.iter.next_back().map(&mut self.map)
    }
    #[inline]
    fn nth_back(&mut self, n: usize) -> Option<R> {
        self.iter.nth_back(n).map(&mut self.map)
    }
}
impl<I: Iterator, F> fmt::Debug for LazyMapFn<I, F> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("iters::LazyMapFn")
            .field(&self.iter.size_hint())
            .finish()
    }
}
#[cfg(todo)]
impl<I, F> From<iter::Map<I, F>> for LazyMapFn<I, F> {}

impl<I, F, O: tree::TraversalOrder> tree::TreeTraversal<O> for LazyMapFn<I, F>
where
    I: tree::TreeTraversal<O>,
    Self: Iterator,
{
    fn node_depth(&self) -> Option<usize> {
        self.iter.node_depth()
    }
    fn node_advance_by(&mut self, amt: usize) {
        self.iter.node_advance_by(amt);
    }
}
impl<I: tree::DfsPre, F, R> tree::DfsPre for LazyMapFn<I, F>
where
    F: FnMut(I::Item) -> R,
{
    fn node_next_sibling(&mut self) -> Option<Result<Self::Item, Self::Item>> {
        match self.iter.node_next_sibling() {
            None => None,
            Some(Ok(item)) => Some(Ok((&mut self.map)(item))),
            Some(Err(item)) => Some(Err((&mut self.map)(item))),
        }
    }
}
impl<I: tree::PeekableDfsPre, F> tree::PeekableDfsPre for LazyMapFn<I, F>
where
    Self: tree::PeekableTreeTraversal<tree::PreOrder> + tree::DfsPre,
{
    fn node_skip_to_sibling(&mut self) -> Option<usize> {
        self.iter.node_skip_to_sibling()
    }
}
impl<O: tree::TraversalOrder, I: tree::PeekableTreeTraversal<O>, F, R> tree::PeekableTreeTraversal<O>
    for LazyMapFn<I, F>
where
    F: FnMut(I::Item) -> R,
    I::Item: Clone,
    R: Clone,
{
    fn peek_node(&mut self) -> Option<Cow<'_, Self::Item>> {
        self.iter
            .peek_node()
            .map(Cow::into_owned)
            .map(&mut self.map)
            .map(Cow::Owned)
    }
    fn peek_depth(&mut self) -> Option<usize> {
        self.iter.peek_depth()
    }
}
