use super::registry::SharedLoaderBox;

use {
    crate::{
        controller::pathing::{
            PathingController, PathingEvent,
            visible::LoadedMapPack,
            shared::{SharedPackInfo, SharedPackLoad, PathingShared, MapPackInfo, SharedPackLoaded},
            registry::{PackLoader, PackActivateContext, UnloadedReason, PackInfo},
        },
        exports::runtime as rt,
        controller::Controller,
        render::{machine::RenderTaskPriority, RenderState},
        settings::{Settings, SettingsLock, SourceKind},
    }, anyhow::Context, futures::StreamExt, std::sync::Arc,
    taimi_sync::watched::watch,
    taimi_hoard::loc::{Locator, LocationRef},
    taimi_meta::packs::{PackPath, PackMapPath, MapIndex, collections::PackSet},
    taimi_pack::Pack,
    std::collections::BTreeSet,
    std::future::Future,
    std::iter,
    std::path::Path,
    tokio::fs::create_dir_all,
};
#[cfg(todo)]
use {
    crate::{
        exports::runtime::{self as rt, locator::LocationMut, Locator},
        controller::pathing::{
            registry::{
                LoadedPack, PackLoader, PackPath,
                PackRegistry, MapIndex, PackMapPath,
            },
            state::{
                MapPackInfoStorage,
                info::MapPackInfo,
                visible::LoadedMapPack,
            },
        },
    },
};
#[cfg(todo)]
pub use self::{
    reactor::{PathingEventContext, PathingEvent, PathingTaskBox},
};

#[cfg(todo)]
mod config;

impl PathingController {
    #[cfg(deleteme)]
    pub(super) async fn handle_pack_loaded(&mut self, path: PackPath, loaded: Result<PackActivateLoaded, Option<UnloadedReason>>) {
    }

