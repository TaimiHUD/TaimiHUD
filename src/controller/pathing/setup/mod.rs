use {
    crate::{
        controller::pathing::{
            PathingController,
            FestivalState,
            state::shared::{SharedMapPackState, SharedMapPackLoaded},
            festivals::FestivalFixup,
            state::{
                MapPackInfoStorage,
                info::MapPackInfo,
            },
            registry::{PackRegistry, MapIndex, PackMapPath},
            registry::{LoadedPack, PackLoader, PackPath},
            visible::LoadedMapPack,
        },
        controller::Controller,
        exports::runtime::{self as rt, locator::{LocationMut, LocationRef}, Locator},
        render::{machine::RenderTaskPriority, RenderState},
        settings::{Settings, SourceKind},
    }, anyhow::{anyhow, Context}, futures::StreamExt, std::{collections::btree_map, sync::Arc},
    tokio::fs::create_dir_all,
};
pub use self::{
    reactor::{PathingEventContext, PathingEvent},
    space::{SetupPoi, SetupTrail},
};

mod space;
mod config;
mod filter;
mod reactor;
mod interact;

impl PathingController {
    pub async fn setup(&mut self, ctx: &mut PathingEventContext) {
        let settings = &self.loader.settings;
        let mut enabled = false;
        let festivals = async {
            let settings = settings.read().await;
            enabled = settings.enable_katrender;
            let (on, off) = settings.pathing().festival_preferences();
            FestivalState {
                active: FestivalFixup::current_festivals(),
                on,
                off,
            }
        };
        let achievements = async move {
            use tokio::io::AsyncReadExt;
            if let Ok(mut file) = tokio::fs::File::open(rt::addon_dir().join("achievements.json")).await {
                let mut data = Vec::new();
                file.read_to_end(&mut data).await?;
                serde_json::from_slice::<crate::settings::pathing::PathingAchievementApi>(&data)
                    .map_err(anyhow::Error::from)
                    .map(crate::settings::pathing::PathingAchievementSave::from)
                    .map(Some)
            } else {
                Ok(None)
            }
        };
        let preload = self.preload_all();
        let (_preload, festivals, achievements) = tokio::join!(preload, festivals, achievements);

        ctx.festivals.set(festivals);
        self.enabled = enabled;

        let achievements = achievements
            .context("loading achievements.json");
        if let Some(Some(achievements)) = rt::log::warn_ok(achievements) {
            self.filter_state.achievements.status = achievements.into();
        }

        ctx.pack_info.send_if_modified(|shared| {
            shared.shared_loader = Some(self.loader.clone());
            true
        });
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
        let defer_notify = true;
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
        let packs = Self::packs().read().await;
        ctx.pack_info.send_if_modified(|shared_info| {
            for (path, loaded) in packs.all_packs() {
                // if loaded_pack.contains(&path) ?
                shared_info.update_pack(path, loaded);
            }
            // !loaded_pack.is_empty()
            defer_notify
        });
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
            None => return {
                ctx.pack_info.send_if_modified(|shared_info| {
                    shared_info.update_pack(path.clone(), pack);
                    notify
                });
                Ok(None)
            },
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
        ctx.pack_info.send_if_modified(|shared_info| {
            shared_info.update_pack(path.clone(), pack);
            shared_info.map_info.insert(key.clone(), SharedMapPackLoaded::with_loaded(map_pack_info.clone(), &map_pack));
            shared_info.map_state.insert(key.clone(), SharedMapPackState::with_static(key, &map_pack));
            notify
        });
        Ok(Some((map_pack_info, map_pack)))
    }

    /// eager [self.handle_map_leave()]
    fn handle_map_suspend(&mut self, ctx: &mut PathingEventContext) {
        ctx.spawn_render(RenderTaskPriority::High, Self::cb_render_clear_active);
    }
    fn handle_map_leave(&mut self, _ctx: &mut PathingEventContext) {
        self.filter_state.hidden.reset_map_leave();
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

    async fn preload_all(&self) {
        let _ = create_dir_all(SourceKind::Pathing.get_user_dir()).await;

        let found_packs = {
            let mut found_packs = Vec::new();
            let dir = Settings::read_source_dir(self.loader.settings.clone(), SourceKind::Pathing).await;
            futures::pin_mut!(dir);
            while let Some(entry) = dir.next().await {
                let (path, source) = match entry {
                    Ok(e) => e,
                    Err(e) => {
                        log::error!("Failed to list pathing files: {e}");
                        continue
                    },
                };
                let datasource = source.map(Locator::with_path);
                found_packs.push((path, datasource));
            }
            found_packs
        };
        let mut packs = Self::packs().write().await;
        for (path, datasource) in found_packs {
            packs.preload(path, datasource, &self.loader);
        }
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
        {
            let remove_packs = match remove {
                true => None,
                false => Some(Self::packs().read().await),
            };
            ctx.pack_info.send_modify(|pack_info| {
                pack_info.map_info.clear();
                pack_info.map_state.clear();
                if let Some(packs) = &remove_packs {
                    for (path, pack) in packs.all_packs() {
                        pack_info.update_pack(path, pack);
                    }
                } else {
                    pack_info.pack_info.clear();
                }
            });
        }
    }

    async fn unload_pack(&mut self, ctx: &mut PathingEventContext, path: PackPath, remove: bool) {
        log::debug!("Unloading {path}...");
        Self::unload_pack_inner(self.loader.clone(), path, remove).await;
        self.map_packs.retain(|k, _| k.root != path);
        if remove {
            self.map_pack_info.retain(|k, _| k.root != path);
        }
        {
            let remove_packs = match remove {
                true => None,
                false => Some(Self::packs().read().await),
            };
            let remove_pack = remove_packs.as_ref()
                .and_then(|packs| packs.lookup_ref(&path));
            ctx.pack_info.send_modify(|pack_info| {
                pack_info.map_info.retain(|k, _| k.root != path);
                pack_info.map_state.retain(|k, _| k.root != path);
                if let Some(pack) = remove_pack {
                    pack_info.update_pack(path, pack);
                } else {
                    pack_info.pack_info.remove(&path);
                }
            });
        }
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
        ctx.pack_info.send_if_modified(|pack_info| {
            pack_info.map_state.retain(|path, _| path.path == map_id);
            for (path, map_pack) in self.map_packs.iter().filter(|(path, _)| path.path == map_id) {
                // TODO: skip update if unchanged etc
                match pack_info.map_state.entry(path.clone()) {
                    btree_map::Entry::Occupied(mut e) => {
                        e.get_mut().update_static(map_pack);
                        e.get_mut().update_with_loaded(map_pack);
                        e.get_mut().update_with_hidden(*path, &self.filter_state.hidden, map_pack);
                    },
                    btree_map::Entry::Vacant(e) => {
                        e.insert(SharedMapPackState::with_loaded(*path, map_pack, &self.filter_state.hidden));
                    },
                }
            }
            true
        });
    }

    fn post_prepare_map(&mut self, ctx: &mut PathingEventContext, path: PackMapPath) {
        self.mark_hidden_dirty(ctx, Some(path));
    }
}
