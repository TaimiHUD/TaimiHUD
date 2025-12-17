use {
    crate::controller::pathing::{
        registry::{
            LoadedPoiIndex,
            LoadedPoiPath,
            LoadedTrailIndex,
            LoadedTrailPath,
            LoadedTrailSectionPath,
            PackInfoSignature,
            PackVecOf,
        },
        space::DrawSpace,
        state::{LoadedMapInfo, LoadedMaps, LoadedPacks},
    },
    bvh::{aabb, bvh::Bvh},
    glamour::{Box3, Point3},
    std::{
        collections::{BTreeMap, BTreeSet},
        mem,
        ops,
    },
    taimi_hoard::{
        collections::slice_offset_from,
        flags::BitSet,
        iters::IterExt as _,
        loc::{indexed::IndexedList, LocationMut, LocationRef},
    },
    taimi_meta::{
        coords::vec_eq,
        packs::{
            id::{MarkerId, MarkerIndex, MarkerPath},
            MapIndex,
            PackIndex,
            PackMapPath,
            PackRegistryNs,
            TrailSectionPath,
        },
        spatial::{box3aabb, irrelevant_box3, BvhShape, MintConv},
    },
};

#[derive(Clone)]
pub struct SpacePack {
    pub info_sig: PackInfoSignature,
    /// an entry is allocated in [SpaceEntities::entities]
    pub populated_pois: BitSet,
    /// an entry is allocated in [SpaceEntities::entities]
    pub populated_trails: BitSet,
}
impl SpacePack {
    pub fn new() -> Self {
        SpacePack {
            info_sig: PackInfoSignature::EMPTY,
            populated_pois: BitSet::default(),
            populated_trails: BitSet::default(),
        }
    }

    pub fn clear(&mut self) {
        self.info_sig = PackInfoSignature::EMPTY;
        self.clear_entities();
    }
    pub fn clear_entities(&mut self) {
        self.populated_pois.clear();
        self.populated_trails.clear();
    }

    pub fn mark_unpopulated(&mut self, marker: MarkerIndex) -> bool {
        match marker.namespace() {
            MarkerIndex::NS_POI => {
                let idx = marker.index_poi_unchecked();
                self.populated_pois.remove_at(idx)
            },
            MarkerIndex::NS_TRAIL => {
                let idx = marker.trail_index_unchecked();
                self.populated_trails.remove_at(idx)
            },
            _ => None,
        }
        .unwrap_or(false)
    }
    pub fn mark_populated(&mut self, marker: MarkerIndex) -> bool {
        match marker.namespace() {
            MarkerIndex::NS_POI => {
                let idx = marker.index_poi_unchecked();
                self.populated_pois.insert_at(idx)
            },
            MarkerIndex::NS_TRAIL => {
                let idx = marker.trail_index_unchecked();
                self.populated_trails.insert_at(idx)
            },
            _ => false,
        }
    }
    pub fn is_populated(&self, marker: MarkerIndex) -> Option<bool> {
        match marker.namespace() {
            MarkerIndex::NS_POI => {
                let idx = marker.index_poi_unchecked();
                self.populated_pois.get_at(idx)
            },
            MarkerIndex::NS_TRAIL => {
                let idx = marker.trail_index_unchecked();
                self.populated_trails.get_at(idx)
            },
            _ => None,
        }
    }
    pub fn any_populated(&self) -> bool {
        self.populated_pois.any() || self.populated_trails.any()
    }
    pub fn populated_pois(&self) -> impl Iterator<Item = LoadedPoiPath> + '_ {
        self.populated_pois
            .iter_ones()
            .lazy_map(|i| LoadedPoiPath::with_path(i as LoadedPoiIndex))
    }
    pub fn populated_trails(&self) -> impl Iterator<Item = LoadedTrailPath> + '_ {
        self.populated_trails
            .iter_ones()
            .lazy_map(|i| LoadedTrailPath::with_path(i as LoadedTrailIndex))
    }
}
impl Default for SpacePack {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct SpaceEntity {
    pub id: MarkerId,
    pub bounds: aabb::Aabb<f32, 3>,
}
impl SpaceEntity {
    pub fn new(id: MarkerId, bounds: Box3<DrawSpace>) -> Self {
        Self { id, bounds: box3aabb(bounds) }
    }

