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
        Self::load_strict(loader, false)
    }

    pub fn load_strict<L: PackLoaderContext>(loader: &mut L, strict: bool) -> anyhow::Result<Pack> {
        let mut pack = Pack::default();

        let pack_defs = loader.all_files_with_ext_owned("xml");
        let mut error = None;
        for def in pack_defs {
            let res = def.and_then(|def|
                parse_pack_def(&mut pack, loader, &def.to_string_lossy())
            );
            match res {
                Ok(()) => (),
                Err(e) if strict =>
                    return Err(e.into()),
                Err(e) => {
                    log::error!("Pack load failure: {e}");
                    error.get_or_insert(e);
                },
            }
        }

        match error {
            Some(e) if pack.is_empty() =>
                return Err(e.into()),
            _ => (),
        }

        merge_category_attributes(&mut pack);
        apply_marker_attributes(&mut pack);

        Ok(pack)
    }

    pub fn is_empty(&self) -> bool {
        self.categories.is_empty() && self.pois.is_empty() && self.trails.is_empty()
    }
}

#[derive(Default)]
pub struct CategoryCollection {
    /// Map full_id -> Category
    pub all_categories: IndexMap<String, Category>,
    /// List of root categories.
    pub root_categories: IndexSet<String>,
}

impl CategoryCollection {
    pub fn is_empty(&self) -> bool {
        self.all_categories.is_empty() && self.root_categories.is_empty()
    }
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

pub fn file_path_eq<P: AsRef<[u8]>>(locator: &str, path: P) -> bool {
    let locator = locator.as_bytes();
    let path = path.as_ref();
    if path.len() != locator.len() {
        return false
    }
    locator.iter().zip(path.iter())
        .all(|(&l, &p)| match (l, p) {
            // path seps whee
            (b'/', b'\\') | (b'\\', b'/') => true,
            (l, p) => l.eq_ignore_ascii_case(&p),
        })
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
    let pack_xml = String::from_utf8_lossy(&buf);
    #[cfg(feature = "fixup-typos")]
    let pack_xml = fixup_xml_typos(&pack_xml);

    let mut parser = xml::EventReader::new(Cursor::new(pack_xml.into_owned().into_bytes()));

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
                            log::warn!("POI parse failed in {asset}: {e}");
                            parse_stack.push(PartialItem::PoisonElem);
                        }
                    },
                    "trail" => match Trail::from_xml(ctx, asset, attributes) {
                        Ok(trail) => parse_stack.push(PartialItem::Trail(trail)),
                        Err(e) => {
                            log::warn!("Trail parse failed in {asset}: {e}");
                            parse_stack.push(PartialItem::PoisonElem);
                        }
                    },
                    _ => anyhow::bail!("Unexpected <{name}> while parsing {}", parse_stack.last().unwrap_or(&PartialItem::PoisonElem)),
                }
            }
            #[cfg(feature = "fixup-typos")]
            XmlEvent::StartElement { name, .. } | XmlEvent::EndElement { name, .. } if name.local_name.eq_ignore_ascii_case("route") => {
                // GW2 TacO ReActif FR Externe.taco?
                log::warn!("ignoring unsupported <{name}> group");
            },
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

