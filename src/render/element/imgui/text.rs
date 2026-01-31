use crate::exports::runtime as rt;
use super::{imgui, Ui, AsUi, RawCast, UiToken, UiTokenMut, UiTokenDyn};
use glamour::{Point2, Size2};
use core::fmt::{self, Write};
use std::io;
use core::borrow::BorrowMut;
use core::ptr::{self, NonNull};
use core::mem;

pub trait UiTextExt<'ui>: AsUi<'ui> {
    #[inline]
    fn push_font_token<N: UiFont<'ui>>(&self, font: N) -> N::FontToken {
        font.push_font(self.ui())
    }
    fn with_font<'a, N, R, F>(&'a self, font: N, f: F) -> R where
        F: FnOnce(&'a Self) -> R,
        N: UiFont<'ui>,
        N::FontToken: UiToken,
    {
        let token = font.push_font(self.ui());
        let res = f(self);
        token.token_pop();
        res
    }

    fn text_with_font<'a, N, S>(&'a self, font: N, text: S) where
        S: AsRef<str>,
        N: UiFont<'ui>,
        N::FontToken: UiToken,
    {
        let text = text.as_ref();
        self.with_font(font, move |ui| {
            let mut write = UiTextWrite::new(ui.ui());
            write.append(text);
        });
    }
    fn display_with_font<'a, N, S>(&'a self, font: &N, text: &S) where
        S: fmt::Display,
        N: UiFontDyn<'ui>,
    {
        self.display_with_font_dyn(font, text)
    }
    fn display_with_font_dyn(&self, font: &dyn UiFontDyn<'ui>, text: &dyn fmt::Display) {
        let ui = self.ui();
        let font = font.push_font_dyn(ui);
        let mut write = UiTextWrite::new(ui);
        let _res = match text {
            #[cfg(todo = "unnecessary")]
            _ => write.write_fmt(text),
            _ => write!(write, "{text}"),
        };
        debug_assert!(_res.is_ok());
        font.token_pop();
    }

    fn wrap_text_with_font<'a, N, S>(&'a self, font: N, text: S) where
        S: AsRef<str>,
        N: UiFont<'ui>,
        N::FontToken: UiTokenMut,
    {
        let text = text.as_ref();
        self.with_font(font, move |ui| {
            let mut write = UiText::new_wrapped(ui.ui());
            write.append(text);
        });
    }
    fn wrap_display_with_font<'a, N, S>(&'a self, font: &N, text: &S) where
        S: fmt::Display,
        N: UiFontDyn<'ui>,
    {
        self.wrap_display_with_font_dyn(font, text)
    }
    fn wrap_display_with_font_dyn<'a>(&'a self, font: &dyn UiFontDyn<'ui>, text: &dyn fmt::Display) {
        let ui = self.ui();
        let font = font.push_font_dyn(ui);
        let mut write = UiText::new_wrapped(ui);
        let _res = match text {
            #[cfg(todo = "unnecessary")]
            _ => write.write_fmt(text),
            _ => write!(write, "{text}"),
        };
        debug_assert!(_res.is_ok());
        font.token_pop();
    }
}
impl<'ui> UiTextExt<'ui> for Ui<'ui> {}

#[cfg(deleteme)]
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
#[cfg(deleteme)]
impl<'ui> From<imgui::FontStackToken<'ui>> for UiTokenDyn<'ui> {
    #[inline]
    fn from(token: imgui::FontStackToken<'ui>) -> Self {
        Self::new(token)
    }
}
#[cfg(deleteme)]
unsafe impl<'ui> UiTokenZst for imgui::FontStackToken<'ui> {
    #[inline(always)]
    unsafe fn materialize_mut<'a>() -> &'a mut Self {
        &mut *ptr::dangling_mut()
    }
}

