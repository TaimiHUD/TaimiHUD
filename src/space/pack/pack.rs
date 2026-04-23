use {
    super::{render, PoiCommonRenderData},
    crate::{
        controller::pathing::{
            registry::{
                LoadedPoiIndex,
                LoadedPoiNs,
                LoadedPoiPath,
                LoadedTrailIndex,
                LoadedTrailNs,
                LoadedTrailPath,
                PoiMapPath,
                PackIndex,
                PackRegistryNs,
                PackVecOf,
            },
            shared::{
                SharedGameplayMap,
                SharedLoaderPacksInfo,
                SharedMapPackLoaded,
                SharedMapPackState,
                SharedPackInfo,
                SharedMarkerRef,
                LoadedMarkerRef,
            },
            space::{
                SpaceEntities, SpacePackCollection, SpacePackShared, TextureLoadRequests, TrailGeometryRequests,
                PoiScale, TrailScale, TrailTextureMap,
            },
            PathingController, PathingEvent,
        },
        exports::runtime::{self as rt,
            textures::TextureSlot,
        },
        render::machine::{frame_log, RenderMachine, RenderPosition},
        space::{
            dx11::RenderBackend,
            pack::{instance::{self, EntityInstanceBuffer, EntityInstanceData, PoiVertexBuffer}, PoiRender, TrailRender},
            DrawSpace, ScreenSpace,
        },
        resources::shader::ShaderLoader,
        settings::pathing::{PathingSettings, SpaceSettings},
    },
    anyhow::Context,
    bvh::aabb,
    futures::future::Either,
    glamour::{Size2, Vector2, Box3, Point3, Matrix4},
    rustc_hash::FxHashSet,
    std::{collections::{BTreeSet, BTreeMap}, iter, mem, num::NonZero, ops, sync::Arc, time::Instant},
    taimi_d3d::{
        dx11::{
            self,
            prelude::*,
            buffer::{ConstantBufferP, ConstantBufferV},
        },
        shader::ShaderKind,
    },
    taimi_hoard::{
        loc::{indexed::IndexedList, LocationMut, LocationRef},
        statistics::Counter,
    },
    taimi_meta::{
        packs::{
            id::{MarkerId, MarkerIndex, MarkerIndexVariant},
            MapIndex,
            PackMapPath,
            PoiIndex,
            TrailSectionPath,
        },
        spatial::{box3aabb, cull::BvhQuery},
        ui::{LocalContext, MapContext, MapCalibration},
    },
    taimi_sync::{
        arcs::ArcPtrCmp,
        watched::{watch, Watched},
    },
    taimi_pack::attributes::{keys, BounceBehavior},
};

/// Internal rendering data.
pub struct PackRenderData {
    pub info: Arc<SharedPackInfo>,
    pub map_info: Option<SharedMapPackLoaded>,
    pub map_state: SharedMapPackState,
    pub pois: IndexedList<LoadedPoiNs, LoadedPoiIndex, Vec<PoiRender>>,
    pub trails: IndexedList<LoadedTrailNs, LoadedTrailIndex, Vec<TrailRender>>,
    pub render_poi_bookmark: usize,
}

impl PackRenderData {
    pub fn new() -> Self {
        Self {
            info: Default::default(),
            map_info: None,
            map_state: Default::default(),
            pois: Default::default(),
            trails: Default::default(),
            render_poi_bookmark: 0,
        }
    }

    pub fn render_poi_bookmarks(&self) -> ops::Range<PoiIndex> {
        match self.render_poi_bookmark {
            0 => 0..0,
            start => {
                let end = (start + self.pois.len()) as PoiIndex;
                let start = start as PoiIndex;
                start..end
            },
        }
    }

    pub fn is_empty(&self) -> bool {
        let trails = self.trails.is_empty();
        let pois = self.render_poi_bookmark == 0 || self.pois.is_empty();
        trails && pois
    }

    pub fn clear(&mut self) {
        self.pois.clear();
        self.trails.clear();
        self.map_info = None;
        self.render_poi_bookmark = 0;
    }

    pub fn cleanup_background(mut self) {
        // mostly just make a point of not cleaning up render resources...
        for poi in self.pois.drain(..) {
            poi.cleanup_background();
        }
        for trail in self.trails.drain(..) {
            trail.cleanup_background();
        }
    }

    pub fn map_path(&self) -> Option<PackMapPath> {
        self.map_info.as_ref().map(|i| i.path)
    }
}

pub struct PackRender {
    pub pack_data: PackVecOf<PackRenderData>,

    pub spacepacks: Watched<Arc<SpacePackCollection>>,
    pub trail_rx: TrailGeometryRequests,
    pub texture_rx: TextureLoadRequests,
    packs_rx: Option<watch::Receiver<SharedLoaderPacksInfo>>,
    packs_map: Option<watch::Receiver<SharedGameplayMap>>,

    pub render_list: PackRenderList,
    pub draw_state: PackRenderState,
    pub resources: PackRenderResources,
    pub shared_v: instance::ConstantDataV,
    pub shared_p: instance::ConstantDataP,
}

impl PackRender {
    pub fn new(backend: &RenderBackend) -> anyhow::Result<Self> {
        let poi_common = PoiCommonRenderData::new(backend)?;
        Ok(Self {
            spacepacks: Default::default(),
            trail_rx: TrailGeometryRequests::empty(),
            texture_rx: TextureLoadRequests::empty(),
            packs_rx: None,
            packs_map: None,
            pack_data: Default::default(),
            render_list: Default::default(),
            draw_state: Default::default(),
            shared_v: Default::default(),
            shared_p: Default::default(),
            resources: PackRenderResources {
                poi_common: Some(poi_common),
                .. PackRenderResources::default()
            },
        })
    }

    fn mark_buffers_dirty(&mut self) {
        if let Some(poi_common) = &mut self.resources.poi_common {
            poi_common.clear();
        }
        for pack in self.pack_data.values_mut() {
            pack.render_poi_bookmark = 0;
        }
    }

    pub fn destroy_buffers(&mut self) {
        self.mark_buffers_dirty();
    }

