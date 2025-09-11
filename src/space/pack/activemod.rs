use {
    anyhow::anyhow,
    crate::{
        Controller,
        ControllerEvent,
    },
    nexus::{
        imgui::{ComboBox, Ui, Condition, TreeNode},
        alert::send_alert,
    },
    crate::render::pathing_window::{PathingFilterState, PathingSearchState},
    crate::space::{
        pack::Pack,
        dx11::{PerspectiveHandler, PerspectiveInputData, RenderBackend},
        render_list::{MapFrustum, RenderEntity, RenderId, RenderList, RenderListBuilder},
        resources::Texture,
    },
    crate::marker::atomic::MapSpace,
    anyhow::Context,
    bitvec::vec::BitVec,
    glam::Vec3,
    glamour::{Point3, Vector3},
    indexmap::{map::Entry, IndexMap, IndexSet},
    super::{
        category::Category,
        loader::{DirectoryLoader, PackLoaderContext, ZipLoader},
        poi::{Poi, ActivePoi, PoiCommonRenderData},
        trail::ActiveTrail,
    },
    std::{
        collections::{HashMap, HashSet},
        fs::{create_dir_all, read_dir},
        io::{Cursor, Read as _},
        path::Path,
        sync::Arc,
    },
    uuid::Uuid,
    windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11DeviceContext},
    xml::{common::Position, reader::XmlEvent},
};

#[derive(Debug)]
pub enum UnloadedReason {
    Disabled,
    UnknownFormat,
    LoadingFailed(String),
}



#[derive(Default)]
pub struct ActivePack {
    pub pack: Pack,

    // Actively loaded data.
    pub enabled_categories: BitVec,
    pub user_category_state: BitVec,
    pub active_trails: IndexMap<Uuid, ActiveTrail>,
    pub active_pois: IndexMap<Uuid, ActivePoi>,

    // Internal rendering data.
    loader: Option<Box<dyn PackLoaderContext + Send>>,
    texture_list: HashMap<String, PackTexture>,
    textures: Vec<PackTexture>,
    loaded_textures: BitVec,
    unused_textures: BitVec,
    dirty_trails: BitVec,
    dirty_pois: BitVec,
    render_list_bookmark: usize,
    poi_bookmark: usize,

    // TODO: Scripting.
    _script_engine: (),
}

impl ActivePack {
    pub fn load(mut loader: impl PackLoaderContext + Send + 'static) -> anyhow::Result<ActivePack> {
        let pack = Pack::load(loader)?;

        let mut active_pack = ActivePack::default();

        active_pack.pack = pack;

        active_pack.enabled_categories
            .reserve(active_pack.pack.categories.all_categories.len());

        for category in active_pack.pack.categories.all_categories.values() {
            active_pack.enabled_categories.push(category.default_toggle);
        }

        active_pack.user_category_state = active_pack.enabled_categories.clone();

        Ok(active_pack)
    }

    pub fn get_copyable_pois(&self) -> Vec<Poi> {
        let mut current_pois = Vec::new();
        for (_, poi) in &self.active_pois {
            if !poi.filtered {
                let actual_poi = &self.pack.pois[poi.poi_idx];
                if actual_poi.attributes.copy_value.is_some() {
                    let actual_poi = actual_poi.clone();
                    current_pois.push(actual_poi);
                }
            }
        }
        current_pois
    }
    pub fn draw_categories(&mut self, ui: &Ui, filter_state: PathingFilterState, open_items: &mut HashSet<String>, is_root: bool, recompute: &mut bool, search_state: &PathingSearchState) {
        let root = &mut self.pack.categories.root_categories;
        let all_categories = &self.pack.categories.all_categories;
        let enabled_categories = &mut self.user_category_state;
        for cat_name in root.iter() {
            Self::draw_category(ui,
                &all_categories[cat_name],
                all_categories,
                enabled_categories,
                filter_state,
               open_items,
                true,
                recompute,
                search_state
            );
        }
    }

