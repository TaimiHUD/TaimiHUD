use {
    super::{prelude::*, sys},
    core::{
        ffi::{c_char, c_int},
        mem::{self, MaybeUninit},
        ptr,
    },
};

/// [imgui::Font::from_raw]
#[cfg(todo)]
pub unsafe fn font_ref_from_ptr<'a>(font: *const sys::ImFont) -> Option<&'a imgui::Font> {
    NonNull::new(font as *mut _).map(|ptr| font_ref_from_nn(ptr))
}
#[cfg(todo)]
pub unsafe fn font_ref_from_nn<'a>(font: NonNull<sys::ImFont>) -> &'a imgui::Font {
    imgui::Font::from_raw(&*font.as_ptr())
}
#[cfg(todo)]
impl<'ui> ImFontStack<'ui, imgui::Font> for Ui<'ui> {
    type FontToken = <Self as ImFontStack<'ui, imgui::FontId>>::FontToken;
    #[inline]
    fn push_font(&mut self, font: imgui::Font) -> Self::FontToken {
        ImFontStack::push_font(self, font.id())
    }
}
#[cfg(todo)]
impl<'ui> ImFontStack<'ui, imgui::Font> for Ui<'ui> {
    type FontToken = <&'static Ui<'ui> as ImFontStack<'ui, imgui::Font>>::FontToken;
    #[inline(always)]
    fn push_font(&mut self, font: imgui::Font) -> Self::FontToken {
        ImFontStack::push_font(&mut &*self, font)
    }
}
impl<'ui> ImFontStack<'ui, f32> for &'_ Ui<'ui> {
    type FontToken = UiTokenDyn<'ui>;
    #[inline(always)]
    fn push_font(&mut self, font_scale: f32) -> Self::FontToken {
        unsafe {
            let size_base = self.with_style(|style| style.FontSizeBase);
            let () = sys::igPushFont_Float(ptr::null_mut(), size_base * font_scale);
            UiTokenFn::new_fn_item(&mut im192_pop_font)
        }
    }
}
impl<'ui> ImFontStack<'ui, f32> for Ui<'ui> {
    type FontToken = UiTokenDyn<'ui>;
    #[inline(always)]
    fn push_font(&mut self, font_scale: f32) -> Self::FontToken {
        ImFontStack::push_font(&mut &*self, font_scale).into()
    }
}
/// XXX: this is a pointer without a lifetime, be careful!
#[cfg(todo)]
impl<'ui> ImFontStack<'ui, imgui::FontId> for &'_ Ui<'ui> {
    type FontToken = imgui::FontStackToken<'ui>;
    #[inline]
    fn push_font(&mut self, font: imgui::FontId) -> Self::FontToken {
        unsafe { self.immortal_ui() }.push_font(font)
    }
}
#[cfg(todo)]
impl<'ui> ImFontStack<'ui, imgui::FontId> for Ui<'ui> {
    type FontToken = <&'static Ui<'ui> as ImFontStack<'ui, imgui::FontId>>::FontToken;
    #[inline(always)]
    fn push_font(&mut self, font: imgui::FontId) -> Self::FontToken {
        ImFontStack::push_font(&mut &*self, font)
    }
}
#[cfg(todo)]
impl<'a, 'ui> UiFontDyn<'ui, &'a Ui<'ui>> for imgui::FontId {
    #[inline]
    fn push_font_dyn_into(&mut self, ui: &mut &'a Ui<'ui>) -> UiTokenDyn<'ui> {
        self.clone().push_font_dyn(ui)
    }
}
#[cfg(todo)]
impl<'a, 'ui> UiFontDyn<'ui, Ui<'ui>> for imgui::FontId {
    #[inline]
    fn push_font_dyn_into(&mut self, ui: &mut Ui<'ui>) -> UiTokenDyn<'ui> {
        self.clone().push_font_dyn(ui)
    }
}
#[cfg(todo)]
impl<'a, 'ui> UiFontDyn<'ui, &'a Ui<'ui>> for imgui::Font {
    #[inline(always)]
    fn push_font_dyn_into(&mut self, ui: &mut &'a Ui<'ui>) -> UiTokenDyn<'ui> {
        UiFontDyn::push_font_dyn_into(&mut self.id(), ui)
    }
    #[cfg(todo)]
    fn into_push_font_dyn(&mut self) -> &mut dyn UiFontDyn<'ui, &'a Ui<'ui>> {
        self.id_mut_lol().into_push_font_dyn()
    }
}
#[cfg(todo)]
impl<'a, 'ui> UiFontDyn<'ui, Ui<'ui>> for imgui::Font {
    #[inline(always)]
    fn push_font_dyn_into(&mut self, ui: &mut Ui<'ui>) -> UiTokenDyn<'ui> {
        UiFontDyn::push_font_dyn_into(&mut self.id(), ui)
    }
}

impl<'ui> ImDrawText for &'_ Ui<'ui> {
    #[inline]
    fn calc_text_size_dyn(&self, text: &mut dyn ImStr) -> ImSize2 {
        let omit_hash_id = false;
        <dyn ImStr>::with_bstr(text, |text| unsafe {
            let ptr = text.as_ptr() as *const c_char;
            let end = ptr.add(text.len());
            let wrap_width = None;
            match () {
                #[cfg(feature = "cimgui-struct-return")]
                _ => self
                    .units()
                    .map(sys::igCalcTextSize(
                        ptr,
                        end,
                        omit_hash_id,
                        wrap_width.unwrap_or(-1.0),
                    ))
                    .to_size()
                    .cast(),
                #[cfg(not(feature = "cimgui-struct-return"))]
                _ => {
                    let mut out = MaybeUninit::<ImSize2>::uninit();
                    let () = sys::igCalcTextSize(
                        out.as_mut_ptr() as *mut sys::ImVec2,
                        ptr,
                        end,
                        omit_hash_id,
                        wrap_width.unwrap_or(-1.0),
                    );
                    out.assume_init()
                },
            }
        })
    }
    #[inline]
    fn text_line_height(&self) -> f32 {
        unsafe {
            sys::igGetTextLineHeight()
        }
    }
    #[inline]
    fn text_unformatted_dyn(&mut self, text: &mut dyn ImStr) {
        <dyn ImStr>::with_bstr(text, |text| unsafe {
            let ptr = text.as_ptr() as *const c_char;
            let end = ptr.add(text.len());
            sys::igTextUnformatted(ptr, end)
        })
    }
    #[inline]
    fn text_wrapped_dyn(&mut self, text: &mut dyn ImStr) {
        <dyn ImStr>::with_cbstr(text, |text| match text {
            Ok(c) => unsafe { sys::igTextWrapped(FMT_CSTR.as_ptr(), c.as_ptr()) },
            Err(text) => unsafe {
                let ptr = text.as_ptr() as *const c_char;
                sys::igTextWrapped(FMT_STR.as_ptr(), text.len() as c_int, ptr)
            },
        })
    }
    #[inline]
    fn label_text_dyn(&mut self, label: &mut dyn ImStr, text: &mut dyn ImStr) {
        <dyn ImStr>::with_cstr(label, |label| unsafe {
            <dyn ImStr>::with_cbstr(text, |text| match text {
                Ok(c) => sys::igLabelText(label.as_ptr(), FMT_CSTR.as_ptr(), c.as_ptr()),
                Err(text) => {
                    let ptr = text.as_ptr() as *const c_char;
                    sys::igLabelText(label.as_ptr(), FMT_STR.as_ptr(), text.len() as c_int, ptr)
                },
            })
        })
    }