    /// `Ok(false)` if not ready to render
    ///
    /// won't render if not in a map, or if too early in load and
    /// more setup may be pending
    pub fn prepare(&mut self, device: &Dx11Device, machine: &RenderMachine, settings: Option<&SpaceSettings>) -> anyhow::Result<bool> {
        let Some(pathing) = &machine.pathing else { anyhow::bail!("no shared data") };
        let packs_rx = self.packs_rx.get_or_insert_with(|| {
            let mut rx = pathing.packs.packs.subscribe();
            rx.mark_changed();
            rx
        });
        let _ = self.packs_map.get_or_insert_with(|| {
            let mut rx = pathing.gameplay.subscribe();
            rx.mark_changed();
            rx
        });
        if !self.spacepacks.is_watching() {
            self.spacepacks.restart_watching(&pathing.space.collection);
        }
        if !self.trail_rx.is_watching() {
            self.trail_rx.subscribe_to(&pathing.space.trail_geometry);
        }
        if !self.texture_rx.is_watching() {
            self.texture_rx.subscribe_to(&pathing.space.texture_loads);
        }
        if packs_rx.has_changed().unwrap_or(false) {
            self.render_list.mark_dirty();
            let packs = packs_rx.borrow_and_update();
            if self.pack_data.len() < packs.len() {
                self.pack_data.data.resize_with(packs.len(), PackRenderData::new);
            }
            for (pack, dest) in packs.values().zip(self.pack_data.values_mut()) {
                #[cfg(todo)]
                let prev_sig = dest.info.sig;
                dest.info = pack.info.clone();
            }
        }
        let mut space_dirty = false;
        if let Some(spacepacks) = self.spacepacks.try_read_if_changed() {
            space_dirty = self.render_list.update_space(&*spacepacks);
        }
        if space_dirty {
            self.resources.clear_buffers();
            self.mark_buffers_dirty();
            if self.render_list.spacepacks.map_id.is_some() {
                self.resources.dirty = true;
            }
        }
        let arcrender = || settings.map(|s| s.goggles.arcrender_enabled()).unwrap_or(false);
        let mut ibs_dirty = self.resources.poi_common.as_ref().map(|c| c.is_empty()).unwrap_or(false);
        let map_id = match self.render_list.spacepacks.map_id {
            map_id if map_id != machine.is_ingame() => None,
            map_id => map_id,
        };
        let prev_map_id = self.draw_state.prev_map_id;
        if prev_map_id != map_id {
            self.resources.clear_buffers();
            if map_id.is_some() {
                self.clear_packs();
                self.draw_state.clear();
                self.draw_state.prev_map_id = map_id;
                self.resources.dirty = true;
                self.render_list.mark_dirty();
                space_dirty = true;
            } else {
                STATS_ENTITY_COUNT.reset(0);
                self.render_list.cleanup();
            }
        }
        if space_dirty {
            STATS_ENTITY_COUNT.reset_with(|| if map_id.is_some() {
                match &self.render_list.spacepacks {
                    #[cfg(todo)]
                    space => space.render_entities.entities.iter().filter(|e| !e.is_invalid()).count(),
                    space => space.loaded_packs.values().flat_map(|p| [
                        p.populated_pois.count_ones() as u32,
                        p.populated_trails.count_ones() as u32,
                    ]).sum::<u32>(),
                }
            } else { 0 });
        }
        let packs_map_changed = {
            let packs_changed = match self.packs_map.as_mut() {
                Some(packs_map) if space_dirty || packs_map.has_changed().unwrap_or(false) =>
                    Some(packs_map.borrow_and_update()),
                _ => None,
            };
            match packs_changed {
                Some(packs_map) if packs_map.map_id != map_id => None,
                #[cfg(todo)]
                Some(..) if machine.is_ingame_paused() => None,
                packs_map => packs_map,
            }
        };
        let prev_waiting = mem::replace(&mut self.draw_state.prev_waiting, pathing.packs.read_still_waiting().0);
        let summarize = !self.spacepacks.render_entities.entities.is_empty() && map_id.is_some() && (
            prev_map_id.is_none()
            || prev_waiting
        ) && !self.draw_state.prev_waiting;
        if summarize {
            let (pois, trails) = self.spacepacks.loaded_packs.values().fold(
                (0usize, 0usize),
                |(mut pois, mut trails), p| {
                    pois += p.populated_pois.count_ones();
                    trails += p.populated_trails.count_ones();
                    (pois, trails)
                },
            );
            log::info!("Loaded {trails} trails and {pois} POIs");
        }
        if prev_waiting && !self.draw_state.prev_waiting {
            self.render_list.mark_dirty();
        }
        if let Some(map_id) = map_id {
            if let Some(packs_map) = &packs_map_changed {
                log::trace!("gameplay maps rx @ {map_id}");
                self.render_list.mark_dirty();
                if let Some(maps) = packs_map.get_ref(map_id) {
                    for (pack_path, pack) in self.pack_data.iter_mut() {
                        let Some((packmap_path, map_info)) = maps.get_info_for(pack_path) else {
                            pack.clear();
                            continue
                        };
                        let map_info = pack.map_info.insert(map_info.clone());

                        let poi_len = map_info.info.poi_count();
                        if pack.pois.len() != poi_len {
                            let dirty_start = if poi_len < pack.pois.len() {
                                0
                            } else {
                                pack.pois.len()
                            };
                            pack.pois.resize_with(poi_len, PoiRender::empty);
                            let dirty_pois = pack.pois.values_mut()
                                .zip(map_info.pois_iter())
                                .skip(dirty_start);
                            for (poi, info) in dirty_pois {
                                let attrs = info.poi_attrs();
                                poi.occlude = attrs.occlude();
                                if poi.occlude && attrs.icon_file.is_none() {
                                    poi.icon = Some(TextureSlot::Unavailable);
                                }
                            }
                        }

                        let trail_len = map_info.trail_count();
                        if pack.trails.len() != trail_len {
                            pack.trails.resize_with(trail_len, TrailRender::empty);
                        }

                        let map = maps.get_state(packmap_path);
                        if let Some(map) = map {
                            pack.map_state.clone_from(map);

                            let empty_trails = pack.trails.iter().filter(|(path, trail)| {
                                if !trail.is_empty() {
                                    return false
                                }
                                if map_info.info.is_trail_info_loaded(*path) {
                                    return false
                                }
                                if let Some(ltrail) = map.trails().lookup_ref(path) {
                                    if !ltrail.visibility.is_visible() {
                                        return false
                                    }
                                }
                                true
                            });
                            for (ltrail_path, _trail) in empty_trails {
                                // schedule geometry load imminently
                                let ltrail_path = packmap_path.rel(ltrail_path.path);
                                self.draw_state
                                    .drawn_incomplete
                                    .insert(SpacePackShared::trail_geometry_id(&ltrail_path));
                            }
                        } else {
                            pack.map_state.clear();
                        }

                        #[cfg(todo)]
                        for (poi, lpoi) in pack.pois.values_mut().zip(pack.map_state.loaded_pois(map_info))
                        {
                            poi.update(device, &pack.info, Some(lpoi))
                        }
                        #[cfg(todo)]
                        for (trail, ltrail) in pack
                            .trails
                            .values_mut()
                            .zip(pack.map_state.loaded_trails(map_info))
                        {
                            trail.update(device, &pack.info, Some(ltrail))
                        }
                    }
                }
                self.resources.dirty = true;
                if !ibs_dirty {
                    if self
                        .pack_data
                        .values()
                        .any(|p| p.render_poi_bookmarks().len() != p.pois.len())
                    {
                        ibs_dirty = true;
                    }
                }
                if !ibs_dirty {
                    let (ib_pack_len, ib_len) = self.resources.poi_common.as_ref().map(|c| (
                        c.ib_len_for_packs(&self.pack_data),
                        c.ib_len(),
                    )).unwrap_or((0, 0));
                    if !ibs_dirty {
                        ibs_dirty |= ib_pack_len != ib_len;
                    }
                }
            }
            drop(packs_map_changed);
            for (ltrail_path, trail_incoming) in self.trail_rx.try_recv_fulfilled() {
                self.draw_state
                    .drawn_incomplete
                    .remove(&SpacePackShared::trail_geometry_id(&ltrail_path));
                match self.render_list.spacepacks.map_id {
                    Some(mid) if mid != ltrail_path.root.path => {
                        log::info!("received outdated geometry for {ltrail_path}?");
                        continue
                    },
                    _ => (),
                }
                let Some(pack_data) = self.pack_data.lookup_mut(&ltrail_path.root.root) else {
                    log::error!("received geometry for {ltrail_path} - unrecognized pack?");
                    continue
                };
                let path: LoadedTrailPath = ltrail_path.unscope();
                let Some(trail) = pack_data.trails.lookup_mut(&path) else {
                    log::error!("received geometry for {ltrail_path} - unrecognized trail?");
                    continue
                };
                let res = match trail_incoming {
                    geometry if geometry.is_empty() => None,
                    geometry => rt::log::error_ok(
                        trail
                            .setup_geometry(device, geometry, arcrender())
                            .context("loading trail geometry"),
                    ),
                };
                if res.is_none() {
                    trail.disable();
                } else {
                    self.render_list.mark_dirty();
                }
            }
            for (marker_path, texture) in self.texture_rx.try_recv_fulfilled() {
                // texture loader should be notified, so no need to do anything really?
                let id = MarkerId::for_marker(marker_path);
                if texture.is_none() {
                    log::error!("request for tex {marker_path} failed");
                }
                if marker_path.root.path != map_id {
                    log::info!("received outdated tex for {marker_path}");
                    continue
                }
                self.draw_state.drawn_incomplete.remove(&id);
                let Some(pack_data) = self.pack_data.lookup_mut(&marker_path.root.root) else {
                    log::error!("received tex for {marker_path} - unrecognized pack?");
                    continue
                };
                match marker_path.path.variant() {
                    MarkerIndexVariant::Poi(poii) => {
                        let path: LoadedPoiPath = LoadedPoiPath::with_path(poii);
                        let Some(poi) = pack_data.pois.lookup_mut(&path) else {
                            log::error!("received tex for {marker_path} - unrecognized poi?");
                            continue
                        };
                        match texture {
                            Some(key) => {
                                poi.icon_handle = Some(key);
                                poi.icon = None;
                            },
                            None => {
                                poi.icon = Some(TextureSlot::Unavailable);
                            },
                        }
                    },
                    MarkerIndexVariant::Trail(traili) | MarkerIndexVariant::TrailSection(traili, _) => {
                        let path: LoadedTrailPath = LoadedTrailPath::with_path(traili);
                        let Some(trail) = pack_data.trails.lookup_mut(&path) else {
                            log::error!("received tex for {marker_path} - unrecognized trail?");
                            continue
                        };
                        match texture {
                            Some(key) => {
                                trail.texture_handle = Some(key);
                                trail.texture = None;
                            },
                            None => {
                                trail.texture = Some(TextureSlot::Unavailable);
                            },
                        }
                    },
                    _ => {
                        log::error!("received tex for {marker_path} - invalid path?");
                    },
                }
            }
            let mut incomplete_trail_geometry = BTreeSet::new();
            let mut incomplete_textures = BTreeSet::new();
            for id in self.draw_state.drawn_incomplete.iter() {
                let Some(path) = id.marker_path::<PackMapPath>() else {
                    log::error!("invalid incomplete marker {id}??");
                    continue
                };
                let _pack_path = match path.root {
                    path if path.path != map_id => continue,
                    path => path.root,
                };
                let pack_data = self.pack_data.lookup_mut(&path.root.root);
                match path.path.variant() {
                    MarkerIndexVariant::Poi(poii) => {
                        let has_texture = pack_data.and_then(|pack_data| {
                            let lpath: LoadedPoiPath = LoadedPoiPath::with_path(poii);
                            let info = &pack_data.info;
                            match pack_data.pois.lookup_mut(&lpath) {
                                Some(poi) if poi.needs_texture_info() => None,
                                poi => poi.map(move |r| (info, r)),
                            }
                        });
                        match has_texture {
                            Some((pack_info, poi)) => {
                                poi.update(device, pack_info, None);
                            },
                            None => {
                                incomplete_textures.insert(path);
                            },
                        }
                    },
                    MarkerIndexVariant::Trail(traili) => {
                        let has_texture = pack_data.and_then(|pack_data| {
                            let lpath: LoadedTrailPath = LoadedTrailPath::with_path(traili);
                            let info = &pack_data.info;
                            match pack_data.trails.lookup_mut(&lpath) {
                                Some(trail) if trail.needs_texture_info() => None,
                                trail => trail.map(move |r| (info, r)),
                            }
                        });
                        match has_texture {
                            Some((pack_info, trail)) => {
                                trail.update(device, pack_info, None);
                            },
                            None => {
                                incomplete_textures.insert(path);
                            },
                        }
                    },
                    MarkerIndexVariant::TrailSection(traili, _sectioni) => {
                        // we load the whole thing at once so ignore sections...
                        let path = path.root.rel(traili);
                        incomplete_trail_geometry.insert(path);
                    },
                    _ => (),
                }
            }
            self.trail_rx.request_many(incomplete_trail_geometry);
            self.texture_rx.request_many(incomplete_textures);
            if self.resources.dirty {
                if arcrender() {
                    let res = self.resources.prepare(device, self.pack_data.map_ref_as_slice(), &mut self.draw_state, self.spacepacks.render_entities.entities.iter().map(|e| e.id)).context("RenderResources::prepare");
                    rt::log::error_ok(res);
                }
            }
        } else {
            drop(packs_map_changed);
        }
        self.draw_state.clear_active();
        if map_id.is_some() {
            if let Some(poi_common) = &mut self.resources.poi_common {
                poi_common.update_fallback(device, machine);
            }
            if ibs_dirty {
                self.recreate_buffers(device, machine)?;
            }
        }

        Ok(map_id.is_some())
    }
    pub fn prepare_frame(
        &mut self,
        anim_timestamp: Option<f32>,
    ) {
        self.resources.anim_timestamp = anim_timestamp;
        if let Some(anim_timestamp) = self.resources.anim_timestamp {
            self.draw_state.prune_anims(anim_timestamp);
            if self.draw_state.end_anims(self.pack_data.map_mut_as_slice()) {
                self.resources.dirty = true;
            }
        } else {
            self.draw_state.clear_anims();
        }
        self.render_list.prepare_frame();
        STATS_ENTITY_DRAW.reset(0);
        STATS_ENTITY_DRAW_PASS.reset(0);
        STATS_ENTITY_DRAW_ALL.reset(0);
        STATS_ENTITY_DRAW_MAP.reset(0);
    }
    #[inline]
    pub fn setup_frame(
        &mut self,
        _device_context: &Dx11Context,
    ) {
        self.render_list.setup_frame();
    }

