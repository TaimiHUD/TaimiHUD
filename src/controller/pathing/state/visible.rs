use {
    crate::controller::pathing::registry::PackRoot,
    taimi_pack::{
        attributes::MarkerAttributes,
        category::{Category, CategoryFlags},
    },
};
pub use {
    crate::controller::pathing::state::{
        LoadedCategory,
        LoadedMapPack,
        LoadedPoi,
        LoadedTrail,
        LoadedTrailGeometry,
        LoadedTrailSection,
    },
    taimi_meta::packs::VisibilityFlags,
};

pub trait VisibilityFlagsExt: Sized {
    fn from_category_flags(cat_flags: CategoryFlags) -> Self;
    fn set_from_category_flags(&mut self, cat_flags: CategoryFlags);
    fn from_pack_category(category: &Category) -> Self;
    fn from_pack_root(_root: &PackRoot) -> Self;
    fn from_attributes(marker_attributes: &MarkerAttributes) -> Self;
    fn set_from_attributes(&mut self, marker_attributes: &MarkerAttributes);
    fn set_defaults_from_attributes(&mut self, marker_attributes: &MarkerAttributes);
}
impl VisibilityFlagsExt for VisibilityFlags {
    fn from_category_flags(cat_flags: CategoryFlags) -> Self {
        let mut flags = Self::empty();
        flags.set_from_category_flags(cat_flags);
        flags
    }
    fn set_from_category_flags(&mut self, cat_flags: CategoryFlags) {
        self.set(Self::TOGGLE, !cat_flags.contains(CategoryFlags::DISABLED));
    }
    fn from_pack_category(category: &Category) -> Self {
        let mut flags = Self::from_attributes(&category.marker_attributes);
        flags.set_from_category_flags(category.flags);
        flags
    }
    /// TODO: if [PackRoot] survives, give it a [CategoryFlags] field
    fn from_pack_root(root: &PackRoot) -> Self {
        Self::TOGGLES
    }
    fn from_attributes(marker_attributes: &MarkerAttributes) -> Self {
        let mut flags = Self::empty();
        flags.set_from_attributes(marker_attributes);
        flags
    }
    fn set_from_attributes(&mut self, marker_attributes: &MarkerAttributes) {
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
    fn set_defaults_from_attributes(&mut self, marker_attributes: &MarkerAttributes) {
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
