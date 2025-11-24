use {
    super::{
        poi::{ActivePoi, PoiCommonRenderData},
        trail::ActiveTrail,
    },
    crate::{
        controller::pathing::registry::{PackIndex, PackPath, PoiIndex, TrailIndex, TrailSectionIndex},
        render::machine::{RenderMachine, RenderPosition},
        space::{
            dx11::{InstanceBufferData, RenderBackend},
            render_list::{MapFrustum, RenderEntity, RenderId, RenderList, RenderListBuilder},
            resources::Texture,
            DrawSpace,
            LocalContext,
            MapContext,
        },
    },
    anyhow::{anyhow, Context},
    bitvec::vec::BitVec,
    glamour::Box3,
    indexmap::IndexMap,
    std::{
        mem,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
    },
    taimi_d3d::dx11::{buffer::BufferOf, prelude::*},
    taimi_pack::loader::PackLoaderContext,
};

pub struct ActivePack {
    pub active_trails: Vec<ActiveTrail>,
    pub active_pois: Vec<ActivePoi>,

    // Internal rendering data.
    texture_list: IndexMap<String, Option<Arc<Texture>>>,
    loaded_textures: BitVec,
    unused_textures: BitVec,
    pub render_list_bookmark: Option<usize>,
    render_poi_bookmark: usize,
    poi_bookmark: usize,
    // TODO: Scripting.
    //_script_engine: (),
}

impl ActivePack {
    pub fn new() -> Self {
        ActivePack {
            active_pois: Default::default(),
            active_trails: Default::default(),
            texture_list: Default::default(),
            loaded_textures: Default::default(),
            unused_textures: Default::default(),
            render_list_bookmark: Default::default(),
            render_poi_bookmark: Default::default(),
            poi_bookmark: Default::default(),
        }
    }

    #[cfg(todo)]
    pub fn update(&mut self, render_list: &mut RenderList) {
        // why are we doing 4 for loops over all trails and pois currently active every frame?
        // ::update(...) is a no-op, filters should NOT be changing every frame and even then
        // should be a matter of when recompute_enabled(); is called :s
        /*self.update_filters();

        for trail_idx in 0..self.active_trails.len() {
            ActiveTrail::update(self, trail_idx);
        }
        for poi_idx in 0..self.active_pois.len() {
            ActivePoi::update(self, poi_idx);
        }*/

        // TODO: Scripting engine update.

        for trail_idx in self.dirty_trails.iter_ones() {
            let trail = &self.active_trails[trail_idx];
            for i_section in 0..trail.section_bounds.len() {
                render_list.update(trail.render_bookmark as usize + i_section);
            }
        }
        for poi_idx in self.dirty_pois.iter_ones() {
            render_list.update(self.poi_bookmark + poi_idx);
        }
    }

    pub fn register_texture(&mut self, asset: &str) -> PackTextureHandle {
        if let Some(id) = self.texture_list.get_index_of(asset) {
            return PackTextureHandle(id);
        }

        self.loaded_textures.push(false);
        self.unused_textures.push(false);
        let idx = self.texture_list.insert_full(asset.to_string(), None).0;
        PackTextureHandle(idx)
    }