    pub fn gameplay_map_enter(&mut self, _prev_anchor: Option<Instant>) {
        self.resources.dirty = true;
        // timestamps reset, so relative ones are invalid now...
        for pack in self.pack_data.values_mut() {
            for poi in pack.pois.values_mut() {
                poi.anim = None;
            }
        }
        self.draw_state.clear_anims();
    }
    pub fn poi_anim_start(&mut self, lpath: PoiMapPath, when: f32) {
        let Some(pack) = self.pack_data.lookup_mut(&lpath.root.root) else { return };
        let lpoi_path: LoadedPoiPath = lpath.unscope();
        let Some(poi) = pack.pois.lookup_mut(&lpoi_path) else { return };
        match &mut poi.anim {
            &mut Some(start) if start < when => (),
            anim => {
                *anim = Some(when);
                self.resources.dirty = true;
            },
        }
    }
    /// TODO: ease out or something
    pub fn poi_anim_end(&mut self, lpath: PoiMapPath, when: f32) {
        if self.draw_state.anims.contains_key(&lpath) { return }
        let Some(pack) = self.pack_data.lookup_mut(&lpath.root.root) else { return };
        let lpoi_path: LoadedPoiPath = lpath.unscope();
        let Some(poi) = pack.pois.lookup_mut(&lpoi_path) else { return };
        let Some(prev_anim) = poi.anim else { return };
        let elapsed = when - prev_anim;
        let (bounce_behavour, bounce_duration, bounce_delay) = pack.map_info.as_ref().and_then(|map_info| map_info.pois().lookup_ref(&lpoi_path))
            .map(|info| {
                let i = info.interaction_attrs();
                (i.bounce_behavior, i.bounce_duration(), i.bounce_delay())
            }).unwrap_or((None, keys::BounceDuration::DEFAULT.into(), keys::BounceDelay::DEFAULT.into()));
        let bounce = bounce_behavour.unwrap_or(BounceBehavior::Bounce);
        let elapsed = (elapsed - bounce_delay).max(0.0);
        let rem = match bounce {
            BounceBehavior::Bounce => bounce_duration - elapsed % bounce_duration,
            BounceBehavior::Rise => {
                // reverse anim...
                self.resources.dirty = true;
                let rem = bounce_duration.min(elapsed) / 2.0;
                poi.anim = Some(when + rem);
                rem
            },
        };
        match rem {
            #[cfg(todo)]
            0.0..=0.2 => {
                // close enough, just stop
                poi.anim = None;
                self.resources.dirty = true;
            },
            rem => {
                self.draw_state.anims.insert(lpath, when + rem);
            },
        }
    }

    fn recreate_buffers(&mut self, device: &Dx11Device, machine: &RenderMachine) -> anyhow::Result<()> {
        let res = self
            .recreate_buffers_inner(device, machine)
            .context("preparing POI instance buffers");
        #[cfg(todo)]
        if res.is_err() {
            self.mark_buffers_dirty();
        }
        res
    }
    fn recreate_buffers_inner(
        &mut self,
        device: &Dx11Device,
        machine: &RenderMachine,
    ) -> anyhow::Result<()> {
        self.allocate_poi_buffers(1);
        if let Some(poi_common) = &mut self.resources.poi_common {
            poi_common.rebuild_ib(device, machine, &self.pack_data)?;
        }

        Ok(())
    }

    /// offset (starting len) currently = 1 to leave space for an identity buffer
    /// at index 0 for drawing trails with
    ///
    /// also [PackRenderData::render_poi_bookmark] of 0 is treated as empty so uh don't
    /// use that
    pub fn allocate_poi_buffers(&mut self, mut offset: usize) -> usize {
        for pack in self.pack_data.values_mut() {
            pack.render_poi_bookmark = offset;
            let poi_count = match &pack.map_info {
                Some(map_info) => map_info.poi_count(),
                None => 0,
            };
            offset += poi_count;
        }
        offset
    }
    pub fn reset_poi_buffers(&mut self) {
        for pack in self.pack_data.values_mut() {
            pack.render_poi_bookmark = 0;
        }
    }

