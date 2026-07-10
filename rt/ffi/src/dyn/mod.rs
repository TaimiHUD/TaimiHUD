use {
    crate::transmute_unchecked,
    core::{marker::PhantomData, ptr::NonNull},
};

pub type DynMetadataPtr = NonNull<()>;
pub type DynTargetPtr = NonNull<()>;
#[cfg(todo)]
pub type DynMetadataPtr = *const ();
#[cfg(todo)]
pub type DynTargetPtr = *mut ();

pub type DynSlice<'a, V> = DynSliceOf<V, &'a [DynTargetPtr]>;
pub struct DynSliceOf<V: ?Sized, T: ?Sized> {
    /// bleh I guess add a lifetime...
    _vtbl: PhantomData<V>,
    vtbl: DynMetadataPtr,
    data: T,
}
impl<V, T> DynSliceOf<V, T>
where
    V: ?Sized,
    T: ?Sized,
{
    #[inline]
    #[doc(hidden)]
    pub fn iter_ptrs_data<'a>(&'a self) -> <&'a T as IntoIterator>::IntoIter
    where
        &'a T: IntoIterator,
    {
        (&self.data).into_iter()
    }
    #[inline]
    #[doc(hidden)]
    pub unsafe fn iter_ptrs_data_mut<'a>(&'a mut self) -> <&'a mut T as IntoIterator>::IntoIter
    where
        &'a mut T: IntoIterator,
    {
        (&mut self.data).into_iter()
    }
    pub fn len<'a>(&'a self) -> usize
    where
        &'a T: IntoIterator,
        <&'a T as IntoIterator>::IntoIter: ExactSizeIterator,
    {
        self.iter_ptrs_data().len()
    }
    #[inline]
    pub fn ptr_at<'a>(&'a self, n: usize) -> Option<DynTargetPtr>
    where
        &'a T: IntoIterator,
        <&'a T as IntoIterator>::Item: DynTargetContainer<V>,
    {
        self.iter_ptrs_data().nth(n).map(|p| p.as_target_ptr())
    }
    #[inline(always)]
    fn check_ptr() {
        debug_assert_eq!(size_of::<Option<&V>>(), size_of::<[*const (); 2]>());
    }
    #[inline(always)]
    pub unsafe fn ptr_to_ref_unchecked<'a>(p: DynTargetPtr, vtbl: DynMetadataPtr) -> &'a V {
        // XXX: Option variant that unwraps the nn and zero vtbl if null?
        let p = [p, vtbl];
        unsafe { transmute_unchecked(p) }
    }
    #[inline(always)]
    pub unsafe fn ptr_to_mut_unchecked<'a>(p: DynTargetPtr, vtbl: DynMetadataPtr) -> &'a mut V {
        // XXX: Option variant that unwraps the nn and zero vtbl if null?
        let p = [p, vtbl];
        unsafe { transmute_unchecked(p) }
    }
    #[inline]
    pub fn borrow_at<'a>(&'a self, n: usize) -> Option<&'a V>
    where
        &'a T: IntoIterator,
        <&'a T as IntoIterator>::Item: DynTargetContainer<V>,
    {
        Self::check_ptr();
        let p = self.ptr_at(n)?;
        Some(unsafe { Self::ptr_to_ref_unchecked(p, self.vtbl) })
    }
    #[inline]
    pub fn borrow_at_mut<'a>(&'a mut self, n: usize) -> Option<&'a mut V>
    where
        &'a T: IntoIterator,
        <&'a T as IntoIterator>::Item: DynTargetContainer<V>,
    {
        Self::check_ptr();
        let p = self.ptr_at(n)?;
        Some(unsafe { Self::ptr_to_mut_unchecked(p, self.vtbl) })
    }
    #[inline]
    pub fn iter<'a>(&'a self) -> impl Iterator<Item = &'a V> + 'a
    where
        &'a T: IntoIterator,
        <&'a T as IntoIterator>::Item: DynTargetContainer<V>,
    {
        Self::check_ptr();
        let vtbl = self.vtbl;
        self.iter_ptrs_data()
            .map(move |p| unsafe { Self::ptr_to_ref_unchecked(p.as_target_ptr(), vtbl) })
    }
    /// TODO: only safe if `Self: !Clone` - need to use a marker trait or `SliceMut` type etc...
    #[inline]
    pub unsafe fn iter_mut_unchecked<'a>(&'a mut self) -> impl Iterator<Item = &'a mut V> + 'a
    where
        &'a T: IntoIterator,
        <&'a T as IntoIterator>::Item: DynTargetContainer<V>,
    {
        Self::check_ptr();
        let vtbl = self.vtbl;
        self.iter_ptrs_data()
            .map(move |p| unsafe { Self::ptr_to_mut_unchecked(p.as_target_ptr(), vtbl) })
    }
}
impl<V, T> DynSliceOf<V, T>
where
    V: ?Sized,
{
    pub unsafe fn from_parts_unchecked(vtbl: DynMetadataPtr, data: T) -> Self {
        Self { _vtbl: PhantomData, vtbl, data }
    }
    /// e.g. `ptr::dangling::<Thing>() as *const dyn Trait`
    #[inline(always)]
    pub unsafe fn with_example_unchecked(vtbl_ptr: *const V, data: T) -> Self {
        let vtbl = vtable_ptr_of::<V>(NonNull::new_unchecked(vtbl_ptr as *mut _));
        Self::from_parts_unchecked(vtbl, data)
    }
    pub unsafe fn slice_from_parts_unchecked<'a, U>(vtbl: DynMetadataPtr, data: &'a [U]) -> Self
    where
        &'a [U]: Into<T>,
    {
        Self::from_parts_unchecked(vtbl, data.into())
    }
    #[inline]
    pub unsafe fn into_iter_unchecked<'a>(self) -> impl Iterator<Item = &'a V>
    where
        T: IntoIterator,
        <T as IntoIterator>::Item: DynTargetContainer<V>,
        V: 'a,
    {
        Self::check_ptr();
        let Self { data, vtbl, .. } = self;
        data.into_iter()
            .map(move |p| unsafe { Self::ptr_to_ref_unchecked(p.as_target_ptr(), vtbl) })
    }
}
impl<'a, V, T> DynSliceOf<V, T>
where
    V: ?Sized,
    T: AsRef<[DynTargetPtr]>,
{
    pub fn slice_ptrs(&self) -> &[DynTargetPtr] {
        self.data.as_ref()
    }
}
impl<'a, V, T> DynSliceOf<V, T>
where
    V: ?Sized,
    T: AsRef<[DynTargetPtr]>,
{
    pub fn get(&self) -> &[DynTargetPtr] {
        self.data.as_ref()
    }
}
impl<V, T> Copy for DynSliceOf<V, T>
where
    V: ?Sized,
    T: ?Sized + Copy,
{
}
impl<V, T> Clone for DynSliceOf<V, T>
where
    V: ?Sized,
    T: ?Sized + Clone,
{
    #[inline]
    fn clone(&self) -> Self {
        unsafe { Self::from_parts_unchecked(self.vtbl, self.data.clone()) }
    }
}
unsafe impl<V, T> Sync for DynSliceOf<V, T>
where
    T: ?Sized + Sync,
    V: ?Sized,
    for<'a> &'a V: Send,
{
}
unsafe impl<V, T> Send for DynSliceOf<V, T>
where
    T: ?Sized + Send,
    V: ?Sized,
    for<'a> &'a V: Send,
{
}

