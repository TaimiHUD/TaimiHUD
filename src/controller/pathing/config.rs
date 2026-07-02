use {
    crate::{
        controller::pathing::{
            info::MapPackInfo,
            registry::{
                LoadedCategoryIndex,
                LoadedCategoryNs,
                LoadedCategoryPath,
                LoadedMarkerPath,
                PackCategoryInfo,
                PackLoader,
                PackMapPath,
                PackPath,
            },
            shared::{LocDisplay, SharedPackConfig},
            state::{
                filter::{self, FilterState},
                LoadedCategory,
                LoadedMapPack,
            },
            PathingController,
            PathingEvent,
            VisibilityFlagsExt,
        },
        exports::runtime as rt,
        settings::{pathing::PathingSave, state::SaveState, Settings},
        space::{engine::SpaceEvent, Engine},
    },
    anyhow::Context,
    std::{
        borrow::BorrowMut,
        collections::{BTreeMap, HashSet, VecDeque},
        future::Future,
        iter,
        sync::Arc,
    },
    taimi_hoard::loc::{indexed::IndexedList, LocationGet, LocationMut, LocationRef, Locator},
    taimi_meta::{
        packs::{
            collections::CategorySet,
            id::{MarkerId, MarkerPath},
            CategoryIndex,
            CategoryPath,
            MapIndex,
            PackCategoryNs,
            VisibilityFlags,
        },
        ui::MapContext,
    },
    taimi_pack::{
        attributes::{FilterAttributes, MarkerAttributes},
        category::id::{CategoryId, IdNameBox},
        Pack,
    },
    taimi_sync::{arcs::ArcLazyMut, watched::watch},
};
#[cfg(feature = "paths-filter")]
use taimi_pack::attributes::{keys, cell::GetAttrDynExt};

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
    /// TODO: inherit overrides too?
    pub fn fill_settings(
        &mut self,
        pack: &Pack,
        save: &PathingSave,
        legacy_disabled_paths: &HashSet<String>,
    ) {
        for id in legacy_disabled_paths {
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
        if disabled_compat {
            let disabled_cats = pack
                .categories
                .all_categories
                .iter()
                .enumerate()
                .filter(|(_, (_, cat))| !cat.default_toggle());
            for (i, (full_id, _disabled_cat)) in disabled_cats {
                let path = CategoryPath::with_path(i as CategoryIndex);
                if !legacy_disabled_paths.contains(&full_id.id_to_str()[..]) {
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
        for root in &pack.categories.root_categories {
            for (id, dev) in save.categories.visibility_deviations_for(root) {
                #[cfg(todo = "unnecessary")]
                if vis.is_empty() {
                    continue
                }
                let cat_path = pack
                    .categories
                    .all_categories
                    .get_index_of(id)
                    .map(|idx| CategoryPath::with_path(idx as CategoryIndex));
                let Some(cat_path) = cat_path else { continue };
                self.set_visibility_deviation(cat_path, dev);
            }
        }
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
            self.loader
                .update_map_states(true, true, &mut { maps }, Some(&self.filter_state));
        }
    }
    #[cfg(todo)]
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
    ) -> Option<(CategoryPath, VisibilityFlags, bool)> {
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

        let dev = VisibilityFlags::visible(state);
        let out = cat_vis ^ state;
        changed.then_some((path.unscope(), dev, out))
    }
    async fn handle_toggle_post(
        &mut self,
        pack_path: PackPath,
        cat_vis: (CategoryPath, VisibilityFlags, bool),
    ) {
        self.category_commit_vis_post(pack_path, iter::once(cat_vis))
            .await
    }
    async fn category_commit_vis_post<C>(&mut self, pack_path: PackPath, dirty_cats: C)
    where
        C: IntoIterator<Item = (CategoryPath, VisibilityFlags, bool)> + Send + 'static,
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
    pub(super) fn category_commit_vis<'a>(
        &'a mut self,
        pack_path: PackPath,
        dirty_cats: &'_ mut dyn Iterator<Item = CategoryPath>,
    ) -> impl Future<Output = ()> + 'a {
        let commit = self.category_commit_vis_of(pack_path, dirty_cats);
        if commit.is_none() {
            log::error!("cannot save category settings for unloaded {pack_path}");
        }
        async move {
            if let Some(commit) = commit {
                commit.await
            }
        }
    }
    pub(super) fn category_commit_vis_of<'a>(
        &'a mut self,
        pack_path: PackPath,
        dirty_cats: &'_ mut dyn Iterator<Item = CategoryPath>,
    ) -> Option<impl Future<Output = ()> + 'a> {
        let pack_info = self.loader.pack_info(pack_path);
        let (categories, ..) = pack_info
            .as_ref()
            .and_then(|pack_info| pack_info.category_info())?;
        let config = self.loader.pack_config(pack_path)?;
        let changes = {
            // TODO: avoid collect but also avoid borrowing or copying config :<
            let config = config.borrow();
            dirty_cats
                .map(|path| {
                    let default = !categories.disabled.contains(path);
                    let vis = config.config.visibility_deviation_for(path);
                    let state = vis.is_visible() ^ default;
                    (path, vis, state)
                })
                .collect::<Vec<_>>()
        };
        Some(self.category_commit_vis_post(pack_path, changes))
    }
    async fn category_commit_vis_task<C>(
        loader: Arc<PackLoader>,
        pack_path: PackPath,
        dirty_cats: C,
    ) -> anyhow::Result<PathingEvent>
    where
        C: IntoIterator<Item = (CategoryPath, VisibilityFlags, bool)> + Send,
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
        C: IntoIterator<Item = (CategoryPath, VisibilityFlags, bool)> + Send,
    {
        let mut settings = loader.settings.write().await;
        Self::category_commit_vis_write(&mut settings, pack, &mut dirty_cats.into_iter())
    }
    /// two locks scary..?
    fn category_commit_vis_write(
        settings: &mut Settings,
        pack: &Pack,
        dirty_cats: &mut dyn Iterator<Item = (CategoryPath, VisibilityFlags, bool)>,
    ) {
        SaveState::try_write_with(|save| {
            let mut save_dirty = false;
            for (path, vis_dev, vis_state) in dirty_cats {
                let Some(full_id) = Self::get_category_id_in(pack, path) else { continue };
                settings.pathing_state_update(full_id.to_string(), vis_state);
                save.pathing_mut()
                    .categories
                    .set_visibility_deviation(full_id, vis_dev);
                save_dirty = true;
            }
            save_dirty
        });
    }
    fn get_category_id_in(pack: &Pack, cat_path: CategoryPath) -> Option<&CategoryId> {
        let full_id = pack
            .categories
            .all_categories
            .get_index(cat_path.path as usize)
            .map(|(_id, cat)| &cat.full_id);
        if full_id.is_none() {
            log::warn!("{cat_path} not found for toggle state update");
        }
        full_id
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
        for (_marker_path, _lpath, category_index, visibility, filters, _guid) in pois.chain(trails) {
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
            #[cfg(feature = "paths-filter")]
            let guid_filter_state = match filter_state {
                _ if !visibility.is_visible() => None,
                Some(filter_state) if filter_state.hidden.is_empty() => None,
                f => f,
            };
            #[cfg(feature = "paths-filter")]
            if let Some(filter_state) = guid_filter_state {
                let inverted: bool = filters.as_ref().and_then(|f| f.clone_attr_of::<keys::InvertBehaviour>()).unwrap_or_default().into();
                // TODO: use GroupConfig properly here and move most of this into a method!
                let marker_path: MarkerPath<PackPath> =
                    MarkerPath::with_parts(_lpath.root.root, _marker_path.path);
                let marker_id = MarkerId::for_marker(marker_path);
                let lmarker_id = MarkerId::for_marker(_lpath);
                let guid_id = _guid
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
    pub(super) fn can_filter(filters: &FilterAttributes) -> bool {
        filter::FilterConfig::filters_is_empty(filters)
    }
    /// interested in retaining full attrs in memory
    fn marker_wants_attrs(_ispoi: bool, attrs: &MarkerAttributes) -> bool {
        let can_filter = attrs
            .filters
            .as_ref()
            .map(|f| !filter::FilterConfig::filters_is_empty(f))
            .unwrap_or(false);
        if can_filter {
            return true
        }

        #[cfg(feature = "paths-interact")]
        {
            let can_interact = _ispoi
                && attrs
                    .interaction
                    .as_ref()
                    .map(|i| Self::can_interact(i))
                    .unwrap_or(false);
            #[cfg(feature = "paths-lua")]
            #[cfg(todo = "unnecessary")]
            let can_interact = can_interact || attrs.script.as_ref().map(|s| s.script_focus.is_some() | s.script_trigger.is_some()).unwrap_or(false);
            if can_interact {
                return true
            }
        }
        #[cfg(feature = "paths-lua")]
        if attrs.script.as_ref().is_some() { return true }

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
    pub(super) async fn set_visible_settings(
        &mut self,
        context: Option<MapContext>,
        set: Option<bool>,
    ) -> bool {
        let mut settings = self.loader.settings.write().await;
        let pathing = settings.pathing_mut().into_mut();
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

    pub(super) fn update_filter_state(&mut self) -> bool {
        let mut dirty = false;
        if let Ok(ml) = rt::mumble_link_ptr() {
            dirty |= self.filter_state.map.update_from_mumblelink_context(&ml);
            dirty |= self.filter_state.avatar.update_from_mumblelink_context(&ml);
        }
        dirty
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
            self.loader
                .update_map_states(true, true, &mut { maps }, Some(&self.filter_state));
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
        LoadedCategory::update_category_config(&mut self.categories, info, categories, config)
    }

    pub fn refresh_categories(
        &mut self,
        info: &MapPackInfo,
        categories: &PackCategoryInfo,
        config: &PackConfig,
        damage: Option<&CategorySet>,
    ) {
        let mut loaded = IndexedList::from_mut(Arc::make_mut(&mut self.categories));
        LoadedCategory::refresh_categories(&mut loaded, info, categories, config, damage)
    }
}
impl LoadedCategory {
    pub(super) fn update_category_config(
        loaded: &mut Arc<[Self]>,
        info: &MapPackInfo,
        categories: &PackCategoryInfo,
        config: &PackConfig,
    ) -> Result<bool, CategorySet> {
        let mut damage = match loaded.len() {
            amt if info.category_count() != amt => {
                *loaded = iter::repeat(LoadedCategory::INVALID)
                    .take(info.category_count())
                    .collect();
                None
            },
            _ => Some(CategorySet::default()),
        };
        let mut loaded = ArcLazyMut::new(loaded);
        let _changed = Self::populate_vis(
            &mut loaded,
            damage.as_mut(),
            &mut info.loaded_categories(),
            categories,
            config,
        );
        match damage {
            #[cfg(todo = "unnecessary")]
            _ if !_changed => Ok(true),
            Some(damage) => match damage.is_empty() {
                true => Ok(true),
                false => Err(damage),
            },
            None => Ok(false),
        }
    }
    /// TODO: LocationMut instead?
    #[inline]
    pub(crate) fn populate_vis<L, I>(
        loaded: &mut L,
        damage: Option<&mut CategorySet>,
        category_paths: I,
        categories: &PackCategoryInfo,
        config: &PackConfig,
    ) -> bool
    where
        I: IntoIterator<Item = (LoadedCategoryPath, CategoryPath)>,
        L: BorrowMut<[Self]>,
    {
        let category_paths = category_paths.into_iter();
        Self::populate_vis_dyn(loaded, damage, &mut { category_paths }, categories, config)
    }
    pub(crate) fn populate_vis_dyn(
        loaded: &mut dyn BorrowMut<[Self]>,
        mut damage: Option<&mut CategorySet>,
        category_paths: &mut dyn Iterator<Item = (LoadedCategoryPath, CategoryPath)>,
        categories: &PackCategoryInfo,
        config: &PackConfig,
    ) -> bool {
        let mut changed = false;
        for (lpath, path) in category_paths {
            let i = lpath.path as usize;
            let Some(prev) = loaded.borrow().get(i) else { continue };
            let prev = prev.visibility;
            let (default_toggles, is_override) = Self::default_toggles_for(path, categories, config);
            let defaults = default_toggles.toggles_to_default();
            let prev_defaults = prev & VisibilityFlags::DEFAULTS;
            let is_override_clean = !is_override || prev & VisibilityFlags::TOGGLES == default_toggles;
            let is_clean = damage.is_some() && defaults == prev_defaults && is_override_clean;
            if is_clean {
                continue
            }

            let out = unsafe { loaded.borrow_mut().get_unchecked_mut(i) };
            out.visibility.set_defaults(defaults);

            if let Some(damage) = &mut damage {
                damage.insert(path);
            }
            changed = true;
        }

        if let Some(damage) = damage {
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
        }
        changed
    }

    pub fn default_toggles_for(
        path: CategoryPath,
        categories: &PackCategoryInfo,
        config: &PackConfig,
    ) -> (VisibilityFlags, bool) {
        let defaults = categories
            .visibility
            .get_for(path)
            .unwrap_or(VisibilityFlags::TOGGLES);
        let deviation = config.visibility_deviation_for(path);
        let default_toggles = defaults ^ deviation;
        let is_override = config.visibility_overrides.contains(path);
        (default_toggles, is_override)
    }

    /// TODO: LookupMut=VisibilityFlags
    pub fn refresh_categories<L, I>(
        loaded: &mut L,
        info: &I,
        categories: &PackCategoryInfo,
        config: &PackConfig,
        damage: Option<&CategorySet>,
    ) where
        L: LocationMut<LoadedCategoryNs, LoadedCategoryIndex, LookupRef = LoadedCategory>,
        I: LocationGet<PackCategoryNs, CategoryIndex, LookupGet = LoadedCategoryPath>,
    {
        Self::refresh_categories_dyn(loaded, info, categories, config, damage)
    }
    pub(crate) fn refresh_categories_dyn<N, P>(
        loaded: &mut dyn LocationMut<N, P, LookupRef = LoadedCategory>,
        info: &dyn LocationGet<PackCategoryNs, CategoryIndex, LookupGet = Locator<N, P>>,
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
        while let Some((path, parent_is_override, parent_vis)) = children.pop_front() {
            let Some(index) = info.lookup_get(&path) else {
                // rest of tree should be irrelevant
                continue
            };
            let is_override = config.visibility_overrides.contains(path);
            let default_vis = loaded
                .lookup_ref(&index)
                .map(|cat| cat.visibility.default_toggles());
            let visibility = match is_override {
                true => default_vis,
                false => {
                    let inherited = parent_vis.or_else(|| {
                        categories
                            .parent_of(path)
                            .map(|parent| {
                                info.lookup_get(&parent)
                                    .and_then(|i| loaded.lookup_ref(&i))
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
            if let Some(loaded) = loaded.lookup_mut(&index) {
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
