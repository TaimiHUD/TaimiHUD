use {
    crate::controller::pathing::registry::{
        PackBoxOf,
        PackCategoryInfo,
        PackRoot,
        PackPath, PackIndex,
        PackConfig,
        PackInfo, PackInfoSignature,
        UnloadedReason,
        PackActivateLoaded,
        SharedLoaderBox,
    },
    crate::exports::runtime as rt,
    crate::settings::sources::DataSourcePath,
    rustc_hash::FxHashMap,
    std::{fmt, mem, sync::{Arc, Weak, RwLock}, path::Path, collections::{BTreeMap, btree_map}},
    taimi_sync::{arcs::weak_is_null, watched::{watch, Watcher}},
    taimi_meta::packs::MapIndex,
    taimi_pack::{attributes::AttrString, Pack},
    taimi_hoard::loc::LocationMut,
    taimi_sync::arcs::ArcPtrCmp,
};

pub type SharedLoaderPacksInfo = PackBoxOf<SharedPackLoad>;
#[cfg(deleteme)]
pub type SharedLoaderPackInfo = Box<[LoadedPackInfo]>;
#[cfg(deleteme)]
pub type SharedLoaderPackData = Box<[Weak<ActivePack>]>;
#[cfg(deleteme)]
pub type SharedLoaderPackConfig = Box<[Option<watch::Sender<Arc<PackConfig>>>]>;
/// TODO: maybe split this up into sender and receiver halves...
/// or just rework LoadedPack into something more sane to share
#[derive(Debug)]
pub struct SharedPacks {
    pub packs: watch::Sender<SharedLoaderPacksInfo>,
    /// TODO: deprecate
    #[cfg(deleteme)]
    pub info: watch::Sender<SharedLoaderPackInfo>,
    /// TODO: deprecate
    #[cfg(deleteme)]
    pub data: watch::Sender<SharedLoaderPackData>,
    /// TODO: deprecate
    #[cfg(deleteme)]
    pub config: watch::Sender<SharedLoaderPackConfig>,
}

impl SharedPacks {
    pub fn new() -> Self {
        Self {
            packs: Default::default(),
            #[cfg(deleteme)]
            info: Default::default(),
            #[cfg(deleteme)]
            data: Default::default(),
            #[cfg(deleteme)]
            config: Default::default(),
        }
    }

    #[cfg(deleteme)]
    pub fn packs<I: IntoIterator>(packs: I) -> impl Iterator<Item = (PackPath, I::Item)> {
        packs.into_iter().enumerate()
            .map(|(i, p)| (PackPath::with_path(i as PackIndex), p))
    }

    #[cfg(deleteme)]
    pub fn pack_at<D>(packs: &[D], path: PackPath) -> Option<&D> {
        let idx = path.path as usize;
        packs.get(idx)
    }
    #[cfg(deleteme)]
    pub fn try_shared_pack_active(packs: &[Weak<ActivePack>], path: PackPath) -> Option<Arc<ActivePack>> {
        Self::pack_at(packs, path)
            .and_then(Weak::upgrade)
    }
    /// TODO: keeping this as a marker to indicate the loader may be able to load on-demand later on
    /// (and anything still using this probably wants to switch to that instead)
    #[cfg(deleteme)]
    pub fn pack_active(packs: &[Weak<ActivePack>], path: PackPath) -> Option<Arc<ActivePack>> {
        Self::try_shared_pack_active(packs, path)
    }

    #[cfg(deleteme)]
    pub(crate) fn update_pack_info(&self, path: PackPath, pack: &LoadedPackInfo) {
        log::error!("TODO: PackLoader::update_pack_info");
    }
    #[cfg(deleteme)]
    pub(crate) fn update_pack_info(&self, path: PackPath, pack: &LoadedPackInfo) {
        self.info.send_if_modified(|shared| {
            let idx = path.path as usize;
            let amt = shared.len();
            match shared.get_mut(idx) {
                Some(out) => match *out == *pack {
                    true => false,
                    false => {
                        out.clone_from(pack);
                        true
                    },
                },
                None if idx == amt => {
                    let info = Vec::from(mem::take(shared));
                    *shared = info.into_iter()
                        .chain(iter::once(pack.clone()))
                        .collect();
                    true
                },
                None => {
                    log::error!("shared updates incomplete, can't reach {}", pack.index);
                    false
                },
            }
        });
    }

