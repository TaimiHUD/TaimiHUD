use {
    crate::{
        controller::pathing::{
            registry::{
                LoadedMarkerPath,
                LoadedTrailPath,
                PackActivateLoaded,
                PackBoxOf,
                PackCategoryInfo,
                PackIndex,
                PackInfo,
                PackInfoSignature,
                PackMapPath,
                PackPath,
                PackRoot,
                SharedLoaderBox,
                UnloadedReason,
            },
            shared::{LoadedTrailGeometry, TrailGeometrySections},
            PackConfig,
        },
        exports::runtime::{self as rt, textures::TextureKey},
        settings::source::sources::DataSourcePath,
    },
    rustc_hash::FxHashMap,
    std::{
        collections::{btree_map, BTreeMap, BTreeSet},
        fmt,
        mem,
        ops,
        path::Path,
        sync::{atomic::{AtomicUsize, AtomicU32, AtomicU16, Ordering}, Arc, RwLock},
    },
    taimi_hoard::{
        cmp::CmpIgnore,
        iters::IterExt as _,
        loc::{LocationMut, Locator},
    },
    taimi_meta::packs::{MarkerIndex, MapIndex, PoiIndex, PoiPath, TrailIndex, TrailPath, CategoryPath, CategoryIndex, MarkerPath},
    taimi_pack::{attributes::AttrString, Pack},
    taimi_sync::{
        arcs::ArcPtrCmp,
        watched::{watch, Watcher},
    },
};

pub type SharedLoaderPacksInfo = PackBoxOf<SharedPackLoad>;
/// TODO: maybe split this up into sender and receiver halves...
#[derive(Debug)]
pub struct SharedPacks {
    pub packs: watch::Sender<SharedLoaderPacksInfo>,
    /// loading grace period
    pub load_period: SharedGracePeriod,
}

impl SharedPacks {
    pub fn new() -> Self {
        Self {
            packs: Default::default(),
            load_period: Default::default(),
        }
    }