    fn text_button_dyn(&mut self, text: &mut dyn ImStr, size: Option<ImSize2>) -> Option<Interacted> {
        Interacted::new(<dyn ImStr>::with_cstr(text, |text| unsafe {
            let ptr = text.as_ptr() as *const c_char;
            let size = ImSpaces(size.unwrap_or(ImSize2::ZERO));
            sys::igButton(ptr, size.into())
        }))
    }
    /// roughly a button with 0.0f vertical [ImStyle::frame_padding] and `ImGuiButtonFlags_AlignTextBaseLine`
    fn text_button_small_dyn(&mut self, text: &mut dyn ImStr) -> Option<Interacted> {
        Interacted::new(<dyn ImStr>::with_cstr(text, move |text| unsafe {
            let ptr = text.as_ptr() as *const c_char;
            sys::igSmallButton(ptr)
        }))
    }
    fn checkbox_mut_dyn(&mut self, text: &mut dyn ImStr, state: &mut bool) -> Option<Interacted> {
        Interacted::new(<dyn ImStr>::with_cstr(text, move |text| unsafe {
            let ptr = text.as_ptr() as *const c_char;
            sys::igCheckbox(ptr, state as *mut bool)
        }))
    }

    //#[cfg(todo)]
    fn tooltip_text_dyn(&mut self, text: &mut dyn ImStr) {
        <dyn ImStr>::with_cbstr(text, |text| match text {
            Ok(c) => unsafe { sys::igSetTooltip(FMT_CSTR.as_ptr(), c.as_ptr()) },
            Err(text) => unsafe {
                let ptr = text.as_ptr() as *const c_char;
                sys::igSetTooltip(FMT_STR.as_ptr(), text.len() as c_int, ptr)
            },
        })
    }
}
impl<'ui> ImDrawText for Ui<'ui> {
    #[inline(always)]
    fn calc_text_size_dyn(&self, text: &mut dyn ImStr) -> ImSize2 {
        ImDrawText::calc_text_size_dyn(&self, text)
    }
    #[inline(always)]
    fn text_line_height(&self) -> f32 {
        ImDrawText::text_line_height(&self)
    }
    #[inline(always)]
    fn text_unformatted_dyn(&mut self, text: &mut dyn ImStr) {
        ImDrawText::text_unformatted_dyn(&mut &*self, text)
    }
    #[inline(always)]
    fn text_wrapped_dyn(&mut self, text: &mut dyn ImStr) {
        ImDrawText::text_wrapped_dyn(&mut &*self, text)
    }
    #[inline(always)]
    fn label_text_dyn(&mut self, label: &mut dyn ImStr, text: &mut dyn ImStr) {
        ImDrawText::label_text_dyn(&mut &*self, label, text)
    }
    #[inline(always)]
    fn text_button_dyn(&mut self, text: &mut dyn ImStr, size: Option<ImSize2>) -> Option<Interacted> {
        ImDrawText::text_button_dyn(&mut &*self, text, size)
    }
    #[inline(always)]
    fn text_button_small_dyn(&mut self, text: &mut dyn ImStr) -> Option<Interacted> {
        ImDrawText::text_button_small_dyn(&mut &*self, text)
    }
    #[inline(always)]
    fn checkbox_mut_dyn(&mut self, text: &mut dyn ImStr, state: &mut bool) -> Option<Interacted> {
        ImDrawText::checkbox_mut_dyn(&mut &*self, text, state)
    }
    #[inline(always)]
    fn tooltip_text_dyn(&mut self, text: &mut dyn ImStr) {
        ImDrawText::tooltip_text_dyn(&mut &*self, text)
    }
}
impl<'ui> ImDrawTextStack<'ui> for &'_ Ui<'ui> {
    type TextWrapPosToken = UiTokenDyn<'ui>;
    fn text_wrap_pos_push(&mut self, pos: Option<Option<f32>>) -> Self::TextWrapPosToken {
        unsafe {
            let pos = match pos {
                None => -1.0,
                Some(None) => 0.0,
                Some(Some(pos)) => pos,
            };
            let () = sys::igPushTextWrapPos(pos);
            UiTokenFn::new_fn_item(&mut im192_text_wrap_pos_pop)
        }
    }
    type TextColourToken = <Self as ImColourStack<'ui, ImColourIndex>>::StyleTokenColour;
    #[inline]
    fn text_colour_push(&mut self, colour: ImColour) -> Self::TextColourToken {
        ImColourStack::push_style_colour(self, StyleColor::Text, colour).into()
    }

