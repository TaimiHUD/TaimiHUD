use {
    core::iter::{self, FusedIterator},
    num_traits::{AsPrimitive, ConstZero},
};

pub trait AsPrimitiveFrom<T>: Sized {
    fn as_from(prim: T) -> Self;
}
impl<T, U> AsPrimitiveFrom<T> for U
where
    U: Copy + 'static,
    T: AsPrimitive<Self>,
{
    #[inline(always)]
    fn as_from(prim: T) -> Self {
        prim.as_()
    }
}
pub trait EnumRepr: Sized {
    type Repr: Sized + Copy + Ord + AsPrimitive<usize> + AsPrimitiveFrom<usize> + ConstZero + 'static;

    const ENUM_MIN: Self;
    const ENUM_MAX: Self;
    const REPR_MIN: Self::Repr;
    const REPR_MAX: Self::Repr;
    /// TODO: not guaranteed `>Self::REPR_MIN`, so `Self::REPR_MIN..Self::REPR_END` may or may not be valid?
    const REPR_END: Self::Repr;
    const REPR_COUNT: Self::Repr;
    const REPR_INDEX_MAX: Self::Repr;

    fn to_repr(self) -> Self::Repr;
    fn to_repr_index(self) -> Self::Repr;

    unsafe fn from_repr_unchecked(repr: Self::Repr) -> Self;
    fn from_repr(repr: Self::Repr) -> Option<Self> {
        match repr {
            r if r < Self::REPR_MIN || r > Self::REPR_MAX || r == Self::REPR_END => None,
            repr => Some(unsafe { Self::from_repr_unchecked(repr) }),
        }
    }

    unsafe fn from_repr_index_unchecked(index: Self::Repr) -> Self;
    fn from_repr_index(index: Self::Repr) -> Option<Self> {
        match index {
            r if r < <Self::Repr as ConstZero>::ZERO || r >= Self::REPR_COUNT => None,
            #[cfg(todo = "unnecessary")]
            r if r > Self::REPR_INDEX_MAX => None,
            index => Some(unsafe { Self::from_repr_index_unchecked(index) }),
        }
    }

    #[inline(always)]
    fn repr_iter() -> EnumReprIter<Self> {
        EnumReprIter::NEW
    }
}
pub unsafe trait EnumReprCountEq<const REPR_COUNT: usize>: EnumRepr {}
pub trait EnumReprIndex<R: EnumRepr> {
    type ReprIndexed: ?Sized;
    fn get_at_repr(&self, index: R) -> Option<&Self::ReprIndexed>;
    fn get_at_repr_mut(&mut self, index: R) -> Option<&mut Self::ReprIndexed>;
}
pub unsafe trait EnumReprCollection<R: EnumRepr>: EnumReprIndex<R> {
    fn at_repr(&self, index: R) -> &Self::ReprIndexed {
        unsafe { self.get_at_repr(index).unwrap_unchecked() }
    }
    fn at_repr_mut(&mut self, index: R) -> &mut Self::ReprIndexed {
        unsafe { self.get_at_repr_mut(index).unwrap_unchecked() }
    }
}
impl<T, R: EnumRepr, const N: usize> EnumReprIndex<R> for [T; N]
where
// TODO: specialize around R: EnumReprCountEq<N>?
{
    type ReprIndexed = T;
    #[inline]
    fn get_at_repr(&self, index: R) -> Option<&T> {
        self.get(index.to_repr_index().as_())
    }
    #[inline]
    fn get_at_repr_mut(&mut self, index: R) -> Option<&mut T> {
        self.get_mut(index.to_repr_index().as_())
    }
}
impl<T, R> EnumReprIndex<R> for [T]
where
    R: EnumRepr,
{
    type ReprIndexed = T;
    #[inline]
    fn get_at_repr(&self, index: R) -> Option<&T> {
        self.get(index.to_repr_index().as_())
    }
    #[inline]
    fn get_at_repr_mut(&mut self, index: R) -> Option<&mut T> {
        self.get_mut(index.to_repr_index().as_())
    }
}
unsafe impl<T, R, const N: usize> EnumReprCollection<R> for [T; N]
where
    R: EnumReprCountEq<N> + EnumRepr,
    //R: EnumRepr<REPR_COUNT = N>,
{
    #[inline]
    fn at_repr(&self, index: R) -> &T {
        unsafe { self.get_unchecked(index.to_repr_index().as_()) }
    }
    #[inline]
    fn at_repr_mut(&mut self, index: R) -> &mut T {
        unsafe { self.get_unchecked_mut(index.to_repr_index().as_()) }
    }
}

