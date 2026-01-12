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
            },
            space::{SpacePackCollection, SpacePackShared, TextureLoadRequests, TrailGeometryRequests},
        },
        exports::runtime as rt,
        render::machine::{RenderMachine, RenderPosition},
        space::{
            dx11::RenderBackend,
            pack::{PoiRender, TrailRender},
            DrawSpace,
        },
    },
    anyhow::Context,
    bvh::aabb,
    glamour::{Box3, Point3},
    rustc_hash::FxHashSet,
    std::{collections::BTreeSet, ops, sync::Arc},
    taimi_d3d::dx11::prelude::*,
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
};

/// Internal rendering data.
pub struct PackRenderData {
    pub info: Arc<SharedPackInfo>,
    pub map_info: Option<SharedMapPackLoaded>,
    pub map_state: SharedMapPackState,
    pub pois: IndexedList<LoadedPoiNs, LoadedPoiIndex, Vec<PoiRender>>,
    pub trails: IndexedList<LoadedTrailNs, LoadedTrailIndex, Vec<TrailRender>>,
    render_poi_bookmark: usize,
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
    pub fn prepare(&mut self, device: &Dx11Device, machine: &RenderMachine) -> anyhow::Result<bool> {
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
            ArcPtrCmp::from_mut(&mut self.render_list.spacepacks).clone_from_arc(&*spacepacks);
            // TODO: actual dirty check bleh
            space_dirty = true;
        }
        if space_dirty {
            self.mark_buffers_dirty();
        }
        let mut ibs_dirty = self.poi_common.is_empty();
        let map_id = match self.render_list.spacepacks.map_id {
            map_id if map_id != machine.is_ingame() => None,
            map_id => map_id,
        };
        let prev_map_id = self.draw_state.prev_map_id;
        if map_id.is_some() && prev_map_id != map_id {
            self.clear();
            self.draw_state.prev_map_id = map_id;
        }
        let packs_map_changed = {
            let packs_changed = match self.packs_map.as_mut() {
                Some(packs_map) if space_dirty || packs_map.has_changed().unwrap_or(false) =>
                    Some(packs_map.borrow_and_update()),
                _ => None,
            };
            match packs_changed {
                Some(packs_map) if packs_map.map_id != map_id => None,
                packs_map => packs_map,
            }
        };
        if prev_map_id.is_none() && space_dirty && self.spacepacks.render_entities.entities.len() > 0 {
            let (pois, trails) = self.spacepacks.loaded_packs.values().fold(
                (0usize, 0usize),
                |(mut pois, mut trails), p| {
                    pois += p.populated_pois.count_ones();
                    trails += p.populated_trails.count_ones();
                    (pois, trails)
                },
            );
            log::info!("Loaded {trails} trails and {pois} POIs");
            // TODO: this will not trigger properly as items slowly load in...
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
                            pack.pois.resize_with(poi_len, PoiRender::empty);
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
                use crate::exports::runtime::textures::TextureSlot;
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

        Ok(map_id.is_some())
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
        device_context: &Dx11Context,
    ) {
        let Some(spacepacks) = self.spacepacks.cached.as_ref() else { return };
        let entities =
            self.render_list
                .iter_markers_visible(self.pack_data.map_ref_as_slice(), frustum, camera);
        Self::draw_entities(
            &mut self.draw_state,
            &self.poi_common,
            device_context,
            backend,
            entities,
        );
        STATS_ENTITY_COUNT.reset_with(|| spacepacks.render_entities.entities.len() as _);
    }
    #[cfg(feature = "goggles")]
    pub fn draw_obscured(
        &mut self,
        camera: RenderPosition,
        frustum: &MapFrustum,
        backend: &RenderBackend,
        device_context: &Dx11Context,
    ) {
        let entities =
            self.render_list
                .iter_markers_visible(self.pack_data.map_ref_as_slice(), frustum, camera);
        Self::draw_entities(
            &mut self.draw_state,
            &self.poi_common,
            device_context,
            backend,
            entities,
        );
    }

