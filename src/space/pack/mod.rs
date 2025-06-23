use {
    super::{
        dx11::{PerspectiveHandler, PerspectiveInputData, RenderBackend},
        render_list::{MapFrustum, RenderEntity, RenderId, RenderList, RenderListBuilder},
        resources::Texture,
    },
    crate::marker::atomic::MapSpace,
    anyhow::Context,
    bitvec::vec::BitVec,
    category::Category,
    glam::Vec3,
    glamour::{Point3, Vector3},
    indexmap::{map::Entry, IndexMap, IndexSet},
    loader::{DirectoryLoader, PackLoaderContext, ZipLoader},
    poi::{ActivePoi, PoiCommonRenderData},
    std::{
        collections::{HashMap, HashSet},
        fs::read_dir,
        io::{Cursor, Read as _},
        path::Path,
        sync::Arc,
    },
    trail::ActiveTrail,
    uuid::Uuid,
    windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11DeviceContext},
    xml::{common::Position, reader::XmlEvent},
};

pub mod attributes;
pub mod category;
pub mod loader;
pub mod poi;
pub mod trail;

pub struct PackCollection {
    pub loaded_packs: IndexMap<String, Pack>,
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
            Pack::load(loader)
        } else {
            match path.extension().map(|e| e.as_encoded_bytes()) {
                Some(e) if e.eq_ignore_ascii_case(b"taco") => {
                    ZipLoader::new(path).and_then(Pack::load)
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

#[derive(Debug)]
pub enum UnloadedReason {
    Disabled,
    UnknownFormat,
    LoadingFailed(String),
}

#[derive(Default)]
pub struct Pack {
    pub name: String,

    // Descriptive data.
    pub pois: Vec<poi::Poi>,
    pub trails: Vec<trail::Trail>,
    pub categories: CategoryCollection,

    // Actively loaded data.
    pub enabled_categories: BitVec,
    pub user_category_state: BitVec,
    pub active_trails: IndexMap<Uuid, ActiveTrail>,
    pub active_pois: IndexMap<Uuid, ActivePoi>,

    // Internal rendering data.
    loader: Option<Box<dyn PackLoaderContext>>,
    texture_list: HashMap<String, PackTextureHandle>,
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

impl Pack {
    pub fn load(mut loader: impl PackLoaderContext + 'static) -> anyhow::Result<Pack> {
        let mut pack = Pack::default();

        let pack_defs = loader.all_files_with_ext("xml")?;
        for def in pack_defs {
            parse_pack_def(&mut pack, &mut loader, &def)?;
        }

        merge_category_attributes(&mut pack);
        apply_marker_attributes(&mut pack);

        pack.enabled_categories
            .reserve(pack.categories.all_categories.len());
        for category in pack.categories.all_categories.values() {
            pack.enabled_categories.push(category.default_toggle);
        }
        pack.user_category_state = pack.enabled_categories.clone();

        pack.loader = Some(Box::new(loader));

        Ok(pack)
    }

    pub fn get_copyable_pois(&self) -> Vec<poi::Poi> {
        let mut current_pois = Vec::new();
        for (_, poi) in &self.active_pois {
            if !poi.filtered {
                let actual_poi = &self.pois[poi.poi_idx];
                if actual_poi.attributes.copy_value.is_some() {
                    let actual_poi = actual_poi.clone();
                    current_pois.push(actual_poi);
                }
            }
        }
        current_pois
    }

    pub fn disable_paths(&mut self, paths: &HashSet<String>) {
        for path in paths {
            if let Some(idx) = self.categories.all_categories.get_index_of(path) {
                if let Some(mut state) = self.user_category_state.get_mut(idx) {
                    *state = false;
                }
            }
        }
        self.recompute_enabled();
    }

    pub fn recompute_enabled(&mut self) {
        let all = &mut self.categories.all_categories;
        for root_category_id in &self.categories.root_categories {
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

    fn register_texture(&mut self, asset: &str) -> PackTextureHandle {
        if let Some(&id) = self.texture_list.get(asset) {
            return id;
        }

        let id = PackTextureHandle(self.textures.len());
        self.textures.push(PackTexture {
            asset: asset.to_string(),
            texture: None,
        });
        self.loaded_textures.push(false);
        self.unused_textures.push(false);
        self.texture_list.insert(asset.to_string(), id);
        id
    }

    fn get_or_load_texture(
        &mut self,
        handle: PackTextureHandle,
        device: &ID3D11Device,
    ) -> anyhow::Result<Arc<Texture>> {
        let Some(loader) = &mut self.loader else {
            anyhow::bail!("Inconsistent internal state.");
        };
        let slot = &mut self.textures[handle.0];
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
                self.loaded_textures.set(handle.0, true);
                texture
            }
            (_, Some(texture)) => texture.clone(),
        };
        self.unused_textures.set(handle.0, false);
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

        for i_trail in 0..self.trails.len() {
            if self.trails[i_trail].data.map_id != map_id {
                continue;
            }
            let mut id = self.trails[i_trail].guid;
            if self.active_trails.contains_key(&id) {
                log::warn!(
                    "Pack {} contains a duplicate trail GUID `{id}`. \
                    Randomizing to ensure it may still be rendered.",
                    self.name
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

        for i_poi in 0..self.pois.len() {
            if self.pois[i_poi].map_id != map_id {
                continue;
            }
            let mut id = self.pois[i_poi].guid;
            if self.active_trails.contains_key(&id) {
                log::warn!(
                    "Pack {} contains a duplicate poi GUID `{id}`. \
                    Randomizing to ensure it may still be rendered.",
                    self.name
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

struct PackTexture {
    asset: String,
    texture: Option<Arc<Texture>>,
}

#[derive(Default)]
pub struct CategoryCollection {
    /// Map full_id -> Category
    pub all_categories: IndexMap<String, Category>,
    /// List of root categories.
    pub root_categories: IndexSet<String>,
}

fn taco_safe_name(value: &str, is_full: bool) -> String {
    let mut result = String::with_capacity(value.len());
    for c in value.chars() {
        if c.is_ascii_alphanumeric() || (is_full && c == '.') {
            result.push(c);
        } else {
            result.push('_');
        }
    }
    result
}

/// I hate this. See: https://github.com/blish-hud/Pathing/blob/main/Utility/AttributeParsingUtil.cs#L39
fn taco_xml_to_guid(value: &str) -> Uuid {
    use base64::{engine::general_purpose, Engine as _};
    let mut raw_guid = [0u8; 16];
    if let Ok(len) = general_purpose::STANDARD.decode_slice(value, &mut raw_guid) {
        if len == 16 {
            return Uuid::from_bytes_le(raw_guid);
        }
    }
    Uuid::from_bytes_le(md5::compute(value).0)
}

pub fn parse_pack_def(
    pack: &mut Pack,
    ctx: &mut impl PackLoaderContext,
    asset: &str,
) -> anyhow::Result<()> {
    let mut stream = ctx.load_asset(asset)?;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf)?;
    let data = String::from_utf8_lossy(&buf);
    let mut parser = xml::EventReader::new(Cursor::new(data.into_owned().into_bytes()));

    match inner_parse_pack_def(pack, ctx, &mut parser) {
        Ok(()) => Ok(()),
        Err(e) => Err(e).context(format!("Parsing pack def at {asset}:{}", parser.position())),
    }
}

fn merge_category_attributes(pack: &mut Pack) {
    for id in &pack.categories.root_categories {
        inner_merge_category_attributes(&mut pack.categories.all_categories, id);
    }
}

fn inner_merge_category_attributes(categories: &mut IndexMap<String, Category>, parent: &str) {
    let attrs = categories[parent].marker_attributes.clone();
    let children = categories[parent].sub_categories.clone();
    for (_, id) in &*children {
        if let Some(category) = categories.get_mut(id) {
            Arc::make_mut(&mut category.marker_attributes).merge(&attrs);
        } else {
            log::error!("Inconsistent internal state, missing category `{id}`");
            continue;
        }
        inner_merge_category_attributes(categories, id);
    }
}

fn apply_marker_attributes(pack: &mut Pack) {
    for poi in &mut pack.pois {
        let Some(category) = pack.categories.all_categories.get(&poi.category) else {
            continue;
        };
        poi.attributes.merge(&category.marker_attributes);
    }
    for trail in &mut pack.trails {
        let Some(category) = pack.categories.all_categories.get(&trail.category) else {
            continue;
        };
        trail.attributes.merge(&category.marker_attributes);
    }
}

fn inner_parse_pack_def(
    pack: &mut Pack,
    ctx: &mut impl PackLoaderContext,
    parser: &mut xml::EventReader<impl std::io::Read>,
) -> anyhow::Result<()> {
    let mut parse_stack: Vec<PartialItem> = Vec::with_capacity(16);

    loop {
        match parser.next()? {
            XmlEvent::StartElement {
                name, attributes, ..
            } if valid_elem_start(parse_stack.last(), &name) => {
                match name.local_name.to_ascii_lowercase().as_str() {
                    "overlaydata" => {
                        parse_stack.push(PartialItem::OverlayData);
                    }
                    "markercategory" => {
                        let category = Category::from_xml(pack, &parse_stack, attributes)?;
                        parse_stack.push(PartialItem::MarkerCategory(category));
                    }
                    "pois" => {
                        parse_stack.push(PartialItem::PoiGroup);
                    }
                    "poi" => match poi::Poi::from_xml(pack, attributes) {
                        Ok(poi) => parse_stack.push(PartialItem::Poi(poi)),
                        Err(e) => {
                            log::warn!("POI parse failed: {e:?}");
                            parse_stack.push(PartialItem::PoisonElem);
                        }
                    },
                    "trail" => match trail::Trail::from_xml(pack, ctx, attributes) {
                        Ok(trail) => parse_stack.push(PartialItem::Trail(trail)),
                        Err(e) => {
                            log::warn!("Trail parse failed: {e:?}");
                            parse_stack.push(PartialItem::PoisonElem);
                        }
                    },
                    _ => anyhow::bail!("Unexpected <{name}>"),
                }
            }
            XmlEvent::StartElement { name, .. } => anyhow::bail!("Unexpected <{name}>"),
            XmlEvent::EndElement { .. }
                if parse_stack.last().map(|i| i.is_poison()).unwrap_or(false) =>
            {
                parse_stack.pop();
            }
            XmlEvent::EndElement { name } if valid_elem_end(parse_stack.last(), &name) => {
                match name.local_name.to_ascii_lowercase().as_str() {
                    "overlaydata" | "pois" => {
                        parse_stack.pop();
                    }
                    "markercategory" => {
                        let Some(PartialItem::MarkerCategory(category)) = parse_stack.pop() else {
                            anyhow::bail!("Inconsistent internal state");
                        };

                        match parse_stack.last_mut() {
                            Some(PartialItem::OverlayData) => {
                                pack.categories
                                    .root_categories
                                    .insert(category.full_id.clone());
                            }
                            Some(PartialItem::MarkerCategory(parent)) => {
                                let subs = Arc::make_mut(&mut parent.sub_categories);
                                subs.insert(category.id.clone(), category.full_id.clone());
                            }
                            _ => anyhow::bail!("Inconsistent internal state"),
                        }
                        match pack
                            .categories
                            .all_categories
                            .entry(category.full_id.clone())
                        {
                            Entry::Occupied(mut existing) => {
                                existing.get_mut().merge(category);
                            }
                            Entry::Vacant(vacant) => {
                                vacant.insert(category);
                            }
                        }
                    }
                    "poi" => {
                        let Some(PartialItem::Poi(poi)) = parse_stack.pop() else {
                            anyhow::bail!("Inconsistent internal state");
                        };

                        pack.pois.push(poi);
                    }
                    "trail" => {
                        let Some(PartialItem::Trail(trail)) = parse_stack.pop() else {
                            anyhow::bail!("Inconsistent internal state");
                        };

                        pack.trails.push(trail);
                    }
                    _ => anyhow::bail!("Unexpected </{name}>"),
                }
            }
            XmlEvent::EndElement { name } => {
                anyhow::bail!("Unexpected </{name}>")
            }
            XmlEvent::StartDocument { .. } => {}
            XmlEvent::EndDocument => {
                if !parse_stack.is_empty() {
                    anyhow::bail!("Unexpected end of document");
                }
                break;
            }
            XmlEvent::ProcessingInstruction { .. } => {}
            XmlEvent::CData(_) => {}
            XmlEvent::Comment(_) => {}
            XmlEvent::Characters(_) => {}
            XmlEvent::Whitespace(_) => {}
        }
    }
    Ok(())
}

pub enum PartialItem {
    OverlayData,
    MarkerCategory(Category),
    PoiGroup,
    Poi(poi::Poi),
    Trail(trail::Trail),
    PoisonElem,
}

impl PartialItem {
    fn as_category(&self) -> Option<&Category> {
        match self {
            PartialItem::MarkerCategory(category) => Some(category),
            _ => None,
        }
    }

    fn is_poison(&self) -> bool {
        match self {
            PartialItem::PoisonElem => true,
            _ => false,
        }
    }
}

fn valid_elem_start(stack_top: Option<&PartialItem>, name: &xml::name::OwnedName) -> bool {
    match (name.local_name.to_ascii_lowercase().as_str(), stack_top) {
        ("overlaydata", None) => true,
        ("markercategory", Some(PartialItem::OverlayData | PartialItem::MarkerCategory(_))) => true,
        ("pois", Some(PartialItem::OverlayData)) => true,
        ("poi", Some(PartialItem::PoiGroup)) => true,
        ("trail", Some(PartialItem::PoiGroup)) => true,
        _ => false,
    }
}

fn valid_elem_end(stack_top: Option<&PartialItem>, name: &xml::name::OwnedName) -> bool {
    match (name.local_name.to_ascii_lowercase().as_str(), stack_top) {
        ("overlaydata", Some(PartialItem::OverlayData)) => true,
        ("markercategory", Some(PartialItem::MarkerCategory(_))) => true,
        ("pois", Some(PartialItem::PoiGroup)) => true,
        ("poi", Some(PartialItem::Poi(_))) => true,
        ("trail", Some(PartialItem::Trail(_))) => true,
        _ => false,
    }
}