    type TextScaleToken = <&'ui Ui<'ui> as ImFontStack<'ui, f32>>::FontToken;
    #[inline]
    fn text_scale_push(&mut self, scale: f32) -> Self::TextScaleToken {
        <&Ui<'ui> as ImFontStack<'ui, f32>>::push_font(&mut *self, scale).into()
    }
}
fn im192_text_wrap_pos_pop() {
    unsafe { sys::igPopTextWrapPos() }
}
fn im192_pop_font() {
    unsafe { sys::igPopFont() }
}
fn im192_pop_style_colour() {
    unsafe { sys::igPopStyleColor(1) }
}
impl<'ui> ImDrawTextStack<'ui> for Ui<'ui> {
    type TextWrapPosToken = UiTokenDyn<'ui>;
    #[inline]
    fn text_wrap_pos_push(&mut self, pos: Option<Option<f32>>) -> Self::TextWrapPosToken {
        ImDrawTextStack::text_wrap_pos_push(&mut &*self, pos).into()
    }
    type TextColourToken = UiTokenDyn<'ui>;
    #[inline]
    fn text_colour_push(&mut self, colour: ImColour) -> Self::TextColourToken {
        ImDrawTextStack::text_colour_push(&mut &*self, colour).into()
    }
    type TextScaleToken = UiTokenDyn<'ui>;
    #[inline]
    fn text_scale_push(&mut self, scale: f32) -> Self::TextScaleToken {
        ImDrawTextStack::text_scale_push(&mut &*self, scale).into()
    }
}
pub trait ImDrawText192: ImDrawText {
    #[cfg(todo)]
    fn scratch_txt_dyn(&self, text: &dyn fmt::Display) -> &CStr;
    #[inline(always)]
    #[cfg(todo)]
    fn scratch_txt(&self, text: impl fmt::Display) -> &CStr {
        self.scratch_txt_dyn(&text)
    }
}
pub struct StyleColor(sys::ImGuiCol);
#[allow(non_upper_case_globals)]
impl StyleColor {
    pub const Text: Self = Self(sys::ImGuiCol_Text as _);
    pub const NavCursor: Self = Self(sys::ImGuiCol_NavCursor as _);
}
impl<'ui> ImDrawText192 for Ui<'ui> {}
impl<'ui> ImColourStack<'ui, StyleColor> for &'_ Ui<'ui> {
    type StyleTokenColour = UiTokenDyn<'ui>;
    #[inline]
    fn push_style_colour(&mut self, colour_id: StyleColor, colour: ImColour) -> Self::StyleTokenColour {
        unsafe {
            let colour = mint::Vector4::from(colour).into();
            let () = sys::igPushStyleColor_Vec4(colour_id.0 as sys::ImGuiCol, colour);
            UiTokenFn::new_fn_item(&mut im192_pop_style_colour)
        }
    }
}
impl<'ui> ImColourStack<'ui, ImColourIndex> for &'_ Ui<'ui> {
    #[cfg(todo)]
    type StyleTokenColour = imgui::ColorStackToken<'ui>;
    type StyleTokenColour = UiTokenDyn<'ui>;
    #[inline]
    fn push_style_colour(&mut self, colour_id: ImColourIndex, colour: ImColour) -> Self::StyleTokenColour {
        ImColourStack::push_style_colour(self, StyleColor::from(colour_id), colour).into()
    }
}
impl<'ui> ImColourStack<'ui, StyleColor> for Ui<'ui> {
    type StyleTokenColour = <&'static Ui<'ui> as ImColourStack<'ui, StyleColor>>::StyleTokenColour;
    #[inline(always)]
    fn push_style_colour(&mut self, colour_id: StyleColor, colour: ImColour) -> Self::StyleTokenColour {
        ImColourStack::push_style_colour(&mut &*self, colour_id, colour)
    }
}
impl<'ui> ImColourStack<'ui, ImColourIndex> for Ui<'ui> {
    type StyleTokenColour = <&'static Ui<'ui> as ImColourStack<'ui, ImColourIndex>>::StyleTokenColour;
    #[inline(always)]
    fn push_style_colour(&mut self, colour_id: ImColourIndex, colour: ImColour) -> Self::StyleTokenColour {
        ImColourStack::push_style_colour(&mut &*self, colour_id, colour)
    }
}
impl<'ui> ImColourContainer<StyleColor> for Ui<'ui> {
    #[cfg(todo = "unnecessary")]
    fn lookup_style_colour(&self, colour_id: StyleColor) -> ImColour {
        Ui::style_color(colour_id)
    }
    fn lookup_style_colour(&self, colour_id: StyleColor) -> ImColour {
        self.with_style(|s| s.lookup_style_colour(colour_id))
    }
}
impl ImColourContainer<StyleColor> for sys::ImGuiStyle {
    #[inline(always)]
    fn lookup_style_colour(&self, colour_id: StyleColor) -> ImColour {
        self.Colors
            .get(colour_id.0 as usize)
            .map(|c| ImColour::from(ImSpaces(*c)))
            .unwrap_or(ImColourIndex::V4_FALLBACK)
    }
}
#[cfg(todo)]
impl ImColourContainer<StyleColor> for imgui::Style {
    #[inline(always)]
    fn lookup_style_colour(&self, colour_id: StyleColor) -> ImColour {
        self[colour_id].into()
    }
}
impl<'ui> ImColourContainer<ImColourIndex> for Ui<'ui> {
    #[inline]
    fn lookup_style_colour(&self, colour_id: ImColourIndex) -> ImColour {
        self.lookup_style_colour(StyleColor::from(colour_id))
    }
}
impl From<ImColourIndex> for StyleColor {
    #[inline]
    fn from(colour_id: ImColourIndex) -> StyleColor {
        let idx = colour_id.index();
        #[cfg(debug_assertions)]
        assert!(
            match (colour_id, idx as sys::ImGuiCol) {
                (ImColourIndex::Text, idx) => idx == sys::ImGuiCol_Text as sys::ImGuiCol,
                //&& idx == StyleColor::Text as sys::ImGuiCol
                (ImColourIndex::TEXT_DISABLED, idx) => idx == sys::ImGuiCol_TextDisabled as sys::ImGuiCol,
                //&& idx == StyleColor::TextDisabled as sys::ImGuiCol
                (ImColourIndex::Button, idx) => idx == sys::ImGuiCol_Button as sys::ImGuiCol,
                //&& idx == StyleColor::Button as sys::ImGuiCol
                (ImColourIndex::ButtonHovered, idx) => idx == sys::ImGuiCol_ButtonHovered as sys::ImGuiCol,
                //&& idx == StyleColor::ButtonHovered as sys::ImGuiCol
                (ImColourIndex::PlotHistogram, idx) => idx == sys::ImGuiCol_PlotHistogram as sys::ImGuiCol,
                //&& idx == StyleColor::PlotHistogram as sys::ImGuiCol
                (ImColourIndex::NavCursor, idx) => idx == sys::ImGuiCol_NavCursor as sys::ImGuiCol, //&& idx == StyleColor::NavCursor as sys::ImGuiCol
            },
            "colour_id mismatch: {colour_id:?}"
        );
        unsafe { mem::transmute(idx as sys::ImGuiCol) }
    }
}

