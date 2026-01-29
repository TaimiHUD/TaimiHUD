use {
    crate::{
        collections::TaimiExtend,
        iters::IterExt as _,
        loc::{LocationGet, LocationMut, LocationRef, Locator, PhantomNamespace},
    },
    core::{hash::Hash, iter, marker::PhantomData, mem, ops},
    num_traits::AsPrimitive,
};

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IndexedList<N, P, T: ?Sized> {
    pub root: N,
    pub _index: PhantomData<P>,
    pub data: T,
}

impl<N, P, T> IndexedList<N, P, T> {
    #[inline(always)]
    pub const fn with_parts(root: N, data: T) -> Self {
        Self { root, data, _index: PhantomData }
    }

    pub fn new(data: T) -> Self
    where
        N: Default,
    {
        Self::with_parts(N::default(), data)
    }
    pub fn empty(root: N) -> Self
    where
        T: Default,
    {
        Self::with_parts(root, T::default())
    }
    #[inline(always)]
    pub fn map_into<R, F: FnOnce(&N, T) -> R>(self, f: F) -> IndexedList<N, P, R> {
        let Self { root, data, .. } = self;
        let data = f(&root, data);
        IndexedList::with_parts(root, data)
    }
    #[inline(always)]
    pub fn map_data<R, F: FnOnce(T) -> R>(self, f: F) -> IndexedList<N, P, R> {
        self.map_into(move |_, data| f(data))
    }
    #[inline(always)]
    pub fn into_values(self) -> <T as IntoIterator>::IntoIter
    where
        T: IntoIterator,
    {
        IntoIterator::into_iter(self.data)
    }
}