    pub(crate) fn update_packs_extend(
        &self,
        packs: &mut dyn Iterator<Item = SharedPackLoad>,
    ) -> impl Iterator<Item = PackPath> {
        #[cfg(todo = "unnecessary")]
        let mut appended: PackSet = Default::default();
        let invalid = PackIndex::MAX;
        let mut appended = invalid..invalid;
        let (_min, max) = packs.size_hint();
        if max == Some(0) {
            // empty iterator, don't bother
            return appended.lazy_map(PackPath::with_path)
        }
        self.packs.send_if_modified(|shared| {
            appended.start = shared.end_path().path;
            appended.end = appended.start;
            let next_path = &mut appended.end;
            let packs = packs.map(|mut pack| {
                pack.ensure_index(PackPath::with_path(*next_path));
                *next_path += 1;
                pack
            });
            let prev = match shared.data {
                #[cfg(todo = "unnecessary")]
                ref data => data.iter().cloned(),
                // *shrug* I guess an empty box allocation is better than cloning some arcs
                ref mut data => Box::into_iter(mem::take(data)),
            };
            let updated = prev.chain(packs).collect::<Box<[_]>>();
            shared.data = updated;
            !appended.is_empty()
        });
        appended.lazy_map(PackPath::with_path)
    }
    pub(crate) fn update_packs_loaded(
        &self,
        loaded: &mut dyn Iterator<Item = (PackPath, Result<PackActivateLoaded, Option<UnloadedReason>>)>,
    ) {
        self.packs.send_if_modified(|shared| {
            let mut changed = false;
            for (path, loaded) in loaded {
                let Some(pack) = shared.lookup_mut(&path) else {
                    log::warn!("nonexistent pack update for {path}?");
                    continue
                };
                match &loaded {
                    Err(Some(reason @ (
                        UnloadedReason::UnknownFormat
                        | UnloadedReason::LoadingFailed(..)
                    ))) => {
                        log::error!("failed to load {}: {reason}", pack.info);
                    },
                    Err(None) => {
                        log::debug!("offloaded {}", pack.info);
                    },
                    #[cfg(taimi_debug)]
                    Err(Some(reason)) => {
                        log::debug!("marked {path}: {reason}");
                    },
                    #[cfg(taimi_debug)]
                    Ok(..) => {
                        log::debug!("marked {path}: loaded");
                    },
                    #[cfg(not(taimi_debug))]
                    _ => (),
                }

                changed |= match loaded {
                    Ok(loaded) => pack.set_loaded(loaded),
                    Err(reason) => pack.set_unloaded(reason),
                };
            }
            changed
        });
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SharedPackInfo {
    pub index: PackPath,
    pub path: Arc<Path>,
    pub info: Option<Arc<PackInfo>>,
    pub datasource: Option<DataSourcePath>,
    pub sig: PackInfoSignature,
    /// TODO: include in hashes?
    pub dynamics: ArcPtrCmp<SharedPackDynamics>,
    allocated_keys: ArcPtrCmp<RwLock<FxHashMap<AttrString, Arc<str>>>>,
}
impl SharedPackInfo {
    pub fn new_unloaded(index: PackPath, path: Arc<Path>, datasource: Option<DataSourcePath>) -> Self {
        Self {
            index,
            path,
            datasource,
            info: None,
            sig: PackInfoSignature::EMPTY,
            allocated_keys: Default::default(),
            dynamics: Default::default(),
        }
    }
    pub fn empty(index: Option<PackPath>) -> Self {
        Self {
            index: index.unwrap_or(PackPath::with_path(PackIndex::MAX)),
            path: Path::new("").into(),
            info: None,
            datasource: None,
            sig: PackInfoSignature::EMPTY,
            allocated_keys: Default::default(),
            dynamics: Default::default(),
        }
    }

    pub fn info(&self) -> Option<(PackPath, &Arc<PackInfo>, PackInfoSignature)> {
        self.info.as_ref().map(|i| (self.index, i, self.sig))
    }
    pub fn category_info(&self) -> Option<(&Arc<PackCategoryInfo>, &Arc<PackInfo>)> {
        self.info.as_ref().map(|i| (&i.categories, i))
    }
    pub fn primary_root(&self) -> Option<&PackRoot> {
        self.info.as_ref().and_then(|i| i.primary_root())
    }
    pub fn unique_root(&self) -> Option<&PackRoot> {
        let mut roots = self.info.as_ref()?.roots.iter().filter(|r| !r.flags.is_hidden());
        let root = roots.next()?;
        if roots.next().is_none() {
            // as long as no additional visible roots were found...
            Some(root)
        } else {
            None
        }
    }

    /// check [self.info] manually if `None` matters
    pub fn has_map(&self, map_id: MapIndex) -> bool {
        self.info
            .as_ref()
            .map(|i| i.maps.contains(map_id))
            .unwrap_or(false)
    }

    /// forcibly unload because it's expected to change on the filesystem for
    /// some reason?
    pub fn unload_info(&mut self) -> Option<PackInfoSignature> {
        self.info = None;
        mem::replace(&mut self.sig, PackInfoSignature::EMPTY).get()
    }
    pub fn set_info(&mut self, info: Arc<PackInfo>) -> Option<PackInfoSignature> {
        let sig = PackInfoSignature::from_info(&info);
        self.info = Some(info);
        let sig_prev = mem::replace(&mut self.sig, sig);
        (sig != sig_prev).then_some(sig)
    }

    pub fn is_dead(&self) -> bool {
        self.index.path == PackIndex::MAX
    }
    pub fn kill(&mut self) {
        self.index.path = PackIndex::MAX;
        self.sig = PackInfoSignature::EMPTY;
        let _ = self.info.take();
        let _ = self.datasource.take();
    }

    pub fn path_name(&self) -> &Path {
        let fallback = || {
            let relpath = rt::relative_path(&self.path);
            relpath.strip_prefix("addons/Taimi/pathing/")
            .unwrap_or(relpath)
        };
        let addon_dir = rt::ADDON_DIR.try_read().ok().and_then(|d| *d);
        addon_dir
            .and_then(|d|
                self.path.strip_prefix(d).ok()
                .map(|p| p.strip_prefix("pathing/").unwrap_or(p))
            ).unwrap_or_else(fallback)
    }

    /// unique texture names
    pub fn key_for_subresource(&self, resource: &AttrString) -> Arc<str> {
        if let Ok(keys) = self.allocated_keys.try_read() {
            if let Some(v) = keys.get(resource) {
                return v.clone()
            }
        }

        let Ok(mut keys) = self.allocated_keys.write() else {
            log::error!("poisoned");
            return Arc::from(self.gen_key_for_subresource(resource))
        };
        keys.entry(resource.clone())
            .or_insert_with(|| Arc::from(self.gen_key_for_subresource(resource)))
            .clone()
    }
    pub fn gen_key_for_subresource(&self, resource: &AttrString) -> String {
        let packname = self.path_name();
        let resource = &resource[..];
        let storage;
        let resourceid = match resource.len() {
            0..=24 => resource,
            _toolong => {
                use {
                    rustc_hash::FxHasher,
                    std::hash::{Hash, Hasher},
                };
                let mut hasher = FxHasher::with_seed(self.index.path as usize);
                resource.hash(&mut hasher);
                storage = format!("{:x}", hasher.finish());
                &storage
            },
        };
        format!("{}_{resourceid}", packname.display())
    }
    pub fn drain_subresource_keys(&self) -> impl IntoIterator<Item = (AttrString, Arc<str>)> + 'static {
        let mut keys = self.allocated_keys.write().unwrap_or_else(|e| e.into_inner());
        keys.drain().collect::<Vec<_>>()
    }
    pub fn iter_subresource_keys(&self, drain: bool) -> impl IntoIterator<Item = (AttrString, Arc<str>)> + 'static {
        let mut keys = self.allocated_keys.write().unwrap_or_else(|e| e.into_inner());
        match drain {
            true => keys.drain().collect::<Vec<_>>(),
            false => keys.iter().map(|(k, v)| (k.clone(), v.clone())).collect::<Vec<_>>(),
        }
    }
    pub(crate) fn shared_subresources(&self) -> &Arc<RwLock<FxHashMap<AttrString, Arc<str>>>> {
        &self.allocated_keys
    }
}
impl Default for SharedPackInfo {
    fn default() -> Self {
        Self::empty(None)
    }
}
impl fmt::Display for SharedPackInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(display_name) = self.primary_root().and_then(|r| r.display_name.as_ref()) {
            f.write_str(&display_name[..])
        } else if let Some(datasource) = &self.datasource {
            fmt::Display::fmt(&datasource.path, f)
        } else if !self.path.as_os_str().is_empty() {
            let path = self
                .path
                .file_name()
                .unwrap_or_else(|| rt::relative_path(&self.path).as_os_str());
            fmt::Display::fmt(&path.display(), f)
        } else {
            fmt::Display::fmt(&self.index, f)
        }
    }
}

