use core::{borrow::Borrow, cmp, hash::{Hash, Hasher}, mem, ops};
use std::sync::Arc;

/// XXX: not the same as [Arc::ptr_eq],
/// may compare metadata!
///
/// cmp trait impls just use [Arc::as_ptr]
#[derive(Debug, Default)]
#[repr(transparent)]
pub struct ArcPtrCmp<T: ?Sized>(pub Arc<T>);
impl<T: ?Sized> ArcPtrCmp<T> {
    pub const fn from_ref(inner: &Arc<T>) -> &Self {
        unsafe {
            mem::transmute(inner)
        }
    }
    pub fn from_mut(inner: &mut Arc<T>) -> &mut Self {
        unsafe {
            mem::transmute(inner)
        }
    }
    pub fn into_inner_arc(self) -> Arc<T> { self.0 }
    /// skip clone op if unchanged
    pub fn clone_from_arc(&mut self, arc: &Arc<T>) -> bool {
        if Self::eq(&*self, arc) { return false }
        self.0 = arc.clone();
        true
    }
    /// idk a correct stable way to do this, so...
    #[inline]
    pub const fn is_unsized_ptr() -> bool {
        const SIZED_SIZE: usize = mem::size_of::<*const ()>();
        match mem::size_of::<*const T>() {
            SIZED_SIZE => false,
            _ => true,
        }
    }
    #[inline]
    pub fn cmp_scalar_ptr(v: &T) -> [usize; 2] {
        let sz = mem::size_of_val(v);
        let ptr = v as *const T;
        [ptr as *const () as usize, sz]
    }
    #[inline]
    pub fn cmp_scalar_arc(&self) -> [usize; 2] {
        Self::cmp_scalar_ptr(&*self.0)
    }
}

impl<T: ?Sized, R: ?Sized> PartialEq<R> for ArcPtrCmp<T> where
    R: Borrow<T>,
{
    #[inline]
    fn eq(&self, rhs: &R) -> bool {
        let rhs = Self::cmp_scalar_ptr(rhs.borrow());
        let lhs = Self::cmp_scalar_arc(self);
        PartialEq::eq(&lhs, &rhs)
    }
}
impl<T: ?Sized> Eq for ArcPtrCmp<T> {}

impl<T: ?Sized, R: ?Sized> PartialOrd<R> for ArcPtrCmp<T> where
    R: Borrow<T>,
{
    #[inline]
    fn partial_cmp(&self, rhs: &R) -> Option<cmp::Ordering> {
        let rhs = Self::cmp_scalar_ptr(rhs.borrow());
        let lhs = Self::cmp_scalar_arc(self);
        PartialOrd::partial_cmp(&lhs, &rhs)
    }
}
impl<T: ?Sized> Ord for ArcPtrCmp<T> {
    #[inline]
    fn cmp(&self, rhs: &Self) -> cmp::Ordering {
        let rhs = Self::cmp_scalar_ptr(rhs.borrow());
        let lhs = Self::cmp_scalar_arc(self);
        Ord::cmp(&lhs, &rhs)
    }
}
/// XXX: hashes [Self::cmp_scalar_arc] if unsized,
/// or just ptr if sized
impl<T: ?Sized> Hash for ArcPtrCmp<T> {
    fn hash<H: Hasher>(&self, h: &mut H) {
        match Self::is_unsized_ptr() {
            false => Hash::hash(&Arc::as_ptr(&self.0), h),
            true => {
                let p = Self::cmp_scalar_arc(self);
                Hash::hash(&p, h)
            },
        }
    }
}

impl<T: ?Sized> Clone for ArcPtrCmp<T> {
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
    #[inline]
    fn clone_from(&mut self, rhs: &Self) {
        self.clone_from_arc(&rhs.0);
    }
}

impl<T: ?Sized, U: ?Sized> Borrow<U> for ArcPtrCmp<T> where
    Arc<T>: Borrow<U>,
{
    fn borrow(&self) -> &U { Borrow::borrow(&self.0) }
}
impl<T: ?Sized, U: ?Sized> AsRef<U> for ArcPtrCmp<T> where
    Arc<T>: AsRef<U>,
{
    fn as_ref(&self) -> &U { AsRef::as_ref(&self.0) }
}
impl<T: ?Sized> ops::Deref for ArcPtrCmp<T> {
    type Target = Arc<T>;
    fn deref(&self) -> &Self::Target { &self.0 }
}
impl<T: ?Sized> ops::DerefMut for ArcPtrCmp<T> {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
}
impl<T: ?Sized> From<Arc<T>> for ArcPtrCmp<T> {
    fn from(inner: Arc<T>) -> Self {
        Self(inner)
    }
}
impl<T: ?Sized> From<ArcPtrCmp<T>> for Arc<T> {
    fn from(v: ArcPtrCmp<T>) -> Self {
        v.0
    }
}
