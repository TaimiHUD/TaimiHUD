#[cfg(todo)]
use crate::controller::pathing::{
    state::interactive::InteractivePoi,
    filter::MapFilters,
    taimi_pack::attributes::keys::Guid,
};
use {
    crate::{
        controller::pathing::{
            registry::{LoadedPack, PackCategoryInfo, PackConfig, PackPath, PackInfoSignature, LoadedPoiNs, LoadedTrailNs, LoadedPoiIndex, LoadedTrailIndex},
            space::{DrawSpace, TrailParams},
            shared::{MapPackInfo, LoadedPoiInfo, LoadedTrailInfo, LoadedMarkerInfo},
            shared::EMPTY_RENDER_ATTRS,
            PackSpace,
        },
        resources::Vertex,
    },
    std::{iter, mem, sync::{Arc, LazyLock}, num::NonZero, ops, hash::Hash, collections::{VecDeque, BTreeMap}},
    taimi_hoard::collections::lru::RecentlyUsed,
    taimi_hoard::flags::set::{BitFlagForSet, FlagSet},
    taimi_hoard::loc::{indexed::{self, IndexedList}, LocationMut, LocationRef},
    taimi_hoard::iters::IterExt,
    taimi_meta::packs::{
        collections::CategorySet,
        CategoryIndex, CategoryPath, MapIndex, PoiPath, TrailPath, TrailSectionPath, TrailSectionIndex,
        TrailSectionNs,
    },
    taimi_meta::spatial::irrelevant_box3,
    taimi_pack::attributes::{self, RenderAttributes, PoiAttributes, TrailAttributes},
    bitvec::{order::Lsb0, slice::BitSlice, vec::BitVec, view::BitView},
};
use bitflags::bitflags;
use glam::Vec3Swizzles;
use glamour::{Box3, Point3, Size3, Vector3};
use taimi_meta::{map::MapID, ui::{MapContext, LocalContext}};
use taimi_pack::category::{Category, CategoryFlags};
use taimi_pack::{trail::{TrailData, TrailSection}, MarkerAttributes, Pack, Poi, Trail};

#[derive(Debug, Clone, Default)]
pub struct LoadedCategory {
    pub visibility: VisibilityFlags,
}

impl LoadedCategory {
    pub const INVALID: Self = Self {
        visibility: VisibilityFlags::empty(),
    };
}

#[derive(Debug, Clone, Default)]
pub struct LoadedPoi {
    pub visibility: VisibilityFlags,
    pub marker_position: Point3<DrawSpace>,
    info: LoadedPoiInfo,
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
        let category = match () {
            #[cfg(todo)]
            _ => pack.categories.all_categories.get_index_of(poi.category.as_id()).map(|c| c as CategoryIndex),
            _ => pack.categories.all_categories.get_full(poi.category.as_id()).map(|(index, _, category)| {
                visibility.set(VisibilityFlags::DEFAULT_TOGGLE, category.default_toggle());
                #[cfg(todo = "unnecessary")]
                visibility.set_defaults_from_attributes(&category.marker_attributes);
                index as CategoryIndex
            }),
        }.unwrap_or(CategoryIndex::MAX);
        visibility.set_defaults_from_attributes(&poi.attributes);
        let marker_position = Self::marker_position_for(poi);

        let mut attrs = poi.attributes.render.clone()
            .unwrap_or_else(|| EMPTY_RENDER_ATTRS.clone());
        if !attrs.poi.is_some() {
            log::warn!("{path} has incomplete render attrs?");
            let _ = Arc::make_mut(&mut attrs).poi.get_or_insert_default();
        }

