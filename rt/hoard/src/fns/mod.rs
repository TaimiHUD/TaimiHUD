use core::mem;

/// ZST [fn]
///
/// TODO: was there a reason for args to not be associated?
/// overloading isn't exactly common in rustland...
pub unsafe trait Zfn<A>: Sized {
    type Output: Sized;

    unsafe fn call_unchecked(self, args: A) -> Self::Output;

    unsafe fn materialize() -> Self {
        debug_assert_eq!(mem::size_of::<Self>(), 0);
        let f = ();
        transmute_unchecked(f)
    }
}

/// ZST [Zfn] + [Fn]
pub unsafe trait ZFn<A>: Zfn<A> {
    fn call(self, args: A) -> Self::Output {
        unsafe { self.call_unchecked(args) }
    }
}
pub trait ZFn0: ZFn<()> {
    type Fn: Fn() -> <Self as Zfn<()>>::Output;

    fn call0(self) -> <Self as Zfn<()>>::Output {
        self.call(())
    }
    unsafe fn materialize0() -> Self::Fn;
}
pub trait ZFn1<A0>: ZFn<(A0,)> {
    type Fn: Fn(A0) -> <Self as Zfn<(A0,)>>::Output;

    fn call1(self, a0: A0) -> <Self as Zfn<(A0,)>>::Output {
        self.call((a0,))
    }
    unsafe fn materialize1() -> Self::Fn;
}

unsafe impl<R> Zfn<()> for fn() -> R {
    type Output = R;
    #[inline]
    unsafe fn call_unchecked(self, (): ()) -> Self::Output {
        self()
    }
}
#[cfg(todo = "unnecessary")]
unsafe impl<R> ZFn<()> for fn() -> R {}
unsafe impl<R> Zfn<()> for unsafe fn() -> R {
    type Output = R;
    #[inline]
    unsafe fn call_unchecked(self, (): ()) -> Self::Output {
        self()
    }
}
unsafe impl<R, F: ZFn<(), Output = R> + Fn() -> R> ZFn<()> for F {}
impl<R, F: ZFn<(), Output = R> + Fn() -> R> ZFn0 for F {
    type Fn = F;
    unsafe fn materialize0() -> Self::Fn {
        Self::materialize()
    }
}
unsafe impl<A0, R> Zfn<(A0,)> for fn(A0) -> R {
    type Output = R;
    #[inline]
    unsafe fn call_unchecked(self, (a0,): (A0,)) -> Self::Output {
        self(a0)
    }
}
unsafe impl<A0, R> Zfn<(A0,)> for unsafe fn(A0) -> R {
    type Output = R;
    #[inline]
    unsafe fn call_unchecked(self, (a0,): (A0,)) -> Self::Output {
        self(a0)
    }
}
unsafe impl<A0, R, F: ZFn<(A0,), Output = R> + Fn(A0) -> R> ZFn<(A0,)> for F {}
#[cfg(todo = "unnecessary")]
unsafe impl<A0, R> ZFn<(A0,)> for fn(A0) -> R {}
impl<R, A0, F: ZFn<(A0,), Output = R> + Fn(A0) -> R> ZFn1<A0> for F {
    type Fn = F;
    unsafe fn materialize1() -> Self::Fn {
        Self::materialize()
    }
}

pub const unsafe fn zfn<Z: Zfn<A>, A>() -> Z {
    match mem::size_of::<Z>() {
        0 => {
            let f = ();
            transmute_unchecked(f)
        },
        _ => panic!("Zfn must be impl'd on a ZST (e.g. fn item type)"),
    }
}
/// what use is this?
pub const unsafe fn try_zfn0<R, Z: Fn() -> R>(_f: &'_ Z) -> Option<Z> {
    match mem::size_of::<Z>() {
        0 => Some(unsafe {
            let f = ();
            mem::transmute_copy(&f)
        }),
        _ => None,
    }
}

/// TODO: unstable intrinsic?
#[inline]
const unsafe fn transmute_unchecked<S, D>(s: S) -> D {
    let s = mem::ManuallyDrop::new(s);
    mem::transmute_copy(&s)
}
