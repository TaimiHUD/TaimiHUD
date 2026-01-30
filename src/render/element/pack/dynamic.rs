use crate::controller::pathing::space::TrailGeometryRequests;
use crate::util::PositionInput;
use std::io::{self, Write, Seek};
use std::fs;
use std::borrow::Cow;
use anyhow::{anyhow, Context};
use taimi_meta::packs::SectionOfTrail;
use crate::{controller::{pathing::{
    info::MapPackInfo, registry::{PackRoot, PackCategoryInfo, LoadedMarkerPath, LoaderBox, PackFormat, PackIndex, PackInfo, PackInfoSignature, PackPath, PackRegistryNs, SharedLoaderBox}, shared::{PathingShared, SharedMapPackLoaded, SharedMapPackState, SharedPackConfig, SharedPackInfo, SharedPackLoad, SharedPackLoaded}, state::LoadedMapPack, PathingEvent, UnloadedReason
}, Controller}, settings::SourceKind, TEXTURES};
use crate::exports::runtime as rt;
use glamour::Point3;
use glam::Vec4;
use taimi_hoard::{str_opt, str_opt_ref};
use relative_path::PathExt;
use uuid::Uuid;
use std::mem;
use std::fmt;
use taimi_sync::watched::Watched;
use std::hash::Hash;
use std::iter;
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;
use taimi_hoard::loc::{indexed::IndexedList, LocationRef, LocationMut};
use crate::render::element::im::prelude::*;
use std::collections::BTreeMap;
use crate::exports::runtime::textures::{TextureKey, TextureSlot};
use taimi_meta::packs::{
    id::{MarkerId, MarkerIndex},
    CategoryPath,
    MarkerPath, CategoryIndex,
    PoiIndex, TrailIndex, TrailSectionIndex,
    MapIndex,
    PoiPath, TrailPath, TrailSectionPath,
};
use taimi_meta::coords::LocalSpace;
use taimi_pack::{category::{id::{IdNameBox, FullIdRef, AsFullId}, Category, CategoryFlags, CategoryId}, loader::DirectoryLoader, Poi, trail::{Trail, TrailSection, TrailHeader, TrailData},
    attributes::{string_into, AttrString, MarkerAttributes},
    trail::TrlPath,
};
use imgui::TreeNode;
use taimi_pack::Pack;
use std::path::{PathBuf, Path};
use super::{PackElement, PackVisibility};