    pub fn get_or_load_texture<'t>(
        &'t mut self,
        handle: PackTextureHandle,
        loader: &mut dyn PackLoaderContext,
        device: &Dx11Device,
    ) -> anyhow::Result<&'t Arc<Texture>> {
        let PackTextureHandle(idx) = handle;
        let (asset, slot) = self
            .texture_list
            .get_index_mut(idx)
            .ok_or_else(|| anyhow!("Texture {} not in list at all", idx))?;

        let texture = match slot {
            slot_texture @ None => {
                let data = loader.load_asset_dyn(asset)?;
                let image = image::ImageReader::new(data)
                    .with_guessed_format()
                    .map_err(anyhow::Error::from)
                    .and_then(|image| image.decode().map_err(Into::into))
                    .with_context(|| "decoding {asset}")?
                    .into_rgba8()
                    .into_flat_samples();

                let texture = Texture::load_rgba8_uncached(device, image)
                    .with_context(|| format!("loading {asset}"))?;
                let texture = Arc::new(texture);
                let texture = slot_texture.insert(texture);
                self.loaded_textures.set(idx, true);
                texture
            },
            Some(texture) => texture,
        };
        self.unused_textures.set(idx, false);
        Ok(texture)
    }

    fn prepare_new_map<P, T>(
        &mut self,
        pack_idx: PackIndex,
        pois: P,
        trails: T,
        render_entities: &mut Vec<RenderEntity>,
    ) where
        P: IntoIterator<Item = ActivePoi>,
        T: IntoIterator<Item = ActiveTrail>,
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

        self.cleanup_textures();

        //self.recompute_enabled();
    }

    pub fn clear(&mut self) {
        //self.unused_textures.copy_from_bitslice(&self.loaded_textures);
        self.unused_textures |= &self.loaded_textures;
        self.active_trails.clear();
        self.active_pois.clear();
        self.render_list_bookmark = None;
        self.render_poi_bookmark = 0;
        self.poi_bookmark = 0;
    }

    /// Unload no longer needed textures.
    pub fn cleanup_textures(&mut self) {
        for handle in self.unused_textures.iter_ones() {
            self.texture_list[handle] = None;
            self.loaded_textures.set(handle, false);
        }
        self.unused_textures.fill(false);
    }
}

#[repr(transparent)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PackTextureHandle(usize);

pub struct PackCollection {
    pub loaded_packs: Vec<ActivePack>,

    pub render_list: RenderList,
    pub poi_common: PoiCommonRenderData,
}

impl PackCollection {
    pub fn new(backend: &RenderBackend) -> anyhow::Result<PackCollection> {
        let poi_common = PoiCommonRenderData::new(backend)?;
        Ok(PackCollection {
            loaded_packs: Default::default(),
            render_list: RenderListBuilder::default().build(),
            poi_common,
        })
    }

    pub fn clear(&mut self) {
        self.loaded_packs.clear();

        self.render_list.clear();
        self.poi_common.clear();
    }