impl<N, P, T: ?Sized> IndexedList<N, P, T> {
    #[inline]
    pub fn from_ref(data: &T) -> &IndexedList<N, P, T>
    where
        N: PhantomNamespace,
    {
        unsafe { mem::transmute(data) }
    }
    #[inline]
    pub fn from_mut(data: &mut T) -> &mut IndexedList<N, P, T>
    where
        N: PhantomNamespace,
    {
        unsafe { mem::transmute(data) }
    }
    #[inline]
    pub fn len<'a>(&'a self) -> usize
    where
        &'a T: IntoIterator,
        <&'a T as IntoIterator>::IntoIter: ExactSizeIterator,
    {
        match self.values() {
            values => values.len(),
            // specialization..? .-.
            #[cfg(todo)]
            values => values.count(),
        }
    }
    #[inline]
    pub fn is_empty<'a>(&'a self) -> bool
    where
        &'a T: IntoIterator,
    {
        let values = self.values();
        match values.size_hint() {
            #[cfg(todo)]
            (_, Some(0)) => true,
            #[cfg(debug_assertions)]
            (0, ..) => {
                let mut values = values;
                debug_assert!(values.next().is_none());
                true
            },
            (min, _max) => min == 0,
            #[cfg(todo)]
            _ => values.next().is_some(),
        }
    }

    #[inline(always)]
    pub fn values<'a>(&'a self) -> <&'a T as IntoIterator>::IntoIter
    where
        &'a T: IntoIterator,
    {
        IntoIterator::into_iter(&self.data)
    }
    #[inline(always)]
    pub fn values_mut<'a>(&'a mut self) -> <&'a mut T as IntoIterator>::IntoIter
    where
        &'a mut T: IntoIterator,
    {
        IntoIterator::into_iter(&mut self.data)
    }

    #[inline(always)]
    pub fn at<'a>(&'a self, path: P) -> Option<<&'a T as IntoIterator>::Item>
    where
        &'a T: IntoIterator,
        P: AsPrimitive<usize> + Copy + 'static,
    {
        self.nth(path.as_())
    }
    #[inline(always)]
    pub fn at_mut<'a>(&'a mut self, path: P) -> Option<<&'a mut T as IntoIterator>::Item>
    where
        &'a mut T: IntoIterator,
        P: AsPrimitive<usize> + Copy + 'static,
    {
        self.nth_mut(path.as_())
    }

    #[inline]
    pub fn nth<'a>(&'a self, index: usize) -> Option<<&'a T as IntoIterator>::Item>
    where
        &'a T: IntoIterator,
    {
        self.values().nth(index)
    }
    #[inline]
    pub fn nth_mut<'a>(&'a mut self, index: usize) -> Option<<&'a mut T as IntoIterator>::Item>
    where
        &'a mut T: IntoIterator,
    {
        self.values_mut().nth(index)
    }
    pub fn lookup_extend_with<'a, E, F: FnMut() -> E>(
        &'a mut self,
        path: P,
        default: F,
    ) -> <&'a mut T as IntoIterator>::Item
    where
        T: TaimiExtend<E>,
        &'a mut T: IntoIterator,
        P: AsPrimitive<usize> + Copy + 'static,
    {
        self.extend_to_with(path.as_(), default)
    }
    pub fn extend_to_with<'a, E, F: FnMut() -> E>(
        &'a mut self,
        index: usize,
        mut default: F,
    ) -> <&'a mut T as IntoIterator>::Item
    where
        T: TaimiExtend<E>,
        &'a mut T: IntoIterator,
    {
        let len = self.data.current_len();
        if let Some(diff) = index.checked_sub(len) {
            self.data.extend_from((0..=diff).map(|_| default()));
        }
        match self.values_mut().nth(index) {
            #[cfg(debug_assertions)]
            v => v.unwrap(),
            #[cfg(not(debug_assertions))]
            v => unsafe { v.unwrap_unchecked() },
        }
    }
    #[inline(always)]
    pub fn map_data_to<'a, R, F: FnOnce(&'a T) -> R>(&'a self, f: F) -> IndexedList<N, P, R> where
        N: Clone,
    {
        let Self { root, data, .. } = self;
        let data = f(data);
        IndexedList::with_parts(root.clone(), data)
    }
    #[inline(always)]
    pub fn map_data_to_mut<'a, R, F: FnOnce(&'a mut T) -> R>(&'a mut self, f: F) -> IndexedList<N, P, R> where
        N: Clone,
    {
        let Self { root, data, .. } = self;
        let data = f(data);
        IndexedList::with_parts(root.clone(), data)
    }
    #[inline]
    pub fn map_full(
        &self,
        _: ops::RangeFull,
    ) -> IndexedList<N, P, &<T as ops::Index<ops::RangeFull>>::Output>
    where
        T: ops::Index<ops::RangeFull>,
        N: Clone,
    {
        IndexedList::with_parts(self.root.clone(), &self.data[..])
    }
    #[inline]
    pub fn map_as_ref<U: ?Sized>(&self) -> IndexedList<N, P, &U>
    where
        T: AsRef<U>,
        N: Clone,
    {
        IndexedList::with_parts(self.root.clone(), self.data.as_ref())
    }
    #[inline]
    pub fn map_ref<U: ?Sized>(&self) -> &IndexedList<N, P, U>
    where
        T: AsRef<U>,
        N: PhantomNamespace,
    {
        IndexedList::from_ref(self.data.as_ref())
    }
    #[inline]
    pub fn map_as_slice<U>(&self) -> IndexedList<N, P, &[U]>
    where
        T: AsRef<[U]>,
        N: Clone,
    {
        self.map_as_ref()
    }
    #[inline]
    pub fn map_ref_as_slice<U>(&self) -> &IndexedList<N, P, [U]>
    where
        T: AsRef<[U]>,
        N: PhantomNamespace,
    {
        self.map_ref()
    }

    #[inline]
    pub fn map_mut<U: ?Sized>(&mut self) -> &mut IndexedList<N, P, U>
    where
        T: AsMut<U>,
        N: PhantomNamespace,
    {
        IndexedList::from_mut(self.data.as_mut())
    }
    #[inline]
    pub fn map_mut_as_slice<U>(&mut self) -> &mut IndexedList<N, P, [U]>
    where
        T: AsMut<[U]>,
        N: PhantomNamespace,
    {
        self.map_mut()
    }
}
impl<N, P, T> IndexedList<N, P, [T]> {
    #[inline]
    pub fn get_index(&self, loc: Locator<N, P>) -> Option<&T> where
        P: AsPrimitive<usize> + Copy + 'static,
        N: PartialEq,
    {
        if loc.root != self.root {
            return None
        }
        self.data.get(loc.path.as_())
    }
    #[inline]
    pub fn get_index_mut(&mut self, loc: Locator<N, P>) -> Option<&mut T> where
        P: AsPrimitive<usize> + Copy + 'static,
        N: PartialEq,
    {
        if loc.root != self.root {
            return None
        }
        self.data.get_mut(loc.path.as_())
    }
    #[inline(always)]
    pub unsafe fn index_unchecked(&self, loc: Locator<N, P>) -> &T where
        P: AsPrimitive<usize> + Copy + 'static,
    {
        self.data.get_unchecked(loc.path.as_())
    }
    #[inline(always)]
    pub unsafe fn index_mut_unchecked(&mut self, loc: Locator<N, P>) -> &mut T where
        P: AsPrimitive<usize> + Copy + 'static,
    {
        self.data.get_unchecked_mut(loc.path.as_())
    }
}
impl<N, P, T: ?Sized> IndexedList<N, P, T> {
    #[inline]
    pub fn empty_ref<'a>() -> &'a IndexedList<N, P, T>
    where
        N: PhantomNamespace,
        &'a T: Default,
    {
        Self::from_ref(Default::default())
    }
    #[inline]
    pub fn empty_mut<'a>() -> &'a mut IndexedList<N, P, T>
    where
        N: PhantomNamespace,
        &'a mut T: Default,
    {
        Self::from_mut(Default::default())
    }
}
impl<N, P, T: ?Sized> IndexedList<N, P, T>
where
    N: Clone,
    P: Copy + 'static,
    usize: AsPrimitive<P>,
{
    #[inline(always)]
    pub fn paths<'a, 'n>(&'a self) -> impl ExactSizeIterator<Item = Locator<N, P>> + DoubleEndedIterator + 'n
    where
        &'a T: IntoIterator,
        <&'a T as IntoIterator>::IntoIter: ExactSizeIterator,
        N: 'n,
    {
        let len = self.len();
        LocatorRelIter::new(
            self.root.clone(),
            EnumerateAs::<P, _>::new(iter::repeat_n((), len)),
        )
        .lazy_map(|p| p.map_path(|(p, ())| p))
    }
    #[inline(always)]
    pub fn end_path<'a>(&'a self) -> Locator<N, P>
    where
        &'a T: IntoIterator,
        <&'a T as IntoIterator>::IntoIter: ExactSizeIterator,
    {
        let len = self.len();
        Locator::with_parts(self.root.clone(), len.as_())
    }

    #[inline]
    pub fn position<'a, F: FnMut(<&'a T as IntoIterator>::Item) -> bool>(
        &'a self,
        pred: F,
    ) -> Option<Locator<N, P>>
    where
        &'a T: IntoIterator,
    {
        let root = self.root.clone();
        self.values()
            .position(pred)
            .map(|idx| Locator::with_parts(root, idx.as_()))
    }
    #[inline]
    pub fn rposition<'a, F: FnMut(<&'a T as IntoIterator>::Item) -> bool>(
        &'a self,
        pred: F,
    ) -> Option<Locator<N, P>>
    where
        &'a T: IntoIterator,
        <&'a T as IntoIterator>::IntoIter: ExactSizeIterator + DoubleEndedIterator,
    {
        let root = self.root.clone();
        self.values()
            .rposition(pred)
            .map(|idx| Locator::with_parts(root, idx.as_()))
    }

    #[inline]
    pub fn iter<'a>(&'a self) -> LocatorEnumerateAsRel<N, P, <&'a T as IntoIterator>::IntoIter>
    where
        &'a T: IntoIterator,
    {
        let data = IntoIterator::into_iter(&self.data);
        LocatorRelIter0::enumerate(self.root.clone(), data)
    }
    #[inline]
    pub fn iter_mut<'a>(&'a mut self) -> LocatorEnumerateAsRel<N, P, <&'a mut T as IntoIterator>::IntoIter>
    where
        &'a mut T: IntoIterator,
    {
        let data = IntoIterator::into_iter(&mut self.data);
        LocatorRelIter0::enumerate(self.root.clone(), data)
    }
    #[inline]
    pub fn into_iter(self) -> LocatorEnumerateAsRel<N, P, T::IntoIter>
    where
        T: Sized + IntoIterator,
    {
        let Self { root, data, _index: _ } = self;
        let data = IntoIterator::into_iter(data);
        LocatorRelIter0::enumerate(root, data)
    }
}
impl<'a, N, P, T: ?Sized> IntoIterator for &'a IndexedList<N, P, T>
where
    N: Clone,
    P: Copy + 'static,
    usize: AsPrimitive<P>,
    &'a T: IntoIterator,
{
    type Item = (Locator<N, P>, <&'a T as IntoIterator>::Item);
    type IntoIter = LocatorEnumerateAsRel<N, P, <&'a T as IntoIterator>::IntoIter>;
    #[inline(always)]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl<'a, N, P, T: ?Sized> IntoIterator for &'a mut IndexedList<N, P, T>
where
    N: Clone,
    P: Copy + 'static,
    usize: AsPrimitive<P>,
    &'a mut T: IntoIterator,
{
    type Item = (Locator<N, P>, <&'a mut T as IntoIterator>::Item);
    type IntoIter = LocatorEnumerateAsRel<N, P, <&'a mut T as IntoIterator>::IntoIter>;
    #[inline(always)]
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}
impl<N, P, T> IntoIterator for IndexedList<N, P, T>
where
    N: Clone,
    P: Copy + 'static,
    usize: AsPrimitive<P>,
    T: IntoIterator,
{
    type Item = (Locator<N, P>, <T as IntoIterator>::Item);
    type IntoIter = LocatorEnumerateAsRel<N, P, <T as IntoIterator>::IntoIter>;
    #[inline(always)]
    fn into_iter(self) -> Self::IntoIter {
        self.into_iter()
    }
}
impl<N, P, T: ?Sized, TI> LocationGet<N, P> for IndexedList<N, P, T>
where
    for<'a> &'a T: IntoIterator<Item = TI>,
    P: AsPrimitive<usize> + Copy + 'static,
    N: PartialEq,
    TI: 'static,
{
    type LookupGet = TI;
    fn lookup_get(&self, loc: &Locator<N, P>) -> Option<Self::LookupGet> {
        let Locator { root, path } = loc;
        if root != &self.root {
            return None
        }
        self.at(*path)
    }
}
impl<N, P, T: ?Sized, TI: ?Sized> LocationRef<N, P> for IndexedList<N, P, T>
where
    for<'a> &'a T: IntoIterator<Item = &'a TI>,
    P: AsPrimitive<usize> + Copy + 'static,
    N: PartialEq,
{
    type LookupRef = TI;
    fn lookup_ref(&self, loc: &'_ Locator<N, P>) -> Option<&TI> {
        let Locator { root, path } = loc;
        if root != &self.root {
            return None
        }
        self.at(*path)
    }
}
impl<N, P, T: ?Sized, TI: ?Sized> LocationMut<N, P> for IndexedList<N, P, T>
where
    for<'a> &'a mut T: IntoIterator<Item = &'a mut TI>,
    for<'a> &'a T: IntoIterator<Item = &'a TI>,
    P: AsPrimitive<usize> + Copy + 'static,
    N: PartialEq,
{
    fn lookup_mut(&mut self, loc: &'_ Locator<N, P>) -> Option<&mut TI> {
        let Locator { root, path } = loc;
        if root != &self.root {
            return None
        }
        self.at_mut(*path)
    }
}
impl<N, P, T, A> FromIterator<A> for IndexedList<N, P, T>
where
    N: Default,
    T: FromIterator<A>,
{
    fn from_iter<I: IntoIterator<Item = A>>(iter: I) -> Self {
        Self::new(T::from_iter(iter))
    }
}
impl<N, P, T, A> Extend<A> for IndexedList<N, P, T>
where
    T: Extend<A>,
{
    fn extend<I: IntoIterator<Item = A>>(&mut self, iter: I) {
        self.data.extend(iter)
    }
}
impl<N, P, T: ?Sized> ops::Deref for IndexedList<N, P, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.data
    }
}
impl<N, P, T: ?Sized> ops::DerefMut for IndexedList<N, P, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AdjacentList<P, T> {
    pub paths: P,
    pub data: T,
}

impl<P, T> AdjacentList<P, T> {
    #[inline(always)]
    pub const fn with_parts(paths: P, data: T) -> Self {
        Self { paths, data }
    }

    pub fn new(data: T) -> Self
    where
        P: Default,
    {
        Self::with_parts(P::default(), data)
    }
    pub fn empty(paths: P) -> Self
    where
        T: Default,
    {
        Self::with_parts(paths, T::default())
    }
}
impl<P, T> AdjacentList<P, T> {
    pub fn iter<'a>(
        &'a self,
    ) -> ZipPrefix<<&'a P as IntoIterator>::IntoIter, <&'a T as IntoIterator>::IntoIter>
    where
        &'a P: IntoIterator,
        &'a T: IntoIterator,
    {
        let paths = IntoIterator::into_iter(&self.paths);
        let data = IntoIterator::into_iter(&self.data);
        ZipPrefix::new(paths, data)
    }
    pub fn iter_mut<'a>(
        &'a mut self,
    ) -> ZipPrefix<<&'a P as IntoIterator>::IntoIter, <&'a mut T as IntoIterator>::IntoIter>
    where
        &'a P: IntoIterator,
        &'a mut T: IntoIterator,
    {
        let paths = IntoIterator::into_iter(&self.paths);
        let data = IntoIterator::into_iter(&mut self.data);
        ZipPrefix::new(paths, data)
    }
    pub fn into_iter(self) -> ZipPrefix<<P as IntoIterator>::IntoIter, <T as IntoIterator>::IntoIter>
    where
        P: IntoIterator,
        T: IntoIterator,
    {
        let Self { paths, data } = self;
        let paths = IntoIterator::into_iter(paths);
        let data = IntoIterator::into_iter(data);
        ZipPrefix::new(paths, data)
    }
}
impl<'a, P, T> IntoIterator for &'a AdjacentList<P, T>
where
    &'a P: IntoIterator,
    &'a T: IntoIterator,
{
    type Item = (<&'a P as IntoIterator>::Item, <&'a T as IntoIterator>::Item);
    type IntoIter = ZipPrefix<<&'a P as IntoIterator>::IntoIter, <&'a T as IntoIterator>::IntoIter>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl<'a, P, T> IntoIterator for &'a mut AdjacentList<P, T>
where
    &'a P: IntoIterator,
    &'a mut T: IntoIterator,
{
    type Item = (<&'a P as IntoIterator>::Item, <&'a mut T as IntoIterator>::Item);
    type IntoIter = ZipPrefix<<&'a P as IntoIterator>::IntoIter, <&'a mut T as IntoIterator>::IntoIter>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}
impl<P, T> IntoIterator for AdjacentList<P, T>
where
    P: IntoIterator,
    T: IntoIterator,
{
    type Item = (P::Item, T::Item);
    type IntoIter = ZipPrefix<P::IntoIter, T::IntoIter>;
    fn into_iter(self) -> Self::IntoIter {
        self.into_iter()
    }
}
impl<P, T, PN, PI, TI> LocationGet<PN, PI> for AdjacentList<P, T>
where
    for<'a> &'a P: IntoIterator<Item = Locator<PN, PI>>,
    for<'a> &'a T: IntoIterator<Item = TI>,
    Locator<PN, PI>: PartialEq,
    TI: 'static,
{
    type LookupGet = TI;
    fn lookup_get(&self, loc: &Locator<PN, PI>) -> Option<Self::LookupGet> {
        self.iter().find(|(p, _i)| p == loc).map(|(_p, i)| i)
    }
}
impl<P, T, PN, PI, TI> LocationRef<PN, PI> for AdjacentList<P, T>
where
    for<'a> &'a P: IntoIterator<Item = Locator<PN, PI>>,
    for<'a> &'a T: IntoIterator<Item = &'a TI>,
    Locator<PN, PI>: PartialEq,
{
    type LookupRef = TI;
    fn lookup_ref(&self, loc: &'_ Locator<PN, PI>) -> Option<&Self::LookupRef> {
        self.iter().find(|(p, _i)| p == loc).map(|(_p, i)| i)
    }
}
impl<P, T, PN, PI> LocationMut<PN, PI> for AdjacentList<P, T>
where
    for<'a> &'a P: IntoIterator<Item = Locator<PN, PI>>,
    for<'a> &'a mut T: IntoIterator<Item = &'a mut <Self as LocationRef<PN, PI>>::LookupRef>,
    Self: LocationRef<PN, PI>,
    Locator<PN, PI>: PartialEq,
{
    fn lookup_mut(&mut self, loc: &'_ Locator<PN, PI>) -> Option<&mut Self::LookupRef> {
        self.iter_mut().find(|(p, _i)| p == loc).map(|(_p, i)| i)
    }
}

pub type LocatorEnumerateAsRel<N, P, I> = LocatorRelIter0<N, EnumerateAs<P, I>>;

#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocatorRelIter<N, I> {
    pub root: N,
    pub iter: I,
}
impl<N, I> LocatorRelIter<N, I> {
    #[inline(always)]
    pub const fn new(root: N, iter: I) -> Self {
        Self { root, iter }
    }
}
impl<N, I> LocatorRelIter<N, I>
where
    N: Clone,
    I: Iterator,
{
    #[inline(always)]
    pub fn map_item(&self, item: I::Item) -> Locator<N, I::Item> {
        Locator::with_parts(self.root.clone(), item)
    }
}
impl_iter_wrap! {
    impl{N: Clone, I} Iterator for LocatorRelIter<N, I>, I
        where{}
    {
        type Item = Locator<N, I::Item>;
        let iter = |this| this.iter;
        let iter = |&this| &this.iter;
        let iter = |&mut this| &mut this.iter;
        let item = |&mut this, item| this.map_item(item);

        fn last(self) -> Option<Self::Item> {
            let Self { root, iter } = self;
            iter.last().map(|i| Locator::with_parts(root, i))
        }
        #[cfg(todo)]
        fn is_sorted(self) -> bool where
            Self::Item: PartialOrd,
        {
            self.iter.is_sorted()
        }
    }
    impl{N: Clone, I} DoubleEndedIterator for LocatorRelIter<N, I>, I
        where{}
    {
        let iter = |&mut this| &mut this.iter;
        let item = |&mut this, item| this.map_item(item);
    }
    impl{N: Clone, I} ExactSizeIterator for LocatorRelIter<N, I>, I
        where{}
    {
        let iter = |&this| &this.iter;
    }
    impl{N, I} &IntoIterator<Clone> for LocatorRelIter<N, I> {}
}
#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocatorRelIter0<N, I> {
    pub iter: LocatorRelIter<N, I>,
}
impl<N, I> LocatorRelIter0<N, I> {
    #[inline(always)]
    pub const fn new(root: N, iter: I) -> Self {
        Self { iter: LocatorRelIter::new(root, iter) }
    }
}