#[derive(Debug, Default)]
pub struct SharedPackDynamics {
    pub paths: SharedPackAllocation,
    #[cfg(todo)]
    static_end_poi: PoiPath,
}
impl SharedPackDynamics {
    pub fn reset_for_pack(&self, pack: &Pack) {
        self.paths.reset_with_static_range(SharedPackAllocation::static_ranges_for(pack));
    }
    #[cfg(todo)]
    pub fn is_dynamic_path(&self, path: MarkerPath) -> Option<bool> {}
}
#[derive(Debug)]
pub struct SharedPackAllocation {
    #[cfg(todo)]
    pub masked: RwLock<MarkerSet>,
    pub masked: RwLock<BTreeSet<MarkerPath>>,
    next_index_poi: AtomicU32,
    next_index_trail: AtomicU16,
    next_index_category: AtomicU32,
}
/// dynamic allocations typically follow
pub type StaticMarkerRanges = (ops::RangeTo<PoiPath>, ops::RangeTo<TrailPath>, ops::RangeTo<CategoryPath>);
impl SharedPackAllocation {
    const ORDERING_CHECK: Ordering = Ordering::Relaxed;
    const ORDERING_RESERVE: Ordering = Ordering::SeqCst;
    #[inline]
    pub fn end_path_poi(&self) -> PoiPath {
        PoiPath::new_path(self.next_index_poi.load(Self::ORDERING_CHECK))
    }
    #[inline]
    pub fn end_path_trail(&self) -> TrailPath {
        TrailPath::new_path(self.next_index_trail.load(Self::ORDERING_CHECK))
    }
    #[inline]
    pub fn end_path_category(&self) -> CategoryPath {
        CategoryPath::new_path(self.next_index_category.load(Self::ORDERING_CHECK))
    }

