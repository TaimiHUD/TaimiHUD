use {
    crate::im::prelude::*,
    core::{
        ffi::CStr,
        fmt::{self, Write},
        ops,
    },
};

pub const FMT_CSTR: &CStr = c"%s";
pub const FMT_STR: &CStr = c"%.*s";

pub trait UiTextExt<'ui>: ImDrawTextStack<'ui> + ImDrawText {
    #[inline]
    #[must_use]
    fn push_font_token<N>(&mut self, font: N) -> <Self as ImFontStack<'ui, N>>::FontToken
    where
        Self: ImFontStack<'ui, N>,
    {
        ImFontStack::<'ui, N>::push_font(self, font)
    }
    #[inline]
    #[must_use]
    fn push_font_opt<N>(&mut self, font: Option<N>) -> <Self as ImFontStack<'ui, N>>::FontToken
    where
        Self: ImFontStack<'ui, N>,
        <Self as ImFontStack<'ui, N>>::FontToken: Default,
    {
        font.map(|f| ImFontStack::<'ui, N>::push_font(self, f))
            .unwrap_or_default()
    }
    fn with_font<'a, N, R, F>(&'a mut self, font: N, f: F) -> R
    where
        F: FnOnce(&'a mut Self) -> R,
        Self: ImFontStack<'ui, N>,
    {
        let token = font.push_font(self);
        let res = f(self);
        token.token_pop();
        res
    }

    fn text_with_font<N, S>(&mut self, font: N, text: S)
    where
        S: ImStrExt,
        Self: ImFontStack<'ui, N>, //Self: ImDrawItemStack<'ui>,
    {
        self.with_font(font, move |ui| {
            let text = text.im_into_imstr();
            match () {
                #[cfg(todo = "unnecessary")]
                _ => {
                    let mut write = UiTextWrite::new(ui);
                    <dyn ImStr>::write_to(&mut write, &text);
                },
                _ => ui.text_unformatted(text),
            }
        });
    }
    fn display_with_font<N, S>(&mut self, font: N, text: &S)
    where
        S: fmt::Display,
        Self: ImDrawText + ImDrawItemStack<'ui>,
        N: UiFontDyn<'ui, Self>,
    {
        ImDrawTextExt::display_with_font(self, font, text)
    }

    fn wrap_text_with_font<N, S>(&mut self, font: N, text: S)
    where
        S: ImStrExt,
        Self: ImFontStack<'ui, N> + ImDrawText + ImDrawItemStack<'ui>,
        <Self as ImFontStack<'ui, N>>::FontToken: UiTokenMut,
    {
        self.with_font(font, move |ui| {
            let text = text.im_into_imstr();
            let mut write = UiText::new_wrapped(ui);
            <dyn ImStr>::write_to(&mut write, &text);
        });
    }
    fn wrap_display_with_font<N, S>(&mut self, font: N, text: &S)
    where
        S: fmt::Display,
        Self: ImDrawText + ImDrawItemStack<'ui>,
        N: UiFontDyn<'ui, Self>,
    {
        ImDrawTextExt::wrap_display_with_font(self, font, text)
    }
}
impl<'ui, U: ?Sized + ImDrawTextStack<'ui> + ImDrawText> UiTextExt<'ui> for U {}

pub trait ImDrawText: ImDraw {
    fn calc_text_size_dyn(&self, text: &mut dyn ImStr) -> ImSize2;
    fn text_unformatted_dyn(&mut self, text: &mut dyn ImStr);
    fn text_wrapped_dyn(&mut self, text: &mut dyn ImStr);

    fn label_text_dyn(&mut self, label: &mut dyn ImStr, text: &mut dyn ImStr);
    fn text_button_dyn(&mut self, text: &mut dyn ImStr, size: Option<ImSize2>) -> Option<imw::Interacted>;
    fn text_button_small_dyn(&mut self, text: &mut dyn ImStr) -> Option<imw::Interacted>;
    fn checkbox_mut_dyn(&mut self, text: &mut dyn ImStr, state: &mut bool) -> Option<imw::Interacted>;

    fn tooltip_text_dyn(&mut self, text: &mut dyn ImStr);

    #[cfg(todo)]
    #[inline]
    fn tooltip_text_dyn(&mut self, text: &mut dyn ImStr) {
        let _guard = self.begin_tooltip_dyn().into_guard();
        self.text_unformatted_dyn(text)
    }
}
pub trait ImDrawTextStack<'ui> {
    type TextWrapPosToken: UiToken + IntoTokenGuard;
    #[must_use]
    fn text_wrap_pos_push(&mut self, pos: Option<Option<f32>>) -> Self::TextWrapPosToken;
    #[must_use]
    #[cfg(todo)]
    fn text_wrap_pos_push_dyn(&mut self, pos: Option<Option<f32>>) -> UiTokenDyn<'ui>;
    #[cfg(todo)]
    fn text_wrap_pos(&self) -> f32;

