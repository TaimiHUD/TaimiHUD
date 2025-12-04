#[cfg(doc)]
use taimi_pack::attributes::keys;
use tokio::sync::{RwLock, RwLockMappedWriteGuard, RwLockWriteGuard};
use crate::exports::runtime::locator::LocationMut;

use {
    crate::{
        exports::runtime as rt,
        settings::{DataSourcePath, PathingSettings},
        controller::pathing::state::{VisibilityFlagSet, VisibilityFlags},
    },
    anyhow::anyhow,
    bitvec::vec::BitVec,
    futures::{future::{self, Either}, stream::{self, FusedStream, Stream, StreamExt}, FutureExt}, std::{cmp, collections::{BTreeMap, BTreeSet, HashSet}, error::Error as StdError, fmt, hash, iter, path::{Path, PathBuf}, ptr, sync::Arc}, taimi_meta::map::MapID, taimi_pack::{
        category::{id::{AsFullId, CategoryId}, Category}, pack::CategoryCollection, Pack
    }, tokio::sync::watch,
    tokio_util::sync::ReusableBoxFuture,
};
pub use taimi_meta::loc::packs::id::{self, MarkerId, MarkerPath, MarkerIndex, MarkerIndexVariant};
pub use self::{
    active::{ActivePack, PackFormat, PackLoader},
    collections::{BitFlagForSet, CategorySet, FlagSet, MapSet, MarkerSet, RecentlyUsed},
    namespace::*,
};

mod active;
mod collections;
mod namespace;

#[derive(Debug, Default)]
pub struct PackRegistry {
    pub packs: Vec<LoadedPack>,
}

impl PackRegistry {
    pub const fn new() -> Self {
        Self {
            packs: Vec::new(),
        }
    }

    pub fn all_paths(&self) -> impl Iterator<Item = PackPath> + Clone {
        let len = self.packs.len();
        (0..len).into_iter().map(|path| PackPath::with_path(path as PackIndex))
    }

