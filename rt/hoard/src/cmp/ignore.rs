use core::{
    borrow::{Borrow, BorrowMut},
    cmp,
    hash::{Hash, Hasher},
    mem,
    ops,
};

#[derive(Debug, Copy, Clone, Default)]
#[repr(transparent)]
pub struct CmpIgnore<T: ?Sized>(pub T);
impl<T: ?Sized> CmpIgnore<T> {
    #[inline(always)]
    pub const fn from_ref(v: &T) -> &Self {
        unsafe { mem::transmute(v) }
    }
    #[inline]
    pub fn from_mut(v: &mut T) -> &mut Self {
        unsafe { mem::transmute(v) }
    }
    #[inline(always)]
    pub const fn inner_ref(&self) -> &T {
        &self.0
    }
    #[inline(always)]
    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.0
    }
}
impl<T> CmpIgnore<T> {
    #[inline(always)]
    pub const fn new(v: T) -> Self {
        Self(v)
    }
    #[inline(always)]
    pub fn into_inner(self) -> T {
        self.0
    }
}
impl<T: ?Sized> ops::Deref for CmpIgnore<T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<T: ?Sized> ops::DerefMut for CmpIgnore<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
/// always true
impl<T: ?Sized, R: ?Sized> PartialEq<CmpIgnore<R>> for CmpIgnore<T> {
    #[inline]
    fn eq(&self, _rhs: &CmpIgnore<R>) -> bool {
        true
    }
}
impl<T: ?Sized> Eq for CmpIgnore<T> {}
/// always equal
impl<T: ?Sized, R: ?Sized> PartialOrd<CmpIgnore<R>> for CmpIgnore<T> {
    #[inline(always)]
    fn partial_cmp(&self, _rhs: &CmpIgnore<R>) -> Option<cmp::Ordering> {
        Some(cmp::Ordering::Equal)
    }
}
/// always equal
impl<T: ?Sized> Ord for CmpIgnore<T> {
    #[inline(always)]
    fn cmp(&self, _rhs: &Self) -> cmp::Ordering {
        cmp::Ordering::Equal
    }
}
/// no-op, as if we were never here
impl<T: ?Sized> Hash for CmpIgnore<T> {
    #[inline(always)]
    fn hash<H: Hasher>(&self, state: &mut H) {
        match state {
            #[cfg(todo = "unnecessary")]
            state => ().hash(state),
            _ => (),
        }
    }
}
impl<T: ?Sized> Borrow<T> for CmpIgnore<T> {
    #[inline(always)]
    fn borrow(&self) -> &T {
        &self.0
    }
}
impl<T: ?Sized> BorrowMut<T> for CmpIgnore<T> {
    #[inline(always)]
    fn borrow_mut(&mut self) -> &mut T {
        &mut self.0
    }
}
impl<T: ?Sized, U: ?Sized> AsRef<U> for CmpIgnore<T>
where
    T: AsRef<U>,
{
    #[inline(always)]
    fn as_ref(&self) -> &U {
        self.0.as_ref()
    }
}
impl<T: ?Sized, U: ?Sized> AsMut<U> for CmpIgnore<T>
where
    T: AsMut<U>,
{
    #[inline(always)]
    fn as_mut(&mut self) -> &mut U {
        self.0.as_mut()
    }
}
impl<'a, T> From<T> for CmpIgnore<T> {
    #[inline(always)]
    fn from(v: T) -> CmpIgnore<T> {
        CmpIgnore(v)
    }
}