impl<N: Clone, I, P, II> LocatorRelIter0<N, I>
where
    I: Iterator<Item = (P, II)>,
{
    #[inline(always)]
    pub fn map_item(item: <LocatorRelIter<N, I> as Iterator>::Item) -> (Locator<N, P>, II) {
        let Locator { root, path: (path, item) } = item;
        (Locator::with_parts(root, path), item)
    }
}
impl<N, P, I: Iterator> LocatorRelIter0<N, EnumerateAs<P, I>> {
    #[inline(always)]
    pub fn enumerate(root: N, iter: I) -> Self {
        Self::new(root, EnumerateAs::new(iter))
    }
}
impl_iter_wrap! {
    impl{N: Clone, I, P, II} Iterator for LocatorRelIter0<N, I>, I
        where{
            I: Iterator<Item = (P, II)>,
        }
    {
        type Item = (Locator<N, P>, II);
        let iter = |this| this.iter.iter;
        let iter = |&this| &this.iter.iter;
        let iter = |&mut this| &mut this.iter;
        let item = |&mut _this, item| Self::map_item(item);

        fn last(self) -> Option<Self::Item> {
            self.iter.last().map(Self::map_item)
        }
        #[cfg(todo)]
        fn is_sorted(self) -> bool where
            Self::Item: PartialOrd,
        {
            self.iter.is_sorted()
        }
    }
    impl{N: Clone, I, P, II} DoubleEndedIterator for LocatorRelIter0<N, I>, I
        where{
            I: Iterator<Item = (P, II)>,
        }
    {
        let iter = |&mut this| &mut this.iter;
        let item = |&mut _this, item| Self::map_item(item);
    }
    impl{N: Clone, I} ExactSizeIterator for LocatorRelIter0<N, I>, I
        where{
            Self: Iterator,
        }
    {
        let iter = |&this| &this.iter.iter;
    }
    impl{N, I} &IntoIterator<Clone> for LocatorRelIter0<N, I> {}
}
#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct LocatorPathAs<P, I> {
    pub iter: I,
    pub _index: PhantomData<P>,
}
impl<P, I> LocatorPathAs<P, I> {
    #[inline(always)]
    pub const fn new(iter: I) -> Self {
        Self { iter, _index: PhantomData }
    }
}
impl<P, IN, IP> LocatorPathAs<P, Locator<IN, IP>>
where
    P: Copy + 'static,
    IP: AsPrimitive<P>,
{
    pub fn path_as(self) -> Locator<IN, P> {
        self.iter.map_path(AsPrimitive::as_)
    }
}
impl_iter_wrap! {
    impl{IN, IP, P, I} Iterator for LocatorPathAs<P, I>, I
        where{
            P: Copy + 'static,
            IP: AsPrimitive<P>,
            I: Iterator<Item = Locator<IN, IP>>,
        }
    {
        type Item = Locator<IN, P>;
        let iter = |this| this.iter;
        let iter = |&this| &this.iter;
        let iter = |&mut this| &mut this.iter;
        let item = |&mut _this, item| LocatorPathAs::<P, _>::new(item).path_as();

        fn last(self) -> Option<Self::Item> {
            self.iter.last().map(|i| LocatorPathAs::<P, _>::new(i).path_as())
        }
        #[cfg(todo)]
        fn is_sorted(self) -> bool where
            Self::Item: PartialOrd,
        {
            self.iter.is_sorted()
        }
    }
    impl{IN, IP, P, I} DoubleEndedIterator for LocatorPathAs<P, I>, I
        where{
            P: Copy + 'static,
            IP: AsPrimitive<P>,
            I: Iterator<Item = Locator<IN, IP>>,
        }
    {
        let iter = |&mut this| &mut this.iter;
        let item = |&mut _this, item| LocatorPathAs::<P, _>::new(item).path_as();
    }
    impl{P, I} ExactSizeIterator for LocatorPathAs<P, I>, I
        where{
            Self: Iterator,
        }
    {
        let iter = |&this| &this.iter;
    }
    impl{P, I} &IntoIterator<Clone> for LocatorPathAs<P, I> {}
}