    pub fn all_packs<'a>(&'a self) -> impl Iterator<Item = (PackPath, &'a LoadedPack)> {
        self.packs.iter().enumerate()
            .map(|(path, pack)| (PackPath::with_path(path as PackIndex), pack))
    }
    pub fn all_packs_mut<'a>(&'a mut self) -> impl Iterator<Item = (PackPath, &'a mut LoadedPack)> {
        self.packs.iter_mut().enumerate()
            .map(|(path, pack)| (PackPath::with_path(path as PackIndex), pack))
    }
    #[cfg(todo = "unused")]
    pub fn loaded_packs<'a>(&'a self) -> impl Iterator<Item = (PackPath, &'a LoadedPack, &'a Arc<PackInfo>)> {
        self.all_packs().filter_map(|(path, pack)| match &pack.info.info {
            Ok(info) => Some((path, pack, info)),
            Err(..) => None,
        })
    }
    pub fn active_packs<'a>(&'a self) -> impl Iterator<Item = (PackPath, &'a LoadedPack, &'a Arc<ActivePack>)> {
        self.all_packs().filter_map(|(path, pack)| match &pack.active {
            Some(active) => Some((path, pack, active)),
            None => None,
        })
    }
    #[cfg(todo = "unused")]
    pub fn unloaded_packs<'a>(&'a self) -> impl Iterator<Item = (PackPath, &'a LoadedPack, Result<&'a Arc<PackInfo>, &'a UnloadedReason>)> {
        self.all_packs().filter_map(|(path, pack)| match &pack.info.info {
            Err(reason) => Some((path, pack, Err(reason))),
            Ok(info) if pack.active.is_none() => Some((path, pack, Ok(info))),
            Ok(..) => None,
        })
    }

    pub fn packs_for_map(&self, map_id: MapIndex) -> impl Iterator<Item = (PackPath, &LoadedPack)> {
        self.all_packs()
            .filter(move |(_, pack)| pack.any_enabled_for_map(map_id))
    }
    pub fn packs_for_map_mut(&mut self, map_id: MapIndex) -> impl Iterator<Item = (PackPath, &mut LoadedPack)> {
        self.all_packs_mut()
            .filter(move |(_, pack)| pack.any_enabled_for_map(map_id))
    }

    pub const CONCURRENT_LOAD_LIMIT: usize = 8;
    pub async fn load_packs_for_map<'a, 'm>(registry: &'a RwLock<Self>, manager: &'m PackLoader, map_id: MapIndex) -> impl Stream<Item = (PackPath, RwLockMappedWriteGuard<'a, LoadedPack>)> + 'm where
        'a: 'm,
    {
        let map_packs = {
            let registry = registry.read().await;
            let pack_paths =registry.all_paths();
            let map_packs: BitVec = registry.all_packs()
                .map(|(_path, pack)| pack.any_enabled_for_map(map_id))
                .collect();
            pack_paths
                .zip(map_packs.into_iter())
                .filter_map(|(path, enabled)| enabled.then_some(path))
        };
        stream::iter(map_packs)
            .map(move |path| async move {
                let pack = Self::load_pack(registry, manager, path).await;
                (path, pack)
            }).buffer_unordered(Self::CONCURRENT_LOAD_LIMIT)
            .filter_map(move |(path, pack)| future::ready(match pack {
                Err(e) => {
                    log::error!("{e:#}");
                    None
                },
                Ok(pack) if !pack.any_enabled_for_map(map_id) =>
                    None,
                Ok(pack) if pack.info.info.is_err() =>
                    None,
                Ok(pack) => Some((path, pack)),
            }))
    }

    pub async fn load_pack<'a, 'm>(registry: &'a RwLock<Self>, manager: &'m PackLoader, path: PackPath) -> anyhow::Result<RwLockMappedWriteGuard<'a, LoadedPack>> {
        let pack = {
            let pack = RwLockWriteGuard::try_map(
                registry.write().await,
                |r| r.lookup_mut(&path),
            );
            let pack = match pack {
                Ok(pack) => Some(pack),
                Err(_reg) => None,
            };

            let start = match pack {
                Some(mut pack) => match pack.activate_start().transpose() {
                    None => Either::Left(Some(pack)),
                    Some(start) => Either::Right(
                        start.map(|start| (start, pack.info.path.clone()))
                    ),
                },
                pack => Either::Left(pack),
            };
            match start {
                Either::Left(pack) => Either::Left(future::ready(Ok(pack))),
                Either::Right(start) => Either::Right(async move {
                    let (start, filepath) = start?;

                    let res = LoadedPack::activate_load(start, filepath.to_path_buf(), manager).await;

                    let pack = RwLockWriteGuard::try_map(
                        registry.write().await,
                        |r| r.lookup_mut(&path),
                    ).ok();
                    match pack {
                        Some(mut pack) => pack.activate_finish(res, manager)
                            .map(move |()| Some(pack)),
                        // shouldn't happen but...
                        None => match res {
                            Err(e) => Err(e),
                            Ok(..) => Ok(None),
                        },
                    }
                }),
            }
        };
        match pack.await.transpose() {
            None => Err(anyhow!("pack {path} does not exist")),
            Some(res) => res,
        }
    }

    pub fn preload<P>(&mut self, path: P, datasource: Option<DataSourcePath>, manager: &PackLoader) -> PackPath where
        P : AsRef<Path> + Into<PathBuf>,
    {
        if let Some((i, pack)) = self.packs.iter_mut().enumerate().find(|(_, p)| p.info.path.as_ref() == path.as_ref()) {
            // ?
            pack.info.datasource = datasource;
            let path = PackPath::with_path(i as PackIndex);
            manager.shared.packs.update_pack_info(path, &pack.info);
            return path
        }

        let i = self.packs.len();
        let index = PackPath::with_path(i as PackIndex);
        self.packs.push(LoadedPack::new_unloaded(index, path.into(), datasource));
        if let Some(pack) = self.packs.last() {
            // dumb if :<
            manager.shared.packs.update_pack_info(pack.info.index, &pack.info);
        }
        index
    }

    pub fn watch_config_changes(&self) -> impl FusedStream<Item = (PackPath, watch::Receiver<Arc<PackConfig>>)> + Unpin + Send + 'static {
        async fn changed_static<T>(mut watch: watch::Receiver<T>) -> Result<watch::Receiver<T>, watch::error::RecvError> {
            watch.changed().await
                .map(move |()| watch)
        }

        fn watch_config_change(path: PackPath, mut config: watch::Receiver<Arc<PackConfig>>) -> impl Stream<Item = (PackPath, watch::Receiver<Arc<PackConfig>>)> + Unpin + Send + 'static {
            use std::task::Poll;

            config.mark_changed();
            let mut storage = Some(ReusableBoxFuture::new(changed_static(config)));
            stream::poll_fn(move |cx| {
                let Some(changed) = &mut storage else { return Poll::Pending };
                let res = futures::ready!(changed.poll_unpin(cx));
                match res {
                    Ok(watch) => {
                        changed.set(changed_static(watch.clone()));
                        Poll::Ready(Some((path, watch)))
                    },
                    Err(..) => {
                        let _ = storage.take();
                        Poll::Ready(None)
                    },
                }
            })
        }

        self.all_packs()
            .filter_map(|(path, pack)| pack.config.as_ref().map(|config|
                (path, config.subscribe())
            )).map(|(path, config)| watch_config_change(path, config))
            .collect::<stream::SelectAll<_>>()
    }
}

