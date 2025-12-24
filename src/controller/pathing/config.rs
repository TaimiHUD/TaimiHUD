use {
    crate::{
        controller::{
            pathing::{
                registry::{PackLoader, PackPath, PackConfig},
                shared::SharedPacks,
                visible::VisibilityFlags,
                PathingController, PathingEvent
            }, Controller
        },
        render::machine::RenderTaskPriority,
        settings::PathingSettings,
        space::{
            engine::SpaceEvent, Engine
        },
    },
    taimi_hoard::loc::LocationRef,
    taimi_meta::packs::{
        CategoryPath, MapIndex, MarkerIndex, MarkerPath, 
    },
    anyhow::Context,
    std::{iter, sync::Arc},
    taimi_meta::ui::MapContext,
};

impl PathingController {
    pub(super) async fn handle_config_change(&mut self, path: PackPath, config: &PackConfig) {
        let Some((_info, info)) = self.packs.lookup_info(path) else { return };
        let mut dirty = false;
        for (map_path, map, map_info) in self.maps.iter_pack_mut_with_info(&self.map_info, path) {
            {
                let damage = map.update_category_config(&map_info, &info.categories, &config);
                if let Ok(true) = damage {
                    continue
                }
                map.refresh_categories(&map_info, &info.categories, &config, damage.err().as_ref());
            }
            dirty = true;
            self.loader.shared.gameplay.send_if_modified(|shared_map| {
                let Some(shared_state) = shared_map.get_state_mut(map_path) else {
                    return false
                };
                shared_state.categories = map.categories.clone();
                false
            });
        }
        if dirty {
            self.loader.shared.gameplay.send_if_modified(|_| true);
            //ctx.filter_state_signal = true;
            PathingEvent::RequestDisabledPaths.try_send();
        }
    }