    pub(super) fn preload_all(&self) -> impl Future<Output = ()> + Send + 'static {
        Self::do_preload_all(self.loader.clone())
    }
    async fn do_preload_all(manager: Arc<PackLoader>) {
        let (mut next_index, paths, datasources) = {
            let packs = manager.shared.packs.packs.borrow();
            let paths = packs.values().map(|v| v.info.path.clone())
                .collect::<BTreeSet<_>>();
            let datasources = packs.values().filter_map(|v| v.info.datasource.clone())
                .map(|ds| ds.path)
                .collect::<BTreeSet<_>>();
            (packs.end_path(), paths, datasources)
        };
        let _ = create_dir_all(SourceKind::Pathing.get_user_dir()).await;

        let found_packs = {
            let mut found_packs = Vec::new();
            let dir = Settings::read_source_dir(manager.settings.clone(), SourceKind::Pathing).await;
            futures::pin_mut!(dir);
            while let Some(entry) = dir.next().await {
                let (path, source) = match entry {
                    Ok(e) => e,
                    Err(e) => {
                        log::error!("Failed to list pathing files: {e}");
                        continue
                    },
                };
                if paths.contains(path.as_path()) || source.as_ref().map(|s| datasources.contains(s)).unwrap_or(false) {
                    log::debug!("preload skipping {}; already loaded", path.display());
                    continue
                }
                let datasource = source.map(Locator::with_path);
                let info = SharedPackInfo::new_unloaded(next_index, path.into(), datasource);
                let pack = SharedPackLoad::new_preload(Arc::new(info));
                // TODO: next_index should just be overwritten at insertion anyway
                next_index.path += 1;
                found_packs.push(pack);
            }
            found_packs
        };
        manager.shared.packs.update_packs_extend(&mut found_packs.into_iter());
    }

    pub(super) async fn do_load_all(manager: Arc<PackLoader>) -> anyhow::Result<()> {
        let res = Self::new_task_load_all(manager).await;
        match res {
            res @ (Ok(PathingEvent::Nop) | Err(..)) =>
                res.map(drop),
            Ok(event) => {
                let event_name = format!("{event}");
                if !PathingController::try_send(event) {
                    anyhow::bail!("TODO: failed to send {event_name} after pack load, oh no")
                }
                Ok(())
            },
        }
    }
    pub(super) fn new_task_load_all(manager: Arc<PackLoader>) -> impl Future<Output = anyhow::Result<PathingEvent>> + Send + 'static {
        let paths = {
            let packs = manager.shared.packs.packs.borrow();
            packs.iter().filter_map(|(i, pack)| match &pack.info {
                _ => Some(i),
                _ => None,
            }).collect()
        };
        Self::new_task_pack_loads(manager, paths)
    }

    pub(super) fn gameplay_map(&self) -> Option<MapIndex> {
        self.rx.gameplay.cached.as_ref().and_then(|g| g.gameplay_map())
    }
    pub(super) fn latest_map(&self) -> Option<MapIndex> {
        self.rx.gameplay.cached.as_ref().and_then(|g| g.latest_map())
    }

    /// discard cached data related to all maps except the most recent
    pub(super) fn trim_inactive_maps(&mut self, info_too: bool) {
        let latest_map = self.latest_map();
        if info_too {
            self.map_info.clear(latest_map);
        }
        if let Some(latest_map) = self.latest_map() {
            for (path, map) in self.maps.iter_mut(None) {
                if path.path != latest_map {
                    map.used.mark_for_death();
                }
            }
            self.maps.prune(Some(&self.map_info));
        } else {
            if !self.rx.gameplay.cached.as_ref().map(|g| g.is_initial()).unwrap_or(true) {
                log::info!("unsure of active map - pruning everything");
            }
            self.maps.clear();
        }
    }

    /// eager [self.handle_map_leave()]
    ///
    /// unless reentering, which indicates leave+enter will immediately follow
    pub(super) fn handle_map_suspend(&mut self, reentering_urgent: bool) {
        match &mut self.space.packs {
            #[cfg(todo = "unnecessary")]
            packs => Arc::make_mut(packs).clear(),
            packs =>
                *packs = Arc::new(Default::default()),
        };
        if !reentering_urgent {
            self.maps.prune(Some(&self.map_info));
        }
    }
    pub(super) fn handle_map_leave(&mut self) {
        self.map_info.age_tick(None);
        self.map_info.prune(Some(&self.packs));
        self.maps.age_tick(None);
        self.maps.prune(Some(&self.map_info));
        // TODO: shared map update to None ig
        //self.filter_state.hidden.reset_map_leave();
    }
    pub(super) fn handle_map_enter(&mut self, map_id: MapIndex) {
        self.map_info.age_tick(Some(map_id));
        self.maps.age_tick(Some(map_id));
        let mut need_load = PackSet::new();
        // TODO: could use a `pack_data_for(path).await` here instead of scheduling for load,
        // but would want to spawn it anwyay to avoid blocking event loop so kinda irrelevant
        let map_packs = self.packs.on_map(map_id).filter_map(|(path, pack)| {
            if pack.is_loaded() {
                Some(path)
            } else {
                if pack.can_reload() {
                    need_load.insert(path);
                }
                None
            }
        }).collect::<PackSet>();
        self.map_info.prune(Some(&self.packs));
        self.maps.prune(Some(&self.map_info));
        self.loader.shared.update_map_id(Some(map_id), false);
        let mut shared_map_dirty = false;
        for path in &map_packs {
            let map_path = path.rel(map_id);
            shared_map_dirty |= self.prepare_for_pack_map(map_path, false);
        }
        if /*shared_map_dirty*/ true {
            self.loader.shared.update_map_notify(map_id);
        }
        self.packs.age_tick(Some(&self.map_info));
        self.request_pack_loads(need_load);
    }
    pub(super) fn prepare_for_pack_map(
        &mut self,
        map_path: PackMapPath,
        notify: bool,
    ) -> bool {
        let Some((data, pack_info, info)) = Self::pack_data_if_loaded(&self.loader, map_path.root) else { return false };
        let info_sig = info.sig;
        let map_info = self.map_info.write(map_path);
        if map_info.info_sig != info_sig {
            log::info!("PATHY: updating map info {map_path}");
            map_info.set_info(MapPackInfo::with_pack(map_path.path, &data, &pack_info));
        }
        let map = self.maps.write(map_path);
        if map.info_sig != info_sig {
            log::info!("PATHY: updating map {map_path}");
            *map = LoadedMapPack::from_pack(map_path.path, &*map_info, &data);
            // TODO: if config is going to trigger update immediately after, this may be unnecessary?
            let config = self.loader.shared.packs.packs.borrow().lookup_ref(&map_path.root).as_ref().map(|p| p.config.clone());
            let vis_dirty = if let Some(config) = config.as_ref().map(|c| c.borrow()) {
                if config.info_sig == info_sig {
                    map.refresh_categories(&*map_info, &pack_info.categories, &config.config, None);
                    true
                } else {
                    false
                }
            } else { false };
            if vis_dirty {
                Self::update_loaded_visibility_inner(map_path, map, &*map_info, Some(&self.rx.get_filter_state()));
            }
        } else {
            log::info!("PATHY: skipping map??? {map_path}");
        }
        self.loader.shared.update_map(map_path, &map_info.info, &*map, notify)
    }

    fn pack_data_if_loaded(manager: &PackLoader, path: PackPath) -> Option<(Arc<Pack>, Arc<PackInfo>, Arc<SharedPackInfo>)> {
        let (shared, loaded) = {
            let packs = manager.shared.packs.packs.borrow();
            packs.lookup_ref(&path).map(|pack|
                (pack.info.clone(), pack.loaded.clone())
            )
        }?;
        let info = shared.info.clone()?;
        let data = loaded.borrow().pack.clone();
        data.map(|data| (data, info, shared))
    }

    fn request_pack_loads(&mut self, packs: PackSet) {
        if packs.is_empty() { return }
        for path in &packs {
            self.packs.mark_used(path);
        }
        let loads = Self::new_task_pack_loads(self.loader.clone(), packs);
        let _cancel = self.tasks.spawn(loads);
    }
    fn new_task_pack_loads(manager: Arc<PackLoader>, paths: PackSet) -> impl Future<Output = anyhow::Result<PathingEvent>> + Send + 'static {
        let pending_packs = {
            let packs = manager.shared.packs.packs.borrow();
            paths.iter().map(|path| packs.lookup_ref(&path).map(|pack| {
                (pack.info.path.clone(), pack.info.info.clone())
            }))
                .collect::<Box<[_]>>()
        };
        async move {
            let mut pending = paths.iter().zip(Box::into_iter(pending_packs));
            Self::task_pack_loads(manager, &mut pending).await
        }
    }
    async fn task_pack_loads(manager: Arc<PackLoader>, pending: &mut (dyn Iterator<Item = (PackPath, Option<(Arc<Path>, Option<Arc<PackInfo>>)>)> + Send)) -> anyhow::Result<PathingEvent> {
        let pending_packs = pending
            .filter_map(|(i, info)| info.map(move |(path, prev_info)| {
            let prev_info = prev_info.as_ref().map(|i| &**i);
            let res = match PackActivateContext::new(&*path, None, prev_info) {
                Ok(a) => Ok(a),
                Err(e) => {
                    log::error!("{e:#}");
                    Err(UnloadedReason::UnknownFormat)
                },
            };
            (i, res)
        }));
        let mut failed = Vec::new();
        // TODO: parallel loading and batch shared updates
        for (i, activate) in pending_packs {
            let res = match activate {
                Ok(activate) => activate.load(&manager).await
                    .inspect_err(|e| log::error!("{e:#}"))
                    .map_err(|e| UnloadedReason::LoadingFailed(rt::log::anyhow_into_arc(e))),
                Err(e) => Err(e),
            };
            match res {
                Ok(loaded) => {
                    manager.shared.packs.update_packs_loaded(&mut iter::once((i, Ok(loaded))));
                },
                Err(e) => failed.push((i, e)),
            }
        }
        manager.shared.packs.update_packs_loaded(&mut failed.into_iter().map(|(i, e)| (
            i,
            Err(Some(e)),
        )));
        Ok(PathingEvent::Nop)
    }

    /// TODO: move this to a SharedPacks method?
    fn pack_loader_if_loaded(manager: &PackLoader, path: PackPath) -> Option<Result<SharedLoaderBox, watch::Receiver<SharedPackLoaded>>> {
        let mut loaded = manager.shared.packs.packs.borrow().lookup_ref(&path)?.loaded.subscribe();
        let loader = loaded.borrow_and_update().loader.clone();
        Some(match loader {
            Some(loader) => Ok(loader),
            None => Err(loaded),
        })
    }
    /// TODO: move this to a PackLoader or SharedPacks method?
    pub(super) async fn pack_loader_for(manager: &PackLoader, path: PackPath) -> anyhow::Result<SharedLoaderBox> {
        let loader = Self::pack_loader_if_loaded(manager, path)
            .with_context(|| format!("{path} unrecognized?"))?;
        match loader {
            Ok(l) => Ok(l),
            Err(loaded) => {
                anyhow::bail!("TODO: request loader for {path} idk");
                // TODO: loaded.watch_for(|| loader).timeout(1minuteidk)
            },
        }
    }
}
#[cfg(todo)]
impl PathingController {
    pub async fn setup(&mut self, ctx: &mut PathingEventContext) {
        let api_setup = self.api_setup_get(ctx);

        let settings = &self.loader.settings;
        let get_settings = async {
            let settings = settings.read().await;
            let enabled = settings.enable_katrender;
            let festivals = Self::get_festival_state(&settings.pathing());
            (enabled, festivals)
        };
        let preload = self.preload_all();
        let (_preload, get_settings, api_setup) = tokio::join!(preload, get_settings, api_setup);

        if let (enabled, festivals) = get_settings {
            self.enabled = enabled;
            ctx.festivals.set(festivals);
        }

        for api_setup in api_setup {
            if let Some(task) = Self::api_setup_get_each(api_setup) {
                ctx.tasks.spawn(task);
            }
        }
    }