#[derive(Debug)]
pub struct LoadedPack {
    pub info: LoadedPackInfo,
    pub active: Option<Arc<ActivePack>>,
    pub config: Option<watch::Sender<Arc<PackConfig>>>,
}

impl LoadedPack {
    pub fn new_unloaded(index: PackPath, path: PathBuf, datasource: Option<DataSourcePath>) -> Self {
        Self {
            info: LoadedPackInfo {
                index,
                path: path.into(),
                datasource,
                info: Err(UnloadedReason::Pending),
            },
            active: None,
            config: None,
        }
    }

    /// Free up memory related to this pack when not in use
    pub fn deactivate(&mut self, manager: &PackLoader) {
        let _ = self.active.take();
        manager.shared.packs.update_pack_active(self.info.index, None);
    }

    /// leave a gravestone marker so our index is never used again
    pub fn mark_dead(&mut self, manager: &PackLoader) {
        let _ = self.config.take();
        self.info = LoadedPackInfo::gravestone(self.info.index);
        manager.shared.packs.update_pack_info(self.info.index, &self.info);
        self.deactivate(manager);
    }

    pub fn mark_reload(&mut self, manager: &PackLoader) {
        if let Err(reason) = &mut self.info.info {
            if !reason.can_reload() {
                return
            }
            *reason = UnloadedReason::Pending;
            manager.shared.packs.update_pack_info(self.info.index, &self.info);
        }
        self.deactivate(manager);
    }

    pub fn mark_loading(&mut self, manager: &PackLoader) {
        if let Err(reason) = &mut self.info.info {
            *reason = UnloadedReason::Loading;
            manager.shared.packs.update_pack_info(self.info.index, &self.info);
        }
        self.deactivate(manager);
    }

    pub fn with_config<R, F: FnOnce(&PackConfig) -> R>(&self, f: F) -> R {
        match &self.config {
            Some(c) => f(&c.borrow()),
            None => f(&PackConfig::default()),
        }
    }
    pub fn get_info(&self) -> Option<(Arc<PackInfo>, Arc<PackConfig>)> {
        let config = self.config.as_ref().map(|c| c.borrow().clone());
        self.info.info.as_ref().ok().cloned()
            .map(|i| (i, config.unwrap_or_default()))
    }

    /// Early check for whether a pack is disabled and can be skipped during load
    pub fn any_enabled(&self) -> bool {
        let info = match &self.info.info {
            Ok(info) => info,
            // generally assume so if info is missing at this point
            Err(reason) =>
                return reason.can_reload(),
        };
        let config = self.config.as_ref().map(|c| c.borrow());
        let Some(config) = config else { return true };
        config.any_enabled(&info.categories)
    }
    pub fn any_enabled_for_map(&self, map_id: MapIndex) -> bool {
        let on_map = match &self.info.info {
            Ok(info) => info.maps.contains(map_id),
            Err(UnloadedReason::Pending) => {
                // don't know yet, this shouldn't happen often...
                log::debug!("unsure whether {self} is on map {map_id}");
                true
            },
            Err(..) => false,
        };
        on_map && self.any_enabled()
    }

