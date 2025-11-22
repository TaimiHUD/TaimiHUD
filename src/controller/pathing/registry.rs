use {
    super::{festivals::FestivalFixup, visible::{VisibilityFlagSet, VisibilityFlags}}, crate::{
        controller::Controller,
        exports::runtime::{self as rt, locator::{LocationMut, LocationRef}, Locator},
        settings::{DataSourcePath, PathingSettings, SettingsLock},
    }, anyhow::{anyhow, Context}, bitvec::vec::BitVec, futures::{future, stream::{self, FusedStream, Stream, StreamExt}, FutureExt}, std::{cmp, collections::{btree_set, BTreeMap, BTreeSet, HashSet}, error::Error as StdError, fmt, future::Future, hash, iter, mem, num::NonZero, path::{Path, PathBuf}, ptr, sync::{Arc, Weak}}, taimi_meta::map::MapID, taimi_pack::{
        attributes::Festival, category::{id::{AsFullId, CategoryId}, Category}, loader::{DirectoryLoader, PackLoaderContext, ZipLoader}, pack::CategoryCollection, trail::TrailData, Pack
    }, tokio::sync::{watch, Mutex},
    tokio_util::sync::ReusableBoxFuture,
};
#[cfg(doc)]
use taimi_pack::attributes::keys;

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

    pub fn all_packs<'a>(&'a self) -> impl Iterator<Item = (PackPath, &'a LoadedPack)> {
        self.packs.iter().enumerate()
            .map(|(path, pack)| (PackPath::with_path(path as PackIndex), pack))
    }
    pub fn all_packs_mut<'a>(&'a mut self) -> impl Iterator<Item = (PackPath, &'a mut LoadedPack)> {
        self.packs.iter_mut().enumerate()
            .map(|(path, pack)| (PackPath::with_path(path as PackIndex), pack))
    }
    pub fn active_packs<'a>(&'a self) -> impl Iterator<Item = (PackPath, &'a LoadedPack, &'a Arc<ActivePack>)> {
        self.all_packs().filter_map(|(path, pack)| match &pack.active {
            Some(active) => Some((path, pack, active)),
            None => None,
        })
    }
    pub fn unloaded_packs<'a>(&'a self) -> impl Iterator<Item = (PackPath, &'a LoadedPack, Result<&'a Arc<PackInfo>, &'a UnloadedReason>)> {
        self.all_packs().filter_map(|(path, pack)| match &pack.info.info {
            Err(reason) => Some((path, pack, Err(reason))),
            Ok(info) if pack.active.is_none() => Some((path, pack, Ok(info))),
            Ok(..) => None,
        })
    }

    pub fn packs_for_map(&self, map_id: MapIndex) -> impl Iterator<Item = (PackPath, &LoadedPack)> {
        self.all_packs()
            .filter(move |(_, pack)| match &pack.info.info {
                Ok(info) => info.maps.contains(map_id),
                _ => false,
            })
    }

    pub const CONCURRENT_LOAD_LIMIT: usize = 8;
    pub fn load_packs_for_map<'a, 'm>(&'a mut self, manager: &'m PackLoader, map_id: MapIndex) -> impl Stream<Item = (PackPath, &'a mut LoadedPack)> + 'm where
        'a: 'm,
    {
        stream::iter(self.all_packs_mut())
            .map(move |(path, pack)| async move {
                match pack.activate(manager).await {
                    Ok(..) => match &pack.info.info {
                        Ok(info) if info.maps.contains(map_id) =>
                            Some((path, pack)),
                        _ => None,
                    },
                    Err(e) => {
                        log::error!("{e:#}");
                        None
                    },
                }
            }).buffer_unordered(Self::CONCURRENT_LOAD_LIMIT)
            .filter_map(|res| future::ready(res))
    }

    pub fn preload<P>(&mut self, path: P, datasource: Option<DataSourcePath>, manager: &PackLoader) -> PackPath where
        P : AsRef<Path> + Into<PathBuf>,
    {
        if let Some((i, pack)) = self.packs.iter_mut().enumerate().find(|(_, p)| p.info.path.as_ref() == path.as_ref()) {
            // ?
            pack.info.datasource = datasource;
            let path = PackPath::with_path(i as PackIndex);
            manager.shared_update_pack_info(path, &pack.info);
            return path
        }

        let i = self.packs.len();
        let index = PackPath::with_path(i as PackIndex);
        self.packs.push(LoadedPack::new_unloaded(index, path.into(), datasource));
        if let Some(pack) = self.packs.last() {
            // dumb if :<
            manager.shared_update_pack_info(pack.info.index, &pack.info);
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

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackRegistryNs;
pub type PackIndex = u16;
pub type PackPath<N = PackRegistryNs> = Locator<N, PackIndex>;
impl fmt::Display for PackRegistryNs {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("controller/packs")
    }
}
impl LocationRef<PackRegistryNs, PackIndex> for PackRegistry {
    type LookupRef = LoadedPack;

    fn lookup_ref<'a>(&'a self, loc: &Locator<PackRegistryNs, PackIndex>) -> Option<&'a Self::LookupRef> {
        self.packs.get(loc.path as usize)
    }
}
impl LocationMut<PackRegistryNs, PackIndex> for PackRegistry {
    fn lookup_mut<'a>(&'a mut self, loc: &Locator<PackRegistryNs, PackIndex>) -> Option<&'a mut Self::LookupRef> {
        self.packs.get_mut(loc.path as usize)
    }
}
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackCategoryNs;
pub type CategoryIndex = u32;
pub type CategoryPath<N = PackCategoryNs> = Locator<N, CategoryIndex>;
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackPoiNs;
pub type PoiIndex = u32;
pub type PoiPath<N = PackPoiNs> = Locator<N, PoiIndex>;
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackTrailNs;
pub type TrailIndex = u16;
pub type TrailPath<N = PackTrailNs> = Locator<N, TrailIndex>;
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackTrailSectionNs;
pub type TrailSectionIndex = u16;
pub type TrailSectionPath<N = PackTrailSectionNs> = Locator<N, TrailSectionIndex>;
pub type MapIndex = NonZero<MapID>;
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MapNs;
pub type MapPath<N = MapNs> = Locator<N, MapIndex>;
pub type PackMapPath<N = PackPath> = MapPath<N>;
impl fmt::Display for PackCategoryNs {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("pack/category")
    }
}
impl fmt::Display for PackPoiNs {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("pack/poi")
    }
}
impl fmt::Display for PackTrailNs {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("pack/trail")
    }
}
impl fmt::Display for MapNs {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("map")
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
        manager.shared_update_pack_active(self.info.index, None);
    }

    pub fn mark_reload(&mut self, manager: &PackLoader) {
        if let Err(reason) = &mut self.info.info {
            *reason = UnloadedReason::Pending;
            manager.shared_update_pack_info(self.info.index, &self.info);
        }
        self.deactivate(manager);
    }

    pub fn activate_start(&mut self) -> anyhow::Result<Option<(PackFormat, String, Option<bool>)>> {
        if self.active.is_some() {
            return Ok(None)
        }

        let (format, context) = match &self.info.info {
            Ok(info) => (
                Some(info.format),
                format!("Reloading pack {info}"),
            ),
            Err(UnloadedReason::Pending) => {
                let name = self.info.path.file_name()
                    .map(Path::new)
                    .unwrap_or_else(|| rt::relative_path(&self.info.path));
                (
                    PackFormat::guess_for_path(&self.info.path),
                    format!("Loading pack {}", name.display()),
                )
            },
            Err(_reason) => return Ok(None),
        };

        match format {
            Some(format) => {
                let wants_config = match &self.config {
                    None => Some(true),
                    #[cfg(todo)]
                    Some(config) if !config.borrow().is_empty() => Some(false),
                    _ => None,
                };
                Ok(Some((format, context, wants_config)))
            },
            None => {
                self.info.info = Err(UnloadedReason::UnknownFormat);
                Err(anyhow!("unknown pack format").context(context))
            },
        }
    }

    pub async fn activate_load((format, context, wants_config): (PackFormat, String, Option<bool>), path: PathBuf, manager: &PackLoader) -> anyhow::Result<(Arc<ActivePack>, Arc<PackInfo>, Option<PackConfig>)> {
        let loader = format.loader_for_path(path).await;
        let pack = match loader {
            Ok(loader) => manager.load_pack(loader).await,
            Err(e) => Err(e),
        }.context(context);
        let res = pack.map(|pack| {
            let info = PackInfo::from_pack(&pack.pack, format);
            (pack, Arc::new(info))
        });

        match res {
            Ok((pack, info)) => {
                let settings = match wants_config {
                    Some(true) => Some(manager.settings.read().await),
                    None => manager.settings.try_read().ok(),
                    Some(false) => None,
                };
                let config = settings.map(|settings| {
                    let mut config = PackConfig::default();
                    config.fill_settings(&pack.pack, &settings.pathing(), &settings.disabled_paths);
                    config
                });
                Ok((Arc::new(pack), info, config))
            },
            Err(e) => Err(e),
        }
    }

    pub fn activate_finish(&mut self, pack: anyhow::Result<(Arc<ActivePack>, Arc<PackInfo>, Option<PackConfig>)>, manager: &PackLoader) -> anyhow::Result<()> {
        match pack {
            Err(e) => {
                #[cfg(todo)]
                let e = e.into_boxed_dyn_error().into();
                let e: Arc<dyn StdError + Send + Sync> = Box::<dyn StdError + Send + Sync>::from(e).into();
                self.info.info = Err(UnloadedReason::LoadingFailed(e.clone()));
                manager.shared_update_pack_info(self.info.index, &self.info);
                Err(e.into())
            },
            Ok((pack, info, config)) => {
                self.info.info = Ok(info);
                let active = self.active.insert(pack);
                if let Some(config) = config {
                    let update_config = self.config.is_none();
                    Self::try_update_config_inner(&mut self.config, config);
                    if update_config {
                        manager.shared_update_pack_config(self.info.index, self.config.as_ref());
                    }
                }
                manager.shared_update_pack_active(self.info.index, Some(active));
                Ok(())
            },
        }
    }

    pub async fn activate(&mut self, manager: &PackLoader) -> anyhow::Result<Option<()>> {
        let activate = match self.activate_start()? {
            None => return Ok(None),
            Some(activate) => activate,
        };
        let res = Self::activate_load(activate, self.info.path.to_path_buf(), manager).await;
        self.activate_finish(res, manager).map(Some)
    }

    #[cfg(todo = "unused")]
    pub fn try_update_config(&mut self, config: PackConfig) {
        Self::try_update_config_inner(&mut self.config, config)
    }
    fn try_update_config_inner(out: &mut Option<watch::Sender<Arc<PackConfig>>>, config: PackConfig) {
        match out {
            out @ None => {
                let _ = out.insert(watch::Sender::new(Arc::new(config)));
            },
            Some(out) => {
                out.send_if_modified(|out| {
                    if **out == config {
                        return false
                    }
                    *Arc::make_mut(out) = config;
                    true
                });
            },
        }
    }
    #[cfg(todo)]
    pub async fn try_populate_config(&mut self, settings: &SettingsLock) -> bool {
        #[cfg(todo = "unnecessary")]
        let Ok(info) = &self.info else { return false };
        let Some(active) = &self.active else { return false };
        Self::populate_config(&mut self.config, active, settings);
        true
    }
    #[cfg(todo)]
    pub async fn populate_config(out: &mut Option<watch::Sender<Arc<PackConfig>>>, active: &ActivePack, settings: &SettingsLock) {
        let mut config = PackConfig::default();
        {
            let settings = settings.write().await;
            config.fill_settings(&active.pack, &settings.pathing(), &settings.disabled_paths);
        }
        Self::try_update_config_inner(out, config)
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

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PackFormat {
    TacoZip,
    TacoDir,
}

impl PackFormat {
    pub fn guess_for_path(path: &Path) -> Option<Self> {
        let is_taco = path
            .extension()
            .map(|e| e.eq_ignore_ascii_case("taco") || e.eq_ignore_ascii_case("zip"));
        match is_taco {
            _ if path.is_dir() => Some(Self::TacoDir),
            Some(true) => Some(Self::TacoZip),
            _ => None,
        }
    }

    pub async fn loader_for_path(&self, path: PathBuf) -> anyhow::Result<LoaderBox> {
        match self {
            Self::TacoZip => {
                let context = "Loading TacO zip";
                let loader = move || ZipLoader::new(&path)
                    .with_context(|| {
                        let path = path.file_name()
                            .map(Path::new)
                            .unwrap_or_else(|| rt::relative_path(&path));
                        format!("{context} {}", path.display())
                    });
                Controller::try_run_blocking(context, loader).await
                    .map(|loader| Box::new(loader) as LoaderBox)
            },
            Self::TacoDir => Ok(Box::new(DirectoryLoader::new(path))),
        }
    }
}

impl fmt::Display for PackFormat {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::TacoZip =>
                f.write_str("TacO zip archive"),
            Self::TacoDir =>
                f.write_str("TacO folder"),
        }
    }
}

