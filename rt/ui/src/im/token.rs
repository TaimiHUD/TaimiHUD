use core::{
    fmt,
    marker::PhantomData,
    mem::{self, ManuallyDrop},
    pin::Pin,
};

pub trait UiToken {
    #[inline]
    #[cfg(todo = "unused")]
    fn token_empty(&self) -> bool {
        false
    }
    unsafe fn token_pop_mut_unchecked(&mut self);

    fn token_pop(self)
    where
        Self: Sized;

    fn token_impls_guard() -> bool
    where
        Self: Sized;
}
pub trait IntoTokenGuard {
    /// TODO? `+ UiToken`
    type TokenGuardType: UiTokenGuarded;
    fn into_guard(self) -> Self::TokenGuardType
    where
        Self: Sized;
}
pub trait UiTokenDrop {
    /// TODO: `&mut ManuallyDrop<Self>` requires unstable `feature(arbitrary_self_types_pointers)`
    unsafe fn token_drop_in_place<'a>(self: Pin<&'a mut Self>);
}
impl<'ui> UiTokenDrop for () {
    unsafe fn token_drop_in_place<'a>(self: Pin<&'a mut Self>) {}
}
#[cfg(todo)]
impl<T> UiTokenDrop for T
where
    T: ?Sized + UiToken,
{
    unsafe fn token_drop_in_place<'a>(self: Pin<&'a mut Self>) {
        if token_impls_guard() {
            let this = <dyn UiTokenDrop>::receiver(self);
            ManuallyDrop::drop(this);
        } else {
            <dyn UiTokenDrop>::impl_drop_in_place(self)
        }
    }
}
/// `&'static mut Boxed<T>` is transparently equivalent to `Box<T>`
///
/// this is all so a `Box<T>` can be shoved into a `&mut dyn Trait`
/// that resolves to the impl `Box<T>: Trait`. basically:
/// 1. impl'ing on `Boxed<T>` is compatible with object-safe traits, where the
///     `&Self` receiver can be upcasted to `&T`
/// 1. impl'ing object-safe traits on `Boxed<T>` can receive `&Boxed<T>` without
///     requiring indirection via `&Box<T>` which is not an unsizeable reference
///     despite the global allocator being a ZST :<
///
/// I'm probably dumb and sleepy and missing something, but it works probably
/// (maybe related to DispatchFromDyn?)
#[doc(hidden)]
#[repr(transparent)]
#[must_use]
pub struct Boxed<T: ?Sized>(ManuallyDrop<T>);
impl<T: ?Sized> Boxed<T> {
    /// Box<T> -> *mut T
    #[inline(always)]
    pub fn new(v: Box<T>) -> &'static mut Self where {
        Self::from_box(v)
    }
    /// Box<T> -> *mut T
    #[inline(always)]
    pub fn from_box<'a>(v: Box<T>) -> &'a mut Self where {
        unsafe { mem::transmute(v) }
    }
    /// *mut Box<T> -> *mut *mut T
    #[inline(always)]
    pub fn from_mut<'a>(v: &'a mut Box<T>) -> &'a mut &'static mut Self where {
        unsafe { mem::transmute(v) }
    }
    /// *mut Manual<Box<T>> -> *mut Manual<*mut T>
    #[inline(always)]
    pub fn from_manual_mut<'a>(v: &'a mut ManuallyDrop<Box<T>>) -> &'a mut ManuallyDrop<&'static mut Self> where
    {
        unsafe { mem::transmute(v) }
    }
    #[inline(always)]
    pub fn into_box(&'static mut self) -> Box<T> {
        unsafe { Self::ptr_into_box(mem::transmute(self)) }
    }
    #[inline(always)]
    pub fn into_manual(&'static mut self) -> Box<ManuallyDrop<T>> {
        unsafe { Self::into_manual_unchecked(self) }
    }
    #[inline(always)]
    pub unsafe fn into_manual_unchecked<'a>(&'a mut self) -> Box<ManuallyDrop<T>> {
        unsafe { Self::ptr_into_manual(mem::transmute(self)) }
    }
    /// *mut Manual<*mut T> -> Box<T>
    #[inline(always)]
    pub unsafe fn take_box<'a, 'b>(this: &'a mut ManuallyDrop<&'b mut Self>) -> Box<T> {
        unsafe { Self::ptr_into_box(ManuallyDrop::take(this) as *mut Self) }
    }
    #[inline(always)]
    pub fn leak<'a>(&'a mut self) -> &'a mut T {
        unsafe { mem::transmute(self) }
    }
    #[inline(always)]
    pub fn leak_manual<'a>(&'a mut self) -> &'a mut ManuallyDrop<T> {
        &mut self.0
    }

    #[inline(always)]
    pub unsafe fn ptr_mut(this: *mut Self) -> *mut T {
        this as *mut T
    }
    #[inline(always)]
    pub unsafe fn ptr_into_box(this: *mut Self) -> Box<T> {
        #[cfg(todo)]
        return mem::transmute(self);
        Box::from_raw(Self::ptr_mut(this))
    }
    #[inline(always)]
    pub unsafe fn ptr_into_manual(this: *mut Self) -> Box<ManuallyDrop<T>> {
        unsafe { Boxed::ptr_into_box(this as *mut Boxed<ManuallyDrop<T>>) }
    }

    /// *mut T -> *mut Manual<*mut T>
    #[inline(always)]
    pub unsafe fn manual_mut2<'a, 'b>(this: &'a mut &'b mut Self) -> &'a mut ManuallyDrop<&'b mut Self> {
        mem::transmute(this)
    }
    /// *mut *mut T -> *mut Manual<*mut T>
    #[inline(always)]
    pub unsafe fn manual_box<'a, 'b>(this: &'a mut &'b mut Self) -> &'a mut ManuallyDrop<Box<T>> {
        mem::transmute(this)
    }
    #[inline(always)]
    pub unsafe fn drop_in_place_inner(&mut self) {
        ManuallyDrop::drop(&mut self.0)
    }
    #[inline]
    pub unsafe fn drop_in_place_ptr(this: *mut Self) {
        drop(Self::ptr_into_box(this))
    }
    #[inline]
    pub fn drop_and_free(this: &'static mut Self) {
        drop(Self::into_box(this))
    }
}
/// unreachable by mortals...
impl<T: ?Sized> Drop for Boxed<T> {
    #[inline(always)]
    fn drop(&mut self) {
        unsafe { Self::drop_in_place_ptr(self) }
    }
}
impl<T> UiTokenDrop for Boxed<T>
where
    T: ?Sized + UiTokenDrop,
{
    #[inline]
    unsafe fn token_drop_in_place<'a>(self: Pin<&'a mut Self>) {
        let mut this = ManuallyDrop::new(self.get_unchecked_mut());
        T::token_drop_in_place(<dyn UiTokenDrop>::to_receiver::<T>(this.leak_manual()));
        let this = Boxed::into_manual_unchecked(ManuallyDrop::take(&mut { this }));
        drop(this);
    }
}
impl<T> UiTokenDrop for Box<T>
where
    T: ?Sized + UiTokenDrop,
{
    #[inline(always)]
    unsafe fn token_drop_in_place<'a>(self: Pin<&'a mut Self>) {
        let this = <dyn UiTokenDrop>::receiver(self);
        match () {
            #[cfg(todo)]
            _ => {
                let this = Boxed::from_manual_mut(this);
                <&'static mut Boxed<T> as UiTokenDrop>::token_drop_in_place(<dyn UiTokenDrop>::to_receiver(
                    this,
                ))
            },
            _ => {
                let this = Boxed::from_box(ManuallyDrop::take(this));
                let this = mem::transmute::<&mut Boxed<T>, &mut ManuallyDrop<Boxed<T>>>(this);
                #[cfg(todo)]
                let this = Boxed::from_manual_mut(this);
                <Boxed<T> as UiTokenDrop>::token_drop_in_place(<dyn UiTokenDrop>::to_receiver(this))
            },
        }
    }
}
impl<T: UiTokenDrop> UiTokenDrop for Option<T> {
    #[inline]
    unsafe fn token_drop_in_place<'a>(self: Pin<&'a mut Self>) {
        let this = mem::transmute::<&'a mut ManuallyDrop<Option<T>>, &'a mut Option<ManuallyDrop<T>>>(
            <dyn UiTokenDrop>::receiver(self),
        );
        // don't like how this reborrowed using ref mut pattern matches but w/e
        let Some(this) = this else { return };
        UiTokenDrop::token_drop_in_place(<dyn UiTokenDrop>::to_receiver(this))
    }
}
#[cfg(todo)]
impl<'ui> UiTokenDrop for dyn UiTokenDrop + 'ui {
    unsafe fn token_drop_in_place<'a>(self: Pin<&'a mut Self>) {
        let this = <dyn UiTokenDrop>::receiver(self);
        UiTokenDrop::token_drop_in_place(what)
    }
}
impl<T: ?Sized + UiTokenDrop> UiTokenDrop for &'static mut T {
    unsafe fn token_drop_in_place<'a>(self: Pin<&'a mut Self>) {
        let this = self.map_unchecked_mut(|this| *this);
        UiTokenDrop::token_drop_in_place(this)
    }
}
impl dyn UiTokenDrop {
    pub const EMPTY: &'static mut Self = {
        static mut EMPTY: () = ();
        let empty = unsafe { &mut *(&raw mut EMPTY) };
        empty as &mut dyn UiTokenDrop
    };

    #[inline(always)]
    pub unsafe fn impl_drop_in_place<'a, T>(token: Pin<&'a mut T>)
    where
        T: ?Sized + UiToken + 'a,
    {
        let token = <dyn UiTokenDrop>::receiver(token);
        UiToken::token_pop_mut_unchecked(&mut **token);
        ManuallyDrop::drop(token);
    }
    #[inline(always)]
    pub unsafe fn drop_in_place<'a, T>(token: &'a mut ManuallyDrop<T>)
    where
        T: ?Sized + UiTokenDrop + 'a,
    {
        let token = <dyn UiTokenDrop>::to_receiver(token);
        UiTokenDrop::token_drop_in_place(token)
    }
    #[inline(always)]
    pub unsafe fn to_receiver<'a, T>(token: &'a mut ManuallyDrop<T>) -> Pin<&'a mut T>
    where
        T: ?Sized,
    {
        mem::transmute(token)
    }
    #[inline(always)]
    pub unsafe fn receiver<'a, T>(token: Pin<&'a mut T>) -> &'a mut ManuallyDrop<T>
    where
        T: ?Sized,
    {
        mem::transmute(token)
    }
}
pub trait UiTokenMut: UiToken {
    fn token_pop_mut(&mut self);
}
impl<T: UiToken> UiToken for Option<T> {
    #[inline]
    #[cfg(todo = "unused")]
    fn token_empty(&self) -> bool {
        self.as_ref().map(UiToken::token_empty).unwrap_or(true)
    }
    #[inline]
    fn token_pop(self)
    where
        Self: Sized,
    {
        if let Some(token) = self {
            token.token_pop();
        }
    }
    #[inline]
    unsafe fn token_pop_mut_unchecked(&mut self) {
        self.take().unwrap_unchecked().token_pop()
    }