    pub const INDEX_MAX_POI: PoiIndex = MarkerIndex::INDEX_MAX_POI as PoiIndex;
    pub const INDEX_MAX_TRAIL: TrailIndex = match MarkerIndex::INDEX_MAX_TRAIL {
        // expected to be 0xffff, so...
        m if m >= TrailIndex::MAX as u32 => m - 0x10,
        m => m,
    } as TrailIndex;
    pub const INDEX_MAX_CATEGORY: CategoryIndex = MarkerIndex::INDEX_MAX_CAT as CategoryIndex;
    pub fn reserve_poi(&self) -> PoiPath {
        let path = self.next_index_poi.fetch_add(1, Self::ORDERING_RESERVE);
        if path >= Self::INDEX_MAX_POI {
            log::warn!("exhausted dynamic pois");
            // racy but there's headroom and allocating over static paths will cause mayhem so try at least...
            self.next_index_poi.store(PoiIndex::MAX, Self::ORDERING_RESERVE);
            return PoiPath::new_path(PoiIndex::MAX)
        }
        PoiPath::new_path(path)
    }
    pub fn reserve_trail(&self) -> TrailPath {
        let path = self.next_index_trail.fetch_add(1, Self::ORDERING_RESERVE);
        if path >= Self::INDEX_MAX_TRAIL {
            log::warn!("exhausted dynamic trails");
            self.next_index_trail.store(TrailIndex::MAX, Self::ORDERING_RESERVE);
            return TrailPath::new_path(TrailIndex::MAX)
        }
        TrailPath::new_path(path)
    }
    pub fn reserve_category(&self) -> CategoryPath {
        let path = self.next_index_category.fetch_add(1, Self::ORDERING_RESERVE);
        if path >= Self::INDEX_MAX_CATEGORY {
            log::warn!("exhausted dynamic cats");
            self.next_index_category.store(Self::INDEX_MAX_CATEGORY, Self::ORDERING_RESERVE);
            return CategoryPath::new_path(CategoryIndex::MAX)
        }
        CategoryPath::new_path(path)
    }

    pub fn is_masked(&self, path: MarkerPath) -> bool {
        let Ok(m) = self.masked.write() else { return false };
        m.contains(&path)
    }
    pub fn mask_marker(&self, path: MarkerPath) {
        let Ok(mut m) = self.masked.write() else { return };
        m.insert(path);
    }
    pub fn unmask_marker(&self, path: MarkerPath) {
        let Ok(mut m) = self.masked.write() else { return };
        m.remove(&path);
    }

    pub fn is_allocated_path(&self, path: MarkerPath) -> bool {
        let end = self.allocated_range_for_namespace(path.path.namespace());
        let idx = match path.path.namespace() {
            MarkerIndex::NS_TRAIL => path.path.trail_index_unchecked() as u32,
            _ => path.path.index(),
        };
        end.contains(&idx)
    }
    pub fn allocated_range_for_namespace(&self, ns: u32) -> ops::RangeTo<u32> {
        let end = match ns {
            MarkerIndex::NS_POI => self.end_path_poi().path as u32,
            MarkerIndex::NS_TRAIL => self.end_path_trail().path as u32,
            MarkerIndex::NS_CAT => self.end_path_category().path as u32,
            _ => 0,
        };
        ..end
    }