pub type SharedLoaderPackInfo = Box<[LoadedPackInfo]>;
pub type SharedLoaderPackData = Box<[Weak<ActivePack>]>;
pub type SharedLoaderPackConfig = Box<[Option<watch::Sender<Arc<PackConfig>>>]>;
#[derive(Debug)]
pub struct PackLoader {
    pub settings: SettingsLock,
    pub festival_categories: BTreeMap<&'static str, Festival>,

    pub shared_pack_info: watch::Sender<SharedLoaderPackInfo>,
    pub shared_pack_data: watch::Sender<SharedLoaderPackData>,
    pub shared_pack_config: watch::Sender<SharedLoaderPackConfig>,
}

impl PackLoader {
    pub fn new(settings: SettingsLock) -> Self {
        Self {
            settings,
            festival_categories: FestivalFixup::festival_categories(),
            shared_pack_info: Default::default(),
            shared_pack_data: Default::default(),
            shared_pack_config: Default::default(),
        }
    }

    pub async fn load_pack(&self, mut loader: LoaderBox) -> anyhow::Result<ActivePack> {
        let context = "loading TacO pack";
        let (mut pack, loader) = Controller::try_run_blocking(context, move || Pack::load(&mut loader)
            .context(context)
            .map(move |pack| (pack, loader))
        ).await?;

        self.fixup_pack(&mut pack);
        Ok(ActivePack::with_pack(Arc::new(pack), loader))
    }