    pub fn draw_markers<'e, E>(
        draw_state: &mut PackRenderState,
        resources: &PackRenderResources,
        context: &Dx11Context,
        backend: &RenderBackend,
        entities: E,
        arcrender: bool,
    ) where
        E: IntoIterator<Item = (&'e PackRenderData, usize, &'e MarkerId)>,
    {
        match arcrender {
            true if resources.shader_variant.is_none() => (),
            true => {
                let mut draw = render::DrawSpaceArc {
                    context,
                    resources,
                    state: None,
                    last_quad: None,
                };
                Self::draw_entities(
                    draw_state,
                    &mut draw,
                    entities,
                );
            },
            false => {
                let mut draw = render::DrawSpacePack {
                    context,
                    shaders: &backend.shaders,
                    poi_common: match &resources.poi_common {
                        Some(poi_common) => poi_common,
                        None => return,
                    },
                    state: None,
                    shader_trail: None,
                };
                Self::draw_entities(
                    draw_state,
                    &mut draw,
                    entities,
                );
            },
        }
    }

    pub fn draw_entities<'e, D, E>(
        draw_state: &mut PackRenderState,
        draw: &mut D,
        entities: E,
    ) where
        E: IntoIterator<Item = (&'e PackRenderData, usize, &'e MarkerId)>,
        D: render::DrawSpaceEntity,
    {
        let mut num_drawn = 0usize;
        for (pack_data, space_idx, marker_id) in entities {
            let render_id = marker_id.get_marker_index();
            match render_id.namespace() {
                MarkerIndex::NS_TRAIL => {
                    let path = {
                        let (t, s) = render_id.index_trail_section_unchecked();
                        LoadedTrailPath::with_path(t).rel(TrailSectionPath::with_path(s))
                    };
                    let trail = pack_data.trails.lookup_ref(&path.root).and_then(|trail| {
                        pack_data
                            .map_state
                            .trails()
                            .lookup_ref(&path.root)
                            .map(|ltrail| (trail, ltrail))
                    });
                    let Some((trail, ltrail)) = trail else {
                        log::error!("Render ID refers to missing {path} in {}", pack_data.info);
                        continue
                    };
                    if !ltrail.visibility.is_visible_for_space() {
                        continue
                    }
                    if trail.report_incomplete(&marker_id, draw_state, path, draw.is_arcrender()) {
                        continue
                    }
                    if draw.draw_trail_section(pack_data, space_idx, trail, path.root, path.path) {
                        num_drawn += 1;
                    }
                },
                MarkerIndex::NS_POI => {
                    let path = LoadedPoiPath::with_path(render_id.index_poi_unchecked());
                    let poi = (
                        pack_data.pois.lookup_ref(&path),
                        pack_data.map_state.pois().lookup_ref(&path),
                    );
                    let (Some(poi), Some(lpoi)) = poi else {
                        log::error!("Render ID refers to missing {path} in {}", pack_data.info);
                        continue
                    };
                    let mut visible = lpoi.visibility.is_visible_for_space();
                    if visible && poi.report_incomplete(&marker_id, draw_state) {
                        continue
                    }
                    if !visible && draw.poi_visible_override(draw_state, pack_data, space_idx, poi, path) {
                        visible = true;
                    }
                    if !visible {
                        continue
                    }
                    if draw.draw_poi(pack_data, space_idx, poi, path) {
                        num_drawn += 1;
                    }
                },
                _ => {
                    log::error!("Render ID {render_id} refers to invalid marker {marker_id}");
                },
            }
        }
        draw.finish();
        if draw_state.is_secondary_draw() {
            STATS_ENTITY_DRAW_PASS.increment(1);
        } else {
            STATS_ENTITY_DRAW.reset(num_drawn);
        }
        STATS_ENTITY_DRAW_ALL.increment(num_drawn);
    }
    pub fn draw_map_entities<'e, E>(
        draw_state: &mut PackRenderState,
        resources: &PackRenderResources,
        device_context: &Dx11Context,
        backend: &RenderBackend,
        map: MapContext,
        entities: E,
    ) where
        E: IntoIterator<Item = (&'e PackRenderData, usize, &'e MarkerId)>,
    {
        let Some(poi_common) = &resources.poi_common else { return };
        draw_state.primary_draw_map = true;
        let mut shader_state = ShaderState::None;
        let mut num_drawn = 0usize;
        let ctx = LocalContext::/*Map(map)*/MAP;
        for (pack_data, _space_idx, marker_id) in entities {
            let render_id = marker_id.get_marker_index();
            let ns = render_id.namespace();
            match ns {
                MarkerIndex::NS_TRAIL => {
                    let path = {
                        let (t, s) = render_id.index_trail_section_unchecked();
                        LoadedTrailPath::with_path(t).rel(TrailSectionPath::with_path(s))
                    };
                    let trail = pack_data.trails.lookup_ref(&path.root).and_then(|trail| {
                        pack_data
                            .map_state
                            .trails()
                            .lookup_ref(&path.root)
                            .map(|ltrail| (trail, ltrail))
                    });
                    let Some((trail, ltrail)) = trail else {
                        log::error!("Render ID refers to missing {path} in {}", pack_data.info);
                        continue
                    };
                    if !ltrail.visibility.is_visible_for_map(map) {
                        continue
                    }
                    if trail.report_incomplete(&marker_id, draw_state, path, false) {
                        continue
                    }
                    if shader_state == ShaderState::None {
                        backend.shaders.set_named(device_context, "map");
                        poi_common.set_primitive(device_context);
                        poi_common.set_instance(device_context, ctx);
                    }
                    if shader_state != ShaderState::Trail {
                        shader_state = ShaderState::Trail;
                    }
                    trail.bind_texture(device_context, poi_common, ctx);
                    trail.draw_section(device_context, path.path, ctx);
                },
                MarkerIndex::NS_POI => {
                    let path = LoadedPoiPath::with_path(render_id.index_poi_unchecked());
                    let poi = (
                        pack_data.pois.lookup_ref(&path),
                        pack_data.map_state.pois().lookup_ref(&path),
                    );
                    let (Some(poi), Some(lpoi)) = poi else {
                        log::error!("Render ID refers to missing {path} in {}", pack_data.info);
                        continue
                    };
                    if !lpoi.visibility.is_visible_for_map(map) {
                        continue
                    }
                    if poi.report_incomplete(&marker_id, draw_state) {
                        continue
                    }
                    if shader_state == ShaderState::None {
                        backend.shaders.set_named(device_context, "map");
                        poi_common.set_primitive(device_context);
                        poi_common.set_instance(device_context, ctx);
                    }
                    if shader_state != ShaderState::Poi {
                        shader_state = ShaderState::Poi;
                        poi_common.set_vertex(device_context, ctx);
                    }
                    poi.bind_texture(device_context, poi_common, ctx);
                    poi.draw(
                        device_context,
                        pack_data.render_poi_bookmark + path.path as usize,
                        ctx,
                    );
                },
                _ => {
                    log::error!("Render ID {render_id} refers to invalid marker {marker_id}");
                    continue
                },
            }
            num_drawn += 1;
        }
        STATS_ENTITY_DRAW_MAP.reset(num_drawn);
        draw_state.primary_draw_map = false;
    }

    pub fn clear(&mut self) {
        self.clear_packs();
        self.draw_state.clear();
        self.resources.clear();
    }
    pub fn clear_packs(&mut self) {
        for pack in self.pack_data.values_mut() {
            pack.clear();
        }
    }
    pub fn stop(&mut self) {
        self.clear_packs();
        self.cleanup_textures();
        if let Some(poi_common) = &mut self.resources.poi_common {
            poi_common.clear();
        }
        self.resources.clear();
    }
    /// See [crate::space::engine::Engine::cleanup_background]
    ///
    /// TODO: revisit, avoid, etc
    pub fn cleanup_background(self) {
        let Self { pack_data, resources, .. } = self;
        resources.cleanup_background();
        for pack in pack_data.data.into_iter() {
            pack.cleanup_background();
        }
    }

    pub fn cleanup_textures(&mut self) {
        let todo = ();
    }
}

#[derive(Debug, Default)]
pub struct PackRenderResources {
    pub poi_common: Option<PoiCommonRenderData>,

    pub len: usize,
    pub dirty: bool,
    pub anim_timestamp: Option<f32>,