    #[inline(always)]
    fn token_impls_guard() -> bool {
        T::token_impls_guard()
    }
}
impl<T> IntoTokenGuard for Option<T>
where
    T: IntoTokenGuard,
{
    type TokenGuardType = Option<T::TokenGuardType>;
    #[inline(always)]
    fn into_guard(self) -> Self::TokenGuardType {
        self.map(T::into_guard)
    }
}
impl<T: UiToken> UiToken for Box<T> {
    #[inline]
    fn token_pop(self)
    where
        Self: Sized,
    {
        let inner: T = *self;
        T::token_pop(inner)
    }
    #[inline]
    unsafe fn token_pop_mut_unchecked(&mut self) {
        T::token_pop_mut_unchecked(&mut **self)
    }

    #[inline(always)]
    fn token_impls_guard() -> bool {
        T::token_impls_guard()
    }
}
impl<T> IntoTokenGuard for Box<T>
where
    T: IntoTokenGuard,
{
    type TokenGuardType = T::TokenGuardType;
    #[inline(always)]
    fn into_guard(self) -> Self::TokenGuardType {
        let inner: T = *self;
        let guard = T::into_guard(inner);
        #[cfg(todo)]
        let guard = Box::new(guard);
        guard
    }
}
impl UiToken for Box<dyn UiTokenDrop> {
    #[inline]
    fn token_pop(self)
    where
        Self: Sized,
    {
        let mut token = ManuallyDrop::new(self);
        unsafe { <dyn UiTokenDrop>::drop_in_place(&mut token) }
    }
    #[inline]
    unsafe fn token_pop_mut_unchecked(&mut self) {
        let token = mem::replace(self, Box::new(()) as Box<dyn UiTokenDrop>);
        UiToken::token_pop(token)
    }

