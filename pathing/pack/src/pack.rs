use {
    crate::{
        attributes,
        category::{
            id::{CategoryId, FullIdRef},
            Category,
        },
        loader::PackLoaderContext,
        poi::Poi,
        trail::Trail,
    },
    anyhow::Context,
    indexmap::{map::Entry, IndexMap, IndexSet},
    std::{
        fmt,
        io::{Cursor, Read as _},
        iter,
        mem,
        path::Path,
    },
    uuid::Uuid,
    xml::{common::Position, name::OwnedName, reader::XmlEvent},
};

#[derive(Debug, Clone, Default)]
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
            let res = def.and_then(|def| parse_pack_def(&mut pack, loader, &def.to_string_lossy()));
            match res {
                Ok(()) => (),
                Err(e) if strict => return Err(e.into()),
                Err(e) => {
                    log::error!("Pack load failure: {e:#}");
                    error.get_or_insert(e);
                },
            }
        }

        match error {
            Some(e) if pack.is_empty() => return Err(e.into()),
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

#[derive(Debug, Clone, Default)]
pub struct CategoryCollection {
    /// Map full_id -> Category
    pub all_categories: IndexMap<CategoryId, Category>,
    /// List of root categories.
    pub root_categories: IndexSet<CategoryId>,
}

impl CategoryCollection {
    pub fn is_empty(&self) -> bool {
        self.all_categories.is_empty()
        // && self.root_categories.is_empty()
    }

    pub fn root_categories(&self) -> impl Iterator<Item = &Category> {
        self.root_categories
            .iter()
            .filter_map(|c| self.all_categories.get(c))
    }

    pub fn trim_attributes(&mut self) {
        for cat in self.all_categories.values_mut() {
            cat.trim_attributes();
        }
    }
}

fn taco_safe_char(is_full: bool, c: char) -> bool {
    c.is_ascii_alphanumeric() || (is_full && c == '.')
}
pub fn to_taco_safe_name<V: AsRef<str>>(value: V, is_full: bool) -> Result<V, String> {
    let mut result = None::<String>;
    let s = value.as_ref();
    let mut segments = s.split(|c| !taco_safe_char(is_full, c)).peekable();
    while let Some(segment) = segments.next() {
        let result = match &mut result {
            None if segments.peek().is_none() => continue,
            Some(result) => {
                result.push('_');
                result
            },
            result @ None => result.insert(String::with_capacity(s.len())),
        };
        result.push_str(segment);
    }
    match result {
        Some(r) => Err(r),
        None => Ok(value),
    }
}
pub fn taco_safe_name(value: &str, is_full: bool) -> String {
    to_taco_safe_name(value, is_full)
        .map(String::from)
        .unwrap_or_else(|safe| safe)
}

pub fn file_path_eq<P: AsRef<[u8]>>(locator: &str, path: P) -> bool {
    let locator = locator.as_bytes();
    let path = path.as_ref();
    if path.len() != locator.len() {
        return false
    }
    locator.iter().zip(path.iter()).all(|(&l, &p)| match (l, p) {
        // path seps whee
        (b'/', b'\\') | (b'\\', b'/') => true,
        (l, p) => l.eq_ignore_ascii_case(&p),
    })
}

/// I hate this. See: <https://github.com/blish-hud/Pathing/blob/25b65248c7861e585b2e80a52ffb7fd4ddb371d5/Utility/AttributeParsingUtil.cs#L39>
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

fn inner_merge_category_attributes(categories: &mut IndexMap<CategoryId, Category>, parent_id: &FullIdRef) {
    use indexmap::map::MutableKeys;

    let error = "Inconsistent internal state, missing category";
    let Some((parent_index, _, parent)) = categories.get_full_mut(parent_id) else {
        log::error!("{error} parent {:?}", parent_id.as_str());
        return
    };
    let attrs = parent.marker_attributes.clone();
    let mut children = mem::take(&mut parent.sub_categories);
    for id in children.iter_mut() {
        let child_index = if let Some((child_index, _child_id, category)) = categories.get_full_mut2(id) {
            category.attributes_mut().merge(&attrs, true);
            child_index
        } else {
            log::error!("{error} child {:?}", id.as_str());
            continue;
        };

        inner_merge_category_attributes(categories, id);

        let child_id = categories
            .get_index(child_index)
            .map(|(_, child)| child.full_id.clone());
        if let Some(child_id) = child_id {
            *id = child_id;
        }
    }
    if let Some((id, parent)) = categories.get_index_mut2(parent_index) {
        // deduplicate all child ID allocations...
        parent.sub_categories = children;
        // ... then pick a child to truncate for parent ID
        let id_redundant = parent.full_id.is_full_id();
        let child_id = id_redundant
            .then_some(parent.sub_categories.first())
            .and_then(|id| id.cloned());
        if let Some(child_id) = child_id {
            let truncated =
                unsafe { CategoryId::new_unchecked(child_id.inner().clone(), parent.full_id.len()) };
            debug_assert_eq!(&parent.full_id, &truncated);
            parent.full_id = truncated;
        }
        // finally deduplicate map keys too
        *id = parent.full_id.clone();
    }
}

fn apply_marker_attributes(pack: &mut Pack) {
    for poi in &mut pack.pois {
        let Some(category) = pack.categories.all_categories.get(&poi.category[..]) else {
            continue;
        };
        if let Some(id) = category.full_id.as_full_id() {
            poi.category = id.clone();
        }
        poi.attributes.merge(&category.marker_attributes, true);
    }
    for trail in &mut pack.trails {
        let Some(category) = pack.categories.all_categories.get(&trail.category[..]) else {
            continue;
        };
        if let Some(id) = category.full_id.as_full_id() {
            trail.category = id.clone();
        }
        trail.attributes.merge(&category.marker_attributes, true);
        let _ = trail.attributes.interaction.take();
    }
}

fn inner_parse_pack_def(
    pack: &mut Pack,
    ctx: &mut impl PackLoaderContext,
    parser: &mut xml::EventReader<impl std::io::Read>,
    asset: &str,
) -> anyhow::Result<()> {
    let mut parse_stack: Vec<PartialItem> = Vec::with_capacity(16);
    let asset_parent = Path::new(asset).parent().map(|p| {
        let mut parent = p.to_string_lossy().into_owned();
        parent.push_str("/");
        attributes::string_into(parent)
    });

    loop {
        let elem = parser.next()?;
        let elem = match elem {
            #[cfg(feature = "fixup-ladyelyssa")]
            XmlEvent::StartElement { name, attributes, namespace }
                if name.local_name.eq_ignore_ascii_case("MarkerCategorykerCategory") =>
            {
                // LadyElyssa.taco typo/corruption
                log::debug!("compensating for invalid element {name}");
                XmlEvent::StartElement {
                    name: OwnedName::local("markercategory"),
                    attributes,
                    namespace,
                }
            },
            #[cfg(feature = "fixup-ladyelyssa")]
            XmlEvent::EndElement { name }
                if name.local_name.eq_ignore_ascii_case("MarkerCategorykerCategory") =>
            {
                log::debug!("compensating for invalid element {name}");
                XmlEvent::EndElement {
                    name: OwnedName::local("markercategory"),
                }
            },
            elem => elem,
        };
        match &elem {
            #[cfg(feature = "fixup-tehstrails")]
            XmlEvent::StartElement { name, .. }
                if name.local_name.eq_ignore_ascii_case("poi")
                    && parse_stack
                        .last()
                        .map(|p| matches!(p, PartialItem::OverlayData))
                        .unwrap_or(false) =>
            {
                // TehsTrails/Parser/TehsTrails.xml issue
                log::debug!("compensating for invalid element <{}> inside OverlayData", name);
                parse_stack.push(PartialItem::PoiGroup);
            },
            #[cfg(feature = "fixup-tehstrails")]
            XmlEvent::EndElement { name, .. }
                if name.local_name.eq_ignore_ascii_case("overlaydata")
                    && parse_stack
                        .last()
                        .map(|p| matches!(p, PartialItem::PoiGroup))
                        .unwrap_or(false) =>
            {
                parse_stack.pop();
            },
            _ => (),
        }
        match elem {
            XmlEvent::StartElement { name, attributes, .. }
                if valid_elem_start(parse_stack.last(), &name) =>
            {
                match name.local_name.to_ascii_lowercase().as_str() {
                    "overlaydata" => {
                        parse_stack.push(PartialItem::OverlayData);
                    },
                    "markercategory" => {
                        let category = Category::from_xml(&parse_stack, attributes)?;
                        parse_stack.push(PartialItem::MarkerCategory(category));
                    },
                    "pois" => {
                        parse_stack.push(PartialItem::PoiGroup);
                    },
                    "poi" => match Poi::from_xml(asset_parent.as_ref(), attributes) {
                        Ok(poi) => parse_stack.push(PartialItem::Poi(poi)),
                        Err(e) => {
                            log::warn!("POI parse failed in {asset}: {e:#}");
                            parse_stack.push(PartialItem::PoisonElem);
                        },
                    },
                    "trail" => {
                        let trail =
                            Trail::from_xml(asset_parent.as_ref(), attributes).and_then(|mut trail| {
                                if trail.map_id.is_none() {
                                    trail.update_map_id(ctx)?
                                }
                                Ok(trail)
                            });
                        match trail {
                            Ok(trail) => parse_stack.push(PartialItem::Trail(trail)),
                            Err(e) => {
                                log::warn!("Trail parse failed in {asset}: {e:#}");
                                parse_stack.push(PartialItem::PoisonElem);
                            },
                        }
                    },
                    _ => anyhow::bail!(
                        "Unexpected <{name}> while parsing {}",
                        parse_stack.last().unwrap_or(&PartialItem::PoisonElem)
                    ),
                }
            },
            #[cfg(feature = "fixup-typos")]
            XmlEvent::StartElement { name, .. } | XmlEvent::EndElement { name, .. }
                if name.local_name.eq_ignore_ascii_case("route") =>
            {
                // GW2 TacO ReActif FR Externe.taco?
                log::warn!("ignoring unsupported <{name}> group");
            },
            XmlEvent::StartElement { name, .. } => anyhow::bail!(
                "Unexpected <{name}> while parsing {}",
                parse_stack.last().unwrap_or(&PartialItem::PoisonElem)
            ),
            XmlEvent::EndElement { .. }
                if parse_stack
                    .last()
                    .map(|i| matches!(i, PartialItem::PoisonElem))
                    .unwrap_or(false) =>
            {
                parse_stack.pop();
            },
            XmlEvent::EndElement { name } if valid_elem_end(parse_stack.last(), &name) => {
                match name.local_name.to_ascii_lowercase().as_str() {
                    "overlaydata" | "pois" => {
                        parse_stack.pop();
                    },
                    "markercategory" => {
                        let Some(PartialItem::MarkerCategory(category)) = parse_stack.pop() else {
                            anyhow::bail!("Inconsistent internal state");
                        };

                        match parse_stack.last_mut() {
                            Some(PartialItem::OverlayData) => {
                                pack.categories.root_categories.insert(category.full_id.clone());
                            },
                            Some(PartialItem::MarkerCategory(parent)) => {
                                parent.append_children(iter::once(category.full_id.clone()));
                            },
                            _ => anyhow::bail!("Inconsistent internal state"),
                        }
                        match pack.categories.all_categories.entry(category.full_id.clone()) {
                            Entry::Occupied(mut existing) => {
                                existing.get_mut().merge(category);
                            },
                            Entry::Vacant(vacant) => {
                                vacant.insert(category);
                            },
                        }
                    },
                    "poi" => {
                        let Some(PartialItem::Poi(poi)) = parse_stack.pop() else {
                            anyhow::bail!("Inconsistent internal state");
                        };

                        pack.pois.push(poi);
                    },
                    "trail" => {
                        let Some(PartialItem::Trail(trail)) = parse_stack.pop() else {
                            anyhow::bail!("Inconsistent internal state");
                        };

                        pack.trails.push(trail);
                    },
                    _ => anyhow::bail!("Unexpected </{name}>"),
                }
            },
            XmlEvent::EndElement { name } => {
                anyhow::bail!("Unexpected </{name}>")
            },
            XmlEvent::StartDocument { .. } => {},
            XmlEvent::EndDocument => {
                if !parse_stack.is_empty() {
                    anyhow::bail!("Unexpected end of document");
                }
                break;
            },
            XmlEvent::ProcessingInstruction { .. } => {},
            XmlEvent::CData(_) => {},
            XmlEvent::Comment(_) => {},
            XmlEvent::Characters(_) => {},
            XmlEvent::Whitespace(_) => {},
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
        std::{borrow::Cow, fmt::Write, sync::LazyLock},
    };

    macro_rules! pats {
        (&) => {
            r#"(?<amp_pre>[^&"]*)&(?<amp_ok>[#a-zA-Z0-9]{1,5};)?(?<amp_post>[^"]*)"#
        };
    }
    const PAT_FIXUP_AMPERSAND: &'static str = pats!(&);
    const PAT_FIXUP: &'static str = concat!(
        r#"=\s*""#,
        pats!(&),
        "\"",
        // reactif-en
        "|",
        r#"\s+(?<attr_typo>nim[sS]ize|reset[lL]enght)\s*=\s*""#,
        "|",
        r#""(?<attr_nospace>"#,
        // reactif-fr
        "fadeNear|zpos",
        // linus voe
        "|GUID",
        r#")\s*=\s*""#,
        // rediche's WvW marker pack
        "|",
        r#"(?<dup_attr>type)\s*=\s*"(?<dup_attr_v0>[^" ]+)"\s+type\s*=\s*"(?<dup_attr_v1>[^" ]+)""#,
        "|",
        r#"(?<dup_attr1>miniMapVisibility)\s*=\s*"(?<dup_attr1_v0>[^" ]+)".+mapVisibility=[^ ]+ miniMapVisibility="(?<dup_attr1_v1>[^" ]+)""#,
        // lady elyssa v21.8
        "|",
        r#"<MarkerCatego(?<garbage_prefix><MarkerCategory)"#,
        "|",
        r#"=="copy="\[(?<copy_rest>&[^"]*)""#,
    );

    fn new_regex(pattern: &'static str) -> Regex {
        let regex = RegexBuilder::new(pattern)
            .multi_line(true)
            .dot_matches_new_line(false)
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
        captures.get(0).map(|m| m.as_str()).unwrap_or_default()
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
            if let (Some(amp_pre), amp_ok, Some(amp_post)) =
                (caps.name("amp_pre"), caps.name("amp_ok"), caps.name("amp_post"))
            {
                let amp_pre = amp_pre.as_str();
                let amp_post = amp_post.as_str();
                let amp_ok = amp_ok.map(|ok| ok.as_str()).unwrap_or("amp;");
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
                let _ = write!(dst, "\" {attr_nospace}=\"");
            } else if let (Some(dup_attr), Some(dup_attr_v0), Some(dup_attr_v1)) = (
                caps.name("dup_attr").or_else(|| caps.name("dup_attr1")),
                caps.name("dup_attr_v0").or_else(|| caps.name("dup_attr1_v0")),
                caps.name("dup_attr_v1").or_else(|| caps.name("dup_attr1_v1")),
            ) {
                let dup_attr = dup_attr.as_str();
                let dup_attr_v0 = dup_attr_v0.as_str();
                let dup_attr_v1 = dup_attr_v1.as_str();
                let dup_attr_mid = caps
                    .name("dup_attr_mid")
                    .or_else(|| caps.name("dup_attr1_mid"))
                    .map(|m| m.as_str())
                    .unwrap_or_default();
                if dup_attr_v0 != dup_attr_v1 {
                    log::error!("pack contains inconsistent duplicate:  {dup_attr}={dup_attr_v0:?} and {dup_attr}={dup_attr_v1:?}");
                }
                let _ = write!(dst, r#"{dup_attr_mid}{dup_attr}="{dup_attr_v0}""#);
            } else if let Some(garbage_prefix) = caps.name("garbage_prefix") {
                let garbage_prefix = garbage_prefix.as_str();
                let _ = dst.write_str(garbage_prefix);
            } else if let Some(copy_rest) = caps.name("copy_rest") {
                let copy_rest = copy_rest.as_str();
                let copy_rest = FIXUP_AMP.replace_all(copy_rest, ReplacementsAmp { rec: true });
                let _ = write!(dst, r#"==" copy="[{copy_rest}""#);
            } else {
                replacements_bad(caps, dst)
            }
        }
    }

    let pack_xml = FIXUP.replace_all(&pack_xml, Replacements);

    // Metal Marker Myriad case mismatch
    const PAT_FIXUP_OVERLAYDATA: &'static str = r#"<(?<openclose>\/)?(?i)(?<tag>overlaydata)\b"#;
    static FIXUP_OVERLAYDATA: LazyLock<Regex> = LazyLock::new(|| new_regex(PAT_FIXUP_OVERLAYDATA));

    let overlaydata_matches = |n: u8| {
        FIXUP_OVERLAYDATA
            .captures_iter(&pack_xml)
            .filter_map(|x| x.name("tag"))
            .map(|m| m.as_str())
            .map(|m| m.as_bytes().get(0).copied())
            .any(|o| o == Some(n))
    };
    let inconsistent_case = overlaydata_matches(b'o') && overlaydata_matches(b'O');
    let pack_xml = match inconsistent_case {
        true => FIXUP_OVERLAYDATA
            .replace_all(&pack_xml, "<${openclose}OverlayData")
            .into_owned()
            .into(),
        false => pack_xml,
    };
    pack_xml
}
