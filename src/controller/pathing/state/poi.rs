use {
    super::get_overrides_mut,
    crate::controller::pathing::{
        info::LoadedPoiInfo,
        space::DrawSpace,
        state::VisibilityFlagsExt as _,
        PackSpace,
    },
    glamour::{Box3, Point3, Size3},
    taimi_meta::packs::{CategoryIndex, CategoryPath, PoiPath, VisibilityFlags},
    taimi_pack::{
        attributes::{InteractionAttributes, PoiAttributes, RenderAttributes},
        Pack,
        Poi,
    },
};

/// a component of a [LoadedMapPack](super::LoadedMapPack)
#[derive(Debug, Clone, Default)]
pub struct LoadedPoi {
    pub visibility: VisibilityFlags,
    pub marker_position: Point3<DrawSpace>,
    pub(crate) info: LoadedPoiInfo,
    overrides: Option<Box<RenderAttributes>>,
}

impl LoadedPoi {
    pub fn invalid() -> Self {
        Self {
            info: LoadedPoiInfo::empty(),
            visibility: VisibilityFlags::empty(),
            marker_position: Point3::INFINITY,
            overrides: None,
        }
    }

    pub fn from_pack(path: PoiPath, pack: &Pack) -> Self {
        let Some(poi) = pack.pois.get(path.path as usize) else {
            return Self::invalid()
        };
        let mut visibility = VisibilityFlags::DEFAULTS;
        let category =
            match () {
                #[cfg(todo)]
                _ => pack
                    .categories
                    .all_categories
                    .get_index_of(poi.category.as_id())
                    .map(|c| c as CategoryIndex),
                _ => pack.categories.all_categories.get_full(poi.category.as_id()).map(
                    |(index, _, category)| {
                        visibility.set(VisibilityFlags::DEFAULT_TOGGLE, category.default_toggle());
                        #[cfg(todo = "unnecessary")]
                        visibility.set_defaults_from_attributes(&category.marker_attributes);
                        index as CategoryIndex
                    },
                ),
            }
            .unwrap_or(CategoryIndex::MAX);
        visibility.set_defaults_from_attributes(&poi.attributes);
        let marker_position = Self::marker_position_for(poi);

        Self {
            info: LoadedPoiInfo::with_marker_attrs(CategoryPath::with_path(category), &poi.attributes),
            visibility: visibility.restore_default_toggles(),
            marker_position,
            overrides: None,
        }
    }

    pub fn render_attrs(&self) -> &RenderAttributes {
        self.overrides.as_ref().map(|a| &**a).unwrap_or(self.info.attrs())
    }
    pub fn poi_attrs(&self) -> &PoiAttributes {
        let poi = self.render_attrs().poi.as_ref().map(|p| &**p);
        unsafe { poi.unwrap_unchecked() }
    }
    /// TODO: might want ability to override later, in which case make sure
    /// [Self::get_interaction_attrs] is adjusted to match!
    #[inline]
    #[cfg(feature = "paths-interact")]
    pub fn interaction_attrs(&self) -> &InteractionAttributes {
        self.info().interaction_attrs()
    }
    #[inline]
    #[cfg(feature = "paths-interact")]
    pub fn get_interaction_attrs(&self) -> Option<&InteractionAttributes> {
        self.info().get_interaction_attrs().map(|i| &**i)
    }
    #[cfg(todo = "unused")]
    pub fn filter_attrs(&self) -> Option<&FilterAttributes> {
        self.info.get_filter_attrs().map(|f| &**f)
    }

    pub fn clear_overrides(&mut self) {
        self.overrides = None;
    }
    pub fn set_overrides(&mut self, overrides: RenderAttributes) {
        let overrides = self.overrides.insert(Box::new(overrides));
        let _ = overrides.poi.get_or_insert_default();
        overrides.merge(self.info.attrs());
    }
    #[inline]
    pub fn set_attrs(&mut self, overrides: Option<RenderAttributes>) {
        match overrides {
            Some(o) => self.set_overrides(o),
            None => self.clear_overrides(),
        }
    }
    pub fn with_overrides_mut<R, F: FnOnce(&mut RenderAttributes) -> R>(&mut self, f: F) -> R {
        let overrides = get_overrides_mut(&mut self.overrides);
        let res = f(overrides);
        // please don't clear the attributes, that would be very rude...
        let _ = overrides.poi.get_or_insert_default();
        res
    }
    pub fn poi_overrides_mut(&mut self) -> &mut PoiAttributes {
        let overrides = get_overrides_mut(&mut self.overrides);
        unsafe { overrides.poi.as_mut().unwrap_unchecked() }
    }

    pub fn bounds(&self) -> Box3<DrawSpace> {
        let max_diagonal = match self.poi_attrs().icon_size {
            Some(edge_len) => (edge_len.powi(2) * 2.0).sqrt(),
            None => {
                const DEFAULT_DIAG: f32 = match taimi_pack::attributes::keys::IconSize::DEFAULT.0 {
                    // 2.0.sqrt()
                    1.0 => 1.41421,
                    #[cfg(todo = "unnecessary")]
                    ohno => (ohno.powi(2) * 2.0).sqrt(),
                    _ => panic!("default poi size changed!"),
                };
                DEFAULT_DIAG
            },
        };
        Box3::from_origin_and_size(self.position(), Size3::splat(max_diagonal))
    }
    pub fn position(&self) -> Point3<DrawSpace> {
        self.marker_position + self.offset()
    }

    pub fn offset(&self) -> Point3<PackSpace> {
        Point3::ZERO.with_y(self.poi_attrs().height_offset())
    }
    pub fn marker_position_for(poi: &Poi) -> Point3<PackSpace> {
        Point3::from_raw(poi.position.into())
    }

    pub fn is_invalid(&self) -> bool {
        self.info.is_empty()
    }
    pub fn get(&self) -> Option<&Self> {
        match self.is_invalid() {
            false => Some(self),
            true => None,
        }
    }

    #[inline]
    pub fn category_path(&self) -> CategoryPath {
        self.info.category_path
    }
    pub fn category(&self) -> Option<CategoryPath> {
        match self.is_invalid() {
            false => Some(self.category_path()),
            true => None,
        }
    }
    pub fn info(&self) -> &LoadedPoiInfo {
        &self.info
    }
}
