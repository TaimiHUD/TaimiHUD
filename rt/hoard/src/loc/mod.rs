use core::{cmp, fmt, mem};

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

    pub fn borrowed(&self) -> Locator<&N, &L> {
        let Locator { root, path } = self;
        Locator::with_parts(root, path)
    }

    /// TODO: rename to `from_path` so this can be const version? .-.
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
    pub fn rel_zip<R, P>(self, path: Locator<R, P>) -> Locator<Locator<N, R>, (L, P)> {
        let Locator { root, path } = path;
        Locator::with_parts(
            Locator::with_parts(self.root, root),
            (self.path, path),
        )
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
impl<N, L> Locator<N, L> {
    #[cfg(todo)]
    pub fn map_to<P>(self) -> Locator<N::ConvMapNs, P> where
        N: NamespaceConvMap<L, P>,
    {
        N::conv_map(self)
    }
    pub fn pivot_to<R>(self) -> Locator<R, N::NsPivotToPath> where
        N: NamespacePivotTo<R, L>,
    {
        N::loc_pivot_to(self)
    }
    pub fn pivot_to_ref<'a, R>(&'a self) -> Locator<R, <&'a N as NamespacePivotTo<R, &'a L>>::NsPivotToPath> where
        &'a N: NamespacePivotTo<R, &'a L>,
    {
        NamespacePivotTo::loc_pivot_to(self.borrowed())
    }
    #[cfg(todo)]
    pub fn pivot_as<'l, R>(&'l self) -> Locator<R, N::NsPivotToRefPath<'l>> where
        N: NamespacePivotToRef<R, L>,
    {
        N::loc_pivot_to_ref(self)
    }
    pub fn pivot_from<R>(self) -> Locator<R, R::NsPivotFromPath> where
        R: NamespacePivotFrom<N, L>,
    {
        R::loc_pivot_from(self)
    }
    pub fn to<O>(self) -> O where
        N: NamespaceConvTo<L, O>,
    {
        N::conv_to(self)
    }
    pub fn try_to<O>(self) -> Option<O> where
        N: NamespaceTryConvTo<L, O>,
    {
        N::try_conv_to(self)
    }
    pub fn try_pivot_to<O>(self) -> Option<O> where
        N: NamespaceTryConvTo<L, O>,
    {
        N::try_conv_to(self)
    }
    pub fn conv<O>(self) -> O where
        O: NamespaceConvFrom<N, L>,
    {
        O::conv_from(self)
    }
}
impl<N0, N1, L0, L1> Locator<Locator<N0, N1>, (L0, L1)> {
    pub fn zipped_root0(&self) -> Locator<&N0, &L0> {
        Locator::with_parts(&self.root.root, &self.path.0)
    }
    pub fn zipped_path1(&self) -> Locator<&N1, &L1> {
        Locator::with_parts(&self.root.path, &self.path.1)
    }
    pub fn unzip(self) -> (Locator<N0, L0>, Locator<N1, L1>) {
        let Locator { root, path: (l0, l1) } = self;
        (
            Locator::with_parts(root.root, l0),
            Locator::with_parts(root.path, l1),
        )
    }
}
impl<N0, N1, N2, L0, L1, L2> Locator<Locator<N0, Locator<N1, N2>>, (L0, (L1, L2))> {
    pub fn zipped_root1(&self) -> Locator<&N1, &L1> {
        Locator::with_parts(&self.root.path.root, &self.path.1.0)
    }
    pub fn zipped_path2(&self) -> Locator<&N2, &L2> {
        Locator::with_parts(&self.root.path.path, &self.path.1.1)
    }
    #[inline]
    pub fn unzip1(self) -> (Locator<N0, L0>, (Locator<N1, L1>, Locator<N2, L2>)) {
        let (r0, l12) = self.unzip();
        (r0, l12.unzip())
    }
}
impl<N0, N1: Namespace, L0, L1> Locator<Locator<N0, L0>, Locator<N1, L1>> {
    pub fn zip_rel(self) -> Locator<Locator<N0, N1>, (L0, L1)> {
        let Locator { root, path } = self;
        Locator::with_parts(
            Locator::with_parts(root.root, path.root),
            (root.path, path.path),
        )
    }
}
impl<N: PhantomNamespace, L> Locator<N, L> {
    /// TODO: rename to `with_path`?
    pub const fn new_path(path: L) -> Self {
        Self::with_parts(N::ZST, path)
    }

    pub const fn marker() -> Self where
        L: PhantomNamespace,
    {
        Self::with_parts(N::ZST, L::ZST)
    }

    pub const fn marker_static() -> &'static Self where
        L: PhantomNamespace,
    {
        let zst: &'static () = &();
        Self::from_path_ref_const(unsafe {
            mem::transmute(zst)
        })
    }