    #[cfg(deleteme)]
    pub(crate) fn update_pack_config(&self, path: PackPath, config: Option<&watch::Sender<Arc<PackConfig>>>) {
        log::error!("TODO: PackLoader::update_pack_config");
    }
    #[cfg(deleteme)]
    pub(crate) fn update_pack_config(&self, path: PackPath, config: Option<&watch::Sender<Arc<PackConfig>>>) {
        let Some(config) = config else { return };
        let idx = path.path as usize;
        self.config.send_if_modified(|shared| match shared.get_mut(idx) {
            Some(Some(out)) if config.same_channel(out) =>
                false,
            Some(out) => {
                *out = Some(config.clone());
                true
            },
            None => {
                let mut configs = Vec::from(mem::take(shared));
                configs.resize_with(idx, || None);
                configs.push(Some(config.clone()));
                *shared = configs.into_boxed_slice();
                true
            },
        });
    }

    #[cfg(deleteme)]
    pub(crate) fn update_pack_active(&self, path: PackPath, pack: Option<&Arc<ActivePack>>) {
        log::error!("TODO: PackLoader::update_pack_active");
    }
    #[cfg(deleteme)]
    pub(crate) fn update_pack_active(&self, path: PackPath, pack: Option<&Arc<ActivePack>>) {
        self.data.send_if_modified(|shared| {
            let pack_shared = || pack.map(Arc::downgrade).unwrap_or(Weak::new());
            let idx = path.path as usize;
            match shared.get_mut(idx) {
                Some(out) => {
                    match pack.map(Arc::as_ptr) {
                        Some(p) if p == Weak::as_ptr(out) =>
                            false,
                        None if weak_is_null(&*out) =>
                            false,
                        _ => {
                            *out = pack_shared();
                            true
                        },
                    }
                },
                None => {
                    let mut info = Vec::from(mem::take(shared));
                    info.resize_with(idx, || Weak::new());
                    *shared = info.into_iter()
                        .chain(iter::once(pack_shared()))
                        .collect();
                    true
                },
            }
        });
    }

