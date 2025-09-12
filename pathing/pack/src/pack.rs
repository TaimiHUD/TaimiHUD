use {
    anyhow::Context,
    crate::{
        category::Category,
        loader::PackLoaderContext,
        poi::Poi,
        trail::Trail,
    },
    indexmap::{map::Entry, IndexMap, IndexSet},
    std::{
        fmt,
        io::{Cursor, Read as _},
        sync::Arc,
    },
    uuid::Uuid,
    xml::{common::Position, name::OwnedName, reader::XmlEvent},
};

#[derive(Default)]
pub struct Pack {
    pub name: String,

    // Descriptive data.
    pub pois: Vec<Poi>,
    pub trails: Vec<Trail>,
    pub categories: CategoryCollection,
}

impl Pack {
    pub fn load<L: PackLoaderContext>(loader: &mut L) -> anyhow::Result<Pack> {
        let mut pack = Pack::default();

        let pack_defs = loader.all_files_with_ext("xml")?;
        for def in pack_defs {
            parse_pack_def(&mut pack, loader, &def)?;
        }

        merge_category_attributes(&mut pack);
        apply_marker_attributes(&mut pack);

        Ok(pack)
    }
}

#[derive(Default)]
pub struct CategoryCollection {
    /// Map full_id -> Category
    pub all_categories: IndexMap<String, Category>,
    /// List of root categories.
    pub root_categories: IndexSet<String>,
}

