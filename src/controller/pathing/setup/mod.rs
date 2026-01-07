use {
    crate::{
        controller::pathing::{
            PathingController, PathingEvent,
            visible::LoadedMapPack,
            shared::{SharedPacks, SharedPackInfo, SharedPackLoad, MapPackInfo, SharedPackLoaded},
            registry::{PackLoader, PackActivateContext, UnloadedReason, PackInfo, SharedLoaderBox},
        },
        exports::runtime as rt,
        settings::{Settings, SourceKind},
    }, anyhow::{anyhow, Context}, std::sync::Arc,
    futures::stream::{self, Stream, StreamExt},
    taimi_sync::watched::watch,
    taimi_hoard::loc::{Locator, LocationRef},
    taimi_meta::packs::{PackPath, PackMapPath, MapIndex, collections::PackSet},
    taimi_pack::Pack,
    std::collections::BTreeSet,
    std::future::Future,
    std::iter,
    std::path::Path,
    tokio::{fs::create_dir_all, time::{timeout, Duration}},
};

impl PathingController {
    #[cfg(deleteme)]
    pub(super) async fn handle_pack_loaded(&mut self, path: PackPath, loaded: Result<PackActivateLoaded, Option<UnloadedReason>>) {
    }

    pub(super) fn preload_all(&self) -> impl Future<Output = ()> + Send + 'static {
        Self::do_preload_all(self.loader.clone())
    }
    async fn do_preload_all(manager: Arc<PackLoader>) {
        let _ = create_dir_all(SourceKind::Pathing.get_user_dir()).await;

        let _ = Self::do_refresh_all(&manager, true).await;
    }
    async fn do_refresh_all(manager: &Arc<PackLoader>, include_datasources: bool) -> impl Iterator<Item = PackPath> + '_ {
        let found_packs: Vec<_> = Self::iter_new_packs(&manager, include_datasources)
            .filter_map(|res| async move {
                rt::log::error_ok(res)
            }).collect().await;
        manager.shared.packs.update_packs_extend(&mut found_packs.into_iter())
    }
    fn iter_new_packs(manager: &Arc<PackLoader>, include_datasources: bool) -> impl Stream<Item = anyhow::Result<SharedPackLoad>> {
        let (mut next_index, paths, datasources) = {
            let packs = manager.shared.packs.packs.borrow();
            let paths = packs.values().map(|v| v.info.path.clone())
                .collect::<BTreeSet<_>>();
            let datasources = match include_datasources {
                true => packs.values().filter_map(|v| v.info.datasource.clone())
                    .map(|ds| ds.path)
                    .collect::<BTreeSet<_>>(),
                false => BTreeSet::new(),
            };
            (packs.end_path(), paths, datasources)
        };

        stream::once(
            Settings::read_source_dir(manager.settings.clone(), SourceKind::Pathing)
        ).flatten().map(move |entry| {
            let (path, source) = entry.context("Failed to list pathing files")?;
            if !include_datasources && source.is_some() { return Ok(None) }
            if paths.contains(path.as_path()) || source.as_ref().map(|s| datasources.contains(s)).unwrap_or(false) {
                let path = rt::relative_path(&path);
                log::debug!("skipping {}; already loaded", path.display());
                return Ok(None)
            }
            let datasource = source.map(Locator::with_path);
            let info = SharedPackInfo::new_unloaded(next_index, path.into(), datasource);
            let pack = SharedPackLoad::new_preload(Arc::new(info));
            // index is just a guess, and will be overwritten upon insertion if incorrect
            next_index.path += 1;
            Ok(Some(pack))
        }).filter_map(|res| async move { res.transpose() })
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
            map_info.set_info(MapPackInfo::with_pack(map_path.path, &data, &pack_info));
        }
        let map = self.maps.write(map_path);
        if map.info_sig != info_sig {
            *map = LoadedMapPack::from_pack(map_path.path, &*map_info, &data);
        }
        let vis_dirty = {
            // TODO: if config is going to trigger update immediately after, this may be unnecessary?
            let config = self.loader.shared.packs.packs.borrow().lookup_ref(&map_path.root).as_ref().map(|p| p.config.clone());
            let config = config.as_ref().map(|c| c.borrow());
            if let Some(config) = config {
                if config.info_sig == info_sig {
                    let damage = map.update_category_config(&*map_info, &pack_info.categories, &config.config);
                    if let Ok(true) = &damage {
                        false
                    } else {
                        map.refresh_categories(&*map_info, &pack_info.categories, &config.config, damage.err().as_ref());
                        true
                    }
                } else {
                    false
                }
            } else { false }
        };
        if vis_dirty {
            Self::update_loaded_visibility_inner(map_path, map, &*map_info, Some(&self.rx.get_filter_state()));
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

    pub(super) fn mark_map_state_dirty(&self, map_id: Option<MapIndex>, info_dirty: bool) -> bool {
        let dirty = false;
        #[cfg(todo)]
        {
            ctx.shared.maps.send_if_modified(|shared_maps| {
                 dirty = shared_maps.update_prune_maps(&self.maps, &self.map_info);
                 false
            });
        }
        let notified = self.loader.update_map_states(true, info_dirty, &mut self.maps.iter_with_info(&self.map_info, map_id));
        if let Some(map_id) = map_id {
            if dirty && !notified {
                self.loader.shared.update_map_notify(map_id);
            }
        }

        dirty | notified
    }

    #[cfg(todo)]
    pub(super) fn post_prepare_map(&mut self, ctx: &mut PathingEventContext, path: PackMapPath) {
        self.mark_hidden_dirty(ctx, Some(path));
    }
}
impl PackLoader {
    pub(super) fn update_map_states(&self, notify: bool, info_dirty: bool, maps: &mut dyn Iterator<Item = (PackMapPath, &LoadedMapPack, &Arc<MapPackInfo>)>) -> bool {
        let mut dirty = false;
        if let (_, Some(0)) = maps.size_hint() {
            return dirty
        }
        self.shared.gameplay.send_if_modified(|shared_map| {
            for (path, map, map_info) in maps {
                if let Some(shared_info) = shared_map.get_info_mut(path) {
                    dirty |= shared_info.update_with_info(map_info);
                    if info_dirty {
                        dirty |= shared_info.update_with(map);
                    }
                };
                let Some(shared_state) = shared_map.get_state_mut(path) else {
                    continue
                };
                dirty |= shared_state.update_static(map);
                dirty |= shared_state.update_with_loaded(map);
                #[cfg(todo)] {
                    dirty |= shared_state.update_with_hidden(path, &self.filter_state.hidden, map_pack);
                }
            }
            notify && dirty
        });
        dirty
    }

    pub(super) async fn pack_loader_for(&self, path: PackPath) -> anyhow::Result<SharedLoaderBox> {
        self.shared.packs.pack_loader_for(path).await
    }
}
impl SharedPacks {
    pub(crate) fn pack_loader_if_loaded(&self, path: PackPath) -> Option<Result<SharedLoaderBox, watch::Receiver<SharedPackLoaded>>> {
        let mut loaded = self.packs.borrow().lookup_ref(&path)?.loaded.subscribe();
        let loader = loaded.borrow_and_update().loader.clone();
        Some(match loader {
            Some(loader) => Ok(loader),
            None => Err(loaded),
        })
    }
    const LOADER_TIMEOUT: Duration = Duration::from_secs(10);
    const LOADER_TIMEOUT_RETRY: u8 = 6;
    pub(crate) async fn pack_loader_for(&self, path: PackPath) -> anyhow::Result<SharedLoaderBox> {
        let loader = self.pack_loader_if_loaded(path)
            .with_context(|| format!("{path} unrecognized?"))?;
        match loader {
            Ok(l) => Ok(l),
            Err(mut loaded) => Self::wait_for_pack_loader(path, &mut loaded).await,
        }
    }
    pub(crate) async fn wait_for_pack_loader(path: PackPath, loaded: &mut watch::Receiver<SharedPackLoaded>) -> anyhow::Result<SharedLoaderBox> {
        let (mut is_loading, mut retry) = (false, Self::LOADER_TIMEOUT_RETRY);
        let prev = {
            let loaded = loaded.borrow_and_update();
            if let Some(loader) = loaded.loader.as_ref() {
                // unlikely but worth checking anyway...
                return Ok(loader.clone())
            }
            match &loaded.unloaded {
                None => {
                    log::warn!("{path} missing loader?");
                    retry = 1;
                    0
                },
                Some(reason @ UnloadedReason::Loading) => {
                    is_loading = true;
                    reason.discriminant()
                },
                Some(reason) if reason.can_reactivate(false) => {
                    PathingEvent::LoadPack(path).try_send();
                    reason.discriminant()
                },
                Some(reason) =>
                    anyhow::bail!("{path} not loaded: {reason}"),
            }
        };
        let loader = loop {
            let loaded = loaded.wait_for(|loaded| {
                if loaded.loader.is_some() {
                    return true
                }
                match &loaded.unloaded {
                    Some(UnloadedReason::Pending) => false,
                    Some(UnloadedReason::Loading) => {
                        is_loading = true;
                        false
                    },
                    Some(reason) if reason.discriminant() != prev || !reason.can_reactivate(false) => true,
                    None if prev != 0 => true,
                    _ if is_loading => true,
                    _ => false,
                }
            });
            match timeout(Self::LOADER_TIMEOUT, loaded).await {
                Ok(Ok(loaded)) => break loaded.loader.clone(),
                Err(..) if is_loading => match retry.checked_sub(1) {
                    None => break None,
                    Some(c) => retry = c,
                },
                Err(..) | Ok(Err(..)) => break None,
            }
        }.ok_or_else(|| loaded.borrow_and_update());
        match loader {
            Ok(loader) => Ok(loader),
            Err(loaded) => match (loaded.loader.clone(), loaded.unloaded.as_ref()) {
                (Some(loader), _) => Ok(loader),
                (None, Some(reason)) =>
                    Err(anyhow!("{path} not loaded: {reason}")),
                (None, None) =>
                    Err(anyhow!("{path} not loaded?")),
            },
        }
    }
}