    #[inline]
    pub fn from_path_ref(path: &L) -> &Self {
        debug_assert!((path as *const L as *const Self).is_aligned());
        Self::from_path_ref_const(path)
    }

    /// prefer [Self::from_path_ref] where possible
    #[inline(always)]
    pub const fn from_path_ref_const(path: &L) -> &Self {
        unsafe {
            mem::transmute(path)
        }
    }
}
impl<'a, N, L> Locator<&'a N, &'a L> {
    #[inline]
    pub fn borrowed_path(self) -> &'a Locator<N, L> where
        N: PhantomNamespace,
    {
        Locator::from_path_ref(self.path)
    }
    #[inline(always)]
    pub const fn borrowed_path_const(self) -> &'a Locator<N, L> where
        N: PhantomNamespace,
    {
        Locator::from_path_ref_const(self.path)
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
#[cfg(todo = "unnecessary")]
impl<'a, N, L> Into<Locator<&'a N, &'a L>> for &'a Locator<N, L> {
    fn into(self) -> Locator<&'a N, &'a L> {
        self.borrowed()
    }
}
impl<'a, N, L> From<&'a Locator<N, L>> for Locator<&'a N, &'a L> {
    fn from(loc: &'a Locator<N, L>) -> Self {
        loc.borrowed()
    }
}
#[cfg(todo)]
impl<'a, N, L, R> NamespaceConvFrom<N, L> for Locator<R, R::NsConvFromPath> where
    R: NamespaceConvFrom<N, L>,
{}

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
impl<N, L> AsRef<Self> for Locator<N, L> {
    fn as_ref(&self) -> &Locator<N, L> {
        self
    }
}
impl<NR, N, L> AsRef<Locator<NR, N>> for Locator<Locator<NR, N>, L> {
    fn as_ref(&self) -> &Locator<NR, N> {
        &self.root
    }
}
impl<L> AsRef<L> for Locator<(), L> {
    fn as_ref(&self) -> &L {
        &self.path
    }
}
impl<'a, N: PhantomNamespace, L> From<&'a L> for &'a Locator<N, L> {
    fn from(path: &'a L) -> &'a Locator<N, L> {
        Locator::from_path_ref(path)
    }
}
#[cfg(todo)]
impl<N: PhantomNamespace, L> Borrow<L> for Locator<N, L> {
    fn borrow(&self) -> &L {
        &self.path
    }
}
impl<T: Copy + 'static, N: PhantomNamespace + Copy + 'static, L: num_traits::AsPrimitive<T>> num_traits::AsPrimitive<T> for Locator<N, L> {
    fn as_(self) -> T {
        self.path.as_()
    }
}

pub unsafe trait PhantomNamespace: Sized {
    const ZST: Self;
}
unsafe impl PhantomNamespace for () {
    const ZST: Self = ();
}
unsafe impl<N, L> PhantomNamespace for Locator<N, L> where
    N: PhantomNamespace,
    L: PhantomNamespace,
{
    const ZST: Self = Locator::with_parts(N::ZST, L::ZST);
}

pub trait Namespace: Sized {
    #[cfg(todo)]
    pub const UNSAFE_IS_ZST: bool;
}
pub trait NamespacePivotTo<R, L>: Namespace {
    type NsPivotToPath;
    fn loc_pivot_to(path: Locator<Self, L>) -> Locator<R, Self::NsPivotToPath>;
}
/// seems excessive...
#[cfg(todo)]
pub trait NamespacePivotToRef<R, L>: Namespace {
    type NsPivotToRefPath<'l> where
        Self: 'l,
        R: 'l,
        L: 'l,
        ;
    fn loc_pivot_to_ref<'l>(path: &'l Locator<Self, L>) -> Locator<R, Self::NsPivotToRefPath<'l>>;
}
pub struct RootScopeNs(core::convert::Infallible);
impl LocationParent for RootScopeNs {
    type NsParentGet = Self;
    #[inline]
    fn get_parent(self) -> Option<Self::NsParentGet> {
        match self.0 {}
    }
}
/*impl LocationParent for T {
    type NsParentGet = RootScopeNs;
    fn get_parent(self) -> Option<Self::NsParentGet> {
        None
    }
}*/
pub trait LocationLeaf<P> {
    fn ns_leaf(self) -> P;
    #[cfg(todo)]
    fn root_parent(self) -> <Self::NsParentGet as BornLocation>::NsParent where
        Self::NsParentGet: BornLocation,
    {

    }
}
pub trait LocationLeafExt {
    #[inline]
    fn leaf_of<L>(self) -> L where
        Self: Sized + LocationLeaf<L>,
    {
        LocationLeaf::ns_leaf(self)
    }
}
impl<T: Sized> LocationLeafExt for T {}
impl<N, L, P> LocationLeaf<P> for Locator<N, L> where
    N: LocationLeaf<P>,
{
    #[inline]
    fn ns_leaf(self) -> P {
        self.root.ns_leaf()
    }
}
impl<'a, N, L> LocationLeaf<L> for &'a Locator<N, L> where
    L: Copy,
{
    #[inline]
    fn ns_leaf(self) -> L {
        self.path
    }
}
pub trait LocationParent {
    type NsParentGet;
    fn get_parent(self) -> Option<Self::NsParentGet>;
    #[cfg(todo)]
    fn root_parent(self) -> <Self::NsParentGet as BornLocation>::NsParent where
        Self::NsParentGet: BornLocation,
    {

    }
}
impl<T: BornLocation> LocationParent for T {
    type NsParentGet = <T as BornLocation>::NsParent;
    fn get_parent(self) -> Option<Self::NsParentGet> {
        Some(self.ns_parent())
    }
}
pub trait BornLocation: LocationParent<NsParentGet = <Self as BornLocation>::NsParent> {
    type NsParent;
    type NsParentRoot /*: BornLocation<NsParentRoot = <<Self as BornLocation>::NsParent as BornLocation>::NsParentRoot> where*/
        //<Self as BornLocation>::NsParent: BornLocation,
        ;
    fn ns_parent(self) -> Self::NsParent;
    fn ns_root(self) -> Self::NsParentRoot;
    #[cfg(todo)]
    fn parent_ref(&self) -> &Self::NsParent;
    #[inline]
    #[cfg(todo)]
    fn next_parent(self) -> Option<Self::NsParent> where
        Self: Sized,
    {
        Some(self.parent())
    }
    #[inline]
    #[cfg(todo)]
    fn next_parent_ref(&self) -> Option<&Self::NsParent> {
        Some(self.parent_ref())
    }
}
impl<N: BornLocation, L> BornLocation for Locator<N, L> {
    type NsParent = N;
    type NsParentRoot = N::NsParentRoot;
    #[inline]
    fn ns_parent(self) -> Self::NsParent {
        self.root
    }
    #[inline]
    fn ns_root(self) -> Self::NsParentRoot {
        self.ns_parent().ns_root()
    }
    #[inline]
    #[cfg(todo)]
    fn parent_ref(&self) -> &Self::NsParent {
        &self.root
    }
}
#[cfg(todo)]
impl<N: Namespace> BornLocation for N {
    #[inline]
    fn parent(self) -> Self::NsParent {
        Some(self)
    }
}
#[cfg(todo)]
impl<N: Namespace + PhantomNamespace> BornLocation for N {
    type NsParent = Option<N>;
    #[inline]
    fn parent(self) -> Self::NsParent {
        Some(self)
    }
    #[inline]
    #[cfg(todo)]
    fn parent_ref(&self) -> &Self::NsParent {
        unsafe {
            mem::transmute(self)
        }
    }
}
#[cfg(todo)]
impl<N: BornLocation> BornLocation for Option<N> {
    type NsParent = Option<N>;
    #[inline]
    fn parent(self) -> Self::NsParent {
        None
    }
    #[inline]
    fn next_parent(self) -> Option<Self::NsParent> {
        None
    }
    #[inline]
    #[cfg(todo)]
    fn parent_ref(&self) -> &Self::NsParent {
        self
    }
    #[cfg(todo)]
    #[inline]
    fn next_parent_ref(&self) -> Option<&Self::NsParent> {
        None
    }
}
pub trait NamespacePivotFrom<R, L>: Namespace {
    type NsPivotFromPath;
    fn loc_pivot_from(path: Locator<R, L>) -> Locator<Self, Self::NsPivotFromPath>;
}
pub trait NamespaceConvTo<L, O>: Namespace {
    fn conv_to(path: Locator<Self, L>) -> O;
}
pub trait NamespaceTryConvTo<L, O>: Namespace {
    fn try_conv_to(path: Locator<Self, L>) -> Option<O>;
}
pub trait NamespaceConvFrom<N, L> {
    fn conv_from(path: Locator<N, L>) -> Self;
}
#[cfg(todo)]
pub trait NamespaceConvRef<L>: Namespace {
    type ConvOutputRef<'a> where
        Self: 'a,
        L: 'a,
    ;
    fn conv_ref<'a>(path: &'a Locator<Self, L>) -> Self::ConvOutputRef<'a> where
        Self: 'a,
    ;
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

