//! Shared data with a single receiver, an spsc/mpsc-alike mutex I guess.
//! Start with [MoveShare::new()]
//!
//! Tradeoffs made here mean [sending](MoveShare) a bit inefficient in order
//! to allow [receiver](MoveShared) to avoid locks that should block. If you're
//! obsessive about avoiding blocks, [fall back to an outdated value](MoveShared::try_get_updated)
//!
//! Prefer [watched](crate::watched) over this unless you really want a single
//! exclusive receiver (render thread for example)

use {
    crate::{drop_poison, PoisonError},
    core::{marker::PhantomData, mem, num::NonZero, ops},
    std::sync::{Arc, Mutex, Weak},
};

pub struct MoveShareArc<T: ?Sized> {
    pub inner: MoveShareInner<T>,
}
impl<T> MoveShareArc<T> {
    /// create the "channel"
    ///
    /// delayed construction via [Self::new_unshared] isn't recommended
    pub fn new<V: Into<Arc<T>>>(initial: V) -> (Self, MoveShared<T>) {
        let this = Self::new_unshared(initial.into());
        let rx = MoveShared::subscribe_unchecked(&this);
        (this, rx)
    }

    pub fn subscribe(&mut self) -> Option<MoveShared<T>> {
        if self.receiver_count().is_some() {
            return None
        }

        Some(MoveShared::subscribe_unchecked(self))
    }
}
impl<T: ?Sized> MoveShareArc<T> {
    #[inline]
    pub fn write_inplace<R, F: FnOnce(&mut Arc<T>) -> R>(&self, f: F) -> Result<R, PoisonError<()>> {
        Self::inner_with(&self.inner, f)
    }
    fn inner_with<R, F: FnOnce(&mut Arc<T>) -> R>(
        inner: &MoveShareInner<T>,
        f: F,
    ) -> Result<R, PoisonError<()>> {
        let mut inner = inner.lock().map_err(drop_poison)?;
        Ok(f(&mut inner))
    }
    /// I'd care to make this correct but `Arc::is_unique` is unstable so
    /// don't be pathological thanks
    pub fn receiver_count(&self) -> Option<NonZero<usize>> {
        NonZero::new(Arc::strong_count(&self.inner))
    }

    /// if we're exclusive...
    ///
    /// only works with single-sender so if multiple are needed, see
    /// [MoveShared::subscribe_unchecked]
    pub fn subscribe_unsized(&mut self) -> Option<MoveShared<T>> {
        if self.receiver_count().is_some() {
            return None
        }

        Some(MoveShared::subscribe_unchecked_unsized(self))
    }

    pub const fn new_with(inner: MoveShareInner<T>) -> Self {
        Self { inner }
    }

    pub fn new_unshared(initial: Arc<T>) -> Self {
        Self::new_with(Arc::new(Mutex::new(initial)))
    }
}
impl<T: ?Sized> Clone for MoveShareArc<T> {
    fn clone(&self) -> Self {
        Self::new_with(self.inner.clone())
    }
}

pub struct MoveShare<T> {
    pub inner: MoveShareArc<T>,
    pub working: Option<T>,
}
impl<T> MoveShare<T> {
    pub fn new<V: Into<Arc<T>>>(initial: V) -> (Self, MoveShared<T>) {
        let this = Self::new_unshared(initial.into());
        let rx = MoveShared::subscribe_unchecked(&this);
        (this, rx)
    }

    /// prefer [Self::new_unshared]
    pub const fn new_with(inner: MoveShareArc<T>) -> Self {
        Self { inner, working: None }
    }

    /// prefer [Self::new]
    ///
    /// otherwise this can be used in conjunction with [self.subscribe()]
    pub fn new_unshared(initial: Arc<T>) -> Self {
        Self::new_with(MoveShareArc::new_unshared(initial))
    }

    pub fn place<I: Into<Arc<T>>>(&mut self, v: I) -> Result<(), PoisonError<()>> {
        let v = v.into();
        self.inner.write_inplace(move |i| mem::replace(i, v))?;
        self.working = None;
        Ok(())
    }
}
impl<T: Clone> MoveShare<T> {
    /// prefer [self.work_with] for large or non-Copy types,
    /// or [self.place] if `T: !Clone`
    ///
    /// if the difference is irrelevant, consider a plain `Mutex<T>`
    pub fn set<I: Into<Arc<T>>>(&mut self, v: I) -> Result<(), PoisonError<()>> {
        let v = v.into();
        let working = v.clone();
        self.inner.write_inplace(move |i| mem::replace(i, v))?;
        self.working = Some(T::clone(&working));
        Ok(())
    }