#[derive(Debug, Default, Clone)]
#[repr(transparent)]
pub struct EnumerateAs<P, I> {
    pub iter: iter::Enumerate<I>,
    pub _index: PhantomData<P>,
}
impl<P, I> EnumerateAs<P, I> {
    #[inline(always)]
    pub fn new(iter: I) -> Self
    where
        I: Iterator,
    {
        Self {
            iter: iter.enumerate(),
            _index: PhantomData,
        }
    }
}
impl<P, I, E> EnumerateAs<P, I>
where
    I: Iterator,
    iter::Enumerate<I>: Iterator<Item = (E, I::Item)>,
    P: Copy + 'static,
    E: AsPrimitive<P>,
{
    #[inline(always)]
    pub fn map_item(item: <iter::Enumerate<I> as Iterator>::Item) -> (P, I::Item) {
        let (e, i) = item;
        (AsPrimitive::as_(e), i)
    }
}
impl_iter_wrap! {
    impl{P, I, E} Iterator for EnumerateAs<P, I>, iter::Enumerate<I>
        where{
            I: Iterator,
            iter::Enumerate<I>: Iterator<Item = (E, I::Item)>,
            P: Copy + 'static,
            E: AsPrimitive<P>,
        }
    {
        type Item = (P, I::Item);
        let iter = |this| this.iter;
        let iter = |&this| &this.iter;
        let iter = |&mut this| &mut this.iter;
        let item = |&mut _this, item| Self::map_item(item);

        fn last(self) -> Option<Self::Item> {
            self.iter.last().map(Self::map_item)
        }
        #[cfg(todo)]
        fn is_sorted(self) -> bool where
            Self::Item: PartialOrd,
        {
            self.iter.is_sorted()
        }
    }
    impl{P, I, E} DoubleEndedIterator for EnumerateAs<P, I>, iter::Enumerate<I>
        where{
            I: Iterator,
            iter::Enumerate<I>: Iterator<Item = (E, I::Item)>,
            P: Copy + 'static,
            E: AsPrimitive<P>,
        }
    {
        let iter = |&mut this| &mut this.iter;
        let item = |&mut _this, item| Self::map_item(item);
    }
    impl{P, I} ExactSizeIterator for EnumerateAs<P, I>, iter::Enumerate<I>
        where{
            I: Iterator,
            Self: Iterator,
        }
    {
        let iter = |&this| &this.iter;
    }
    impl{P, I} &IntoIterator<Clone> for EnumerateAs<P, I> {}
}