    #[inline(always)]
    fn token_impls_guard() -> bool {
        false
    }
}
impl<'ui> IntoTokenGuard for Box<dyn UiTokenDrop + 'ui> {
    type TokenGuardType = Box<ImGuard<dyn UiTokenDrop + 'ui>>;
    #[inline(always)]
    fn into_guard(self) -> Self::TokenGuardType {
        ImGuard::from_box(self)
    }
}
impl UiToken for &'static mut dyn UiTokenDrop {
    #[inline]
    fn token_pop(self) {
        unsafe {
            let token = mem::transmute::<Self, &'static mut ManuallyDrop<dyn UiTokenDrop>>(self);
            UiTokenDrop::token_drop_in_place(<dyn UiTokenDrop>::to_receiver(token))
        }
    }
    #[inline]
    unsafe fn token_pop_mut_unchecked(&mut self) {
        let token = mem::replace(self, <dyn UiTokenDrop>::EMPTY);
        Self::token_pop(token)
    }

    #[inline(always)]
    fn token_impls_guard() -> bool {
        false
    }
}
impl<'ui, 'a> IntoTokenGuard for &'static mut (dyn UiTokenDrop + 'ui) {
    type TokenGuardType = ImGuard<&'static mut (dyn UiTokenDrop + 'ui)>;
    #[inline(always)]
    fn into_guard(self) -> Self::TokenGuardType {
        ImGuard::new(self)
    }
}
#[must_use]
#[repr(transparent)]
pub struct UiTokenCell<'a> {
    token: Option<&'a mut dyn UiToken>,
}
impl<'a> UiTokenCell<'a> {
    pub const EMPTY: Self = Self { token: None };
    #[inline(always)]
    pub unsafe fn with_dyn(token: &'a mut dyn UiToken) -> Self {
        Self::with_token(Some(token))
    }
    #[inline(always)]
    pub unsafe fn with_token(token: Option<&'a mut dyn UiToken>) -> Self {
        Self { token }
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.token.is_none()
    }

