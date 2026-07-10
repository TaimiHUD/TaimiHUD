//! most of this hopefully is just a temporary staging ground for things
//! before they're ready to be pulled into [arcffi]...

use core::{cell::UnsafeCell, ops};

pub mod cstr;
pub mod data;
#[cfg(todo)]
pub mod r#dyn;
#[cfg(feature = "fnalloc")]
pub mod fnalloc;
pub mod repr;
#[cfg(feature = "windows")]
pub mod win32;

// allow this to serve as a package alias for convenience
#[rustfmt::skip]
pub use ::arcffi::{
    self,
    nn::{self, nonnull_ref, nonnull_unwrap, nonnull_unwrap_mut},
    // ptr::*
    transmute_unchecked, write_copy_of_slice, write_clone_of_slice,
    UserMallocFn, UserFreeFn,
    extern_fns,
    wide,
    windows,
    // std re-exports
    alloc,
    NonNull, c_void, c_int, c_uint, c_long, c_ulong, c_longlong, c_ulonglong, c_char, c_uchar, c_schar,
    c_wchar, c_senum, c_uenum, c_bool, c_bool32,
};

#[repr(transparent)]
#[derive(Debug, Default)]
pub struct UnsaferCell<T: ?Sized>(pub UnsafeCell<T>);
unsafe impl<T: ?Sized> Sync for UnsaferCell<T> {}
unsafe impl<T: ?Sized> Send for UnsaferCell<T> {}
impl<T: ?Sized> UnsaferCell<T> {
    #[inline(always)]
    pub const unsafe fn new(v: T) -> Self
    where
        T: Sized,
    {
        Self(UnsafeCell::new(v))
    }
    #[inline(always)]
    pub const unsafe fn from_ref(v: &UnsafeCell<T>) -> &Self {
        &*(v as *const UnsafeCell<T> as *const Self)
    }
    #[inline(always)]
    pub const unsafe fn from_mut(v: &mut UnsafeCell<T>) -> &mut Self {
        &mut *(v as *mut UnsafeCell<T> as *mut Self)
    }
    #[inline(always)]
    pub const unsafe fn from_ptr<'a>(v: *mut T) -> &'a Self {
        &*(v as *mut UnsafeCell<T> as *const _ as *const Self)
    }

    #[inline(always)]
    pub const fn get(&self) -> *mut T {
        self.0.get()
    }
    #[inline(always)]
    pub const fn raw_get(this: *const Self) -> *mut T {
        UnsafeCell::raw_get(this as *const UnsafeCell<T>)
    }

    #[inline(always)]
    pub const unsafe fn as_ref_unchecked(&self) -> &T {
        &*self.0.get()
    }
    #[inline(always)]
    pub unsafe fn as_mut_unchecked(&self) -> &mut T {
        &mut *self.0.get()
    }
}
impl<T: ?Sized> ops::Deref for UnsaferCell<T> {
    type Target = UnsafeCell<T>;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<T: ?Sized> ops::DerefMut for UnsaferCell<T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
