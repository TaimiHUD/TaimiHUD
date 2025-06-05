use {
    super::resources::Texture, anyhow::Context, bitvec::vec::BitVec, category::Category, indexmap::IndexSet, loader::PackLoaderContext, std::{
        collections::{hash_map::Entry, HashMap, HashSet},
        io::{Cursor, Read as _},
        sync::Arc,
    }, uuid::Uuid, windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11DeviceContext}, xml::{common::Position, reader::XmlEvent}
};

pub mod attributes;
pub mod category;
pub mod loader;
pub mod poi;
pub mod trail;

#[derive(Default)]
pub struct Pack {
    // Descriptive data.
    pub pois: Vec<poi::Poi>,
    pub trails: Vec<trail::Trail>,
    pub categories: CategoryCollection,

    // Actively loaded data.
    pub current_map: Option<i32>,
    pub active_categories: Vec<String>,
    pub enabled_categories: BitVec,
    pub active_trails: Vec<()>,
    pub active_pois: Vec<()>,

    // Internal rendering data.
    loader: Option<Box<dyn PackLoaderContext>>,
    texture_list: HashMap<String, PackTextureHandle>,
    textures: Vec<PackTexture>,
    loaded_textures: BitVec,
    unused_textures: BitVec,

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

        pack.loader = Some(Box::new(loader));

        Ok(pack)
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

    pub fn get_or_load_texture(
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

    pub fn prepare_new_map(&mut self, map_id: i32, device: &ID3D11Device) -> anyhow::Result<()> {
        if self.current_map == Some(map_id) {
            return Ok(());
        }

        self.unused_textures
            .copy_from_bitslice(&self.loaded_textures);

        // TODO: Prepare all of the pack items for the new map. Textures will get marked !unused if we access them.

        // Unload no longer needed textures.
        for handle in self.unused_textures.iter_ones() {
            self.textures[handle].texture = None;
            self.loaded_textures.set(handle, false);
        }
        Ok(())
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
    pub all_categories: HashMap<String, Category>,
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

fn inner_merge_category_attributes(categories: &mut HashMap<String, Category>, parent: &str) {
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