    #[inline]
    pub fn token(&self) -> Option<&dyn UiToken> {
        match &self.token {
            Some(token) => Some(&**token),
            None => None,
        }
    }
    #[inline]
    pub unsafe fn token_mut(&mut self) -> &mut Option<&'a mut dyn UiToken> {
        &mut self.token
    }
}
impl<'a> fmt::Debug for UiTokenCell<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("UiTokenCell")
            .field(&self.token.as_ref().map(drop))
            .finish()
    }
}
impl UiToken for UiTokenCell<'_> {
    #[inline]
    #[cfg(todo = "unused")]
    fn token_empty(&self) -> bool {
        self.token().map(UiToken::token_empty).unwrap_or(false)
    }
    #[inline]
    fn token_pop(mut self) {
        self.token_pop_mut();
    }
    #[inline]
    unsafe fn token_pop_mut_unchecked(&mut self) {
        self.token.take().unwrap_unchecked().token_pop_mut_unchecked()
    }

    #[inline(always)]
    fn token_impls_guard() -> bool {
        false
    }
}
#[cfg(todo)]
impl IntoTokenGuard for UiTokenCell<'_> {
    type TokenGuardType = ImGuard<Self>;
    #[inline(always)]
    fn into_guard(self) -> Self::TokenGuardType {
        unsafe { ImGuard::new_unchecked(self) }
    }
}
impl<'ui> UiTokenMut for UiTokenCell<'ui> {
    #[inline]
    fn token_pop_mut(&mut self) {
        if let Some(token) = self.token.take() {
            unsafe { token.token_pop_mut_unchecked() }
        }
    }
}
/// how often do you need to mix token types in a stack though?
/// the most variety you'll ever see is an option...
#[must_use]
#[repr(transparent)]
pub struct UiTokenDyn<'ui> {
    token: &'ui mut dyn UiToken,
}
impl<'ui> UiTokenDyn<'ui> {
    #[inline(always)]
    pub fn empty() -> Self {
        Self::new(())
    }
    #[inline(always)]
    pub fn new<T: UiTokenZst + UiToken + 'ui>(token: T) -> Self {
        debug_assert_eq!(mem::size_of::<T>(), 0);
        mem::forget(token);
        unsafe { Self::materialize::<T>() }
    }
    #[inline(always)]
    pub fn from_static<T: UiToken + 'ui>(token: &'static mut T) -> Self {
        unsafe { Self::with_token(token as &mut dyn UiToken) }
    }
    #[inline(always)]
    /// TODO: store &mut dyn UiTokenDrop instead so ImGuard is irrelevant
    pub fn from_box<T: UiTokenDrop + 'ui>(token: Box<T>) -> Self {
        let token = ImGuard::from_mut(Boxed::from_box(token));
        unsafe { Self::with_token(token) }
    }
    #[inline(always)]
    pub unsafe fn materialize<T: UiTokenZst + 'ui>() -> Self {
        Self::with_token(T::materialize_dyn())
    }
    #[inline(always)]
    pub unsafe fn with_token(token: &'ui mut dyn UiToken) -> Self {
        Self { token }
    }
    #[inline]
    pub fn token(&self) -> &dyn UiToken {
        &*self.token
    }
    #[inline]
    pub unsafe fn token_mut(&mut self) -> &mut &'ui mut dyn UiToken {
        &mut self.token
    }
}
impl<'ui> UiTokenGuarded for UiTokenDyn<'ui> {}
impl<'ui> UiTokenGuard for UiTokenDyn<'ui> {
    type GuardInner = &'ui mut dyn UiToken;
    #[inline(always)]
    fn guard_leak(self) -> Self::GuardInner {
        unsafe { mem::transmute(self) }
    }
}
impl<'ui> From<()> for UiTokenDyn<'ui> {
    #[inline]
    fn from(token: ()) -> Self {
        Self::new(token)
    }
}
impl<'ui, T> From<Option<T>> for UiTokenDyn<'ui>
where
    T: Into<UiTokenDyn<'ui>>,
{
    #[inline]
    fn from(token: Option<T>) -> Self {
        token.map(Into::<Self>::into).unwrap_or(Self::empty())
    }
}
impl<'ui> UiToken for UiTokenDyn<'ui> {
    #[inline]
    #[cfg(todo = "unused")]
    fn token_empty(&self) -> bool {
        self.token().token_empty()
    }
    #[inline]
    fn token_pop(self) {
        drop(self)
    }
    #[inline]
    unsafe fn token_pop_mut_unchecked(&mut self) {
        self.token.token_pop_mut_unchecked()
    }