    /// hopefully clones are cheap or this particular write is unlikely to
    /// require committing the change!
    pub fn work_with<R, F: FnOnce(&mut T, &mut bool) -> R>(&mut self, f: F) -> Result<R, PoisonError<()>>
    where
        T: Clone,
    {
        let working = match &mut self.working {
            Some(working) => working,
            working @ None => working.insert({
                let current = self.inner.write_inplace(|i| Arc::clone(i))?;
                T::clone(&current)
            }),
        };
        let mut commit = true;
        let res = f(working, &mut commit);
        if commit {
            let updated = Arc::new(working.clone());
            self.inner.write_inplace(|i| mem::replace(i, updated))?;
        }
        Ok(res)
    }
    pub fn write_inplace<R, F: FnOnce(&mut Arc<T>, &mut bool) -> R>(
        &mut self,
        f: F,
    ) -> Result<R, PoisonError<()>> {
        let mut commit = true;
        let res = self.inner.write_inplace(|i| f(i, &mut commit));
        if commit {
            self.working = None;
        }
        res
    }
}
impl<T> ops::Deref for MoveShare<T> {
    type Target = MoveShareArc<T>;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl<T> ops::DerefMut for MoveShare<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
impl<T> Clone for MoveShare<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            working: None,
        }
    }
}

/// Receiving end of [MoveShare]
pub struct MoveShared<T: ?Sized> {
    /// please don't thanks
    pub inner: MoveShareInner<T>,
    pub handle: Weak<T>,
    pub prev: Option<Arc<T>>,
    pub _unsync: PhantomData<*const ()>,
}
impl<T> MoveShared<T> {
    /// please don't thanks
    #[doc(hidden)]
    pub fn subscribe_unchecked(share: &MoveShareArc<T>) -> Self {
        Self {
            inner: share.inner.clone(),
            handle: Weak::new(),
            prev: None,
            _unsync: PhantomData,
        }
    }
}
impl<T: ?Sized> MoveShared<T> {
    /// please don't thanks
    #[doc(hidden)]
    pub fn subscribe_unchecked_unsized(share: &MoveShareArc<T>) -> Self {
        let handle = share
            .write_inplace(|i| Arc::downgrade(i))
            .expect("MoveShared::subscribe");
        Self {
            inner: share.inner.clone(),
            handle,
            prev: None,
            _unsync: PhantomData,
        }
    }