    pub fn reset_with_static_range(&self, ranges: StaticMarkerRanges) {
        self.clear_masks(Some(ranges));
        let (poi, trail, cat) = ranges;
        self.next_index_poi.store(poi.end.path, Self::ORDERING_RESERVE);
        self.next_index_trail.store(trail.end.path, Self::ORDERING_RESERVE);
        self.next_index_category.store(cat.end.path, Self::ORDERING_RESERVE);
    }
    /// exception for preserving static masked markers
    pub fn clear_masks(&self, exception: Option<StaticMarkerRanges>) {
        let mut m = self.masked.write().unwrap_or_else(|e| e.into_inner());
        match exception {
            Some((poi, trail, cat)) => {
                m.retain(|mask| match mask.path.namespace() {
                    MarkerIndex::NS_POI => mask.path.index_poi_unchecked() < poi.end.path,
                    MarkerIndex::NS_TRAIL => mask.path.trail_index_unchecked() < trail.end.path,
                    MarkerIndex::NS_CAT => mask.path.index_category_unchecked() < cat.end.path,
                    _ => {
                        #[cfg(taimi_debug)]
                        log::debug!("unexpected mask {mask}");
                        true
                    },
                });
            },
            None => {
                m.clear();
            },
        }
    }
    pub fn static_ranges_for(pack: &Pack) -> StaticMarkerRanges {
        (
            ..PoiPath::new_path(pack.pois.len() as PoiIndex),
            ..TrailPath::new_path(pack.trails.len() as TrailIndex),
            ..CategoryPath::new_path(pack.categories.all_categories.len() as CategoryIndex),
        )
    }
    pub fn static_range_for_namespace(pack: &Pack, ns: u32) -> ops::RangeTo<u32> {
        let end = match ns {
            MarkerIndex::NS_POI => pack.pois.len() as u32,
            MarkerIndex::NS_TRAIL => pack.trails.len() as u32,
            MarkerIndex::NS_CAT => pack.categories.all_categories.len() as u32,
            _ => 0,
        };
        ..end
    }
}
impl Default for SharedPackAllocation {
    fn default() -> Self {
        Self {
            masked: Default::default(),
            next_index_poi: AtomicU32::new(Self::INDEX_MAX_POI),
            next_index_trail: AtomicU16::new(Self::INDEX_MAX_TRAIL),
            next_index_category: AtomicU32::new(Self::INDEX_MAX_CATEGORY),
        }
    }
}

#[derive(Clone, Default)]
pub struct SharedPackLoaded {
    pub unloaded: Option<UnloadedReason>,
    pub loader: Option<SharedLoaderBox>,
    pub pack: Option<Arc<Pack>>,
}
impl SharedPackLoaded {
    pub fn new_unloaded(reason: UnloadedReason) -> Self {
        Self {
            unloaded: Some(reason),
            loader: None,
            pack: None,
        }
    }
    pub fn empty() -> Self {
        Self {
            unloaded: None,
            pack: None,
            loader: None,
        }
    }
    pub fn pack_or(&self) -> Result<&Arc<Pack>, Option<&UnloadedReason>> {
        self.pack.as_ref().ok_or(self.unloaded.as_ref())
    }
    pub fn kill(&mut self) {
        *self = Self {
            unloaded: Some(UnloadedReason::Gravestone),
            ..Default::default()
        };
    }
    /// TODO: don't let inane reasons clobber errors
    pub fn unload(&mut self, reason: Option<UnloadedReason>) {
        self.unloaded = reason;
        self.pack = None;
        match self.unloaded {
            Some(UnloadedReason::Loading | UnloadedReason::Pending) | None => (),
            Some(..) => {
                self.loader = None;
            },
        }
    }
}
impl fmt::Debug for SharedPackLoaded {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("SharedPackLoaded")
            .field("unloaded", &self.unloaded)
            .field("pack", &self.pack)
            .field("loader", &self.loader.as_ref().map(Arc::as_ptr))
            .finish()
    }
}

