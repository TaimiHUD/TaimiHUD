use {
    crate::ui::{MapContext, LocalContext},
    core::ops,
    taimi_hoard::flags::{
        set::{BitFlagForSet, FlagSet, BitsOrder},
        BitSlice, BitVec, BitView,
    },
    bitflags::bitflags,
};

bitflags! {
    #[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct VisibilityFlags: u8 {
        const TOGGLE = 0x01;
        const TOGGLE_SPACE = 0x02;
        const TOGGLE_MINIMAP = 0x04;
        const TOGGLE_GLOBAL = 0x08;

        const DEFAULT_TOGGLE = 0x10;
        const DEFAULT_SPACE = 0x20;
        const DEFAULT_MINIMAP = 0x40;
        const DEFAULT_GLOBAL = 0x80;
    }
}

impl VisibilityFlags {
    pub const TOGGLE_COUNT: usize = 4;

    pub const DEFAULTS: Self = Self::from_bits_retain(
        Self::DEFAULT_TOGGLE.bits() | Self::DEFAULT_SPACE.bits() | Self::DEFAULT_GLOBAL.bits() | Self::DEFAULT_MINIMAP.bits()
    );
    pub const TOGGLES: Self = Self::from_bits_retain(
        Self::TOGGLE.bits() | Self::TOGGLE_SPACE.bits() | Self::TOGGLE_GLOBAL.bits() | Self::TOGGLE_MINIMAP.bits()
    );

    pub const fn visible(visible: bool) -> Self {
        match visible {
            true => Self::TOGGLE,
            false => Self::empty(),
        }
    }

    pub const fn and_as_defaults(self) -> Self {
        Self::from_bits_retain(self.bits() | self.toggles_to_default().bits())
    }

    pub fn restore_default_toggles(mut self) -> VisibilityFlags {
        self.set_toggles(self.default_toggles());
        self
    }

    /// Get [Self::DEFAULTS] shifted to [Self::TOGGLES]
    pub const fn default_toggles(self) -> VisibilityFlags {
        Self::from_bits_retain((self.bits() & Self::DEFAULTS.bits()) >> 4)
    }
    pub const fn toggles_to_default(self) -> VisibilityFlags {
        Self::from_bits_retain((self.bits() & Self::TOGGLES.bits()) << 4)
    }

    pub fn set_toggles(&mut self, visible: VisibilityFlags) {
        self.remove(Self::TOGGLES);
        self.insert(visible & Self::TOGGLES);
    }
    pub fn set_defaults(&mut self, visible: VisibilityFlags) {
        self.remove(Self::DEFAULTS);
        self.insert(visible & Self::DEFAULTS);
    }

    pub fn toggle_for_context(ctx: LocalContext) -> VisibilityFlags {
        match ctx {
            LocalContext::World => Self::TOGGLE_SPACE,
            LocalContext::Map(map) => Self::toggle_for_map(map),
        }
    }
    pub fn toggle_for_map(map: MapContext) -> VisibilityFlags {
        match map {
            MapContext::Minimap => Self::TOGGLE_MINIMAP,
            MapContext::Global => Self::TOGGLE_GLOBAL,
        }
    }
    pub fn default_for_context(ctx: LocalContext) -> VisibilityFlags {
        match ctx {
            LocalContext::World => Self::DEFAULT_SPACE,
            LocalContext::Map(map) => Self::default_for_map(map),
        }
    }
    pub fn default_for_map(map: MapContext) -> VisibilityFlags {
        match map {
            MapContext::Minimap => Self::DEFAULT_MINIMAP,
            MapContext::Global => Self::DEFAULT_GLOBAL,
        }
    }
    pub fn is_visible(&self) -> bool {
        self.contains(Self::TOGGLE)
    }
    pub fn is_visible_for_space(&self) -> bool {
        self.contains(Self::TOGGLE | Self::TOGGLE_SPACE)
    }
    pub fn is_visible_for_map(&self, map: MapContext) -> bool {
        self.contains(Self::TOGGLE | VisibilityFlags::toggle_for_map(map))
    }
}
#[cfg(deleteme)]
impl VisibilityFlags {
    pub fn from_category_flags(cat_flags: CategoryFlags) -> Self {
        let mut flags = Self::empty();
        flags.set_from_category_flags(cat_flags);
        flags
    }
    pub fn set_from_category_flags(&mut self, cat_flags: CategoryFlags) {
        self.set(Self::TOGGLE, !cat_flags.contains(CategoryFlags::DISABLED));
    }
    pub fn from_pack_category(category: &Category) -> Self {
        let mut flags = Self::from_attributes(&category.marker_attributes);
        flags.set_from_category_flags(category.flags);
        flags
    }
    /// TODO: if [PackRoot] survives, give it a [CategoryFlags] field
    pub fn from_pack_root(_root: &PackRoot) -> Self {
        Self::TOGGLES
    }
    pub fn from_attributes(marker_attributes: &MarkerAttributes) -> Self {
        let mut flags = Self::empty();
        flags.set_from_attributes(marker_attributes);
        flags
    }
    pub fn set_from_attributes(&mut self, marker_attributes: &MarkerAttributes) {
        if let Some(value) = marker_attributes.in_game_visibility {
            self.set(Self::TOGGLE_SPACE, value);
        }
        if let Some(value) = marker_attributes.map_visibility {
            self.set(Self::TOGGLE_GLOBAL, value);
        }
        if let Some(value) = marker_attributes.minimap_visibility {
            self.set(Self::TOGGLE_MINIMAP, value);
        }
    }
    pub fn set_defaults_from_attributes(&mut self, marker_attributes: &MarkerAttributes) {
        if let Some(value) = marker_attributes.in_game_visibility {
            self.set(Self::DEFAULT_SPACE, value);
        }
        if let Some(value) = marker_attributes.map_visibility {
            self.set(Self::DEFAULT_GLOBAL, value);
        }
        if let Some(value) = marker_attributes.minimap_visibility {
            self.set(Self::DEFAULT_MINIMAP, value);
        }
    }
}

impl From<bool> for VisibilityFlags {
    fn from(visible: bool) -> Self {
        Self::visible(visible).and_as_defaults()
    }
}
impl From<VisibilityFlags> for bool {
    fn from(flags: VisibilityFlags) -> Self {
        flags.contains(VisibilityFlags::TOGGLE)
    }
}

pub type VisibilityFlagSet<V = BitVec<u8>> = FlagSet<VisibilityFlags, V>;
impl BitFlagForSet for VisibilityFlags {
    type Repr = u8;
    const BIT_WIDTH: usize = 4;

    fn as_bits(&self) -> &Self::Repr {
        unsafe {
            &*(self as *const Self as *const u8)
        }
    }
    fn as_bits_mut(&mut self) -> &mut Self::Repr {
        unsafe {
            &mut *(self as *mut Self as *mut u8)
        }
    }
    fn as_bitslice(&self) -> &BitSlice<Self::Repr, BitsOrder> {
        unsafe {
            self.as_bits().view_bits().get_unchecked(..Self::BIT_WIDTH)
        }
    }
    fn as_bitslice_mut(&mut self) -> &mut BitSlice<Self::Repr, BitsOrder> {
        unsafe {
            self.as_bits_mut().view_bits_mut().get_unchecked_mut(..Self::BIT_WIDTH)
        }
    }

    fn range_for(index: usize) -> ops::Range<usize> {
        let start = index << 2;
        let end = start + Self::BIT_WIDTH;
        start..end
    }
}