    type TextColourToken: UiToken + IntoTokenGuard;
    #[must_use]
    fn text_colour_push(&mut self, colour: ImColour) -> Self::TextColourToken;

    type TextScaleToken: UiToken + IntoTokenGuard;
    #[must_use]
    fn text_scale_push(&mut self, scale: f32) -> Self::TextScaleToken;
}
impl<'ui, U: ?Sized> ImDrawTextStack<'ui> for &'_ mut U
where
    U: ImDrawTextStack<'ui>,
    U::TextWrapPosToken: Into<UiTokenDyn<'ui>>,
    U::TextColourToken: Into<UiTokenDyn<'ui>>,
    U::TextScaleToken: Into<UiTokenDyn<'ui>>,
{
    type TextWrapPosToken = UiTokenDyn<'ui>;
    #[inline(always)]
    fn text_wrap_pos_push(&mut self, pos: Option<Option<f32>>) -> Self::TextWrapPosToken {
        ImDrawTextStack::text_wrap_pos_push(*self, pos).into()
    }
    type TextColourToken = UiTokenDyn<'ui>;
    #[inline(always)]
    fn text_colour_push(&mut self, colour: ImColour) -> Self::TextColourToken {
        ImDrawTextStack::text_colour_push(*self, colour).into()
    }
    type TextScaleToken = UiTokenDyn<'ui>;
    #[inline(always)]
    fn text_scale_push(&mut self, scale: f32) -> Self::TextScaleToken {
        ImDrawTextStack::text_scale_push(*self, scale).into()
    }
}
pub trait ImDrawTextExt: ImDrawText {
    #[inline(always)]
    fn calc_text_size<S: ImStrExt>(&self, mut text: S) -> ImSize2 {
        text.with_imstr_dyn(|text| self.calc_text_size_dyn(text))
    }
    #[inline(always)]
    fn text_wrapped<S: ImStrExt>(&mut self, mut text: S) {
        text.with_imstr_dyn(|text| self.text_wrapped_dyn(text))
    }
    #[inline(always)]
    fn text_unformatted<S: ImStrExt>(&mut self, mut text: S) {
        text.with_imstr_dyn(|text| self.text_unformatted_dyn(text))
    }
    #[inline(always)]
    fn text<S: ImStrExt>(&mut self, text: S) {
        self.text_unformatted(text)
    }
    #[inline(always)]
    fn text_disabled<'ui, S>(&mut self, text: S)
    where
        Self: ImColourContainer<ImColourIndex> + ImDrawTextStack<'ui>,
        S: ImStrExt,
    {
        self.text_unformatted_coloured_index(text, ImColourIndex::TEXT_DISABLED)
    }

    #[inline(always)]
    fn label_text<L, S>(&mut self, mut label: L, mut text: S)
    where
        S: ImStrExt,
        L: ImStrExt,
    {
        label.with_imstr_dyn(|label| text.with_imstr_dyn(|text| self.label_text_dyn(label, text)))
    }
    #[inline(always)]
    fn text_button<S>(&mut self, mut text: S, size: Option<ImSize2>) -> Option<imw::Interacted>
    where
        S: ImStrExt,
    {
        text.with_imstr_dyn(|text| self.text_button_dyn(text, size))
    }
    #[inline(always)]
    fn text_button_small<S>(&mut self, mut text: S) -> Option<imw::Interacted>
    where
        S: ImStrExt,
    {
        text.with_imstr_dyn(|text| self.text_button_small_dyn(text))
    }
    #[inline(always)]
    fn checkbox_mut<S>(&mut self, mut text: S, state: &mut bool) -> Option<imw::Interacted>
    where
        S: ImStrExt,
    {
        text.with_imstr_dyn(|text| self.checkbox_mut_dyn(text, state))
    }
    fn checkbox_flags<T: ?Sized, F, S>(&mut self, text: S, field: &mut T, flag: F) -> bool
    where
        F: Clone + PartialEq<<T as ops::BitAnd<F>>::Output>,
        T: Clone + ops::BitXorAssign<F> + ops::BitAnd<F>,
        S: ImStrExt,
    {
        let state = field.clone() & flag.clone();
        let mut state = flag == state;
        let res = self.checkbox_mut(text, &mut state);
        if let Some(..) = res {
            *field ^= flag
        }
        res.is_some()
    }
    /// imgui-rs compatibility alias
    #[inline(always)]
    fn button<S: ImStrExt>(&mut self, text: S) -> bool {
        imw::Interacted::r#bool(self.text_button(text, None))
    }
    /// imgui-rs compatibility alias
    #[inline(always)]
    fn small_button<S: ImStrExt>(&mut self, text: S) -> bool {
        imw::Interacted::r#bool(self.text_button_small(text))
    }
    #[inline(always)]
    fn checkbox<S: ImStrExt>(&mut self, text: S, state: &mut bool) -> bool {
        imw::Interacted::r#bool(self.checkbox_mut(text, state))
    }
    #[inline(always)]
    fn push_text_wrap_pos_with_pos<'ui>(&mut self, pos: f32) -> Self::TextWrapPosToken
    where
        Self: ImDrawTextStack<'ui>,
    {
        self.text_wrap_pos_push(Some(Some(pos)))
    }
    #[inline(always)]
    fn push_text_wrap_pos<'ui>(&mut self) -> Self::TextWrapPosToken
    where
        Self: ImDrawTextStack<'ui>,
    {
        self.text_wrap_pos_push(Some(None))
    }