    pub fn invalid() -> Self {
        Self {
            id: MarkerId::EMPTY,
            bounds: box3aabb(irrelevant_box3::<DrawSpace>()),
        }
    }
    pub fn is_invalid(&self) -> bool {
        self.id.uuid.is_nil()
    }
}
impl aabb::Bounded<f32, 3> for SpaceEntity {
    fn aabb(&self) -> aabb::Aabb<f32, 3> {
        self.bounds
    }
}
/// associated data with a [SpaceEntity] but not strictly required for bvh
#[derive(Clone)]
pub struct SpaceEntityExtra {
    /// could consider moving this here and just use the index/offset into here?
    #[cfg(todo)]
    pub id: MarkerId,
    pub position: Point3<DrawSpace>,
}
impl SpaceEntityExtra {
    pub fn invalid() -> Self {
        Self { position: Point3::INFINITY }
    }
    pub fn is_invalid(&self) -> bool {
        self.position.x.is_infinite()
    }
}
#[derive(Clone)]
pub struct SpaceEntities {
    pub entities: Vec<BvhShape<SpaceEntity>>,
    pub extra: Vec<SpaceEntityExtra>,
}
impl SpaceEntities {
    pub fn new() -> Self {
        Self {
            entities: Vec::new(),
            extra: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.entities.clear();
        self.extra.clear();
    }

    pub fn needs_rebuild(&self) -> bool {
        self.extra.len() != self.entities.len()
    }

    #[inline]
    fn is_residue(e: &BvhShape<SpaceEntity>, check_bvh: Option<&Bvh<f32, 3>>) -> bool {
        e.is_invalid() && check_bvh.map(|bvh| e.is_bh_removed_from(bvh)).unwrap_or(true)
    }
    /// number of unallocated entries at end of list
    pub fn trailing_residue(&self, check_bvh: Option<&Bvh<f32, 3>>) -> usize {
        self.entities
            .iter()
            .rev()
            .take_while(|e| Self::is_residue(e, check_bvh))
            .count()
    }
    pub fn trim_trailing(&mut self, check_bvh: Option<&Bvh<f32, 3>>) {
        let removed = self.trailing_residue(check_bvh);
        if removed > 0 {
            let residue_start = self.entities.len() - removed;
            match () {
                #[cfg(todo = "unnecessary")]
                _ => self.drain_range(residue_start..),
                _ => self.truncate(residue_start),
            }
        }
    }
    pub fn truncate(&mut self, new_len: usize) {
        if self.entities.len() > new_len {
            self.entities.truncate(new_len);
        }
        if self.extra.len() > new_len {
            self.extra.truncate(new_len);
        }
    }
    /// will ruin indices
    pub fn prune_residue(&mut self) {
        self.trim_trailing(None);
        let mut iter = 0..self.entities.len();
        while let Some(i) = iter.next() {
            while self.entities.get(i).map(|e| e.is_invalid()).unwrap_or(false) {
                self.swap_remove(i);
                if iter.next_back().is_none() {
                    break
                }
            }
        }
    }
    pub fn drain_range<R: ops::RangeBounds<usize>>(&mut self, range: R) {
        use ops::Bound;
        let (start, end) = (range.start_bound().cloned(), range.end_bound().cloned());
        drop(self.entities.drain(range));
        let start = match start {
            Bound::Included(s) => Some(s),
            Bound::Excluded(s) => Some(s + 1),
            Bound::Unbounded => None,
        };
        let extra_inrange_start = start.map(|s| self.extra.get(s).is_some());
        if extra_inrange_start.unwrap_or(true) {
            let end = match end {
                Bound::Included(e) => e + 1,
                Bound::Excluded(e) => e,
                Bound::Unbounded => usize::MAX,
            }
            .min(self.extra.len());
            drop(self.extra.drain(start.unwrap_or(0)..end));
        }
    }
    pub fn swap_remove(&mut self, index: usize) {
        self.entities.swap_remove(index);
        if self.extra.len() > index {
            self.extra.swap_remove(index);
        }
    }
    pub fn rebuild_extra(
        &mut self,
        dirty_indices: Option<&mut dyn Iterator<Item = usize>>,
        map_info: &LoadedMapInfo,
        maps: &LoadedMaps,
    ) {
        let entities_count = match () {
            #[cfg(todo = "unnecessary")]
            _ => self.entities.len() - self.trailing_residue(),
            _ => self.entities.len(),
        };
        self.extra.resize_with(entities_count, SpaceEntityExtra::invalid);
        let mut range;
        let (dirty_check, indices) = match dirty_indices {
            Some(indices) => (false, indices),
            None => {
                range = 0..self.extra.len();
                (true, &mut range as &mut dyn Iterator<Item = usize>)
            },
        };
        for i in indices {
            let Some(extra) = self.extra.get_mut(i) else { continue };
            if dirty_check && !extra.is_invalid() {
                continue
            }
            let Some(e) = self.entities.get_mut(i) else { continue };
            if dirty_check && e.is_bh_removed() {
                continue
            }
            let Some(path) = e.value.id.marker_path::<PackMapPath>() else { continue };
            let map = maps.lookup_with_info(map_info, &path.root);
            match path.path.namespace() {
                MarkerIndex::NS_POI => {
                    let lpath: LoadedPoiPath = LoadedPoiPath::with_path(path.path.index_poi_unchecked());
                    let lpoi = map.and_then(|(map, _i)| map.lpois().lookup_ref(&lpath));
                    if let Some(lpoi) = lpoi {
                        extra.position = lpoi.position();
                    }
                },
                MarkerIndex::NS_TRAIL => {
                    // no need to look up pos since we can infer it from the bounds...
                    extra.position = MintConv::from_nalg(e.bounds.center());
                },
                _ => (),
            }
        }
    }

    pub fn retain<
        F: FnMut(
            usize,
            &mut BvhShape<SpaceEntity>,
            Option<&mut SpaceEntityExtra>,
            Option<(MarkerPath<PackMapPath>, &mut SpacePack)>,
        ) -> bool,
    >(
        &mut self,
        pack_data: &mut IndexedList<PackRegistryNs, PackIndex, [SpacePack]>,
        mut cond: F,
    ) -> BitSet {
        let mut removed: BitSet = Default::default();
        removed.reserve_exact(self.entities.len());
        removed.extend(self.entities.iter_mut().enumerate().filter_map(|(i, e)| {
            let mut pack_data = e
                .value
                .id
                .marker_path::<PackMapPath>()
                .and_then(|path| pack_data.lookup_mut(&path.root.root).map(|d| (path, d)));
            let extra = self.extra.get_mut(i);
            match cond(i, e, extra, pack_data.as_mut().map(|(p, pd)| (*p, &mut **pd))) {
                #[cfg(todo = "unnecessary")]
                _ if e.is_invalid() => None,
                false => {
                    if let Some((path, pack_data)) = pack_data {
                        pack_data.mark_unpopulated(path.path);
                    }
                    e.value = SpaceEntity::invalid();
                    Some(i)
                },
                true => None,
            }
        }));
        #[cfg(todo = "unnecessary")]
        for i in removed.iter_ones() {
            let Some(extra) = self.extra.get_mut(i) else { continue };
            *extra = SpaceEntityExtra::invalid();
        }
        removed
    }
    pub fn invalidate(
        &mut self,
        pack_data: &mut IndexedList<PackRegistryNs, PackIndex, [SpacePack]>,
        index: usize,
    ) {
        let Some(e) = self.entities.get_mut(index) else { return };
        let pack_data = e
            .value
            .id
            .marker_path::<PackMapPath>()
            .and_then(|path| pack_data.lookup_mut(&path.root.root).map(|d| (path, d)));
        if let Some((path, pack_data)) = pack_data {
            pack_data.mark_unpopulated(path.path);
        }
        e.value = SpaceEntity::invalid();
        #[cfg(todo = "unnecessary")]
        if let Some(extra) = self.extra.get_mut(i) {
            *extra = SpaceEntityExtra::invalid();
        }
    }
    pub fn deactivate_entities_inplace(
        &mut self,
        _pack_data: &mut IndexedList<PackRegistryNs, PackIndex, [SpacePack]>,
        bvh: &mut Bvh<f32, 3>,
        indices: &mut dyn Iterator<Item = usize>,
    ) {
        for i in indices {
            let Some(e) = self.entities.get_mut(i) else {
                log::warn!("cannot deactivate missing entity #{i}");
                continue
            };
            #[cfg(todo = "unnecessary")]
            let pack_data = e
                .value
                .id
                .marker_path::<PackMapPath>()
                .and_then(|path| pack_data.lookup_mut(&path.root.root).map(|d| (path, d)));
            if e.is_bh_removed() {
                continue
            }
            bvh.remove_shape(&mut self.entities, i, false);
            let e = unsafe { self.entities.get_unchecked_mut(i) };
            e.set_bh_removed();
        }
    }
}
impl Extend<(MarkerId, Box3<DrawSpace>, Point3<DrawSpace>)> for SpaceEntities {
    fn extend<T: IntoIterator<Item = (MarkerId, Box3<DrawSpace>, Point3<DrawSpace>)>>(&mut self, iter: T) {
        if self.entities.len() != self.extra.len() {
            log::error!(
                "SpaceEntities len({}) mismatches extra({})",
                self.entities.len(),
                self.extra.len()
            );
            return
        }
        let iter = iter.into_iter();
        let (min, max) = iter.size_hint();
        let cap = max.unwrap_or(min);
        self.entities.reserve(cap);
        self.extra.reserve(cap);
        for (id, bounds, position) in iter {
            let entity = SpaceEntity { id, bounds: box3aabb(bounds) };
            self.entities.push(BvhShape::new(entity));
            self.extra.push(SpaceEntityExtra { position });
        }
    }
}

#[derive(Clone)]
pub struct SpacePackCollection {
    pub map_id: Option<MapIndex>,
    pub loaded_packs: PackVecOf<SpacePack>,
    pub render_entities: SpaceEntities,
    pub bvh: Bvh<f32, 3>,
}

impl SpacePackCollection {
    pub fn new() -> SpacePackCollection {
        SpacePackCollection {
            map_id: None,
            loaded_packs: Default::default(),
            render_entities: SpaceEntities::new(),
            bvh: Bvh { nodes: Vec::new() },
        }
    }

