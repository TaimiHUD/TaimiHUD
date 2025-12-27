use {
    crate::{
        controller::pathing::{
            registry::{PackVecOf, PackInfoSignature, LoadedTrailSectionPath},
            space::DrawSpace,
            shared::SharedGameplayMap,
            state::{LoadedPacks, LoadedMaps, LoadedMapInfo},
        },
        space::render_list::{MapFrustum, RenderEntity, RenderId, RenderList, RenderListBuilder},
    },
    taimi_hoard::collections::slice_offset_from,
    taimi_hoard::loc::LocationRef,
    bitvec::vec::BitVec,
    taimi_meta::{
        spatial::{box3aabb, irrelevant_box3, BvhShape},
        packs::{id::{IdVariant, MarkerId, MarkerIndex, MarkerPath}, MapIndex, PackIndex, PackPath, PoiIndex, TrailIndex, TrailSectionIndex},
    },
    glamour::{Box3, Point3},
    std::{mem, sync::Arc, ops},
    bvh::{aabb, bvh::Bvh}
};

#[derive(Clone)]
pub struct SpacePack {
    pub info_sig: PackInfoSignature,
    // Internal rendering data.
    #[cfg(todo)]
    pub render_list_bookmark: Option<usize>,
    #[cfg(todo)]
    poi_bookmark: usize,
}
impl SpacePack {
    pub fn new() -> Self {
        SpacePack {
            info_sig: PackInfoSignature::EMPTY,
            #[cfg(todo)]
            render_list_bookmark: Default::default(),
            #[cfg(todo)]
            poi_bookmark: Default::default(),
        }
    }

    pub fn clear(&mut self) {
        self.info_sig = PackInfoSignature::EMPTY;
        #[cfg(todo)]
        {
            self.render_list_bookmark = None;
            self.poi_bookmark = 0;
        }
    }
}
impl Default for SpacePack {
    fn default() -> Self { Self::new() }
}

#[cfg(deleteme)]
impl SpacePack {
    fn prepare_new_map<P, T>(
        &mut self,
        pack_idx: PackIndex,
        pois: P,
        trails: T,
        render_entities: &mut Vec<RenderEntity>,
    ) where
        P: IntoIterator<Item = SpacePoi>,
        T: IntoIterator<Item = SpaceTrail>,
    {
        self.clear();
        self.render_list_bookmark = Some(render_entities.len());

        for mut trail in trails {
            let trail_idx = self.active_trails.len() as TrailIndex;
            trail.render_bookmark = render_entities.len() as _;
            for i_section in 0..trail.section_bounds.len() {
                let render_id = RenderId::TrailSection {
                    pack_idx,
                    trail_idx,
                    section: i_section as TrailSectionIndex,
                };
                let entity = RenderEntity {
                    bounds: trail.section_bounds[i_section],
                    position: trail.section_bounds[i_section].center(),
                    // TODO: just sort by y and reverse draw order if camera dir.y is negative? :p
                    // then only intersecting paths are an issue...
                    //draw_ordered: true,
                    draw_ordered: false,
                    render_id: match trail.is_empty() {
                        false => Some(render_id),
                        true => None,
                    },
                };
                render_entities.push(entity);
            }

            self.active_trails.push(trail);
        }

        self.poi_bookmark = render_entities.len();

        for poi in pois {
            let poi_idx = self.active_pois.len() as PoiIndex;
            let entity = RenderEntity {
                bounds: poi.bounds,
                position: poi.position,
                draw_ordered: true,
                render_id: match poi.is_empty() {
                    false => Some(RenderId::Poi { pack_idx, poi_idx }),
                    true => None,
                },
            };
            render_entities.push(entity);
            self.active_pois.push(poi);
        }
    }
}