#[derive(Debug, Clone, Default)]
pub struct SharedPackConfig {
    pub config: PackConfig,
    pub info_sig: PackInfoSignature,
}
impl SharedPackConfig {
    pub fn is_empty(&self) -> bool {
        self.info_sig.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct SharedPackLoad {
    pub info: Arc<SharedPackInfo>,
    pub loaded: watch::Sender<SharedPackLoaded>,
    pub config: watch::Sender<SharedPackConfig>,
}
impl SharedPackLoad {
    pub fn new_preload(info: Arc<SharedPackInfo>) -> Self {
        Self {
            info,
            loaded: watch::Sender::new(SharedPackLoaded::new_unloaded(UnloadedReason::Pending)),
            config: Default::default(),
        }
    }

    pub fn new(info: Arc<SharedPackInfo>) -> Self {
        Self {
            info,
            loaded: Default::default(),
            config: Default::default(),
        }
    }

    pub fn is_dead(&self) -> bool {
        self.info.is_dead()
    }

    /// TODO: replace info with a shared tombstone arc instead
    pub fn kill(&mut self) {
        Arc::make_mut(&mut self.info).kill();
        self.loaded.send_modify(|l| {
            l.kill();
        });
    }

    pub fn unload_info(&mut self) -> Option<PackInfoSignature> {
        Arc::make_mut(&mut self.info).unload_info()
    }
    pub fn set_info(&mut self, info: Arc<PackInfo>) -> Option<PackInfoSignature> {
        Arc::make_mut(&mut self.info).set_info(info)
    }
    pub fn ensure_index(&mut self, index: PackPath) {
        if self.info.index == index {
            return
        }
        Arc::make_mut(&mut self.info).index = index;
    }
    pub fn set_loaded(&mut self, loaded: PackActivateLoaded) -> bool {
        let PackActivateLoaded { info, config, pack, loader } = loaded;
        let changed = self.set_info(info).is_some();
        let info_sig = self.info.sig.clone();
        self.info.dynamics.reset_for_pack(&pack);
        self.loaded.send_if_modified(|shared| {
            let dirty = shared.unloaded.is_some() || changed;
            shared.unloaded = None;
            shared.pack = Some(pack);
            shared.loader = Some(loader);
            dirty
        });
        if config.is_some() || changed {
            if config.is_none() {
                log::info!("TODO: reload pack config bleh");
            }
            self.config.send_if_modified(|shared| {
                shared.info_sig = config
                    .is_some()
                    .then_some(info_sig)
                    .unwrap_or(PackInfoSignature::EMPTY);
                shared.config = config.unwrap_or_default();
                true
            });
        }
        changed
    }
    pub fn set_unloaded(&mut self, reason: Option<UnloadedReason>) -> bool {
        if let Some(UnloadedReason::Gravestone) = reason {
            let was_dead = self.is_dead();
            self.kill();
            return !was_dead
        }

        self.loaded.send_if_modified(|shared| {
            let dirty = shared.pack.is_some() || shared.unloaded != reason;
            shared.unload(reason);
            dirty
        });
        false
    }

    pub fn clear_for_shutdown(&mut self, notify: bool) {
        Arc::make_mut(&mut self.info).kill();
        self.loaded.send_if_modified(|l| {
            l.kill();
            notify
        });
    }
}

pub type SharedResourceRequestsTx<K, T> = watch::Sender<BTreeMap<K, Option<T>>>;
#[derive(Clone)]
pub struct SharedResourceRequests<K, T> {
    /// idk these things just seem like bad condvars but bleh
    pub resources: Watcher<BTreeMap<K, Option<T>>>,
}
impl<K, T> SharedResourceRequests<K, T> {
    pub fn empty() -> Self {
        Self { resources: Watcher::EMPTY }
    }
    pub fn new() -> Self {
        Self {
            resources: Watcher::new(BTreeMap::new()),
        }
    }
    pub fn new_sender() -> SharedResourceRequestsTx<K, T> {
        watch::Sender::new(BTreeMap::new())
    }
    pub fn subscribed_to(sender: &SharedResourceRequestsTx<K, T>) -> Self {
        Self {
            resources: Watcher::start_watching(sender),
        }
    }
    pub fn subscribe_to(&mut self, sender: &SharedResourceRequestsTx<K, T>) {
        self.resources.restart_watching(sender);
    }
    pub fn is_watching(&self) -> bool {
        self.resources.is_watching()
    }
}
impl<K, T> SharedResourceRequests<K, T>
where
    K: Ord,
{
    /// `Err(T)` if `take` and already completed
    pub fn request(&self, id: K, take: bool) -> Result<(), T> {
        let mut res = Ok(());
        self.resources
            .write_if(|resources| match Self::request_one(resources, id, take) {
                Ok(newly_requested) => newly_requested,
                Err(r) => {
                    res = Err(r);
                    false
                },
            });
        // would do this if it weren't a race condition...
        // self.resources.mark_unchanged();
        res
    }
    pub fn request_many<I: IntoIterator<Item = K>>(&self, ids: I) {
        self.request_many_dyn(&mut ids.into_iter())
    }
    pub fn request_many_dyn(&self, ids: &mut dyn Iterator<Item = K>) {
        self.resources.write_if(|resources| {
            let mut notify = false;
            for id in ids {
                match Self::request_one(resources, id, false) {
                    Ok(true) => notify = true,
                    #[cfg(debug_assertions)]
                    Err(..) => unreachable!(),
                    _ => (),
                }
            }
            notify
        });
        // would do this if it weren't a race condition...
        // self.resources.mark_unchanged();
    }
    fn request_one(resources: &mut BTreeMap<K, Option<T>>, id: K, take: bool) -> Result<bool, T> {
        match resources.entry(id) {
            btree_map::Entry::Occupied(e) => {
                let res = if e.get().is_some() && take {
                    let (_, resource) = e.remove_entry();
                    resource
                } else {
                    None
                };
                match res {
                    Some(r) => Err(r),
                    None =>
                    // don't need to tell other side whether we "took" it or not
                        Ok(false),
                }
            },
            btree_map::Entry::Vacant(e) => {
                e.insert(None);
                Ok(true)
            },
        }
    }
    pub fn fill_request(&mut self, id: K, v: T) {
        self.resources.write_if(|resources| {
            resources.insert(id, Some(v));
            true
        });
        // would do this if it weren't a race condition...
        // self.resources.mark_unchanged();
    }
    /// Leaving them unfulfilled isn't unreasonable, so consider if this is needed?
    pub fn cancel_request(&mut self, id: &K) {
        self.resources.write_if(|resources| {
            resources.remove(id);
            // no point in waking them up, just please don't repeat it thanks
            false
        });
    }
    pub fn retain<F: FnMut(&K, &mut Option<T>) -> bool>(&mut self, filter: F) {
        self.resources.write_if(|resources| {
            resources.retain(filter);
            // no point in waking them up
            false
        });
    }
    pub fn clear_all(&mut self) {
        self.resources.write_if(|resources| {
            resources.clear();
            // no point in waking them up
            false
        });
    }
}
impl<K, T> SharedResourceRequests<K, T>
where
    K: Ord + Clone,
{
    pub fn take_fulfilled<F: FnMut(&K) -> bool>(&self, mut filter: F, out: &mut Vec<(K, T)>) {
        self.resources.write_if(|resources| {
            resources.retain(|id, v| {
                if v.is_some() && !filter(id) {
                    return true
                }
                match v.take() {
                    Some(v) => {
                        out.push((id.clone(), v));
                        false
                    },
                    None => true,
                }
            });
            // no need to notify when removing
            false
        });
    }
    pub fn try_recv_fulfilled(&self) -> impl ExactSizeIterator<Item = (K, T)> {
        let mut out = Vec::new();
        if self.resources.has_changed() {
            self.take_fulfilled(|_| true, &mut out);
        }
        out.into_iter()
    }
    pub fn get_requests<F: FnMut(&K) -> bool>(&mut self, mut filter: F) -> Vec<K> {
        let resources = self.resources.read_update();
        resources
            .iter()
            .filter_map(|(id, v)| match v {
                None if filter(id) => Some(id.clone()),
                _ => None,
            })
            .collect()
    }
    pub async fn recv_requests<F: FnMut(&K) -> bool>(&mut self, mut filter: F) -> Vec<K> {
        loop {
            self.resources.when_changed().await;
            let reqs = self.get_requests(&mut filter);
            if !reqs.is_empty() {
                break reqs
            }
        }
    }
}
impl<K, T> Default for SharedResourceRequests<K, T> {
    fn default() -> Self {
        Self::empty()
    }
}
#[derive(Debug)]
pub enum LoadReport {
    TrailGeometry {
        path: Locator<PackMapPath, LoadedTrailPath>,
        geometry: anyhow::Result<LoadedTrailGeometry>,
        section_info: Option<TrailGeometrySections>,
    },
    Texture {
        path: LoadedMarkerPath<PackMapPath>,
        texture: anyhow::Result<TextureKey>,
        resource: Option<AttrString>,
    },
}

/// TODO: a real struct with signal notify stuff
pub type SharedGracePeriod = AtomicUsize;
impl SharedPacks {
    const GRACE_BIT_WAITING: usize = 1 << (mem::size_of::<usize>() * 8 - 1);
    const GRACE_COUNT_MASK: usize = u16::MAX as usize & !Self::GRACE_BIT_WAITING;
    pub(crate) fn update_load_period(&self, pending: usize, state: bool) -> bool {
        let value = pending.min(Self::GRACE_COUNT_MASK) | state.then_some(Self::GRACE_BIT_WAITING).unwrap_or(0);
        let prev = self.load_period.swap(value, Ordering::Relaxed);
        self.notify_load_if(state, prev)
    }
    pub(crate) fn update_load_count(&self, pending: usize) {
        let pending = pending.min(Self::GRACE_COUNT_MASK);
        let _ = self.load_period.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |prev| {
            Some((prev & Self::GRACE_BIT_WAITING) | pending)
        });
    }
    pub(crate) fn update_load_state(&self, state: bool) -> bool {
        let prev = match state {
            true => self.load_period.fetch_or(Self::GRACE_BIT_WAITING, Ordering::Relaxed),
            false => self.load_period.fetch_and(!Self::GRACE_BIT_WAITING, Ordering::Relaxed),
        };
        self.notify_load_if(state, prev)
    }
    fn notify_load_if(&self, state: bool, prev: usize) -> bool {
        let prev_state = prev & Self::GRACE_BIT_WAITING != 0;
        let changed = state != prev_state;
        if changed {
            if !state {
                log::debug!("TODO: notify grace period waiters");
            }
        }
        changed
    }
    pub fn read_still_waiting(&self) -> (bool, usize) {
        let v = self.load_period.load(Ordering::Acquire);
        let state = v & Self::GRACE_BIT_WAITING != 0;
        (state, v & Self::GRACE_COUNT_MASK)
    }
    #[inline]
    pub(crate) fn is_still_waiting(&self) -> bool {
        self.load_period.load(Ordering::Relaxed) & Self::GRACE_BIT_WAITING != 0
    }
    #[inline]
    pub(crate) fn is_quiet(&self) -> bool {
        self.load_period.load(Ordering::Relaxed) & (Self::GRACE_BIT_WAITING | Self::GRACE_COUNT_MASK) == 0
    }
}