    const USED_THRESHOLD_MAP_INFO: u32 = 4;
    const USED_THRESHOLD_MAP: u32 = 3;
    pub async fn handle_map_enter(&mut self, map_id: MapIndex, ctx: &mut PathingEventContext) {
        self.map_pack_info.retain(|path, map| match path.path == map_id {
            true => {
                map.used.mark_used();
                true
            },
            false => {
                map.used.mark_unused();
                !map.used.is_elderly(Self::USED_THRESHOLD_MAP_INFO)
            },
        });
        self.map_packs.retain(|path, map| match path.path == map_id {
            true => {
                map.used.mark_used();
                true
            },
            false => {
                map.used.mark_unused();
                !map.used.is_elderly(Self::USED_THRESHOLD_MAP)
            },
        });
        self.load_maps_for(ctx, map_id).await;
    }
    async fn load_maps_for(&mut self, ctx: &mut PathingEventContext, map_id: MapIndex) {
        let loaded_pack = self.load_packs_for(ctx, map_id).await;
        for &path in &loaded_pack {
            self.prepare_pack(ctx, path.rel(map_id)).await;
        }
    }

    async fn load_packs_for(&mut self, ctx: &mut PathingEventContext, map_id: MapIndex) -> Vec<PackPath> {
        // defer update until all are loaded
        let defer_notify = false;
        let mut loaded_pack = Vec::new();
        {
            let load_packs = PackRegistry::load_packs_for_map(Self::packs(), &self.loader, map_id).await;
            futures::pin_mut!(load_packs);
            while let Some((path, mut pack)) = load_packs.next().await {
                let key = path.rel(map_id);
                if self.map_packs.contains_key(&key) && self.map_pack_info.contains_key(&key) {
                    loaded_pack.push(path.clone());
                    continue
                }
                let loaded = Self::prepare_pack_map(ctx, key, &mut pack, !defer_notify);
                match rt::log::error_ok(loaded) {
                    Some(Some((map_info, map))) => {
                        self.map_pack_info.insert(key.clone(), MapPackInfoStorage::new(map_info));
                        self.map_packs.insert(key, map);
                        loaded_pack.push(path.clone());
                    }
                    Some(None) => {
                        log::debug!("deactivating {pack}, why did we think it was relevant to {map_id}?");
                        pack.deactivate(&self.loader);
                    },
                    None => (),
                }
            }
        }
        if !loaded_pack.is_empty() {
            self.update_loaded_visibility();
            //ctx.filter_state_signal = true;
            for &path in &loaded_pack {
                self.post_prepare_map(ctx, path.rel(map_id));
            }
        }
        // now notify
        if defer_notify {
            ctx.shared.update_map_notify(map_id);
        }
        loaded_pack
    }