#[macro_export]
macro_rules! locator_ns {
    (
        $(#[$meta:meta])*
        $vis:vis struct $ns:ident
            $({$($ns_struct:tt)*})?
            $($(($($ns_tuple:tt)*))?;)?
        impl LocatorNamespace {
            $(
                $index_vis:vis index $index:ident = $index_ty:ty;
                $path_vis:vis path $path:ident;
            )?
            $(fn fmt(&$fmt_this:ident, $fmt_arg:ident) {
                $($fmt_imp:tt)*
            })?
        }
        $($rest:tt)*
    ) => {
        $(
            $index_vis type $index = $index_ty;
            $path_vis type $path<N = $ns> = $crate::loc::Locator<N, $index>;
        )?
        $(impl ::core::fmt::Display for $ns {
            fn fmt(&$fmt_this, $fmt_arg: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                $($fmt_imp)*
            }
        })?
        $crate::loc::locator_ns! {
            @def (
                ($(#[$meta])*)
                $vis struct $ns {$($($ns_struct)*)?} ($($($($ns_tuple)*)?)?)
            );
            @impl(Debug) (
                ($(#[$meta])*)
                $vis struct $ns {$($($ns_struct)*)?} ($($($($ns_tuple)*)?)?)
            ) ($($fmt_this, $fmt_arg)?)
            ; $($rest)*
        }
        impl $crate::loc::BornLocation for $ns {
            type NsParent = Self;
            type NsParentRoot = Self;
            #[inline]
            fn ns_parent(self) -> Self::NsParent {
                self
            }
            #[inline]
            fn ns_root(self) -> Self::NsParentRoot {
                self
            }
        }
    };
    (@def
        (
            ($(#[$meta:meta])*)
            $ns_vis:vis struct $ns:ident {} ()
        )
        ; $($($rest:tt)+)?
    ) => {
        $(#[$meta])*
        #[derive(Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        $ns_vis struct $ns;

        impl $crate::loc::Namespace for $ns {
            #[cfg(todo)]
            const UNSAFE_IS_ZST: bool = true;
        }
        unsafe impl $crate::loc::PhantomNamespace for $ns {
            const ZST: Self = $ns;
        }
        $($crate::loc::locator_ns! { $($rest)+ })?
    };
    (@def
        (
            ($(#[$meta:meta])*)
            $ns_vis:vis struct $ns:ident {$($ns_struct:tt)+} ()
        )
        ; $($($rest:tt)+)?
    ) => {
        $(#[$meta])*
        $ns_vis struct $ns {$($ns_struct)*}
        impl $crate::loc::Namespace for $ns {
            #[cfg(todo)]
            const UNSAFE_IS_ZST: bool = false;
        }
        $($crate::loc::locator_ns! { $($rest)+ })?
    };
    (@def
        (
            ($(#[$meta:meta])*)
            $ns_vis:vis struct $ns:ident {} ($($ns_tuple:tt)+)
        )
        ; $($($rest:tt)+)?
    ) => {
        $(#[$meta])*
        $ns_vis struct $ns ($($ns_struct)*);
        unsafe impl $crate::loc::Namespace for $ns {
            const IS_ZST: bool = false;
        }
        $($crate::loc::locator_ns! { $($rest)+ })?
    };
    /*(@def
        (
            ($(#[$meta:meta])*)
            $ns_vis:vis struct $ns:ident () ()
        )
        ; $($($rest:tt)+)?
    ) => {
        $(#[$meta])*
        #[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        $ns_vis struct $ns {$($ns_struct)*}
        $($crate::loc::locator_ns! { $($rest)+ })?
    };*/
    (@impl(Debug)
        (
            $ns_meta:tt
            $ns_vis:vis struct $ns:ident $ns_struct:tt $ns_tuple:tt
        ) ($fmt_this:ident, $fmt_arg:ident)
        ; $($($rest:tt)+)?
    ) => {
        impl ::core::fmt::Debug for $ns {
            fn fmt(&$fmt_this, $fmt_arg: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                ::core::fmt::Display::fmt($fmt_this, $fmt_arg)
            }
        }
        $($crate::loc::locator_ns! { $($rest)+ })?
    };
    (@impl(Debug)
        (
            $ns_meta:tt
            $ns_vis:vis struct $ns:ident {} ()
        ) ()
        ; $($($rest:tt)+)?
    ) => {
        impl ::core::fmt::Debug for $ns {
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                f.write_str(stringify!($ns))
            }
        }
        $($crate::loc::locator_ns! { $($rest)+ })?
    };
    (@impl(Debug)
        (
            $ns_meta:tt
            $ns_vis:vis struct $ns:ident $ns_struct:tt $ns_tuple:tt
        ) ()
        ; $($($rest:tt)+)?
    ) => {
        $($crate::loc::locator_ns! { $($rest)+ })?
    };
}
pub use locator_ns;