    pub fn draw_category(ui: &Ui, category: &Category, all_categories: &IndexMap<String, Category>, state: &mut BitVec, filter_state: PathingFilterState, open_items: &mut HashSet<String>, is_root: bool, recompute: &mut bool, search_state: &PathingSearchState) {
        let push_token = ui.push_id(&category.full_id);
        if category.is_hidden {
            push_token.pop();
            return;
        }
        let mut display = true;
        if let Some(idx) = all_categories.get_index_of(&category.full_id) {
            if let Some(substate) = state.get(idx) {
                let enabled_filter = *substate && filter_state.contains(PathingFilterState::Enabled);
                let disabled_filter = !*substate && filter_state.contains(PathingFilterState::Disabled);
                let is_root_filter = is_root && filter_state.contains(PathingFilterState::IgnoreRoot);
                let is_leaf = category.sub_categories.is_empty();
                let is_branch = !is_leaf;
                let is_leaf_filter = is_leaf && filter_state.contains(PathingFilterState::IgnoreLeaves);
                let is_branch_filter = is_branch && filter_state.contains(PathingFilterState::IgnoreBranches);
                let search_filter = if !search_state.buffer.is_empty() {
                    search_state.search_candidates.contains(&category.full_id)
                } else {
                    true
                };
                display = search_filter && (enabled_filter || disabled_filter || is_root_filter || is_leaf_filter || is_branch_filter);
            }
        }
        if display {
            if category.marker_attributes.copy_value.is_some() {
                ui.indent();
                if let Some(copy_value) = &category.marker_attributes.copy_value {
                    if ui.small_button(&category.display_name) {
                        ui.set_clipboard_text(copy_value);
                        if let Some(copy_message) = &category.marker_attributes.copy_message {
                            send_alert(copy_message);
                        }
                    }
                }
                ui.unindent();
                ui.table_next_column();
                ui.table_next_column();
            } else {
                let mut unbuilt = TreeNode::new(&category.display_name)
                    .frame_padding(true)
                    .tree_push_on_open(false)
                    .opened(open_items.contains(&category.full_id), Condition::Always);
                if category.is_separator {
                    unbuilt = unbuilt.leaf(true);
                } else if category.sub_categories.is_empty() {
                    unbuilt = unbuilt.bullet(true);
                } else {
                    unbuilt = unbuilt.framed(true);
                }
                let tree_token = unbuilt.push(ui);
                ui.table_next_column();
                if !category.is_separator {
                    if let Some(idx) = all_categories.get_index_of(&category.full_id) {
                        if let Some(mut substate) = state.get_mut(idx) {
                            if ui.checkbox("", &mut substate) {
                                *recompute = true;
                                Controller::try_send(ControllerEvent::PathingStateUpdate(category.full_id.clone(), *substate));
                            };
                        }
                    }
                }
                let mut internal_closure = || {
                    if !open_items.contains(&category.full_id) {
                        open_items.insert(category.full_id.clone());
                    }
                    if !category.sub_categories.is_empty() {
                        ui.indent(); //_by(1.0);
                    }
                    for (_local, global) in category.sub_categories.iter() {
                        Self::draw_category(ui,
                    &all_categories[global],
                            all_categories,
                            state,
                            filter_state,
                            open_items,
                            false,
                            recompute,
                            search_state
                        );
                    }
                    if !category.sub_categories.is_empty() {
                        ui.unindent(); //_by(1.0);
                    }
                };
                ui.table_next_column();
                if let Some(token) = tree_token {
                    internal_closure();
                    token.pop();
                } else {
                    if open_items.contains(&category.full_id) {
                        open_items.remove(&category.full_id);
                    }
                }
            }
        }
        push_token.pop();
    }

    pub fn disable_paths(&mut self, paths: &HashSet<String>) {
        for path in paths {
            if let Some(idx) = self.pack.categories.all_categories.get_index_of(path) {
                if let Some(mut state) = self.user_category_state.get_mut(idx) {
                    *state = false;
                }
            }
        }
        self.recompute_enabled();
    }

