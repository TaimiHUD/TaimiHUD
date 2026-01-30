use core::fmt;
use core::ptr;
use core::mem;
use self::text::UiTextExt as _;
#[allow(unused_imports)]
pub use crate::exports::runtime::imgui::{self, internal::{RawCast, RawWrapper}, Ui};

pub mod text;
pub mod prelude {
    #![allow(unused_imports)]
    pub use super::{
        text::{UiText, UiTextWrite, UiFont, UiTextExt as _, NexusLinkFont},
        imgui::{self, MouseButton}, Ui, AsUi, UiToken,
    };
    pub use crate::with_i18n;
}

pub trait AsUi<'ui> {
    fn ui(&self) -> &Ui<'ui>;
    unsafe fn immortal_ui<'a>(&'a self) -> &'ui Ui<'ui> {
        mem::transmute(self.ui())
    }
    fn push_token_font<N>(&self, font: N) -> UiTokenDyn<'ui> where
        N: text::UiFont<'ui>,
        N::FontToken: Into<UiTokenDyn<'ui>>,
    {
        self.ui().push_font_token(font).into()
    }
}
impl<'ui> AsUi<'ui> for Ui<'ui> {
    #[inline(always)]
    fn ui(&self) -> &Ui<'ui> { self }
}
impl<'ui, U: ?Sized + AsUi<'ui>> AsUi<'ui> for &'_ U {
    #[inline(always)]
    fn ui(&self) -> &Ui<'ui> { AsUi::ui(*self) }
}

pub trait UiToken {
    #[inline]
    #[cfg(todo = "unused")]
    fn token_empty(&self) -> bool { false }
    #[inline]
    unsafe fn token_pop_mut_unchecked(&mut self);

    fn token_pop(self) where Self: Sized;
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
    fn token_pop(self) where Self: Sized { drop(self) }
    #[inline]
    unsafe fn token_pop_mut_unchecked(&mut self) {
        self.take().unwrap_unchecked().token_pop()
    }
}
#[repr(transparent)]
pub struct UiTokenCell<'a> {
    token: Option<&'a mut dyn UiToken>,
}
impl<'a> UiTokenCell<'a> {
    pub const EMPTY: Self = Self {
        token: None,
    };
    #[inline(always)]
    pub unsafe fn with_dyn(token: &'a mut dyn UiToken) -> Self {
        Self::with_token(Some(token))
    }
    #[inline(always)]
    pub unsafe fn with_token(token: Option<&'a mut dyn UiToken>) -> Self {
        Self { token }
    }
    #[inline]
    pub fn is_empty(&self) -> bool { self.token.is_none() }

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
        if let Some(token) = self.token.take() {
            unsafe {
                token.token_pop_mut_unchecked()
            }
        }
    }
    #[inline]
    unsafe fn token_pop_mut_unchecked(&mut self) {
        self.token.take().unwrap_unchecked().token_pop_mut_unchecked()
    }
}
/// how often do you need to mix token types in a stack though?
/// the most variety you'll ever see is an option...
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
        unsafe {
            Self::materialize::<T>()
        }
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
impl<'ui, T: UiTokenZst + UiToken + 'ui> From<Option<T>> for UiTokenDyn<'ui> {
    #[inline]
    fn from(token: Option<T>) -> Self {
        token.map(Self::new).unwrap_or(Self::empty())
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
}
impl<'ui> Drop for UiTokenDyn<'ui> {
    fn drop(&mut self) {
        unsafe {
            self.token.token_pop_mut_unchecked()
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
unsafe trait UiTokenZst: UiToken + Sized {
    unsafe fn materialize_mut<'a>() -> &'a mut Self;
    #[inline(always)]
    unsafe fn materialize_dyn<'a>() -> &'a mut dyn UiToken where Self: 'a {
        Self::materialize_mut() as &mut dyn UiToken
    }
}
impl UiToken for imgui::FontStackToken<'_> {
    #[inline]
    #[cfg(todo = "unused")]
    fn token_empty(&self) -> bool {
        false
    }
    #[inline]
    fn token_pop(self) { self.pop() }
    #[inline]
    unsafe fn token_pop_mut_unchecked(&mut self) { ptr::drop_in_place(self) }
}
unsafe impl<'ui> UiTokenZst for imgui::FontStackToken<'ui> {
    #[inline(always)]
    unsafe fn materialize_mut<'a>() -> &'a mut Self {
        &mut *ptr::dangling_mut()
    }
}
impl<'ui> From<imgui::FontStackToken<'ui>> for UiTokenDyn<'ui> {
    #[inline]
    fn from(token: imgui::FontStackToken<'ui>) -> Self {
        Self::new(token)
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
}