#[derive(Debug, Default, Clone)]
pub struct ZipPrefix<P, I> {
    pub prefix: P,
    pub iter: I,
}
impl<P, I> ZipPrefix<P, I> {
    #[inline(always)]
    pub const fn new(prefix: P, iter: I) -> Self {
        Self { prefix, iter }
    }
}
impl<P, I> ZipPrefix<P, I>
where
    P: Iterator,
    I: Iterator,
{
    pub fn map_item(item: I::Item, prefix: Option<P::Item>) -> Option<(P::Item, I::Item)> {
        let prefix = match prefix {
            #[cfg(debug_assertions)]
            None => panic!("zip expected prefix"),
            prefix => prefix?,
        };
        Some((prefix, item))
    }
}
impl_iter_wrap! {
    impl{P, I} Iterator for ZipPrefix<P, I>, I
        where{
            I: Iterator,
            P: Iterator,
        }
    {
        type Item = (P::Item, I::Item);
        let iter = |this| this.iter;
        let iter = |&this| &this.iter;
        fn next(&mut self) -> Option<Self::Item> {
            let item = self.iter.next()?;
            Self::map_item(item, self.prefix.next())
        }
        fn nth(&mut self, i: usize) -> Option<Self::Item> {
            let item = self.iter.nth(i)?;
            Self::map_item(item, self.prefix.nth(i))
        }
        fn last(self) -> Option<Self::Item> {
            let Self { iter, mut prefix } = self;
            let (last, n) = match iter.size_hint() {
                (_, Some(left)) => (iter.last(), left),
                (min, None) => {
                    let mut n = min.saturating_sub(1);
                    let last = iter.skip(min).inspect(|_| n += 1).last();
                    (last, n)
                },
            };
            last.and_then(move |i| Self::map_item(i, prefix.nth(n - 1)))
        }
        /// just ignore the prefix
        #[cfg(todo = "unnecessary")]
        fn is_sorted(self) -> bool where
            Self::Item: PartialOrd,
        {
            self.iter.is_sorted()
        }
    }
    impl{P, I} DoubleEndedIterator for ZipPrefix<P, I>, I
        where{
            I: DoubleEndedIterator,
            P: DoubleEndedIterator,
        }
    {
        fn next_back(&mut self) -> Option<<Self as Iterator>::Item> {
            let item = self.iter.next_back()?;
            Self::map_item(item, self.prefix.next_back())
        }
        fn nth_back(&mut self, n: usize) -> Option<<Self as Iterator>::Item> {
            let item = self.iter.nth_back(n)?;
            Self::map_item(item, self.prefix.nth_back(n))
        }
    }
    impl{P, I} ExactSizeIterator for ZipPrefix<P, I>, I
        where{
            P: Iterator,
            I: Iterator,
            Self: Iterator,
        }
    {
        let iter = |&this| &this.iter;
    }
    impl{P, I} &IntoIterator<Clone> for ZipPrefix<P, I> {}
}