    #[inline(always)]
    fn token_impls_guard() -> bool {
        true
    }
}
impl<'ui> IntoTokenGuard for UiTokenDyn<'ui> {
    type TokenGuardType = Self;
    #[inline(always)]
    fn into_guard(self) -> Self::TokenGuardType {
        self
    }
}
impl<'ui> UiTokenDrop for UiTokenDyn<'ui> {
    #[inline]
    unsafe fn token_drop_in_place<'a>(self: Pin<&'a mut Self>) {
        let token = <dyn UiTokenDrop>::receiver(self);
        ManuallyDrop::drop(token);
    }
}
impl<'ui> UiTokenMut for UiTokenDyn<'ui> {
    #[inline]
    fn token_pop_mut(&mut self) {
        let prev = mem::replace(self, Self::empty());
        prev.token_pop()
    }
}
impl<'ui> Drop for UiTokenDyn<'ui> {
    fn drop(&mut self) {
        unsafe { self.token.token_pop_mut_unchecked() }
    }
}
impl fmt::Debug for UiTokenDyn<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("UiTokenDyn")
            .field(&(&*self.token as *const dyn UiToken))
            .finish()
    }
}
/// marker trait for a [UiToken] that pops itself on drop
///
/// see also: [UiTokenGuard], [ImGuard]
pub trait UiTokenGuarded {
    #[inline(always)]
    fn end(self)
    where
        Self: Sized,
    {
        drop(self)
    }
}
pub trait UiTokenGuard: Sized + UiTokenGuarded {
    type GuardInner;
    fn guard_leak(self) -> Self::GuardInner;
}
impl<T: UiTokenGuarded> UiTokenGuarded for Option<T> {}
impl<T: UiTokenGuard> UiTokenGuard for Option<T> {
    type GuardInner = Option<T::GuardInner>;
    #[inline(always)]
    fn guard_leak(self) -> Self::GuardInner {
        self.map(T::guard_leak)
    }
}
impl<T: ?Sized + UiTokenGuarded> UiTokenGuarded for Box<T> {}
impl<T: UiTokenGuard> UiTokenGuard for Box<T> {
    type GuardInner = T::GuardInner;
    #[inline(always)]
    fn guard_leak(self) -> Self::GuardInner {
        let inner: T = *self;
        T::guard_leak(inner)
    }
}
#[derive(Debug)]
#[must_use]
#[repr(transparent)]
/// TODO: a ?Sized variant
pub struct ImGuard<T: ?Sized + UiTokenDrop> {
    token: ManuallyDrop<T>,
}
#[cfg(todo)]
impl<T: ?Sized + UiTokenDrop + ?Sized> ImGuard<T> {
    #[inline(always)]
    pub fn unwrap_new(token: T) -> Self where {
        if T::token_impls_guard() {
            panic!("guard token");
        }
        unsafe { Self::new_unchecked(token) }
    }
}
impl<T: ?Sized + UiTokenDrop> ImGuard<T> {
    #[inline(always)]
    pub fn from_mut(token: &mut T) -> &mut Self {
        unsafe { mem::transmute(token) }
    }
    #[inline(always)]
    pub fn from_box(token: Box<T>) -> Box<Self> {
        unsafe { mem::transmute(token) }
    }

