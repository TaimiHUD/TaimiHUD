use {
    core::{cmp, mem},
    std::borrow::Cow,
};

pub trait TraversalOrder {}

pub struct PreOrder;
impl TraversalOrder for PreOrder {}

pub trait TreeTraversal<O: TraversalOrder>: Iterator {
    fn next_node(&mut self) -> Option<Self::Item> {
        Iterator::next(self)
    }
    /// of the latest produced node
    ///
    /// calling this prior to [Self::next_node] is a bad idea
    fn node_depth(&self) -> Option<usize>;

    #[cfg(todo)]
    fn node_advance_by(&mut self, amt: usize) {
        let _ = self.by_ref().take(amt).count();
    }
    fn node_advance_by(&mut self, amt: usize) {
        for _ in 0..amt {
            let _ = self.next_node();
        }
    }

    /// use [PeekableTreeExt::from_mut] directly if unsized thanks
    #[inline]
    fn peekable_ext_mut(&mut self) -> &mut PeekableTreeExt<Self>
    where
        Self: Sized + PeekableTreeTraversal<PreOrder>,
    {
        PeekableTreeExt::from_mut(self)
    }
    #[inline]
    fn peekable_ext(self) -> PeekableTreeExt<Self>
    where
        Self: Sized + PeekableTreeTraversal<PreOrder>,
    {
        PeekableTreeExt(self)
    }
}
pub trait PeekableTreeTraversal<O: TraversalOrder>: TreeTraversal<O> {
    fn peek_node(&mut self) -> Option<Cow<'_, Self::Item>>
    where
        Self::Item: Clone;
    fn peek_depth(&mut self) -> Option<usize>;

    fn peek_node_depth(&mut self) -> Option<(Cow<'_, Self::Item>, usize)>
    where
        Self::Item: Clone,
    {
        let depth = self.peek_depth()?;
        self.peek_node().map(move |node| (node, depth))
    }
}

pub trait DfsPre: TreeTraversal<PreOrder> {
    /// or up to next parent otherwise
    fn node_next_sibling(&mut self) -> Option<Result<Self::Item, Self::Item>> {
        let mut prev_depth = self.node_depth().map(Some);
        while let Some(next) = self.next_node() {
            let depth = self.node_depth();
            let prev_depth = *prev_depth.get_or_insert(depth);
            match depth.cmp(&prev_depth) {
                cmp::Ordering::Equal => return Some(Ok(next)),
                cmp::Ordering::Less => return Some(Err(next)),
                cmp::Ordering::Greater => (),
            }
        }
        None
    }
}

pub trait PeekableDfsPre: DfsPre + PeekableTreeTraversal<PreOrder> {
    /// returns amount of children consumed, if known
    fn node_skip_to_sibling(&mut self) -> Option<usize> {
        let depth = self.node_depth()?;
        let mut consumed = 0;
        while let Some(next_depth) = self.peek_depth() {
            match next_depth.cmp(&depth) {
                cmp::Ordering::Equal | cmp::Ordering::Less => break,
                cmp::Ordering::Greater => {
                    self.node_advance_by(1);
                    consumed += 1;
                },
            }
        }
        Some(consumed)
    }
    /// TODO: adapter with impls .-.
    fn node_skip_to_sibling_while<F: FnMut(Cow<Self::Item>, usize) -> bool>(
        &mut self,
        mut filter: F,
    ) -> Option<usize>
    where
        Self::Item: Clone,
    {
        let mut next = self.peek_node_depth()?;
        let mut consumed = 0;
        while filter(next.0, next.1) {
            consumed += self.node_skip_to_sibling()?;
            next = match self.peek_node_depth() {
                Some(next) => next,
                None => break,
            };
        }
        Some(consumed)
    }
}

#[derive(Debug, Copy, Clone, Default)]
#[repr(transparent)]
pub struct PeekableTreeExt<I: ?Sized>(pub I);
impl<I: ?Sized> PeekableTreeExt<I> {
    pub fn from_mut(iter: &mut I) -> &mut Self {
        unsafe { mem::transmute(iter) }
    }
}
/// TODO: use macro!
impl<I: ?Sized> Iterator for PeekableTreeExt<I>
where
    I: Iterator,
{
    type Item = I::Item;
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

/// TODO: provide macro!
impl<O: TraversalOrder, I: ?Sized> TreeTraversal<O> for PeekableTreeExt<I>
where
    I: TreeTraversal<O>,
{
    fn next_node(&mut self) -> Option<Self::Item> {
        self.0.next_node()
    }
    fn node_depth(&self) -> Option<usize> {
        self.0.node_depth()
    }
    fn node_advance_by(&mut self, amt: usize) {
        self.0.node_advance_by(amt)
    }
}
impl<I: ?Sized> DfsPre for PeekableTreeExt<I>
where
    I: DfsPre,
{
    fn node_next_sibling(&mut self) -> Option<Result<Self::Item, Self::Item>> {
        self.0.node_next_sibling()
    }
}
impl<O: TraversalOrder, I: ?Sized> PeekableTreeTraversal<O> for PeekableTreeExt<I>
where
    I: PeekableTreeTraversal<O>,
{
    #[inline]
    fn peek_node(&mut self) -> Option<Cow<'_, Self::Item>>
    where
        Self::Item: Clone,
    {
        self.0.peek_node()
    }
    #[inline]
    fn peek_depth(&mut self) -> Option<usize> {
        self.0.peek_depth()
    }
}
impl<I: ?Sized> PeekableDfsPre for PeekableTreeExt<I> where I: DfsPre + PeekableTreeTraversal<PreOrder> {}