    pub entities_ib: Option<EntityInstanceBuffer>,
    pub shader_poi: Option<(dx11::ShaderV, dx11::shader::InputLayout)>,
    pub shader_trail: Option<(dx11::ShaderV, dx11::shader::InputLayout)>,
    pub shader_p_trail: Option<dx11::ShaderP>,
    pub shader_p_poi: Option<dx11::ShaderP>,
    pub shader_variant: Option<render::ArcShaderVariant>,
    pub poi_vb: Option<PoiVertexBuffer>,
    pub poi_vb_trans: Option<PoiVertexBuffer>,
    pub shared_cb_v: Option<ConstantBufferV>,
    pub shared_cb_p: Option<ConstantBufferP>,
    #[cfg(todo)]
    pub map_ib: Option<MapEntityInstanceBuffer>,
}
impl PackRenderResources {
    pub fn prepare<I: IntoIterator<Item = MarkerId>>(
        &mut self,
        device: &Dx11Device,
        pack_data: &IndexedList<PackRegistryNs, PackIndex, [PackRenderData]>,
        draw_state: &mut PackRenderState,
        markers: I,
    ) -> anyhow::Result<()> {
        use glam::Quat;

        let markers = markers.into_iter();
        let mut out = Vec::with_capacity(markers.size_hint().1.unwrap_or(0));
        for mid in markers {
            let path = mid.marker_path::<PackMapPath>()
                .and_then(|path|
                    pack_data.lookup_ref(&path.root.root).map(|p| (p, path))
                );
            let mut ib = EntityInstanceData::INVALID;
            let marker = path.and_then(|(pack, path)|
                pack.map_info.as_ref().and_then(|i|
                    SharedMarkerRef::from_loaded_path(i, Some(&pack.map_state), path)
                        .and_then(|m| m.to_loaded())
                        .map(|m| (m, pack))
            ));
            let common = match &marker {
                Some((LoadedMarkerRef::Poi(poi), pack_data)) => {
                    let attrs = poi.poi_attrs();
                    let ib = ib.write_poi(instance::PoiInstanceData {
                        model: {
                            let scale = glamour::Vector3::<f32>::splat(
                                attrs.icon_size()
                            );
                            let pos = poi.lpoi().position.to_vector();
                            let rot = match attrs {
                                #[cfg(deleteme)]
                                _ => {
                                    let rot = attrs.rotate.map(|r| r.map(f32::to_radians));
                                    use glam::EulerRot;
                                    let erot = Self::tmp_rot().get();
                                    let pre = Self::tmp_pre().get() * core::f32::consts::PI;
                                    //let pre = Quat::from_euler(EulerRot::XYZ, pre.x, pre.y, pre.z);
                                    let post = Self::tmp_post().get() * core::f32::consts::PI;
                                    let post = Quat::from_euler(EulerRot::XYZ, post.x, post.y, post.z);
                                    let xyz = (rot * Self::tmp_mul().get()).to_array();
                                    let mut swizz = glam::Vec3::ZERO;
                                    let order = Self::tmp_order().get();
                                    for (i, out) in order.iter().zip([&mut swizz.x, &mut swizz.y, &mut swizz.z]) {
                                        *out = xyz[*i];
                                    }
                                    //let swizz = rot * Self::tmp_mul().get();
                                    let rot = //pre *
                                        Quat::from_euler(erot, swizz.x + pre.x, swizz.y + pre.y, swizz.z + pre.z)
                                        * post
                                        ;
                                    rot
                                },
                                #[cfg(todo)]
                                attrs => attrs.rotation().map(|rot| rot * Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2)),
                                attrs => attrs.rotate().map(|rot|
                                    // can maybe get away with less fancy math idk...
                                    Quat::from_euler(glam::EulerRot::XZY, rot.x - core::f32::consts::FRAC_PI_2, rot.y, -rot.z)
                                ),
                            };
                            glamour::Matrix4::from_scale_rotation_translation(scale,
                                rot.unwrap_or(Quat::IDENTITY),
                                pos.to_untyped(),
                            )
                        },
                        .. instance::PoiInstanceData::INVALID
                    });
                    let anim_start = pack_data.pois.lookup_ref(&poi.loaded_index())
                        .and_then(|rpoi| rpoi.anim);
                    let bounce_args = poi.lpoi_info().get_interaction_attrs()
                        .map(|i| (i.bounce_behavior, i.bounce_height(), i.bounce_duration(), i.bounce_delay()));
                    let mut bounce_delay: f32 = keys::BounceDelay::DEFAULT.into();
                    let bounce = match bounce_args {
                        Some((Some(behaviour), height, duration, delay)) => {
                            bounce_delay = delay;
                            Some((behaviour, height, duration))
                        },
                        bounce if anim_start.is_some() => Some({
                            let (height, duration) = bounce.map(|(_, height, duration, delay)| {
                                bounce_delay = delay;
                                (height, duration)
                            }).unzip();
                            (BounceBehavior::Bounce, height.unwrap_or(keys::BounceHeight::DEFAULT.into()), duration.unwrap_or(keys::BounceDuration::DEFAULT.into()))
                        }),
                        _ => None,
                    };
                    if let Some((behaviour, height, duration)) = bounce {
                        let ending = draw_state.anims.contains_key(&poi.loaded_path());
                        ib.set_bounce(height, duration, behaviour, ending, bounce_delay, anim_start);
                    } else {
                        ib.clear_bounce();
                    }
                    if attrs.rotate.is_none() {
                        ib.marker.flags |= instance::MarkerInstanceData::FLAG_BILLBOARD;
                    }
                    if attrs.occlude() {
                        ib.marker.flags |= instance::MarkerInstanceData::FLAG_OPAQUE;
                    }
                    if !attrs.scale_on_map_with_zoom() {
                        ib.marker.flags |= instance::MarkerInstanceData::FLAG_MAP_STATIC_SCALE;
                    }
                    // pixels at 1.0 map scale, translated to local space, but quad is 2.0x2.0...
                    ib.map_scale = attrs.map_display_size() / 2.0;
                    ib.billboard_scale = attrs.icon_size();
                    ib.set_size_range(attrs.min_size(), attrs.max_size());
                    Some((&mut ib.marker, poi.lpoi_info().marker_info()))
                },
                Some((LoadedMarkerRef::Trail(trail), _pack_data)) => {
                    let attrs = trail.trail_attrs();
                    let ib = ib.write_trail(instance::TrailInstanceData {
                        .. instance::TrailInstanceData::INVALID
                    });
                    ib.marker.set_anim_scale(attrs.anim_speed());
                    if attrs.is_wall() {
                        ib.marker.flags |= instance::MarkerInstanceData::FLAG_WALL;
                    }
                    Some((&mut ib.marker, trail.ltrail_info().marker_info()))
                },
                _ => None,
            };
            if let Some((ib, attrs)) = common {
                use glam::Vec4Swizzles;
                use taimi_pack::attributes::CullDirection;

                let r = attrs.attrs();
                let tint = r.tint();
                ib.colour = tint.xyz().into();
                ib.set_alpha(tint.w);
                let can_fade = match r.can_fade() {
                    // I'd rather not fade all POIs by default...
                    true if r.can_fade.is_none() && matches!(marker, Some((LoadedMarkerRef::Poi(..), ..))) =>
                        false,
                    f => f,
                };
                if !can_fade {
                    ib.flags |= instance::MarkerInstanceData::FLAG_OBSCURE_FADE;
                }
                ib.flags |= match r.cull() {
                    CullDirection::None => 0,
                    dir => {
                        let cull_front = matches!(dir, CullDirection::CounterClockwise)
                            .then_some(instance::MarkerInstanceData::FLAG_FACE_CULL_FRONT);

                        instance::MarkerInstanceData::FLAG_FACE_CULL | cull_front.unwrap_or(0)
                    },
                };
                ib.set_fade_range(r.fade_near(), r.fade_far());
            }

            out.push(ib);
        }
        let mut res = Ok(());
        let len = out.len();
        self.entities_ib = match out {
            out if out.is_empty() => None,
            out => match EntityInstanceData::alloc_populated(device, &out[..]) { Ok(ib) => Some(ib),
                Err(e) => {
                    res = Err(e);
                    None
                },
            },
        };
        if res.is_ok() {
            self.len = len;
        }
        self.dirty = false;
        if self.entities_ib.is_some() {
            if self.poi_vb.is_none() {
                self.poi_vb = Some(instance::PoiVertex::alloc(device, &instance::PoiVertex::POI_QUAD)?);
            }
            if self.poi_vb_trans.is_none() {
                self.poi_vb_trans = Some(instance::PoiVertex::alloc(device, &instance::PoiVertex::POI_QUAD_TRANSPARENT)?);
            }
        }
        STATS_ENTITY_INSTANCE_SIZE.reset(self.entities_ib.is_some()
            .then_some(len * mem::size_of::<EntityInstanceData>())
            .unwrap_or(0)
        );

