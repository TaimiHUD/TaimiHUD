use {
    crate::{
        controller::pathing::state::VisibilityFlags,
        space::resources::Texture,
    },
    std::sync::Arc,
    taimi_meta::packs::{CategoryPath, PoiPath, CategoryIndex},
    taimi_pack::attributes::{PoiAttributes, RenderAttributes},
};

pub struct SpacePoi {
    #[cfg(todo = "unnecessary")]
    pub path: PoiPath,
    pub category: CategoryPath,
    pub visibility: VisibilityFlags,
    attrs: Arc<RenderAttributes>,
    overrides: Option<Box<RenderAttributes>>,
    pub icon: Option<Arc<Texture>>,
}

impl SpacePoi {
    pub fn new(
        visibility: VisibilityFlags,
        _path: PoiPath,
        category: CategoryPath,
        mut attrs: Arc<RenderAttributes>,
    ) -> Self {
        if !attrs.poi.is_some() {
            log::warn!("{_path} has incomplete render attrs?");
            let _ = Arc::make_mut(&mut attrs).poi.get_or_insert_default();
        }
        Self {
            #[cfg(todo = "unnecessary")]
            path,
            category,
            visibility,
            attrs,
            overrides: None,
            icon: None,
        }
    }

    pub fn empty() -> Self {
        Self {
            #[cfg(todo = "unnecessary")]
            path: PoiPath::with_path(PoiIndex::MAX),
            category: CategoryPath::with_path(CategoryIndex::MAX),
            visibility: VisibilityFlags::empty(),
            attrs: super::EMPTY_RENDER_ATTRS.clone(),
            overrides: None,
            icon: None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.category.path == CategoryIndex::MAX
    }

    pub fn clear_overrides(&mut self) {
        self.overrides = None;
    }
    pub fn set_overrides(&mut self, overrides: RenderAttributes) {
        let overrides = self.overrides.insert(Box::new(overrides));
        let _ = overrides.poi.get_or_insert_default();
        overrides.merge(&self.attrs);
    }
    #[inline]
    pub fn set_attrs(&mut self, overrides: Option<RenderAttributes>) {
        match overrides {
            Some(o) => self.set_overrides(o),
            None => self.clear_overrides(),
        }
    }

    pub fn render_attrs(&self) -> &RenderAttributes {
        self.overrides.as_ref().map(|a| &**a)
            .unwrap_or(&self.attrs)
    }
    pub fn poi_attrs(&self) -> &PoiAttributes {
        let poi = self.render_attrs().poi.as_ref()
            .map(|p| &**p);
        unsafe {
            poi.unwrap_unchecked()
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct PoiScale {
    pub expansion: f32,
}

impl PoiScale {
    /// No expansion, standard sizing
    pub const DEFAULT: Self = Self::new(0.0);

    pub const fn new(expansion: f32) -> Self {
        Self { expansion }
    }

    /// Convert from settings
    pub const fn with_scale(poi_scale: f32) -> Self {
        Self::new(poi_scale - 1.0)
    }

    pub const fn scale(&self) -> f32 {
        self.expansion + 1.0
    }
}

impl Default for PoiScale {
    fn default() -> Self {
        Self::DEFAULT
    }
}