pub trait UiFont<'ui> {
    type FontToken: UiToken + 'ui;
    fn push_font(self, ui: &Ui<'ui>) -> Self::FontToken;

}
pub trait UiFontDyn<'ui> {
    fn push_font_dyn(&self, ui: &Ui<'ui>) -> UiTokenDyn<'ui>;
}
impl<'ui, T: UiFont<'ui>> UiFontDyn<'ui> for T where
    T::FontToken: Into<UiTokenDyn<'ui>>,
    T: Clone,
{
    fn push_font_dyn(&self, ui: &Ui<'ui>) -> UiTokenDyn<'ui> {
        self.clone().push_font(ui).into()
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NexusLinkFont {
    Big,
    Ui,
    Font,
}
#[cfg(feature = "extension-nexus")]
impl NexusLinkFont {
    pub fn ptr_from_nexus_link(self, nl: &nexus::data_link::NexusLink) -> *mut imgui::sys::ImFont {
        match self {
            Self::Big => nl.font_big,
            Self::Ui => nl.font_ui,
            Self::Font => nl.font,
        }
    }
    pub unsafe fn read_ptr_from_nexus_link(self, nl: *const nexus::data_link::NexusLink) -> *mut imgui::sys::ImFont {
        match self {
            Self::Big => ptr::read(&raw const (*nl).font_big),
            Self::Ui => ptr::read(&raw const (*nl).font_ui),
            Self::Font => ptr::read(&raw const (*nl).font),
        }
    }
    pub fn font_ptr(self) -> Option<NonNull<imgui::sys::ImFont>> {
        rt::nexus_link_ptr().ok().and_then(|nl| unsafe {
            NonNull::new(self.read_ptr_from_nexus_link(nl.as_ptr()))
        })
    }
    pub fn read_font(self) -> Option<&'static imgui::Font> {
        self.font_ptr().map(|ptr| unsafe { font_ref_from_nn(ptr) })
    }
    pub fn read_font_id(self) -> Option<imgui::FontId> {
        self.read_font().map(|font| font.id())
    }
}
/// [imgui::Font::from_raw]
pub unsafe fn font_ref_from_ptr<'a>(font: *const imgui::sys::ImFont) -> Option<&'a imgui::Font> {
    NonNull::new(font as *mut _).map(|ptr| font_ref_from_nn(ptr))
}
pub unsafe fn font_ref_from_nn<'a>(font: NonNull<imgui::sys::ImFont>) -> &'a imgui::Font {
    imgui::Font::from_raw(&*font.as_ptr())
}
impl<'ui> UiFont<'ui> for NexusLinkFont {
    type FontToken = Option<<imgui::FontId as UiFont<'ui>>::FontToken>;
    #[cfg(feature = "extension-nexus")]
    fn push_font(self, ui: &Ui<'ui>) -> Self::FontToken {
        self.read_font_id().map(|font| font.push_font(ui))
    }
    #[cfg(not(feature = "extension-nexus"))]
    fn push_font(self, _ui: &Ui<'ui>) -> Self::FontToken { None }
}
impl<'ui> UiFont<'ui> for &'ui imgui::Font {
    type FontToken = <imgui::FontId as UiFont<'ui>>::FontToken;
    #[inline]
    fn push_font(self, ui: &Ui<'ui>) -> Self::FontToken {
        self.id().push_font(ui)
    }
}
/// XXX: this is a pointer without a lifetime, be careful!
impl<'ui> UiFont<'ui> for imgui::FontId {
    type FontToken = imgui::FontStackToken<'ui>;
    #[inline]
    fn push_font(self, ui: &Ui<'ui>) -> Self::FontToken {
        unsafe {
            ui.immortal_ui()
        }.push_font(self)
    }
}
impl<'ui> UiFont<'ui> for () {
    type FontToken = ();
    #[inline]
    fn push_font(self, _: &Ui<'ui>) -> Self::FontToken {
        ()
    }
}

#[derive(Debug, Copy, Clone)]
pub struct UiTextWrite<'a, 'ui> {
    pub ui: &'a Ui<'ui>,
    pub start_of_line: Option<Point2<f32>>,
}
impl<'a, 'ui> UiTextWrite<'a, 'ui> {
    #[inline(always)]
    pub const fn new(ui: &'a Ui<'ui>) -> Self {
        Self {
            ui,
            start_of_line: None,
        }
    }
    #[inline]
    pub fn append(&mut self, s: &str) {
        if s.is_empty() {
            return
        }
        let mut _spacingtoken = None;
        if let Some(next_start) = self.start_of_line.take() {
            match next_start {
                s if s.x.is_infinite() => {
                    _spacingtoken = Some(self.ui.push_style_var(imgui::StyleVar::ItemSpacing([f32::EPSILON, 0.0])));
                    self.ui.same_line();
                },
                s =>
                    self.ui.set_cursor_pos(s.to_array()),
            }
        }
        let plain = s.strip_suffix("\n");
        let endln = plain.is_some();
        let can_resume = !endln && !s.as_bytes().contains(&b'\n');
        let prev_start = can_resume.then(|| Point2::from_array(self.ui.cursor_pos()));
        self.ui.text(plain.unwrap_or(s));
        if let Some(prev_start) = prev_start {
            self.start_of_line = Some(prev_start + Size2::from_array(self.ui.item_rect_size()).with_height(0.0).to_vector());
        } else if !endln {
            self.start_of_line = Some(Point2::INFINITY);
        }
    }
    pub fn end_line(&mut self) {
        match self.start_of_line {
            None => self.ui.new_line(),
            Some(..) => self.start_of_line = None,
        }
    }
    #[inline]
    pub fn draw_line(&mut self, s: &str) {
        self.ui.text(s);
        self.start_of_line = None;
    }
}
impl<'a, 'ui> fmt::Write for UiTextWrite<'a, 'ui> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        Ok(self.append(s))
    }
}
#[derive(Debug, Clone)]
pub struct UiText<'a, 'ui, B: BorrowMut<String> = String> {
    pub text: UiTextWrite<'a, 'ui>,
    pub wrap: bool,
    pub buffer: B,
}
impl<'a, 'ui> UiText<'a, 'ui> {
    #[inline]
    pub fn new_wrapped<U>(ui: &'a U) -> Self where
        U: AsUi<'ui>,
    {
        Self::new(ui, true)
    }
}
impl<'a, 'ui, B: BorrowMut<String>> UiText<'a, 'ui, B> {
    #[inline]
    pub fn new<U>(ui: &'a U, wrap: bool) -> Self where
        U: AsUi<'ui>,
        B: Default,
    {
        Self {
            text: UiTextWrite::new(ui.ui()),
            wrap,
            buffer: B::default(),
        }
    }
    #[inline]
    pub fn append_buffer<S: AsRef<str>>(&mut self, s: S) {
        self.buffer.borrow_mut().push_str(s.as_ref());
    }
    #[inline]
    pub fn append(&mut self, s: &str) {
        if s.is_empty() {
            return
        }
        let buffer = self.buffer.borrow_mut();
        let (text, rest) = match self.wrap {
            true => s.rsplit_once("\n").unzip(),
            false => {
                if !buffer.is_empty() {
                    self.text.append(&buffer[..]);
                    buffer.clear();
                    self.text.append(&s);
                }
                (None, Some(""))
            },
        };
        if let Some(mut text) = text {
            if !buffer.is_empty() {
                buffer.push_str(text);
                text = &buffer[..];
            }
            self.text.ui.text_wrapped(text);
            buffer.clear();
        }
        let rest = rest.unwrap_or(s);
        buffer.push_str(rest);
    }
    pub fn display<D: fmt::Display>(&mut self, s: D) {
        let _res = write!(self, "{s}");
        debug_assert!(_res.is_ok());
    }
    pub fn end_line(&mut self) {
        self.flush_buffer();
        self.text.end_line();
    }
    pub fn flush_buffer(&mut self) {
        self.draw_buffer();
        self.buffer.borrow_mut().clear();
    }
    pub fn draw_buffer(&mut self) {
        let buffer = self.buffer.borrow_mut();
        if !buffer.is_empty() {
            match self.wrap {
                true => self.text.ui.text_wrapped(&buffer[..]),
                false => self.text.append(&buffer[..]),
            }
        }
    }
}
impl<'a, 'ui, B: BorrowMut<String>> fmt::Write for UiText<'a, 'ui, B> {
    #[inline]
    fn write_str(&mut self, s: &str) -> fmt::Result {
        Ok(self.append(s))
    }
}
impl<'a, 'ui, B: BorrowMut<String>> io::Write for UiText<'a, 'ui, B> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let s = match buf {
            #[cfg(debug_assertions)]
            buf => str::from_utf8(buf).unwrap(),
            #[cfg(not(debug_assertions))]
            buf => unsafe { str::from_utf8_unchecked(buf) },
        };
        self.append(s);
        Ok(s.len())
    }
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        Ok(self.flush_buffer())
    }
}
impl<'a, 'ui, B: BorrowMut<String>> Drop for UiText<'a, 'ui, B> {
    fn drop(&mut self) {
        self.draw_buffer();
    }
}