    pub fn recompute_enabled(&mut self) {
        let all = &mut self.pack.categories.all_categories;
        for root_category_id in &self.pack.categories.root_categories {
            if let Some(root) = all.get(root_category_id) {
                root.recompute_enabled(all, &mut self.enabled_categories, &self.user_category_state, true);
            }
        }
        // in response to update(...), moving update_filters down here where it should actually be
        // effective to save on useless loops
        self.update_filters();

    }

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
                render_list.update(trail.render_bookmark + i_section);
            }
        }
        for poi_idx in self.dirty_pois.iter_ones() {
            render_list.update(self.poi_bookmark + poi_idx);
        }
    }

    pub fn register_texture(&mut self, asset: &str) -> PackTexture {
        if let Some(id) = self.texture_list.get(asset) {
            return id.clone();
        }

        let pt = PackTexture {
            asset: asset.to_string(),
            texture: None,
        };
        self.textures.push(pt.clone());
        self.loaded_textures.push(false);
        self.unused_textures.push(false);
        self.texture_list.insert(asset.to_string(), pt.clone());
        pt
    }

    pub fn get_or_load_texture(
        &mut self,
        handle: String,
        device: &ID3D11Device,
    ) -> anyhow::Result<Arc<Texture>> {
        let Some(loader) = &mut self.loader else {
            anyhow::bail!("Inconsistent internal state.");
        };
        let slot = &mut self.texture_list.get(&handle)
            .ok_or_else(|| { anyhow!("Texture {} not in list at all", handle) })?;
        let texture = match (&slot.asset, &mut slot.texture) {
            (asset, slot_texture @ None) => {
                let data = loader.load_asset_dyn(asset)?;
                let image = image::ImageReader::new(data)
                    .with_guessed_format()?
                    .decode()?
                    .into_rgba8()
                    .into_flat_samples();

                let texture = Arc::new(Texture::load_rgba8_uncached(device, image)?);
                *slot_texture = Some(texture.clone());
                self.loaded_textures.set(handle, true);
                texture
            }
            (_, Some(texture)) => texture.clone(),
        };
        self.unused_textures.set(handle, false);
        Ok(texture)
    }

    fn prepare_new_map(
        &mut self,
        pack_idx: usize,
        map_id: i32,
        device: &ID3D11Device,
        render_entities: &mut Vec<RenderEntity>,
    ) -> anyhow::Result<()> {
        self.unused_textures
            .copy_from_bitslice(&self.loaded_textures);
        self.active_trails.clear();
        self.active_pois.clear();
        self.dirty_trails.clear();
        self.dirty_pois.clear();
        self.render_list_bookmark = render_entities.len();

        for i_trail in 0..self.pack.trails.len() {
            if self.pack.trails[i_trail].data.map_id != map_id {
                continue;
            }
            let mut id = self.pack.trails[i_trail].guid;
            if self.active_trails.contains_key(&id) {
                log::warn!(
                    "Pack {} contains a duplicate trail GUID `{id}`. \
                    Randomizing to ensure it may still be rendered.",
                    self.pack.name
                );
                while self.active_trails.contains_key(&id) {
                    id = Uuid::new_v4();
                }
            }

            let trail = match ActiveTrail::build(self, i_trail, render_entities.len(), device) {
                Ok(trail) => trail,
                Err(e) => {
                    log::warn!("Error loading trail: {e:?}");
                    continue;
                }
            };

            let trail_idx = self.active_trails.len();
            for i_section in 0..trail.section_bounds.len() {
                let entity = RenderEntity {
                    bounds: trail.section_bounds[i_section],
                    position: trail.section_bounds[i_section].center(),
                    draw_ordered: false,
                    render_id: RenderId::TrailSection {
                        pack_idx,
                        trail_idx,
                        section: i_section,
                    },
                };
                render_entities.push(entity);
            }

            self.active_trails.insert(id, trail);
            self.dirty_trails.push(false);

        }

        self.poi_bookmark = render_entities.len();

        for i_poi in 0..self.pack.pois.len() {
            if self.pack.pois[i_poi].map_id != map_id {
                continue;
            }
            let mut id = self.pack.pois[i_poi].guid;
            if self.active_trails.contains_key(&id) {
                log::warn!(
                    "Pack {} contains a duplicate poi GUID `{id}`. \
                    Randomizing to ensure it may still be rendered.",
                    self.pack.name
                );
                while self.active_trails.contains_key(&id) {
                    id = Uuid::new_v4();
                }
            }

            let poi = match ActivePoi::build(self, i_poi, device) {
                Ok(poi) => poi,
                Err(e) => {
                    log::warn!("Error loading poi: {e:?}");
                    continue;
                }
            };

            let poi_idx = self.active_pois.len();
            let entity = RenderEntity {
                bounds: poi.bounds,
                position: poi.position,
                draw_ordered: true,
                render_id: RenderId::Poi { pack_idx, poi_idx },
            };
            render_entities.push(entity);
            self.active_pois.insert(id, poi);
            self.dirty_pois.push(false);
        }

        log::info!(
            "Loaded {} trails and {} POIs",
            self.active_trails.len(),
            self.active_pois.len()
        );

        // Unload no longer needed textures.
        for handle in self.unused_textures.iter_ones() {
            self.textures[handle].texture = None;
            self.loaded_textures.set(handle, false);
        }

        self.recompute_enabled();

        Ok(())
    }

    fn update_filters(&mut self) {
        for (_, trail) in &mut self.active_trails {
            trail.filtered = !self.enabled_categories[trail.category_idx];
        }
        for (_, poi) in &mut self.active_pois {
            poi.filtered = !self.enabled_categories[poi.category_idx];
        }
    }

}

#[repr(transparent)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PackTextureHandle(usize);

#[derive(Clone, Debug)]
struct PackTexture {
    asset: String,
    texture: Option<Arc<Texture>>,
}