pub fn taco_safe_name(value: &str, is_full: bool) -> String {
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
pub fn taco_xml_to_guid(value: &str) -> Uuid {
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
    let pack_xml = data.into_owned();
    let pack_xml = match pack_xml {
        #[cfg(feature = "fixup-ladyelyssa")]
        xml => xml
            // TODO: regex etc .-.
            // Clean up LadyElyssa.taco typos
            .replace("b&w.png", "b&amp;w.png")
            .replace(r#""[&"#, r#""[&amp;"#)
            .replace(r#"[&B"#, r#"[&amp;B"#)
            .replace("R&D Waypoint", "R&amp;D Waypoint")
            .replace(" & ", " &amp; ")
            .replace("Remains&DESTROY", "Remains&amp;DESTROY")
        ,
        #[allow(unreachable_patterns)]
        xml => xml,
    };

    let mut parser = xml::EventReader::new(Cursor::new(pack_xml.into_bytes()));

    match inner_parse_pack_def(pack, ctx, &mut parser, asset) {
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
    asset: &str,
) -> anyhow::Result<()> {
    let mut parse_stack: Vec<PartialItem> = Vec::with_capacity(16);

    loop {
        let elem = parser.next()?;
        let elem = match elem {
            #[cfg(feature = "fixup-ladyelyssa")]
            XmlEvent::StartElement { name, attributes, namespace } if name.local_name.eq_ignore_ascii_case("MarkerCategorykerCategory") => {
                // LadyElyssa.taco typo/corruption
                log::debug!("compensating for invalid element {name}");
                XmlEvent::StartElement {
                    name: OwnedName::local("markercategory"),
                    attributes,
                    namespace,
                }
            },
            #[cfg(feature = "fixup-ladyelyssa")]
            XmlEvent::EndElement { name } if name.local_name.eq_ignore_ascii_case("MarkerCategorykerCategory") => {
                log::debug!("compensating for invalid element {name}");
                XmlEvent::EndElement {
                    name: OwnedName::local("markercategory"),
                }
            },
            elem => elem,
        };
        match &elem {
            #[cfg(feature = "fixup-tehstrails")]
            XmlEvent::StartElement { name, .. } if name.local_name.eq_ignore_ascii_case("poi") && parse_stack.last().map(|p| matches!(p, PartialItem::OverlayData)).unwrap_or(false) => {
                // TehsTrails/Parser/TehsTrails.xml issue
                log::debug!("compensating for invalid element <{}> inside OverlayData", name);
                parse_stack.push(PartialItem::PoiGroup);
            },
            #[cfg(feature = "fixup-tehstrails")]
            XmlEvent::EndElement { name, .. } if name.local_name.eq_ignore_ascii_case("overlaydata") && parse_stack.last().map(|p| matches!(p, PartialItem::PoiGroup)).unwrap_or(false) => {
                parse_stack.pop();
            },
            _ => (),
        }
        match elem {
            XmlEvent::StartElement {
                name, attributes, ..
            } if valid_elem_start(parse_stack.last(), &name) => {
                match name.local_name.to_ascii_lowercase().as_str() {
                    "overlaydata" => {
                        parse_stack.push(PartialItem::OverlayData);
                    }
                    "markercategory" => {
                        let category = Category::from_xml(&parse_stack, attributes)?;
                        parse_stack.push(PartialItem::MarkerCategory(category));
                    }
                    "pois" => {
                        parse_stack.push(PartialItem::PoiGroup);
                    }
                    "poi" => match Poi::from_xml(asset, attributes) {
                        Ok(poi) => parse_stack.push(PartialItem::Poi(poi)),
                        Err(e) => {
                            log::warn!("POI parse failed: {e:?}");
                            parse_stack.push(PartialItem::PoisonElem);
                        }
                    },
                    "trail" => match Trail::from_xml(ctx, asset, attributes) {
                        Ok(trail) => parse_stack.push(PartialItem::Trail(trail)),
                        Err(e) => {
                            log::warn!("Trail parse failed: {e:?}");
                            parse_stack.push(PartialItem::PoisonElem);
                        }
                    },
                    _ => anyhow::bail!("Unexpected <{name}> while parsing {}", parse_stack.last().unwrap_or(&PartialItem::PoisonElem)),
                }
            }
            XmlEvent::StartElement { name, .. } => anyhow::bail!("Unexpected <{name}> while parsing {}", parse_stack.last().unwrap_or(&PartialItem::PoisonElem)),
            XmlEvent::EndElement { .. }
                if parse_stack.last().map(|i| matches!(i, PartialItem::PoisonElem)).unwrap_or(false) =>
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
    Poi(Poi),
    Trail(Trail),
    PoisonElem,
}

impl PartialItem {
    #[cfg(todo = "unused")]
    fn as_category(&self) -> Option<&Category> {
        match self {
            PartialItem::MarkerCategory(category) => Some(category),
            _ => None,
        }
    }
}

impl fmt::Display for PartialItem {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::OverlayData => {
                write!(f, "overlay data")
            },
            Self::PoiGroup => write!(f, "POI group"),
            Self::MarkerCategory(category) => {
                write!(f, "category {}", category.full_id)
            },
            Self::Poi(poi) => {
                write!(f, "poi {}", poi.guid)
            },
            Self::Trail(trail) => {
                write!(f, "trail {}", trail.guid)
            },
            Self::PoisonElem => write!(f, "poisoned"),
        }
    }
}

fn valid_elem_start(stack_top: Option<&PartialItem>, name: &OwnedName) -> bool {
    match (name.local_name.to_ascii_lowercase().as_str(), stack_top) {
        ("overlaydata", None) => true,
        ("markercategory", Some(PartialItem::OverlayData | PartialItem::MarkerCategory(_))) => true,
        ("pois", Some(PartialItem::OverlayData)) => true,
        ("poi", Some(PartialItem::PoiGroup)) => true,
        ("trail", Some(PartialItem::PoiGroup)) => true,
        _ => false,
    }
}

fn valid_elem_end(stack_top: Option<&PartialItem>, name: &OwnedName) -> bool {
    match (name.local_name.to_ascii_lowercase().as_str(), stack_top) {
        ("overlaydata", Some(PartialItem::OverlayData)) => true,
        ("markercategory", Some(PartialItem::MarkerCategory(_))) => true,
        ("pois", Some(PartialItem::PoiGroup)) => true,
        ("poi", Some(PartialItem::Poi(_))) => true,
        ("trail", Some(PartialItem::Trail(_))) => true,
        _ => false,
    }
}