    pub(crate) fn update_packs_extend(&self, packs: &mut dyn Iterator<Item = SharedPackLoad>) {
        let (_min, max) = packs.size_hint();
        if max == Some(0) {
            // empty iterator, don't bother
            return
        }
        self.packs.send_if_modified(|shared| {
            let mut next_path = shared.end_path();
            let mut any_appended = false;
            let packs = packs.map(|mut pack| {
                pack.ensure_index(next_path);
                next_path.path += 1;
                any_appended = true;
                pack
            });
            let prev = match shared.data {
                #[cfg(todo = "unnecessary")]
                ref data => data.iter().cloned(),
                // *shrug* I guess an empty box allocation is better than cloning some arcs
                ref mut data => Box::into_iter(mem::take(data)),
            };
            let updated = prev
                .chain(packs)
                .collect::<Box<[_]>>();
            shared.data = updated;
            any_appended
        });
    }
    pub(crate) fn update_packs_loaded(&self, loaded: &mut dyn Iterator<Item = (PackPath, Result<PackActivateLoaded, Option<UnloadedReason>>)>) {
        self.packs.send_if_modified(|shared| {
            let mut changed = false;
            for (path, loaded) in loaded {
                if let Err(Some(reason)) = &loaded {
                    log::error!("failed to load {path}: {reason}");
                }
                let Some(pack) = shared.lookup_mut(&path) else {
                    log::warn!("nonexistent pack update for {path}?");
                    continue
                };

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
        }
    }

    pub fn info(&self) -> Option<(PackPath, &Arc<PackInfo>, PackInfoSignature)> {
        self.info.as_ref().map(|i|
            (self.index, i, self.sig)
        )
    }
    pub fn category_info(&self) -> Option<(&Arc<PackCategoryInfo>, &Arc<PackInfo>)> {
        self.info.as_ref().map(|i|
            (&i.categories, i)
        )
    }
    pub fn primary_root(&self) -> Option<&PackRoot> {
        self.info.as_ref().and_then(|i| i.primary_root())
    }
    pub fn unique_root(&self) -> Option<&PackRoot> {
        let mut roots = self.info.as_ref()?.roots.iter()
            .filter(|r| !r.hidden);
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
        self.info.as_ref().map(|i| i.maps.contains(map_id))
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
        keys.entry(resource.clone()).or_insert_with(|| Arc::from(self.gen_key_for_subresource(resource)))
            .clone()
    }
    pub fn gen_key_for_subresource(&self, resource: &AttrString) -> String {
        let packname = rt::relative_path(&self.path);
        let resource = &resource[..];
        let storage;
        let resourceid = match resource.len() {
            0..=24 => resource,
            _toolong => {
                use std::hash::{Hash, Hasher, DefaultHasher};
                // don't care sorry
                let mut hasher = DefaultHasher::new();
                resource.hash(&mut hasher);
                storage = format!("{:x}", hasher.finish());
                &storage
            },
        };
        format!("{}_{resourceid}", packname.display())
    }
    pub fn drain_subresource_keys(&self) -> impl IntoIterator<Item = (AttrString, Arc<str>)> + 'static {
        let mut keys = self.allocated_keys.write()
            .unwrap_or_else(|e| e.into_inner());
        keys.drain().collect::<Vec<_>>()
    }
}
impl Default for SharedPackInfo {
    fn default() -> Self {
        Self::empty(None)
    }
}
impl fmt::Display for SharedPackInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(root) = self.primary_root() {
            f.write_str(&root.display_name)
        } else if let Some(datasource) = &self.datasource {
            fmt::Display::fmt(&datasource.path, f)
        } else if !self.path.as_os_str().is_empty() {
            let path = self.path.file_name().unwrap_or_else(|| rt::relative_path(&self.path).as_os_str());
            fmt::Display::fmt(&path.display(), f)
        } else {
            fmt::Display::fmt(&self.index, f)
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
        *self = Default::default();
    }
    /// TODO: don't let inane reasons clobber errors
    pub fn unload(&mut self, reason: Option<UnloadedReason>) {
        self.unloaded = reason;
        self.pack = None;
        if self.unloaded.is_some() {
            self.loader = None;
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
        if self.info.index == index { return }
        Arc::make_mut(&mut self.info).index = index;
    }
    pub fn set_loaded(&mut self, loaded: PackActivateLoaded) -> bool {
        let PackActivateLoaded { info, config, pack, loader } = loaded;
        let changed = self.set_info(info).is_some();
        let info_sig = self.info.sig.clone();
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
                shared.info_sig = config.is_some().then_some(info_sig).unwrap_or(PackInfoSignature::EMPTY);
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
            let dirty = shared.unloaded != reason;
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
    pub resources: Watcher<BTreeMap<K, Option<T>>>
}
impl<K, T> SharedResourceRequests<K, T> {
    pub fn empty() -> Self {
        Self {
            resources: Watcher::EMPTY,
        }
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
impl<K, T> SharedResourceRequests<K, T> where
    K: Ord,
{
    /// `Err(T)` if `take` and already completed
    pub fn request(&self, id: K, take: bool) -> Result<(), T> {
        let mut res = Ok(());
        self.resources.write_if(|resources| {
            match Self::request_one(resources, id, take) {
                Ok(newly_requested) =>
                    newly_requested,
                Err(r) => {
                    res = Err(r);
                    false
                },
            }
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
                    Ok(true) =>
                        notify = true,
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
                        Ok(false)
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
impl<K, T> SharedResourceRequests<K, T> where
    K: Ord + Clone,
{
    pub fn take_fulfilled<F: FnMut(&K) -> bool>(&self, mut filter: F, out: &mut Vec<(K, T)>) {
        self.resources.write_if(|resources| {
            resources.retain(|id, v| {
                if v.is_some() && !filter(id) { return true }
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
        resources.iter().filter_map(|(id, v)| match v {
            None if filter(id) => Some(id.clone()),
            _ => None,
        }).collect()
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