    pub fn pack_mut<'a>(&'a mut self, path: &PackPath) -> &'a mut ActivePack {
        let index = path.path as usize;
        if self.loaded_packs.len() <= index {
            self.loaded_packs.resize_with(index + 1, || ActivePack::new());
        }
        &mut self.loaded_packs[index]
    }

    pub fn load_pack<P, T>(&mut self, _device: &Dx11Device, pack_idx: PackIndex, pois: P, trails: T) -> anyhow::Result<()> where
        P: IntoIterator<Item = ActivePoi>,
        T: IntoIterator<Item = ActiveTrail>,
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

    fn build_active_pack<P, T>(
        &mut self,
        pack_idx: PackIndex,
        pois: P, trails: T,
        render_entities: Option<&mut Vec<RenderEntity>>,
    ) -> anyhow::Result<()> where
        P: IntoIterator<Item = ActivePoi>,
        T: IntoIterator<Item = ActiveTrail>,
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

    pub fn rebuild_active(&mut self, _device: &Dx11Device) -> anyhow::Result<()> {
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

    fn recreate_buffers_inner(
        &mut self,
        device: &Dx11Device,
        machine: &RenderMachine,
    ) -> anyhow::Result<()> {
        // identity at start for trail drawing
        let mut data_world = vec![InstanceBufferData::IDENTITY; 1];
        let mut data_map = vec![InstanceBufferData::IDENTITY; 1];

        let mut render_poi_bookmark = 1;
        for pack in &mut self.loaded_packs {
            data_world.extend(pack.active_pois.iter().map(|poi| poi.instance_data()));
            data_map.extend(
                pack.active_pois
                    .iter()
                    .map(|poi| poi.instance_data_map(machine)),
            );
            pack.render_poi_bookmark = render_poi_bookmark;
            render_poi_bookmark += pack.active_pois.len();
        }
        let (data_world, data_map) = (&data_world[..], &data_map[..]);
        super::poi::STATS_POI_INSTANCE_SIZE
            .reset_with(|| (size_of_val(data_map) + size_of_val(data_world)) as _);
        let (poi_ib_world, poi_ib_map) = (
            Some(BufferOf::new_with_data(device, Ok(data_world), ())?),
            Some(BufferOf::new_with_data(device, Ok(data_map), ())?),
        );
        self.poi_common.world_ib = poi_ib_world;
        self.poi_common.map_ib = poi_ib_map;

        Ok(())
    }

    fn recreate_buffers(&mut self, device: &Dx11Device, machine: &RenderMachine) -> anyhow::Result<()> {
        let res = self
            .recreate_buffers_inner(device, machine)
            .context("preparing POI instance buffers");
        if res.is_err() {
            self.mark_buffers_dirty();
        }
        res
    }

    fn mark_buffers_dirty(&mut self) {
        self.poi_common.clear();
        for pack in &mut self.loaded_packs {
            pack.render_poi_bookmark = 0;
        }
    }

    pub fn prepare(&mut self, device: &Dx11Device, machine: &RenderMachine) -> anyhow::Result<()> {
        if
        /* !self.loaded_packs.is_empty() &&*/
        self.poi_common.is_empty() {
            self.recreate_buffers(device, machine)?;
        }
        self.poi_common.update(device);

        Ok(())
    }

    #[cfg(todo)]
    pub fn update(&mut self) {
        for pack in &mut self.loaded_packs {
            pack.update(&mut self.render_list);
        }
    }

    pub fn draw(
        &mut self,
        camera: RenderPosition,
        frustum: &MapFrustum,
        backend: &RenderBackend,
        device_context: &Dx11Context,
    ) {
        let entities = self.render_list.get_entities_for_drawing(camera, frustum);
        Self::draw_entities(
            &self.loaded_packs,
            &self.poi_common,
            device_context,
            backend,
            entities,
        );
        STATS_ENTITY_COUNT.store(self.render_list.entities_count(), Ordering::Relaxed);
    }

    pub fn draw_entities<'e, E>(
        loaded_packs: &[ActivePack],
        poi_common: &PoiCommonRenderData,
        device_context: &Dx11Context,
        backend: &RenderBackend,
        entities: E,
    ) where
        E: IntoIterator<Item = &'e RenderEntity>,
    {
        poi_common.set_primitive(device_context);

        let mut shader_state = ShaderState::None;
        let mut num_drawn = 0usize;
        for entity in entities {
            let render_id = match entity.render_id {
                Some(id) => id,
                None => continue,
            };
            match render_id {
                RenderId::TrailSection { pack_idx, trail_idx, section } => {
                    let trail = loaded_packs.get(pack_idx as usize).and_then(|pack| {
                        pack.active_trails.get(trail_idx as usize)
                    });
                    let trail = match trail {
                        Some(t) => t,
                        None => {
                            log::error!("Render ID refers to missing trail#{trail_idx} pack#{pack_idx} section#{section}");
                            continue
                        },
                    };
                    if !trail.visibility.is_visible_for_space() {
                        continue
                    }
                    if shader_state != ShaderState::Trail {
                        shader_state = ShaderState::Trail;
                        backend.shaders.set_named(device_context, "trail");
                    }
                    trail.bind_texture(device_context, poi_common, LocalContext::World);
                    trail.draw_section(device_context, section, LocalContext::World);
                },
                RenderId::Poi { pack_idx, poi_idx } => {
                    let poi = loaded_packs.get(pack_idx as usize).and_then(|pack|
                        pack.active_pois.get(poi_idx as usize).map(|poi| (pack, poi))
                    );
                    let (pack, poi) = match poi {
                        Some((pack, _)) if pack.render_poi_bookmark == 0 => continue,
                        Some(t) => t,
                        None => {
                            log::error!("Render ID refers to missing PoI#{poi_idx} pack#{pack_idx}");
                            continue
                        },
                    };
                    if !poi.visibility.is_visible_for_space() {
                        continue
                    }
                    if shader_state != ShaderState::Poi {
                        shader_state = ShaderState::Poi;
                        poi_common.set(device_context);
                    }
                    poi.bind_texture(device_context, poi_common, LocalContext::World);
                    poi.draw(
                        device_context,
                        pack.render_poi_bookmark + poi_idx as usize,
                        LocalContext::World,
                    );
                },
            }
            num_drawn += 1;
        }
        STATS_ENTITY_DRAW.store(num_drawn, Ordering::Relaxed);
    }

    #[cfg(feature = "goggles")]
    pub fn entities_obscured<'a>(
        &'a self,
        frustum: &'a MapFrustum,
    ) -> impl Iterator<Item = &'a RenderEntity> + 'a {
        self.render_list.visible_entities(frustum)
    }

    pub fn draw_map_entities<'e, E>(
        loaded_packs: &[ActivePack],
        poi_common: &PoiCommonRenderData,
        device_context: &Dx11Context,
        backend: &RenderBackend,
        map: MapContext,
        entities: E,
    ) where
        E: IntoIterator<Item = &'e RenderEntity>,
    {
        let mut shader_state = ShaderState::None;
        let mut num_drawn = 0usize;
        let ctx = LocalContext::/*Map(map)*/MAP;
        for entity in entities {
            let render_id = match entity.render_id {
                Some(id) => id,
                None => continue,
            };
            match render_id {
                RenderId::TrailSection { pack_idx, trail_idx, section } => {
                    let trail = loaded_packs.get(pack_idx as usize).and_then(|pack| {
                        pack.active_trails.get(trail_idx as usize)
                    });
                    let trail = match trail {
                        Some(t) => t,
                        None => {
                            log::error!("Render ID refers to missing trail#{trail_idx} pack#{pack_idx} section#{section}");
                            continue
                        },
                    };
                    if !trail.visibility.is_visible_for_map(map) {
                        continue
                    }
                    if shader_state == ShaderState::None {
                        backend.shaders.set_named(device_context, "map");
                        poi_common.set_primitive(device_context);
                        poi_common.set_instance(device_context, ctx);
                    }
                    if shader_state != ShaderState::Trail {}
                    shader_state = ShaderState::Trail;
                    trail.bind_texture(device_context, poi_common, ctx);
                    trail.draw_section(device_context, section, ctx);
                },
                RenderId::Poi { pack_idx, poi_idx } => {
                    let poi = loaded_packs.get(pack_idx as usize).and_then(|pack|
                        pack.active_pois.get(poi_idx as usize).map(|poi| (pack, poi))
                    );
                    let (pack, poi) = match poi {
                        Some((pack, _)) if pack.render_poi_bookmark == 0 => continue,
                        Some(t) => t,
                        None => {
                            log::error!("Render ID refers to missing PoI#{poi_idx} pack#{pack_idx}");
                            continue
                        },
                    };
                    if !poi.visibility.is_visible_for_map(map) {
                        continue
                    }
                    // TODO: handle ScaleOnMapWithZoom and related attrs
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
                    poi.draw(device_context, pack.render_poi_bookmark + poi_idx as usize, ctx);
                },
            }
            num_drawn += 1;
        }
        STATS_ENTITY_DRAW_MAP.store(num_drawn, Ordering::Relaxed);
    }

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

    pub fn unload_map(&mut self, _device_context: &Dx11Context, _map_id: u32) -> anyhow::Result<()> {
        self.clear_active();

        Ok(())
    }

    #[cfg(todo)]
    pub fn load_map(
        &mut self,
        device: &Dx11Device,
        _device_context: &Dx11Context,
        map_id: u32,
    ) -> anyhow::Result<()> {
        self.prepare_new_map(map_id as i32, device)
    }

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

    pub fn clear_active(&mut self) {
        self.render_list.clear();
        for pack in &mut self.loaded_packs {
            pack.clear();
        }

        self.poi_common.clear();
    }

    pub fn cleanup_textures(&mut self) {
        for pack in &mut self.loaded_packs {
            pack.cleanup_textures();
        }
    }

    /// See [crate::space::engine::Engine::cleanup_background]
    ///
    /// TODO: revisit, avoid, etc
    pub fn cleanup_background(self) {
        let Self { loaded_packs, poi_common, .. } = self;
        mem::forget((loaded_packs, poi_common));
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum ShaderState {
    None,
    Trail,
    Poi,
}

pub static STATS_ENTITY_DRAW: AtomicUsize = AtomicUsize::new(0);
pub static STATS_ENTITY_COUNT: AtomicUsize = AtomicUsize::new(0);
pub static STATS_ENTITY_DRAW_MAP: AtomicUsize = AtomicUsize::new(0);