    pub async fn pack_info(&mut self, manager: &'_ PackLoader) -> Option<&Arc<PackInfo>> {
        let try_load = matches!(&self.info.info, Err(UnloadedReason::Pending));
        if try_load {
            let res = self.activate(manager).await;
            let _ = rt::log::error_ok(res);
        }
        self.info.info.as_ref().ok()
    }
}

impl fmt::Display for LoadedPack {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Ok(info) = &self.info.info {
            fmt::Display::fmt(info, f)
        } else if let Some(datasource) = &self.info.datasource {
            fmt::Display::fmt(&datasource.path, f)
        } else {
            let name = self.info.path.file_name()
                .map(Path::new)
                .unwrap_or_else(|| rt::relative_path(&self.info.path));
            fmt::Display::fmt(&name.display(), f)
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackInfo {
    pub format: PackFormat,
    pub roots: BTreeSet<PackRoot>,
    pub categories: Arc<PackCategoryInfo>,
    pub maps: MapSet,
}

impl PackInfo {
    #[cfg(todo)]
    pub async fn read_from_loader(loader: LoaderBox) -> anyhow::Result<Self> {
    }

    /// TODO: deprecate this soon
    pub fn from_pack(pack: &Pack, format: PackFormat) -> Self {
        let roots = pack.categories.root_categories
            .iter()
            .filter_map(|id| pack.categories.all_categories.get_full(id))
            .map(|(i, _, cat)| PackRoot::from_category(CategoryPath::with_path(i as CategoryIndex), cat))
            .collect();

        let trail_maps = pack.trails.iter()
            .filter_map(|trail| trail.map_id);
        let poi_maps = pack.pois.iter()
            .map(|poi| poi.map_id);
        let maps = trail_maps.chain(poi_maps)
            .filter_map(|id| MapID::try_from(id).ok())
            .collect();

        let mut categories = PackCategoryInfo::from_collection(&pack.categories);
        let not_lonely = {
            let pois = pack.pois.iter().map(|m| &m.category);
            let trails = pack.trails.iter().map(|m| &m.category);
            pois.chain(trails).filter_map(|c| pack.categories.all_categories.get_index_of(c.as_id()))
                .map(|i| CategoryPath::with_path(i as CategoryIndex))
        };
        categories.fill_lonely(not_lonely);

        PackInfo {
            format,
            roots,
            maps,
            categories: Arc::new(categories),
        }
    }

    pub fn primary_root(&self) -> Option<&PackRoot> {
        self.roots.iter().max_by_key(|root| (
            !root.separator,
            !root.hidden,
            root.child_count,
            &root.id,
        ))
    }
}

impl fmt::Display for PackInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.primary_root() {
            Some(root) =>
                f.write_str(&root.display_name),
            None =>
                fmt::Display::fmt(&self.format, f),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LoadedPackInfo {
    pub index: PackPath,
    pub path: Arc<Path>,
    pub info: Result<Arc<PackInfo>, UnloadedReason>,
    pub datasource: Option<DataSourcePath>,
}

impl LoadedPackInfo {
    pub fn gravestone(index: PackPath) -> Self {
        Self {
            index,
            path: Path::new("").into(),
            info: Err(UnloadedReason::Gravestone),
            datasource: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackCategoryInfo {
    pub all: Box<[PackCategory]>,
    pub roots: Box<[CategoryIndex]>,
    pub visibility: VisibilityFlagSet,
    /// [keys::IsSeparator]
    pub separators: CategorySet,
    /// [keys::IsHidden]
    pub hidden: CategorySet,
    /// ![keys::DefaultToggle]
    pub disabled: CategorySet,
    /// [keys::CopyValue] is valid on [self.separators]
    pub copyable: CategorySet,
    /// Categories that lack any marker children, toggling would be meaningless
    ///
    /// TODO: reconsider if this is useful
    /// Currently this also includes category parents, but this may change.
    pub lonely: CategorySet,
}

impl PackCategoryInfo {
    pub fn from_collection(collection: &CategoryCollection) -> Self {
        let all = PackCategory::build(collection);
        let roots = collection.root_categories.iter()
            .filter_map(|id| collection.all_categories.get_index_of(id))
            .map(|i| i as CategoryIndex)
            .collect();
        let visibility = collection.all_categories.values()
            .map(VisibilityFlags::from_pack_category)
            .collect();
        let (separators, hidden, disabled, copyable) = collection.all_categories.values().enumerate()
            .map(|(i, cat)| (i as CategoryIndex, cat))
            .map(|(i, cat)| (
                cat.is_separator().then_some(i),
                cat.is_hidden().then_some(i),
                (!cat.default_toggle()).then_some(i),
                cat.marker_attributes.interaction.as_ref().map(|i| i.copy_value.is_some()).unwrap_or(false).then_some(i),
            )).collect();

        Self {
            all: all.into_boxed_slice(),
            roots,
            visibility,
            separators,
            hidden,
            disabled,
            copyable,
            lonely: Default::default(),
        }
    }

    pub fn fill_lonely<C>(&mut self, with_children: C) where
        C: IntoIterator<Item = CategoryPath>,
    {
        let mut marker_parents: BitVec = BitVec::with_capacity(self.all.len());
        marker_parents.resize(self.all.len(), false);
        for m in with_children {
            if let Some(mut b) = marker_parents.get_mut(m.path as usize) {
                if b.replace(true) {
                    continue
                }
            } else {
                // who are you?
                continue
            }
            // mark up to the root now...
            let mut parent = m;
            while let Some(next) = self.parent_of(parent) {
                if let Some(mut p) = marker_parents.get_mut(next.path as usize) {
                    if p.replace(true) {
                        break
                    }
                }
                parent = next;
            }
        }
        for lonely in marker_parents.iter_zeros() {
            let cat = CategoryPath::with_path(lonely as CategoryIndex);
            let Some(info) = self.info_of(cat) else { continue };
            #[cfg(todo = "unnecessary")]
            if info.child().is_some() {
                // this may not even be right? you can have children that are all hidden or separators etc...
                continue
            };
            if self.lonely.insert(cat) {
                // XXX: reconsider whether to include parents in this collection or not...
                // it's easy to filter them out at least?
                let mut cat = cat;
                while let Some(parent) = self.parent_of(cat) {
                    if !self.lonely.insert(cat) {
                        break
                    }
                    cat = parent;
                }
            }
        }
    }

    pub fn root_paths(&self) -> impl Iterator<Item = CategoryPath> + '_ {
        self.roots.iter().copied().map(CategoryPath::with_path)
    }

    pub fn count(&self) -> usize {
        self.all.len()
    }

    pub fn info_of(&self, path: CategoryPath) -> Option<PackCategory> {
        self.all.get(path.path as usize).copied()
    }

    /// immediate, see [self.descendents_of] for recursion
    pub fn children_of(&self, path: CategoryPath) -> impl Iterator<Item = CategoryPath> + '_ {
        let firstborn = self.firstborn_of(path).into_iter();
        firstborn.flat_map(|firstborn| iter::once(firstborn)
            .chain(self.younger_siblings_of(firstborn))
        )
    }

    /// DFS, excludes the path itself
    pub fn descendents_of(&self, path: CategoryPath) -> impl Iterator<Item = CategoryPath> + '_ {
        let mut cycle_limit = self.all.len();
        let target = self.info_of(path);
        let firstborn = target.and_then(|c| c.child());
        let mut stack: Vec<CategoryPath> = match &target {
            #[cfg(todo = "unnecessary")]
            Some(info) if firstborn.is_some() => {
                // rough count of categories under a root...
                let amt = (self.all.len() - self.lonely.len()) / self.roots.len();
                let depth = match info.parent() {
                    Some(p) => self.parents_of(p).count(),
                    None => 0,
                } + 1;
                let stride = self.younger_siblings_of(path).count();
                #[cfg(todo = "unnecessary")]
                let mut child_cap = amt / 4;

                let cap = 0x40;
                let cap = match amt.checked_ilog2() {
                    Some(est) => {
                        let depth_rem = est.saturating_sub(depth) + 1;
                        #[cfg(todo = "unnecessary")]
                        child_cap = 2usize.ipow2(depth_rem).min(self.all.len());
                        (depth_rem + 1) * 2
                    },
                    None => cap,
                };
                (
                    Vec::with_capacity(cap),
                    #[cfg(todo = "unnecessary")]
                    CategorySet::with_capacity(child_cap),
                )
            },
            Some(..) if firstborn.is_some() =>
                Vec::with_capacity(0x40),
            _ => Vec::new(),
        };
        if let Some(firstborn) = firstborn {
            stack.push(CategoryPath::with_path(firstborn));
        }
        iter::from_fn(move || {
            loop {
                let next = stack.pop()?;
                match cycle_limit.checked_sub(1) {
                    Some(l) =>
                        cycle_limit = l,
                    None => {
                        // who knows, mistakes and/or corruption can happen!
                        log::error!("category descendents exceeded cycle limit, stuck at {next} while {} deep", stack.len());
                        return None
                    },
                }
                let Some(next_info) = self.info_of(next) else { continue };
                if let Some(sibling) = next_info.sibling() {
                    stack.push(CategoryPath::with_path(sibling));
                }
                if let Some(child) = next_info.child() {
                    stack.push(CategoryPath::with_path(child));
                }
                break Some(next)
            }
        })
    }

    /// TODO: rename to ancestors_of
    pub fn parents_of(&self, path: CategoryPath) -> impl Iterator<Item = CategoryPath> + '_ {
        let mut next = self.parent_of(path);
        iter::from_fn(move || {
            let current = next.take()?;
            next = self.parent_of(current);
            Some(current)
        })
    }

    pub fn younger_siblings_of(&self, path: CategoryPath) -> impl Iterator<Item = CategoryPath> + '_ {
        let mut next = self.sibling_of(path);
        iter::from_fn(move || {
            let current = next.take()?;
            next = self.sibling_of(current);
            Some(current)
        })
    }

    pub fn parent_of(&self, path: CategoryPath) -> Option<CategoryPath> {
        self.info_of(path).and_then(|c| c.parent())
            .map(CategoryPath::with_path)
    }
    pub fn sibling_of(&self, path: CategoryPath) -> Option<CategoryPath> {
        self.info_of(path).and_then(|c| c.sibling())
            .map(CategoryPath::with_path)
    }
    pub fn firstborn_of(&self, path: CategoryPath) -> Option<CategoryPath> {
        self.info_of(path).and_then(|c| c.child())
            .map(CategoryPath::with_path)
    }

    pub fn disabled(&self) -> impl Iterator<Item = CategoryPath> + Clone + '_ {
        self.disabled.iter()
            .map(CategoryPath::with_path)
    }
    pub fn hidden(&self) -> impl Iterator<Item = CategoryPath> + Clone + '_ {
        self.hidden.iter()
            .map(CategoryPath::with_path)
    }
    pub fn separators(&self) -> impl Iterator<Item = CategoryPath> + Clone + '_ {
        self.separators.iter()
            .map(CategoryPath::with_path)
    }
}

