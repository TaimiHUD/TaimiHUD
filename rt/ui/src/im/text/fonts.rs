use crate::im::prelude::*;

pub trait ImFontStack<'ui, F>: ImUiWindow {
    type FontToken: UiToken + 'ui;
    #[must_use]
    fn push_font(&mut self, font: F) -> Self::FontToken;
}
impl<'ui, U: ?Sized + ImUiWindow> ImFontStack<'ui, ()> for U {
    type FontToken = ();
    #[inline]
    fn push_font(&mut self, _: ()) -> Self::FontToken {}
}
#[cfg(todo)]
impl<'ui, U: ?Sized, F, T> ImFontStack<'ui, Option<F>> for U
where
    U: ImFontStack<'ui, F, FontToken = Option<T>>,
    Option<T>: UiToken + 'ui,
{
    type FontToken = Option<T>;
    #[inline]
    fn push_font(&mut self, font: Option<F>) -> Self::FontToken {
        font.and_then(|font| ImFontStack::<'ui, F>::push_font(self, font))
    }
}
pub trait UiFontExt<'ui, U: ?Sized + ImFontStack<'ui, Self>>: Sized {
    #[must_use]
    fn push_font(self, ui: &mut U) -> U::FontToken;
    #[inline]
    #[must_use]
    fn push_font_dyn(self, ui: &mut U) -> UiTokenDyn<'ui>
    where
        U::FontToken: Into<UiTokenDyn<'ui>>,
    {
        self.push_font(ui).into()
    }
}
impl<'ui, U: ?Sized, F> UiFontExt<'ui, U> for F
where
    U: ImFontStack<'ui, F>,
{
    fn push_font(self, ui: &mut U) -> U::FontToken {
        ui.push_font(self)
    }
}
pub trait UiFontDyn<'ui, U: ?Sized> {
    #[must_use]
    fn push_font_dyn_into(&mut self, ui: &mut U) -> UiTokenDyn<'ui>;

    #[inline(always)]
    fn into_push_font_dyn(&mut self) -> &mut dyn UiFontDyn<'ui, U>
    where
        Self: Sized,
    {
        self
    }
}
impl<'ui, U: ?Sized> UiFontDyn<'ui, U> for () {
    #[inline]
    fn push_font_dyn_into(&mut self, _: &mut U) -> UiTokenDyn<'ui> {
        UiTokenDyn::empty()
    }
}
#[cfg(todo)]
impl<U, F> UiFontDyn<U> for F
where
    U: ?Sized + ImFontStack<F>,
    for<'ui> U::FontToken<'ui>: Into<UiTokenDyn<'ui>>,
    F: Clone,
{
    #[cfg(todo)]
    #[inline(always)]
    fn push_font_into_dyn<'ui>(self, ui: &mut U) -> UiTokenDyn<'ui>
    where
        Self: Sized,
    {
        UiFontExt::push_font_dyn(self, ui)
    }
    fn push_font_dyn_into<'ui>(&mut self, ui: &mut U) -> UiTokenDyn<'ui> {
        UiFontExt::push_font_dyn(self.clone(), ui)
    }
}
impl<'ui, U, F> UiFontDyn<'ui, U> for &'_ mut F
where
    U: ?Sized,
    F: UiFontDyn<'ui, U>,
{
    #[inline]
    fn push_font_dyn_into(&mut self, ui: &mut U) -> UiTokenDyn<'ui> {
        UiFontDyn::<U>::push_font_dyn_into(*self, ui)
    }

    /// TODO: `F: ?Sized` and specialize this conversion for Sized? :<
    #[inline(always)]
    fn into_push_font_dyn(&mut self) -> &mut dyn UiFontDyn<'ui, U>
    where
        Self: Sized,
    {
        //UiFontDyn::<U>::into_push_font_dyn(*self)
        *self
    }
}