pub struct PackEdit {
    pub pack_path: PackPath,
    pub info_sig: PackInfoSignature,
    pub root_dir: PathBuf,
    pub env: PackEditEnv,
    pub cursor: EditCursor,
    pub pack: PackData,
    pub loaded: Watched<SharedPackLoaded>,
    pub config: Watched<SharedPackConfig>,
    pub loader: Option<SharedLoaderBox>,
    pub map_info: Option<Arc<MapPackInfo>>,
    pub map_state: Option<LoadedMapPack>,
    pub markers_dirty: bool,
    pub cats_dirty: bool,
}
impl PackEdit {
    pub fn empty() -> Self {
        Self {
            pack_path: PackPath::with_path(PackIndex::MAX),
            root_dir: Default::default(),
            env: Default::default(),
            cursor: Default::default(),
            pack: Default::default(),
            loader: Default::default(),
            info_sig: PackInfoSignature::EMPTY,
            map_info: Default::default(),
            map_state: Default::default(),
            loaded: Watched::EMPTY,
            config: Watched::EMPTY,
            markers_dirty: false,
            cats_dirty: false,
        }
    }
    fn clear_map(&mut self) {
        self.map_info = None;
        self.map_state = None;
    }
    pub fn close(&mut self) {
        if self.is_open() {
            if !self.pack.do_not_steal {
                PathingEvent::PackUnlock { path: self.pack_path }.try_send();
            }
        }
        self.clear_map();
        self.pack_path = PackPath::with_path(PackIndex::MAX);
        self.info_sig = PackInfoSignature::EMPTY;
        self.pack = Default::default();
        self.cursor = Default::default();
        self.loaded = Default::default();
        self.config = Default::default();
        self.loader = None;
    }
    pub fn is_open(&self) -> bool {
        self.pack_path.path != PackIndex::MAX
    }
    pub fn post_open(&mut self) {
        self.refresh_textures();
        self.env.pos.position = None;
        self.env.colour = Vec4::ONE;
    }
    pub fn pre_draw(&mut self, visibility: PackVisibility, latest_map: Option<MapIndex>) {
        if visibility.is_closed() { return }

        let dirty_map = match &self.map_state {
            Some(map) if latest_map.is_some() => Some(map.map_id) != latest_map,
            _ => false,
        };
        if dirty_map {
            self.clear_map();
        }

        self.env.latest_map = latest_map;
    }
    pub fn init(&mut self, shared: &Arc<PathingShared>) {
        self.env.shared = Some(shared.clone());
    }
    fn populate_info(&mut self) {
        if self.cats_dirty || *self.pack.pack_info.categories != self.pack.pack.categories {
            let cats = PackCategoryInfo::from_pack(&self.pack.pack);
            self.pack.pack_info.categories = Arc::new(cats);
            self.pack.pack_info.roots = PackRoot::from_category_collection(&self.pack.pack.categories).collect();
        } else if self.markers_dirty {
            let categories = Arc::make_mut(&mut self.pack.pack_info.categories);
            categories.lonely.clear();
            categories.fill_lonely_from_pack(&self.pack.pack);
        }
        //self.info_sig = PackInfoSignature::from_info(&self.pack.pack_info);
        self.info_sig.hash += 1;
        self.cats_dirty = false;
        self.markers_dirty = false;
    }
    fn update_info(&mut self) {
        let Some(shared) = &self.env.shared else { return };
        let pack_info = Arc::new(self.pack.pack_info.clone());
        self.loaded.write_if(|loaded| {
            loaded.unload(Some(UnloadedReason::Loading));
            None
        });
        shared.packs.packs.send_if_modified(|packs| {
            let Some(pack) = packs.lookup_mut(&self.pack_path) else { return false };
            // clobber with our dummy one
            if pack.info.is_dead() {
                pack.info = Arc::new(SharedPackInfo::new_unloaded(self.pack_path, self.root_dir.clone().into(), None));
            }
            let info = Arc::make_mut(&mut pack.info);
            info.sig = self.info_sig;
            info.info = Some(pack_info);
            #[cfg(todo = "unnecessary")] {
                pack.set_info(pack_info);
                self.info_sig = pack.info.sig;
            }
            true
        });
    }
    fn unload(&mut self, reason: Option<UnloadedReason>) {
        let Some(shared) = &self.env.shared else { return };
        shared.packs.packs.send_if_modified(|packs| {
            let Some(pack) = packs.lookup_mut(&self.pack_path) else { return false };
            pack.set_unloaded(reason);
            true
        });
        PathingEvent::UnloadPack(self.pack_path, true).try_send();
    }
    fn update_loaded(&mut self) {
        let pack_arc = Arc::new(self.pack.pack.clone());
        self.loaded.write_if(|loaded| {
            loaded.unloaded = None;
            loaded.loader = self.loader.clone();
            loaded.pack = Some(pack_arc);
            Some(true)
        });
        self.config.write_if(|config| {
            config.info_sig = self.info_sig;
            Some(true)
        });
    }
    fn update_gameplay(&mut self) -> bool {
        let Some(map_id) = self.env.latest_map else { return false };
        let Some(shared) = &self.env.shared else { return false };
        let map_path = self.pack_path.rel(map_id);
        let map_info = &*self.map_info.get_or_insert_with(||
            Arc::new(MapPackInfo::with_pack(map_id, &self.pack.pack, &self.pack.pack_info))
        );
        let map_state = &*self.map_state.get_or_insert_with(||
            LoadedMapPack::from_pack(map_id, map_info, &self.pack.pack)
        );
        shared.gameplay.send_modify(|gameplay| {
            let (shared_info, shared_map) = gameplay.for_pack_mut(self.pack_path);
            *shared_info = Some(SharedMapPackLoaded::with_loaded(map_path, map_info.clone(), map_state));
            *shared_map = Some(SharedMapPackState::with_static(map_path, map_state));
        });
        true
    }
    pub fn pack_alloc(&mut self) -> Result<(), ()> {
        if self.env.pack_alloc_name.is_empty() { return Err(()) }
        let Some(shared) = &self.env.shared else { return Err(()) };
        self.root_dir = SourceKind::Pathing.get_user_dir()
            .join(&self.env.pack_alloc_name);
        let _ = rt::log::error_ok(fs::create_dir_all(&self.root_dir));
        self.loader = Some(Arc::new(TokioMutex::new(Box::new(DirectoryLoader::new(&self.root_dir)) as LoaderBox)));
        self.pack.do_not_steal = true;
        self.pack.pack = Default::default();
        let mut cat = Category {
            full_id: CategoryId::with_full_id(str_opt_ref(&self.env.pack_cat_id)
                .unwrap_or(&self.env.pack_alloc_name)),
            flags: CategoryFlags::ROOT,
            display_name: Some(str_opt_ref(&self.env.pack_cat_name).unwrap_or("packedit").into()),
            sub_categories: Default::default(),
            marker_attributes: Default::default(),
        };
        if self.env.pack_cat_id.is_empty() {
            self.env.pack_cat_id = self.env.pack_alloc_name.clone();
        }
        self.env.pack_cat_id.push_str(".");
        self.env.pack_alloc_name.clear();
        self.env.apply_attrs(&mut cat.marker_attributes);
        self.cursor.category_path.path = 0;
        self.pack.pack.categories.root_categories.insert(cat.full_id.clone());
        self.pack.pack.categories.all_categories.insert(cat.full_id.clone(), cat);
        Ok(())
    }
    fn init_pack(&mut self, pack_path: PackPath) {
        self.pack_path = pack_path;
        self.pack.pack_info = PackInfo::from_pack(&self.pack.pack, PackFormat::TacoDir);
        self.markers_dirty = false;
        self.cats_dirty = false;
        if let Some(map_id) = self.env.latest_map {
            self.pack.pack_info.maps.insert(map_id);
        }
        self.info_sig = PackInfoSignature::hash_with(|h| self.root_dir.hash(h));
    }
    const CONTENT_DIR_MARKERS: &'static str = "content/markers";
    pub fn refresh_textures(&mut self) {
        let prev = mem::take(&mut self.env.avail_textures);
        let Some(shared) = self.env.shared.clone() else { return };
        let packs = shared.packs.packs.borrow();
        let loader = self.loader.as_ref().map(|l| l.blocking_lock());
        if let Some(mut loader) = loader {
            self.env.refresh_textures_dir(&mut *loader, &self.root_dir, self.pack_path);
        }
        for pack in packs.values().filter(|pack| (pack.info.index != self.pack_path) | !self.pack.do_not_steal) {
            self.env.refresh_textures(pack);
        }
        // dropping this too early could invalidate weak handles maybe...
        drop(prev);
    }
    fn asset_file_path<P: ?Sized + AsRef<Path>>(&self, asset: &P) -> Option<PathBuf> {
        matches!(self.pack.pack_info.format, PackFormat::TacoDir).then(|| self.root_dir.join(asset.as_ref()))
    }
    fn resolve_trl(&self, trl: Result<&TrlPath, TrailPath>) -> anyhow::Result<PathBuf> {
        let trl = trl.or_else(|path| {
            // TODO: read+copy from loader?
            self.pack.pack.trails.get(path.path as usize).and_then(|trail| trail.trail_path.as_ref())
                .ok_or(path)
        }).ok();
        match trl {
            Some(trl) => self.asset_file_path(&trl.path[..])
                .context("can't add file to zip"),
            None => Err(
                anyhow!("couldn't resolve trl path")
            ),
        }
    }
    fn write_to_trl(&self, trl: Result<&TrlPath, TrailPath>, trunc: bool, map_id: Option<i32>) -> anyhow::Result<fs::File> {
        let fpath = self.resolve_trl(trl)?;
        let op = match trunc {
            true => "clearing",
            false => "opening",
        };
        let context = || format!("{op} {}", rt::relative_path(&fpath).display());

        let mut open = fs::OpenOptions::new();
        let mut file = match trunc {
            true => open.create(true).write(true).truncate(true),
            #[cfg(todo)]
            false => open.create_new(true),
            #[cfg(todo)]
            false => {
                // bad idea if you want to rewind or check pos for header...
                open.create(true).append(true)
            },
            false =>
                open.create(true).write(true)
        }.open(&fpath)
        .with_context(context)?;
        if let Some(map_id) = map_id {
            let TODO = trunc;
            let pos = match trunc {
                true => Ok(0),
                false =>
                    file.seek(io::SeekFrom::End(0)),
            };
            if pos.ok() == Some(0) {
                TrailHeader::with_map_id(map_id).write_header(&mut file)?;
            }
        }
        Ok(file)
    }
}
impl fmt::Debug for PackEdit {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("PackEdit")
            .field("pack_path", &self.pack_path)
            .field("cursor", &self.cursor)
            .finish()
    }
}
impl Default for PackEdit {
    fn default() -> Self { Self::empty() }
}
#[derive(Debug)]
pub struct PackData {
    pub pack: Pack,
    pub pack_info: PackInfo,
    pub do_not_steal: bool,
}
impl PackData {
    pub fn empty() -> Self {
        Self {
            pack: Default::default(),
            pack_info: PackInfo {
                format: PackFormat::TacoDir,
                roots: Default::default(),
                categories: Default::default(),
                maps: Default::default(),
            },
            do_not_steal: false,
        }
    }
}
impl Default for PackData {
    fn default() -> Self { Self::empty() }
}
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EditCursor {
    pub id: LoadedMarkerPath,
    pub category_path: CategoryPath,
}
impl EditCursor {
    pub fn empty() -> Self {
        Self {
            id: LoadedMarkerPath::with_path(MarkerIndex::UNK),
            category_path: CategoryPath::with_path(CategoryIndex::MAX),
        }
    }
}
impl Default for EditCursor {
    fn default() -> Self { Self::empty() }
}
#[derive(Debug, Default)]
pub struct PackEditEnv {
    pub latest_map: Option<MapIndex>,
    pub latest_pos: Option<Point3<LocalSpace>>,
    pub shared: Option<Arc<PathingShared>>,
    pub avail_textures: BTreeMap<String, MarkerTexture>,
    // working data
    pub pack_alloc_name: String,
    pub pack_cat_id: String,
    pub pack_cat_name: String,
    pub tip_name: String,
    pub trl_name: String,
    pub colour: Vec4,
    pub tex_selected: String,
    pub tex_selected_source: Option<MarkerId>,
    pub pos: PositionInput,
}
impl PackEditEnv {
    pub fn refresh_textures(&mut self, pack: &SharedPackLoad) {
        let subresources = pack.info.shared_subresources();
        let Ok(subresources) = subresources.read() else { return };
        for (attr, key) in subresources.iter() {
            let slot = TEXTURES.lookup_with(key, |slot| match slot {
                #[cfg(todo = "unnecessary")]
                slot => slot.clone(),
                _ => None,
            }).flatten();
            let format = pack.info.info.as_ref().map(|i| i.format);
            let file_path = matches!(format, Some(PackFormat::TacoDir)).then(|| pack.info.path.join(&attr[..]));
            let file_path = match file_path {
                Some(p) if !p.try_exists().unwrap_or(false) => None,
                p => p,
            };
            let source = MarkerPath::with_parts(pack.info.index, MarkerIndex::UNK);
            let tex = MarkerTexture {
                source: MarkerId::for_marker(source),
                source_attr: Some(attr.clone()),
                file_path: file_path.unwrap_or_default(),
                slot: slot.unwrap_or(TextureSlot::Unavailable),
                key: key.clone(),
            };
            self.avail_textures.insert(key[..].into(), tex);
        }
    }
    fn refresh_textures_dir(&mut self, loader: &mut LoaderBox, root_dir: &Path, source: PackPath) {
        let image_extensions = [
            "png",
            "jpg",
            "jpeg",
            "bmp",
            "tiff",
            //"webm",
        ];
        for ext in image_extensions {
            let paths = loader.all_files_with_ext(ext)
                .filter_map(move |path| match path.context("search for textures") {
                    Ok(path) => Some(path.into_owned()),
                    Err(e) => {
                        log::debug!("{e:#}");
                        None
                    },
                }).collect::<Vec<_>>();
            for path in paths {
                let file_path = match path.to_str().and_then(|p| loader.asset_absolute_path(p)) {
                    Some(p) => p,
                    None if path.is_absolute() => path,
                    None => {
                        log::debug!("idk how to make {} a real path", path.display());
                        continue
                    },
                };
                let key = match file_path.relative_to(&root_dir) {
                    Ok(rel) => rel.to_string(),
                    _ => rt::relative_path(&file_path).to_string_lossy().into_owned(),
                };
                let tex = MarkerTexture {
                    source: MarkerId::for_marker(MarkerPath::with_parts(source, MarkerIndex::UNK)),
                    source_attr: None,
                    file_path,
                    slot: TextureSlot::Reserved,
                    key: key[..].into(),
                };
                self.avail_textures.insert(key, tex);
            }
        }
    }
    fn fallback_category_id(&self) -> CategoryId {
        str_opt(&self.pack_alloc_name)
            .map(CategoryId::with_full_id)
            .unwrap_or_else(|| CategoryId::with_full_id("mew"))
    }
    fn apply_attrs(&mut self, attrs: &mut MarkerAttributes) {
        if let Some(value) = str_opt_ref(&self.tip_name) {
            attrs.tip_name = Some(string_into(value));
        }
        self.tip_name.clear();
    }
    fn apply_render_attrs(&mut self, attrs: &mut MarkerAttributes) {
        if self.colour != Vec4::ONE {
            attrs.render_mut().tint = Some(self.colour);
        }
        self.colour = Vec4::ONE;
    }
    fn tex_attr(&mut self, root_dir: &Path) -> Option<AttrString> {
        let tex = self.avail_textures.get(str_opt_ref(&self.tex_selected)?);
        let tex_source = tex.as_ref()
            .and_then(|tex|
                tex.source.marker_path::<PackPath>()
                .and_then(|source| tex.source_attr.as_ref().map(|attr|
                        (source, &attr[..])
                    )
                )
            );
        let file_path = tex
            .and_then(|tex| (!tex.file_path.as_os_str().is_empty()).then_some(&tex.file_path))
            .and_then(|file_path| file_path.relative_to(&root_dir).ok());
        let attrkey = match (file_path, tex_source) {
            (Some(p), _) => Some(string_into(p.as_str())),
            (_, Some((source, attr))) => {
                let loaded = self.shared.as_ref().and_then(|shared|
                    shared.packs.packs.borrow().lookup_ref(&source.root)
                        .map(|pack| pack.loaded.clone())
                );
                let loaded = loaded.as_ref().map(|l| l.borrow());
                let mut loader = loaded.as_ref().and_then(|l| l.loader.as_ref()
                    .map(|loader| loader.blocking_lock())
                );
                let context = || format!("copying {}/{attr}", source);
                let asset = loader.as_mut().map(|loader| loader.load_asset_dyn(
                        attr
                    ).with_context(context));
                match asset {
                    Some(Ok(mut asset)) => {
                        let new_asset = rt::log::error_ok(PackEditEnv::setup_asset(root_dir, &mut asset, attr));
                        if let Some(new_asset) = &new_asset {
                            self.tex_selected.clear();
                            self.tex_selected.push_str(&new_asset[..]);
                        }
                        new_asset
                    },
                    Some(Err(e)) => {
                        log::error!("{e:#}");
                        None
                    },
                    None => None,
                }
            },
            _ => {
                log::warn!("TODO: copy texture file into {}", rt::relative_path(&root_dir).display());
                tex.and_then(|tex| tex.file_path.file_name())
                    .map(|fname| string_into(fname.to_string_lossy()))
            },
        };
        Some(attrkey.unwrap_or_else(|| string_into(&self.tex_selected[..])))
    }
    fn apply_poi_attrs(&mut self, root_dir: &Path, attrs: &mut MarkerAttributes) {
        if let Some(icon_file) = self.tex_attr(root_dir) {
            attrs.poi_mut().icon_file = Some(icon_file);
        }
    }
    fn apply_trail_attrs(&mut self, root_dir: &Path, attrs: &mut MarkerAttributes) {
        if let Some(texture) = self.tex_attr(root_dir) {
            attrs.trail_mut().texture = Some(texture);
        } else {
            attrs.trail_mut().texture = Some(string_into("taimi"));
        }
    }
    fn setup_asset<R: io::Read>(root_dir: &Path, asset: &mut R, sourcename: &str) -> anyhow::Result<AttrString> {
        let mut outpath = root_dir.join(PackEdit::CONTENT_DIR_MARKERS);
        let _ = rt::log::warn_ok(fs::create_dir_all(&outpath));
        outpath.push(Path::new(sourcename).file_name().unwrap_or(sourcename.as_ref()));
        let outattr = || {
            let path = Path::new(&outpath).relative_to(root_dir);
            rt::log::error_ok(path)
                .map(|p| Cow::Owned(p.as_str().into()))
                .unwrap_or_else(|| outpath.to_string_lossy())
        };
        let mut out = match fs::File::create_new(&outpath) {
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                log::warn!("reusing {sourcename}: {e:#}");
                return Ok(string_into(outattr()))
            },
            out => out?,
        };
        io::copy(asset, &mut out)
            .map_err(Into::into)
            .map(|_| outattr())
        .map(string_into)
    }
}
#[derive(Debug)]
pub struct MarkerTexture {
    pub source: MarkerId,
    pub source_attr: Option<AttrString>,
    pub slot: TextureSlot,
    pub key: TextureKey,
    pub file_path: PathBuf,
}
impl MarkerTexture {
    pub fn empty() -> Self {
        Self {
            source: Self::empty_source(MarkerIndex::NS_UNK),
            source_attr: None,
            slot: TextureSlot::Reserved,
            key: Default::default(),
            file_path: Default::default(),
        }
    }
    pub fn marker_ns(&self) -> u32 {
        self.source.get_marker_index().namespace()
    }
    pub fn empty_source(ns: u32) -> MarkerId {
        #[cfg(todo = "unnecessary")]
        let max = match ns {
            MarkerIndex::NS_TRAIL => TrailIndex::MAX,
            MarkerIndex::NS_POI => PoiIndex::MAX,
            _ => u32::MAX,
        };
        let idx = MarkerIndex::new_invalid(ns);
        let path: MarkerPath = MarkerPath::with_path(idx);
        MarkerId::for_marker(path)
    }
}
impl Default for MarkerTexture {
    fn default() -> Self {
        Self::empty()
    }
}

