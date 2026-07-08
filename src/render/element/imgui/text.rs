use taimi_ui::im::{
    im180::prelude::*,
    text::{ImFontStack, UiFontDyn},
    token::UiTokenDyn,
};
#[cfg(feature = "extension-nexus")]
use {
    crate::exports::runtime as rt,
    core::ptr::{self, NonNull},
    taimi_ui::im::im180::text::font_ref_from_nn,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NexusLinkFont {
    Big,
    Ui,
    Font,
}
#[cfg(feature = "extension-nexus")]
impl NexusLinkFont {
    pub fn ptr_from_nexus_link(self, nl: &nexus::data_link::NexusLink) -> *mut imgui::sys::ImFont {
        let font = match self {
            Self::Big => nl.font_big,
            Self::Ui => nl.font_ui,
            Self::Font => nl.font,
        };
        font as *mut _ as *mut imgui::sys::ImFont
    }
    pub unsafe fn read_ptr_from_nexus_link(
        self,
        nl: *const nexus::data_link::NexusLink,
    ) -> *mut imgui::sys::ImFont {
        ptr::read(match self {
            Self::Big => &raw const (*nl).font_big,
            Self::Ui => &raw const (*nl).font_ui,
            Self::Font => &raw const (*nl).font,
        } as *const *mut imgui::sys::ImFont)
    }
    pub fn font_ptr(self) -> Option<NonNull<imgui::sys::ImFont>> {
        rt::nexus_link_ptr()
            .ok()
            .and_then(|nl| unsafe { NonNull::new(self.read_ptr_from_nexus_link(nl.as_ptr())) })
    }
    pub fn read_font(self) -> Option<&'static imgui::Font> {
        self.font_ptr().map(|ptr| unsafe { font_ref_from_nn(ptr) })
    }
    pub fn read_font_id(self) -> Option<imgui::FontId> {
        self.read_font().map(|font| font.id())
    }
}
impl std::str::FromStr for NexusLinkFont {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "big" => Self::Big,
            "ui" => Self::Ui,
            "font" => Self::Font,
            _ => return Err(()),
        })
    }
}
impl crate::render::TextFont {
    pub fn to_nexus(&self) -> Option<NexusLinkFont> {
        Some(match self {
            Self::Fontless => return None,
            Self::Font => NexusLinkFont::Font,
            Self::Big => NexusLinkFont::Big,
            Self::Ui => NexusLinkFont::Ui,
        })
    }
}
impl<'ui> ImFontStack<'ui, NexusLinkFont> for Ui<'ui> {
    type FontToken = <&'static Ui<'ui> as ImFontStack<'ui, NexusLinkFont>>::FontToken;
    #[inline(always)]
    fn push_font(&mut self, font: NexusLinkFont) -> Self::FontToken {
        ImFontStack::push_font(&mut &*self, font)
    }
}
impl<'ui> ImFontStack<'ui, NexusLinkFont> for &'_ Ui<'ui> {
    #[cfg(todo)]
    type FontToken = Option<<Ui<'ui> as ImFontStack<'ui, imgui::FontId>>::FontToken>;
    type FontToken = Option<UiTokenDyn<'ui>>;
    #[cfg(feature = "extension-nexus")]
    fn push_font(&mut self, font: NexusLinkFont) -> Self::FontToken {
        font.read_font_id()
            .map(|font| ImFontStack::push_font(self, font))
            .map(Into::into)
    }
    #[cfg(not(feature = "extension-nexus"))]
    fn push_font(&mut self, _font: NexusLinkFont) -> Self::FontToken {
        None
    }
}
#[cfg(taimi_imgui = "192")]
impl<'ui> ImFontStack<'ui, NexusLinkFont> for taimi_ui::im::im192::Ui<'ui> {
    type FontToken = Option<UiTokenDyn<'ui>>;
    #[inline(always)]
    fn push_font(&mut self, font: NexusLinkFont) -> Self::FontToken {
        None
    }
}
#[cfg(todo)]
impl<'a, 'ui> UiFontDyn<'ui, &'a Ui<'ui>> for NexusLinkFont {
    #[inline]
    fn push_font_dyn_into(&mut self, ui: &mut &'a Ui<'ui>) -> UiTokenDyn<'ui> {
        (*self).push_font_dyn(ui)
    }
}
#[cfg(todo)]
impl<'ui> UiFontDyn<'ui, Ui<'ui>> for NexusLinkFont {
    #[inline]
    fn push_font_dyn_into(&mut self, ui: &mut Ui<'ui>) -> UiTokenDyn<'ui> {
        (*self).push_font_dyn(ui)
    }
}