pub trait EnumReprArrayOf<T>: EnumRepr {
    type EnumArray: Sized + EnumReprCollection<Self, ReprIndexed = T> + IntoIterator<Item = T>;

    fn enum_into_iter<'a>(
        array: Self::EnumArray,
    ) -> EnumEnumerate<Self, <Self::EnumArray as IntoIterator>::IntoIter>
    where
        Self::EnumArray: IntoIterator,
    {
        Self::repr_iter().zip(IntoIterator::into_iter(array))
    }
    #[inline(always)]
    fn enum_iter<'a>(
        array: &'a Self::EnumArray,
    ) -> EnumEnumerate<Self, <&'a Self::EnumArray as IntoIterator>::IntoIter>
    where
        &'a Self::EnumArray: IntoIterator,
    {
        Self::repr_iter().zip(IntoIterator::into_iter(array))
    }
    #[inline(always)]
    fn enum_iter_mut<'a>(
        array: &'a mut Self::EnumArray,
    ) -> EnumEnumerate<Self, <&'a mut Self::EnumArray as IntoIterator>::IntoIter>
    where
        &'a mut Self::EnumArray: IntoIterator,
    {
        Self::repr_iter().zip(IntoIterator::into_iter(array))
    }
}

pub type EnumEnumerate<R, I> = iter::Zip<EnumReprIter<R>, I>;
pub struct EnumReprIter<R: EnumRepr> {
    /// TODO: un-pub to avoid need for saturating ops?
    pub repr: R::Repr,
}
impl<R: EnumRepr> EnumReprIter<R> {
    pub const NEW: Self = Self::starting_at_repr(R::REPR_MIN);
    pub const EMPTY: Self = Self { repr: R::REPR_END };
    #[inline(always)]
    pub const fn new() -> Self {
        Self::NEW
    }
    #[inline(always)]
    pub const fn starting_at_repr(repr: R::Repr) -> Self {
        Self { repr }
    }
    #[inline(always)]
    pub fn starting_at(repr: R) -> Self {
        Self::starting_at_repr(repr.to_repr())
    }
}
impl<R: EnumRepr> Iterator for EnumReprIter<R> {
    type Item = R;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        debug_assert!(R::REPR_MIN != R::REPR_END);

        let next = R::from_repr(self.repr);
        if next.is_some() {
            self.repr = AsPrimitiveFrom::as_from(AsPrimitive::<usize>::as_(self.repr) + 1);
        }
        next
    }
    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
    /// XXX: saturating_add or clamp etc for correctness
    #[inline]
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.repr = AsPrimitiveFrom::as_from(AsPrimitive::<usize>::as_(self.repr) + n);
        self.next()
    }
}
impl<R: EnumRepr> ExactSizeIterator for EnumReprIter<R> {
    #[inline]
    fn len(&self) -> usize {
        R::REPR_END.as_().saturating_sub(self.repr.as_())
    }
}
impl<R: EnumRepr> FusedIterator for EnumReprIter<R> {}

