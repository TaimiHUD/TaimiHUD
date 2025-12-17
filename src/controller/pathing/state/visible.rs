use std::collections::VecDeque;
use std::hash::Hash;
use std::{num::NonZero, ops};
use std::sync::Arc;
use std::{iter, mem};
use crate::space::pack::PackSpace;
use crate::{
    controller::pathing::space::{DrawSpace, TrailParams},
    resources::Vertex,
};
#[cfg(deleteme)]
use {
    crate::controller::pathing::{
        state::interactive::InteractivePoi,
        registry::{LoadedPack, PackCategoryInfo, PackConfig},
        filter::MapFilters,
        MapPackInfo,
    },
    taimi_hoard::collections::lru::RecentlyUsed,
};
use taimi_meta::packs::{
    collections::CategorySet,
    CategoryIndex, CategoryPath, MapIndex, PoiIndex, PoiPath, TrailPath,
};
use taimi_hoard::flags::set::{BitFlagForSet, FlagSet};
use bitflags::bitflags;
use bitvec::order::Lsb0;
use bitvec::slice::BitSlice;
use bitvec::{vec::BitVec, view::BitView};
use glam::Vec3Swizzles;
use glamour::{Box3, Point3, Size3, Vector3};
use taimi_meta::{map::MapID, ui::{MapContext, LocalContext}};
use taimi_pack::attributes::keys::Guid;
use taimi_pack::Category;
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
    pub category: CategoryIndex,
    pub visibility: VisibilityFlags,
    pub bounds: Box3<DrawSpace>,
    pub position: Point3<DrawSpace>,
}

impl LoadedPoi {
    pub const INVALID: Self = Self {
        category: CategoryIndex::MAX,
        visibility: VisibilityFlags::empty(),
        position: Point3::INFINITY,
        bounds: Box3::ZERO,
    };

