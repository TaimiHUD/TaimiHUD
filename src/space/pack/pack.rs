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
            space::{SpacePackCollection, SpacePackShared, TextureLoadRequests, TrailGeometryRequests},
        },
        exports::runtime::{self as rt,
            textures::TextureSlot,
        },
        render::machine::{RenderMachine, RenderPosition},
        space::{
            dx11::RenderBackend,
            pack::{instance::{self, EntityInstanceBuffer, EntityInstanceData, PoiVertexBuffer}, PoiRender, TrailRender},
            DrawSpace,
        },
        resources::shader::ShaderLoader,
        settings::pathing::SpaceSettings,
    },
    anyhow::Context,
    bvh::aabb,
    glamour::{Box3, Point3},
    rustc_hash::FxHashSet,
    std::{collections::{BTreeSet, BTreeMap}, mem, ops, sync::Arc},
    std::time::Instant,
    taimi_d3d::dx11::prelude::*,
    taimi_d3d::dx11::buffer::{ConstantBufferP, ConstantBufferV},
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
        spatial::{box3aabb, cull::MapFrustum},
        ui::{LocalContext, MapContext},
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
    pub poi_common: PoiCommonRenderData,

    pub spacepacks: Watched<Arc<SpacePackCollection>>,
    pub trail_rx: TrailGeometryRequests,
    pub texture_rx: TextureLoadRequests,
    packs_rx: Option<watch::Receiver<SharedLoaderPacksInfo>>,
    packs_map: Option<watch::Receiver<SharedGameplayMap>>,

    pub render_list: PackRenderList,
    pub draw_state: PackRenderState,
    pub resources: PackRenderResources,
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
            resources: Default::default(),
            poi_common,
        })
    }

    fn mark_buffers_dirty(&mut self) {
        self.poi_common.clear();
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
        let mut ibs_dirty = self.poi_common.is_empty();
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
                space_dirty = true;
            }
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
        if let Some(map_id) = map_id {
            if let Some(packs_map) = &packs_map_changed {
                log::trace!("gameplay maps rx @ {map_id}");
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
                    let ib_pack_len = self.poi_common.ib_len_for_packs(&self.pack_data);
                    let ib_len = self.poi_common.ib_len();
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
                            .setup_geometry(device, geometry)
                            .context("loading trail geometry"),
                    ),
                };
                if res.is_none() {
                    trail.disable();
                }
            }
            for (marker_path, texture) in self.texture_rx.try_recv_fulfilled() {
                // texture loader should be notified, so no need to do anything really?
                let id = MarkerId::for_marker(marker_path);
                if texture.is_none() {
                    log::debug!("request for tex {marker_path} failed");
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
                let arcrender = settings.map(|s| s.goggles.arcrender_enabled()).unwrap_or(false);
                if arcrender {
                    let res = self.resources.prepare(device, self.pack_data.map_ref_as_slice(), &mut self.draw_state, self.spacepacks.render_entities.entities.iter().map(|e| e.id)).context("RenderResources::prepare");
                    rt::log::error_ok(res);
                }
            }
        } else {
            drop(packs_map_changed);
        }
        self.draw_state.clear_active();
        if map_id.is_some() {
            self.poi_common.update_fallback(device, machine);
            if ibs_dirty {
                self.recreate_buffers(device, machine)?;
            }
        }

        Ok(map_id.is_some() && !machine.is_ingame_paused())
    }
    pub fn prepare_frame(
        &mut self,
        _machine: &mut RenderMachine,
        _device_context: &Dx11Context,
        anim_timestamp: Option<f32>,
        fresh: bool,
    ) -> anyhow::Result<()> {
        if fresh {
            self.resources.anim_timestamp = anim_timestamp;
            if let Some(anim_timestamp) = self.resources.anim_timestamp {
                self.draw_state.prune_anims(anim_timestamp);
                if self.draw_state.end_anims(self.pack_data.map_mut_as_slice()) {
                    self.resources.dirty = true;
                }
            } else {
                self.draw_state.clear_anims();
            }
        }
        self.render_list.prepare_frame();
        Ok(())
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
        self.poi_common.rebuild_ib(device, machine, &self.pack_data)?;

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

    pub fn draw(
        &mut self,
        camera: RenderPosition,
        frustum: &MapFrustum,
        backend: &RenderBackend,
        context: &Dx11Context,
        arcrender: bool,
    ) {
        let Some(spacepacks) = self.spacepacks.cached.as_ref() else { return };
        let entities =
            self.render_list
                .iter_markers_visible(self.pack_data.map_ref_as_slice(), frustum, camera);
        match arcrender {
            true => {
                if self.resources.shader_trail.is_none() {
                    rt::log::error_ok(self.resources.prepare_shaders(&backend.shaders));
                }
                let mut draw = render::DrawSpaceArc {
                    context,
                    resources: &self.resources,
                    poi_common: &self.poi_common,
                    state: None,
                    last_quad: None,
                };
                Self::draw_entities(
                    &mut self.draw_state,
                    &mut draw,
                    entities,
                );
            },
            false => {
                let mut draw = render::DrawSpacePack {
                    context,
                    shaders: &backend.shaders,
                    poi_common: &self.poi_common,
                    state: None,
                    shader_trail: None,
                };
                Self::draw_entities(
                    &mut self.draw_state,
                    &mut draw,
                    entities,
                );
            },
        }
        STATS_ENTITY_COUNT.reset_with(|| spacepacks.render_entities.entities.len());
    }
    /// TESTING123
    #[cfg(deleteme)]
    pub fn draw_arc(
        &mut self,
        camera: RenderPosition,
        frustum: &MapFrustum,
        backend: &RenderBackend,
        device_context: &Dx11Context,
    ) {
        let Some(spacepacks) = self.spacepacks.cached.as_ref() else { return };
        if self.resources.shader_trail.is_none() {
            rt::log::error_ok(self.resources.prepare_shaders(&backend.shaders));
        }
        let mut shader_state = ShaderState::None;
        let mut shader_poi_quad = None;
        let mut num_drawn = 0usize;
        for (i, shape) in spacepacks.render_entities.entities.iter().enumerate() {
            let e = &shape.value;
            if e.is_invalid() { continue }
            #[cfg(todo)]
            if !frustum.intersects_aabb(&shape.bounds) { continue }
            if !frustum.intersects(&shape.bounds) { continue }
            let Some(pack_path) = e.id.marker_path::<PackMapPath>() else { continue };
            let Some(pack_data) = self.pack_data.lookup_ref(&pack_path.root.root) else { continue };
            let render_id = pack_path.path;
            match render_id.namespace() {
                MarkerIndex::NS_TRAIL => {
                    let (t, s) = render_id.index_trail_section_unchecked();
                    let tpath: LoadedTrailPath = LoadedTrailPath::with_path(t);
                    #[cfg(todo)]
                    let path = tpath.rel(TrailSectionPath::with_path(s));
                    let Some(trail) = pack_data.trails.lookup_ref(&tpath) else { continue };
                    //if !matches!(trail.texture, Some(TextureSlot::Loaded(..))) { continue }
                    if trail.texture.is_none() { continue }
                    let Some(vb) = &trail.section_vb_ng else { continue };
                    let Some(ops::Range { start, end}) = trail.section_geometry_vertices(s) else { continue };
                    if shader_state == ShaderState::None {
                        let Some(shaderp) = &self.resources.shader_p else { continue };
                        let Some(ib) = &self.resources.entities_ib else { continue };
                        let Some(cb_p) = &self.resources.shared_cb_p else { continue };
                        let Some(cb_v) = &self.resources.shared_cb_v else { continue };
                        shaderp.set(device_context);
                        ib.set(device_context, 1);
                        cb_p.set(device_context, 0);
                        cb_v.set(device_context, 0);
                    }
                    if shader_state != ShaderState::Trail {
                        let Some((shaderv, shaderl)) = &self.resources.shader_trail else { continue };
                        shaderv.set(device_context);
                        shaderl.set(device_context);
                        shader_state = ShaderState::Trail;
                    }
                    trail.bind_texture(device_context, &self.poi_common, LocalContext::MAP);
                    vb.set(device_context, 0);
                    unsafe {
                        device_context.DrawInstanced(end - start, 1, start, i as u32);
                    }
                },
                MarkerIndex::NS_POI => {
                    let path = LoadedPoiPath::with_path(render_id.index_poi_unchecked());
                    let Some(poi) = pack_data.pois.lookup_ref(&path) else { continue };
                    if shader_state == ShaderState::None {
                        let Some(shaderp) = &self.resources.shader_p else { continue };
                        let Some(ib) = &self.resources.entities_ib else { continue };
                        let Some(cb_p) = &self.resources.shared_cb_p else { continue };
                        let Some(cb_v) = &self.resources.shared_cb_v else { continue };
                        shaderp.set(device_context);
                        ib.set(device_context, 1);
                        cb_p.set(device_context, 0);
                        cb_v.set(device_context, 0);
                    }
                    let vb_quad = match poi.occlude {
                        true => self.resources.poi_vb_trans.as_ref(),
                        _ => {
                            if poi.icon.is_none() { continue }
                            self.resources.poi_vb.as_ref()
                        },
                    };
                    let Some(vb_quad) = vb_quad else { continue };
                    if shader_state != ShaderState::Poi {
                        let Some((shaderv, shaderl)) = &self.resources.shader_poi else { continue };
                        shaderv.set(device_context);
                        shaderl.set(device_context);
                        shader_poi_quad = Some(vb_quad);
                        vb_quad.set(device_context, 0);
                        shader_state = ShaderState::Poi;
                    } else if shader_poi_quad != Some(vb_quad) {
                        vb_quad.set(device_context, 0);
                        shader_poi_quad = Some(vb_quad);
                    }
                    poi.bind_texture(device_context, &self.poi_common, LocalContext::MAP);
                    unsafe {
                        device_context.DrawInstanced(instance::TrailVertex::POI_QUAD.len() as u32, 1, 0, i as u32);
                    }
                },
                _ => (),
            }
            num_drawn += 1;
        }
        STATS_ENTITY_DRAW.reset(num_drawn);
    }
    #[cfg(feature = "goggles")]
    pub fn draw_obscured(
        &mut self,
        camera: RenderPosition,
        frustum: &MapFrustum,
        backend: &RenderBackend,
        context: &Dx11Context,
        arcrender: bool,
    ) {
        let entities =
            self.render_list
                .iter_markers_visible(self.pack_data.map_ref_as_slice(), frustum, camera);
        match arcrender {
            true => {
                if self.resources.shader_trail.is_none() {
                    rt::log::error_ok(self.resources.prepare_shaders(&backend.shaders));
                }
                let mut draw = render::DrawSpaceArc {
                    context,
                    resources: &self.resources,
                    poi_common: &self.poi_common,
                    state: None,
                    last_quad: None,
                };
                Self::draw_entities(
                    &mut self.draw_state,
                    &mut draw,
                    entities,
                );
            },
            false => {
                let mut draw = render::DrawSpacePack {
                    context,
                    shaders: &backend.shaders,
                    poi_common: &self.poi_common,
                    state: None,
                    shader_trail: None,
                };
                Self::draw_entities(
                    &mut self.draw_state,
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
                    if trail.report_incomplete(&marker_id, draw_state, path) {
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
        STATS_ENTITY_DRAW.reset(num_drawn);
    }
    pub fn draw_map_entities<'e, E>(
        draw_state: &mut PackRenderState,
        poi_common: &PoiCommonRenderData,
        device_context: &Dx11Context,
        backend: &RenderBackend,
        map: MapContext,
        entities: E,
    ) where
        E: IntoIterator<Item = (&'e PackRenderData, &'e MarkerId)>,
    {
        let mut shader_state = ShaderState::None;
        let mut num_drawn = 0usize;
        let ctx = LocalContext::/*Map(map)*/MAP;
        for (pack_data, marker_id) in entities {
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
                    if trail.report_incomplete(&marker_id, draw_state, path) {
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
                },
            }
            num_drawn += 1;
        }
        STATS_ENTITY_DRAW_MAP.reset(num_drawn);
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
        self.poi_common.clear();
        self.resources.clear();
    }
    /// See [crate::space::engine::Engine::cleanup_background]
    ///
    /// TODO: revisit, avoid, etc
    pub fn cleanup_background(self) {
        let Self { pack_data, poi_common, resources, .. } = self;
        poi_common.cleanup_background();
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
    pub len: usize,
    pub dirty: bool,
    pub anim_timestamp: Option<f32>,

    pub entities_ib: Option<EntityInstanceBuffer>,
    pub shader_poi: Option<(taimi_d3d::dx11::ShaderV, taimi_d3d::dx11::shader::InputLayout)>,
    pub shader_trail: Option<(taimi_d3d::dx11::ShaderV, taimi_d3d::dx11::shader::InputLayout)>,
    pub shader_p: Option<taimi_d3d::dx11::ShaderP>,
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
    pub fn update_shared(&mut self, device_context: &Dx11Context, backend: &RenderBackend, machine: &RenderMachine, settings: &ArcrenderSettings) {
        use glam::Vec2;
        let shared_p = instance::ConstantDataP {
            render: instance::RenderConstantDataP {
                viewport: Vec2::new(
                    backend.perspective_handler.constant_buffer_pixel_data.viewport_param.x,
                    backend.perspective_handler.constant_buffer_pixel_data.viewport_param.y,
                ).into(),
                player_feather: backend.perspective_handler.constant_buffer_pixel_data.overlap_threshold(),
                distance_fade: backend.perspective_handler.constant_buffer_pixel_data.distance_param.y,
                edge_feather: Vec2::new(
                    backend.perspective_handler.constant_buffer_pixel_data.distance_param.z,
                    backend.perspective_handler.constant_buffer_pixel_data.distance_param.w,
                ).into(),
            },
        };
        match &mut self.shared_cb_p {
            Some(cb) => {
                cb.update_singleton(device_context, &shared_p);
            },
            cb => *cb = rt::log::error_ok(ConstantBufferP::new_with_data(&backend.device, &shared_p)),
        }
        let (camera_pos, camera_dir) = match machine.get_camera_mumblelink() {
            Some((pos, dir, ..)) => (
                pos.to_vector().to_untyped(),
                dir.to_untyped(),
            ),
            None => (glamour::Vector3::ZERO, glamour::Vector3::Z),
        };
        let shared_v = instance::ConstantDataV {
            render: instance::RenderConstantDataV {
                player_pos: backend.perspective_handler.constant_buffer_data.player.truncate().into(),
                anim_timestamp: self.anim_timestamp.unwrap_or(0.0),
                camera_pos,
                camera_dir,
                view: backend.perspective_handler.constant_buffer_data.view.into(),
                projection: backend.perspective_handler.constant_buffer_data.projection.into(),
                _padding0: 0.0,
                viewport_pixel_scale: 1.0 / backend.perspective_handler.constant_buffer_pixel_data.viewport_param.y,
                #[cfg(todo = "unnecessary")]
                viewport_pixel_scale: {
                    let vp_size = glam::Vec2::new(
                        backend.perspective_handler.constant_buffer_pixel_data.viewport_param.x,
                        backend.perspective_handler.constant_buffer_pixel_data.viewport_param.y,
                    );
                    vp_size.dot(vp_size).sqrt() * 2.0
                },
                _padding2: glamour::Vector4::ZERO,
            },
            poi: instance::PoiConstantDataV {
                marker: instance::MarkerConstantDataV {
                    alpha: backend.perspective_handler.alpha(),
                    scale: backend.perspective_handler.constant_buffer_data.poi_expansion.scale(),
                    anim_scale: settings.poi_anim_speed,
                    flags:
                        settings.poi_distance_fade.then_some(instance::MarkerConstantDataV::FLAG_DISTANCE_FADE).unwrap_or(0) |
                        (!settings.poi_can_fade).then_some(instance::MarkerConstantDataV::FLAG_OBSCURE_FADE).unwrap_or(0) |
                        settings.poi_limit_size.then_some(instance::MarkerConstantDataV::FLAG_POI_LIMIT_SIZE).unwrap_or(0) |
                        settings.poi_flags,
                },
                billboard: taimi_meta::coords::billboard_from_look(backend.perspective_handler.constant_buffer_data.view.into()),
                map_scale: machine.map.calibration.local_space().scale.abs().y,
                _padding0: glamour::Vector3::ZERO,
            },
            trail: instance::TrailConstantDataV {
                marker: instance::MarkerConstantDataV {
                    alpha: backend.perspective_handler.alpha(),
                    scale: backend.perspective_handler.constant_buffer_data.trail_expansion.normal_expansion,
                    anim_scale: settings.trail_anim_speed,
                    flags:
                        settings.trail_distance_fade.then_some(instance::MarkerConstantDataV::FLAG_DISTANCE_FADE).unwrap_or(0) |
                        (!settings.trail_can_fade).then_some(instance::MarkerConstantDataV::FLAG_OBSCURE_FADE).unwrap_or(0) |
                        settings.trail_flags,
                },
                tex_scale: backend.perspective_handler.constant_buffer_data.trail_texture.v_scale,
                tex_offset: backend.perspective_handler.constant_buffer_data.trail_texture.v_offset,
                _padding0: glamour::Vector2::ZERO,
            },
        };
        match &mut self.shared_cb_v {
            Some(cb) => {
                cb.update_singleton(device_context, &shared_v);
            },
            cb => *cb = rt::log::error_ok(ConstantBufferV::new_with_data(&backend.device, &shared_v)),
        }
    }
    pub fn clear(&mut self) {
        self.clear_buffers();
        self.poi_vb = None;
        self.poi_vb_trans = None;
        self.shader_trail = None;
        self.shader_poi = None;
        self.shader_p = None;
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
        mem::forget(self.shader_p.take());
        mem::forget(self.poi_vb.take());
        mem::forget(self.poi_vb_trans.take());
        mem::forget(self.shared_cb_v.take());
        mem::forget(self.shared_cb_p.take());
    }
}

#[derive(Debug, Default)]
pub struct PackRenderState {
    pub drawn_incomplete: FxHashSet<MarkerId>,
    pub prev_map_id: Option<MapIndex>,
    /// TODO: stash this in a common place like machine maybe?
    pub prev_waiting: bool,
    pub anims: BTreeMap<PoiMapPath, f32>,
    pub anim_stop: BTreeSet<PoiMapPath>,
    #[cfg(todo)]
    pub drawn_visible: BitSet,
}
impl PackRenderState {
    pub fn clear(&mut self) {
        self.drawn_incomplete = Default::default();
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
    spacepacks: Arc<SpacePackCollection>,
    draw_order_heap: render::RenderOrderHeap<usize>,
    dirty: bool,
}
impl PackRenderList {
    pub fn prepare_frame(&mut self) {
        if mem::replace(&mut self.dirty, false) {
            let shapes = self.spacepacks.render_entities.entities.len();
            let min_cap = shapes / 8;
            self.draw_order_heap.clear();
            self.draw_order_heap.reserve(min_cap);
        }
    }

    /// TODO: actual dirty check?
    pub fn update_space(&mut self, spacepacks: &Arc<SpacePackCollection>) -> bool {
        let dirty = ArcPtrCmp::from_mut(&mut self.spacepacks).clone_from_arc(spacepacks);
        self.dirty |= dirty;
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
    pub fn iter_markers_map<'a, 'e, Q: aabb::IntersectsAabb<f32, 3>>(
        &'a self,
        pack_data: &'e IndexedList<PackRegistryNs, PackIndex, [PackRenderData]>,
        _map: MapContext,
        query: &'a Q,
    ) -> impl Iterator<Item = (&'e PackRenderData, &'a MarkerId)> {
        self.spacepacks.bvh_iter(query).filter_map(move |(_idx, id)| {
            let pack_path = id.get_marker_pack_path();
            pack_data.lookup_ref(&pack_path).map(|p| (p, id))
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
    pub fn iter_markers_visible<'a, 'e, Q: aabb::IntersectsAabb<f32, 3>>(
        &'a mut self,
        pack_data: &'e IndexedList<PackRegistryNs, PackIndex, [PackRenderData]>,
        query: &'a Q,
        camera: RenderPosition,
    ) -> impl Iterator<Item = (&'e PackRenderData, usize, &'a MarkerId)> {
        self.iter_entities_visible(query, camera)
            .filter_map(|(idx, id)| {
                let pack_path = id.get_marker_pack_path();
                pack_data.lookup_ref(&pack_path).map(|p| (p, idx, id))
            })
    }
    fn iter_entities_visible<'a, Q: aabb::IntersectsAabb<f32, 3>>(
        &'a mut self,
        query: &'a Q,
        (cam_origin, cam_dir, _cam_up): RenderPosition,
    ) -> impl Iterator<Item = (usize, &'a MarkerId)> + 'a {
        let shapes = &self.spacepacks.render_entities.entities[..];
        let extra = &self.spacepacks.render_entities.extra[..];
        self.draw_order_heap.clear();

        let bvh_iter = self.spacepacks.bvh_iter(query).filter_map(move |(idx, _id)| {
            let ignore_draw_order = _id.get_marker_index().namespace() == MarkerIndex::NS_TRAIL;
            extra.get(idx).map(|extra| {
                let pos = match ignore_draw_order {
                    true => Point3::INFINITY,
                    false => extra.position,
                };
                (pos, idx)
            })
        });
        let ordered = render::RenderOrderBuilder {
            bvh_iter,
            cam_origin,
            cam_dir,
            draw_order_heap: &mut self.draw_order_heap,
        };
        let iter = ordered.map(move |idx| {
            let mid = unsafe { shapes.get_unchecked(idx) };
            (idx, &mid.value.id)
        });
        iter
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum ShaderState {
    None,
    Trail,
    Poi,
}
pub struct ArcrenderSettings {
    pub poi_anim_speed: f32,
    pub trail_anim_speed: f32,
    pub trail_can_fade: bool,
    pub trail_distance_fade: bool,
    pub trail_flags: u32,
    pub poi_can_fade: bool,
    pub poi_distance_fade: bool,
    pub poi_limit_size: bool,
    pub poi_flags: u32,
}
impl ArcrenderSettings {
    pub const DEFAULT: Self = Self {
        poi_can_fade: SpaceSettings::DEFAULT_PLAYER_OVERLAP_POI,
        poi_limit_size: SpaceSettings::DEFAULT_POI_LIMIT_SIZE,
        trail_can_fade: SpaceSettings::DEFAULT_PLAYER_OVERLAP_THRESHOLD > 0.0,
        trail_anim_speed: SpaceSettings::DEFAULT_TRAIL_ANIM,
        poi_distance_fade: SpaceSettings::DEFAULT_DISTANCE_FADE_RANGE,
        trail_distance_fade: SpaceSettings::DEFAULT_DISTANCE_FADE_RANGE,
        poi_anim_speed: 1.0,
        trail_flags: 0,
        poi_flags: 0,
    };
}

pub static STATS_ENTITY_INSTANCE_SIZE: Counter = Counter::DEFAULT;
pub static STATS_ENTITY_DRAW: Counter = Counter::DEFAULT;
pub static STATS_ENTITY_COUNT: Counter = Counter::DEFAULT;
pub static STATS_ENTITY_DRAW_MAP: Counter = Counter::DEFAULT;