    fn fixup_pack(&self, pack: &mut Pack) {
        let mut fixed_festival_categories = BTreeMap::new();
        for (_name, category) in &mut pack.categories.all_categories {
            let is_festival = FestivalFixup::FESTIVAL_PREFIXES
                .iter()
                .copied()
                .find(|&prefix| category.full_id.id_starts_with(prefix));
            match is_festival {
                Some(prefix) if category.full_id.as_id() != prefix => (),
                _ => continue,
            }
            let festival = self
                .festival_categories
                .iter()
                .find_map(|(&prefix, &fest)| category.full_id.id_starts_with(prefix).then_some(fest));
            if let Some(festival) = festival {
                let festivals = Arc::make_mut(&mut category.marker_attributes).festivals.insert(festival.into());
                fixed_festival_categories.insert(&category.full_id, festivals.clone());
            } else {
                log::info!("unrecognized festival category: `{}`", category.full_id);
            }
        }
        if !fixed_festival_categories.is_empty() {
            // TODO: this should be less necessary once a tree of attribute inherits exist...
            let pois = pack.pois.iter_mut().filter_map(|poi| match fixed_festival_categories.get(poi.category.as_id()) {
                Some(f) => Some((poi, f)),
                None => None,
            });
            for (poi, f) in pois {
                if poi.attributes.festivals.is_none() {
                    poi.attributes.festivals = Some(f.clone());
                }
            }
            let trails = pack.trails.iter_mut().filter_map(|trail| match fixed_festival_categories.get(trail.category.as_id()) {
                Some(f) => Some((trail, f)),
                None => None,
            });
            for (trail, f) in trails {
                if trail.attributes.festivals.is_none() {
                    trail.attributes.festivals = Some(f.clone());
                }
            }
        }
    }