    #[cfg(todo)]
    pub fn needs_rebuild(&self, map_id: MapIndex, packs: &LoadedPacks, maps: &LoadedMaps) -> bool {
        self.map_id != Some(map_id)
            || !packs.sigs_match(self.loaded_packs.values().map(|p| p.info_sig))
            || self
                .loaded_packs
                .iter()
                .any(|(path, pd)| pd.is_dirty(maps.lookup_ref(&path.rel(map_id))))
    }

    #[cfg(todo)]
    pub fn rebuild_entities_if_dirty(
        &mut self,
        map_id: MapIndex,
        packs: &LoadedPacks,
        map_info: &LoadedMapInfo,
        maps: &LoadedMaps,
    ) {
        if self.needs_rebuild(map_id, packs, maps) {
            self.rebuild_entities(map_id, packs, map_info, maps);
        }
    }
    /// hidden = activated && !visible, dirty = boundschanged || (visible && !activated)
    fn prepare_entity_update(
        &mut self,
        map_id: MapIndex,
        packs: &LoadedPacks,
        map_info: &LoadedMapInfo,
        maps: &LoadedMaps,
    ) -> EntityUpdateReport {
        let mut report = EntityUpdateReport::new(map_id);
        self.loaded_packs
            .data
            .resize_with(packs.packs.len(), SpacePack::default);
        if self.map_id != report.map_id {
            self.clear();
            self.prepare_entity_population(map_id, packs, map_info, maps);
            return report
        }

        for ((_path, pack), pack_data) in packs.packs.iter().zip(self.loaded_packs.values_mut()) {
            if pack.is_loaded() && pack.info.has_map(map_id) && pack_data.info_sig == pack.info.sig {
                continue
            }
            pack_data.clear();
        }
        let pd = self.loaded_packs.map_mut_as_slice();
        let cx = (maps, map_info, &self.bvh);
        report.retain_entities(&mut self.render_entities, pd, cx);
        self.prepare_entity_population(map_id, packs, map_info, maps);
        report
    }
    fn prepare_entity_population(
        &mut self,
        map_id: MapIndex,
        packs: &LoadedPacks,
        map_info: &LoadedMapInfo,
        maps: &LoadedMaps,
    ) {
        for ((path, _pack), pack_data) in packs.packs.iter().zip(self.loaded_packs.values_mut()) {
            let path = path.rel(map_id);
            let Some((map, _map_info)) = maps.lookup_with_info(map_info, &path) else {
                continue
            };
            pack_data
                .populated_pois
                .extend_to_size(map.lpois().len() as usize, false);
            pack_data
                .populated_trails
                .extend_to_size(map.ltrails().len() as usize, false);
        }
    }
    /// `Err(true)` if bvh requires rebuild, `Err(false)` if mutated in-place
    pub fn rebuild_entities(
        &mut self,
        map_id: MapIndex,
        packs: &LoadedPacks,
        map_info: &LoadedMapInfo,
        maps: &LoadedMaps,
    ) -> Result<(), bool> {
        let EntityUpdateReport {
            removed,
            mut unallocated,
            mut dirty,
            hidden,
            ..
        } = self.prepare_entity_update(map_id, packs, map_info, maps);
        self.map_id = Some(map_id);

        let prev_entities_end = self.render_entities.entities.len();
        for ((path, pack), pack_data) in packs.packs.iter().zip(self.loaded_packs.values_mut()) {
            let path = path.rel(map_id);
            let map = maps.lookup_with_info(map_info, &path);
            if map.is_some() || !pack.info.has_map(map_id) {
                let _info_sig_prev = mem::replace(&mut pack_data.info_sig, pack.info.sig);
            }
            let Some((map, map_info)) = map else {
                pack_data.clear_entities();
                continue
            };
            // to iter of (marker_id, bounds, position)
            let pois = map
                .lpois()
                .into_iter()
                .zip(pack_data.populated_pois.iter_mut())
                .filter(|((_, lpoi), _)| lpoi.visibility.is_visible())
                .filter_map(|(v, mut populated)| match mem::replace(&mut *populated, true) {
                    false => Some(v),
                    true => None,
                })
                .filter_map(move |(lpoi_path, lpoi)| {
                    let marker_path: MarkerPath = lpoi_path.pivot_to();
                    let mpath = path.rel(marker_path.path);
                    let mid = MarkerId::for_marker(mpath);
                    Some((mid, lpoi.bounds()))
                });

            let trails = map.ltrails().into_iter().zip(map_info.trail_info.data.iter());
            let trails = trails
                .zip(pack_data.populated_trails.iter_mut())
                .filter(|(((_, ltrail), _), _)| ltrail.visibility.is_visible())
                .filter_map(|(v, mut populated)| match mem::replace(&mut *populated, true) {
                    false => Some(v),
                    true => None,
                })
                .flat_map(move |((ltrail_path, _ltrail), trail_info)| {
                    trail_info
                        .section_bounds()
                        .filter(move |(_, section, _)| section.is_visible())
                        .map(move |(section_path, _, bounds)| {
                            let ts_path: LoadedTrailSectionPath =
                                LoadedTrailSectionPath::with_path(ltrail_path.rel(section_path));
                            let marker_path: MarkerPath = ts_path.pivot_to();
                            let mpath = path.rel(marker_path.path);
                            let mid = MarkerId::for_marker(mpath);
                            (mid, bounds)
                        })
                });
            for (mid, bounds) in pois.chain(trails) {
                let entity = SpaceEntity::new(mid, bounds);
                let entity = if let Some(i) = unallocated.pop_first() {
                    if let Some(e) = self.render_entities.entities.get_mut(i) {
                        debug_assert!(e.is_bh_removed_from(&self.bvh));
                        e.value = entity;
                        dirty.insert(mid, i);
                        None
                    } else {
                        // TODO: could also start to reclaim from removed - just make sure to remove prior to adding
                        Some(entity)
                    }
                } else {
                    Some(entity)
                };
                if let Some(entity) = entity {
                    self.render_entities.entities.push(BvhShape::new_removed(entity));
                }
            }
        }
        let new_entities = prev_entities_end..self.render_entities.entities.len();
        let dirty_indices = dirty.values().copied().chain(new_entities.clone());

        // update extra data that's been invalidated
        self.render_entities
            .rebuild_extra(Some(&mut dirty_indices.clone()), map_info, maps);

        let removed_count = removed.count_ones();
        let partial_rebuild = {
            let dead_space = removed_count + unallocated.len();
            let wasted = dead_space + hidden.len();
            let allocated = self.render_entities.entities.len();
            let used = allocated.saturating_sub(wasted);
            let threshold = allocated / 3;
            let used_threshold = used / 5;
            let update_count = dirty.len() + new_entities.len();
            wasted < threshold && update_count < used_threshold
        };
        let full_rebuild = match partial_rebuild {
            _ if self.bvh.nodes.len() <= 1 => true,
            true if removed_count == 0 => false,
            _ if dirty.is_empty() && new_entities.is_empty() => false,
            _ => true,
        };

        if full_rebuild {
            // since we're doing a full rebuild anyway, free up the filtered items
            for (_mid, &i) in hidden.iter() {
                self.render_entities
                    .invalidate(self.loaded_packs.map_mut_as_slice(), i);
            }
            self.signal_bvh_rebuild();
        } else {
            #[cfg(todo)]
            let deactivations = dirty.values().copied().chain(removed.iter_ones());
            let mut removed = removed;
            removed.extend(dirty.values().copied());
            let deactivations = removed.iter_ones().rev();
            // TODO: may be able to use remove_swap here?
            self.render_entities.deactivate_entities_inplace(
                self.loaded_packs.map_mut_as_slice(),
                &mut self.bvh,
                &mut { deactivations },
            );
            for i in dirty_indices.clone() {
                self.bvh.add_shape(&mut self.render_entities.entities, i);
            }
            self.render_entities.trim_trailing(Some(&self.bvh));
        }

        if full_rebuild || self.render_entities.entities.is_empty() != self.bvh.nodes.is_empty() {
            Err(true)
        } else {
            match !dirty.is_empty() || !new_entities.is_empty() || removed_count > 0 {
                true => Err(false),
                false => Ok(()),
            }
        }
    }
    /// TODO: check map state sigs or something idk what needs to change
    pub fn entities_dirty(&self, map_id: MapIndex, packs: &LoadedPacks) -> bool {
        self.render_entities.needs_rebuild()
    }

