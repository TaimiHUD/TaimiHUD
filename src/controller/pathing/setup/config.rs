use {
    crate::{
        controller::{
            pathing::{
                filter::{self, MarkerFilter},
                registry::{CategoryPath, MapIndex, MarkerIndex, MarkerPath, PackLoader, PackPath},
                state::shared::SharedPacks,
                visible::VisibilityFlags, PathingController, PathingEvent, PathingEventContext
            }, Controller
        },
        exports::runtime::locator::LocationRef,
        render::machine::RenderTaskPriority,
        settings::{state::SaveState, PathingSettings},
        space::{
            engine::SpaceEvent, Engine
        },
    },
    anyhow::Context,
    std::{iter, sync::Arc, time::SystemTime},
    taimi_meta::ui::MapContext,
};

impl PathingController {
    pub(super) async fn handle_config_change(&mut self, ctx: &mut PathingEventContext, path: PackPath) {
        let mut dirty = false;
        for (map_path, map) in self.map_packs.iter_mut().filter(|(p, _)| p.root == path) {
            let Some(map_info) = self.map_pack_info.get(map_path) else { continue };
            let Some((info, config)) = Self::packs().read().await.packs.get(path.path as usize).and_then(|p| p.get_info()) else { continue };
            {
                let damage = map.update_category_config(&map_info, &info.categories, &config);
                if let Ok(true) = damage {
                    continue
                }
                map.refresh_categories(&map_info, &info.categories, &config, damage.err().as_ref());
            }
            dirty = true;
            ctx.shared.gameplay.send_if_modified(|shared_map| {
                let Some(shared_state) = shared_map.get_state_mut(*map_path) else {
                    return false
                };
                shared_state.categories = map.categories.clone();
                false
            });
        }
        if dirty {
            ctx.shared.gameplay.send_if_modified(|_| true);
            //ctx.filter_state_signal = true;
            PathingEvent::RequestDisabledPaths.try_send();
        }
    }

    #[cfg(todo)]
    pub(super) async fn handle_vis(&mut self, ctx: &mut PathingEventContext, path: CategoryPath<PackPath>, state: VisibilityFlags) {
        let packs = Self::packs().read().await;
        let Some(pack) = packs.lookup_ref(&path.root) else { return };
        let Some(config) = &pack.config else {
            log::error!("can't update {path}={}, no config state?", state.bits());
            return
        };
        let Ok(info) = &pack.info.info else { return };
        let cat_vis = info.categories.visibility.get_for(path)
            .unwrap_or(VisibilityFlags::TOGGLES);
        let state_dev = cat_vis ^ state;

        config.send_if_modified(|config| {
            let path = path.unscope();
            if config.visibility_deviation_for(path) == state_dev {
                return false
            }
            Arc::make_mut(config).set_visibility_deviation(path, state_dev);
            true
        });
    }

    pub(super) async fn handle_toggle(loader: &PackLoader, path: CategoryPath<PackPath>, state: Option<bool>) {
        // TODO: rethink whether controller wants to use loader like this or not?
        let categories = SharedPacks::pack_at(&loader.shared.packs.info.borrow(), path.root)
            .and_then(|pack_info| pack_info.info.as_ref().ok()
                .map(|info| info.categories.clone())
            );
        let config = SharedPacks::pack_at(&loader.shared.packs.config.borrow(), path.root)
            .and_then(|config| config.as_ref()
                .map(|config| config.clone())
            );
        let (Some(categories), Some(config)) = (categories, config) else {
            log::error!("can't update {path}={state:?}, no config state?");
            return
        };
        let cat_vis = !categories.disabled.contains(path);
        let toggle_dev = state.map(|state| cat_vis ^ state);

        let mut state = state.unwrap_or(false);
        let changed = config.send_if_modified(|config| {
            let path = path.unscope();
            let prev = config.visibility_deviation_for(path);
            let mut state_dev = prev;
            state = toggle_dev.unwrap_or(!prev.contains(VisibilityFlags::TOGGLE));
            state_dev.set(VisibilityFlags::TOGGLE, state);
            if prev == state_dev {
                return false
            }
            Arc::make_mut(config).set_visibility_deviation(path, state_dev);
            true
        });

        if changed {
            let changes = iter::once((path.unscope(), cat_vis ^ state));
            Self::category_commit_vis_set(loader, path.root, changes).await;
        }
    }
    pub(super) async fn category_commit_vis<C>(loader: &PackLoader, pack_path: PackPath, dirty_cats: C) where
        C: IntoIterator<Item = CategoryPath>,
    {
        let categories = SharedPacks::pack_at(&loader.shared.packs.info.borrow(), pack_path)
            .and_then(|pack_info| pack_info.info.as_ref().ok()
                .map(|info| info.categories.clone())
            );
        let config = SharedPacks::pack_at(&loader.shared.packs.config.borrow(), pack_path)
            .and_then(|config| config.as_ref()
                .map(|config| config.borrow().clone())
            );
        let (Some(categories), Some(config)) = (categories, config) else {
            log::error!("cannot save category settings for unloaded {pack_path}");
            return
        };
        let changes = dirty_cats.into_iter()
            .map(|path| {
                let default = !categories.disabled.contains(path);
                let vis = config.visibility_deviation_for(path);
                let state = vis.is_visible() ^ default;
                (path, state)
            });
        Self::category_commit_vis_set(loader, pack_path, changes).await
    }
    pub(super) async fn category_commit_vis_set<C>(loader: &PackLoader, pack_path: PackPath, dirty_cats: C) where
        C: IntoIterator<Item = (CategoryPath, bool)>,
    {
        #[cfg(todo)]
        if dirty_cats.is_empty() {
            return
        }
        let packs = Self::packs().read().await;
        let Some(loaded) = packs.lookup_ref(&pack_path) else {
            return
        };
        let mut settings = None;
        for (path, vis_state) in dirty_cats {
            let full_id =loaded.active.as_ref()
                    .and_then(|active| active.pack.categories.all_categories.get_index(path.path as usize))
                    .map(|(_id, cat)| cat.full_id.clone());
            if let Some(full_id) = full_id {
                let settings = match &mut settings {
                    Some(s) => s,
                    s @ None => s.insert(loader.settings.write().await),
                };
                PathingSettings::pathing_state_update(settings, full_id.to_string(), vis_state).await;
            } else {
                log::warn!("{path} not found for toggle state update");
            }
        }
    }