    pub fn shared_packs<I: IntoIterator>(packs: I) -> impl Iterator<Item = (PackPath, I::Item)> {
        packs.into_iter().enumerate()
            .map(|(i, p)| (PackPath::with_path(i as PackIndex), p))
    }

    pub fn shared_pack_at<D>(packs: &[D], path: PackPath) -> Option<&D> {
        let idx = path.path as usize;
        packs.get(idx)
    }
    pub fn try_shared_pack_active(packs: &[Weak<ActivePack>], path: PackPath) -> Option<Arc<ActivePack>> {
        Self::shared_pack_at(packs, path)
            .and_then(Weak::upgrade)
    }
    /// TODO: keeping this as a marker to indicate the loader may be able to load on-demand later on
    /// (and anything still using this probably wants to switch to that instead)
    pub fn shared_pack_active(packs: &[Weak<ActivePack>], path: PackPath) -> Option<Arc<ActivePack>> {
        Self::try_shared_pack_active(packs, path)
    }

    pub fn shared_update_pack_info(&self, path: PackPath, pack: &LoadedPackInfo) {
        self.shared_pack_info.send_if_modified(|shared| {
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

    pub fn shared_update_pack_config(&self, path: PackPath, config: Option<&watch::Sender<Arc<PackConfig>>>) {
        let Some(config) = config else { return };
        let idx = path.path as usize;
        self.shared_pack_config.send_if_modified(|shared| match shared.get_mut(idx) {
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

    pub fn shared_update_pack_active(&self, path: PackPath, pack: Option<&Arc<ActivePack>>) {
        self.shared_pack_data.send_if_modified(|shared| {
            let pack_shared = || pack.map(Arc::downgrade).unwrap_or(Weak::new());
            let idx = path.path as usize;
            match shared.get_mut(idx) {
                Some(out) => {
                    match pack.map(Arc::as_ptr) {
                        Some(p) if p == Weak::as_ptr(out) =>
                            false,
                        None if Weak::ptr_eq(&*out, &Weak::new()) =>
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
}

#[derive(Clone)]
pub struct ActivePack {
    pub pack: Arc<Pack>,
    pub loader: Arc<Mutex<LoaderBox>>,
}

impl ActivePack {
    pub fn with_pack(pack: Arc<Pack>, loader: LoaderBox) -> Self {
        Self {
            pack,
            loader: Arc::new(Mutex::new(loader)),
        }
    }

    pub fn read_trail_data(&self, index: TrailIndex) -> anyhow::Result<TrailData> {
        let Some(trail) = self.pack.trails.get(index as usize) else {
            anyhow::bail!("Trail #{index} not found in {self}")
        };

        let mut loader = self.loader.blocking_lock();
        trail.read_trl_data(&mut *loader)
            .with_context(|| format!("Reading trail {trail} vertices from {self}"))
    }

    pub fn load_trail_data(&self, index: TrailIndex) -> impl Future<Output = anyhow::Result<TrailData>> + Send + 'static {
        let pack = self.clone();
        Controller::try_run_blocking("reading trl", move || pack.read_trail_data(index))
    }

    pub fn iter_weak<'a>(active: &'a [Weak<Self>]) -> impl Iterator<Item = (PackPath, &'a Weak<Self>)> {
        active.into_iter().enumerate().map(|(i, a)| (
            PackPath::with_path(i as PackIndex),
            a,
        ))
    }
    pub fn iter_strong<'a>(active: &'a [Weak<Self>]) -> impl Iterator<Item = (PackPath, Arc<Self>)> + 'a {
        active.into_iter().enumerate().filter_map(|(i, a)| a.upgrade().map(|a| (
            PackPath::with_path(i as PackIndex),
            a,
        )))
    }
    pub fn enum_strong<'a>(active: &'a [Weak<Self>]) -> impl Iterator<Item = (PackPath, bool)> + 'a {
        active.into_iter().enumerate().map(|(i, a)| (
            PackPath::with_path(i as PackIndex),
            a.strong_count() > 0,
        ))
    }
}

impl fmt::Display for ActivePack {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.pack.name, f)
    }
}
impl fmt::Debug for ActivePack {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("ActivePack")
            .field(&self.pack)
            .finish()
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
    /// [keys::CopyValue] is valid on [separators](keys::Separators)
    pub copyable: CategorySet,
    /// Categories that lack any marker children, toggling would be meaningless.
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

    pub fn children_of(&self, path: CategoryPath) -> impl Iterator<Item = CategoryPath> + '_ {
        let mut next = self.info_of(path).and_then(|c| c.child());
        iter::from_fn(move || {
            let current = CategoryPath::with_path(next.take()?);
            next = self.info_of(current).and_then(|c| c.sibling());
            Some(current)
        })
    }
    pub fn parents_of(&self, path: CategoryPath) -> impl Iterator<Item = CategoryPath> + '_ {
        let mut next = self.info_of(path).and_then(|c| c.parent());
        iter::from_fn(move || {
            let current = CategoryPath::with_path(next.take()?);
            next = self.info_of(current).and_then(|c| c.parent());
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
}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MapSet(BTreeSet<MapID>);
#[cfg(todo)]
pub struct MapSet(BitVec);

impl MapSet {
    pub fn contains<M: Into<MapID>>(&self, map: M) -> bool {
        self.0.contains(&map.into())
    }
}

impl FromIterator<MapID> for MapSet {
    fn from_iter<I: IntoIterator<Item = MapID>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CategorySet(BTreeSet<CategoryIndex>);

impl CategorySet {
    pub fn insert_index<C: Into<CategoryIndex>>(&mut self, index: C) -> bool {
        self.0.insert(index.into())
    }
    /// false indicates the value was already present
    pub fn insert<N>(&mut self, path: CategoryPath<N>) -> bool {
        self.insert_index(path.path)
    }
    pub fn remove_index<C: Into<CategoryIndex>>(&mut self, index: C) -> bool {
        self.0.remove(&index.into())
    }
    pub fn remove<N>(&mut self, path: CategoryPath<N>) -> bool {
        self.remove_index(path.path)
    }
    pub fn contains_index<C: Into<CategoryIndex>>(&self, index: C) -> bool {
        self.0.contains(&index.into())
    }
    pub fn contains<N>(&self, path: CategoryPath<N>) -> bool {
        self.contains_index(path.path)
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter<'a>(&'a self) -> <&'a Self as IntoIterator>::IntoIter {
        IntoIterator::into_iter(self)
    }
    pub fn paths<'a>(&'a self) -> impl Iterator<Item = CategoryPath> + Clone + 'a {
        self.iter().map(CategoryPath::with_path)
    }
    pub fn into_paths(self) -> impl Iterator<Item = CategoryPath> {
        self.into_iter().map(CategoryPath::with_path)
    }
}

impl IntoIterator for CategorySet {
    type Item = CategoryIndex;
    type IntoIter = btree_set::IntoIter<CategoryIndex>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}
impl<'a> IntoIterator for &'a CategorySet {
    type Item = CategoryIndex;
    type IntoIter = iter::Copied<btree_set::Iter<'a, CategoryIndex>>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter().copied()
    }
}

impl FromIterator<CategoryIndex> for CategorySet {
    fn from_iter<I: IntoIterator<Item = CategoryIndex>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}
impl FromIterator<Option<CategoryIndex>> for CategorySet {
    fn from_iter<I: IntoIterator<Item = Option<CategoryIndex>>>(iter: I) -> Self {
        Self(iter.into_iter().filter_map(|c| c).collect())
    }
}
impl Extend<CategoryIndex> for CategorySet {
    fn extend<I: IntoIterator<Item = CategoryIndex>>(&mut self, iter: I) {
        self.0.extend(iter)
    }
}
impl<N> Extend<Locator<N, CategoryIndex>> for CategorySet {
    #[inline]
    fn extend<I: IntoIterator<Item = Locator<N, CategoryIndex>>>(&mut self, iter: I) {
        self.extend(iter.into_iter().map(Locator::into_path))
    }
}
impl Extend<Option<CategoryIndex>> for CategorySet {
    fn extend<I: IntoIterator<Item = Option<CategoryIndex>>>(&mut self, iter: I) {
        self.0.extend(iter.into_iter().filter_map(|c| c))
    }
}

#[derive(Debug, Clone)]
pub enum UnloadedReason {
    #[cfg(todo = "unused")]
    Disabled,
    Pending,
    UnknownFormat,
    LoadingFailed(Arc<dyn StdError + Send + Sync>),
}

impl UnloadedReason {
    fn discriminant(&self) -> u8 {
        match self {
            Self::Pending => 1,
            Self::UnknownFormat => 2,
            Self::LoadingFailed(..) => 3,
        }
    }
}

impl Eq for UnloadedReason {}
impl PartialEq for UnloadedReason {
    fn eq(&self, rhs: &Self) -> bool {
        match (self, rhs) {
            (Self::Pending, Self::Pending) | (Self::UnknownFormat, Self::UnknownFormat) =>
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
            Self::Pending =>
                f.write_str("not yet loaded"),
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

pub type LoaderBox = Box<dyn PackLoaderContext + Send + 'static>;

/// A poor man's LRU cache
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RecentlyUsed {
    pub generation: u32,
}

impl RecentlyUsed {
    pub const DEFAULT: Self = Self {
        generation: 0,
    };

    pub fn mark_used(&mut self) {
        self.generation = 0;
    }

    pub fn mark_unused(&mut self) {
        self.generation = self.generation.saturating_add(1);
    }

    pub fn is_elderly(&self, threshold: u32) -> bool {
        self.generation > threshold
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LoadedPackInfo {
    pub index: PackPath,
    pub path: Arc<Path>,
    pub info: Result<Arc<PackInfo>, UnloadedReason>,
    pub datasource: Option<DataSourcePath>,
}