    #[cfg(todo)]
    pub fn needs_bvh_rebuild(&self) -> bool {
        let entity_count = self.render_entities.entities.len();
        if entity_count > 0 && self.bvh.nodes.is_empty() {
            return true
        }

        let bvh_leaf_count = self
            .bvh
            .nodes
            .iter()
            .filter(|node| matches!(node, bvh::bvh::BvhNode::Leaf { .. }))
            .count();

        entity_count != bvh_leaf_count
    }

    pub fn clear_bvh(&mut self) {
        self.bvh = Bvh { nodes: Vec::new() };
    }
    pub fn rebuild_bvh(&mut self) {
        self.render_entities.prune_residue();
        if self.render_entities.entities.is_empty() {
            for pack in self.loaded_packs.values_mut() {
                pack.clear_entities();
            }
            self.clear_bvh();
            return
        }
        self.bvh = Bvh::build(&mut self.render_entities.entities);
    }
    fn signal_bvh_rebuild(&mut self) {
        if !self.render_entities.entities.is_empty() {
            self.clear_bvh();
        }
    }

    pub fn clear(&mut self) {
        for pack in self.loaded_packs.values_mut() {
            pack.clear();
        }
        self.render_entities.clear();
        self.bvh = Bvh { nodes: Vec::new() };
        self.map_id = None;
    }
    pub fn is_empty(&self) -> bool {
        self.map_id.is_none() || (self.bvh.nodes.is_empty() && self.render_entities.entities.is_empty())
    }

