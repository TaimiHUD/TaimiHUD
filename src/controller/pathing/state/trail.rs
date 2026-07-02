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
    glamour::{Box3, Vector3},
    std::mem,
    taimi_hoard::iters::IterExt,
    taimi_meta::{
        packs::{CategoryIndex, CategoryPath, TrailPath, VisibilityFlags},
        spatial::{irrelevant_box3, IRRELEVANT_MIN},
    },
    taimi_pack::{
        attributes::{cell::GetAttrDynExt, keys::{self, GetAttr}, RenderAttributes, TrailAttributes},
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
        colour: Vector3,
    ) -> LoadedTrailGeometry {
        Self::vertices_with_data(trail_data, params, trail.scale(), trail.is_wall(), colour)
    }

    pub fn load_with_data<A: ?Sized>(
        trail_data: &TrailData,
        params: &TrailParams,
        attrs: &A,
        y_offset: f32,
    ) -> LoadedTrailGeometry where
        A: GetAttr<keys::TrailScale>
            + GetAttr<keys::IsWall>
            + GetAttr<keys::Tint>
            + GetAttr<keys::MapTint>
            //+ GetAttr<keys::InGameVisibility>
            //+ GetAttr<keys::MapVisibility> + GetAttr<keys::MinimapVisibility>
            // TODO: ew remove bound
            + GetAttrDynExt,
    {
        let include_map = true;
        let mut map = LegacyTrailGeometry::empty();
        let legacy = Self::vertices_with_data(
            trail_data,
            params,
            attrs.attr_or_default::<keys::TrailScale>().into(),
            attrs.attr_or_default::<keys::IsWall>().into(),
            y_offset,
            glam::Vec4::from(attrs.attr_or_default::<keys::Tint>()).truncate().into(),
            include_map.then_some(&mut map),
        );
        LoadedTrailGeometry {
            legacy,
            map,
        }
    }
    pub fn vertices_with_data(
        trail_data: &TrailData,
        params: &TrailParams,
        scale: f32,
        is_wall: bool,
        y_offset: f32,
        colour: Vector3<f32>,
        mut map: Option<&mut LegacyTrailGeometry>,
    ) -> LegacyTrailGeometry {
        let mut params = params.bake();
        params.y_offset = y_offset;
        let mut vertices = Vec::new();
        let mut section_lengths = Vec::with_capacity(trail_data.sections.len());
        if let Some(map) = &mut map {
            map.section_lengths.reserve_exact(trail_data.sections.len());
        }
        for (isec, section) in trail_data.sections.iter().enumerate() {
            params.y_offset = (params.y_offset - TrailParams::Y_OFFSET_SECTION_GAP).max(0.0);

            let prior_count = vertices.len();
            let (prior_count_map, vertices_map) = map.as_mut().map(|map| {
                let count = map.vertices.len();
                (count as u32, &mut map.vertices)
            }).unzip();
            params.interpolate_section_vertices(&mut vertices, vertices_map, section, scale, is_wall, colour);
            let vertex_count = (vertices.len() - prior_count) as u32;
            #[cfg(taimi_debug)]
            if vertex_count == 0 {
                log::trace!("Section {isec} is empty.");
            } else if log::log_enabled!(log::Level::Trace) {
                let point_count = vertex_count / 2;
                log::trace!(
                    "Section {isec} added {} interpolation points ({} -> {}).",
                    point_count - section.points.len() as u32,
                    section.points.len(),
                    point_count,
                );
            }
            section_lengths.push(vertex_count);
            if let (Some(map), Some(prior)) = (&mut map, prior_count_map) {
                let vertex_count_map = map.vertices.len() as u32 - prior;
                map.section_lengths.push(vertex_count_map);
            }
        }

        LegacyTrailGeometry { vertices, section_lengths }
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
    pub legacy: LegacyTrailGeometry,
    pub map: LegacyTrailGeometry,
    #[cfg(todo)]
    pub space: ArcrenderTrailGeometry,
}
impl LoadedTrailGeometry {
    pub fn empty() -> Self {
        Self {
            legacy: LegacyTrailGeometry::empty(),
            map: LegacyTrailGeometry::empty(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.legacy.is_empty() && self.map.is_empty()
    }
}
#[derive(Debug, Clone)]
pub struct LegacyTrailGeometry {
    pub vertices: Vec<Vertex>,
    pub section_lengths: Vec<u32>,
}
impl LegacyTrailGeometry {
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
