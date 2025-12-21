use {
    crate::{
        controller::pathing::{
            PathingController,
            shared::{SharedPackInfo, SharedPackLoad, PathingShared},
            registry::{PackLoader, PackActivateContext, UnloadedReason},
        },
        exports::runtime as rt,
        controller::Controller,
        render::{machine::RenderTaskPriority, RenderState},
        settings::{Settings, SettingsLock, SourceKind},
    }, anyhow::Context, futures::StreamExt, std::sync::Arc,
    taimi_hoard::loc::Locator,
    taimi_meta::packs::MapIndex,
    std::collections::BTreeSet,
    std::future::Future,
    std::iter,
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
        let pending_packs = {
            let packs = manager.shared.packs.packs.borrow();
            packs.iter().map(|(i, pack)| (i, pack.info.path.clone(), pack.info.info.clone()))
                .collect::<Vec<_>>()
        };
        let mut failed = Vec::new();
        let pending_packs = pending_packs.into_iter().filter_map(|(i, path, prev_info)| {
            let prev_info = prev_info.as_ref().map(|i| &**i);
            let activate = PackActivateContext::new(&*path, None, prev_info);
            match activate {
                Err(e) => {
                    log::error!("{e:#}");
                    failed.push((i, e));
                    None
                },
                Ok(a) => Some((i, a)),
            }
        }).collect::<Vec<_>>();
        // TODO: parallel loading and batch shared updates
        for (i, activate) in pending_packs {
            match activate.load(&manager).await {
                Ok(loaded) => {
                    manager.shared.packs.update_packs_loaded(&mut iter::once((i, Ok(loaded))));
                },
                Err(e) => {
                    log::error!("{e:#}");
                    failed.push((i, e));
                },
            }
        }
        manager.shared.packs.update_packs_loaded(&mut failed.into_iter().map(|(i, e)| (
            i,
            Err(Some(UnloadedReason::LoadingFailed(rt::log::anyhow_into_arc(e)))),
        )));
        Ok(())
    }

    /// eager [self.handle_map_leave()]
    fn handle_map_suspend(&mut self) {
        log::info!("TODO: clear_active() on suspend");
    }
    fn handle_map_leave(&mut self) {
        log::info!("TODO: handle_map_leave()");
        self.maps.cleanup(None);
        // TODO: shared map update to None ig
        //self.filter_state.hidden.reset_map_leave();
    }
    fn handle_map_enter(&mut self, map_id: MapIndex) {
        self.maps.cleanup(Some(map_id));
        let map_packs = {
            TODO("map_info first, then pass on to map state init thanks");
            let packs = self.loader.shared.packs.packs.borrow();
            packs.iter().filter_map(|(path, pack)| pack.info.info.as_ref().and_then(|info| match info.maps.contains(map_id) {
                true => Some((path, info.clone())),
                false => None,
            })).collect::<Vec<_>>()
        };
        for &(path, ref info) in &map_packs {
            let map = self.maps.write(path.rel(map_id));
            if map.info_sig == info.sig { continue }
            *map = LoadedMapPack::from_pack(map_id, map_info, pack);
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