    fn prepare_pack_map(
        ctx: &mut PathingEventContext,
        key: PackMapPath,
        pack: &mut LoadedPack,
        notify: bool,
    ) -> anyhow::Result<Option<(Arc<MapPackInfo>, LoadedMapPack)>> {
        let Locator { root: path, path: map_id } = key;
        let map_pack_info = MapPackInfo::with_pack(pack, map_id);
        let map_pack_info = match map_pack_info.get() {
            Some(map_pack_info) => map_pack_info,
            None => return Ok(None),
        };
        let map_pack_info = Arc::new(map_pack_info);
        // TODO: swap out for a load_from_pack here?
        let mut map_pack = LoadedMapPack::from_pack(map_id, &map_pack_info, pack);
        if let Ok(info) = &pack.info.info {
            pack.with_config(|config| {
                let _damage = map_pack.update_category_config(&map_pack_info, &info.categories, config)
                    .map_err(drop);
                if let Ok(true) = _damage {
                    // expecting damage to be empty on a fresh load, but maybe it can skip this idk
                    return
                }
                map_pack.refresh_categories(&map_pack_info, &info.categories, config, None);
            });
        }
        ctx.shared.update_map(key, &map_pack_info, &map_pack, notify);
        Ok(Some((map_pack_info, map_pack)))
    }