#[cfg(feature = "fixup-typos")]
fn fixup_xml_typos(pack_xml: &str) -> std::borrow::Cow<'_, str> {
    use {
        regex::{Captures, Regex, RegexBuilder, Replacer},
        std::{
            borrow::Cow,
            fmt::Write,
            sync::LazyLock,
        },
    };

    macro_rules! pats {
        (&) => {
            r#"(?<amp_pre>[^&"]*)&(?<amp_ok>[#a-zA-Z0-9]{1,5};)?(?<amp_post>[^"]*)"#
        };
    }
    const PAT_FIXUP_AMPERSAND: &'static str = pats!(&);
    const PAT_FIXUP: &'static str = concat!(
        r#"=\s*""#, pats!(&), "\"",
        // reactif-en
        "|", r#"\s+(?<attr_typo>nim[sS]ize|reset[lL]enght)\s*=\s*""#,
        "|", r#"(?<attr_nospace>fadeNear|zpos)\s*=\s*""#,
        // rediche's WvW marker pack
        "|", r#"(?<dup_attr>type)\s*=\s*"(?<dup_attr_v0>[^" ]+)"\s+type\s*=\s*"(?<dup_attr_v1>[^" ]+)""#,
        "|", r#"(?<dup_attr1>miniMapVisibility)\s*=\s*"(?<dup_attr1_v0>[^" ]+)".+mapVisibility=[^ ]+ miniMapVisibility="(?<dup_attr1_v1>[^" ]+)""#,
    );

    fn new_regex(pattern: &'static str) -> Regex {
        let regex = RegexBuilder::new(pattern)
            .multi_line(true).dot_matches_new_line(false)
            .crlf(true)
            .unicode(true)
            .build();
        match regex {
            Ok(r) => r,
            Err(e) => {
                log::error!("fixup regex failed to build: {e}");
                // try to produce a dummy that allows us to proceed anyway
                Regex::new("").unwrap()
            },
        }
    }
    static FIXUP: LazyLock<Regex> = LazyLock::new(|| new_regex(PAT_FIXUP));
    static FIXUP_AMP: LazyLock<Regex> = LazyLock::new(|| new_regex(PAT_FIXUP_AMPERSAND));

    /// return entire match as a fallback
    fn replacements_0<'a>(captures: &regex::Captures<'a>) -> &'a str {
        captures.get(0)
            .map(|m| m.as_str())
            .unwrap_or_default()
    }
    fn replacements_bad(captures: &regex::Captures, dst: &mut String) {
        log::error!("unexpected pack xml fixup match, this is a bug");
        dst.push_str(replacements_0(captures));
    }

    struct ReplacementsAmp {
        rec: bool,
    }
    impl Replacer for ReplacementsAmp {
        fn replace_append(&mut self, caps: &Captures<'_>, dst: &mut String) {
            if let (Some(amp_pre), amp_ok, Some(amp_post)) = (caps.name("amp_pre"), caps.name("amp_ok"), caps.name("amp_post")) {
                let amp_pre = amp_pre.as_str();
                let amp_post = amp_post.as_str();
                let amp_ok = amp_ok.map(|ok| ok.as_str())
                    .unwrap_or("amp;");
                let (prefix, postfix) = match self.rec {
                    false => (r#"=""#, "\""),
                    true => ("", ""),
                };
                let amp_post = if amp_post.contains("&") {
                    // whee recursion
                    let post = FIXUP_AMP.replace_all(amp_post, ReplacementsAmp { rec: true });
                    post
                } else {
                    Cow::Borrowed(amp_post)
                };
                let _ = write!(dst, "{prefix}{amp_pre}&{amp_ok}{amp_post}{postfix}");
            } else {
                replacements_bad(caps, dst)
            }
        }
    }
    struct Replacements;
    impl Replacer for Replacements {
        fn replace_append(&mut self, caps: &Captures<'_>, dst: &mut String) {
            if let (Some(..), Some(..)) = (caps.name("amp_pre"), caps.name("amp_post")) {
                ReplacementsAmp { rec: false }.replace_append(caps, dst)
            } else if let Some(attr_type) = caps.name("attr_typo") {
                let replacement = match attr_type.as_str() {
                    "nimsize" | "nimSize" => " minsize=\"",
                    "resetlenght" | "resetLenght" => " resetlength=\"",
                    typo => {
                        log::error!("unexpected typo {typo:?}");
                        replacements_0(caps)
                    },
                };
                dst.push_str(replacement);
            } else if let Some(attr_nospace) = caps.name("attr_nospace") {
                let attr_nospace = attr_nospace.as_str();
                let _ = write!(dst, " {attr_nospace}=\"");
            } else if let (Some(dup_attr), Some(dup_attr_v0), Some(dup_attr_v1)) = (
                caps.name("dup_attr").or_else(|| caps.name("dup_attr1")),
                caps.name("dup_attr_v0").or_else(|| caps.name("dup_attr1_v0")),
                caps.name("dup_attr_v1").or_else(|| caps.name("dup_attr1_v1")),
            ) {
                let dup_attr = dup_attr.as_str();
                let dup_attr_v0 = dup_attr_v0.as_str();
                let dup_attr_v1 = dup_attr_v1.as_str();
                let dup_attr_mid = caps.name("dup_attr_mid")
                    .or_else(|| caps.name("dup_attr1_mid"))
                    .map(|m| m.as_str()).unwrap_or_default();
                if dup_attr_v0 != dup_attr_v1 {
                    log::error!("pack contains inconsistent duplicate:  {dup_attr}={dup_attr_v0:?} and {dup_attr}={dup_attr_v1:?}");
                }
                let _ = write!(dst, r#"{dup_attr_mid}{dup_attr}="{dup_attr_v0}""#);
            } else {
                replacements_bad(caps, dst)
            }
        }
    }

    let pack_xml = FIXUP.replace_all(&pack_xml, Replacements);

    // Metal Marker Myriad case mismatch
    const PAT_FIXUP_OVERLAYDATA: &'static str = r#"<(?<openclose>\/)?(?i)(?<tag>overlaydata)\b"#;
    static FIXUP_OVERLAYDATA: LazyLock<Regex> = LazyLock::new(|| new_regex(PAT_FIXUP_OVERLAYDATA));

    let overlaydata_matches = |n: u8|
        FIXUP_OVERLAYDATA.captures_iter(&pack_xml)
            .filter_map(|x| x.name("tag"))
            .map(|m| m.as_str())
            .map(|m| m.as_bytes().get(0).copied())
            .any(|o| o == Some(n));
    let inconsistent_case = overlaydata_matches(b'o') && overlaydata_matches(b'O');
    let pack_xml = match inconsistent_case {
        true => FIXUP_OVERLAYDATA.replace_all(&pack_xml, "<${openclose}OverlayData").into_owned().into(),
        false => pack_xml,
    };
    pack_xml
}