#[macro_export]
macro_rules! EnumRepr {
    (
        $(#[$($meta:tt)*])*
        $vis:vis enum $id:ident {
            $($inner:tt)*
        }
    ) => {
        //derive(::strum::EnumCount) ?
        #[derive(::bytemuck::Contiguous)]
        #[::macro_rules_attribute::apply($crate::repr::EnumReprFromContiguous!)]
        $(#[$($meta)*])*
        $vis enum $id {
            $($inner)*
        }
    };
}
#[macro_export]
macro_rules! EnumReprFromContiguous {
    (
        $(#[$($meta:tt)*])*
        $vis:vis enum $id:ty {
            $($inner:tt)*
        }
    ) => {
        $crate::repr::EnumReprFromContiguous! {
            @impl(EnumRepr)
            @meta()
            $(#[$($meta)*])*
            $vis enum $id {
                $($inner)*
            }
        }
    };
    (@impl(EnumRepr)
        @meta($($meta:tt)*)
        #[repr($repr:ty)]
        $vis:vis enum $id:ty {
            $($inner:tt)*
        }
    ) => {
        unsafe impl $crate::repr::EnumReprCountEq<{<$id as $crate::repr::EnumRepr>::REPR_COUNT as usize}> for $id {}
        impl<T> $crate::repr::EnumReprArrayOf<T> for $id where
            [T; {<$id as $crate::repr::EnumRepr>::REPR_COUNT as usize}]: $crate::repr::EnumReprCollection<Self, ReprIndexed = T>,
        {
            type EnumArray = [T; {<$id as $crate::repr::EnumRepr>::REPR_COUNT as usize}];
        }
        impl $crate::repr::EnumRepr for $id {
            type Repr = $repr;

            const REPR_MIN: Self::Repr = <Self as ::bytemuck::Contiguous>::MIN_VALUE;
            const REPR_MAX: Self::Repr = <Self as ::bytemuck::Contiguous>::MAX_VALUE;
            const REPR_END: Self::Repr = match Self::REPR_MAX {
                // TODO? if REPR_MIN != 0 should consider wrapping but *be careful* about using exclusive matches/ranges in that case!
                <Self::Repr>::MAX =>
                    panic!("enum using entire repr storage ohno"),
                max => max + 1,
            };
            const REPR_COUNT: Self::Repr = Self::REPR_END - Self::REPR_MIN;
            const REPR_INDEX_MAX: Self::Repr = match Self::REPR_COUNT {
                //0 => panic!("empty enum"),
                count => count - 1,
            };
            const ENUM_MIN: Self = unsafe { Self::from_repr_unchecked(Self::REPR_MIN) };
            const ENUM_MAX: Self = unsafe { Self::from_repr_unchecked(Self::REPR_MAX) };

            #[inline(always)]
            fn to_repr(self) -> Self::Repr {
                <$id>::to_repr(self)
            }
            #[inline(always)]
            unsafe fn from_repr_unchecked(repr: Self::Repr) -> Self {
                <$id>::from_repr_unchecked(repr)
            }
            #[inline(always)]
            unsafe fn from_repr_index_unchecked(index: Self::Repr) -> Self {
                <$id>::from_repr_index_unchecked(index)
            }
            #[inline(always)]
            fn to_repr_index(self) -> Self::Repr {
                <$id>::to_repr_index(self)
            }
        }
        impl $id {
            #[inline(always)]
            pub const fn to_repr(self) -> <Self as $crate::repr::EnumRepr>::Repr {
                unsafe { ::core::mem::transmute(self) }
            }
            #[inline(always)]
            pub const fn to_repr_index(self) -> <Self as $crate::repr::EnumRepr>::Repr {
                unsafe { self.to_repr().unchecked_sub(<Self as $crate::repr::EnumRepr>::REPR_MIN) }
            }
            #[inline(always)]
            pub const unsafe fn from_repr_unchecked(repr: <Self as $crate::repr::EnumRepr>::Repr) -> Self {
                ::core::mem::transmute(repr)
            }
            #[inline(always)]
            pub const unsafe fn from_repr_index_unchecked(index: <Self as $crate::repr::EnumRepr>::Repr) -> Self {
                Self::from_repr_unchecked(index.unchecked_add(<Self as $crate::repr::EnumRepr>::REPR_MIN))
            }
        }
    };
    (@impl(EnumRepr)
        @meta($($meta_ignore:tt)*)
        #[repr($repr:ty)]
        $(#[$($meta_trailing:tt)*])+
        $vis:vis enum $id:ty {
            $($inner:tt)*
        }
    ) => {
        $crate::repr::EnumReprFromContiguous! {
            @impl(EnumRepr)
            // if other attributes are of interest, change this to not discard everything!
            @meta($($meta_ignore)* $(#[$($meta_trailing)*])*)
            #[repr($repr)]
            $vis enum $id {
                $($inner)*
            }
        }
    };
    (@impl(EnumRepr)
        @meta($($meta_ignore:tt)*)
        #[$($meta0:tt)*]
        $(#[$($meta:tt)*])*
        $vis:vis enum $id:ty {
            $($inner:tt)*
        }
    ) => {
        $crate::repr::EnumReprFromContiguous! {
            @impl(EnumRepr)
            @meta($($meta_ignore)* #[$($meta0)*])
            $(#[$($meta)*])*
            $vis enum $id {
                $($inner)*
            }
        }
    };
    (@impl(EnumRepr)
        @meta($($meta_ignore:tt)*)
        $vis:vis enum $id:ty {
            $($inner:tt)*
        }
    ) => {
        compile_error! {
            "EnumRepr expected #[repr($ty)]"
        }
    };
}
pub use {crate::EnumRepr, EnumReprFromContiguous};
