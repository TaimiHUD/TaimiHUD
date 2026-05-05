use {
    super::{prelude::*, sys},
    core::{
        ffi::{c_char, c_int},
        mem::{self, MaybeUninit},
        ptr::NonNull,
    },
};

#[cfg(feature = "imgui180-rs")]
use super::{Font, FontId, StyleColor};

#[derive(Debug, Copy, Clone)]
#[repr(transparent)]
#[cfg(not(feature = "imgui180-rs"))]
pub struct FontId {
    font: *mut sys::ImFont,
    #[cfg(todo)]
    _borrow: PhantomData<&'ui sys::ImFont>,
}
#[cfg(not(feature = "imgui180-rs"))]
impl FontId {
    #[inline(always)]
    pub const unsafe fn new_unchecked(font: *mut sys::ImFont) -> Self {
        Self { font }
    }
    #[inline]
    pub unsafe fn from_sys(font: &sys::ImFont) -> Self {
        Self::new_unchecked(font as *const _ as *mut _)
    }
    #[inline]
    pub fn id(&self) -> usize {
        self.font as usize
    }
    #[inline]
    pub fn raw(&self) -> *mut sys::ImFont {
        self.font
    }
}

/// [Font::from_raw]
#[cfg(feature = "imgui180-rs")]
pub unsafe fn font_ref_from_ptr<'a>(font: *const sys::ImFont) -> Option<&'a Font> {
    NonNull::new(font as *mut _).map(|ptr| font_ref_from_nn(ptr))
}
#[cfg(feature = "imgui180-rs")]
pub unsafe fn font_ref_from_nn<'a>(font: NonNull<sys::ImFont>) -> &'a Font {
    Font::from_raw(&*font.as_ptr())
}
pub unsafe fn font_ptr_from_id<'a>(font: FontId) -> Option<NonNull<sys::ImFont>> {
    unsafe { mem::transmute(font) }
}
#[cfg(feature = "imgui180-rs")]
impl<'ui> ImFontStack<'ui, Font> for &'_ Ui<'ui> {
    type FontToken = <Self as ImFontStack<'ui, FontId>>::FontToken;
    #[inline]
    fn push_font(&mut self, font: Font) -> Self::FontToken {
        ImFontStack::push_font(self, font.id())
    }
}
#[cfg(feature = "imgui180-rs")]
impl<'ui> ImFontStack<'ui, Font> for Ui<'ui> {
    type FontToken = <&'static Ui<'ui> as ImFontStack<'ui, Font>>::FontToken;
    #[inline(always)]
    fn push_font(&mut self, font: Font) -> Self::FontToken {
        ImFontStack::push_font(&mut &*self, font)
    }
}
/// XXX: this is a pointer without a lifetime, be careful!
#[cfg(todo)]
impl<'ui> ImFontStack<'ui, FontId> for &'_ Ui<'ui> {
    type FontToken = imgui::FontStackToken<'ui>;
    #[inline]
    fn push_font(&mut self, font: FontId) -> Self::FontToken {
        unsafe { self.immortal_ui() }.push_font(font)
    }
}
impl<'ui> ImFontStack<'ui, FontId> for &'_ Ui<'ui> {
    type FontToken = UiTokenDyn<'ui>;
    fn push_font(&mut self, font: FontId) -> Self::FontToken {
        unsafe {
            font_ptr_from_id(font)
                .map(|font| {
                    let () = sys::igPushFont(font.as_ptr());
                    UiTokenFn::new_fn_item(&mut im180_end_font)
                })
                .unwrap_or_else(|| UiTokenDyn::empty())
        }
    }
}
impl<'ui> ImFontStack<'ui, FontId> for Ui<'ui> {
    type FontToken = <&'static Ui<'ui> as ImFontStack<'ui, FontId>>::FontToken;
    #[inline(always)]
    fn push_font(&mut self, font: FontId) -> Self::FontToken {
        ImFontStack::push_font(&mut &*self, font)
    }
}
impl<'a, 'ui> UiFontDyn<'ui, &'a Ui<'ui>> for FontId {
    #[inline]
    fn push_font_dyn_into(&mut self, ui: &mut &'a Ui<'ui>) -> UiTokenDyn<'ui> {
        self.clone().push_font_dyn(ui)
    }
}
impl<'a, 'ui> UiFontDyn<'ui, Ui<'ui>> for FontId {
    #[inline]
    fn push_font_dyn_into(&mut self, ui: &mut Ui<'ui>) -> UiTokenDyn<'ui> {
        self.clone().push_font_dyn(ui)
    }
}
#[cfg(feature = "imgui180-rs")]
impl<'a, 'ui> UiFontDyn<'ui, &'a Ui<'ui>> for Font {
    #[inline(always)]
    fn push_font_dyn_into(&mut self, ui: &mut &'a Ui<'ui>) -> UiTokenDyn<'ui> {
        UiFontDyn::push_font_dyn_into(&mut self.id(), ui)
    }
    #[cfg(todo)]
    fn into_push_font_dyn(&mut self) -> &mut dyn UiFontDyn<'ui, &'a Ui<'ui>> {
        self.id_mut_lol().into_push_font_dyn()
    }
}
#[cfg(feature = "imgui180-rs")]
impl<'a, 'ui> UiFontDyn<'ui, Ui<'ui>> for Font {
    #[inline(always)]
    fn push_font_dyn_into(&mut self, ui: &mut Ui<'ui>) -> UiTokenDyn<'ui> {
        UiFontDyn::push_font_dyn_into(&mut self.id(), ui)
    }
}

#[cfg(todo = "unnecessary")]
impl<'ui> ImDrawText for &'_ Ui<'ui> {
    #[inline]
    fn calc_text_size(&self, text: &str) -> ImSize2 {
        Ui::calc_text_size(*self, text)
    }
    #[inline]
    fn text(&mut self, text: &str) {
        Ui::text(*self, text)
    }
    fn text_wrapped(&mut self, text: &str) {
        Ui::text_wrapped(*self, text)
    }
}
impl<'ui> ImDrawText for &'_ Ui<'ui> {
    #[inline]
    fn calc_text_size_dyn(&self, text: &mut dyn ImStr) -> ImSize2 {
        let omit_hash_id = false;
        <dyn ImStr>::with_bstr(text, |text| unsafe {
            let mut out = MaybeUninit::<ImSize2>::uninit();
            let ptr = text.as_ptr() as *const c_char;
            let end = ptr.add(text.len());
            let wrap_width = None;
            let () = sys::igCalcTextSize(
                out.as_mut_ptr() as *mut sys::ImVec2,
                ptr,
                end,
                omit_hash_id,
                wrap_width.unwrap_or(-1.0),
            );
            out.assume_init()
        })
    }
    #[inline]
    fn text_line_height(&self) -> f32 {
        unsafe { sys::igGetTextLineHeight() }
    }
    #[cfg(todo)]
    fn text_unformatted_dyn(&mut self, text: &mut dyn ImStr) {
        <dyn ImStr>::display_to(text, &mut |text| unsafe {
            let ptr = text.as_ptr() as *const c_char;
            let end = ptr.add(text.len());
            sys::igTextUnformatted(ptr, end)
        })
    }
    #[inline]
    fn text_unformatted_dyn(&mut self, text: &mut dyn ImStr) {
        <dyn ImStr>::with_bstr(text, |text| unsafe {
            let ptr = text.as_ptr() as *const c_char;
            let end = ptr.add(text.len());
            sys::igTextUnformatted(ptr, end)
        })
    }
    #[cfg(todo = "unnecessary")]
    fn text_wrapped(&mut self, text: &str) {
        unsafe { sys::igTextWrapped(FMT_CSTR.as_ptr(), self.scratch_txt(text).as_ptr()) }
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
    #[cfg(todo)]
    fn text_wrapped(&mut self, text: &str) {
        let _token = self.text_wrap_pos_push(Some(None));
        self.text_unformatted(text);
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
            sys::igPushTextWrapPos(pos);
            // TODO: UiTokenZst::materialize_push()
            #[cfg(todo)]
            return {
                mem::transmute::<*mut imgui::Context, imgui::TextWrapPosStackToken>(
                    // getting context ptr sucks but it's never even used so...
                    ptr::dangling_mut(),
                )
            };
            UiTokenFn::new_fn_item(&mut im180_text_wrap_pos_pop)
        }
    }
    type TextColourToken = <Self as ImColourStack<'ui, ImColourIndex>>::StyleTokenColour;
    #[inline]
    fn text_colour_push(&mut self, colour: ImColour) -> Self::TextColourToken {
        ImColourStack::push_style_colour(self, StyleColor::Text, colour).into()
    }

    type TextScaleToken = UiTokenDyn<'ui>;
    #[cfg(todo)]
    type TextScaleToken = Box<ImGuard<dyn UiTokenDrop>>;
    #[inline]
    fn text_scale_push(&mut self, scale: f32) -> Self::TextScaleToken {
        let prev = super::im180_font_scale_window(unsafe { &Ui::materialize() });
        let () = unsafe { sys::igSetWindowFontScale(scale) };

        let token = UiTokenFn::new(move || unsafe { sys::igSetWindowFontScale(prev) });
        token.into()
    }
}
fn im180_text_wrap_pos_pop() {
    unsafe { sys::igPopTextWrapPos() }
}
fn im180_end_font() {
    unsafe { sys::igPopFont() }
}
fn im180_end_style_colour() {
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
#[cfg(not(feature = "imgui180-rs"))]
pub struct StyleColor(sys::ImGuiCol);
#[cfg(not(feature = "imgui180-rs"))]
#[allow(non_upper_case_globals)]
impl StyleColor {
    pub const Text: Self = Self(sys::ImGuiCol_Text as _);
    pub const NavHighlight: Self = Self(sys::ImGuiCol_NavHighlight as _);
}
pub trait ImDrawText180: ImDrawText {
    #[cfg(todo)]
    fn scratch_txt_dyn(&self, text: &dyn fmt::Display) -> &CStr;
    #[inline(always)]
    #[cfg(todo)]
    fn scratch_txt(&self, text: impl fmt::Display) -> &CStr {
        self.scratch_txt_dyn(&text)
    }
}
impl<'ui> ImDrawText180 for Ui<'ui> {}
impl<'ui> ImColourStack<'ui, StyleColor> for &'_ Ui<'ui> {
    #[cfg(todo)]
    type StyleTokenColour = imgui::ColorStackToken<'ui>;
    type StyleTokenColour = UiTokenDyn<'ui>;
    #[cfg(todo = "unnecessary")]
    fn push_style_colour(&mut self, colour_id: StyleColor, colour: ImColour) -> Self::StyleTokenColour {
        Ui::push_style_color(colour_id, colour.into())
    }
    #[inline]
    fn push_style_colour(&mut self, colour_id: StyleColor, colour: ImColour) -> Self::StyleTokenColour {
        unsafe {
            let colour = mint::Vector4::from(colour).into();
            let idx = match colour_id {
                #[cfg(feature = "imgui180-rs")]
                c => c as sys::ImGuiCol,
                #[cfg(not(feature = "imgui180-rs"))]
                c => c.0,
            };
            let () = sys::igPushStyleColor_Vec4(idx, colour);
            //UiTokenZst::materialize_push()
            UiTokenFn::new_fn_item(&mut im180_end_style_colour)
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
        let idx = match colour_id {
            #[cfg(feature = "imgui180-rs")]
            c => c as usize,
            #[cfg(not(feature = "imgui180-rs"))]
            c => c.0 as usize,
        };
        self.Colors
            .get(idx)
            .map(|c| ImColour::from(ImSpaces(*c)))
            .unwrap_or(ImColourIndex::V4_FALLBACK)
    }
}
#[cfg(feature = "imgui180-rs")]
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
                (ImColourIndex::NavCursor, idx) => idx == sys::ImGuiCol_NavHighlight as sys::ImGuiCol, //&& idx == StyleColor::NavHighlight as sys::ImGuiCol
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
        unsafe { UiTokenFn::new_fn_item(&mut im180_text_wrap_pos_pop) }
    }
}