    pub fn from_pack(path: PoiPath, pack: &Pack) -> Self {
        let Some(poi) = pack.pois.get(path.path as usize) else {
            return Self::INVALID
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
        let (position, bounds) = Self::coords_for(poi);

        Self {
            category,
            visibility: visibility.restore_default_toggles(),
            bounds,
            position,
        }
    }

    pub fn coords_for(poi: &Poi) -> (Point3<DrawSpace>, Box3<DrawSpace>) {
        let edge_len = poi.icon_scale();
        let max_diagonal = (edge_len.powi(2) * 2.0).sqrt();
        let pos = Self::position_for(poi);
        let bounds = Box3::from_origin_and_size(pos, Size3::splat(max_diagonal));
        (pos, bounds)
    }

    pub fn offset_for(poi: &Poi) -> Point3<PackSpace> {
        Point3::ZERO.with_y(poi.height_offset())
    }
    pub fn marker_position_for(poi: &Poi) -> Point3<PackSpace> {
        Point3::from_raw(poi.position.into())
    }
    pub fn position_for(poi: &Poi) -> Point3<PackSpace> {
        Self::marker_position_for(poi) + Self::offset_for(poi)
    }

    pub fn is_invalid(&self) -> bool {
        self.category == CategoryIndex::MAX
    }
    pub fn get(&self) -> Option<&Self> {
        match self.is_invalid() {
            false => Some(self),
            true => None,
        }
    }

    pub fn category(&self) -> Option<CategoryPath> {
        match self.is_invalid() {
            false => Some(CategoryPath::with_path(self.category)),
            true => None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct LoadedTrail {
    pub category: CategoryIndex,
    pub visibility: VisibilityFlags,
    pub sections: Option<Arc<[LoadedTrailSection]>>,
    pub scale: f32,
    pub is_wall: bool,
    // TODO: y_offset?
}

impl LoadedTrail {
    pub const INVALID: Self = Self {
        category: CategoryIndex::MAX,
        visibility: VisibilityFlags::empty(),
        sections: None,
        scale: 1.0,
        is_wall: false,
    };

    pub fn from_pack(path: TrailPath, pack: &Pack) -> Self {
        let Some(trail) = pack.trails.get(path.path as usize) else {
            return Self::INVALID
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

        Self {
            category,
            visibility: visibility.restore_default_toggles(),
            is_wall: trail.is_wall(),
            scale: trail.scale(),
            sections: None,
        }
    }

    pub fn populate_data(&mut self, trail_data: &TrailData) -> bool {
        if self.sections.is_some() {
            return false
        }

        let _ = self.sections.insert(trail_data.sections.iter()
            .map(|section| LoadedTrailSection::with_section(section))
            .collect()
        );
        true
    }

    pub fn vertices_for(&self, trail_data: &TrailData, params: &TrailParams) -> LoadedTrailGeometry {
        let section_count = self.sections.as_ref().map(|s| s.len()).unwrap_or(0);
        // TODO
        let y_offset_sig = (self.category as usize) << 24 | section_count;
        Self::vertices_with_data(trail_data, params, self.scale, self.is_wall, params.y_offset_for(y_offset_sig))
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
        let mut y_offsets = Vec::new();
        for (isec, section) in trail_data.sections.iter().enumerate() {
            y_offset = (y_offset - f32::EPSILON * 40.0).max(0.0);

            let prior_count = vertices.len();
            let vertex_count = if section.points.is_empty() {
                log::trace!("Section {isec} is empty.");
                0
            } else {
                if y_offset != 0.0 {
                    y_offsets.push(y_offset);
                }
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
            y_offsets,
        }
    }

    pub fn is_invalid(&self) -> bool {
        self.category == CategoryIndex::MAX
    }
    pub fn get(&self) -> Option<&Self> {
        match self.is_invalid() {
            false => Some(self),
            true => None,
        }
    }

    pub fn category(&self) -> Option<CategoryPath> {
        match self.is_invalid() {
            false => Some(CategoryPath::with_path(self.category)),
            true => None,
        }
    }
}

#[derive(Debug, Clone, Default)]
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

    pub fn bounds_for(section: &TrailSection) -> Box3<PackSpace> {
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

#[derive(Debug, Clone)]
pub struct LoadedTrailGeometry {
    pub vertices: Vec<Vertex>,
    pub section_lengths: Vec<u32>,
    pub y_offsets: Vec<f32>,
}

#[cfg(deleteme)]
#[derive(Debug, Clone)]
pub struct LoadedMapPack {
    pub map_id: NonZero<MapID>,
    pub used: RecentlyUsed,
    pub pois: Box<[LoadedPoi]>,
    pub poi_guids: Arc<[Guid]>,
    pub interactive_pois: Arc<[InteractivePoi]>,
    pub interactive_pois_nearby: BitVec,
    pub trails: Box<[LoadedTrail]>,
    pub trail_guids: Box<[Guid]>,
    pub categories: Arc<[LoadedCategory]>,
    pub filters: MapFilters,
}

#[cfg(deleteme)]
impl LoadedMapPack {
    pub fn empty(map_id: MapIndex) -> Self {
        Self {
            map_id,
            used: RecentlyUsed::DEFAULT,
            interactive_pois: Default::default(),
            interactive_pois_nearby: Default::default(),
            pois: Default::default(),
            poi_guids: Default::default(),
            trails: Default::default(),
            trail_guids: Default::default(),
            categories: Default::default(),
            filters: Default::default(),
        }
    }

    pub fn from_pack(map_id: MapIndex, info: &MapPackInfo, pack: &LoadedPack) -> Self {
        let Some(active) = &pack.active else {
            return Self::empty(map_id)
        };
        let pack = &active.pack;

        let pois = info.pois()
            .map(|path| LoadedPoi::from_pack(path, pack))
            .collect();
        let poi_guids = info.poi_guid_filter(info.pois())
            .map(|path|
                pack.pois.get(path.path as usize).map(|poi| Guid::from(poi.guid)).unwrap_or_default()
            ).collect();
        let interactive_pois = info.pois().enumerate()
            .map(|(i, path)| InteractivePoi::from_pack(i as PoiIndex, path, pack))
            .filter(|ipoi| !ipoi.is_empty())
            .collect();
        let trails = info.trails()
            .map(|path| LoadedTrail::from_pack(path, pack))
            .collect();
        let trail_guids = info.trail_guid_filter(info.trails())
            .map(|path|
                pack.trails.get(path.path as usize).map(|trail| Guid::from(trail.guid)).unwrap_or_default()
            ).collect();
        let filters = MapFilters::from_pack(info, active);

        let mut loaded = Self {
            map_id,
            interactive_pois_nearby: BitVec::new(),
            interactive_pois,
            pois,
            poi_guids,
            trails,
            trail_guids,
            filters,
            categories: Default::default(),
            used: RecentlyUsed::DEFAULT,
        };
        loaded.interactive_pois_nearby.reserve_exact(loaded.interactive_pois.len());

        loaded
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
    pub fn poi_guids<'a, 'i>(&'a self, info: &'i MapPackInfo) -> impl Iterator<Item = (PoiPath, &'a Guid)> + 'i where
        'a: 'i,
    {
        info.poi_guid_filter(info.pois()).zip(self.poi_guids.iter())
    }
    pub fn poi_at<'a>(&'a self, path: PoiPath<&'_ MapPackInfo>) -> Option<&'a LoadedPoi> {
        let info = path.root;
        info.poi_index(path.unscope())
            .and_then(|i| self.pois.get(i as usize))
    }
    pub fn poi_at_mut<'a>(&'a mut self, path: PoiPath<&'_ MapPackInfo>) -> Option<&'a mut LoadedPoi> {
        let info = path.root;
        info.poi_index(path.unscope())
            .and_then(|i| self.pois.get_mut(i as usize))
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
    pub fn trail_guids<'a, 'i>(&'a self, info: &'i MapPackInfo) -> impl Iterator<Item = (TrailPath, &'a Guid)> + 'i where
        'a: 'i,
    {
        info.trail_guid_filter(info.trails()).zip(&self.trail_guids)
    }
    pub fn trail_at<'a>(&'a self, path: TrailPath<&'_ MapPackInfo>) -> Option<&'a LoadedTrail> {
        let info = path.root;
        info.trail_index(path.unscope())
            .and_then(|i| self.trails.get(i as usize))
    }
    pub fn trail_at_mut<'a>(&'a mut self, path: TrailPath<&'_ MapPackInfo>) -> Option<&'a mut LoadedTrail> {
        let info = path.root;
        info.trail_index(path.unscope())
            .and_then(|i| self.trails.get_mut(i as usize))
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
            .and_then(|i| self.categories.get(i as usize))
    }
    pub fn category_at_mut<'a>(&'a mut self, path: CategoryPath<&'_ MapPackInfo>) -> Option<&'a mut LoadedCategory> {
        let info = path.root;
        info.category_index(path.unscope())
            .and_then(|i| Arc::make_mut(&mut self.categories).get_mut(i as usize))
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
            let default_vis = loaded.get(index as usize)
                .map(|cat| cat.visibility.default_toggles());
            let visibility = match is_override {
                true => default_vis,
                false => {
                    let inherited = parent_vis.or_else(|| categories.parent_of(path)
                        .map(|parent| info.category_index(parent).and_then(|i| loaded.get(i as usize))
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
            if let Some(loaded) = loaded.get_mut(index as usize) {
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
            let Some(state) = category_state.get(poi.category as usize).map(|b| *b) else { continue };
            poi.visibility.set(VisibilityFlags::TOGGLE, state);
        }

        let dirty_trails = self.trails_mut(info)
            .filter(|(_, trail)| trail.category().map(is_damaged).unwrap_or(false));
        for (_path, trail) in dirty_trails {
            let Some(state) = category_state.get(trail.category as usize).map(|b| *b) else { continue };
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

    pub fn from_pack_category(category: &Category) -> Self {
        let mut flags = Self::from_attributes(&category.marker_attributes);
        if category.default_toggle() {
            flags.insert(Self::TOGGLE);
        }
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