    #[inline(always)]
    pub fn token_ref(&self) -> &T {
        &self.token
    }
    #[inline(always)]
    pub fn token_mut(&mut self) -> &mut T {
        &mut self.token
    }
}
impl<T: UiTokenDrop> ImGuard<T> {
    #[inline(always)]
    pub const fn new(token: T) -> Self {
        Self { token: ManuallyDrop::new(token) }
    }
    #[doc(hidden)]
    #[inline(always)]
    pub const unsafe fn new_unchecked(token: T) -> Self {
        Self { token: ManuallyDrop::new(token) }
    }

    #[inline(always)]
    pub fn unpop(self) -> T {
        unsafe {
            match () {
                #[cfg(todo)]
                _ => arcffi::transmute_unchecked(self),
                _ => {
                    let mut guard = ManuallyDrop::new(self);
                    ManuallyDrop::take(&mut guard.token)
                },
            }
        }
    }
}
impl<T: ?Sized + UiTokenDrop> UiTokenGuarded for ImGuard<T> {}
impl<T: UiTokenDrop> UiTokenGuard for ImGuard<T> {
    type GuardInner = T;
    #[inline(always)]
    fn guard_leak(self) -> Self::GuardInner {
        self.unpop()
    }
}
impl<T: UiTokenDrop> UiTokenDrop for ImGuard<T> {
    #[inline]
    unsafe fn token_drop_in_place<'a>(self: Pin<&'a mut Self>) {
        let this = mem::transmute::<Pin<&'a mut ImGuard<T>>, &'a mut ManuallyDrop<T>>(self);
        UiTokenDrop::token_drop_in_place(<dyn UiTokenDrop>::to_receiver::<T>(this))
    }
}
/// TODO: DELETEME
impl<T: ?Sized + UiTokenDrop> UiToken for ImGuard<T> {
    #[inline]
    fn token_pop(self)
    where
        Self: Sized,
    {
        drop(self)
    }
    #[inline]
    unsafe fn token_pop_mut_unchecked(&mut self) {
        T::token_drop_in_place(<dyn UiTokenDrop>::to_receiver(&mut self.token))
    }