        res
    }
    #[cfg(deleteme)]
    pub fn prepare_shaders(
        &mut self,
        shaders: &ShaderLoader,
    ) -> anyhow::Result<()> {
        if self.entities_ib.is_none() { return Ok(()) }

        let trail = shaders.pair_named("trail-ng")?;
        self.shader_trail = Some(trail.0);
        self.shader_p = trail.1;
        self.shader_poi = shaders.vertex.get("poi-ng").cloned();
        Ok(())
    }
    pub fn prepare_shaders_arc(
        &mut self,
        shaders: &ShaderLoader,
        draw_state: &mut PackRenderState,
        variant: render::ArcShaderVariant,
    ) -> bool {
        if self.entities_ib.is_none() { return true }
        if self.shader_variant == Some(variant) { return true }
        self.shader_variant = None;
        let trail_v = Self::lookup_shaders_arc(shaders, draw_state, variant, ShaderKind::Vertex, Some(render::ShaderState::Trail));
        let trail_p = Self::lookup_shaders_arc(shaders, draw_state, variant, ShaderKind::Pixel, Some(render::ShaderState::Trail));
        let poi_v = Self::lookup_shaders_arc(shaders, draw_state, variant, ShaderKind::Vertex, Some(render::ShaderState::Poi));
        let poi_p = Self::lookup_shaders_arc(shaders, draw_state, variant, ShaderKind::Pixel, Some(render::ShaderState::Poi));

        match (trail_v, trail_p) {
            (Some((Some(v), _)), Some((_, p))) => {
                self.shader_trail = Some(v.clone());
                self.shader_p_trail = p.cloned();
            },
            _ => return false,
        }
        match (poi_v, poi_p) {
            (Some((Some(v), _)), Some((_, p))) => {
                self.shader_poi = Some(v.clone());
                self.shader_p_poi = match p {
                    #[cfg(todo)]
                    Some(p) if Some(p.as_d3d_raw()) == self.shader_p_trail.as_ref().map(|p| p.as_d3d_raw()) =>
                        None,
                    p => p.cloned(),
                };
            },
            _ => return false,
        }
        self.shader_variant = Some(variant);
        true
    }
    pub fn lookup_shaders_arc<'a>(
        shaders: &'a ShaderLoader,
        draw_state: &mut PackRenderState,
        variant: render::ArcShaderVariant,
        kind: ShaderKind,
        entity: Option<render::ShaderState>,
    ) -> Option<(Option<&'a (dx11::ShaderV, dx11::shader::InputLayout)>, Option<&'a dx11::ShaderP>)> {
        let Some(id) = variant.id(kind, entity) else {
            return Some((None, None))
        };
        match kind {
            ShaderKind::Vertex => match shaders.vertex.get(id) {
                Some(v) => return Some((Some(v), None)),
                _ => (),
            },
            ShaderKind::Pixel => match shaders.pixel.get(id) {
                Some(v) => return Some((None, v.as_ref())),
                _ => (),
            }
        }
        let template = variant.template_id(kind, entity)
            .and_then(|pid| shaders.partial.get(pid));
        if draw_state.shaders_incomplete.insert((kind, id)) {
            let req = template.map(|t| PathingEvent::LoadShader {
                kind,
                variant,
                entity,
                template: t.clone(),
            });
            match req.map(PathingController::try_send) {
                None =>
                    log::warn!("shader {id} missing"),
                Some(true) =>
                    log::debug!("requesting shader {id}"),
                Some(false) => {
                    draw_state.shaders_incomplete.remove(&(kind, id));
                },
            }
        }

        None
    }
    #[inline]
    pub fn update_shared(
        &mut self,
        device_context: &Dx11Context,
        device: &Dx11Device,
        shared_v: &instance::ConstantDataV,
        shared_p: &instance::ConstantDataP,
    ) {
        match &mut self.shared_cb_p {
            Some(cb) => {
                cb.update_singleton(device_context, shared_p);
            },
            cb => *cb = rt::log::error_ok(ConstantBufferP::new_with_data(device, shared_p)),
        }
        match &mut self.shared_cb_v {
            Some(cb) => {
                cb.update_singleton(device_context, shared_v);
            },
            cb => *cb = rt::log::error_ok(ConstantBufferV::new_with_data(device, shared_v)),
        }
    }
    pub fn clear(&mut self) {
        self.clear_buffers();
        self.poi_vb = None;
        self.poi_vb_trans = None;
        self.shader_variant = None;
        self.shader_trail = None;
        self.shader_poi = None;
        self.shader_p_trail = None;
        self.shader_p_poi = None;
        self.shared_cb_v = None;
        self.shared_cb_p = None;
        STATS_ENTITY_INSTANCE_SIZE.reset(0);
    }
    pub fn clear_buffers(&mut self) {
        self.len = 0;
        self.entities_ib = None;
        self.dirty = false;
    }
    pub fn cleanup_background(mut self) {
        self.len = 0;
        mem::forget(self.entities_ib.take());
        mem::forget(self.shader_poi.take());
        mem::forget(self.shader_trail.take());
        mem::forget(self.shader_p_poi.take());
        mem::forget(self.shader_p_trail.take());
        mem::forget(self.poi_vb.take());
        mem::forget(self.poi_vb_trans.take());
        mem::forget(self.shared_cb_v.take());
        mem::forget(self.shared_cb_p.take());
        if let Some(poi_common) = self.poi_common.take() {
            poi_common.cleanup_background();
        }
    }
}

#[derive(Debug, Default)]
pub struct PackRenderState {
    pub drawn_incomplete: FxHashSet<MarkerId>,
    pub shaders_incomplete: FxHashSet<(ShaderKind, &'static str)>,
    pub prev_map_id: Option<MapIndex>,
    /// TODO: stash this in a common place like machine maybe?
    pub prev_waiting: bool,
    pub anims: BTreeMap<PoiMapPath, f32>,
    pub anim_stop: BTreeSet<PoiMapPath>,
    pub primary_draw: bool,
    pub primary_draw_map: bool,
    #[cfg(todo)]
    pub drawn_visible: BitSet,
}
impl PackRenderState {
    pub fn clear(&mut self) {
        self.drawn_incomplete = Default::default();
        self.shaders_incomplete = Default::default();
        self.clear_anims();
        #[cfg(todo)]
        {
            self.drawn_visible = Default::default();
        }
    }
    pub fn clear_active(&mut self) {
        self.drawn_incomplete.clear();
        #[cfg(todo)]
        {
            self.drawn_visible.clear();
        }
    }
    pub fn clear_anims(&mut self) {
        self.anims.clear();
        self.anim_stop.clear();
    }

    #[inline]
    pub fn is_secondary_draw(&self) -> bool {
        !self.primary_draw & !self.primary_draw_map
    }
    pub fn mark_incomplete(&mut self, id: &MarkerId) -> bool {
        if self.is_secondary_draw() {
            return false
        }
        self.drawn_incomplete.insert(id.clone());
        !self.primary_draw
    }

    pub(super) fn poi_get_anim_end(&self, lpath: PoiMapPath) -> Option<f32> {
        self.anims.get(&lpath).copied()
    }

    fn prune_anims(&mut self, when: f32) {
        self.anims.retain(|path, end| {
            let ongoing = *end > when;
            if !ongoing {
                self.anim_stop.insert(*path);
            }
            ongoing
        });
    }
    fn end_anims(&mut self, pack_data: &mut IndexedList<PackRegistryNs, PackIndex, [PackRenderData]>) -> bool {
        let mut dirty = false;
        for lpath in mem::take(&mut self.anim_stop).into_iter() {
            let Some(pack) = pack_data.lookup_mut(&lpath.root.root) else { continue };
            let lpath: LoadedPoiPath = lpath.unscope();
            let Some(poi) = pack.pois.lookup_mut(&lpath) else { continue };
            if poi.anim.is_some() {
                dirty = true;
                poi.anim = None;
            }
        }
        dirty
    }
}

#[derive(Default)]
pub struct PackRenderList {
    pub(crate) spacepacks: Arc<SpacePackCollection>,
    draw_order_heap: render::RenderOrderHeap<usize>,
    draw_order_cache: Vec<usize>,
    draw_unorder_cache: Vec<usize>,
    pub(crate) draw_order_cache_id: Option<NonZero<usize>>,
    dirty: bool,
    pub(crate) unordered_last: bool,
}
impl PackRenderList {
    #[inline]
    pub fn setup_frame(&mut self) {
        #[cfg(todo = "unnecessary")]
        {
            self.draw_order_heap.clear();
        }
    }
    #[inline]
    pub fn prepare_frame(&mut self) {
        if mem::replace(&mut self.dirty, false) {
            let shapes = self.spacepacks.render_entities.entities.len();
            let min_cap = shapes / 8;
            self.draw_order_heap.clear();
            self.draw_order_heap.reserve(min_cap);
            self.draw_order_cache.clear();
            self.draw_order_cache.reserve(min_cap);
            self.draw_unorder_cache.clear();
            self.draw_unorder_cache.reserve(min_cap / 2);
            self.draw_order_cache_id = None;
        }
    }
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }
    pub fn cleanup(&mut self) {
        self.draw_order_heap = Default::default();
        self.draw_order_cache = Default::default();
        self.draw_unorder_cache = Default::default();
        self.draw_order_cache_id = None;
        self.dirty = true;
    }

    /// TODO: actual dirty check?
    pub fn update_space(&mut self, spacepacks: &Arc<SpacePackCollection>) -> bool {
        let dirty = ArcPtrCmp::from_mut(&mut self.spacepacks).clone_from_arc(spacepacks);
        self.dirty |= dirty;
        self.draw_order_cache_id = None;
        true
    }