    pub(super) async fn handle_toggle(loader: &PackLoader, path: CategoryPath<PackPath>, state: Option<bool>) {
        // TODO: rethink whether controller wants to use loader like this or not?
        let pack_info = loader.pack_info(path.root);
        let categories = pack_info.as_ref().and_then(|pack_info|
            pack_info.category_info()
        );
        let config = loader.pack_config(path.root);
        let (Some((categories, _)), Some(config)) = (categories, config) else {
            log::error!("can't update {path}={state:?}, no config state?");
            return
        };
        let cat_vis = !categories.disabled.contains(path);
        let toggle_dev = state.map(|state| cat_vis ^ state);

        let mut state = state.unwrap_or(false);
        let changed = config.send_if_modified(|config| {
            let path = path.unscope();
            let prev = config.config.visibility_deviation_for(path);
            let mut state_dev = prev;
            state = toggle_dev.unwrap_or(!prev.contains(VisibilityFlags::TOGGLE));
            state_dev.set(VisibilityFlags::TOGGLE, state);
            if prev == state_dev {
                return false
            }
            config.config.set_visibility_deviation(path, state_dev);
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
        let pack_info = loader.pack_info(pack_path);
        let categories = pack_info.as_ref().and_then(|pack_info|
            pack_info.category_info()
        );
        let config = loader.pack_config(pack_path);
        let (Some((categories, _)), Some(config)) = (categories, config) else {
            log::error!("cannot save category settings for unloaded {pack_path}");
            return
        };
        let changes = {
            // TODO: avoid collect but also avoid borrowing or copying config :<
            let config = config.borrow();
            dirty_cats.into_iter()
            .map(|path| {
                let default = !categories.disabled.contains(path);
                let vis = config.config.visibility_deviation_for(path);
                let state = vis.is_visible() ^ default;
                (path, state)
            }).collect::<Vec<_>>()
        };
        Self::category_commit_vis_set(loader, pack_path, changes).await
    }
    pub(super) async fn category_commit_vis_set<C>(loader: &PackLoader, pack_path: PackPath, dirty_cats: C) where
        C: IntoIterator<Item = (CategoryPath, bool)>,
    {
        #[cfg(todo)]
        if dirty_cats.is_empty() {
            return
        }
        let Some(pack) = loader.get_pack_loaded_data(pack_path) else { return };
        let mut settings = None;
        for (path, vis_state) in dirty_cats {
            let full_id =
                    pack.categories.all_categories.get_index(path.path as usize)
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

    pub(super) fn update_loaded_visibility(&mut self) -> bool {
        let map_id = self.rx.gameplay.cached.as_ref().and_then(|g| g.gameplay_map());
        #[cfg(todo)]
        let hidden_guids = SaveState::read_with(|s| s.pathing_state.as_ref().map(|p| p.hidden_guid_expiry.clone()));
        #[cfg(todo)]
        let filter_state = &self.filter_state;
        let mut dirty = false;
        for (path, map_pack, map_info) in self.maps.iter_mut_with_info(&self.map_info, None) {
            let pois = map_info.pois().zip(
                map_pack.pois.iter_mut()
            );
            let pois = pois
                .map(|(poi_path, poi)| {
                    let marker_path = MarkerPath::with_parts(path, MarkerIndex::from(poi_path));
                    (marker_path, poi.category_path(), &mut poi.visibility)
                });
            let trails = map_info.trails().zip(
                map_pack.trails.iter_mut()
            );
            let trails = trails
                .map(|(trail_path, trail)| {
                    let marker_path = MarkerPath::with_parts(path, MarkerIndex::from(trail_path));
                    (marker_path, trail.category_path(), &mut trail.visibility)
                });
            for (_marker_path, category_index, visibility) in pois.chain(trails) {
                let prev = *visibility & VisibilityFlags::TOGGLES;
                *visibility = visibility.restore_default_toggles();
                let cat_vis = map_info.category_index(category_index)
                    .and_then(|i| map_pack.categories.get(i.path as usize))
                    .map(|cat| cat.visibility);
                if let Some(cat_vis) = cat_vis {
                    // TODO: if cat vis is override, set directly or something!
                    visibility.set_toggles((cat_vis & VisibilityFlags::TOGGLE) | (*visibility & !VisibilityFlags::TOGGLE));
                    visibility.set(VisibilityFlags::TOGGLE, cat_vis.contains(VisibilityFlags::TOGGLE));
                }
                if visibility.is_visible() {
                    #[cfg(todo)]
                    if let Some(filter) = &filter {
                        if let filter::FILTER_HIDDEN = filter.is_visible(filter_state) {
                            visibility.remove(VisibilityFlags::TOGGLE);
                        }
                    }
                }
                #[cfg(todo)]
                if visibility.is_visible() {
                    let marker_path: MarkerPath = MarkerPath::with_path(marker_path.path);
                    if let Some(hidden) = guid.and_then(|guid| map_filters.group_filter_for(marker_path, guid)) {
                        if let filter::FILTER_HIDDEN = hidden.is_visible(filter_state) {
                            visibility.remove(VisibilityFlags::TOGGLE);
                        }
                    }
                }
                if *visibility & VisibilityFlags::TOGGLES != prev && Some(path.path) == map_id {
                    dirty = true;
                }
            }
        }
        dirty
    }

    fn visibility_update(&self, map_id: MapIndex) -> impl Iterator<Item = (PackPath, Box<[VisibilityFlags]>, Box<[VisibilityFlags]>)> + '_ {
        self.maps.iter(Some(map_id))
            .map(|(path, map_pack)| {
                let pois: Box<[VisibilityFlags]> = map_pack.pois.iter().map(|poi| poi.visibility).collect();
                let trails: Box<[VisibilityFlags]> = map_pack.trails.iter().map(|trail| trail.visibility).collect();
                (path.root.clone(), pois, trails)
            })
    }

    #[cfg(todo = "deleteme")]
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

    /// resolves state from optional toggle instruction to absolute
    pub(super) async fn set_visible_settings(&mut self, context: Option<MapContext>, set: Option<bool>) -> bool {
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

        Engine::try_send(SpaceEvent::SettingsDirty);
        set
    }
}