pub struct PackCollection {
    pub loaded_packs: IndexMap<String, ActivePack>,
    pub unloaded_packs: IndexMap<String, UnloadedReason>,

    current_map: Option<i32>,
    render_list: RenderList,
    poi_common: PoiCommonRenderData,
}

impl PackCollection {
    pub fn new(backend: &RenderBackend) -> anyhow::Result<PackCollection> {
        let poi_common = PoiCommonRenderData::new(backend)?;
        Ok(PackCollection {
            loaded_packs: IndexMap::new(),
            unloaded_packs: IndexMap::new(),
            current_map: None,
            render_list: RenderListBuilder::default().build(),
            poi_common,
        })
    }

    pub fn disable_paths(&mut self, disabled_paths: HashSet<String>) {
        for (_pn, pack) in &mut self.loaded_packs {
            pack.disable_paths(&disabled_paths);
        }
    }

    pub fn clear(&mut self) {
        self.loaded_packs.clear();
        self.unloaded_packs.clear();
    }

    pub fn load_all(&mut self, base_dir: &Path) -> anyhow::Result<()> {
        if !base_dir.exists() {
            create_dir_all(base_dir)?;
        }
        self.clear();
        for entry in read_dir(base_dir)? {
            let entry = entry?;
            self.load(&entry.file_name().to_string_lossy(), &entry.path());
        }
        Ok(())
    }

    pub fn load(&mut self, name: &str, path: &Path) {
        let result = if path.is_dir() {
            let loader = DirectoryLoader::new(path);
            ActivePack::load(loader)
        } else {
            match path.extension().map(|e| e.as_encoded_bytes()) {
                Some(e) if e.eq_ignore_ascii_case(b"taco") => {
                    ZipLoader::new(path).and_then(ActivePack::load)
                }
                _ => {
                    self.unloaded_packs
                        .insert(name.into(), UnloadedReason::UnknownFormat);
                    return;
                }
            }
        };
        let pack = match result {
            Ok(pack) => pack,
            Err(e) => {
                self.unloaded_packs
                    .insert(name.into(), UnloadedReason::LoadingFailed(format!("{e:?}")));
                return;
            }
        };
        self.loaded_packs.insert(name.into(), pack);
    }

    pub fn prepare_new_map(&mut self, map_id: i32, device: &ID3D11Device) -> anyhow::Result<()> {
        if self.current_map == Some(map_id) {
            return Ok(());
        }
        self.current_map = Some(map_id);
        let mut render_builder = self.render_list.rebuild();

        for (pack_idx, pack) in self.loaded_packs.values_mut().enumerate() {
            pack.prepare_new_map(pack_idx, map_id, device, &mut render_builder.entities)?;
        }

        self.render_list = render_builder.build();
        Ok(())
    }

    pub fn update(&mut self) {
        for (_, pack) in &mut self.loaded_packs {
            pack.update(&mut self.render_list);
        }
    }

    pub fn draw(
        &mut self,
        cam_data: &PerspectiveInputData,
        backend: &RenderBackend,
        device_context: &ID3D11DeviceContext,
    ) {
        let cam_origin: Point3<MapSpace> = cam_data.pos.into();
        let cam_dir: Vector3<MapSpace> = cam_data.front.into();
        let frustum = MapFrustum::from_camera_data(
            cam_data,
            backend.perspective_handler.aspect_ratio(),
            backend.perspective_handler.near(),
            backend.perspective_handler.far(),
        );
        self.poi_common
            .camera_update(cam_data.front.normalize(), Vec3::Y);
        #[derive(Copy, Clone, PartialEq, Eq)]
        enum ShaderState {
            None,
            Trail,
            Poi,
        }
        let mut shader_state = ShaderState::None;
        let mut num_drawn = 0;
        for entity in self
            .render_list
            .get_entities_for_drawing(cam_origin, cam_dir, &frustum)
        {
            num_drawn += 1;
            match entity.render_id {
                RenderId::TrailSection {
                    pack_idx,
                    trail_idx,
                    section,
                } => {
                    if shader_state != ShaderState::Trail {
                        shader_state = ShaderState::Trail;
                        backend.shaders.0["trail"].set(device_context);
                        backend.shaders.1["trail"].set(device_context);
                    }
                    self.loaded_packs[pack_idx].active_trails[trail_idx]
                        .draw_section(device_context, section);
                }
                RenderId::Poi { pack_idx, poi_idx } => {
                    if shader_state != ShaderState::Poi {
                        shader_state = ShaderState::Poi;
                        self.poi_common.shaders.set(device_context);
                    }
                    self.loaded_packs[pack_idx].active_pois[poi_idx]
                        .draw(device_context, &mut self.poi_common);
                }
            }
        }
    }
}
