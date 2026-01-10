use {
    crate::{
        controller::pathing::{
            registry::{PackCategoryInfo, PackLoader, PackMapPath, PackPath},
            shared::SharedPackConfig,
            info::MapPackInfo,
            state::{LoadedCategory, LoadedMapPack},
            ExternalFilterState,
            PathingController,
            VisibilityFlagsExt,
        },
        settings::PathingSettings,
        space::{engine::SpaceEvent, Engine},
    },
    std::{
        collections::{BTreeMap, HashSet, VecDeque},
        iter,
        mem,
        sync::Arc,
    },
    taimi_hoard::loc::LocationRef,
    taimi_meta::{
        packs::{
            collections::CategorySet,
            CategoryIndex,
            CategoryPath,
            MapIndex,
            MarkerIndex,
            MarkerPath,
            VisibilityFlags,
        },
        ui::MapContext,
    },
    taimi_pack::{attributes::FilterAttributes, category::id::AsFullId, Pack},
    taimi_sync::watched::watch,
};

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
    pub fn fill_settings(
        &mut self,
        pack: &Pack,
        pathing: &PathingSettings,
        disabled_paths: &HashSet<String>,
    ) {
        for id in disabled_paths {
            let id = &id[..];
            let Some((i, _id, cat)) = pack.categories.all_categories.get_full(id) else {
                continue
            };
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
            let disabled_cats = pack
                .categories
                .all_categories
                .iter()
                .enumerate()
                .filter(|(_, (_, cat))| !cat.default_toggle());
            for (i, (full_id, _disabled_cat)) in disabled_cats {
                let path = CategoryPath::with_path(i as CategoryIndex);
                if !disabled_paths.contains(&full_id.id_to_str()[..]) {
                    let mut vis = self
                        .category_visibility
                        .get(&path)
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
        self.category_visibility
            .get(&path)
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
        categories.root_paths().any(|path| {
            let default_toggle = !categories.disabled.contains(path);
            let deviation = self.visibility_deviation_for(path).is_visible();
            default_toggle ^ deviation
        })
    }

    pub fn is_empty(&self) -> bool {
        self.category_visibility.is_empty() && self.visibility_overrides.is_empty()
    }
}

impl PathingController {
    pub(super) async fn handle_config_change(
        &mut self,
        path: PackPath,
        config: &watch::Receiver<SharedPackConfig>,
    ) {
        self.trim_inactive_maps(false);
        let Some((info, _info)) = self.packs.lookup_info(path) else { return };
        let mut dirty = false;
        let mut external_filters = None;
        for (map_path, map, map_info) in self.maps.iter_pack_mut_with_info(&self.map_info, path) {
            {
                let config = config.borrow();
                let damage = map.update_category_config(&map_info, &info.categories, &config.config);
                if let Ok(true) = damage {
                    continue
                }
                map.refresh_categories(&map_info, &info.categories, &config.config, damage.err().as_ref());
                // TODO: filter out unaffected pack+maps here using damage
                dirty |= true;
            }
            let rx = &self.rx;
            let external_filters = external_filters.get_or_insert_with(move || rx.get_filter_state());
            dirty |=
                Self::update_loaded_visibility_inner(map_path, map, map_info, Some(&*external_filters));
        }
        if dirty {
            let maps = self.maps.iter_pack_with_info(&self.map_info, path);
            self.loader.update_map_states(true, true, &mut { maps });
        }
    }
    pub(super) fn reload_config_for(&mut self, path: PackPath) {
        let Some(info) = self.packs.lookup_ref(&path) else { return };
        if !info.is_loaded() {
            return
        }

        log::info!("TODO: config reload {path}? did activate not do this?");
    }

    pub(super) async fn handle_toggle(
        loader: &PackLoader,
        path: CategoryPath<PackPath>,
        state: Option<bool>,
    ) {
        // TODO: rethink whether controller wants to use loader like this or not?
        let pack_info = loader.pack_info(path.root);
        let categories = pack_info.as_ref().and_then(|pack_info| pack_info.category_info());
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
    pub(super) async fn category_commit_vis<C>(loader: &PackLoader, pack_path: PackPath, dirty_cats: C)
    where
        C: IntoIterator<Item = CategoryPath>,
    {
        let pack_info = loader.pack_info(pack_path);
        let categories = pack_info.as_ref().and_then(|pack_info| pack_info.category_info());
        let config = loader.pack_config(pack_path);
        let (Some((categories, _)), Some(config)) = (categories, config) else {
            log::error!("cannot save category settings for unloaded {pack_path}");
            return
        };
        let changes = {
            // TODO: avoid collect but also avoid borrowing or copying config :<
            let config = config.borrow();
            dirty_cats
                .into_iter()
                .map(|path| {
                    let default = !categories.disabled.contains(path);
                    let vis = config.config.visibility_deviation_for(path);
                    let state = vis.is_visible() ^ default;
                    (path, state)
                })
                .collect::<Vec<_>>()
        };
        Self::category_commit_vis_set(loader, pack_path, changes).await
    }
    pub(super) async fn category_commit_vis_set<C>(loader: &PackLoader, pack_path: PackPath, dirty_cats: C)
    where
        C: IntoIterator<Item = (CategoryPath, bool)>,
    {
        #[cfg(todo)]
        if dirty_cats.is_empty() {
            return
        }
        let Some(pack) = loader.get_pack_loaded_data(pack_path) else { return };
        let mut settings = None;
        for (path, vis_state) in dirty_cats {
            let full_id = pack
                .categories
                .all_categories
                .get_index(path.path as usize)
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
    pub(super) fn update_loaded_visibility_inner(
        path: PackMapPath,
        map_pack: &mut LoadedMapPack,
        map_info: &MapPackInfo,
        filter_state: Option<&ExternalFilterState>,
    ) -> bool {
        let pois = map_info.pois().zip(map_pack.pois.iter_mut());
        let pois = pois.map(|(poi_path, poi)| {
            let marker_path = MarkerPath::with_parts(path, MarkerIndex::from(poi_path));
            (
                marker_path,
                poi.category_path(),
                &mut poi.visibility,
                poi.info.get_filter_attrs(),
            )
        });
        let trails = map_info.trails().zip(map_pack.trails.iter_mut());
        let trails = trails.map(|(trail_path, trail)| {
            let marker_path = MarkerPath::with_parts(path, MarkerIndex::from(trail_path));
            (
                marker_path,
                trail.category_path(),
                &mut trail.visibility,
                trail.info.get_filter_attrs(),
            )
        });
        let mut dirty = false;
        for (_marker_path, category_index, visibility, filters) in pois.chain(trails) {
            let prev = *visibility & VisibilityFlags::TOGGLES;
            *visibility = visibility.restore_default_toggles();
            let cat_vis = map_info
                .category_index(category_index)
                .and_then(|i| map_pack.categories.get(i.path as usize))
                .map(|cat| cat.visibility);
            if let Some(cat_vis) = cat_vis {
                // TODO: if cat vis is override, set directly or something!
                visibility.set_toggles(
                    (cat_vis & VisibilityFlags::TOGGLE) | (*visibility & !VisibilityFlags::TOGGLE),
                );
                visibility.set(VisibilityFlags::TOGGLE, cat_vis.contains(VisibilityFlags::TOGGLE));
            }
            match (filters, filter_state) {
                (Some(filters), Some(filter_state)) if visibility.is_visible() => {
                    if Self::is_filtered(filters, filter_state) {
                        visibility.remove(VisibilityFlags::TOGGLE);
                    }
                },
                _ => (),
            }
            #[cfg(todo)]
            if visibility.is_visible() {
                if let Some(filter) = &filter {
                    if let filter::FILTER_HIDDEN = filter.is_visible(filter_state) {
                        visibility.remove(VisibilityFlags::TOGGLE);
                    }
                }
            }
            #[cfg(todo)]
            if visibility.is_visible() {
                let marker_path: MarkerPath = MarkerPath::with_path(marker_path.path);
                if let Some(hidden) = guid.and_then(|guid| map_filters.group_filter_for(marker_path, guid))
                {
                    if let filter::FILTER_HIDDEN = hidden.is_visible(filter_state) {
                        visibility.remove(VisibilityFlags::TOGGLE);
                    }
                }
            }
            if *visibility & VisibilityFlags::TOGGLES != prev {
                dirty = true;
            }
        }
        dirty
    }
    #[cfg(todo = "unused")]
    pub(super) fn update_loaded_visibility_for(&mut self, pack_path: PackPath) -> bool {
        let map_id = self.gameplay_map();
        #[cfg(todo)]
        let hidden_guids =
            SaveState::read_with(|s| s.pathing_state.as_ref().map(|p| p.hidden_guid_expiry.clone()));
        #[cfg(todo)]
        let filter_state = &self.filter_state;
        let mut dirty = false;
        for (path, map_pack, map_info) in self.maps.iter_pack_mut_with_info(&self.map_info, pack_path) {
            if Self::update_loaded_visibility_inner(path, map_pack, map_info) && Some(path.path) == map_id {
                dirty = true;
            }
        }
        dirty
    }
    pub(super) fn update_loaded_visibility(&mut self) -> bool {
        let map_id = self.gameplay_map();
        #[cfg(todo)]
        let hidden_guids =
            SaveState::read_with(|s| s.pathing_state.as_ref().map(|p| p.hidden_guid_expiry.clone()));
        #[cfg(todo)]
        let filter_state = &self.filter_state;
        let mut external_filters = None;
        let mut dirty = false;
        for (path, map_pack, map_info) in self.maps.iter_mut_with_info(&self.map_info, None) {
            let rx = &self.rx;
            let external_filters = external_filters.get_or_insert_with(move || rx.get_filter_state());
            if Self::update_loaded_visibility_inner(path, map_pack, map_info, Some(&*external_filters))
                && Some(path.path) == map_id
            {
                dirty = true;
            }
        }
        if dirty {
            self.mark_map_state_dirty(map_id, true);
        }
        dirty
    }
    fn is_filtered(filters: &FilterAttributes, external: &ExternalFilterState) -> bool {
        let (festivals, clears, achievements) = external;
        if let Some(id) = filters.achievement_id {
            let completed = filters
                .achievement_bit
                .and_then(|bit| achievements.is_bit_complete(id as _, bit as _))
                .unwrap_or_else(|| achievements.is_complete(id as _));
            if completed {
                return true
            }
        }
        if let Some(raids) = &filters.raids {
            let completed = !raids.is_empty() && raids.iter().all(|r| clears.contains(r));
            if completed {
                return true
            }
        }
        if let Some(f) = &filters.festivals {
            if !f.is_empty() && !f.intersects(*festivals) {
                return true
            }
        }
        false
    }
    /// limited support atm
    pub(super) fn can_filter(filters: &FilterAttributes) -> bool {
        match filters.achievement_id {
            None | Some(0) => (),
            Some(..) => return true,
        }
        match filters.festivals {
            Some(f) if !f.is_empty() => return true,
            _ => (),
        }
        match &filters.raids {
            Some(r) if !r[..].is_empty() => return true,
            _ => (),
        }

        false
    }

    fn visibility_update(
        &self,
        map_id: MapIndex,
    ) -> impl Iterator<Item = (PackPath, Box<[VisibilityFlags]>, Box<[VisibilityFlags]>)> + '_ {
        self.maps.iter(Some(map_id)).map(|(path, map_pack)| {
            let pois: Box<[VisibilityFlags]> = map_pack.pois.iter().map(|poi| poi.visibility).collect();
            let trails: Box<[VisibilityFlags]> =
                map_pack.trails.iter().map(|trail| trail.visibility).collect();
            (path.root.clone(), pois, trails)
        })
    }

    /// resolves state from optional toggle instruction to absolute
    pub(super) async fn set_visible_settings(
        &mut self,
        context: Option<MapContext>,
        set: Option<bool>,
    ) -> bool {
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

impl LoadedMapPack {
    /// Only updates default flags
    ///
    /// [self.categories] are dirty and require further processing unless `Ok(true)`
    ///
    /// TODO: starting damage mask via parameter and/or info sig instead
    pub fn update_category_config(
        &mut self,
        info: &MapPackInfo,
        categories: &PackCategoryInfo,
        config: &PackConfig,
    ) -> Result<bool, CategorySet> {
        let mut damage = match self.categories.len() {
            loaded if info.category_count() != loaded => {
                self.categories = iter::repeat(LoadedCategory::INVALID)
                    .take(info.category_count())
                    .collect();
                None
            },
            _ => Some(CategorySet::default()),
        };

        let mut loaded: Result<&mut [LoadedCategory], &mut Arc<[LoadedCategory]>> =
            Err(&mut self.categories);
        for (i, path) in info.categories().enumerate() {
            let Some(prev) = match &mut loaded {
                Ok(c) => &c[..],
                Err(c) => &c[..],
            }
            .get(i) else {
                continue
            };
            let prev_defaults = prev.visibility & VisibilityFlags::DEFAULTS;
            let defaults = categories
                .visibility
                .get_for(path)
                .unwrap_or(VisibilityFlags::TOGGLES);
            let deviation = config.visibility_deviation_for(path);
            let default_toggles = defaults ^ deviation;
            let defaults = default_toggles.toggles_to_default();
            let is_override_clean = !config.visibility_overrides.contains(path)
                || prev.visibility & VisibilityFlags::TOGGLES == default_toggles;
            if damage.is_some() && defaults == prev_defaults && is_override_clean {
                continue
            }

            let out = unsafe {
                match (mem::replace(&mut loaded, Ok(&mut [])), &mut loaded) {
                    (Ok(loaded), Ok(out)) => {
                        *out = loaded;
                        out
                    },
                    (Err(loaded), Ok(out)) => {
                        *out = Arc::make_mut(loaded);
                        out
                    },
                    #[cfg(debug_assertions)]
                    (_, Err(..)) => unreachable!(),
                    #[cfg(not(debug_assertions))]
                    (_, Err(..)) => continue,
                }
                .get_unchecked_mut(i)
            };
            out.visibility.set_defaults(defaults);

            if let Some(damage) = &mut damage {
                damage.insert(path);
            }
        }

        match damage {
            Some(mut damage) => {
                // not necessarily likely that multiple changes occur at once, but...
                // we only care about the root-most changes since they propagate down
                let mut redundant_roots = Vec::new();
                for damaged in damage.paths() {
                    let is_redundant = categories.ancestors_of(damaged).any(|p| damage.contains(p));
                    if is_redundant {
                        redundant_roots.push(damaged);
                    }
                }
                for redundant in redundant_roots {
                    damage.remove(redundant);
                }

                if !damage.is_empty() {
                    Err(damage)
                } else {
                    Ok(true)
                }
            },
            None => Ok(false),
        }
    }

    pub fn refresh_categories(
        &mut self,
        info: &MapPackInfo,
        categories: &PackCategoryInfo,
        config: &PackConfig,
        damage: Option<&CategorySet>,
    ) {
        let default_roots = damage.is_none().then(|| categories.root_paths());
        let roots = damage
            .into_iter()
            .flat_map(|damage| damage.paths())
            .chain(default_roots.into_iter().flatten());

        // roots should be independent subtrees, but just in case..
        let mut children: VecDeque<_> = roots
            .map(|root_path| (root_path, config.visibility_overrides.contains(root_path), None))
            .collect();

        let pack_default = VisibilityFlags::TOGGLES;
        let loaded = Arc::make_mut(&mut self.categories);
        while let Some((path, parent_is_override, parent_vis)) = children.pop_front() {
            let Some(index) = info.category_index(path) else {
                // rest of tree should be irrelevant
                continue
            };
            let is_override = config.visibility_overrides.contains(path);
            let default_vis = loaded
                .get(index.path as usize)
                .map(|cat| cat.visibility.default_toggles());
            let visibility = match is_override {
                true => default_vis,
                false => {
                    let inherited = parent_vis.or_else(|| {
                        categories
                            .parent_of(path)
                            .map(|parent| {
                                info.category_index(parent)
                                    .and_then(|i| loaded.get(i.path as usize))
                                    .map(|parent| parent.visibility & VisibilityFlags::TOGGLES)
                                    .or_else(|| categories.visibility.get_for(parent))
                            })
                            .unwrap_or(default_vis)
                    });
                    match parent_is_override {
                        true => inherited, /*.or(default_vis)*/
                        false => inherited.map(|inh| {
                            (inh & default_vis.unwrap_or(VisibilityFlags::TOGGLES)
                                & VisibilityFlags::TOGGLE)
                                | (default_vis.unwrap_or(VisibilityFlags::TOGGLES)
                                    & !VisibilityFlags::TOGGLE)
                        }),
                    }
                },
            }
            .unwrap_or(pack_default);
            if let Some(loaded) = loaded.get_mut(index.path as usize) {
                loaded.visibility.set_toggles(visibility);
            }
            children.extend(
                categories
                    .children_of(path)
                    .map(|c| (c, is_override, Some(visibility))),
            );
        }
    }
}