        Self {
            info: LoadedPoiInfo {
                marker_info: LoadedMarkerInfo {
                    category_path: CategoryPath::with_path(category),
                    attrs,
                },
            },
            visibility: visibility.restore_default_toggles(),
            marker_position,
            overrides: None,
        }
    }

    pub fn render_attrs(&self) -> &RenderAttributes {
        self.overrides.as_ref().map(|a| &**a)
            .unwrap_or(&self.info.attrs)
    }
    pub fn poi_attrs(&self) -> &PoiAttributes {
        let poi = self.render_attrs().poi.as_ref()
            .map(|p| &**p);
        unsafe {
            poi.unwrap_unchecked()
        }
    }

    pub fn clear_overrides(&mut self) {
        self.overrides = None;
    }
    pub fn set_overrides(&mut self, overrides: RenderAttributes) {
        let overrides = self.overrides.insert(Box::new(overrides));
        let _ = overrides.poi.get_or_insert_default();
        overrides.merge(&self.info.attrs);
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
        unsafe {
            overrides.poi.as_mut().unwrap_unchecked()
        }
    }

    pub fn bounds(&self) -> Box3<DrawSpace> {
        let max_diagonal = match self.poi_attrs().icon_size {
            Some(edge_len) =>
                (edge_len.powi(2) * 2.0).sqrt(),
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

#[derive(Debug, Clone, Default)]
pub struct LoadedTrail {
    pub visibility: VisibilityFlags,
    /// TODO: deleteme? not much more than starting yoffset is needed tbh
    pub section_info: Arc<LoadedTrailGeometryInfo>,
    info: LoadedTrailInfo,
    overrides: Option<Box<RenderAttributes>>,
}

impl LoadedTrail {
    pub fn invalid() -> Self {
        Self {
            info: LoadedTrailInfo::empty(),
            visibility: VisibilityFlags::empty(),
            section_info: LoadedTrailGeometryInfo::empty_arc().clone(),
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
            _ => pack.categories.all_categories.get_index_of(trail.category.as_id()).map(|c| c as CategoryIndex),
            _ => pack.categories.all_categories.get_full(trail.category.as_id()).map(|(index, _, category)| {
                visibility.set(VisibilityFlags::DEFAULT_TOGGLE, category.default_toggle());
                #[cfg(todo = "unnecessary")]
                visibility.set_defaults_from_attributes(&category.marker_attributes);
                index as CategoryIndex
            }),
        }.unwrap_or(CategoryIndex::MAX);
        visibility.set_defaults_from_attributes(&trail.attributes);

        let mut attrs = trail.attributes.render.clone()
            .unwrap_or_else(|| EMPTY_RENDER_ATTRS.clone());
        if !attrs.trail.is_some() {
            log::warn!("{path} has incomplete render attrs?");
            let _ = Arc::make_mut(&mut attrs).trail.get_or_insert_default();
        }

        Self {
            info: LoadedTrailInfo {
                marker_info: LoadedMarkerInfo {
                    category_path: CategoryPath::with_path(category),
                    attrs,
                },
                trl: trail.trail_path.clone(),
            },
            visibility: visibility.restore_default_toggles(),
            overrides: None,
            section_info: LoadedTrailGeometryInfo::empty_arc().clone(),
        }
    }

    pub fn render_attrs(&self) -> &RenderAttributes {
        self.overrides.as_ref().map(|a| &**a)
            .unwrap_or(&self.info.attrs)
    }
    pub fn trail_attrs(&self) -> &TrailAttributes {
        let trail = self.render_attrs().trail.as_ref()
            .map(|p| &**p);
        unsafe {
            trail.unwrap_unchecked()
        }
    }

    pub fn clear_overrides(&mut self) {
        self.overrides = None;
    }
    pub fn set_overrides(&mut self, overrides: RenderAttributes) {
        let overrides = self.overrides.insert(Box::new(overrides));
        let _ = overrides.trail.get_or_insert_default();
        overrides.merge(&self.info.attrs);
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
        unsafe {
            overrides.trail.as_mut().unwrap_unchecked()
        }
    }

    pub fn section_info_mut(&mut self) -> &mut LoadedTrailGeometryInfo {
        Arc::make_mut(&mut self.section_info)
    }
    pub fn populate_data(&mut self, trail_data: &TrailData) -> bool {
        if self.section_info.sections.is_some() {
            return false
        }
        self.section_info_mut().populate_data(trail_data)
    }
    pub fn populate_geometry_info(&mut self, section_info: impl IntoIterator<Item = LoadedTrailGeometrySection>, cap: Option<u32>) -> bool {
        if self.section_info.geometry_sections.is_some() {
            return false
        }
        self.section_info_mut().populate_geometry_info(section_info, cap)
    }

    pub fn vertices_for(&self, trail_data: &TrailData, params: &TrailParams) -> LoadedTrailGeometry {
        let section_count = self.section_info.sections.as_ref().map(|s| s.len()).unwrap_or(0);
        // TODO
        let trail = self.trail_attrs();
        let scale = trail.scale();
        let is_wall = trail.is_wall();
        let y_offset = match () {
            #[cfg(todo)]
            _ => params.y_offset_for_trail(pack, path),
            _ => {
                let y_offset_sig = (self.category_path().path as usize) << 24 | section_count;
                params.y_offset_for(y_offset_sig)
            },
        };
        Self::vertices_with_data(trail_data, params, scale, is_wall, y_offset)
    }

    pub fn vertices_with_pack_trail(trail_data: &TrailData, trail: &Trail, params: &TrailParams, y_offset: f32) -> LoadedTrailGeometry {
        Self::vertices_with_data(trail_data, params, trail.scale(), trail.is_wall(), y_offset)
    }

    pub fn vertices_with_data(trail_data: &TrailData, params: &TrailParams, scale: f32, is_wall: bool, mut y_offset: f32) -> LoadedTrailGeometry {
        let width = params.width();
        let resolution = params.resolution();
        let smoothing = params.smoothing();

        let mut vertices = Vec::new();
        let mut section_lengths = Vec::with_capacity(trail_data.sections.len());
        #[cfg(deleteme)]
        let mut y_offsets = Vec::new();
        for (isec, section) in trail_data.sections.iter().enumerate() {
            y_offset = (y_offset - TrailParams::Y_OFFSET_SECTION_GAP).max(0.0);
            #[cfg(deleteme)]
            if y_offset != 0.0 {
                y_offsets.push(y_offset);
            }

            let prior_count = vertices.len();
            let vertex_count = if section.points.is_empty() {
                log::trace!("Section {isec} is empty.");
                0
            } else {
                LoadedTrailSection::vertices_for(&mut vertices, section, scale, is_wall, width, resolution, smoothing, y_offset);
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

        LoadedTrailGeometry {
            vertices,
            section_lengths,
            #[cfg(deleteme)]
            y_offsets,
        }
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
/// TODO: deleteme
#[derive(Debug, Clone, Default)]
pub struct LoadedTrailGeometryInfo {
    pub sections: Option<Arc<[LoadedTrailSection]>>,
    pub geometry_sections: Option<Arc<[LoadedTrailGeometrySection]>>,
    pub geometry_sections_cap: u32,
}
impl LoadedTrailGeometryInfo {
    pub fn empty() -> Self {
        Self {
            sections: None,
            geometry_sections: None,
            geometry_sections_cap: 0,
        }
    }
    fn empty_arc() -> &'static Arc<Self> {
        static EMPTY: LazyLock<Arc<LoadedTrailGeometryInfo>> = LazyLock::new(|| Arc::new(LoadedTrailGeometryInfo::empty()));
        &EMPTY
    }

    fn set_sections(&mut self, sections: &mut dyn Iterator<Item = LoadedTrailSection>) {
        let _ = self.sections.insert(sections.into_iter().collect());
    }
    fn set_geometry_info(&mut self, section_info: &mut dyn Iterator<Item = LoadedTrailGeometrySection>, cap: Option<u32>) {
        let _ = self.geometry_sections.insert(section_info.into_iter().collect());
        self.geometry_sections_cap = cap.unwrap_or(0);
    }
    pub fn populate_data(&mut self, trail_data: &TrailData) -> bool {
        if self.sections.is_some() {
            return false
        }
        self.set_sections(&mut trail_data.sections.iter()
            .map(|section| LoadedTrailSection::with_section(section))
        );
        true
    }

    pub fn populate_geometry_info(&mut self, section_info: impl IntoIterator<Item = LoadedTrailGeometrySection>, cap: Option<u32>) -> bool {
        if self.geometry_sections.is_some() {
            return false
        }
        self.set_geometry_info(&mut section_info.into_iter(), cap);
        true
    }

    pub fn section_geometry_vertices(&self, path: TrailSectionPath) -> Option<ops::Range<u32>> {
        let i = path.path as usize;
        let mut sections = self.geometry_sections.as_ref()?.iter().skip(i);
        let start = sections.next()?.vertex_start;
        let end = match sections.next() {
            Some(s) => s.vertex_start,
            None => self.geometry_sections_cap,
        };
        Some(start..end)
    }

    pub fn trail_section_bounds(&self) -> indexed::LocatorEnumerateAsRel<TrailSectionNs, TrailSectionIndex, impl Iterator<Item = Box3<DrawSpace>> + '_> {
        let geometry = self.geometry_sections.as_ref().map(|g| &g[..]).unwrap_or(&[]);
        let geometry_offsets = geometry.into_iter().map(|g| g.y_offset);
        let sections = self.sections.as_ref().map(|s| &s[..]).unwrap_or(&[]);
        let bounds =sections.iter()
            .zip(geometry_offsets.chain(iter::repeat(0.0f32)))
            .lazy_map(|(section, offset)| match offset {
                0.0 => section.bounds,
                offset => Box3 {
                    min: section.bounds.min + Vector3::ZERO.with_y(offset),
                    max: section.bounds.max + Vector3::ZERO.with_y(offset),
                },
            });
        indexed::LocatorRelIter0::enumerate(Default::default(), bounds)
    }

    #[cfg(deleteme)]
    fn next_section(idx: usize, section: &LoadedTrailSection, len: u32, y_offsets: &mut dyn Iterator<Item = f32>, bookmark: &mut u32) -> LoadedTrailSectionInfo {
        let mut bounds = section.bounds;
        let y_offset = y_offsets.next();
        if len > 0 {
            if let Some(y_offset) = y_offset {
                bounds.min.y += y_offset;
                bounds.max.y += y_offset;
            }
        }
        let start = *bookmark;
        *bookmark += len;
        let end = *bookmark;
        LoadedTrailSectionInfo {
            path: TrailSectionPath::with_path(idx as TrailSectionIndex),
            vertex_range: start..end,
            bounds,
            y_offset: y_offset.unwrap_or_default(),
        }
    }
    #[cfg(deleteme)]
    pub fn sections<'a, 'g>(&'a self, geometry: &'g LoadedTrailGeometry) -> impl Iterator<Item = LoadedTrailSectionInfo> +'a where
        'g: 'a,
    {
        let y_offsets = &geometry.y_offsets;
        self.sections.iter().flat_map(move |sections| {
            let mut bookmark = 0u32;
            let mut y_offsets = y_offsets.iter().copied();
            sections.iter().zip(&geometry.section_lengths).enumerate().map(move |(i, (section, &len))|
                Self::next_section(i, section, len, &mut y_offsets, &mut bookmark)
            )
        })
    }
    #[cfg(todo = "unnecessary")]
    pub fn get_sections<'g>(&self, geometry: &'g LoadedTrailGeometry) -> impl Iterator<Item = LoadedTrailSectionInfo> + 'g {
        let sections = self.sections.clone().map(ArcSliceIter::new_iter);
        let y_offsets = &geometry.y_offsets;
        sections.into_iter().flat_map(move |sections| {
            let mut y_offsets = y_offsets.iter().copied();
            let mut bookmark = 0u32;
            sections.into_iter_arc().zip(&geometry.section_lengths).enumerate().map(move |(i, (section, &len))|
                Self::next_section(i, section, len, &mut y_offsets, &mut bookmark)
            )
        })
    }

    pub(crate) fn sections_sig(&self) -> (Option<usize>, u32) {
        (
            self.sections.as_ref().map(|s| s.len()),
            self.geometry_sections_cap,
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LoadedTrailSection {
    pub bounds: Box3<DrawSpace>,
    pub point_count: u32,
}

impl LoadedTrailSection {
    pub const EMPTY: Self = Self {
        bounds: Box3::ZERO,
        point_count: 0,
    };

    pub fn with_section(section: &TrailSection) -> Self {
        Self {
            point_count: section.points.len() as _,
            bounds: Self::bounds_for(section),
        }
    }
    pub fn with_sections<'a, S: AsRef<TrailSection>, I: IntoIterator<Item = S>>(sections: I) -> impl Iterator<Item = Self> {
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

    pub fn vertices_for(vertices: &mut Vec<Vertex>, section: &TrailSection, scale: f32, is_wall: bool, width: f32, resolution: f32, smoothing: Option<f32>, y_offset: f32) {
        // Interpolate points to be no more than 1/resolution metres apart.
        let mut points = Vec::with_capacity(section.points.len());
        let mut prev_point = None;
        for mut point in section.points.iter().copied() {
            point.y += y_offset;

            if let Some(prev_point) = prev_point.replace(point) {
                let dist = prev_point.distance(point);
                let segments = (dist * resolution) as i32;
                for i in 0..segments {
                    let s = (i + 1) as f32 / (segments + 1) as f32;
                    let position = match smoothing {
                        None => s,
                        // bias resolution near corners
                        Some(smoothing) => s.powi(if smoothing > 6.0 { 3 } else { 2 }),
                    };
                    let int_point = prev_point.lerp(point, position);
                    points.push(int_point);
                }
            }
            points.push(point);
        }

        if let Some(smoothing) = smoothing {
            let mut points = &mut points[..];
            while let &mut [prev, mid, ..] = points {
                let next = points.get(2).copied().unwrap_or(mid);
                let target = prev.slerp(next, 0.5);
                let smooth = mid.xz().lerp(target.xz(), smoothing / 10.0);
                points[1] = smooth.extend(mid.y * 0.925 + target.y * 0.075).xzy();
                points = &mut points[1..];
            }
        }

        let mut cur_point = points[0];
        let mut last_offset = Vector3::ZERO;
        let mut flip_over = 1.0f32;
        let normal_offset = width * scale / 2.0;
        let mut mod_distance = Vector3::ZERO;

        let mut distance = 0.0f32;
        for &next_point in points.iter().skip(1) {
            let path_direction = next_point - cur_point;
            let offset = path_direction.cross(Vector3::Y);
            let offset = if is_wall { path_direction.cross(offset) } else { offset };
            let offset = offset.normalize();

            if last_offset != Vector3::ZERO && offset.dot(last_offset) < 0.0 {
                flip_over *= -1.0;
            }

            mod_distance = offset * normal_offset * flip_over;
            let normal_scale_dir = mod_distance.to_raw().normalize_or(
                glam::vec3(1.0, 0.0, 1.0)
                    .normalize()
                    .copysign(mod_distance.to_raw()),
            );

            vertices.push(Vertex {
                position: (cur_point - mod_distance).into(),
                colour: glam::Vec3::ONE,
                normal: -normal_scale_dir,
                texture: glam::vec2(1.0, distance / width - 1.0),
            });
            vertices.push(Vertex {
                position: (cur_point + mod_distance).into(),
                colour: glam::Vec3::ONE,
                normal: normal_scale_dir,
                texture: glam::vec2(0.0, distance / width - 1.0),
            });

            distance += path_direction.length();
            last_offset = offset;
            cur_point = next_point;
        }

        let normal_scale_dir = mod_distance.to_raw().normalize_or(
            glam::vec3(1.0, 0.0, 1.0)
                .normalize()
                .copysign(mod_distance.to_raw()),
        );
        vertices.push(Vertex {
            position: (cur_point - mod_distance).into(),
            colour: glam::Vec3::ONE,
            normal: -normal_scale_dir,
            texture: glam::vec2(1.0, distance / width - 1.0),
        });
        vertices.push(Vertex {
            position: (cur_point + mod_distance).into(),
            colour: glam::Vec3::ONE,
            normal: normal_scale_dir,
            texture: glam::vec2(0.0, distance / width - 1.0),
        });
    }
}

/// TODO: deleteme
#[derive(Debug, Clone, Default)]
pub struct LoadedTrailSectionInfo {
    pub path: TrailSectionPath,
    pub bounds: Box3<DrawSpace>,
    pub vertex_range: ops::Range<u32>,
    pub y_offset: f32,
}
impl LoadedTrailSectionInfo {
    /// [self.bounds] without [self.y_offset]
    pub fn pack_bounds(&self) -> Box3<DrawSpace> {
        let mut bounds = self.bounds;
        if self.y_offset != 0.0 {
            bounds.min.y -= self.y_offset;
            bounds.max.y -= self.y_offset;
        }
        bounds
    }

    pub fn cap(sections: &[Self]) -> Option<u32> {
        sections.last().map(|s| s.vertex_range.end)
    }
}
#[derive(Debug, Clone, Default)]
pub struct LoadedTrailGeometrySection {
    pub vertex_start: u32,
    pub y_offset: f32,
}
impl LoadedTrailGeometrySection {
    pub fn with_info(info: &LoadedTrailSectionInfo) -> Self {
        Self {
            vertex_start: info.vertex_range.start,
            y_offset: info.y_offset,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LoadedTrailGeometry {
    pub vertices: Vec<Vertex>,
    pub section_lengths: Vec<u32>,
    #[cfg(deleteme)]
    pub y_offsets: Vec<f32>,
}
impl LoadedTrailGeometry {
    pub fn clone_metadata(&self) -> Self {
        Self {
            section_lengths: self.section_lengths.clone(),
            #[cfg(deleteme)]
            y_offsets: self.y_offsets.clone(),
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
            #[cfg(deleteme)]
            y_offsets: Vec::new(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty() && self.section_lengths.is_empty()
    }
}

fn get_overrides_mut<'a>(overrides: &'a mut Option<Box<RenderAttributes>>) -> &'a mut Box<RenderAttributes> {
    overrides.get_or_insert_with(|| Box::new((**EMPTY_RENDER_ATTRS).clone()))
}

#[derive(Debug, Clone)]
pub struct LoadedMapPack {
    pub map_id: NonZero<MapID>,
    pub info_sig: PackInfoSignature,
    pub used: RecentlyUsed,
    pub pois: Box<[LoadedPoi]>,
    #[cfg(todo)]
    pub poi_guids: Arc<[Guid]>,
    #[cfg(todo)]
    pub interactive_pois: Arc<[InteractivePoi]>,
    #[cfg(todo)]
    pub interactive_pois_nearby: BitVec,
    pub trails: Box<[LoadedTrail]>,
    #[cfg(todo)]
    pub trail_guids: Box<[Guid]>,
    pub categories: Arc<[LoadedCategory]>,
    #[cfg(todo)]
    pub filters: MapFilters,
}

impl LoadedMapPack {
    pub fn empty(map_id: MapIndex) -> Self {
        Self {
            map_id,
            info_sig: PackInfoSignature::EMPTY,
            used: RecentlyUsed::DEFAULT,
            #[cfg(todo)]
            interactive_pois: Default::default(),
            #[cfg(todo)]
            interactive_pois_nearby: Default::default(),
            pois: Default::default(),
            #[cfg(todo)]
            poi_guids: Default::default(),
            trails: Default::default(),
            #[cfg(todo)]
            trail_guids: Default::default(),
            categories: Default::default(),
            #[cfg(todo)]
            filters: Default::default(),
        }
    }

    pub fn from_pack(map_id: MapIndex, info: &MapPackInfo, pack: &Pack) -> Self {
        let pois = info.pois()
            .map(|path| LoadedPoi::from_pack(path, pack))
            .collect();
        #[cfg(todo)]
        let poi_guids = info.poi_guid_filter(info.pois())
            .map(|path|
                pack.pois.get(path.path as usize).map(|poi| Guid::from(poi.guid)).unwrap_or_default()
            ).collect();
        #[cfg(todo)]
        let interactive_pois = info.pois().enumerate()
            .map(|(i, path)| InteractivePoi::from_pack(i as PoiIndex, path, pack))
            .filter(|ipoi| !ipoi.is_empty())
            .collect();
        let trails = info.trails()
            .map(|path| LoadedTrail::from_pack(path, pack))
            .collect();
        #[cfg(todo)]
        let trail_guids = info.trail_guid_filter(info.trails())
            .map(|path|
                pack.trails.get(path.path as usize).map(|trail| Guid::from(trail.guid)).unwrap_or_default()
            ).collect();
        #[cfg(todo)]
        let filters = MapFilters::from_pack(info, active);

        let mut loaded = Self {
            map_id,
            info_sig: info.info_sig.clone(),
            #[cfg(todo)]
            interactive_pois_nearby: BitVec::new(),
            #[cfg(todo)]
            interactive_pois,
            pois,
            #[cfg(todo)]
            poi_guids,
            trails,
            #[cfg(todo)]
            trail_guids,
            #[cfg(todo)]
            filters,
            categories: Default::default(),
            used: RecentlyUsed::DEFAULT,
        };
        #[cfg(todo)]
        {
            loaded.interactive_pois_nearby.reserve_exact(loaded.interactive_pois.len());
        }

        loaded
    }

    pub fn lpois(&self) -> &IndexedList<LoadedPoiNs, LoadedPoiIndex, [LoadedPoi]> {
        IndexedList::from_ref(&self.pois[..])
    }
    pub fn lpois_mut(&mut self) -> &mut IndexedList<LoadedPoiNs, LoadedPoiIndex, [LoadedPoi]> {
        IndexedList::from_mut(&mut self.pois[..])
    }
    pub fn pois<'a, 'i>(&'a self, info: &'i MapPackInfo) -> impl Iterator<Item = (PoiPath, &'a LoadedPoi)> + 'i where
        'a: 'i,
    {
        info.pois().zip(self.pois.iter())
    }
    pub fn pois_mut<'a, 'i>(&'a mut self, info: &'i MapPackInfo) -> impl Iterator<Item = (PoiPath, &'a mut LoadedPoi)> + 'i where
        'a: 'i,
    {
        info.pois().zip(self.pois.iter_mut())
    }
    #[cfg(todo)]
    pub fn poi_guids<'a, 'i>(&'a self, info: &'i MapPackInfo) -> impl Iterator<Item = (PoiPath, &'a Guid)> + 'i where
        'a: 'i,
    {
        info.poi_guid_filter(info.pois()).zip(self.poi_guids.iter())
    }
    pub fn poi_at<'a>(&'a self, path: PoiPath<&'_ MapPackInfo>) -> Option<&'a LoadedPoi> {
        let info = path.root;
        info.poi_index(path.unscope())
            .and_then(|i| self.pois.get(i.path as usize))
    }
    pub fn poi_at_mut<'a>(&'a mut self, path: PoiPath<&'_ MapPackInfo>) -> Option<&'a mut LoadedPoi> {
        let info = path.root;
        info.poi_index(path.unscope())
            .and_then(|i| self.pois.get_mut(i.path as usize))
    }

    pub fn ltrails(&self) -> &IndexedList<LoadedTrailNs, LoadedTrailIndex, [LoadedTrail]> {
        IndexedList::from_ref(&self.trails[..])
    }
    pub fn ltrails_mut(&mut self) -> &mut IndexedList<LoadedTrailNs, LoadedTrailIndex, [LoadedTrail]> {
        IndexedList::from_mut(&mut self.trails[..])
    }
    pub fn trails<'a, 'i>(&'a self, info: &'i MapPackInfo) -> impl Iterator<Item = (TrailPath, &'a LoadedTrail)> + 'i where
        'a: 'i,
    {
        info.trails().zip(self.trails.iter())
    }
    pub fn trails_mut<'a, 'i>(&'a mut self, info: &'i MapPackInfo) -> impl Iterator<Item = (TrailPath, &'a mut LoadedTrail)> + 'i where
        'a: 'i,
    {
        info.trails().zip(self.trails.iter_mut())
    }
    #[cfg(todo)]
    pub fn trail_guids<'a, 'i>(&'a self, info: &'i MapPackInfo) -> impl Iterator<Item = (TrailPath, &'a Guid)> + 'i where
        'a: 'i,
    {
        info.trail_guid_filter(info.trails()).zip(&self.trail_guids)
    }
    pub fn trail_at<'a>(&'a self, path: TrailPath<&'_ MapPackInfo>) -> Option<&'a LoadedTrail> {
        let info = path.root;
        info.trail_index(path.unscope())
            .and_then(|i| self.trails.get(i.path as usize))
    }
    pub fn trail_at_mut<'a>(&'a mut self, path: TrailPath<&'_ MapPackInfo>) -> Option<&'a mut LoadedTrail> {
        let info = path.root;
        info.trail_index(path.unscope())
            .and_then(|i| self.trails.get_mut(i.path as usize))
    }

    pub fn categories<'a, 'i>(&'a self, info: &'i MapPackInfo) -> impl Iterator<Item = (CategoryPath, &'a LoadedCategory)> + 'i where
        'a: 'i,
    {
        info.categories().zip(self.categories.iter())
    }
    pub fn categories_mut<'a, 'i>(&'a mut self, info: &'i MapPackInfo) -> impl Iterator<Item = (CategoryPath, &'a mut LoadedCategory)> + 'i where
        'a: 'i,
    {
        let categories = Arc::make_mut(&mut self.categories);
        info.categories().zip(categories.iter_mut())
    }
    pub fn category_at<'a>(&'a self, path: CategoryPath<&'_ MapPackInfo>) -> Option<&'a LoadedCategory> {
        let info = path.root;
        info.category_index(path.unscope())
            .and_then(|i| self.categories.get(i.path as usize))
    }
    pub fn category_at_mut<'a>(&'a mut self, path: CategoryPath<&'_ MapPackInfo>) -> Option<&'a mut LoadedCategory> {
        let info = path.root;
        info.category_index(path.unscope())
            .and_then(|i| Arc::make_mut(&mut self.categories).get_mut(i.path as usize))
    }

    /// Only updates default flags
    ///
    /// [self.categories] are dirty and require further processing unless `Ok(true)`
    pub fn update_category_config(&mut self, info: &MapPackInfo, categories: &PackCategoryInfo, config: &PackConfig) -> Result<bool, CategorySet> {
        let mut damage = match self.categories.len() {
            loaded if info.category_count() != loaded => {
                self.categories = iter::repeat(LoadedCategory::INVALID).take(info.category_count()).collect();
                None
            },
            _ => Some(CategorySet::default()),
        };

        let mut loaded: Result<&mut [LoadedCategory], &mut Arc<[LoadedCategory]>> = Err(&mut self.categories);
        for (i, path) in info.categories().enumerate() {
            let Some(prev) = match &mut loaded {
                Ok(c) => &c[..],
                Err(c) => &c[..],
            }.get(i) else { continue };
            let prev_defaults = prev.visibility & VisibilityFlags::DEFAULTS;
            let defaults = categories.visibility.get_for(path)
                .unwrap_or(VisibilityFlags::TOGGLES);
            let deviation = config.visibility_deviation_for(path);
            let default_toggles = defaults ^ deviation;
            let defaults = default_toggles.toggles_to_default();
            let is_override_clean = !config.visibility_overrides.contains(path) || prev.visibility & VisibilityFlags::TOGGLES == default_toggles;
            if damage.is_some() && defaults == prev_defaults && is_override_clean {
                continue
            }

            let out = unsafe {
                match (mem::replace(&mut loaded, Ok(&mut [])), &mut loaded) {
                    (Ok(loaded), Ok(out)) => {
                        *out = loaded;
                        out
                    },
                    (Err(loaded), Ok(out)) => {
                        *out = Arc::make_mut(loaded);
                        out
                    },
                    #[cfg(debug_assertions)]
                    (_, Err(..)) => unreachable!(),
                    #[cfg(not(debug_assertions))]
                    (_, Err(..)) => continue,
                }.get_unchecked_mut(i)
            };
            out.visibility.set_defaults(defaults);

            if let Some(damage) = &mut damage {
                damage.insert(path);
            }
        }

        match damage {
            Some(mut damage) => {
                // not necessarily likely that multiple changes occur at once, but...
                // we only care about the root-most changes since they propagate down
                let mut redundant_roots = Vec::new();
                for damaged in damage.paths() {
                    let is_redundant = categories.parents_of(damaged)
                        .any(|p| damage.contains(p));
                    if is_redundant {
                        redundant_roots.push(damaged);
                    }
                }
                for redundant in redundant_roots {
                    damage.remove(redundant);
                }

                if !damage.is_empty() {
                    Err(damage)
                } else {
                    Ok(true)
                }
            },
            None => Ok(false),
        }
    }

    pub fn refresh_categories(&mut self, info: &MapPackInfo, categories: &PackCategoryInfo, config: &PackConfig, damage: Option<&CategorySet>) {
        let default_roots = damage.is_none().then(|| categories.root_paths());
        let roots = damage.into_iter()
            .flat_map(|damage| damage.paths())
            .chain(default_roots.into_iter().flatten());

        // roots should be independent subtrees, but just in case..
        let mut children: VecDeque<_> = roots.map(|root_path| (root_path, config.visibility_overrides.contains(root_path), None)).collect();

        let pack_default = VisibilityFlags::TOGGLES;
        let loaded = Arc::make_mut(&mut self.categories);
        while let Some((path, parent_is_override, parent_vis)) = children.pop_front() {
            let Some(index) = info.category_index(path) else {
                // rest of tree should be irrelevant
                continue
            };
            let is_override = config.visibility_overrides.contains(path);
            let default_vis = loaded.get(index.path as usize)
                .map(|cat| cat.visibility.default_toggles());
            let visibility = match is_override {
                true => default_vis,
                false => {
                    let inherited = parent_vis.or_else(|| categories.parent_of(path)
                        .map(|parent| info.category_index(parent).and_then(|i| loaded.get(i.path as usize))
                            .map(|parent| parent.visibility & VisibilityFlags::TOGGLES)
                            .or_else(|| categories.visibility.get_for(parent))
                        ).unwrap_or(default_vis)
                    );
                    match parent_is_override {
                        true => inherited/*.or(default_vis)*/,
                        false => inherited.map(|inh|
                            (inh & default_vis.unwrap_or(VisibilityFlags::TOGGLES) & VisibilityFlags::TOGGLE)
                            | (default_vis.unwrap_or(VisibilityFlags::TOGGLES) & !VisibilityFlags::TOGGLE)
                        ),
                    }
                },
            }.unwrap_or(pack_default);
            if let Some(loaded) = loaded.get_mut(index.path as usize) {
                loaded.visibility.set_toggles(visibility);
            }
            children.extend(categories.children_of(path).map(|c| (c, is_override, Some(visibility))));
        }
    }

    pub fn apply_category_visibility(&mut self, info: &MapPackInfo, categories: &PackCategoryInfo, damage: Option<&CategorySet>) {
        let range = 0..info.category_max_count();
        let mut category_state: BitVec = BitVec::with_capacity(range.end as usize);
        category_state.resize(range.end as usize, true);
        for (path, cat) in self.categories(info) {
            if path.path >= range.end {
                log::error!("unexpected {path} out of range");
                continue
            }
            category_state.set(path.path as usize, cat.visibility.is_visible());
        }
        let all_leaf_categories = || {
            let pois =  self.pois.iter()
                .map(|poi| poi.category());
            let trails =  self.trails.iter()
                .map(|trail| trail.category());
            pois.chain(trails).filter_map(|c| c)
        };
        let damaged = damage.into_iter()
            .flat_map(|d| d.iter().map(CategoryPath::with_path))
            .chain(damage.is_none().then(all_leaf_categories).into_iter().flatten());
        for damaged in damaged {
            let mut visible = true;
            for parent in iter::once(damaged).chain(categories.parent_of(damaged)) {
                let idx = parent.path as usize;
                let Some(state) = category_state.get(idx).map(|b| *b) else {
                    log::error!("unexpected {damaged}<{parent} out of range");
                    continue
                };
                if !state {
                    visible = false;
                    break
                }
            }
            if !visible {
                let idx = damaged.path as usize;
                if idx >= category_state.len() {
                    log::error!("unexpected {damaged} out of range");
                    continue
                }
                category_state.set(idx, false);
            }
        }
        let is_damaged = |path: CategoryPath| damage
            .map(|d| d.contains(path))
            .unwrap_or(true);

        let dirty_pois = self.pois_mut(info)
            .filter(|(_, poi)| poi.category().map(is_damaged).unwrap_or(false));
        for (_path, poi) in dirty_pois {
            let Some(state) = category_state.get(poi.category_path().path as usize).map(|b| *b) else { continue };
            poi.visibility.set(VisibilityFlags::TOGGLE, state);
        }

        let dirty_trails = self.trails_mut(info)
            .filter(|(_, trail)| trail.category().map(is_damaged).unwrap_or(false));
        for (_path, trail) in dirty_trails {
            let Some(state) = category_state.get(trail.category_path().path as usize).map(|b| *b) else { continue };
            trail.visibility.set(VisibilityFlags::TOGGLE, state);
        }
    }
}

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
    fn as_bitslice(&self) -> &BitSlice<Self::Repr, Lsb0> {
        unsafe {
            self.as_bits().view_bits().get_unchecked(..Self::BIT_WIDTH)
        }
    }
    fn as_bitslice_mut(&mut self) -> &mut BitSlice<Self::Repr, Lsb0> {
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