    pub fn draw_entities<'e, E>(
        draw_state: &mut PackRenderState,
        poi_common: &PoiCommonRenderData,
        device_context: &Dx11Context,
        backend: &RenderBackend,
        entities: E,
    ) where
        E: IntoIterator<Item = (&'e PackRenderData, &'e MarkerId)>,
    {
        poi_common.set_primitive(device_context);

        let mut shader_state = ShaderState::None;
        let mut num_drawn = 0usize;
        for (pack_data, marker_id) in entities {
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
                    if shader_state != ShaderState::Trail {
                        shader_state = ShaderState::Trail;
                        backend.shaders.set_named(device_context, "trail");
                    }
                    trail.bind_texture(device_context, poi_common, LocalContext::World);
                    trail.draw_section(device_context, path.path, LocalContext::World);
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
                    if !lpoi.visibility.is_visible_for_space() {
                        continue
                    }
                    if poi.report_incomplete(&marker_id, draw_state) {
                        continue
                    }
                    if shader_state != ShaderState::Poi {
                        shader_state = ShaderState::Poi;
                        poi_common.set(device_context);
                    }
                    poi.bind_texture(device_context, poi_common, LocalContext::World);
                    poi.draw(
                        device_context,
                        pack_data.render_poi_bookmark + path.path as usize,
                        LocalContext::World,
                    );
                },
                _ => {
                    log::error!("Render ID {render_id} refers to invalid marker {marker_id}");
                },
            }
            num_drawn += 1;
        }
        STATS_ENTITY_DRAW.reset(num_drawn as _);
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
        STATS_ENTITY_DRAW_MAP.reset(num_drawn as _);
    }

    pub fn clear(&mut self) {
        self.clear_packs();
        self.draw_state.clear();
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
    }
    /// See [crate::space::engine::Engine::cleanup_background]
    ///
    /// TODO: revisit, avoid, etc
    pub fn cleanup_background(self) {
        let Self { pack_data, poi_common, .. } = self;
        poi_common.cleanup_background();
        for pack in pack_data.data.into_iter() {
            pack.cleanup_background();
        }
    }

    pub fn cleanup_textures(&mut self) {
        let todo = ();
    }
}

#[derive(Debug, Default)]
pub struct PackRenderState {
    pub drawn_incomplete: FxHashSet<MarkerId>,
    pub prev_map_id: Option<MapIndex>,
}
impl PackRenderState {
    pub fn clear(&mut self) {
        self.drawn_incomplete = Default::default();
    }
    pub fn clear_active(&mut self) {
        self.drawn_incomplete.clear();
    }
}

#[derive(Default)]
pub struct PackRenderList {
    spacepacks: Arc<SpacePackCollection>,
    draw_order_heap: render::RenderOrderHeap<usize>,
}
impl PackRenderList {
    /// adding some wiggle room around the map edges...
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
        self.spacepacks.bvh_traverse(query).filter_map(move |(_idx, id)| {
            let pack_path = id.get_marker_pack_path();
            pack_data.lookup_ref(&pack_path).map(|p| (p, id))
        })
    }
    pub fn iter_markers_all<'a, 'e>(
        &'a self,
        pack_data: &'e IndexedList<PackRegistryNs, PackIndex, [PackRenderData]>,
    ) -> impl Iterator<Item = (&'e PackRenderData, &'a MarkerId)> {
        let shapes = &self.spacepacks.render_entities.entities[..];
        shapes.iter().filter_map(move |shape| {
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
    ) -> impl Iterator<Item = (&'e PackRenderData, &'a MarkerId)> {
        self.iter_entities_visible(query, camera)
            .filter_map(|(_idx, id)| {
                let pack_path = id.get_marker_pack_path();
                pack_data.lookup_ref(&pack_path).map(|p| (p, id))
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
        self.draw_order_heap.reserve(shapes.len() / 8);

        let bvh_iter = self.spacepacks.bvh_traverse(query).filter_map(move |(idx, _id)| {
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

pub static STATS_ENTITY_DRAW: Counter = Counter::DEFAULT;
pub static STATS_ENTITY_COUNT: Counter = Counter::DEFAULT;
pub static STATS_ENTITY_DRAW_MAP: Counter = Counter::DEFAULT;