enum TrlBoop {
    Append,
    Replace,
    Snip,
    Refresh,
}
pub struct DrawPe<'a, 's, 'ui> {
    pub ui: &'a Ui<'ui>,
    pub state: &'s mut PackEdit,
    pub packs: &'a IndexedList<PackRegistryNs, PackIndex, [PackElement]>,
    pub act_pack_alloc: Option<PackPath>,
    pub act_new_cat: Option<Category>,
    pub act_new_poi: Option<()>,
    pub act_new_trail: Option<()>,
    pub act_trl_clear: Option<(Result<TrlPath, TrailPath>, i32)>,
    pub act_trl_boop: Option<TrlBoop>,
}
impl<'a, 's, 'ui> DrawPe<'a, 's, 'ui> {
    pub fn new(ui: &'a Ui<'ui>,
        state: &'s mut PackEdit,
        packs: &'a IndexedList<PackRegistryNs, PackIndex, [PackElement]>,
    ) -> Self {
        Self {
            ui,
            state,
            packs,
            act_pack_alloc: None,
            act_new_cat: None,
            act_new_poi: None,
            act_new_trail: None,
            act_trl_clear: None,
            act_trl_boop: None,
        }
    }
    pub fn draw(&mut self) {
        if let Ok(ml) = rt::mumble_link_ptr() {
            self.state.env.latest_map = MapIndex::new(ml.read_map_id());
        }
        if self.state.pack_path.path == PackIndex::MAX {
            self.draw_alloc();
            return
        }
        if self.ui.button("sim/unload") {
            self.state.unload(Some(UnloadedReason::Disabled));
        }
        self.ui.same_line();
        if self.ui.button("broadcast info") {
            self.state.update_info();
        }
        self.ui.same_line();
        if self.ui.button("broadcast gameplay") {
            self.state.update_gameplay();
        }
        self.ui.same_line();
        if self.ui.button("broadcast loaded") {
            self.state.update_loaded();
        }

        {
            let _id = self.ui.push_id("pecat");
            let is_new_cat = self.state.env.pack_cat_id.is_empty() || !self.state.env.pack_cat_id.ends_with(".");
            if is_new_cat {
                if self.ui.button("cat") && !self.state.env.pack_cat_id.is_empty() {
                    let mut cat = Category {
                        full_id: CategoryId::with_full_id(&self.state.env.pack_cat_id),
                        display_name: str_opt_ref(&self.state.env.pack_cat_name)
                            .map(|n| n.into()),
                        flags: Default::default(),
                        sub_categories: Default::default(),
                        marker_attributes: Default::default(),
                    };
                    self.state.env.apply_attrs(&mut cat.marker_attributes);
                    self.act_new_cat = Some(cat);
                    self.state.env.pack_cat_id.push_str(".");
                }
            } else {
                let id = str_opt_ref(self.state.env.pack_cat_id[..].strip_suffix(".").unwrap_or(&self.state.env.pack_cat_id)).map(FullIdRef::from_str);
                if let Some(id) = id {
                    let mut wants_id = false;
                    let cat_info = self.state.pack.pack_info.categories.info_of(self.state.cursor.category_path);
                    let sibling = cat_info.and_then(|i|
                        i.sibling()
                        .or_else(|| {
                            let itself = self.state.cursor.category_path.path;
                            let sib = i.parent().and_then(|p| self.state.pack.pack_info.categories.firstborn_of(CategoryPath::with_path(p))).map(|s| s.path);
                            match sib {
                                Some(sib) if sib == itself => None,
                                sib => sib,
                            }
                        })
                    );
                    let mut buttoned = false;
                    if let Some(sibling) = sibling {
                        if self.ui.button("cycle") {
                            self.state.cursor.category_path.path = sibling;
                            wants_id = true;
                        }
                        buttoned = true;
                    }
                    if let Some(firstborn) = cat_info.and_then(|i| i.child()) {
                        if buttoned {
                            self.ui.same_line();
                        }
                        if self.ui.button("next") {
                            self.state.cursor.category_path.path = firstborn;
                            wants_id = true;
                        }
                    }
                    if id.id_is_root() {
                        self.ui.text_disabled("root");
                    } else if self.ui.button("back") {
                        let mut trunc_parent_len = usize::MAX;
                        if let Some(parent_id) = id.parent() {
                            trunc_parent_len = parent_id.as_str().len() + 1;
                            if let Some(parent) = cat_info.and_then(|i| i.parent()) {
                                self.state.cursor.category_path.path = parent;
                            } else if let Some(idx) = self.state.pack.pack.categories.all_categories.get_index_of(parent_id) {
                                self.state.cursor.category_path.path = idx as CategoryIndex;
                            }
                        }
                        if trunc_parent_len < self.state.env.pack_cat_id.len() {
                            self.state.env.pack_cat_id.truncate(trunc_parent_len);
                        }
                        self.state.env.pack_cat_name.clear();
                    }
                    if wants_id {
                        self.state.env.pack_cat_name.clear();
                        self.state.env.pack_cat_id.clear();
                        if let Some((_id, cat)) = self.state.pack.pack.categories.all_categories.get_index(self.state.cursor.category_path.path as usize) {
                            self.state.env.pack_cat_id.push_str(cat.full_id.as_str());
                        }
                    }
                }
            }
            self.ui.same_line();
            let width = self.ui.content_region_avail()[0] * 0.45;
            self.ui.set_next_item_width(width);
            self.ui.input_text("##catid", &mut self.state.env.pack_cat_id).hint("id")
                .chars_noblank(true)
                .build();
            if is_new_cat {
                self.ui.same_line();
                self.ui.set_next_item_width(width);
                self.ui.input_text("##catname", &mut self.state.env.pack_cat_name).hint("name")
                    .build();
            }
        }
        let mpath = self.state.cursor.id;
        let mns = mpath.path.namespace();
        if let MarkerIndex::NS_UNK | MarkerIndex::NS_POI | MarkerIndex::NS_TRAIL = mns {
            let _id = self.ui.push_id("markerpos");
            self.state.env.pos.draw_display(self.ui, false);
            self.state.env.pos.draw_take_current(self.ui);
            self.state.env.pos.draw_edit_manual(self.ui, false);

            let colourid = "markercolour";
            if let Some(_token) = self.ui.begin_popup(colourid) {
                let mut colour = self.state.env.colour.to_array();
                if imgui::ColorPicker::new("##tint", &mut colour).build(self.ui) {
                    self.state.env.colour = colour.into();
                }
                if self.ui.button("ok") {
                    self.ui.close_current_popup();
                }
            }
            self.ui.same_line();
            if self.ui.button("colour") {
                self.ui.open_popup(colourid);
            }
        }
        if let MarkerIndex::NS_UNK | MarkerIndex::NS_POI = mns {
            let poi_path: Option<PoiPath> = mpath.try_to();
            let _id = self.ui.push_id("pepoi");
            let tex_selected = &self.state.env.tex_selected[..];
            if !tex_selected.is_empty() | true {
                let label = if let Some(poi_path) = poi_path {
                    self.ui.display_with_font(&(), &format_args!("poi#{}", poi_path.path));
                    "save"
                } else {
                    "poipoi"
                };
                if self.ui.button(label) {
                    self.act_new_poi = Some(());
                }
                self.ui.same_line();
            }
            let width = self.ui.content_region_avail()[0] * 0.8;
            self.ui.set_next_item_width(width);
            let texs = self.ui.begin_combo("##texs", str_opt_ref(tex_selected).unwrap_or("img"));
            if let Some(_token) = texs {
                let width = self.ui.content_region_avail()[0];
                self.ui.set_next_item_width(width);
                let selected = self.state.env.avail_textures.get(tex_selected)
                    .map(|tex| tex.key.clone());
                self.ui.set_item_default_focus();
                let manual_entry = self.ui.input_text("##tex", &mut self.state.env.tex_selected)
                    .auto_select_all(true)
                    .enter_returns_true(true)
                    .hint("image path")
                    .build();
                if manual_entry {
                    self.ui.close_current_popup();
                }
                let mut selection = None;
                let mut selection_source = None;
                for (key, tex) in self.state.env.avail_textures.iter() {
                    let selected = match &selected {
                        Some(sel) if &sel[..] == &key[..] => true,
                        _ => false,
                    };
                    let display = (!tex.file_path.as_os_str().is_empty()).then(||
                        rt::relative_path(&tex.file_path).to_string_lossy()
                    ).or_else(||
                        tex.source_attr.as_ref().and_then(|s| str_opt(&s[..]))
                            .map(Cow::Borrowed)
                    ).unwrap_or(Cow::Borrowed(&key[..]));
                    if Selectable::new(&display).selected(selected).build(self.ui) {
                        selection = Some(&key[..]);
                        selection_source = Some(tex.source);
                    } else if self.ui.is_item_hovered() {
                        let source_pack = match tex.source.marker_path::<PackPath>() {
                            None => None,
                            Some(p) if p.root == self.state.pack_path =>
                                Some(self.state.root_dir.to_string_lossy()),
                            Some(pack_path) =>
                                self.state.env.shared.as_ref().and_then(|p| p.packs.packs.borrow()
                                    .lookup_ref(&pack_path.root)
                                    .map(|p| Cow::Owned(p.info.to_string()))),
                        };
                        if let Some(source_pack) = source_pack {
                            self.ui.tooltip_text(&source_pack);
                        }
                    }
                }
                if Selectable::new("fallback").build(self.ui) {
                    selection = Some("taimi");
                }
                if let Some(selection) = selection {
                    self.state.env.tex_selected.clear();
                    self.state.env.tex_selected.push_str(selection);
                    self.state.env.tex_selected_source = selection_source;
                }
            }
            self.ui.same_line();
            if self.ui.button("refresh") {
                self.state.refresh_textures();
            }
        }
        if let MarkerIndex::NS_UNK | MarkerIndex::NS_TRAIL = mns {
            if mns == MarkerIndex::NS_UNK {
                self.ui.separator();
            }
            let section_path = mpath.try_to::<SectionOfTrail>();
            let trail_path: Option<TrailPath> = mpath.try_to();
            let trail_root = trail_path.or(section_path.map(|p| p.root));
            let _id = self.ui.push_id("petrail");
            if let Some(_section_path) = section_path {
                if self.ui.button("refresh") {
                    self.act_trl_boop = Some(TrlBoop::Refresh);
                }
                self.ui.same_line();
                if self.ui.button("boop") {
                    self.act_trl_boop = Some(TrlBoop::Append);
                }
                self.ui.same_line();
                if self.ui.button("reboop") {
                    self.act_trl_boop = Some(TrlBoop::Replace);
                }
                self.ui.same_line();
                if self.ui.button("snip") {
                    self.act_trl_boop = Some(TrlBoop::Snip);
                }
                if self.ui.button("end") {
                    self.state.cursor.id.path = MarkerIndex::UNK;
                }
            } else {
                let label = "trailing";
                let can_commit = !self.state.env.trl_name.is_empty() || trail_path.is_some();
                let mut commit = match can_commit {
                    true => self.ui.button("trailing"),
                    false => {
                        self.ui.text_disabled(label);
                        false
                    },
                };
                self.ui.same_line();
                let width = self.ui.content_region_avail()[0] * 0.95;
                self.ui.set_next_item_width(width);
                commit |= self.ui.input_text("##trl", &mut self.state.env.trl_name).hint("trl")
                    .chars_noblank(true)
                    .enter_returns_true(true)
                    .build();
                if commit & can_commit {
                    self.act_new_trail = Some(());
                }
            }
            if let (Some(trail_path), Some(map_id)) = (trail_root, self.state.env.latest_map) {
                if self.ui.button("clear TRL") {
                    self.act_trl_clear = Some((Err(trail_path), map_id.get() as i32));
                } else if self.ui.is_item_hovered() {
                    self.ui.tooltip_text("CLICKING HERE WILL PROBABLY NUKE A TRL FILE FROM THE PACK");
                }
            }
        }
        self.ui.input_text("##tip", &mut self.state.env.tip_name).hint("tip")
            .build();
    }
    pub fn draw_alloc(&mut self) {
        let _id = self.ui.push_id("pe-alloc");
        let mut commit = self.ui.button("allocate new");
        self.ui.same_line();
        commit |= self.ui.input_text("##dir", &mut self.state.env.pack_alloc_name).hint("dir")
            .chars_noblank(true)
            .enter_returns_true(true)
            .build();
        if commit {
            if self.state.env.pack_alloc_name.is_empty() {
                self.state.env.pack_alloc_name.push_str("packedit");
            }
            self.act_pack_alloc = Some(PackPath::with_path(PackIndex::MAX));
        } else if !self.state.env.pack_alloc_name.is_empty() {
            let width = self.ui.content_region_avail()[0] * 0.45;
            self.ui.set_next_item_width(width);
            self.ui.input_text("##packid", &mut self.state.env.pack_cat_id).hint("id")
                .chars_noblank(true)
                .build();
            self.ui.same_line();
            self.ui.set_next_item_width(width);
            self.ui.input_text("##catname", &mut self.state.env.pack_cat_name).hint("name")
                .build();
        }
        self.draw_alloc_open();
    }
    pub fn draw_alloc_open(&mut self) {
        let packs = TreeNode::new("otherpacks")
            .label::<&str, _>("open")
            .leaf(false)
            .framed(true)
            .tree_push_on_open(false)
            .push(self.ui);
        if let Some(packs) = packs {
            self.ui.indent();
            for pd in self.packs.values() {
                let _packid = self.ui.push_id(pd.state.ui_id());
                self.ui.display_with_font(&NexusLinkFont::Ui, &pd.state.info);
                self.ui.same_line();
                if self.ui.button("commandeer") {
                    self.act_pack_alloc = Some(pd.state.pack_path());
                }
            }
            self.ui.unindent();
        }
    }
    pub fn post_draw(&mut self) {
        if let Some(path) = self.act_pack_alloc.take() {
            match self.packs.lookup_ref(&path).map(|pd| pd.state.info.clone()) {
                Some(info) => self.pack_open(info),
                None => self.pack_alloc(),
            }
            self.state.post_open();
        }
        if let Some(()) = self.act_new_poi.take() {
            let poi_path: Option<PoiPath> = self.state.cursor.id.try_to();
            self.state.cursor.id.path = MarkerIndex::UNK;
            let map_id = self.state.env.latest_map.map(|m| m.get() as i32);
            let mut poi = None;
            let mut map_id = map_id;
            if poi_path.is_none() && self.state.env.pos.position.is_none() {
                self.state.env.pos.fill_current();
            }
            let position = match self.state.env.pos.position.take() {
                #[cfg(todo)]
                None => self.state.env.latest_pos.map(Point3::to_untyped),
                pos => pos.map(Point3::from_raw),
            }.unwrap_or(Point3::<f32>::ZERO);
            {
                let poi = if let Some(poi) = poi_path.and_then(|p| self.state.pack.pack.pois.get_mut(p.path as usize)) {
                    poi.position = position;
                    map_id = None;
                    poi
                } else {
                    poi.insert(Poi {
                        category: IdNameBox::new_cloned(self.state.pack.pack.categories.all_categories.get_index(self.state.cursor.category_path.path as usize)
                            .map(|(_, c)| &c.full_id)
                            .or(self.state.pack.pack.categories.root_categories.get_index(0))
                            .cloned()
                            .unwrap_or_else(|| self.state.env.fallback_category_id())),
                        guid: Uuid::new_v4(),
                        map_id: map_id.unwrap_or(0),
                        position,
                        attributes: MarkerAttributes::default(),
                        parent_path: None,
                    })
                };
                self.state.env.apply_attrs(&mut poi.attributes);
                self.state.env.apply_render_attrs(&mut poi.attributes);
                self.state.env.apply_poi_attrs(&self.state.root_dir, &mut poi.attributes);
            }
            let added = poi.is_some();
            if let Some(poi) = poi {
                self.state.pack.pack.pois.push(poi);
            }
            if let Some(map_id) = map_id.and_then(|map_id| MapIndex::new(map_id as _)) {
                self.state.pack.pack_info.maps.insert(map_id);
            }
            self.state.markers_dirty = true;
            if added {
                self.state.populate_info();
                self.state.update_info();
            }
            self.state.update_loaded();
            self.state.update_gameplay();
        }
        if let Some(()) = self.act_new_trail.take() {
            let trail_path: Option<TrailPath> = self.state.cursor.id.try_to();
            self.state.cursor.id.path = MarkerIndex::UNK;
            let mut map_id = self.state.env.latest_map.map(|m| m.get() as i32);
            let mut trail = None;
            {
                let trl = str_opt_ref(&self.state.env.trl_name)
                    .map(|p| TrlPath::new(string_into(p)));
                    let existing = trail_path.and_then(|p| self.state.pack.pack.trails.get_mut(p.path as usize)
                        .map(|trail| (p, trail)));
                let mut open_sec = None;
                let trail = if let Some((trail_path, trail)) = existing {
                    self.state.cursor.id.path = trail_path.into();
                    if let Some(trl) = trl {
                        trail.trail_path = Some(trl);
                    } else if let Some(trl) = &trail.trail_path {
                        open_sec = Some((trail_path, trl.clone()));
                    }
                    map_id = None;
                    trail
                } else {
                    let trail_path: SectionOfTrail = SectionOfTrail::with_parts(TrailPath::with_path(self.state.pack.pack.trails.len() as TrailIndex), TrailSectionPath::with_path(0 as TrailSectionIndex));
                    self.state.cursor.id.path = trail_path.map_path(|p| p.path).into();
                    trail.insert(Trail {
                        category: IdNameBox::new_cloned(self.state.pack.pack.categories.all_categories.get_index(self.state.cursor.category_path.path as usize)
                            .map(|(_, c)| &c.full_id)
                            .or(self.state.pack.pack.categories.root_categories.get_index(0))
                            .cloned()
                            .unwrap_or_else(|| self.state.env.fallback_category_id())),
                        guid: Uuid::new_v4(),
                        map_id,
                        attributes: MarkerAttributes::default(),
                        parent_path: None,
                        trail_path: trl,
                    })
                };
                self.state.env.apply_attrs(&mut trail.attributes);
                self.state.env.apply_render_attrs(&mut trail.attributes);
                self.state.env.apply_trail_attrs(&self.state.root_dir, &mut trail.attributes);
                if let (Some(trl), Some(map_id)) = (trail.trail_path.clone(), map_id) {
                    let trail_exists = self.state.resolve_trl(Ok(&trl))
                        .and_then(|p| p.try_exists().context("trl exists"));
                    if rt::log::warn_ok(trail_exists) == Some(false) {
                        self.act_trl_clear = Some((Ok(trl), map_id));
                    }
                }
                if let Some((trail_path, trl)) = open_sec {
                    // TODO: determine from map state instead?
                    let section_count = self.state.asset_file_path(&trl.path[..])
                        .context("trl sec count")
                        .and_then(|path| fs::File::open(path)
                            .map_err(anyhow::Error::from)
                        ).and_then(|mut f| match TrailData::read_from_trl(&mut f) {
                            Err(e) /*if e.kind() == io::ErrorKind::UnexpectedEof*/ => Ok(Default::default()),
                            res => res,
                        })
                        .map(|trl| trl.sections.len());
                    if let Some(count) = rt::log::info_ok(section_count) {
                        let sec_path: SectionOfTrail = trail_path.rel(TrailSectionPath::with_path(count.saturating_sub(1) as TrailIndex));
                        self.state.cursor.id.path = sec_path.map_path(|p| p.path).into();
                    }
                }
            }
            if let Some(trail) = trail {
                self.state.pack.pack.trails.push(trail);
            }
            if let Some(map_id) = map_id.and_then(|map_id| MapIndex::new(map_id as _)) {
                self.state.pack.pack_info.maps.insert(map_id);
            }
        }
        if let Some((trl, map_id)) = self.act_trl_clear.take() {
            let trl_storage;
            let trl = match trl {
                Ok(trl) => {
                    trl_storage = trl;
                    Ok(&trl_storage)
                },
                Err(e) => Err(e),
            };
            let written = self.state.write_to_trl(trl, true, Some(map_id));
            let _ = rt::log::error_ok(written);
        }
        while let Some(boop) = self.act_trl_boop.take() {
            let map_id = self.state.env.latest_map.map(|m| m.get() as i32);
            let trail_path = match self.state.cursor.id.path.namespace() {
                MarkerIndex::NS_TRAIL => Some(self.state.cursor.id.path.index_trail_section_unchecked()),
                _ => None,
            }.context("trl cursor invalid");
            match boop {
                TrlBoop::Refresh => {
                    self.state.markers_dirty = true;
                    if true {
                        self.state.populate_info();
                        self.state.update_info();
                    }
                    self.state.update_loaded();
                    self.state.update_gameplay();
                    let trail_path: anyhow::Result<TrailPath> = trail_path.map(|(traili, _seci)|
                        TrailPath::with_path(traili)
                    );
                    let res = trail_path.and_then(|trail_path| match map_id.and_then(|id| MapIndex::new(id as _)) {
                        Some(map_id) => {
                            let map_path = self.state.pack_path.rel(map_id);
                            let trail_geo = self.state.env.shared.as_ref().map(|shared| TrailGeometryRequests::subscribed_to(&shared.space.trail_geometry));
                            trail_geo.and_then(|geo| geo.request(map_path.rel(trail_path.path), false).ok())
                                .context("trl refresh")
                        },
                        map_id => map_id.context("map required for refresh").map(drop),
                    });
                    rt::log::error_ok(res);
                },
                boop => {
                    let trl = trail_path.and_then(|(traili, seci)| {
                        let trail_path = TrailPath::with_path(traili);
                        self.state.write_to_trl(Err(trail_path), false, map_id)
                        .map(|trl| (trl, trail_path.rel(seci)))
                    });
                    let trl = rt::log::error_ok(trl);
                    let next_point = match boop {
                        TrlBoop::Snip => Some(None),
                        TrlBoop::Replace | TrlBoop::Append => {
                            self.state.env.pos.fill_current();
                            match self.state.env.pos.position.take() {
                                #[cfg(todo)]
                                None => self.state.env.latest_pos.map(Point3::to_untyped),
                                pos => pos.map(Point3::from_raw),
                            }.map(|pos| Some(pos))
                        },
                        TrlBoop::Refresh => None,
                    };
                    if let Some((mut trl, trail_path)) = trl {
                        if let Some(next_point) = next_point {
                            let data = TrailSection::encode_record(next_point);
                            let res = if let TrlBoop::Replace = boop {
                                match trl.seek(io::SeekFrom::Current(-(TrailSection::POINT_SIZE as i64))) {
                                    Ok(pos) if pos < TrailHeader::SIZE as u64 => {
                                        log::warn!("you went too far, consider clearing!");
                                        Ok(())
                                    },
                                    res => res.map(drop),
                                }
                            } else { Ok(()) };
                            let res = res.and_then(|()| trl.write_all(&data));
                            if rt::log::error_ok(res).is_some() {
                                // see results immediately
                                self.act_trl_boop = Some(TrlBoop::Refresh);
                            }
                        }
                    }
                },
            }
        }
        if let Some(cat) = self.act_new_cat.take() {
            let parent = cat.full_id.parent().and_then(|pid| self.state.pack.pack.categories.all_categories.get_index_of(pid));
            let pidx = match parent {
                Some(pidx) => {
                    if let Some((_, parent)) = self.state.pack.pack.categories.all_categories.get_index_mut(pidx) {
                        parent.append_children(iter::once(cat.full_id.clone()));
                    }
                    Some(pidx)
                },
                None => {
                    self.state.pack.pack.categories.root_categories.insert(cat.full_id.clone());
                    None
                },
            };
            let newidx = self.state.pack.pack.categories.all_categories.len() as CategoryIndex;
            self.state.cursor.category_path.path = newidx;
            self.state.pack.pack.categories.all_categories.insert(cat.full_id.clone(), cat);
            self.state.cats_dirty = true;
            log::debug!("TODO: update cat#{pidx:?} and whatnot");
            self.state.populate_info();
            self.state.update_info();
            self.state.update_loaded();
            self.state.update_gameplay();
        }
    }
    pub fn pack_open(&mut self, path: Arc<SharedPackInfo>) {
        self.state.pack_path = path.index;
        PathingEvent::PackLock { path: path.index }.try_send();
    }
    pub fn pack_alloc(&mut self) {
        if let Err(()) = self.state.pack_alloc() { return }
        self.state.init_pack(self.packs.end_path());
        let mut info = SharedPackInfo::new_unloaded(self.state.pack_path, self.state.root_dir.clone().into(), None);
        info.info = Some(Arc::new(self.state.pack.pack_info.clone()));
        info.sig = self.state.info_sig;
        let pack = SharedPackLoad::new_preload(Arc::new(info));
        let Some(shared) = &self.state.env.shared else { return };
        let Some(realpath) = shared.packs.update_packs_extend(&mut iter::once(pack)).next() else { return };
        if realpath != self.state.pack_path {
            log::error!("I don't like this number");
        }
        self.state.pack_path = realpath;
        shared.packs.packs.send_modify(|packs| {
            let Some(pack) = packs.lookup_mut(&self.state.pack_path) else { return };
            self.state.loaded.resubscribe_to(&pack.loaded);
            self.state.config.resubscribe_to(&pack.config);
        });
        self.state.update_loaded();
        self.state.update_gameplay();
    }
}
impl super::PackElements {
    pub fn draw_dynamic(&mut self, ui: &Ui) {
        let mut draw = DrawPe::new(ui, &mut self.pack_edit, self.pack_state.map_ref_as_slice());
        draw.draw();
        draw.post_draw();
    }
}
