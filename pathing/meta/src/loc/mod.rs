use core::{borrow::Borrow, cmp, fmt, mem};

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
        $($crate::loc::locator_ns! { $($rest)+ })?
    };
    (@def
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
    };
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