    #[inline(always)]
    fn token_impls_guard() -> bool {
        true
    }
}
impl<T: UiTokenDrop + UiToken> IntoTokenGuard for ImGuard<T> {
    type TokenGuardType = Self;
    #[inline(always)]
    fn into_guard(self) -> Self::TokenGuardType {
        self
    }
}
unsafe impl<T: UiTokenDrop + UiTokenZst> UiTokenZst for ImGuard<T> {
    #[inline(always)]
    unsafe fn materialize_mut<'a>() -> &'a mut Self {
        Self::from_mut(T::materialize_mut())
    }
    #[inline(always)]
    unsafe fn materialize_push() -> Self {
        Self::new_unchecked(T::materialize_push())
    }
    #[cfg(todo = "unnecessary")]
    #[inline(always)]
    unsafe fn materialize_mut<'a>() -> &'a mut Self {
        &mut *ptr::dangling_mut()
    }
}
impl<'ui, T> From<ImGuard<T>> for UiTokenDyn<'ui>
where
    T: UiTokenZst + UiTokenDrop + 'ui,
{
    #[inline]
    fn from(token: ImGuard<T>) -> Self {
        Self::new(token)
    }
}
impl<'ui, T: ?Sized + UiTokenDrop> Drop for ImGuard<T> {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            match &mut self.token {
                #[cfg(todo)]
                token => token.token_pop_mut_unchecked(),
                #[cfg(todo)]
                token => ManuallyDrop::take(token).token_pop(),
                token => <dyn UiTokenDrop>::drop_in_place(token),
            }
        }
    }
}

impl<T: UiToken> UiTokenMut for Option<T> {
    #[inline]
    fn token_pop_mut(&mut self) {
        if let Some(token) = self.take() {
            token.token_pop()
        }
    }
}
pub unsafe trait UiTokenZst: UiToken + Sized {
    unsafe fn materialize_mut<'a>() -> &'a mut Self;
    #[inline(always)]
    unsafe fn materialize_dyn<'a>() -> &'a mut dyn UiToken
    where
        Self: 'a,
    {
        Self::materialize_mut() as &mut dyn UiToken
    }
    #[inline(always)]
    unsafe fn materialize_push() -> Self {
        mem::transmute_copy(&*Self::materialize_mut())
    }
}

unsafe impl<'ui> UiTokenZst for () {
    #[inline(always)]
    unsafe fn materialize_mut<'a>() -> &'a mut Self {
        Box::leak(Box::new(()))
    }
    #[cfg(todo = "unnecessary")]
    #[inline(always)]
    unsafe fn materialize_mut<'a>() -> &'a mut Self {
        &mut *ptr::dangling_mut()
    }
}
impl UiToken for () {
    #[inline(always)]
    fn token_pop(self) {}
    #[inline(always)]
    unsafe fn token_pop_mut_unchecked(&mut self) {}
    #[inline(always)]
    fn token_impls_guard() -> bool {
        true
    }
}
impl IntoTokenGuard for () {
    type TokenGuardType = Self;
    #[inline(always)]
    fn into_guard(self) -> Self::TokenGuardType {
        self
    }
}
impl UiTokenGuarded for () {}
impl UiTokenGuard for () {
    type GuardInner = Self;
    #[inline(always)]
    fn guard_leak(self) -> Self::GuardInner {
        self
    }
}

