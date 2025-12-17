use {
    super::get_overrides_mut,
    crate::{
        controller::pathing::{
            info::LoadedTrailInfo,
            space::{DrawSpace, TrailParams},
            state::VisibilityFlagsExt as _,
            PackSpace,
        },
        resources::Vertex,
    },
    glamour::Box3,
    std::mem,
    taimi_hoard::iters::IterExt,
    taimi_meta::{
        packs::{CategoryIndex, CategoryPath, TrailPath, VisibilityFlags},
        spatial::{irrelevant_box3, IRRELEVANT_MIN},
    },
    taimi_pack::{
        attributes::{RenderAttributes, TrailAttributes},
        trail::{TrailData, TrailSection},
        Pack,
    },
};

/// a component of a [LoadedMapPack](super::LoadedMapPack)
#[derive(Debug, Clone, Default)]
pub struct LoadedTrail {
    pub visibility: VisibilityFlags,
    pub(crate) info: LoadedTrailInfo,
    overrides: Option<Box<RenderAttributes>>,
}

impl LoadedTrail {
    pub fn invalid() -> Self {
        Self {
            info: LoadedTrailInfo::empty(),
            visibility: VisibilityFlags::empty(),
            overrides: None,
        }
    }

    pub fn from_pack(path: TrailPath, pack: &Pack) -> Self {
        let Some(trail) = pack.trails.get(path.path as usize) else {
            return Self::invalid()
        };
        let mut visibility = VisibilityFlags::DEFAULTS;
        let category = match () {
            #[cfg(todo)]
            _ => pack
                .categories
                .all_categories
                .get_index_of(trail.category.as_id())
                .map(|c| c as CategoryIndex),
            _ => pack
                .categories
                .all_categories
                .get_full(trail.category.as_id())
                .map(|(index, _, category)| {
                    visibility.set(VisibilityFlags::DEFAULT_TOGGLE, category.default_toggle());
                    #[cfg(todo = "unnecessary")]
                    visibility.set_defaults_from_attributes(&category.marker_attributes);
                    index as CategoryIndex
                }),
        }
        .unwrap_or(CategoryIndex::MAX);
        visibility.set_defaults_from_attributes(&trail.attributes);

        Self {
            info: {
                let mut info = LoadedTrailInfo::with_marker_attrs(
                    CategoryPath::with_path(category),
                    &trail.attributes,
                );
                info.trl = trail.trail_path.clone();
                info
            },
            visibility: visibility.restore_default_toggles(),
            overrides: None,
        }
    }

    pub fn render_attrs(&self) -> &RenderAttributes {
        self.overrides.as_ref().map(|a| &**a).unwrap_or(self.info.attrs())
    }
    pub fn trail_attrs(&self) -> &TrailAttributes {
        let trail = self.render_attrs().trail.as_ref().map(|p| &**p);
        unsafe { trail.unwrap_unchecked() }
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
        let _ = overrides.trail.get_or_insert_default();
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
        let _ = overrides.trail.get_or_insert_default();
        res
    }
    pub fn trail_overrides_mut(&mut self) -> &mut TrailAttributes {
        let overrides = get_overrides_mut(&mut self.overrides);
        unsafe { overrides.trail.as_mut().unwrap_unchecked() }
    }

    #[cfg(todo = "unused")]
    pub fn vertices_with_pack_trail(
        trail_data: &TrailData,
        trail: &Trail,
        params: &TrailParams,
    ) -> LoadedTrailGeometry {
        Self::vertices_with_data(trail_data, params, trail.scale(), trail.is_wall())
    }

    pub fn vertices_with_data(
        trail_data: &TrailData,
        params: &TrailParams,
        scale: f32,
        is_wall: bool,
        y_offset: f32,
    ) -> LoadedTrailGeometry {
        let mut params = params.bake();
        params.y_offset = y_offset;
        let mut vertices = Vec::new();
        let mut section_lengths = Vec::with_capacity(trail_data.sections.len());
        for (isec, section) in trail_data.sections.iter().enumerate() {
            params.y_offset = (params.y_offset - TrailParams::Y_OFFSET_SECTION_GAP).max(0.0);

            let prior_count = vertices.len();
            let vertex_count = if section.points.is_empty() {
                log::trace!("Section {isec} is empty.");
                0
            } else {
                params.interpolate_section_vertices(&mut vertices, section, scale, is_wall);
                let vertex_count = vertices.len() - prior_count;
                if log::log_enabled!(log::Level::Trace) {
                    let point_count = vertex_count / 2;
                    log::trace!(
                        "Section {isec} added {} interpolation points ({} -> {}).",
                        point_count - section.points.len(),
                        section.points.len(),
                        point_count,
                    );
                }
                vertex_count as u32
            };
            section_lengths.push(vertex_count);
        }

        LoadedTrailGeometry { vertices, section_lengths }
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

    pub fn info(&self) -> &LoadedTrailInfo {
        &self.info
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LoadedTrailSection {
    pub bounds: Box3<DrawSpace>,
    pub point_count: u32,
}

impl LoadedTrailSection {
    pub const EMPTY: Self = Self { bounds: Box3::ZERO, point_count: 0 };

    pub fn with_section(section: &TrailSection) -> Self {
        Self {
            point_count: section.points.len() as _,
            bounds: Self::bounds_for(section),
        }
    }
    pub fn with_sections<'a, S: AsRef<TrailSection>, I: IntoIterator<Item = S>>(
        sections: I,
    ) -> impl Iterator<Item = Self> {
        sections.into_iter().lazy_map(|s| Self::with_section(s.as_ref()))
    }

    pub fn bounds_for(section: &TrailSection) -> Box3<PackSpace> {
        if section.is_empty() {
            return irrelevant_box3()
        }
        let min = section.bounds.min.cast();
        let max = section.bounds.max.cast();
        Box3::new(min, max)
    }

    pub fn is_visible(&self) -> bool {
        match self {
            // XXX: empty may be a stub indicating geometry unloaded in future...
            #[cfg(todo)]
            Self { point_count, .. } => *point_count > 0,
            Self { bounds, .. } => bounds.min.x.to_bits() != IRRELEVANT_MIN.to_bits(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LoadedTrailGeometry {
    pub vertices: Vec<Vertex>,
    pub section_lengths: Vec<u32>,
}
impl LoadedTrailGeometry {
    pub fn clone_metadata(&self) -> Self {
        Self {
            section_lengths: self.section_lengths.clone(),
            vertices: Default::default(),
        }
    }
    pub fn take_vertices(&mut self) -> Vec<Vertex> {
        mem::take(&mut self.vertices)
    }

    pub fn empty() -> Self {
        Self {
            vertices: Vec::new(),
            section_lengths: Vec::new(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty() && self.section_lengths.is_empty()
    }
}
