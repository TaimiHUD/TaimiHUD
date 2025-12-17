use {
    core::{mem, ops, ptr::NonNull},
    std::{
        borrow::{Borrow, BorrowMut},
        sync::Arc,
    },
};

pub use self::ArcMutRef as ArcLazyMut;
#[cfg(todo = "unnecessary")]
#[derive(Debug)]
pub struct ArcLazyMut<'a, T: ?Sized> {
    owned: ArcLazyMutState,
    arc: NonNull<Arc<T>>,
    _arc: PhantomData<&'a mut Arc<T>>,
}
#[cfg(todo = "unnecessary")]
impl<'a, T: ?Sized> ArcLazyMut<'a, T> {
    #[inline]
    pub fn new(arc: &'a mut Arc<T>) -> Self {
        Self {
            owned: ArcLazyMutState::Arc,
            arc: unsafe { NonNull::new_unchecked(arc) },
            _arc: PhantomData,
        }
    }
    pub unsafe fn with_mut_unchecked(arc: &'a mut Arc<T>) -> Self {
        Self {
            owned: ArcLazyMutState::Mut,
            arc: unsafe { NonNull::new_unchecked(arc) },
            _arc: PhantomData,
        }
    }
    pub fn with_mut(arc: &'a mut Arc<T>) -> Option<Self> {
        let is_mut = Arc::get_mut(arc).is_some();
        is_mut.then(move || unsafe { Self::with_mut_unchecked(arc) })
    }
    #[inline]
    pub fn arc_ptr(&self) -> NonNull<Arc<T>> {
        self.arc
    }
    #[inline]
    pub fn get_ptr(&self) -> NonNull<T> {
        unsafe { NonNull::new_unchecked(Arc::as_ptr(&*self.arc.as_ptr()) as *mut T) }
    }
    #[cfg(todo = "unnecessary")]
    pub fn get_ptr(&self) -> NonNull<T> {
        unsafe {
            let ptr = self.arc.cast::<usize>().add(2);
            match mem::align_of::<T>() {
                0..=USIZE_ALIGN => ptr,
                alignment => ptr.add_bytes(ptr.align_offset(alignment)).cast::<T>(),
            }
        }
    }
    pub fn make_mut(&mut self) -> &mut T
    where
        T: Clone,
    {
        match self.owned {
            ArcLazyMutState::Mut => unsafe { self.get_mut_unchecked() },
            owned @ ArcLazyMutState::Arc => unsafe {
                let arc = &mut *self.arc.as_ptr();
                let inner = Arc::make_mut(arc);
                self.owned = ArcLazyMutState::Mut;
                inner
            },
        }
    }
    pub fn get_ref(&self) -> &T {
        unsafe { mem::transmute(self.get_ptr()) }
    }
    pub fn get_mut(&mut self) -> Option<&mut T> {
        match self.owned {
            ArcLazyMutState::Mut => Some(unsafe { self.get_mut_unchecked() }),
            ArcLazyMutState::Arc => {
                let inner = unsafe { Arc::get_mut(&mut *self.arc.as_ptr()) };
                self.owned = ArcLazyMutState::Mut;
                inner
            },
        }
    }
    #[inline]
    pub unsafe fn get_mut_unchecked(&mut self) -> &mut T {
        mem::transmute(self.get_ptr())
    }
    #[inline]
    pub unsafe fn arc_ref(&self) -> &Arc<T> {
        mem::transmute(self.arc_ptr())
    }
    #[inline]
    pub unsafe fn arc_mut(&mut self) -> &mut Arc<T> {
        mem::transmute(self.arc_ptr())
    }
}
#[cfg(todo = "unnecessary")]
impl<'a, T: ?Sized> ops::Deref for ArcLazyMut<'a, T> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.get_ref()
    }
}
#[cfg(todo = "unnecessary")]
impl<'a, T: ArcMakeMut> ops::DerefMut for ArcLazyMut<'a, T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.make_mut()
    }
}
#[derive(Debug)]
pub struct ArcMut<T: ?Sized> {
    pub arc: Arc<T>,
}
impl<T: ?Sized> ArcMut<T> {
    #[inline]
    pub const fn new(arc: Arc<T>) -> Self {
        Self { arc }
    }
    #[inline]
    pub const fn from_ref(arc: &Arc<T>) -> &Self {
        unsafe { mem::transmute(arc) }
    }
    #[inline]
    pub fn from_mut(arc: &mut Arc<T>) -> &mut Self {
        unsafe { mem::transmute(arc) }
    }
    #[inline]
    pub fn into_inner(self) -> Arc<T> {
        self.arc
    }
    #[inline]
    pub fn as_arc(&self) -> &Arc<T> {
        &self.arc
    }
    #[inline]
    pub fn as_arc_mut(&mut self) -> &mut Arc<T> {
        &mut self.arc
    }
    #[inline]
    pub fn get_ref(&self) -> &T {
        &*self.arc
    }
    #[inline]
    pub fn make_mut(&mut self) -> &mut T
    where
        T: Clone,
    {
        Arc::make_mut(&mut self.arc)
    }
}
impl<T: ?Sized> ops::Deref for ArcMut<T> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.get_ref()
    }
}
impl<T: Clone> ops::DerefMut for ArcMut<T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.make_mut()
    }
}
impl<T: Clone> ops::DerefMut for ArcMut<[T]> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        Arc::make_mut(&mut self.arc)
    }
}
#[derive(Debug)]
pub struct ArcMutRef<'a, T: ?Sized> {
    owned: ArcLazyMutState,
    arc: &'a mut Arc<T>,
}
impl<'a, T: ?Sized> ArcMutRef<'a, T> {
    #[inline]
    pub fn new(arc: &'a mut Arc<T>) -> Self {
        Self { owned: ArcLazyMutState::Arc, arc }
    }
    pub unsafe fn with_mut_unchecked(arc: &'a mut Arc<T>) -> Self {
        Self { owned: ArcLazyMutState::Mut, arc }
    }
    pub fn with_mut(arc: &'a mut Arc<T>) -> Option<Self> {
        let is_mut = Arc::get_mut(arc).is_some();
        is_mut.then(move || unsafe { Self::with_mut_unchecked(arc) })
    }
    #[inline]
    pub fn by_ref<'b>(&'b mut self) -> ArcMutRef<'b, T>
    where
        'a: 'b,
    {
        ArcMutRef { owned: self.owned, arc: &mut *self.arc }
    }
    pub fn make_mut(&mut self) -> &mut T
    where
        T: ArcMakeMut,
    {
        match self.owned {
            ArcLazyMutState::Mut => unsafe { self.get_mut_unchecked() },
            ArcLazyMutState::Arc => {
                let inner = ArcMakeMut::arc_make_mut(&mut self.arc);
                self.owned = ArcLazyMutState::Mut;
                inner
            },
        }
    }
    #[inline]
    pub fn arc_ptr(&self) -> NonNull<Arc<T>> {
        unsafe { NonNull::new_unchecked(&*self.arc as *const Arc<T> as *mut Arc<T>) }
    }
    #[inline]
    pub fn get_ptr(&self) -> NonNull<T> {
        unsafe { NonNull::new_unchecked(Arc::as_ptr(&self.arc) as *mut T) }
    }
    #[inline]
    pub fn get_ref(&self) -> &T {
        &*self.arc
    }
    pub fn get_mut(&mut self) -> Option<&mut T> {
        match self.owned {
            ArcLazyMutState::Mut => Some(unsafe { self.get_mut_unchecked() }),
            ArcLazyMutState::Arc => {
                let inner = Arc::get_mut(&mut self.arc);
                self.owned = ArcLazyMutState::Mut;
                inner
            },
        }
    }
    #[inline]
    pub unsafe fn get_mut_unchecked(&mut self) -> &mut T {
        mem::transmute(self.get_ptr())
    }
    #[inline]
    pub unsafe fn arc_ref(&self) -> &Arc<T> {
        &self.arc
    }
    #[inline]
    pub unsafe fn arc_mut(&mut self) -> &mut Arc<T> {
        &mut self.arc
    }
}
impl<'a, T: ?Sized> ops::Deref for ArcMutRef<'a, T> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.get_ref()
    }
}
impl<'a, T: ArcMakeMut> ops::DerefMut for ArcMutRef<'a, T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.make_mut()
    }
}
impl<'a, T: ?Sized> From<&'a mut Arc<T>> for ArcMutRef<'a, T> {
    #[inline]
    fn from(arc: &'a mut Arc<T>) -> Self {
        Self::new(arc)
    }
}
impl<'a, T: ?Sized, U: ?Sized> Borrow<U> for ArcMutRef<'a, T>
where
    Arc<T>: Borrow<U>,
{
    #[inline]
    fn borrow(&self) -> &U {
        Borrow::borrow(&*self.arc)
    }
}
impl<'a, T: ?Sized, U: ?Sized> Borrow<U> for &'_ ArcMutRef<'a, T>
where
    Arc<T>: Borrow<U>,
{
    #[inline]
    fn borrow(&self) -> &U {
        Borrow::borrow(&*self.arc)
    }
}
impl<'a, T: ?Sized, U: ?Sized> Borrow<U> for &'_ mut ArcMutRef<'a, T>
where
    Arc<T>: Borrow<U>,
{
    #[inline]
    fn borrow(&self) -> &U {
        Borrow::borrow(&*self.arc)
    }
}
#[cfg(todo)]
impl<'a, T: ArcMakeMut, U: ?Sized> BorrowMut<U> for ArcMutRef<'a, T>
where
    T: AsMut<U>,
{
    #[inline]
    fn borrow_mut(&mut self) -> &mut U {
        AsMut::as_mut(self.make_mut())
    }
}
impl<'a, T: ?Sized + ArcMakeMut> BorrowMut<T> for ArcMutRef<'a, T> {
    #[inline]
    fn borrow_mut(&mut self) -> &mut T {
        self.make_mut()
    }
}
impl<'a, T: ?Sized + ArcMakeMut> BorrowMut<T> for &'_ mut ArcMutRef<'a, T> {
    #[inline]
    fn borrow_mut(&mut self) -> &mut T {
        self.make_mut()
    }
}
impl<'a, T: ?Sized, U: ?Sized> AsRef<U> for ArcMutRef<'a, T>
where
    Arc<T>: AsRef<U>,
{
    #[inline]
    fn as_ref(&self) -> &U {
        AsRef::as_ref(&*self.arc)
    }
}
impl<'a, T: ?Sized + ArcMakeMut, U: ?Sized> AsMut<U> for ArcMutRef<'a, T>
where
    T: AsMut<U>,
{
    #[inline]
    fn as_mut(&mut self) -> &mut U {
        AsMut::as_mut(self.make_mut())
    }
}
#[derive(Debug, Copy, Clone)]
enum ArcLazyMutState {
    Arc,
    Mut,
    #[cfg(todo = "unnecessary")]
    Poisoned,
}

/// CloneToUninit
pub trait ArcMakeMut {
    fn arc_make_mut(arc: &mut Arc<Self>) -> &mut Self;
}
impl<T: Clone> ArcMakeMut for T {
    #[inline(always)]
    fn arc_make_mut(arc: &mut Arc<Self>) -> &mut Self {
        Arc::make_mut(arc)
    }
}
impl<T: Clone> ArcMakeMut for [T] {
    #[inline(always)]
    fn arc_make_mut(arc: &mut Arc<Self>) -> &mut Self {
        Arc::make_mut(arc)
    }
}
#[cfg(todo)]
impl<T: CloneToUninit> ArcMakeMut for T {}
