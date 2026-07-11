use core::{borrow::Borrow, cell::UnsafeCell, marker::PhantomData, mem, ops, ptr::NonNull};

#[cfg_attr(todo, derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[derive(Debug)]
#[repr(transparent)]
pub struct ImPtr<'a, T: ?Sized> {
    pub _borrowed: PhantomData<&'a T>,
    raw: UnsafeCell<T>,
}
impl<'a, T: ?Sized> ImPtr<'a, T> {
    #[inline]
    pub const fn with_ptr(p: *const T) -> *const Self {
        unsafe { mem::transmute(p) }
    }
    #[inline]
    pub const fn with_ptr_mut(p: *mut T) -> *mut Self {
        unsafe { mem::transmute(p) }
    }
    #[inline]
    pub const fn with_nn(p: NonNull<T>) -> NonNull<Self> {
        unsafe { mem::transmute(p) }
    }
    #[inline]
    pub const unsafe fn from_ref(raw: &T) -> &Self {
        mem::transmute(raw)
    }
    #[inline]
    pub unsafe fn from_mut(raw: &mut T) -> &mut Self {
        mem::transmute(raw)
    }

    #[inline]
    pub const fn as_ptr(&self) -> *const T {
        self.raw.get() as *const T
    }
    #[inline]
    pub fn as_ptr_mut(&mut self) -> *mut T {
        self.raw.get()
    }
    #[inline]
    pub const fn get_ptr(&self) -> *mut T {
        self.raw.get()
    }
    #[inline]
    pub const fn as_nn(&self) -> NonNull<T> {
        unsafe { mem::transmute(self) }
    }
    #[inline]
    pub const fn as_nn_ref<'p>(self: &'p &'_ Self) -> &'p NonNull<T> {
        unsafe { mem::transmute(self) }
    }
    /// can't decide whether to keep this safe or not :<
    /// seems likely to complicate impls if not unsafe,
    /// and the UnsafeCell seems pointless then...
    #[inline]
    pub const fn as_raw(&self) -> &T {
        unsafe { &*self.raw.get() }
    }
    #[inline]
    pub unsafe fn as_raw_mut_unchecked(&self) -> &mut T {
        &mut *self.raw.get()
    }
    /// SAFETY: mutating the fields of the wrapped value must be done carefully,
    /// since pointer fields are public!
    #[inline]
    pub unsafe fn as_raw_mut(&mut self) -> &mut T {
        &mut *self.raw.get()
    }
}
impl<'a, T> ImPtr<'a, T> {
    #[inline]
    pub const unsafe fn new(raw: T) -> Self {
        Self {
            _borrowed: PhantomData,
            raw: UnsafeCell::new(raw),
        }
    }
    #[inline]
    pub fn into_raw(self) -> T {
        self.raw.into_inner()
    }
}
impl<'a, T> ops::Deref for ImPtr<'a, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_raw()
    }
}
impl<'a, T> AsRef<T> for ImPtr<'a, T> {
    #[inline]
    fn as_ref(&self) -> &T {
        self.as_raw()
    }
}
impl<'a, T> Borrow<T> for ImPtr<'a, T> {
    #[inline]
    fn borrow(&self) -> &T {
        self.as_raw()
    }
}