#[derive(Debug, Default, Clone)]
pub struct ClonedRef<I> {
    pub iter: I,
}
impl<I> ClonedRef<I> {
    #[inline(always)]
    pub const fn new(iter: I) -> Self {
        Self { iter }
    }
}
impl<I: IntoIterator> IntoIterator for ClonedRef<I> {
    type Item = I::Item;
    type IntoIter = I::IntoIter;
    fn into_iter(self) -> Self::IntoIter {
        self.iter.into_iter()
    }
}
impl<'a, I, II> IntoIterator for &'a ClonedRef<I>
where
    &'a I: IntoIterator<Item = &'a II>,
    II: Clone + 'a,
{
    type Item = II;
    type IntoIter = iter::Cloned<<&'a I as IntoIterator>::IntoIter>;
    fn into_iter(self) -> Self::IntoIter {
        IntoIterator::into_iter(&self.iter).cloned()
    }
}
#[cfg(todo)]
impl<'a, I> IntoIterator for &'a mut ClonedRef<I> {}
impl<I, N, L> LocationGet<N, L> for ClonedRef<I>
where
    I: LocationRef<N, L>,
    I::LookupRef: Clone,
{
    type LookupGet = I::LookupRef;
    fn lookup_get(&self, loc: &Locator<N, L>) -> Option<Self::LookupGet> {
        self.iter.lookup_ref(loc).cloned()
    }
}
impl_iter_wrap! {
    impl{I} LocationRef<N, L> for ClonedRef<I>, I
        where{
        }
    {
        let inner = |&this| &this.iter;
        let inner = |&mut this| &mut this.iter;
    }
}

