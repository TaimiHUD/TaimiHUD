use {
    super::prelude::*,
    core::{fmt, mem},
};

pub trait ImColourContainer<C> {
    fn lookup_style_colour(&self, colour_id: C) -> ImColour;
}
impl<'a, U, C> ImColourContainer<C> for &'a U
where
    U: ImColourContainer<C>,
{
    #[inline(always)]
    fn lookup_style_colour(&self, colour_id: C) -> ImColour {
        ImColourContainer::lookup_style_colour(*self, colour_id)
    }
}
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum ImColourIndex {
    Text = 0,
    Disabled = 1,
    Button = 21,
    ButtonHovered = 22,
    PlotHistogram = 41,
    NavCursor = 49,
}
impl ImColourIndex {
    pub const TEXT_DISABLED: Self = Self::Disabled;

    pub const FALLBACK_IDX: Self = Self::Text;

    pub const IDX_MIN: u8 = Self::Text.index();
    pub const IDX_MAX: u8 = Self::Disabled.index();
    pub const IDX_END: u8 = Self::IDX_MAX + 1;
    #[inline(always)]
    pub const fn index(self) -> u8 {
        self as _
    }
    #[inline(always)]
    pub const unsafe fn from_index_unchecked(colour_id: u8) -> Self {
        mem::transmute(colour_id)
    }
    #[inline]
    pub const fn from_index(colour_id: u8) -> Option<Self> {
        match colour_id {
            idx @ Self::IDX_MIN..=Self::IDX_MAX => Some(unsafe { Self::from_index_unchecked(idx) }),
            _ => None,
        }
    }
}
impl ImColourIndex {
    #[inline]
    pub const fn opaque_rgb(r: f32, g: f32, b: f32) -> ImColour {
        ImColour::new(r, g, b, 1.0)
    }
    #[inline]
    pub const fn opaque_rgb8(r: u8, g: u8, b: u8) -> ImColour {
        ImColour::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0)
    }
    pub const V4_BLACK: ImColour = Self::opaque_rgb(0.0, 0.0, 0.0);
    pub const V4_WHITE: ImColour = ImColour::ONE;
    pub const V4_FALLBACK: ImColour = ImColour::new(0.25, 7.0 / 8.0, 0.8, 0.85);
}
impl fmt::Display for ImColourIndex {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.index(), f)
    }
}
pub trait ImColourStack<'ui, C>: ImColourContainer<C> {
    type StyleTokenColour: UiToken + 'ui;
    #[must_use]
    fn push_style_colour(&mut self, colour_id: C, colour: ImColour) -> Self::StyleTokenColour;
}
pub trait ImColourStackExt<'ui, C>: ImColourStack<'ui, C> + ImColourContainer<C> {
    #[inline]
    fn push_style_colour_index(&mut self, dest: C, src: C) -> Self::StyleTokenColour {
        let colour = self.lookup_style_colour(src);
        self.push_style_colour(dest, colour)
    }
    #[inline]
    fn push_colour<T: Into<ImColour>>(&mut self, dest: C, colour: T) -> Self::StyleTokenColour where {
        self.push_style_colour(dest, colour.into())
    }
}
impl<'ui, U, C> ImColourStackExt<'ui, C> for U where U: ?Sized + ImColourStack<'ui, C> + ImColourContainer<C>
{}

/// TODO: switch to u32 if imgui ever follows through with that plan?
pub type ImColour = glam::Vec4;

impl ImSpaces<ImColour> {
    pub fn to_colour32(self) -> u32 {
        let c32 = (self.0 * 255.0).as_u8vec4().to_array();
        u32::from_le_bytes(c32)
    }
}
impl From<ImSpaces<ImColour>> for u32 {
    #[inline(always)]
    fn from(v: ImSpaces<ImColour>) -> Self {
        v.to_colour32()
    }
}
