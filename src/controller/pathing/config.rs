use {
    crate::{
        controller::pathing::{
            info::MapPackInfo,
            registry::{LoadedMarkerPath, PackCategoryInfo, PackLoader, PackMapPath, PackPath},
            shared::{LocDisplay, SharedPackConfig},
            state::{
                filter::{self, FilterState},
                LoadedCategory,
                LoadedMapPack,
            },
            ExternalFilterState,
            PathingController,
            PathingEvent,
            VisibilityFlagsExt,
        },
        exports::runtime::{self as rt, bindings::TaimiControls},
        settings::{PathingSettings, Settings, SettingsLock},
        space::{engine::SpaceEvent, Engine},
    },
    anyhow::Context,
    std::{
        collections::{BTreeMap, HashSet, VecDeque},
        iter,
        mem,
        sync::Arc,
    },
    taimi_hoard::{loc::LocationRef, time::Timestamp},
    taimi_meta::{
        packs::{
            collections::CategorySet,
            id::{MarkerId, MarkerIndex, MarkerPath},
            CategoryIndex,
            CategoryPath,
            MapIndex,
            VisibilityFlags,
        },
        ui::MapContext,
    },
    taimi_pack::{
        attributes::{FilterAttributes, MarkerAttributes},
        category::id::{AsFullId, IdNameBox},
        Pack,
    },
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
            dirty |=
                Self::update_loaded_visibility_inner(map_path, map, map_info, Some(&self.filter_state));
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

    pub(super) async fn process_category_set(&mut self, path: CategoryPath<PackPath>, state: Option<bool>) {
        let commit = Self::handle_toggle(&self.loader, path, state);
        if let Some(commit) = commit {
            self.handle_toggle_post(path.root, commit).await;
        }
    }
    pub(super) fn handle_toggle(
        loader: &Arc<PackLoader>,
        path: CategoryPath<PackPath>,
        state: Option<bool>,
    ) -> Option<(CategoryPath, bool)> {
        // TODO: rethink whether controller wants to use loader like this or not?
        let pack_info = loader.pack_info(path.root);
        let categories = pack_info.as_ref().and_then(|pack_info| pack_info.category_info());
        let config = loader.pack_config(path.root);
        let (Some((categories, _)), Some(config)) = (categories, config) else {
            log::error!("can't update {path}={state:?}, no config state?");
            return None
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

        changed.then_some((path.unscope(), cat_vis ^ state))
    }
    async fn handle_toggle_post(&mut self, pack_path: PackPath, cat_vis: (CategoryPath, bool)) {
        self.category_commit_vis_post(pack_path, iter::once(cat_vis))
            .await
    }
    async fn category_commit_vis_post<C>(&mut self, pack_path: PackPath, dirty_cats: C)
    where
        C: IntoIterator<Item = (CategoryPath, bool)> + Send + 'static,
    {
        if let Some(pack) = self.loader.get_pack_loaded_data(pack_path) {
            Self::category_commit_vis_save(&self.loader, &pack, dirty_cats).await
        } else {
            let loader = self.loader.clone();
            self.tasks
                .spawn(Self::category_commit_vis_task(loader, pack_path, dirty_cats));
        }
    }
    pub(super) async fn process_category_set_id(
        &mut self,
        pack_path: PackPath,
        id: IdNameBox,
        state: Option<bool>,
    ) {
        if let Some(pack) = self.loader.get_pack_loaded_data(pack_path) {
            let res = Self::category_path_for_id(pack_path, &pack, &id);
            if let Some(cat_path) = rt::log::error_ok(res) {
                let commit = Self::handle_toggle(&self.loader, cat_path.pivot(pack_path), state);
                if let Some(commit) = commit {
                    Self::category_commit_vis_save(&self.loader, &pack, iter::once(commit)).await
                }
            }
        } else {
            self.tasks.spawn(Self::task_category_set_id(
                self.loader.clone(),
                pack_path,
                id,
                state,
            ));
        }
    }
    pub(super) async fn task_category_set_id(
        loader: Arc<PackLoader>,
        pack_path: PackPath,
        id: IdNameBox,
        state: Option<bool>,
    ) -> anyhow::Result<PathingEvent> {
        if let Some(pack) = loader.pack_data_for(pack_path).await {
            let cat_path = Self::category_path_for_id(pack_path, &pack, &id)?;
            let commit = Self::handle_toggle(&loader, cat_path.pivot(pack_path), state);
            if let Some(commit) = commit {
                Self::category_commit_vis_save(&loader, &pack, iter::once(commit)).await;
            }
        }
        Ok(PathingEvent::Nop)
    }
    fn category_path_for_id(
        pack_path: PackPath,
        pack: &Pack,
        id: &IdNameBox,
    ) -> anyhow::Result<CategoryPath> {
        pack.categories
            .all_categories
            .get_full(id.as_id())
            .map(|(i, ..)| CategoryPath::with_path(i as CategoryIndex))
            .with_context(|| {
                let pack_path = LocDisplay(pack_path);
                format!("{pack_path} missing category {}", id.as_str())
            })
    }
    pub(super) async fn category_commit_vis(
        &mut self,
        pack_path: PackPath,
        dirty_cats: &mut (dyn Iterator<Item = CategoryPath> + Send),
    ) {
        let pack_info = self.loader.pack_info(pack_path);
        let categories = pack_info.as_ref().and_then(|pack_info| pack_info.category_info());
        let config = self.loader.pack_config(pack_path);
        let (Some((categories, _)), Some(config)) = (categories, config) else {
            log::error!("cannot save category settings for unloaded {pack_path}");
            return
        };
        let changes = {
            // TODO: avoid collect but also avoid borrowing or copying config :<
            let config = config.borrow();
            dirty_cats
                .map(|path| {
                    let default = !categories.disabled.contains(path);
                    let vis = config.config.visibility_deviation_for(path);
                    let state = vis.is_visible() ^ default;
                    (path, state)
                })
                .collect::<Vec<_>>()
        };
        self.category_commit_vis_post(pack_path, changes).await
    }
    async fn category_commit_vis_task<C>(
        loader: Arc<PackLoader>,
        pack_path: PackPath,
        dirty_cats: C,
    ) -> anyhow::Result<PathingEvent>
    where
        C: IntoIterator<Item = (CategoryPath, bool)> + Send,
    {
        #[cfg(todo)]
        if dirty_cats.is_empty() {
            return
        }
        if let Some(pack) = loader.pack_data_for(pack_path).await {
            Self::category_commit_vis_save(&loader, &pack, dirty_cats).await;
        }
        Ok(PathingEvent::Nop)
    }
    async fn category_commit_vis_save<C>(loader: &PackLoader, pack: &Pack, dirty_cats: C)
    where
        C: IntoIterator<Item = (CategoryPath, bool)> + Send,
    {
        let mut settings = loader.settings.write().await;
        Self::category_commit_vis_write(&mut settings, pack, &mut dirty_cats.into_iter())
    }
    fn category_commit_vis_write(
        settings: &mut Settings,
        pack: &Pack,
        dirty_cats: &mut dyn Iterator<Item = (CategoryPath, bool)>,
    ) {
        for (path, vis_state) in dirty_cats {
            let full_id = pack
                .categories
                .all_categories
                .get_index(path.path as usize)
                .map(|(_id, cat)| cat.full_id.clone());
            if let Some(full_id) = full_id {
                settings.pathing_state_update(full_id.to_string(), vis_state);
            } else {
                log::warn!("{path} not found for toggle state update");
            }
        }
    }
    pub(super) fn update_loaded_visibility_inner(
        path: PackMapPath,
        map_pack: &mut LoadedMapPack,
        map_info: &MapPackInfo,
        filter_state: Option<&FilterState>,
    ) -> bool {
        let poi_guids = {
            let mut poi_guids = map_pack.poi_guids.iter();
            map_info
                .poi_guid_mask()
                .map(move |has| has.then(|| poi_guids.next()).flatten())
        };
        let pois = map_info
            .loaded_pois()
            .zip(map_pack.pois.iter_mut())
            .zip(poi_guids)
            .map(|(((lpoi_path, poi_path), poi), guid)| {
                let marker_path: MarkerPath = poi_path.pivot_from();
                let lpath: LoadedMarkerPath = lpoi_path.pivot_to();
                (
                    marker_path,
                    MarkerPath::with_parts(path, lpath.path),
                    poi.category_path(),
                    &mut poi.visibility,
                    poi.info.get_filter_attrs(),
                    guid,
                )
            });
        let trail_guids = {
            let mut trail_guids = map_pack.trail_guids.iter();
            map_info
                .trail_guid_mask()
                .map(move |has| has.then(|| trail_guids.next()).flatten())
        };
        let trails = map_info
            .loaded_trails()
            .zip(map_pack.trails.iter_mut())
            .zip(trail_guids)
            .map(|(((ltrail_path, trail_path), trail), guid)| {
                let marker_path: MarkerPath = trail_path.pivot_from();
                let lpath: LoadedMarkerPath = ltrail_path.pivot_to();
                (
                    marker_path,
                    MarkerPath::with_parts(path, lpath.path),
                    trail.category_path(),
                    &mut trail.visibility,
                    trail.info.get_filter_attrs(),
                    guid,
                )
            });
        let mut dirty = false;
        for (marker_path, lpath, category_index, visibility, filters, guid) in pois.chain(trails) {
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
                (Some(filters), Some(filter_state)) if visibility.is_visible() =>
                    if let filter::FILTER_HIDDEN =
                        filter::FilterConfig::filters_is_visible(filters, filter_state)
                    {
                        visibility.remove(VisibilityFlags::TOGGLE);
                    },
                _ => (),
            }
            let guid_filter_state = match filter_state {
                _ if !visibility.is_visible() => None,
                Some(filter_state) if filter_state.hidden.is_empty() => None,
                f => f,
            };
            if let Some(filter_state) = guid_filter_state {
                let inverted = filters.as_ref().map(|f| f.invert_behavior()).unwrap_or(false);
                // TODO: use GroupConfig properly here and move most of this into a method!
                let marker_path: MarkerPath<PackPath> =
                    MarkerPath::with_parts(lpath.root.root, marker_path.path);
                let marker_id = MarkerId::for_marker(marker_path);
                let lmarker_id = MarkerId::for_marker(lpath);
                let guid_id = guid
                    .as_ref()
                    .and_then(|guid| (!guid.0.is_nil()).then_some(MarkerId::from_uuid_ref(&guid.0)));
                let marker_ids: [Option<&MarkerId>; 3] = [Some(&marker_id), Some(&lmarker_id), guid_id];
                let filtered = IntoIterator::into_iter(marker_ids).flatten().any(|mid| {
                    filter_state
                        .hidden
                        .is_hidden(mid, &filter_state.map, &filter_state.character)
                });
                if filtered ^ inverted {
                    visibility.remove(VisibilityFlags::TOGGLE);
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
    /// TODO: if individual maps can be marked dirty or something this could be removed
    pub(super) const ALLOW_INCOMPLETE_VIS_UPDATE: bool = true;
    /// XXX: after a [partial update](Self::ALLOW_INCOMPLETE_VIS_UPDATE) (all=false), an unconditional update on map switch will be
    /// required!!
    pub(super) fn update_loaded_visibility(&mut self, all: bool) -> bool {
        let map_id = self.gameplay_map();
        #[cfg(todo)]
        let hidden_guids =
            SaveState::read_with(|s| s.pathing_state.as_ref().map(|p| p.hidden_guid_expiry.clone()));
        let mut dirty = false;
        let update_map_id = match (map_id, all) {
            (None, false) => return dirty,
            (Some(map_id), false) => Some(map_id),
            (_, true) => None,
        };
        for (path, map_pack, map_info) in self.maps.iter_mut_with_info(&self.map_info, update_map_id) {
            if Self::update_loaded_visibility_inner(path, map_pack, map_info, Some(&self.filter_state))
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
    /// deleteme
    pub(super) fn can_filter(filters: &FilterAttributes) -> bool {
        filter::FilterConfig::filters_is_empty(filters)
    }
    /// interested in retaining full attrs in memory
    fn marker_wants_attrs(ispoi: bool, attrs: &MarkerAttributes) -> bool {
        let can_filter = attrs
            .filters
            .as_ref()
            .map(|f| Self::can_filter(f))
            .unwrap_or(false);
        if can_filter {
            return true
        }

        let can_interact = ispoi
            && attrs
                .interaction
                .as_ref()
                .map(|i| Self::can_interact(i))
                .unwrap_or(false);
        if can_interact {
            return true
        }

        false
    }
    pub(super) fn trail_wants_attrs(attrs: &MarkerAttributes) -> bool {
        Self::marker_wants_attrs(false, attrs)
    }
    pub(super) fn poi_wants_attrs(attrs: &MarkerAttributes) -> bool {
        Self::marker_wants_attrs(true, attrs)
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
    pub(super) fn set_visible_settings(
        pathing: &mut PathingSettings,
        control: TaimiControls,
        set: Option<bool>,
    ) -> bool {
        let (is_visible, out) = match control {
            TaimiControls::PATHING_MAP => (
                pathing.space.visible_worldmap(),
                &mut pathing.space.visible_map_world,
            ),
            TaimiControls::PATHING_MINIMAP => (
                pathing.space.visible_minimap(),
                &mut pathing.space.visible_map_mini,
            ),
            TaimiControls::PATHING_SPACE | _ =>
                (pathing.space.visible_space(), &mut pathing.space.visible_space),
        };
        let set = set.unwrap_or(!is_visible);
        *out = Some(set);

        set
    }

    pub async fn debug_req_config_vis(
        &mut self,
        pack_path: Option<PackPath>,
        partial: bool,
        publish: Option<bool>,
    ) {
        let mut dirty = false;
        let maps = self
            .maps
            .iter_mut_with_info(&self.map_info, None)
            .filter(|(path, ..)| pack_path.map(|p| path.root == p).unwrap_or(true));
        for (map_path, map, map_info) in maps {
            let pack_path = map_path.root;
            let mut pack_dirty = true;
            {
                let info = self
                    .packs
                    .lookup_info(pack_path)
                    .and_then(|i| self.loader.pack_config(pack_path).map(|c| (i, c)));
                let Some(((info, _info), config)) = info else {
                    log::error!("missing info+config for {map_path}");
                    continue
                };
                let config = config.borrow();
                let damage = match map.update_category_config(&map_info, &info.categories, &config.config) {
                    Ok(true) => {
                        pack_dirty = false;
                        if partial {
                            log::info!("vis update for {map_path} undamaged");
                            continue
                        } else {
                            None
                        }
                    },
                    _ if !partial => None,
                    damage => damage.err(),
                };
                log::info!("vis updating for {map_path}...");
                map.refresh_categories(&map_info, &info.categories, &config.config, damage.as_ref());
            }
            pack_dirty |=
                Self::update_loaded_visibility_inner(map_path, map, map_info, Some(&self.filter_state));
            log::info!("vis updated; pack_dirty={pack_dirty}");
            dirty |= pack_dirty;
        }
        let publish = match (dirty, publish) {
            (false, None) | (_, Some(false)) => false,
            (true, _) | (_, Some(true)) => true,
        };
        log::info!("vis updated; dirty={dirty}, publish={publish}");
        if publish {
            let maps = self
                .maps
                .iter_with_info(&self.map_info, None)
                .filter(|(path, ..)| pack_path.map(|p| path.root == p).unwrap_or(true));
            self.loader.update_map_states(true, true, &mut { maps });
        }
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