    #[cfg(todo)]
    async fn update_visibility_with(&mut self, disabled_paths: &HashSet<String>, active_festivals: Festivals) {
        let packs = Self::packs().read().await;
        let mut disabled_categories = BTreeMap::<_, bool>::new();
        for (path, map_pack) in &mut self.map_packs {
            let Some(pack) = packs.lookup_ref(&path.root) else { continue };
            let Some(active) = &pack.active else { continue };

            let pois = map_pack.pois.iter_mut()
                .map(|poi| (poi.category, &mut poi.visibility));
            let trails = map_pack.trails.iter_mut()
                .map(|trail| (trail.category, &mut trail.visibility));
            for (category_index, visibility) in pois.chain(trails) {
                *visibility = visibility.restore_default_toggles();
                if !visibility.is_visible() {
                    continue
                }
                let key = (path, category_index);
                let disabled = disabled_categories.get(&key).copied();
                let disabled = match disabled {
                    Some(d) => d,
                    None => {
                        let Some((full_id, category)) = active.pack.categories.all_categories.get_index(category_index as usize) else { continue };
                        let mut disabled = disabled_paths.contains(full_id);
                        if !disabled {
                            for path in disabled_paths.iter() {
                                if full_id.starts_with(path) {
                                    disabled = true;
                                    break
                                }
                            }
                        }
                        if !disabled {
                            let festivals: Festivals = category.marker_attributes.festivals.as_ref()
                                .into_iter().flatten().copied().collect();
                            if !festivals.is_empty() && !festivals.intersects(active_festivals) {
                                disabled = true;
                            }
                        }
                        disabled_categories.insert(key, disabled);
                        disabled
                    },
                };
                if disabled {
                    visibility.remove(VisibilityFlags::TOGGLE);
                }
            }
        }
    }
    pub(super) fn update_loaded_visibility(&mut self) -> bool {
        let hidden_guids = SaveState::read_with(|s| s.pathing_state.as_ref().map(|p| p.hidden_guid_expiry.clone()));
        if let Some(hidden_guids) = hidden_guids {
            let now = SystemTime::now();
            let now_mono = std::time::Instant::now();
            let all_guids = self.map_packs.values()
                .flat_map(|map| map.poi_guids.iter().chain(map.trail_guids.iter()));

            for guid in all_guids {
                if self.filter_state.hidden.hidden.contains_key(guid.as_ref()) {
                    continue
                }
                let Some(&expiry_timestamp) = hidden_guids.get(guid) else { continue };
                self.filter_state.hidden.expire_at_timestamp(guid.clone(), expiry_timestamp, &now, &now_mono);
            }
            self.filter_state.hidden.reset_expired(&now_mono);
        }
        let filter_state = &self.filter_state;
        let mut dirty = false;
        for (path, map_pack) in &mut self.map_packs {
            let Some(map_info) = self.map_pack_info.get(path) else { continue };

            let map_filters = &map_pack.filters;
            let mut poi_filters = map_filters.pois.iter().peekable();
            let mut trail_filters = map_filters.trails.iter().peekable();

            let mut poi_guids = map_pack.poi_guids.iter();
            let pois = map_info.pois().zip(
                map_pack.pois.iter_mut()
                .zip(map_info.poi_guid_mask().map(|guid| guid.then(|| poi_guids.next()).flatten()))
            );
            let pois = pois
                .map(|(poi_path, (poi, guid))| {
                    let filters = loop {
                        match poi_filters.peek() {
                            Some((fp, ..)) if fp.path < poi_path.path => (),
                            Some((fp, ..)) if fp.path == poi_path.path => break poi_filters.next(),
                            _ => break None,
                        };
                    }.map(|(_p, f)| f);
                    let marker_path = MarkerPath::with_parts(path, MarkerIndex::from(poi_path));
                    (marker_path, poi.category, &mut poi.visibility, filters, guid)
                });
            let mut trail_guids = map_pack.trail_guids.iter();
            let trails = map_info.trails().zip(
                map_pack.trails.iter_mut()
                .zip(map_info.trail_guid_mask().map(|guid| guid.then(|| trail_guids.next()).flatten()))
            );
            let trails = trails
                .map(|(trail_path, (trail, guid))| {
                    let filters = loop {
                        match trail_filters.peek() {
                            Some((fp, ..)) if fp.path < trail_path.path => (),
                            Some((fp, ..)) if fp.path == trail_path.path => break trail_filters.next(),
                            _ => break None,
                        };
                    }.map(|(_p, f)| f);
                    let marker_path = MarkerPath::with_parts(path, MarkerIndex::from(trail_path));
                    (marker_path, trail.category, &mut trail.visibility, filters, guid)
                });
            for (marker_path, category_index, visibility, filter, guid) in pois.chain(trails) {
                let prev = *visibility & VisibilityFlags::TOGGLES;
                *visibility = visibility.restore_default_toggles();
                let cat_vis = map_info.category_index(CategoryPath::with_path(category_index))
                    .and_then(|i| map_pack.categories.get(i as usize))
                    .map(|cat| cat.visibility);
                if let Some(cat_vis) = cat_vis {
                    // TODO: if cat vis is override, set directly or something!
                    visibility.set_toggles((cat_vis & VisibilityFlags::TOGGLE) | (*visibility & !VisibilityFlags::TOGGLE));
                    visibility.set(VisibilityFlags::TOGGLE, cat_vis.contains(VisibilityFlags::TOGGLE));
                }
                if visibility.is_visible() {
                    if let Some(filter) = &filter {
                        if let filter::FILTER_HIDDEN = filter.is_visible(filter_state) {
                            visibility.remove(VisibilityFlags::TOGGLE);
                        }
                    }
                }
                if visibility.is_visible() {
                    let marker_path: MarkerPath = MarkerPath::with_path(marker_path.path);
                    if let Some(hidden) = guid.and_then(|guid| map_filters.group_filter_for(marker_path, guid)) {
                        if let filter::FILTER_HIDDEN = hidden.is_visible(filter_state) {
                            visibility.remove(VisibilityFlags::TOGGLE);
                        }
                    }
                }
                if *visibility & VisibilityFlags::TOGGLES != prev {
                    dirty = true;
                }
            }
        }
        dirty
    }