#[derive(Clone)]
pub struct SpaceEntity {
    pub id: MarkerId,
    pub bounds: aabb::Aabb<f32, 3>,
}
impl SpaceEntity {
    pub fn invalid() -> Self {
        Self {
            id: MarkerId::EMPTY,
            bounds: box3aabb(irrelevant_box3::<DrawSpace>()),
        }
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
        Self {
            position: Point3::INFINITY,
        }
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

    #[cfg(todo)]
    pub fn rebuild_extra(&mut self) {
        log::info!("TODO: rebuild_extra");
    }

    pub fn retain<F: FnMut(&mut SpaceEntity) -> bool>(&mut self, mut cond: F) -> BitVec {
        let mut removed: BitVec = Default::default();
        removed.resize(self.entities.len(), false);
        for (i, e) in self.entities.iter_mut().enumerate() {
            if !cond(e) {
                *e = BvhShape::new(SpaceEntity::invalid());
                if let Some(mut b) = removed.get_mut(i) {
                    *b = true;
                }
            }
        }
        #[cfg(todo = "unnecessary")]
        for i in removed.iter_ones() {
            let Some(extra) = self.extra.get_mut(i) else { continue };
            *extra = SpaceEntityExtra::invalid();
        }
        removed
    }
    pub fn remove_pack(&mut self, pack: PackPath) {
        self.retain(|e| match e.id.variant() {
            IdVariant::MarkerRegistered(p) => p.root != pack,
            IdVariant::MarkerLoaded(p) => p.root.root != pack,
            _ => true,
        });
        self.extra = Vec::new();
    }
}
impl Extend<(MarkerId, Box3<DrawSpace>, Point3<DrawSpace>)> for SpaceEntities {
    fn extend<T: IntoIterator<Item = (MarkerId, Box3<DrawSpace>, Point3<DrawSpace>)>>(&mut self, iter: T) {
        if self.entities.len() != self.extra.len() {
            log::error!("SpaceEntities len({}) mismatches extra({})", self.entities.len(), self.extra.len());
            return
        }
        let iter = iter.into_iter();
        let (min, max) = iter.size_hint();
        let cap = max.unwrap_or(min);
        self.entities.reserve(cap);
        self.extra.reserve(cap);
        for (id, bounds, position) in iter {
            let entity = SpaceEntity {
                id,
                bounds: box3aabb(bounds),
            };
            self.entities.push(BvhShape::new(entity));
            self.extra.push(SpaceEntityExtra {
                position,
            });
        }
    }
}

#[derive(Clone)]
pub struct SpacePackCollection {
    pub map_id: Option<MapIndex>,
    pub loaded_packs: PackVecOf<SpacePack>,
    pub render_entities: SpaceEntities,
    pub bvh: Bvh<f32, 3>,

    #[cfg(todo)]
    pub render_list: RenderList,
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

    pub fn needs_rebuild(&self, map_id: MapIndex, packs: &LoadedPacks) -> bool {
        self.map_id != Some(map_id) ||
            packs.sigs_match(self.loaded_packs.values().map(|p| p.info_sig))
    }

    pub fn rebuild_entities_if_dirty(&mut self, map_id: MapIndex, packs: &LoadedPacks, map_info: &LoadedMapInfo, maps: &LoadedMaps) {
        if self.needs_rebuild(map_id, packs) {
            self.rebuild_entities(map_id, packs, map_info, maps);
        }
    }
    pub fn rebuild_entities(&mut self, map_id: MapIndex, packs: &LoadedPacks, map_info: &LoadedMapInfo, maps: &LoadedMaps) {
        #[cfg(todo)]
        if self.map_id != Some(map_id) {
            self.clear();
        } else {
            let entities_len = self.render_entities.entities.len();
            if self.render_entities.extra.is_empty() && entities_len > 0 {
                let trailing_entities = self.render_entities.entities.iter()
                    .enumerate()
                    .skip(self.render_entities.extra.len());
                for (_i, entity) in trailing_entities {
                    let mid = &entity.value.id;
                    let lidx = mid.get_marker_index();
                    let map_path = mid.get_marker_pack_map_path();
                    let map = if map_path.path == map_id {
                        maps.lookup_ref(&map_path)
                    } else { None };
                    let position = map.and_then(|map| {
                        match lidx.namespace() {
                            MarkerIndex::NS_TRAIL => {
                                let (idx, s) = lidx.index_trail_section_unchecked();
                                map.trails.get(idx as usize)
                                    .and_then(|trail| trail.section_info.sections.as_ref())
                                    .and_then(|sections| sections.get(s as usize))
                                    .map(|s| s.bounds.center())
                            },
                            MarkerIndex::NS_POI => {
                                let idx = lidx.index_poi_unchecked();
                                map.pois.get(idx as usize).map(|p| p.position())
                            },
                            _ => None,
                        }
                    });
                    let extra = position.map(|position| SpaceEntityExtra {
                        position,
                    }).unwrap_or_else(|| {
                        log::error!("PATHY: lost marker {mid} @ {lidx}?");
                        SpaceEntityExtra::invalid()
                    });
                    self.render_entities.extra.push(extra);
                }
            }
        }
        self.clear();
        self.map_id = Some(map_id);
        let mut ents = Vec::new();
        for (path, pack) in packs.packs.iter() {
            let spacepack = self.loaded_packs.lookup_extend_with(path.path, SpacePack::default);
            if !pack.is_loaded() {
                spacepack.clear();
                continue
            }
            spacepack.info_sig = pack.info.sig;
            if !pack.info.has_map(map_id) {
                self.render_entities.remove_pack(path);
                continue
            }
            let map_path = path.rel(map_id);
            let Some(map_info) = map_info.lookup_ref(&map_path) else { continue };
            let Some(map) = maps.lookup_ref(&map_path) else { continue };
            // TODO: support searching through existing entities for partial rebuild support
            log::debug!("TODO: partial space rebuild");
            self.render_entities.remove_pack(path);
            // to iter of (marker_id, bounds, position)
            let pois = map.lpois().into_iter().map(|(lpoi_path, lpoi)| {
                let marker_path: MarkerPath = lpoi_path.pivot_to();
                let path = map_path.rel(marker_path.path);
                (MarkerId::for_marker(path), lpoi.bounds(), lpoi.position())
            });
            let trails = map_info.trail_info.iter().flat_map(move |(ltrail_path, ltrail)| ltrail.section_bounds().map(move |(section_path, bounds)| {
                let ts_path: LoadedTrailSectionPath = LoadedTrailSectionPath::with_path(ltrail_path.rel(section_path));
                let marker_path: MarkerPath = ts_path.pivot_to();
                let path = map_path.rel(marker_path.path);
                let pos = bounds.center();
                (MarkerId::for_marker(path), bounds, pos)
            }));
            ents.extend(pois.chain(trails));
        }
        self.render_entities.extend(ents);
        //self.render_entities.rebuild_extra();
    }
    /// TODO: check map state sigs or something idk what needs to change
    pub fn entities_dirty(&self, map_id: MapIndex, packs: &LoadedPacks) -> bool {
        self.render_entities.needs_rebuild()
    }