#[must_use]
#[repr(transparent)]
pub struct UiTokenFn<F: ?Sized> {
    _not_sync: PhantomData<*const ()>,
    f: F,
}
impl<F> UiTokenFn<F> {
    #[inline(always)]
    pub const fn from_inner(f: F) -> Self {
        Self { f, _not_sync: PhantomData }
    }
    #[inline(always)]
    pub const fn new(f: F) -> Self
    where
        F: FnMut(),
    {
        Self::from_inner(f)
    }
    #[inline(always)]
    pub fn new_dyn<'ui>(f: F) -> UiTokenDyn<'ui>
    where
        F: FnOnce() + 'ui,
    {
        let token = Box::new(UiTokenFn::new_once(f));
        UiTokenDyn::from_box(token)
    }
    #[inline(always)]
    pub fn into_inner(self) -> F {
        self.f
    }
}
impl UiTokenFn<dyn FnMut()> {
    #[inline(always)]
    pub unsafe fn new_fn_item<'ui, 'a, Z: Copy + Fn() + 'ui>(f: &'a mut Z) -> UiTokenDyn<'ui> {
        debug_assert_eq!(mem::size_of::<Z>(), 0);
        let f = f as *mut Z;
        UiTokenDyn::with_token(UiTokenFn::from_mut(&mut *f))
    }
    /// prefer [UiTokenFn::new] if possible
    #[inline(always)]
    pub const fn new_once<F>(f: F) -> UiTokenFn<impl FnMut()>
    where
        F: FnOnce(),
    {
        let mut f = Some(f);
        #[cfg(todo)]
        let mut f = mem::MaybeUninit::new(f);
        UiTokenFn::new(move || unsafe {
            #[cfg(todo)]
            let f = mem::transmute::<*mut F, &mut Option<F>>(f.as_mut_ptr());
            match f.take() {
                #[cfg(todo = "unnecessary")]
                f =>
                    if let Some(f) = f {
                        f()
                    },
                f => (f.unwrap_unchecked())(),
            }
        })
    }
}
impl<F: ?Sized> UiTokenFn<F> {
    #[inline(always)]
    pub const fn from_ref(f: &F) -> &Self {
        unsafe { mem::transmute(f) }
    }
    #[inline(always)]
    pub fn from_mut(f: &mut F) -> &mut Self {
        unsafe { mem::transmute(f) }
    }
    #[inline(always)]
    pub fn from_box(f: Box<F>) -> Box<Self> {
        unsafe { mem::transmute(f) }
    }
}
impl<F: ?Sized + FnMut()> UiToken for UiTokenFn<F> {
    #[inline]
    fn token_pop(mut self)
    where
        Self: Sized,
    {
        (self.f)();
    }
    #[inline]
    unsafe fn token_pop_mut_unchecked(&mut self) {
        (self.f)();
    }

    #[inline(always)]
    fn token_impls_guard() -> bool {
        false
    }
}
impl<F: FnMut()> IntoTokenGuard for UiTokenFn<F> {
    type TokenGuardType = ImGuard<Self>;
    #[inline(always)]
    fn into_guard(self) -> Self::TokenGuardType {
        unsafe { ImGuard::new_unchecked(self) }
    }
}
impl<'ui, F> From<UiTokenFn<F>> for UiTokenDyn<'ui>
where
    F: FnMut() + 'ui,
{
    #[inline]
    fn from(token: UiTokenFn<F>) -> Self {
        let token = Box::new(token);
        UiTokenDyn::from_box(token)
    }
}
impl<F: ?Sized> UiTokenDrop for UiTokenFn<F>
where
    Self: UiToken,
{
    #[inline]
    unsafe fn token_drop_in_place<'a>(self: Pin<&'a mut Self>) {
        <dyn UiTokenDrop>::impl_drop_in_place(self)
    }
}
#[cfg(todo)]
unsafe impl<F> UiTokenZst for UiTokenFn<F>
where
    Self: UiToken,
    F: Zfn<()>,
{
}