    fn visibility_update(&self, map_id: MapIndex) -> impl Iterator<Item = (PackPath, Box<[VisibilityFlags]>, Box<[VisibilityFlags]>)> + '_ {
        self.map_packs.iter()
            .filter(move |(path, _)| path.path == map_id)
            .map(|(path, map_pack)| {
                let pois: Box<[VisibilityFlags]> = map_pack.pois.iter().map(|poi| poi.visibility).collect();
                let trails: Box<[VisibilityFlags]> = map_pack.trails.iter().map(|trail| trail.visibility).collect();
                (path.root.clone(), pois, trails)
            })
    }

    pub(super) async fn visibility_send(&mut self, map_id: MapIndex) {
        let update: Vec<_> = self.visibility_update(map_id).collect();

        let res = Controller::try_run_render(RenderTaskPriority::High, move |state| -> anyhow::Result<()> {
            let Some(Ok(engine)) = &mut state.engine else {
                log::debug!("no engine on other end of visibility_send?");
                return Ok(())
            };
            for (path, pois, trails) in update {
                let Some(pack) = engine.packs.loaded_packs.get_mut(path.path as usize) else {
                    continue
                };
                for (active, visibility) in pack.active_pois.iter_mut().zip(pois) {
                    active.visibility = visibility;
                }
                for (active, visibility) in pack.active_trails.iter_mut().zip(trails) {
                    active.visibility = visibility;
                }
            }
            Ok(())
        }).await.context("updating render visibility");
        if let Err(e) = res {
            log::error!("{e:#}");
        }
    }

    pub(crate) async fn set_visible(&mut self, context: Option<MapContext>, set: Option<bool>) {
        let set = {
            let mut settings = self.loader.settings.write().await;
            let pathing = settings.pathing_mut();
            let (is_visible, out) = match context {
                Some(MapContext::Global) => (
                    pathing.space.visible_worldmap(),
                    &mut pathing.space.visible_map_world,
                ),
                Some(MapContext::Minimap) => (
                    pathing.space.visible_minimap(),
                    &mut pathing.space.visible_map_mini,
                ),
                None => (pathing.space.visible_space(), &mut pathing.space.visible_space),
            };
            let set = set.unwrap_or(!is_visible);
            *out = Some(set);
            settings.mark_dirty();
            set
        };

        #[cfg(feature = "goggles")]
        match (context, set) {
            (None, true) =>
                Engine::try_send(SpaceEvent::GogglesRefreshLens { force: false, delay_override: Some(2) }),
            (None, false) => Engine::try_send(SpaceEvent::GogglesClearLens),
            _ => (),
        }
        Engine::try_send(SpaceEvent::SettingsDirty);
    }
}