    #[inline]
    pub fn bvh_traverse_shapes<'a, Q: aabb::IntersectsAabb<f32, 3>>(
        &'a self,
        query: &'a Q,
    ) -> bvh::bvh::BvhTraverseIterator<'a, 'a, f32, 3, Q, BvhShape<SpaceEntity>> {
        self.bvh.traverse_iterator(query, &self.render_entities.entities)
    }
    #[inline]
    pub fn bvh_traverse<'a, Q: aabb::IntersectsAabb<f32, 3>>(
        &'a self,
        query: &'a Q,
    ) -> impl Iterator<Item = (usize, &'a MarkerId)> + 'a {
        let shapes = &self.render_entities.entities[..];
        self.bvh.traverse_iterator(query, shapes).map(move |shape| {
            let idx = slice_offset_from(shapes, shape);
            (idx, &shape.value.id)
        })
    }
}
impl Default for SpacePackCollection {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Default)]
struct EntityUpdateReport {
    removed: BitSet,
    unallocated: BTreeSet<usize>,
    /// boundschanged || (visible && !activated)
    dirty: BTreeMap<MarkerId, usize>,
    /// activated && !visible
    hidden: BTreeMap<MarkerId, usize>,
    map_id: Option<MapIndex>,
}
type EntityRetainContext<'a> = (&'a LoadedMaps, &'a LoadedMapInfo, &'a Bvh<f32, 3>);
impl EntityUpdateReport {
    fn new(map_id: MapIndex) -> Self {
        Self {
            map_id: Some(map_id),
            ..Self::default()
        }
    }
    fn retain_entities(
        &mut self,
        entities: &mut SpaceEntities,
        pack_data: &mut IndexedList<PackRegistryNs, PackIndex, [SpacePack]>,
        cx: EntityRetainContext<'_>,
    ) {
        let removed = entities.retain(pack_data, |i, e, x, pd| self.retain_entity(i, e, x, pd, cx));
        if !self.removed.is_empty() {
            log::debug!("EntityUpdateReport wasn't fresh for retain?");
        }
        self.removed = removed;
    }
    fn retain_entity(
        &mut self,
        i: usize,
        e: &mut BvhShape<SpaceEntity>,
        extra: Option<&mut SpaceEntityExtra>,
        pack_data: Option<(MarkerPath<PackMapPath>, &mut SpacePack)>,
        (maps, map_info, bvh): EntityRetainContext<'_>,
    ) -> bool {
        if e.is_invalid() {
            self.unallocated.insert(i);
            return true
        }
        let Some((map_path, pack_data)) = pack_data else { return true };
        if map_path.root.path == MapIndex::MAX {
            log::debug!("TODO: mapless marker?");
            return true
        }
        match self.map_id {
            Some(map_id) if map_path.root.path != map_id => return false,
            _ => (),
        }
        if pack_data.info_sig.is_empty() {
            return false
        }

        let Some((map, map_info)) = maps.lookup_with_info(map_info, &map_path.root) else {
            log::debug!("TODO: {map_path} info missing for {}", e.id);
            return false
        };
        let (vis, bounds) = match map_path.path.namespace() {
            MarkerIndex::NS_POI => {
                let lpath: LoadedPoiPath = LoadedPoiPath::with_path(map_path.path.index_poi_unchecked());
                let Some(lpoi) = map.lpois().lookup_ref(&lpath) else { return false };
                let bounds = lpoi.bounds();
                let bounds = if !vec_eq(bounds.min.to_array(), e.bounds.min.into())
                    || !vec_eq(bounds.max.to_array(), e.bounds.max.into())
                {
                    Some(bounds)
                } else {
                    None
                };
                (lpoi.visibility, bounds)
            },
            MarkerIndex::NS_TRAIL => {
                let lpath: LoadedTrailPath =
                    LoadedTrailPath::with_path(map_path.path.trail_index_unchecked());
                let seci = map_path.path.trail_index_unchecked();
                let section_path: TrailSectionPath = TrailSectionPath::with_path(seci);
                let Some(ltrail) = map.ltrails().lookup_ref(&lpath) else { return false };
                let Some(tinfo) = map_info.trail_info.lookup_ref(&lpath) else { return false };
                let Some(lsection) = tinfo.sections().lookup_ref(&section_path) else {
                    return false
                };
                if !lsection.is_visible() {
                    return false
                }
                let bounds = &lsection.bounds;
                let bounds = if !vec_eq(bounds.min.to_array(), e.bounds.min.into())
                    || !vec_eq(bounds.max.to_array(), e.bounds.max.into())
                {
                    Some(*bounds)
                } else {
                    None
                };
                (ltrail.visibility, bounds)
            },
            _ => return true,
        };
        let activated = !e.is_bh_removed_from(bvh);
        if !vis.is_visible() {
            if activated {
                self.hidden.insert(e.id.clone(), i);
            }
        } else if !activated {
            self.dirty.insert(e.id.clone(), i);
        }
        if let Some(bounds) = bounds {
            e.bounds = box3aabb(bounds);
            if activated {
                self.dirty.insert(e.id.clone(), i);
            } else if let Some(extra) = extra {
                *extra = SpaceEntityExtra::invalid();
            }
        }
        true
    }
}