    pub fn needs_bvh_rebuild(&self) -> bool {
        let entity_count = self.render_entities.entities.len();
        if entity_count > 0 && self.bvh.nodes.is_empty() {
            return true
        }

        let bvh_leaf_count = self.bvh.nodes.iter()
            .filter(|node| matches!(node, bvh::bvh::BvhNode::Leaf { .. }))
            .count();

        entity_count != bvh_leaf_count
    }

    pub fn rebuild_bvh(&mut self) {
        log::debug!("TODO: rebuild bvh");
        self.bvh = Bvh::build(&mut self.render_entities.entities);
    }

    pub fn clear(&mut self) {
        for pack in self.loaded_packs.values_mut() {
            pack.clear();
        }
        self.render_entities.clear();
        self.bvh = Bvh { nodes: Vec::new() };
        self.map_id = None;
    }

    #[inline]
    pub fn bvh_traverse_shapes<'a, Q: aabb::IntersectsAabb<f32, 3>>(&'a self, query: &'a Q) -> bvh::bvh::BvhTraverseIterator<'a, 'a, f32, 3, Q, BvhShape<SpaceEntity>> {
        self.bvh.traverse_iterator(query, &self.render_entities.entities)
    }
    #[inline]
    pub fn bvh_traverse<'a, Q: aabb::IntersectsAabb<f32, 3>>(&'a self, query: &'a Q) -> impl Iterator<Item = (usize, &'a MarkerId)> + 'a {
        let shapes = &self.render_entities.entities[..];
        self.bvh.traverse_iterator(query, shapes)
            .map(move |shape| {
                let idx = slice_offset_from(shapes, shape);
                (idx, &shape.value.id)
            })
    }

    #[cfg(todo)]
    pub fn pack_mut<'a>(&'a mut self, path: &PackPath) -> &'a mut SpacePack {
        let index = path.path as usize;
        if self.loaded_packs.len() <= index {
            self.loaded_packs.resize_with(index + 1, || SpacePack::new());
        }
        &mut self.loaded_packs[index]
    }

    #[cfg(todo = "deleteme?")]
    pub fn load_pack<P, T>(&mut self, pack_idx: PackIndex, pois: P, trails: T) -> anyhow::Result<()> where
        P: IntoIterator<Item = SpacePoi>,
        T: IntoIterator<Item = SpaceTrail>,
    {
        let pack = self
            .loaded_packs
            .get_mut(pack_idx as usize)
            .with_context(|| format!("unrecognized pack index {pack_idx}"))?;
        if pack.render_list_bookmark.is_some() {
            log::info!("skipping pack#{pack_idx}, already loaded?");
            return Ok(())
        }

        log::debug!("Preparing pack#{pack_idx} for rendering...");
        self.build_active_pack(pack_idx, pois, trails, None)?;

        if log::log_enabled!(log::Level::Info) {
            let pack = &self.loaded_packs[pack_idx as usize];
            if !pack.active_trails.is_empty() || !pack.active_pois.is_empty() {
                log::info!(
                    "Loaded {} trails and {} POIs from pack #{pack_idx}",
                    pack.active_trails.len(),
                    pack.active_pois.len(),
                );
            }
        }

        //self.recreate_buffers(device)?;
        self.mark_buffers_dirty();

        Ok(())
    }

    #[cfg(todo = "deleteme?")]
    fn build_active_pack<P, T>(
        &mut self,
        pack_idx: PackIndex,
        pois: P, trails: T,
        render_entities: Option<&mut Vec<RenderEntity>>,
    ) -> anyhow::Result<()> where
        P: IntoIterator<Item = SpacePoi>,
        T: IntoIterator<Item = SpaceTrail>,
    {
        let pack = self
            .loaded_packs
            .get_mut(pack_idx as usize)
            .with_context(|| format!("unrecognized pack index {pack_idx}"))?;

        let (entities, inplace) = match render_entities {
            Some(e) => (e, false),
            None => (self.render_list.entities_mut(), true),
        };
        let res = Ok(pack.prepare_new_map(pack_idx, pois, trails, entities));
        #[cfg(todo = "unnecessary")]
        if res.is_err() {
            //.with_context(|| format!("loading pack#{pack_idx}"));
            log::info!("pack#{pack_idx} failed to load, disabling...");
            if let Some(bookmark) = pack.render_list_bookmark {
                let _ = entities.drain(bookmark..);
                /*for entity in &mut self.render_list.entities_mut()[bookmark..] {
                    entity.disable();
                }*/
            }
            pack.clear();
            pack.cleanup_textures();
        }
        if inplace {
            self.render_list.entities_mut_end();
        }
        res
    }

    #[cfg(todo)]
    pub fn rebuild_active(&mut self) -> anyhow::Result<()> {
        let mut render_builder = self.render_list.rebuild();

        for (pack_idx, pack) in self.loaded_packs.iter_mut().enumerate() {
            let pois = mem::take(&mut pack.active_pois);
            let trails = mem::take(&mut pack.active_trails);
            pack.clear();
            pack.active_pois.reserve_exact(pois.len());
            pack.active_trails.reserve_exact(trails.len());
            pack.prepare_new_map(pack_idx as PackIndex, pois, trails, &mut render_builder.entities);
        }

        log::info!(
            "Loaded {} trails and {} POIs",
            self.loaded_packs
                .iter()
                .map(|p| p.active_trails.len())
                .sum::<usize>(),
            self.loaded_packs
                .iter()
                .map(|p| p.active_pois.len())
                .sum::<usize>(),
        );

        //let res = self.recreate_buffers(device)?;
        self.mark_buffers_dirty();
        let res = Ok(());

        self.render_list = render_builder.build();

        res
    }

    #[cfg(todo)]
    #[cfg(feature = "goggles")]
    pub fn entities_obscured<'a>(
        &'a self,
        frustum: &'a MapFrustum,
    ) -> impl Iterator<Item = &'a RenderEntity> + 'a {
        self.render_list.visible_entities(frustum)
    }

    #[cfg(todo)]
    pub fn entities_map<'a>(
        &'a self,
        mut bounds: Box3<DrawSpace>,
    ) -> impl Iterator<Item = &'a RenderEntity> + 'a {
        // adding some wiggle room around the map edges...
        let buffer = bounds.size() * 0.15;
        bounds.min.x -= buffer.width;
        bounds.min.z -= buffer.depth;
        bounds.max.x += buffer.width;
        bounds.max.z += buffer.depth;

        self.render_list.map_entities(bounds)
    }

    #[cfg(todo)]
    pub fn deactivate(&mut self, pack_idx: PackIndex, cleanup: bool) {
        let Some(pack) = self.loaded_packs.get_mut(pack_idx as usize) else { return };
        if let Some(bookmark) = pack.render_list_bookmark {
            let bookmark_end = pack.poi_bookmark + pack.active_pois.len();
            let render_list = self.render_list.entities_mut();
            if bookmark_end >= render_list.len() {
                let _ = render_list.drain(bookmark..);
            } else {
                for entity in &mut render_list[bookmark..bookmark_end] {
                    entity.disable();
                }
            }
            self.render_list.entities_mut_end();
        }
        pack.clear();
        if cleanup {
            pack.cleanup_textures();
        }
    }

    #[cfg(todo)]
    pub fn clear_active(&mut self) {
        self.render_list.clear();
        for pack in &mut self.loaded_packs {
            pack.clear();
        }

        #[cfg(deleteme)] {
            self.reset_poi_buffers();
        }
    }

    #[cfg(todo)]
    pub fn all_entities(&self, map: &SharedGameplayMap) -> impl Iterator<Item = MarkerId> + '_ {
    }
}
impl Default for SpacePackCollection {
    fn default() -> Self {
        Self::new()
    }
}