    fn cb_render_clear(state: &mut RenderState) {
        if let Some(Ok(engine)) = &mut state.engine {
            engine.packs.clear();
        }
    }
    fn cb_render_clear_active(state: &mut RenderState) {
        if let Some(Ok(engine)) = &mut state.engine {
            engine.packs.clear_active();
        }
    }

    pub(crate) async fn reload_all(&mut self, ctx: &mut PathingEventContext, remove: bool) {
        self.unload_all(ctx, remove).await;
        self.load_all(ctx).await;
    }
    pub(crate) async fn reload_pack(&mut self, ctx: &mut PathingEventContext, path: PackPath, remove: bool) {
        self.unload_pack(ctx, path, remove).await;
        let res = self.load_pack(ctx, path).await
            .with_context(|| format!("reloading {path}"));
        let _ = rt::log::error_ok(res);
    }

    async fn load_all(&mut self, ctx: &mut PathingEventContext) {
        self.preload_all().await;
        if let Some(map_id) = ctx.gameplay_map() {
            self.load_maps_for(ctx, map_id).await
        }
        //tokio::spawn(Self::load_all_inner(self.loader.clone()));
    }

    async fn lowmem_activate(&mut self, _ctx: &mut PathingEventContext) {
        log::info!("TODO: lowmem mode");
        let mut packs = Self::packs().write().await;
        for pack in &mut packs.packs {
            pack.deactivate(&self.loader);
        }
    }

    async fn unload_all(&mut self, ctx: &mut PathingEventContext, remove: bool) {
        log::info!("Unloading all paths...");
        ctx.spawn_render(RenderTaskPriority::High, Self::cb_render_clear);
        Self::unload_all_inner(self.loader.clone(), remove).await;
        self.map_packs.clear();
        if remove {
            self.map_pack_info.clear();
        }
        ctx.shared.clear_maps_for_packs(true, true);
    }