/// T *must not* be sized
#[inline(always)]
pub unsafe fn vtable_ptr_of<T: ?Sized>(p: NonNull<T>) -> DynMetadataPtr {
    DynSlice::<T>::check_ptr();
    let [_, vtbl] = transmute_unchecked(p);
    match () {
        #[cfg(todo = "unnecessary")]
        _ => NonNull::new_unchecked(vtbl),
        _ => vtbl,
    }
}

pub unsafe trait DynTargetContainer<V: ?Sized> {
    fn as_target_ptr(&self) -> DynTargetPtr;
    #[cfg(todo)]
    fn target_ptr_stride(&self) -> usize;
    /// ehhhh idk
    #[cfg(todo)]
    fn as_target_ptrs(&self) -> &[impl DynTargetContainer<V>] {
        slice::from_ref(self)
    }
}
unsafe impl<V: ?Sized> DynTargetContainer<V> for DynTargetPtr {
    #[inline(always)]
    fn as_target_ptr(&self) -> DynTargetPtr {
        *self
    }
}
unsafe impl<V: ?Sized, T> DynTargetContainer<V> for &'_ T
where
    T: ?Sized + DynTargetContainer<V>,
{
    #[inline(always)]
    fn as_target_ptr(&self) -> DynTargetPtr {
        DynTargetContainer::<V>::as_target_ptr(*self)
    }
}
