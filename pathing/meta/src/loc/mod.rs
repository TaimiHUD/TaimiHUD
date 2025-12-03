use core::{cmp, fmt};

pub mod packs;
pub mod indexed;

/// Generic resource reference
#[derive(Debug, Copy, Clone, Default, Hash)]
pub struct Locator<N, L> {
    /// Root namespace
    pub root: N,
    /// Location
    pub path: L,
}

impl<N, L> Locator<N, L> {
    #[inline]
    pub const fn with_parts(root: N, path: L) -> Self {
        Self { root, path }
    }

    #[inline]
    pub fn new<R: Into<N>, P: Into<L>>(root: R, path: P) -> Self {
        Self::with_parts(root.into(), path.into())
    }

    pub fn into_tuple(self) -> (N, L) {
        let Self { root, path } = self;
        (root, path)
    }
    pub fn into_path(self) -> L {
        self.path
    }

    pub fn with_path<P: Into<L>>(path: P) -> Self where
        N: Default,
    {
        Self::with_parts(N::default(), path.into())
    }

    pub fn and_path<P: Into<L>>(self, path: P) -> Self {
        Self::with_parts(self.root, path.into())
    }

    pub fn map_path_with<P, F: FnOnce(&N, L) -> P>(self, f: F) -> Locator<N, P> {
        let Self { root, path } = self;
        let path = f(&root, path);
        Locator::with_parts(root, path)
    }
    pub fn map_path<P, F: FnOnce(L) -> P>(self, f: F) -> Locator<N, P> {
        self.map_path_with(move |_, p| f(p))
    }

    pub fn map_root_with<R, F: FnOnce(N, &L) -> R>(self, f: F) -> Locator<R, L> {
        let Self { root, path } = self;
        let root = f(root, &path);
        Locator::with_parts(root, path)
    }
    pub fn map_root<R, F: FnOnce(N) -> R>(self, f: F) -> Locator<R, L> {
        self.map_root_with(move |r, _| f(r))
    }

    pub fn pivot<R>(self, new_root: R) -> Locator<R, L> {
        Locator::with_parts(new_root, self.path)
    }
    pub fn unscope<R: Default>(self) -> Locator<R, L> {
        Locator::with_path(self.path)
    }
    pub fn swap<P>(self, new_path: P) -> Locator<N, P> {
        Locator::with_parts(self.root, new_path)
    }
    pub fn rel<P>(self, path: P) -> Locator<Self, P> {
        Locator::with_parts(self, path)
    }

    #[inline]
    pub fn lookup_get<R: ?Sized + LocationGet<N, L>>(&self, repo: &R) -> Option<R::LookupGet> {
        repo.lookup_get(self)
    }
    #[inline]
    pub fn lookup_ref<'a, R: ?Sized + LocationRef<N, L>>(&self, repo: &'a R) -> Option<&'a R::LookupRef> {
        repo.lookup_ref(self)
    }
    #[inline]
    pub fn lookup_mut<'a, R: ?Sized + LocationMut<N, L>>(&self, repo: &'a mut R) -> Option<&'a mut R::LookupRef> {
        repo.lookup_mut(self)
    }
}

impl<N, L> From<(N, L)> for Locator<N, L> {
    fn from((root, path): (N, L)) -> Self {
        Self::with_parts(root, path)
    }
}

impl<N, L> From<Locator<N, L>> for (N, L) {
    fn from(loc: Locator<N, L>) -> Self {
        loc.into_tuple()
    }
}

impl<N, L> fmt::Display for Locator<N, L> where
    N: fmt::Display,
    L: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let Self { root, path } = self;
        write!(f, "{root}/{path}")
    }
}
impl<N: Eq, L: Eq> Eq for Locator<N, L> {}
impl<N: Ord, L: Ord> Ord for Locator<N, L> {
    fn cmp(&self, rhs: &Self) -> cmp::Ordering {
        self.root.cmp(&rhs.root)
            .then_with(|| self.path.cmp(&rhs.path))
    }
}
impl<N, L, RN, RL> PartialEq<Locator<RN, RL>> for Locator<N, L> where
    N: PartialEq<RN>,
    L: PartialEq<RL>,
{
    fn eq(&self, rhs: &Locator<RN, RL>) -> bool {
        self.root == rhs.root && self.path == rhs.path
    }
}
impl<N, L, RN, RL> PartialOrd<Locator<RN, RL>> for Locator<N, L> where
    N: PartialOrd<RN>,
    L: PartialOrd<RL>,
{
    fn partial_cmp(&self, rhs: &Locator<RN, RL>) -> Option<cmp::Ordering> {
        match self.root.partial_cmp(&rhs.root) {
            Some(cmp::Ordering::Equal) =>
                self.path.partial_cmp(&rhs.path),
            ord => ord,
        }
    }
}
impl<N, L, RN, RL> PartialEq<Locator<RN, RL>> for &'_ Locator<N, L> where
    Locator<N, L>: PartialEq<Locator<RN, RL>>,
{
    fn eq(&self, rhs: &Locator<RN, RL>) -> bool {
        *self == rhs
    }
}
impl<N, L, RN, RL> PartialOrd<Locator<RN, RL>> for &'_ Locator<N, L> where
    N: PartialOrd<RN>,
    L: PartialOrd<RL>,
{
    fn partial_cmp(&self, rhs: &Locator<RN, RL>) -> Option<cmp::Ordering> {
        PartialOrd::partial_cmp(*self, rhs)
    }
}
impl<N, L, RN, RL> PartialEq<&'_ Locator<RN, RL>> for Locator<N, L> where
    Locator<N, L>: PartialEq<Locator<RN, RL>>,
{
    fn eq(&self, rhs: &&Locator<RN, RL>) -> bool {
        self == *rhs
    }
}
impl<N, L, RN, RL> PartialOrd<&'_ Locator<RN, RL>> for Locator<N, L> where
    N: PartialOrd<RN>,
    L: PartialOrd<RL>,
{
    fn partial_cmp(&self, rhs: &&Locator<RN, RL>) -> Option<cmp::Ordering> {
        PartialOrd::partial_cmp(self, *rhs)
    }
}

pub trait LocationGet<N, L> {
    type LookupGet: Sized;

    fn lookup_get(&self, loc: &Locator<N, L>) -> Option<Self::LookupGet>;
}

pub trait LocationRef<N, L> {
    type LookupRef;

    fn lookup_ref<'a>(&'a self, loc: &Locator<N, L>) -> Option<&'a Self::LookupRef>;
}

pub trait LocationMut<N, L>: LocationRef<N, L> {
    fn lookup_mut<'a>(&'a mut self, loc: &Locator<N, L>) -> Option<&'a mut Self::LookupRef>;
}