    async fn unload_pack(&mut self, ctx: &mut PathingEventContext, path: PackPath, remove: bool) {
        log::debug!("Unloading {path}...");
        Self::unload_pack_inner(self.loader.clone(), path, remove).await;
        self.map_packs.retain(|k, _| k.root != path);
        if remove {
            self.map_pack_info.retain(|k, _| k.root != path);
        }
        ctx.shared.clear_maps_for_packs((&path,), true);
    }
    async fn load_pack(&mut self, ctx: &mut PathingEventContext, path: PackPath) -> anyhow::Result<Option<()>> {
        let pack = PackRegistry::load_pack(Self::packs(), &self.loader, path).await;
        let key = ctx.gameplay_map()
            .map(|map_id| path.rel(map_id));
        match (pack, key) {
            (Ok(mut pack), Some(key)) => match Self::prepare_pack_map(ctx, key, &mut pack, true) {
                Ok(Some((map_info, map))) => {
                    self.map_pack_info.insert(key.clone(), MapPackInfoStorage::new(map_info));
                    self.map_packs.insert(key, map);
                    self.post_prepare_map(ctx, key);
                    Ok(Some(()))
                },
                Ok(None) => Ok(None),
                Err(e) => Err(e),
            },
            (Ok(..), None) => Ok(Some(())),
            (Err(e), ..) => Err(e),
        }.with_context(|| format!("loading {path}"))
    }

    async fn unload_all_inner(loader: Arc<PackLoader>, remove: bool) {
        let context = "Unloading packs from engine";
        let res = Controller::run_render(RenderTaskPriority::High, Self::cb_render_clear).await
            .context(context);
        let _ = rt::log::warn_ok(res);
        {
            let mut packs = Self::packs().write().await;
            for pack in &mut packs.packs {
                pack.deactivate(&loader);
                if !remove {
                    pack.mark_reload(&loader);
                }
            }
            if remove {
                packs.packs.clear();
            }
        }
    }

    async fn unload_pack_inner(loader: Arc<PackLoader>, path: PackPath, remove: bool) {
        let context = "Unloading pack from engine";
        let res = Controller::run_render(RenderTaskPriority::High, move |state| {
            if let Some(Ok(engine)) = &mut state.engine {
                engine.packs.deactivate(path.path, true);
            }
        }).await.context(context);
        let _ = rt::log::warn_ok(res);
        {
            let mut packs = Self::packs().write().await;
            if let Some(pack) = packs.lookup_mut(&path) {
                pack.deactivate(&loader);
                if remove {
                    pack.mark_dead(&loader);
                } else {
                    pack.mark_reload(&loader);
                }
            }
        }
    }

    fn mark_map_state_dirty(&self, ctx: &mut PathingEventContext, map_id: MapIndex) {
        ctx.shared.maps.send_if_modified(|shared_maps| {
            // shared_maps.update_prune_maps(&Locator::with_parts(Wildcard, map_id));
             shared_maps.update_prune_maps(&self.map_pack_info)
        });
        ctx.shared.gameplay.send_if_modified(|shared_map| {
            let mut dirty = false;
            let Some(shared_map) = shared_map.get_mut(map_id) else { return dirty };
            for (path, shared_state, _shared_info) in shared_map.iter_state_mut() {
                let Some(map_pack) = self.map_packs.get(&path) else {
                    log::debug!("INCONSISTENT missing state for shared map {path}");
                    continue
                };
                dirty |= shared_state.update_static(map_pack);
                dirty |= shared_state.update_with_loaded(map_pack);
                dirty |= shared_state.update_with_hidden(path, &self.filter_state.hidden, map_pack);
            }
            dirty
        });
    }

    fn post_prepare_map(&mut self, ctx: &mut PathingEventContext, path: PackMapPath) {
        self.mark_hidden_dirty(ctx, Some(path));
    }
}
