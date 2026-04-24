use {
    self::text::UiTextExt as _,
    core::{fmt, mem, ptr},
    imgui::Style,
};

#[allow(unused_imports)]
pub use crate::exports::runtime::imgui::{
    self,
    internal::{RawCast, RawWrapper},
    Ui,
};

pub mod text;
pub mod prelude {
    #![allow(unused_imports)]
    pub use {
        super::{
            imgui::{self, ChildWindow, ComboBox, Condition, TreeNode, TreeNodeFlags, MouseButton, Selectable, Slider, StyleVar, Window, WindowFlags},
            text::{NexusLinkFont, UiFont, UiText, UiTextExt as _, UiTextWrite},
            AsUi,
            Ui,
            UiToken,
        },
        crate::with_i18n,
    };
}

pub trait AsUi<'ui> {
    fn ui(&self) -> &Ui<'ui>;
    unsafe fn immortal_ui<'a>(&'a self) -> &'ui Ui<'ui> {
        mem::transmute(self.ui())
    }
    fn push_token_font<N>(&self, font: N) -> UiTokenDyn<'ui>
    where
        N: text::UiFont<'ui>,
        N::FontToken: Into<UiTokenDyn<'ui>>,
    {
        self.ui().push_font_token(font).into()
    }

    /// be careful, since style vars can be pushed and that probably ruins everything!
    #[inline]
    fn with_style<R, F: FnOnce(&Style) -> R>(&self, f: F) -> R {
        debug_assert!(unsafe {
            imgui::sys::igGetIO() as *const imgui::sys::ImGuiIO == self.ui().io().raw()
        });
        let style = unsafe {
            Style::from_raw(&*imgui::sys::igGetStyle())
        };
        f(style)
    }
    #[inline]
    #[cfg(todo = "unnecessary")]
    fn with_io<R, F: FnOnce(&imgui::Io) -> R>(&self, f: F) -> R {
        unsafe {
            let io = imgui::sys::igGetIO() as *const imgui::sys::ImGuiIO;
            debug_assert_eq!(io, self.ui().io().raw());
            let io = imgui::Io::from_raw(&*io);
            f(io)
        }
    }
    #[inline]
    unsafe fn with_io_mut<R, F: FnOnce(&mut imgui::Io) -> R>(&self, f: F) -> R {
        let io = imgui::sys::igGetIO();
        debug_assert_eq!(io as *const imgui::sys::ImGuiIO, self.ui().io().raw());
        let io = imgui::Io::from_raw_mut(&mut *io);
        f(io)
    }

    fn is_cursor_inline(&self) -> Option<f32> {
        let ui = self.ui();
        let [x, _] = ui.cursor_pos();
        let [start_x, _] = ui.cursor_start_pos();
        #[cfg(todo)]
        let start_x = {
            let [min_x, _] = ui.window_content_region_min();
            start_x.max(min_x)
        };
        ((x - start_x).abs() > 2e-1).then_some(x)
    }
    fn reserve_line_checkbox(&self, label: &str) -> bool {
        let ui = self.ui();
        let inline = self.is_cursor_inline();
        let prior_edge = match inline {
            #[cfg(todo)]
            Some(x) => x,
            _ => {
                let [x, _] = ui.item_rect_max();
                let [startx, _] = ui.window_pos();
                x - startx
            },
        };
        let is_inline = inline.is_some();
        let (box_w, [spacing_w, _]) = self.with_style(|style| (style.indent_spacing, style.item_spacing));
        let [text_w, _] = ui.calc_text_size(label);
        let [max_x, _] = ui.content_region_max();
        let threshold = box_w + spacing_w * 2.0;
        if (max_x - text_w - threshold) > prior_edge {
            let is_inline = false;
            if !is_inline {
                ui.same_line();
            }
            true
        } else {
            false
        }
    }
}
impl<'ui> AsUi<'ui> for Ui<'ui> {
    #[inline(always)]
    fn ui(&self) -> &Ui<'ui> {
        self
    }
}
impl<'ui, U: ?Sized + AsUi<'ui>> AsUi<'ui> for &'_ U {
    #[inline(always)]
    fn ui(&self) -> &Ui<'ui> {
        AsUi::ui(*self)
    }
}

pub trait UiToken {
    #[inline]
    #[cfg(todo = "unused")]
    fn token_empty(&self) -> bool {
        false
    }
    #[inline]
    unsafe fn token_pop_mut_unchecked(&mut self);

    fn token_pop(self)
    where
        Self: Sized;
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
        drop(self)
    }
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
        if let Some(token) = self.token.take() {
            unsafe { token.token_pop_mut_unchecked() }
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
        unsafe { Self::materialize::<T>() }
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
impl<'ui> From<()> for UiTokenDyn<'ui> {
    #[inline]
    fn from(token: ()) -> Self {
        Self::new(token)
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
        unsafe { self.token.token_pop_mut_unchecked() }
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
    unsafe fn materialize_dyn<'a>() -> &'a mut dyn UiToken
    where
        Self: 'a,
    {
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
    fn token_pop(self) {
        self.pop()
    }
    #[inline]
    unsafe fn token_pop_mut_unchecked(&mut self) {
        ptr::drop_in_place(self)
    }
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