    #[inline]
    fn text_unformatted_scaled<'ui, S>(&mut self, text: S, scale: f32)
    where
        Self: ImDrawTextStack<'ui>,
        S: ImStrExt,
    {
        let _token = self.text_scale_push(scale).into_guard();
        self.text_unformatted(text)
    }
    #[inline(always)]
    #[must_use]
    fn text_colour_push_index<'ui, C>(&mut self, colour_id: C) -> Self::TextColourToken
    where
        Self: ImColourContainer<C> + ImDrawTextStack<'ui>,
    {
        let colour = self.lookup_style_colour(colour_id);
        self.text_colour_push(colour)
    }
    #[inline]
    fn text_unformatted_coloured<'ui, S, C>(&mut self, text: S, colour: C)
    where
        Self: ImDrawTextStack<'ui>,
        C: Into<ImColour>,
        S: ImStrExt,
    {
        let _token = self.text_colour_push(colour.into()).into_guard();
        self.text_unformatted(text)
    }
    /// please no, but imgui-rs compat...
    #[inline]
    fn text_colored<'ui, S, C>(&mut self, colour: C, text: S)
    where
        Self: ImDrawTextStack<'ui>,
        C: Into<ImColour>,
        S: ImStrExt,
    {
        self.text_unformatted_coloured(text, colour)
    }
    #[inline]
    fn text_unformatted_coloured_index<'ui, S, C>(&mut self, text: S, colour_id: C)
    where
        Self: ImColourContainer<C> + ImDrawTextStack<'ui>,
        S: ImStrExt,
    {
        let _token = self.text_colour_push_index(colour_id).into_guard();
        self.text_unformatted(text)
    }

    #[inline]
    fn display_with_font<'ui, 'a, N, S>(&'a mut self, mut font: N, text: &S)
    where
        S: fmt::Display,
        N: UiFontDyn<'ui, Self>,
        Self: ImDrawItemStack<'ui>,
    {
        self.display_with_font_dyn(font.into_push_font_dyn(), text)
    }
    #[inline(never)]
    fn display_with_font_dyn<'ui, 'a>(
        &'a mut self,
        font: &mut dyn UiFontDyn<'ui, Self>,
        text: &dyn fmt::Display,
    ) where
        Self: ImDrawItemStack<'ui>,
    {
        let font = font.push_font_dyn_into(self);
        let mut write = UiTextWrite::new(self);
        let _res = match text {
            #[cfg(todo = "unnecessary")]
            _ => write.write_fmt(text),
            _ => write!(write, "{text}"),
        };
        debug_assert!(_res.is_ok());
        font.token_pop();
    }

    #[inline(always)]
    fn wrap_display_with_font<'ui, 'a, N, S>(&'a mut self, mut font: N, text: &S)
    where
        S: fmt::Display,
        N: UiFontDyn<'ui, Self>,
        Self: ImDrawItemStack<'ui>,
    {
        self.wrap_display_with_font_dyn(font.into_push_font_dyn(), text)
    }
    #[inline(never)]
    fn wrap_display_with_font_dyn<'ui, 'a>(
        &'a mut self,
        font: &mut dyn UiFontDyn<'ui, Self>,
        text: &dyn fmt::Display,
    ) where
        Self: ImDrawItemStack<'ui>,
    {
        let font = font.push_font_dyn_into(self);
        let mut write = UiText::new_wrapped(self);
        let _res = match text {
            #[cfg(todo = "unnecessary")]
            _ => write.write_fmt(text),
            _ => write!(write, "{text}"),
        };
        debug_assert!(_res.is_ok());
        font.token_pop();
    }

    #[inline(always)]
    fn tooltip_text<S: ImStrExt>(&mut self, mut text: S) {
        text.with_imstr_dyn(|text| self.tooltip_text_dyn(text))
    }
}
impl<U: ?Sized + ImDrawText> ImDrawTextExt for U {}