    pub fn get<'a>(&'a mut self) -> MoveSharedRef<'a, T> {
        if self.prev.is_some() {
            let v = self.try_get_outdated().clone();
            v
        } else if let Some(v) = self.handle.upgrade() {
            MoveSharedRef::from_inner(v)
        } else {
            self.lock_get()
        }
    }
    /// defeat the whole purpose of the [self.handle] why don't you
    pub fn try_get<'a>(&'a mut self) -> Option<MoveSharedRef<'a, T>> {
        self.prev = None;
        self.handle.upgrade().map(MoveSharedRef::from_inner)
    }
    /// defeat the whole purpose of the [self.handle] why don't you
    pub fn lock_get<'a>(&'a mut self) -> MoveSharedRef<'a, T> {
        MoveSharedRef::from_inner(self.lock_and_get())
    }
    /// just [self.lock_get()] instead unless you intend to [self.try_get_outdated()]
    /// later on
    pub fn lock_update<'a>(&'a mut self) -> &'a MoveSharedRef<'a, T> {
        MoveSharedRef::from_ref(&*self.lock_and_update())
    }

    #[doc(hidden)]
    pub fn try_lock_get(&mut self) -> Option<Arc<T>> {
        let res = self.inner.try_lock().ok().map(|v| v.clone());
        if let Some(v) = &res {
            self.handle = Arc::downgrade(&v);
        }
        res
    }
    #[doc(hidden)]
    pub fn just_lock_get(&mut self) -> Arc<T> {
        let v = self.inner.lock().expect("MoveShared").clone();
        self.handle = Arc::downgrade(&v);
        v
    }
    #[doc(hidden)]
    pub fn lock_and_get(&mut self) -> Arc<T> {
        let v = self.just_lock_get();
        self.prev = None;
        v
    }
    #[doc(hidden)]
    pub fn lock_and_update(&mut self) -> &mut Arc<T> {
        let v = self.just_lock_get();
        self.prev.insert(v)
    }

    /// Attempts to update, but may fall back to [self.prev] if lock is contended
    ///
    /// always stashes and returns the result of `Some(ref) = &self.prev`
    pub fn try_get_outdated<'a>(&'a mut self) -> &'a MoveSharedRef<'a, T> {
        MoveSharedRef::from_ref(match self.try_get_inner() {
            Ok(v) => v,
            Err(v) => v,
        })
    }

    #[doc(hidden)]
    pub fn try_get_inner(&mut self) -> Result<&Arc<T>, &Arc<T>> {
        let is_outdated = match (&self.handle, &self.prev) {
            #[cfg(todo)]
            (h, None) if weak_is_null(h) =>
            // requires Sized bound bleh
                true,
            _ => {
                let outdated_strong = match self.prev.as_ref() {
                    Some(prev)
                        if Arc::as_ptr(prev) as *const () as usize
                            == Weak::as_ptr(&self.handle) as *const () as usize =>
                    // if we hold a backup, 1 indicates the sender has updated since
                        1,
                    _ => 0,
                };
                Weak::strong_count(&self.handle) == outdated_strong
            },
        };
        let mut upgrade = None;
        let mut failed_upgrade = false;
        if is_outdated && self.prev.is_some() {
            upgrade = self.try_lock_get();
            if upgrade.is_none() {
                failed_upgrade = true;
            }
        }
        if self.prev.is_none() {
            if upgrade.is_none() {
                upgrade = self.handle.upgrade();
            }
            if upgrade.is_none() {
                // guarantee we cannot be stuck with `None` unless prev exists as a fallback
                upgrade = Some(self.just_lock_get());
                failed_upgrade = false;
            }
        }
        let v = match (&mut self.prev, upgrade) {
            (prev, Some(upgrade)) => &*prev.insert(upgrade),
            #[cfg(debug_assertions)]
            (None, None) => unreachable!(),
            (prev, None) => unsafe { prev.as_ref().unwrap_unchecked() },
        };
        match failed_upgrade {
            true => Err(v),
            false => Ok(v),
        }
    }

    /// if we fail to update and fall back to `prev`, require next call to block
    ///
    /// limits how outdated the result can be if multiple attempts to lock fail in
    /// a row
    pub fn try_get_outdated_once(&mut self) -> MoveSharedRef<'_, T> {
        let outdated = self.try_get_inner().is_err();
        // self.prev is now guaranteed to be Some() now, so check if it's outdated...
        let v = match outdated {
            true => self.prev.take(),
            false => self.prev.clone(),
        };
        MoveSharedRef::from_inner(unsafe { v.unwrap_unchecked() })
    }
}

unsafe impl<T: ?Sized> Send for MoveShared<T> {}

pub type MoveShareInner<T> = Arc<Mutex<Arc<T>>>;

#[repr(transparent)]
pub struct MoveSharedRef<'s, T: ?Sized> {
    /// please don't steal or clone this thanks
    pub inner: Arc<T>,
    pub _borrow: PhantomData<&'s MoveShared<T>>,
}
#[doc(hidden)]
impl<T: ?Sized> MoveSharedRef<'_, T> {
    #[inline]
    pub const fn from_inner(inner: Arc<T>) -> Self {
        Self { inner, _borrow: PhantomData }
    }
    #[inline]
    pub const fn from_ref(inner: &Arc<T>) -> &Self {
        unsafe { mem::transmute(inner) }
    }
    #[inline]
    pub const fn from_mut(inner: &mut Arc<T>) -> &mut Self {
        unsafe { mem::transmute(inner) }
    }
}
impl<T: ?Sized> ops::Deref for MoveSharedRef<'_, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
/// Clone is technically allowed here but does not mean this should live longer
/// than the temporary borrow of the underlying [receiver](MoveShared)
///
/// so if used, you're probably doing something wrong
impl<T: ?Sized> Clone for MoveSharedRef<'_, T> {
    fn clone(&self) -> Self {
        Self::from_inner(self.inner.clone())
    }
}