#[macro_export]
macro_rules! impl_iter_wrap_loc {
    (
        impl{$($imp:tt)*} LocationGet<$n:ident, $l:ident> for $ty:ty, $inner:ty
            $(where{$($where:tt)*})?
        {
            $(
                let inner = |&$this_inner_ref:ident| $inner_ref:expr;
            )?
            $(fn $($inner_get:tt)+)?
        }
        $($($rest:tt)+)?
    ) => {
        impl<$n, $l, $($imp)*> $crate::loc::LocationGet<$n, $l> for $ty where
            $inner: $crate::loc::LocationGet<$n, $l>,
            $($($where)*)?
        {
            $(
                type LookupGet = <$inner as $crate::loc::LocationGet<$n, $l>>::LookupGet;
                fn lookup_get(&self, loc: &'_ $crate::loc::Locator<$n, $l>) -> Option<Self::LookupGet> {
                    let $this_inner_ref = self;
                    $crate::loc::LocationGet::lookup_get($inner_ref, loc)
                }
            )?
            $(fn $($inner_get)+)?
        }
        $(impl_iter_wrap_loc!{$($rest)*})?
    };
    (
        impl{$($imp:tt)*} LocationRef<$n:ident, $l:ident> for $ty:ty, $inner:ty
            $(where{$($where:tt)*})?
        {
            let inner = |&$this_inner_ref:ident| $inner_ref:expr;
            let inner = |&mut $this_inner_mut:ident| $inner_mut:expr;
        }
        $($($rest:tt)+)?
    ) => {
        impl<$n, $l, $($imp)*> $crate::loc::LocationRef<$n, $l> for $ty where
            $inner: $crate::loc::LocationRef<$n, $l>,
            $($($where)*)?
        {
            type LookupRef = <$inner as $crate::loc::LocationRef<$n, $l>>::LookupRef;
            fn lookup_ref(&self, loc: &'_ $crate::loc::Locator<$n, $l>) -> Option<&Self::LookupRef> {
                let $this_inner_ref = self;
                $crate::loc::LocationRef::lookup_ref($inner_ref, loc)
            }
        }
        impl<$n, $l, $($imp)*> $crate::loc::LocationMut<$n, $l> for $ty where
            $inner: $crate::loc::LocationMut<$n, $l>,
            $($($where)*)?
        {
            fn lookup_mut(&mut self, loc: &'_ $crate::loc::Locator<$n, $l>) -> Option<&mut Self::LookupRef> {
                let $this_inner_mut = self;
                $crate::loc::LocationMut::lookup_mut($inner_mut, loc)
            }
        }
        $(impl_iter_wrap_loc!{$($rest)*})?
    };
    ($($rest:tt)*) => {
        $crate::iters::impl_iter_wrap!{$($rest)*}
    };
}
pub use impl_iter_wrap_loc;

use self::impl_iter_wrap_loc as impl_iter_wrap;