    /// adding some wiggle room around the map edges...
    ///
    /// TODO: impl trait and check with rotation instead or something?
    pub fn map_bounds_to_query(_map: MapContext, mut bounds: Box3<DrawSpace>) -> aabb::Aabb<f32, 3> {
        let buffer = bounds.size() * 0.15;
        bounds.min.x -= buffer.width;
        bounds.min.z -= buffer.depth;
        bounds.max.x += buffer.width;
        bounds.max.z += buffer.depth;
        box3aabb(bounds)
    }
    /// TODO: filter by visibility flags here?
    pub fn iter_markers_map<'a, 'e, Q: BvhQuery<3>>(
        &'a mut self,
        pack_data: &'e IndexedList<PackRegistryNs, PackIndex, [PackRenderData]>,
        _map: MapContext,
        query: &'a Q,
    ) -> impl Iterator<Item = (&'e PackRenderData, usize, &'a MarkerId)> {
        self.iter_entities_visible(None, query, move |e, idx, id| {
            let pos = match e.extra.get(idx) {
                _ if id.get_marker_index().namespace() == MarkerIndex::NS_TRAIL =>
                    None,
                None => None,
                Some(extra) if extra.position.x.is_infinite() => Some(i32::MIN),
                Some(extra) => Some(render::RenderOrderSort::dist_to_sort_with(extra.position.y, render::RenderOrderSort::DIST_FACTOR_CONSERVATIVE)),
            };
            Some((pos, idx))
        }).filter_map(|(idx, id)| {
            let pack_path = id.get_marker_pack_path();
            pack_data.lookup_ref(&pack_path).map(|p| (p, idx, id))
        })
    }
    #[cfg(todo = "unused")]
    pub fn iter_markers_all<'a, 'e>(
        &'a self,
        pack_data: &'e IndexedList<PackRegistryNs, PackIndex, [PackRenderData]>,
    ) -> impl Iterator<Item = (&'e PackRenderData, &'a MarkerId)> {
        let shapes = &self.spacepacks.render_entities.entities[..];
        shapes.iter().filter_map(move |shape| {
            if shape.is_invalid() {
                return None
            }
            let id = &shape.value.id;
            let pack_path = id.get_marker_pack_path();
            let pack = pack_data.lookup_ref(&pack_path);
            pack.map(|p| (p, id))
        })
    }
    pub fn iter_markers_visible<'a, 'e, Q: BvhQuery<3>>(
        &'a mut self,
        pack_data: &'e IndexedList<PackRegistryNs, PackIndex, [PackRenderData]>,
        query_id: Option<NonZero<usize>>,
        query: &'a Q,
        camera: &'_ RenderPosition,
    ) -> impl Iterator<Item = (&'e PackRenderData, usize, &'a MarkerId)> {
        let key = render::RenderOrderSort::with_camera(camera);
        self.iter_entities_visible(query_id, query, move |e, idx, id| {
            let ignore_draw_order = id.get_marker_index().namespace() == MarkerIndex::NS_TRAIL;
            e.extra.get(idx).map(|extra| {
                let pos = match ignore_draw_order {
                    true => None,
                    false if extra.position.x.is_infinite() => None,
                    false => Some(key.cam_dist_order_for(extra.position)),
                };
                (pos, idx)
            })
        }).filter_map(|(idx, id)| {
            let pack_path = id.get_marker_pack_path();
            pack_data.lookup_ref(&pack_path).map(|p| (p, idx, id))
        })
    }
    pub(crate) fn iter_entities_visible<'a, Q, F>(
        &'a mut self,
        query_id: Option<NonZero<usize>>,
        query: &'a Q,
        mut filter: F,
    ) -> impl Iterator<Item = (usize, &'a MarkerId)> + 'a where
        Q: BvhQuery<3>,
        F: FnMut(&SpaceEntities, usize, &MarkerId) -> Option<(Option<i32>, usize)> + 'a,
    {
        let entities = &self.spacepacks.render_entities;
        let shapes = &entities.entities[..];
        let mut bvh_iter = Either::Left(self.spacepacks.bvh_iter(query).filter_map(move |(idx, id)|
            filter(entities, idx, id)
        ));
        let reverse = self.unordered_last;
        let mut cache = match query_id {
            None => None,
            id @ Some(_id) if id == self.draw_order_cache_id && !self.draw_order_cache.is_empty() => {
                frame_log!("space; ordercache@{_id} reused");
                let (start, end) = match reverse {
                    true => (
                        self.draw_order_cache.iter(),
                        self.draw_unorder_cache.iter(),
                    ),
                    _ => (
                        self.draw_unorder_cache.iter(),
                        self.draw_order_cache.iter(),
                    ),
                };
                bvh_iter = Either::Right(
                    start.chain(end).copied()
                );
                None
            },
            Some(id) => {
                frame_log!("space; ordercache@{id} populate");
                self.draw_order_cache.clear();
                self.draw_unorder_cache.clear();
                self.draw_order_cache_id = None;
                Some((&mut self.draw_order_cache, &mut self.draw_unorder_cache))
            },
        };
        self.draw_order_heap.clear();
        let cache_id = &mut self.draw_order_cache_id;

        let mut ordered = match bvh_iter {
            Either::Left(bvh_iter) => Either::Left(render::RenderOrderBuilder {
                bvh_iter,
                draw_order_heap: &mut self.draw_order_heap,
            }),
            Either::Right(c) => Either::Right(c),
        };
        let mut reverse = reverse.then_some(0usize);
        iter::from_fn(move || {
            match &mut ordered {
                Either::Left(..) if !matches!(reverse, None | Some(0)) => (),
                Either::Left(ordered) => loop {
                    let cache = &mut cache;
                    let ordered = ordered.next().map(|(dist, idx)| {
                        if let Some((cache, unordered)) = &mut *cache {
                            match dist {
                                None => unordered.push(idx),
                                Some(_dist) => cache.push(idx),
                            }
                        }
                        (dist, idx)
                    });
                    match query_id {
                        Some(id) if ordered.is_none() && cache.is_some() =>
                            *cache_id = Some(id),
                        _ => (),
                    }
                    match ordered {
                        Some((None, ..)) if reverse.is_some() => continue,
                        Some((_, idx)) => return Some(idx),
                        None if reverse.is_some() => break,
                        None => break,
                    }
                },
                Either::Right(cache) => return cache.next(),
            }
            let rev = reverse.and_then(|i| match cache {
                Some((_, ref unordered)) => unordered.get(i).copied(),
                _ => None,
            });
            if let Some(i) = &mut reverse {
                *i += 1;
            }
            rev
        }).map(move |idx| {
            let mid = unsafe { shapes.get_unchecked(idx) };
            (idx, &mid.value.id)
        })
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum ShaderState {
    None,
    Trail,
    Poi,
}
pub struct ArcrenderSettings {
    pub trail_anim_speed: f32,
    pub trail_alpha: f32,
    pub trail_expansion: TrailScale,
    pub trail_can_fade: bool,
    pub trail_distance_fade: bool,
    pub trail_overlap_threshold: Option<f32>,
    pub trail_intensity: Option<f32>,
    pub trail_texture: TrailTextureMap,
    pub trail_flags: u32,
    pub poi_anim_speed: f32,
    pub poi_can_fade: bool,
    pub poi_alpha: f32,
    pub poi_expansion: PoiScale,
    pub poi_distance_fade: bool,
    pub poi_limit_size: bool,
    #[cfg(todo)]
    pub poi_overlap_threshold: Option<f32>,
    #[cfg(todo)]
    pub poi_intensity: Option<f32>,
    pub poi_flags: u32,
    pub feather_scale: Option<Vector2>,
    pub feather_scale1: Option<f32>,
}
impl ArcrenderSettings {
    pub const DEFAULT: Self = Self {
        poi_alpha: 1.0f32,
        poi_expansion: PoiScale::DEFAULT,
        trail_expansion: TrailScale::DEFAULT,
        trail_alpha: 1.0f32,
        poi_can_fade: SpaceSettings::DEFAULT_PLAYER_OVERLAP_POI,
        poi_limit_size: SpaceSettings::DEFAULT_POI_LIMIT_SIZE,
        trail_can_fade: SpaceSettings::DEFAULT_PLAYER_OVERLAP_THRESHOLD > 0.0,
        trail_anim_speed: SpaceSettings::DEFAULT_TRAIL_ANIM,
        poi_distance_fade: SpaceSettings::DEFAULT_DISTANCE_FADE_RANGE,
        trail_distance_fade: SpaceSettings::DEFAULT_DISTANCE_FADE_RANGE,
        poi_anim_speed: 1.0,
        trail_overlap_threshold: None,
        #[cfg(todo)]
        poi_overlap_threshold: None,
        trail_intensity: None,
        #[cfg(todo)]
        poi_intensity: None,
        trail_texture: TrailTextureMap::DEFAULT,
        trail_flags: 0,
        poi_flags: 0,
        feather_scale: None,
        feather_scale1: None,
    };

    pub const OVERLAP_THRESHOLD_OFF: f32 = 0.01;
    #[cfg(todo = "unnecessary")]
    const OVERLAP_THRESHOLD_DEFAULT: f32 = SpaceSettings::DEFAULT_PLAYER_OVERLAP_THRESHOLD;
    pub fn trail_player_feather(&self) -> f32 {
        self.trail_overlap_threshold.unwrap_or(Self::OVERLAP_THRESHOLD_OFF)
    }
    #[cfg(todo)]
    pub fn poi_player_feather(&self) -> f32 {
        self.poi_overlap_threshold.unwrap_or(Self::OVERLAP_THRESHOLD_OFF)
    }

    pub const INTENSITY_OFF: f32 = 1_000_000.0;
    pub fn trail_intensity(&self) -> f32 {
        self.trail_intensity.unwrap_or(Self::INTENSITY_OFF)
    }
    #[cfg(todo)]
    pub fn poi_intensity(&self) -> f32 {
        self.poi_intensity.unwrap_or(Self::INTENSITY_OFF)
    }

    pub const FEATHER_SCALE_SQUARE: Vector2 = Vector2::new(0.065f32, 0.0825f32);
    pub fn set_feather_scale(&mut self, feather_scale: Option<f32>, display_size: Option<Size2<ScreenSpace>>) {
        self.feather_scale1 = feather_scale;
        self.feather_scale = display_size.and_then(|sz| self.edge_feather_for(sz));
    }
    fn edge_feather_for(&self, display_size: Size2<ScreenSpace>) -> Option<Vector2> {
        let aspect_ratio = display_size.width / display_size.height;
        let aspect_ratio_recip = display_size.height / display_size.width;
        self.feather_scale1.map(|scale| {
            let normalized = match () {
                #[cfg(todo = "unnecessary")]
                _ => (Vector2::new(aspect_ratio_recip, aspect_ratio) * Self::FEATHER_SCALE_SQUARE).recip(),
                _ => {
                    const FEATHER_SCALE_SQUARE_RECIP: Vector2 = Vector2::new(1.0f32 / ArcrenderSettings::FEATHER_SCALE_SQUARE.x, 1.0f32 / ArcrenderSettings::FEATHER_SCALE_SQUARE.y);
                    Vector2::new(aspect_ratio, aspect_ratio_recip) * FEATHER_SCALE_SQUARE_RECIP
                },
            };
            scale * normalized
        })
    }

    pub const FEATHER_SCALE_NONE: f32 = 1.0e8;
    pub fn edge_feather(&self, display_size: Size2<ScreenSpace>) -> Vector2 {
        match self.feather_scale {
            None if self.feather_scale1.is_some() =>
                self.edge_feather_for(display_size),
            s => s,
        }.unwrap_or(Vector2::splat(Self::FEATHER_SCALE_NONE))
    }
    pub const VIEWPORT_NONE: f32 = 1.0 / 10000.0;
    pub fn edge_viewport(&self, viewport_size: Size2) -> Vector2 {
        self.feather_scale1.is_some().then_some(viewport_size)
            .map(|size| size.to_vector().recip() * 2.0)
            .unwrap_or(Vector2::splat(Self::VIEWPORT_NONE))
    }

    /// TODO: technically just needs aspect ratio...
    pub fn set_from(&mut self, settings: &PathingSettings, display_size: Option<Size2<ScreenSpace>>) {
        let space = &settings.space;
        self.trail_anim_speed = space.trail_anim_space();
        self.trail_distance_fade = space.distance_fade_range();
        self.trail_overlap_threshold = space.player_overlap_threshold();
        self.trail_intensity = space.distance_fade_intensity();
        self.poi_distance_fade = space.distance_fade_range();
        self.poi_can_fade = space.player_overlap_poi();
        self.trail_can_fade = space.player_overlap_threshold().is_some();
        self.poi_limit_size = space.poi_limit_size();
        self.poi_expansion = PoiScale::with_scale(space.poi_scale_space());
        let prev_trail_expansion = mem::replace(&mut self.trail_expansion, TrailScale::with_scale(space.trail_scale_space()));
        self.trail_alpha = space.trail_alpha();
        self.poi_alpha = space.poi_alpha();
        match space.trail_textured_space() {
            true if prev_trail_expansion == self.trail_expansion
                && self.trail_texture != TrailTextureMap::UNTEXTURED =>
                (),
            true => {
                self.trail_texture.set_scale_from_expansion(self.trail_expansion);
                self.trail_texture.v_offset = 0.0;
            },
            false => self.trail_texture = TrailTextureMap::UNTEXTURED,
        }
        match space.edge_feather_scale() {
            s if self.feather_scale1 == s => (),
            s => self.set_feather_scale(s, display_size),
        }
    }

    #[inline]
    pub fn apply_p(
        &self,
        shared_p: &mut instance::ConstantDataP,
        viewport_size: Size2<ScreenSpace>,
    ) {
        let render = &mut shared_p.render;
        #[cfg(todo)]
        {
            render.viewport = viewport_size.to_vector();
        }
        render.player_feather = self.trail_player_feather();
        render.distance_fade = self.trail_intensity();
        render.edge_feather = self.edge_feather(viewport_size).to_array();
        render.edge_feather_viewport = self.edge_viewport(viewport_size.cast());
    }
    #[inline]
    pub fn apply_v(
        &self,
        shared_v: &mut instance::ConstantDataV,
    ) {
        self.apply_v_trail(&mut shared_v.trail);
        self.apply_v_poi(&mut shared_v.poi);
    }
    #[inline]
    pub fn apply_v_trail(
        &self,
        trail_v: &mut instance::TrailConstantDataV,
    ) {
        trail_v.tex_scale = self.trail_texture.v_scale;
        trail_v.tex_offset = self.trail_texture.v_offset;
        let marker = &mut trail_v.marker;
        marker.alpha = self.trail_alpha;
        marker.scale = self.trail_expansion.normal_expansion;
        marker.anim_scale = self.trail_anim_speed;
        marker.flags =
            self.trail_distance_fade.then_some(instance::MarkerConstantDataV::FLAG_DISTANCE_FADE).unwrap_or(0) |
            (!self.trail_can_fade).then_some(instance::MarkerConstantDataV::FLAG_OBSCURE_FADE).unwrap_or(0) |
            self.trail_flags;
    }
    #[inline]
    pub fn apply_v_poi(
        &self,
        poi_v: &mut instance::PoiConstantDataV,
    ) {
        let marker = &mut poi_v.marker;
        marker.alpha = self.poi_alpha;
        marker.scale = self.poi_expansion.scale();
        marker.anim_scale = self.poi_anim_speed;
        marker.flags =
            self.poi_distance_fade.then_some(instance::MarkerConstantDataV::FLAG_DISTANCE_FADE).unwrap_or(0) |
            (!self.poi_can_fade).then_some(instance::MarkerConstantDataV::FLAG_OBSCURE_FADE).unwrap_or(0) |
            self.poi_limit_size.then_some(instance::MarkerConstantDataV::FLAG_POI_LIMIT_SIZE).unwrap_or(0) |
            self.poi_flags;
    }

    #[inline]
    pub fn setup_v(
        shared_v: &mut instance::ConstantDataV,
        viewport_size: Size2<ScreenSpace>,
        map_calibration: &MapCalibration,
        (camera_pos, camera_dir, _camera_up): &RenderPosition,
        player_pos: Option<Point3<DrawSpace>>,
        projection: Matrix4<f32>,
        view: Matrix4<f32>,
        anim_timestamp: Option<f32>,
    ) {
        shared_v.render.player_pos = player_pos.unwrap_or(Point3::splat(taimi_meta::spatial::IRRELEVANT_MID)).to_vector().cast();
        shared_v.render.anim_timestamp = anim_timestamp.unwrap_or(0.0);
        shared_v.render.camera_pos = camera_pos.to_vector().cast();
        shared_v.render.camera_dir = camera_dir.cast();
        shared_v.render.view = view;
        shared_v.render.projection = projection;
        shared_v.render.viewport_pixel_scale = 1.0 / viewport_size.height;
        #[cfg(todo = "unnecessary")]
        {
            shared_v.render.viewport_pixel_scale = viewport_size.dot(viewport_size).sqrt() * 2.0;
        }
        shared_v.poi.billboard = taimi_meta::coords::billboard_from_look(view.into());
        shared_v.poi.map_scale = map_calibration.local_space().scale.abs().y;
    }
    #[inline]
    pub fn setup_p(
        shared_p: &mut instance::ConstantDataP,
        blending: Option<(f32, f32)>,
    ) {
        shared_p.poi.marker.set_blend_factors(blending.map(|(bp, _)| bp));
        shared_p.trail.marker.set_blend_factors(blending.map(|(_, bt)| bt));
    }
}

pub static STATS_ENTITY_INSTANCE_SIZE: Counter = Counter::DEFAULT;
pub static STATS_ENTITY_DRAW: Counter = Counter::DEFAULT;
pub static STATS_ENTITY_DRAW_PASS: Counter = Counter::DEFAULT;
pub static STATS_ENTITY_DRAW_ALL: Counter = Counter::DEFAULT;
pub static STATS_ENTITY_COUNT: Counter = Counter::DEFAULT;
pub static STATS_ENTITY_DRAW_MAP: Counter = Counter::DEFAULT;