/// why is this one manual idgi...
#[cfg(todo)]
impl UiToken for imgui::TextWrapPosStackToken {
    #[inline]
    fn token_pop(self) {
        let is_empty = match () {
            #[cfg(debug_assertions)]
            _ => unsafe {
                let ptr: *mut imgui::Context = mem::transmute(ptr::read(&self));
                ptr.is_null()
            },
            #[cfg(not(debug_assertions))]
            _ => false,
        };
        match is_empty {
            #[cfg(todo)]
            true => return,
            #[cfg(todo)]
            _ => self.pop(&*ptr::dangling()),
            _ => unsafe {
                let mut token = mem::ManuallyDrop::new(self);
                token.token_pop_mut_unchecked()
            },
        }
    }
    #[inline(always)]
    unsafe fn token_pop_mut_unchecked(&mut self) {
        sys::igPopTextWrapPos()
    }

    #[inline(always)]
    fn token_impls_guard() -> bool {
        false
    }
    type TokenGuardType
        = ImGuard<Self>
    where
        Self: Sized;
    #[inline(always)]
    fn into_guard(self) -> Self::TokenGuardType
    where
        Self: Sized,
    {
        ImGuard::new(self)
    }
}
/// did someone forget about it or what???
#[cfg(todo)]
impl UiTokenGuard for imgui::TextWrapPosStackToken {}
/// not a ZST, bleh...
/// lifetimes aren't real but they can hurt you
#[cfg(todo)]
impl<'ui> From<imgui::TextWrapPosStackToken> for UiTokenDyn<'ui> {
    fn from(token: imgui::TextWrapPosStackToken) -> Self {
        mem::forget(token);
        unsafe { UiTokenFn::new_fn_item(&mut im192_text_wrap_pos_pop) }
    }
}