/// TODO: anything else interesting about the root category?
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackRoot {
    pub index: CategoryIndex,
    pub id: CategoryId,
    pub hidden: bool,
    pub separator: bool,
    pub display_name: Arc<str>,
    pub child_count: usize,
}

impl PackRoot {
    pub fn from_category(path: CategoryPath, category: &Category) -> Self {
        #[cfg(todo = "unnecessary")]
        if category.full_id != category.id {
            return None
        }
        Self {
            index: path.path,
            id: category.full_id.clone(),
            display_name: category.display_name.clone(),
            hidden: category.is_hidden(),
            separator: category.is_separator(),
            child_count: category.sub_categories.len(),
        }
    }

    pub fn path(&self) -> CategoryPath {
        CategoryPath::with_path(self.index)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackCategory {
    pub sibling: CategoryIndex,
    pub child: CategoryIndex,
    pub parent: CategoryIndex,
}

pub(crate) fn category_index_get(index: CategoryIndex) -> Option<CategoryIndex> {
    match index {
        CategoryIndex::MAX => None,
        index => Some(index),
    }
}
#[inline]
pub(crate) fn category_index_set(index: Option<CategoryIndex>) -> CategoryIndex {
    index.unwrap_or(CategoryIndex::MAX)
}
impl PackCategory {
    pub const EMPTY: Self = Self {
        sibling: CategoryIndex::MAX,
        child: CategoryIndex::MAX,
        parent: CategoryIndex::MAX,
    };

    pub fn sibling(&self) -> Option<CategoryIndex> {
        category_index_get(self.sibling)
    }
    pub fn set_sibling(&mut self, index: Option<CategoryIndex>) {
        self.sibling = category_index_set(index);
    }
    pub fn child(&self) -> Option<CategoryIndex> {
        category_index_get(self.child)
    }
    pub fn set_child(&mut self, index: Option<CategoryIndex>) {
        self.child = category_index_set(index);
    }
    pub fn parent(&self) -> Option<CategoryIndex> {
        category_index_get(self.parent)
    }
    pub fn set_parent(&mut self, index: Option<CategoryIndex>) {
        self.parent = category_index_set(index);
    }

    pub fn build(collection: &CategoryCollection) -> Vec<Self> {
        let mut cats = Vec::with_capacity(collection.all_categories.len());
        cats.resize(collection.all_categories.len(), PackCategory::EMPTY);
        for (idx, (_name, category)) in collection.all_categories.iter().enumerate() {
            let path: CategoryPath = CategoryPath::with_path(idx as CategoryIndex);
            let mut children = category.child_ids().filter_map(|child_full_id| match collection.all_categories.get_full(child_full_id) {
                None => {
                    log::warn!("child category {child_full_id} of {_name} not found");
                    None
                },
                Some((child_index, _child_full_id, _child)) => {
                    Some(child_index as CategoryIndex)
                },
            });
            let Some(mut child_index) = children.next() else {
                // empty or leaf category, nothing else to do here
                continue
            };
            match cats.get_mut(idx) {
                Some(cat) if cat.child().is_none() => {
                    cat.set_child(Some(child_index));
                },
                _ => (),
            };
            loop {
                if let Some(child) = cats.get_mut(child_index as usize) {
                    match child.parent() {
                        Some(p) if p != path.path => {
                            log::warn!("category {_name} child#{child_index} already has different parent #{p}?");
                        },
                        Some(..) => (),
                        None => {
                            child.set_parent(Some(path.path));
                        },
                    }
                    #[cfg(todo = "unnecessary")]
                    if let Some(sibling) = child.sibling() {
                        child_index = s;
                        continue
                    }
                    let Some(next_child) = children.next() else { break };
                    child.set_sibling(Some(next_child));
                    child_index = next_child;
                }
            }
        }
        cats
    }
}

impl Default for PackCategory {
    fn default() -> Self {
        Self::EMPTY
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackConfig {
    /// xor with defaults
    pub category_visibility: BTreeMap<CategoryPath, VisibilityFlags>,
    #[cfg(todo = "unnecessary")]
    pub category_visibility: VisibilityFlagSet,
    /// force specific subtrees to a set state
    pub visibility_overrides: CategorySet,
}

impl PackConfig {
    pub fn fill_settings(&mut self, pack: &Pack, pathing: &PathingSettings, disabled_paths: &HashSet<String>) {
        for id in disabled_paths {
            let id = &id[..];
            let Some((i, _id, cat)) = pack.categories.all_categories.get_full(id) else { continue };
            let path = CategoryPath::with_path(i as CategoryIndex);
            let settings_vis = VisibilityFlags::visible(false);
            let default_vis = VisibilityFlags::from_pack_category(&cat);
            let deviation = settings_vis ^ (default_vis & VisibilityFlags::TOGGLE);
            if !deviation.is_empty() {
                self.category_visibility.insert(path, deviation);
            }
        }
        #[cfg(todo)]
        let disabled_compat = pathing.disabled_compat;
        let disabled_compat = true;
        if disabled_compat {
            let disabled_cats = pack.categories.all_categories.iter().enumerate()
                .filter(|(_, (_, cat))| !cat.default_toggle());
            for (i, (full_id, _disabled_cat)) in disabled_cats {
                let path = CategoryPath::with_path(i as CategoryIndex);
                if !disabled_paths.contains(&full_id.id_to_str()[..]) {
                    let mut vis = self.category_visibility.get(&path)
                        .copied()
                        .unwrap_or(VisibilityFlags::empty());
                    vis.insert(VisibilityFlags::TOGGLE);
                    self.category_visibility.insert(path, vis);
                }
            }
        }
        // TODO: new per-flag settings and override list
    }

    /// Indicates a configuration that deviates from the defaults (XOR)
    pub fn visibility_deviation_for(&self, path: CategoryPath) -> VisibilityFlags {
        self.category_visibility.get(&path)
            .copied()
            .unwrap_or(VisibilityFlags::empty())
    }

    pub fn set_visibility_deviation(&mut self, path: CategoryPath, value: VisibilityFlags) {
        //self.category_visibility.extend_for(path, false);
        if value.is_empty() {
            self.category_visibility.remove(&path);
        } else {
            self.category_visibility.insert(path, value);
        }
    }

    /// if false, indicates the pack is disabled (all roots are disabled)
    pub fn any_enabled(&self, categories: &PackCategoryInfo) -> bool {
        if categories.roots.is_empty() {
            // empty pack? *shrug*
            return true
        }
        categories.root_paths()
            .any(|path| {
                let default_toggle = !categories.disabled.contains(path);
                let deviation = self.visibility_deviation_for(path).is_visible();
                default_toggle ^ deviation
            })
    }
}

#[derive(Debug, Clone)]
pub enum UnloadedReason {
    Disabled,
    /// Reserved index will not be reused
    Gravestone,
    Pending,
    Loading,
    UnknownFormat,
    LoadingFailed(Arc<dyn StdError + Send + Sync>),
}

impl UnloadedReason {
    pub fn can_reload(&self) -> bool {
        match self {
            UnloadedReason::Gravestone | UnloadedReason::Disabled | UnloadedReason::Loading =>
                false,
            _ => true,
        }
    }

    fn discriminant(&self) -> u8 {
        match self {
            Self::Pending => 1,
            Self::Loading => 2,
            Self::Disabled => 3,
            Self::Gravestone => 4,
            Self::UnknownFormat => 5,
            Self::LoadingFailed(..) => 6,
        }
    }
}

impl Eq for UnloadedReason {}
impl PartialEq for UnloadedReason {
    fn eq(&self, rhs: &Self) -> bool {
        match (self, rhs) {
            | (Self::Pending, Self::Pending)
            | (Self::Loading, Self::Loading)
            | (Self::Disabled, Self::Disabled)
            | (Self::Gravestone, Self::Gravestone)
            | (Self::UnknownFormat, Self::UnknownFormat)
            =>
                true,
            (Self::LoadingFailed(e), Self::LoadingFailed(rhs)) => Arc::ptr_eq(e, rhs),
            _ => false,
        }
    }
}
impl Ord for UnloadedReason {
    fn cmp(&self, rhs: &Self) -> cmp::Ordering {
        let d = self.discriminant().cmp(&rhs.discriminant());
        match (d, self, rhs) {
            (cmp::Ordering::Equal, Self::LoadingFailed(lhs), Self::LoadingFailed(rhs)) => Arc::as_ptr(lhs).cast::<()>().cmp(&Arc::as_ptr(rhs).cast::<()>()),
            (cmp, ..) => cmp,
        }
    }
}
impl PartialOrd for UnloadedReason {
    fn partial_cmp(&self, rhs: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(rhs))
    }
}
impl hash::Hash for UnloadedReason {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        let e = match self {
            Self::LoadingFailed(e) => Arc::as_ptr(e) as *const (),
            _ => ptr::null(),
        };
        (self.discriminant(), e).hash(state)
    }
}

impl fmt::Display for UnloadedReason {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Disabled =>
                f.write_str("disabled"),
            Self::Gravestone =>
                f.write_str("removed"),
            Self::Pending =>
                f.write_str("not yet loaded"),
            Self::Loading =>
                f.write_str("loading"),
            Self::UnknownFormat =>
                f.write_str("expected TacO zip or folder"),
            Self::LoadingFailed(e) =>
                write!(f, "{e:#}"),
        }
    }
}

impl StdError for UnloadedReason {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::LoadingFailed(e) => e.source(),
            _ => None,
        }
    }
}
