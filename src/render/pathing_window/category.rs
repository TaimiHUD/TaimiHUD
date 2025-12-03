use {
    super::{PathingFilterState, PathingWindowState},
    crate::{
        controller::pathing::{
            registry::{CategoryIndex, CategoryPath, CategorySet, MarkerIndex, MarkerIndexVariant, MarkerPath, PackConfig, PackInfo, PackMapPath, PackPath, PackRoot, UnloadedReason},
            shared::SharedLoaderPackData,
            visible::VisibilityFlags,
            PathingController, PathingEvent,
            SharedPacks,
        }, exports::runtime::{imgui::{
            ChildWindow, Condition, IdStackToken, MouseButton, Selectable, TableFlags, TreeNode, TreeNodeFlags, TreeNodeToken, Ui, WindowFlags
        }, locator::LocationRef, Locator}, fl, render::{machine::RenderMachine, RenderState}, space::engine::Engine, with_i18n, Controller, ControllerEvent
    }, std::{collections::{btree_map, BTreeMap}, iter, sync::{Arc, Weak}},
    taimi_sync::arcs::weak_is_null,
    taimi_pack::{attributes::AttrString, MarkerAttributes},
    tokio::sync::watch,
};

impl PathingWindowState {
    pub fn draw_pack_list(
        &mut self,
        ui: &Ui,
        _machine: &mut RenderMachine,
        _engine: Option<&mut anyhow::Result<Engine>>,
    ) {
        Self::draw_folder_button(ui);
        let (has_packs, has_packs_loaded) = if let Some(loader) = &self.pack_loader {
            let has_packs = loader.packs.info.borrow().is_empty();
            let has_loaded = loader.packs.data.borrow().iter().any(|p| !weak_is_null(p));
            (has_packs, has_loaded)
        } else { (false, false) };
        if has_packs_loaded {
            ui.same_line();
            let button_text = match self.filter_open {
                true => fl!("hide-filter"),
                false => fl!("show-filter"),
            };
            if ui.button(button_text) {
                self.filter_open = !self.filter_open;
            }

            if self.open_items.iter().any(|(_, open)| open.not_all()) {
                ui.same_line();
                if ui.button(&fl!("expand-all")) {
                    if let Some(info) = self.pack_loader.as_ref().map(|l| l.packs.info.borrow()) {
                        for (path, info) in SharedPacks::packs(info.iter()) {
                            let Ok(info) = &info.info else { continue };
                            let open = self.open_items.entry(path).or_default();
                            open.clear();
                            if let Some(search_candidates) = self.search_state.candidate_mask.get(&path) {
                                open.extend_from_bitslice(search_candidates);
                            } else if let Some(current_map) = Self::current_map_filter_of(&self.current_map, path) {
                                open.extend_from_bitslice(current_map);
                            } else {
                                open.resize(info.categories.count(), true);
                            }
                        }
                    }
                }
            }
            if self.open_items.iter().any(|(_, open)| open.any()) {
                ui.same_line();
                if ui.button(&fl!("collapse-all")) {
                    self.open_items.clear();
                }
            }
        }
        ui.same_line();
        if with_i18n!("refresh", |msg| ui.button(msg)) {
            PathingEvent::LoadAll.try_send();
        }
        ui.same_line();
        if with_i18n!("reload-packs", |msg| ui.button(msg)) {
            PathingEvent::ReloadAll(false).try_send();
        }
        {
            ui.same_line();
            if ui.button("later") {
                PathingEvent::UnloadAll(false).try_send();
            }
        }
        ui.same_line();
        if with_i18n!("unload-packs", |msg| ui.button(msg)) {
            PathingEvent::UnloadAll(true).try_send();
        }
        {
            ui.same_line();
            if ui.button("lowmem") {
                self.open = false;
                PathingEvent::LowMemory.try_send();
                ControllerEvent::WindowState(
                    crate::WINDOW_PATHING.into(),
                    Some(self.open),
                ).try_send();
                self.reduce_memory();
                return
            }
        }
        if has_packs_loaded && self.filter_open {
            ui.separator();
            let search_dirty = self.draw_filters(ui);
            ui.dummy([4.0; 2]);
            ui.separator();
            ui.dummy([4.0; 2]);

            if search_dirty {
                self.refresh_search();
            }
        }
        if !has_packs {
            Self::draw_empty_notice(ui);
            return
        }
        ChildWindow::new("pathing_subwindow")
            .flags(WindowFlags::ALWAYS_VERTICAL_SCROLLBAR)
            .size([0.0; 2])
            .build(ui, || {
                let table_flags =
                    TableFlags::RESIZABLE | TableFlags::ROW_BG | TableFlags::BORDERS | TableFlags::NO_PAD_OUTER_X;
                let table_name = format!("pathing");
                let table_token = ui.begin_table_with_flags(
                    &table_name,
                    1,
                    table_flags,
                );
                ui.table_next_column();
                let mut any_loaded = false;
                let packs: Vec<_> = {
                    let Some(pack_loader) = &self.pack_loader else {
                        return
                    };
                    let pack_info = pack_loader.packs.info.borrow();
                    SharedPacks::packs(pack_info.iter())
                        .map(|(path, info)| {
                            let loaded = pack_loader.packs.data.borrow().get(path.path as usize)
                                .map(|data| !weak_is_null(data)).unwrap_or(false);
                            (path, info.info.clone(), loaded)
                        })
                        .collect()
                };
                for &(path, ref pack, _loaded) in &packs {
                    match pack {
                        Ok(info) => {
                            self.refresh_state_of(path, &info);
                            any_loaded = true;
                        },
                        Err(unloaded) => {
                            if self.draw_unloaded_pack(ui, path, unloaded.to_string(), Some(unloaded)) {
                                PathingEvent::LoadPack(path).try_send();
                            }
                        },
                    }
                }
                #[cfg(deleteme)] {
                    self.refresh_current_state();
                }

                for (path, info, is_loaded) in packs {
                    let info = match (info, is_loaded) {
                        (Ok(info), true) => info,
                        (Ok(info), false) => {
                            if self.draw_unloaded_pack(ui, path, info.to_string(), None) {
                                PathingEvent::LoadPack(path).try_send();
                            }
                            continue
                        },
                        (Err(..), ..) => continue,
                    };
                    self.draw_pack(ui, path, &info);
                }
                if let Some(token) = table_token {
                    token.end();
                }
                if !any_loaded {
                    Self::draw_empty_notice(ui);
                }
            });
    }

    pub fn category_visible(
        &mut self,
        path: CategoryPath<PackPath>,
        info: &PackInfo,
    ) -> bool {
        if !self.filter_state.contains(PathingFilterState::ShowHidden) && info.categories.hidden.contains_index(path.path) {
            return false
        }
        let Some(cat) = info.categories.info_of(CategoryPath::with_path(path.path)) else {
            log::warn!("unknown category {path}");
            return true
        };

        let category_idx = path.path as usize;
        let enabled_filter = {
            let mut substate = None;
            let mut substate = || *substate.get_or_insert_with(|| self.item_config_toggle(path, info));
            let disabled = self.filter_state.contains(PathingFilterState::Disabled) || substate();
            let enabled = self.filter_state.contains(PathingFilterState::Enabled) || !substate();
            disabled | enabled
        };
        // since these are recursive, isn't this nonsensical?
        #[cfg(todo)]
        let is_root_filter = self.filter_state.contains(PathingFilterState::IgnoreRoot) && cat.parent().is_none();
        let is_leaf = cat.child().is_none();
        let is_branch = !is_leaf;
        let is_leaf_filter = self.filter_state.contains(PathingFilterState::IgnoreLeaves) && is_leaf;
        let is_branch_filter =
            is_branch && self.filter_state.contains(PathingFilterState::IgnoreBranches);
        let search_filter = match self.search_state.candidate_mask.get(&path.root) {
            Some(mask) if !mask.is_empty() =>
                mask.get(category_idx).map(|b| *b).unwrap_or(false),
            _ => true,
        };
        let map_filter = self.filter_state.contains(PathingFilterState::CurrentMap)
            .then(|| self.current_map_filter(path.root)).flatten();
        let map_filter = map_filter
            .map(|f| f.get(category_idx).map(|b| *b).unwrap_or(false))
            .unwrap_or(true);
        let exclusive = is_leaf_filter | is_branch_filter;
        let filter =
            enabled_filter & !exclusive;
        search_filter && map_filter && filter
    }

    fn get_category_display_name(packs: Option<&SharedLoaderPackData>, _info: &PackInfo, path: CategoryPath<PackPath>) -> Option<Option<Arc<str>>> {
        SharedPacks::pack_active(packs?, path.root)
            .map(|active| active.pack.categories.all_categories.get_index(path.path as usize)
                .map(|(_id, cat)| cat.display_name.clone())
            )
    }
    pub(super) fn marker_display_name<'a>(packs: &Option<watch::Receiver<SharedLoaderPackData>>, names: &'a mut BTreeMap<MarkerPath<PackPath>, Option<Arc<str>>>, info: &PackInfo, path: MarkerPath<PackPath>) -> Option<&'a str> {
        let entry = match names.entry(path) {
            btree_map::Entry::Occupied(e) => return e.into_mut().as_ref().map(|s| &s[..]),
            btree_map::Entry::Vacant(e) => e,
        };
        let packs = packs.as_ref().map(|d| d.borrow());
        let packs = packs.as_ref().map(|d| &**d);
        match path.path.variant() {
            MarkerIndexVariant::Category(idx) =>
                Self::get_category_display_name(packs, info, path.swap(idx)),
            MarkerIndexVariant::Poi(idx) => {
                let idx = idx as usize;
                let tip_name = match Self::get_marker_tip(packs, info, path) {
                    Some((tip, _desc)) if !tip.is_empty() =>
                        Some(Some(Arc::from(&tip[..]))),
                    _ => None,
                };
                let get_display_name = || {
                    let active = SharedPacks::pack_active(packs?, path.root)?;
                    Some(active.pack.pois.get(idx).and_then(|poi|
                        // TODO: idk what this is but it could be useful maybe kinda?
                        poi.attributes.render.as_ref()
                        .and_then(|render| render.poi.as_ref())
                        .and_then(|poi| poi.billboard_text.as_ref())
                        .map(|text| Arc::from(&text[..]))
                        .or_else(||
                            active.pack.categories.all_categories.get(poi.category.as_id()).map(|cat| cat.display_name.clone())
                        )
                        //.or_else(|| poi.attributes.icon_file)
                    ))
                };
                tip_name.or_else(get_display_name)
            },
            _ => {
                log::debug!("unimplemented");
                return None
            },
        }.map(|name| entry.insert(name).as_ref())
            .unwrap_or(None)
            .map(|s| &s[..])
    }
    fn category_display_name<'a>(packs: &Option<watch::Receiver<SharedLoaderPackData>>, category_names: &'a mut BTreeMap<MarkerPath<PackPath>, Option<Arc<str>>>, info: &PackInfo, path: CategoryPath<PackPath>) -> Option<&'a str> {
        Self::marker_display_name(packs, category_names, info, path.map_path(MarkerIndex::with_category))
    }

    pub(super) fn get_marker_tip(packs: Option<&SharedLoaderPackData>, _info: &PackInfo, path: MarkerPath<PackPath>) -> Option<(AttrString, AttrString)> {
        let Some(active) = SharedPacks::pack_active(packs?, path.root) else { return None };
        match path.path.variant() {
            MarkerIndexVariant::Category(path) => {
                let Some((_, cat)) = active.pack.categories.all_categories.get_index(path as usize) else { return None };
                let get_display_name = || &cat.display_name[..];
                Self::get_marker_tip_inner(&cat.marker_attributes, &get_display_name)
            },
            MarkerIndexVariant::Poi(path) => {
                let Some(poi) = active.pack.pois.get(path as usize) else { return None };
                let get_display_name = || active.pack.categories.all_categories.get(poi.category.as_id()).map(|cat| &cat.display_name[..]).unwrap_or("");
                Self::get_marker_tip_inner(&poi.attributes, &get_display_name)
            },
            _ => {
                log::debug!("unimplemented");
                None
            },
        }
    }
    fn get_marker_tip_inner<'a>(attrs: &'a MarkerAttributes, display_name: &dyn Fn() -> &'a str) -> Option<(AttrString, AttrString)> {
        let tip_description = match attrs.tip_description.as_ref() {
            Some(desc) if desc.is_empty() => None,
            d => d,
        };
        let tip_name = match attrs.tip_name.as_ref() {
            Some(title) if title.is_empty() => None,
            Some(title) if display_name().starts_with(&title[..]) => None,
            t => t,
        };

        match (tip_name, tip_description) {
            (None, None) => None,
            (title, desc) => Some((
                title.cloned().unwrap_or_default(),
                desc.cloned().unwrap_or_default(),
            )),
        }
    }

    pub(super) fn get_marker_copy(packs: Option<&SharedLoaderPackData>, _info: &PackInfo, path: MarkerPath<PackPath>) -> Option<(AttrString, AttrString)> {
        let Some(active) = SharedPacks::pack_active(packs?, path.root) else { return None };
        match path.path.variant() {
            MarkerIndexVariant::Category(path) => {
                let Some((_, cat)) = active.pack.categories.all_categories.get_index(path as usize) else { return None };
                Self::get_marker_copy_inner(&cat.marker_attributes)
            },
            MarkerIndexVariant::Poi(path) => {
                let Some(poi) = active.pack.pois.get(path as usize) else { return None };
                Self::get_marker_copy_inner(&poi.attributes)
            },
            _ => {
                log::debug!("unimplemented");
                None
            },
        }
    }
    fn get_marker_copy_inner(attrs: &MarkerAttributes) -> Option<(AttrString, AttrString)> {
        let interaction = attrs.interaction.as_ref()?;
        let copy_value = interaction.copy_value.clone()?;
        let copy_message = match interaction.copy_message.as_ref() {
            Some(m) if m.is_empty() => None,
            m => m,
        };
        Some((copy_value, copy_message.cloned().unwrap_or_default()))
    }

    pub(super) fn get_marker_info(packs: Option<&SharedLoaderPackData>, _info: &PackInfo, path: MarkerPath<PackPath>) -> Option<Option<AttrString>> {
        let Some(active) = SharedPacks::pack_active(packs?, path.root) else { return None };
        match path.path.variant() {
            #[cfg(todo = "unnecessary")]
            MarkerIndexVariant::Category(path) => (),
            MarkerIndexVariant::Poi(path) => {
                let Some(poi) = active.pack.pois.get(path as usize) else { return None };
                Some(Self::get_marker_info_inner(&poi.attributes))
            },
            _ => {
                log::debug!("unimplemented");
                None
            },
        }
    }
    fn get_marker_info_inner(attrs: &MarkerAttributes) -> Option<AttrString> {
        let info = attrs.interaction.as_ref()
            .and_then(|i| i.info.as_ref());
        match info {
            Some(info) if info.is_empty() => None,
            info => info.cloned(),
        }
    }
    pub(super) fn marker_info<'a>(packs: &Option<watch::Receiver<SharedLoaderPackData>>, cache: &'a mut BTreeMap<MarkerPath<PackPath>, Option<AttrString>>, info: &PackInfo, path: MarkerPath<PackPath>) -> Option<&'a str> {
        let entry = match cache.entry(path) {
            btree_map::Entry::Occupied(e) => return e.into_mut().as_ref().map(|s| &s[..]),
            btree_map::Entry::Vacant(e) => e,
        };
        let packs = packs.as_ref().map(|d| d.borrow());
        let packs = packs.as_ref().map(|d| &**d);
        let info = Self::get_marker_info(packs, info, path).flatten();
        entry.insert(info).as_ref()
            .map(|s| &s[..])
    }

    pub fn draw_unloaded_pack(&mut self, ui: &Ui, path: PackPath, name: String, reason: Option<&UnloadedReason>) -> bool {
        let is_button = match reason {
            Some(UnloadedReason::Gravestone) =>
                return false,
            | Some(UnloadedReason::Loading | UnloadedReason::Pending)
            | None
            =>
                false,
            Some(..) => true,
        };
        let _id = ui.push_id(&name);
        ui.popup("pack-context-unloaded", || {
            self.menu_pack_unloaded(ui, path);
        });
        let node = TreeNode::new(name)
            .flags(TreeNodeFlags::SPAN_AVAIL_WIDTH)
            .frame_padding(true)
            .tree_push_on_open(false)
            .opened(false, Condition::Always)
            .leaf(is_button)
            .push(ui);
        let hovered = ui.is_item_hovered();
        // TODO: hovered?
        let pressed = is_button && ui.is_item_clicked() /*&& ui.is_mouse_released(MouseButton::Left)*/;
        let open_context = ui.is_item_clicked_with_button(MouseButton::Right);

        match reason {
            Some(UnloadedReason::Disabled | UnloadedReason::Gravestone) => {
                ui.same_line();
                with_i18n!("disabled", |msg| ui.text(msg));
            },
            Some(UnloadedReason::Loading) => {
                ui.same_line();
                with_i18n!("loading", |msg| ui.text(msg));
            },
            Some(UnloadedReason::Pending) | None => {
                ui.same_line();
                with_i18n!("unloaded", |msg| ui.text(msg));
                if reason.is_some() && hovered {
                    with_i18n!("render-notice-gameplay", |msg| ui.tooltip_text(msg));
                }
            },
            Some(reason @ (UnloadedReason::LoadingFailed(..) | UnloadedReason::UnknownFormat)) => {
                ui.same_line();
                match reason {
                    UnloadedReason::UnknownFormat =>
                        with_i18n!("unknown", |msg| ui.text(msg)),
                    _ =>
                        with_i18n!("error", |msg| ui.text(msg)),
                }
                if hovered {
                    ui.tooltip_text(reason.to_string());
                }
            },
        }
        ui.table_next_column();
        let res = if let Some(node) = node {
            node.pop();
            !is_button || pressed
        } else {
            pressed
        };
        if open_context {
            ui.open_popup("pack-context-unloaded");
        }
        res
    }

    pub fn draw_pack(&mut self, ui: &Ui, path: PackPath, info: &PackInfo) {
        let _pack_id = ui.push_id(path.path as i32);
        self.act_selected_category_open = false;

        let map_path = Controller::with_sender(|s|
            s.gameplay.as_ref().and_then(|g| g.borrow().gameplay_map())
        ).flatten().map(|map_id| path.rel(map_id));
        if self.filter_state.contains(PathingFilterState::CurrentMap) {
            #[cfg(deleteme)]
            if let Some(map_path) = map_path {
                self.refresh_current_map(map_path);
            }
        }

        let open_context;
        let open;
        let (root_path, (_id, tree)) = {
            let primary_root = info.primary_root();
            let root_path = match primary_root {
                Some(root) if root.path().path != CategoryIndex::MAX =>
                    Some(root.path().pivot(path)),
                _ => None,
            };
            open = self.is_open(path, root_path.map(|root| CategoryPath::with_path(root.path)));
            let fallback_name;
            let display_name = match primary_root {
                Some(root) => &root.display_name[..],
                None => {
                    fallback_name = info.to_string();
                    &fallback_name[..]
                },
            };
            let id = self.category_header_prestart(ui, root_path, &display_name);
            let token = self.category_header_start(ui, root_path, &display_name, open, Some(false), false, None);
            open_context = ui.is_item_clicked_with_button(MouseButton::Right);
            if let Some(root_path) = root_path {
                self.category_header_decorate(ui, info, root_path);
            }
            (root_path, (id, token))
        };
        if let Some(root_path) = &root_path {
            let now_open = tree.is_some();
            if now_open != open {
                let open = self.open_items.entry(path).or_default();
                Self::set_bit(open, None, root_path.path as usize, now_open);
            }
        }
        let state = root_path.map(|root_path| self.item_config_toggle(root_path, info))
            .unwrap_or_else(|| self.pack_config_toggle(path, info));
        ui.same_line(); ui.dummy([4.0, f32::EPSILON]); ui.same_line();
        if let Some(toggled) = Self::category_toggle(ui, state) {
            if let Some(root_path) = root_path {
                self.commit_state(root_path, toggled);
            }
        }
        Self::category_name_finish(ui);

        if tree.is_some() {
            let roots = info.roots.iter()
                .map(|root| (root.path().pivot(path), root));
            let root_count = roots.clone()
                .filter(|&(path, _root)| !self.category_visible(path, info))
                .count();
            if root_count > 1 {
                for (root_path, root) in roots {
                    if !self.category_visible(root_path, info) {
                        continue
                    }
                    self.draw_root_category(ui, info, (root_path, map_path), root);
                }
            } else {
                ui.indent();
                for (root_path, _root) in roots {
                    self.draw_children(ui, info, (root_path, map_path));
                }
                ui.unindent();
            }
        }
        Self::category_finish(ui, tree);
        _id.end();

        ui.popup("cat-context", || {
            if let Some(cat) = self.act_selected_category {
                self.menu_category_item(ui, info, cat);
            }
        });
        ui.popup("pack-context", || {
            self.menu_pack(ui, path);
            if let Some(root_path) = root_path {
                ui.separator();
                if let _font = RenderState::push_font("ui", ui) {
                    with_i18n!("category", |header| ui.text(&header));
                }
                ui.separator();
                self.menu_category(ui, info, root_path, Some(state), open);
            }
        });
        if self.act_selected_category_open {
            ui.open_popup("cat-context");
        } else if open_context {
            self.act_selected_pack_active = None;
            ui.open_popup("pack-context");
        }
    }

    pub fn is_open(&self, path: PackPath, category: Option<CategoryPath>) -> bool {
        self.open_items.get(&path)
            .map(|open| category.map(
                |cat| open.get(cat.path as usize).map(|b| *b)
                    .unwrap_or(false)
            ).unwrap_or(open.any()))
            .unwrap_or(false)
    }

    /// whether category is turned on or off.
    /// (ignoring parents, so the item may still be effectively disabled even if it's "on")
    pub fn item_config_toggle(&self, path: CategoryPath<PackPath>, info: &PackInfo) -> bool {
        let default = !info.categories.disabled.contains(path);
        let Some(config) = self.pack_configs.get(&path.root) else {
            return default
        };
        let Some(config) = &config.cached else { return default };
        let dev = config.visibility_deviation_for(path.unscope());
        default ^ dev.contains(VisibilityFlags::TOGGLE)
    }
    pub fn pack_config_toggle(&self, path: PackPath, info: &PackInfo) -> bool {
        if info.categories.roots.is_empty() {
            return false
        }
        info.categories.roots.iter().any(|&c| self.item_config_toggle(path.rel(c), info))
    }

    pub fn item_state(&self, path: CategoryPath<PackPath>) -> VisibilityFlags {
        self.current_state.get(&path.root)
            .map(|state| state.get(path.path as usize).map(|b| *b)
                .unwrap_or(state.any())
            ).unwrap_or(false).into()
    }

    #[cfg(todo = "unnecessary")]
    pub fn set_state(&mut self, path: CategoryPath<PackPath>, state: VisibilityFlags) {
        if let Some(current_state) = self.current_state.get_mut(&path.root) {
            Self::set_bit(current_state, None, path.path as usize, state.is_visible());
        }
    }
    pub fn commit_state(&mut self, path: CategoryPath<PackPath>, state: bool) {
        if !self.pack_configs.contains_key(&path.root) {
            log::info!("TODO: pack config not yet loaded?");
        }
        //self.set_state(path, state);
        if path.path != CategoryIndex::MAX {
            PathingEvent::CategorySetToggle(
                path,
                Some(state),
            ).try_send();
        }
    }

    pub fn draw_root_category(&mut self, ui: &Ui, info: &PackInfo, (root_path, map_path): (CategoryPath<PackPath>, Option<PackMapPath>), root: &PackRoot) {
        self.draw_category_item(ui, info, (root_path, map_path), &root.display_name)
    }

    fn draw_category(&mut self, ui: &Ui, info: &PackInfo, (cat_path, map_path): (CategoryPath<PackPath>, Option<PackMapPath>), display_name: &str) {
        if !self.category_visible(cat_path, info) {
            return
        }
        self.draw_category_item(ui, info, (cat_path, map_path), &display_name)
    }

    fn draw_category_item(&mut self, ui: &Ui, info: &PackInfo, (cat_path, map_path): (CategoryPath<PackPath>, Option<PackMapPath>), display_name: &str) {
        let open = self.is_open(cat_path.root, Some(Locator::with_path(cat_path.path)));
        let unscoped = Locator::with_path(cat_path.path);
        let cat_info = info.categories.info_of(unscoped);
        let cat_lonely = info.categories.lonely.contains(unscoped);
        let is_leaf = cat_info.as_ref().map(|cat| cat.child().is_none());
        let is_leaf = match is_leaf {
            Some(true) if cat_lonely => None,
            Some(l) => Some(l),
            None => {
                log::error!("invalid category {cat_path}???");
                Some(true)
            },
        };
        let is_decorative = info.categories.separators.contains(cat_path);
        let is_copyable = info.categories.copyable.contains(cat_path);
        let state = match (is_decorative, cat_lonely) {
            (false, false) => Some(self.item_config_toggle(cat_path, info)),
            _ => None,
        };
        let _id = self.category_header_prestart(ui, Some(cat_path), &display_name);
        if let Some(state) = state {
            ui.unindent();
            if let Some(toggled) = Self::category_toggle(ui, state) {
                self.commit_state(cat_path, toggled);
            }
            ui.same_line();
        }
        let tree = self.category_header_start(ui, Some(cat_path), &display_name, open, is_leaf, is_decorative, Some(is_copyable));
        if !self.act_selected_category_open && ui.is_item_clicked_with_button(MouseButton::Right) {
            self.act_selected_category = Some((cat_path, state, open, is_copyable));
            self.act_selected_category_open = true;
        }
        self.category_header_decorate(ui, info, cat_path);

        let now_open = tree.is_some();
        if !is_leaf.unwrap_or(false) && open != now_open {
            let open = self.open_items.entry(cat_path.root).or_default();
            Self::set_bit(open, None, cat_path.path as usize, now_open);
        }

        Self::category_name_finish(ui);

        if state.is_some() {
            ui.indent();
        }

        if !is_leaf.unwrap_or(true) && tree.is_some() {
            ui.indent();
            self.draw_children(ui, info, (cat_path, map_path));
            ui.unindent();
        }
        Self::category_finish(ui, tree);
    }

    pub fn draw_children(&mut self, ui: &Ui, info: &PackInfo, (cat_path, map_path): (CategoryPath<PackPath>, Option<PackMapPath>)) {
        for child in info.categories.children_of(Locator::with_path(cat_path.path)) {
            let child_path = child.pivot(cat_path.root);
            let display_name = {
                Self::category_display_name(&self.pack_loader_data, &mut self.category_names, info, child_path)
                .map(|name| name.to_owned())
            }.unwrap_or_else(|| format!("#{}", child.path));
            self.draw_category(ui, info, (child_path, map_path), &display_name);
        }
    }

    pub fn category_header_prestart<'u>(
        &mut self,
        ui: &'u Ui,
        path: Option<CategoryPath<PackPath>>,
        display_name: &str,
    ) -> IdStackToken<'u> {
        let push_token = match path {
            Some(path) => ui.push_id(path.path as i32 ^ ((path.root.path as i32) << 20)),
            _ => ui.push_id(display_name),
        };

        push_token
    }
    pub fn category_header_start<'u>(
        &mut self,
        ui: &'u Ui,
        path: Option<CategoryPath<PackPath>>,
        display_name: &str,
        open: bool,
        is_leaf: Option<bool>,
        is_decorative: bool,
        button_interact: Option<bool>,
    ) -> Option<TreeNodeToken<'u>> {
        let mut unbuilt = TreeNode::new(display_name);
        match button_interact {
            Some(false) if is_decorative || is_leaf.unwrap_or(true) =>
                unbuilt = unbuilt.flags(TreeNodeFlags::SPAN_AVAIL_WIDTH),
            None =>
                unbuilt = unbuilt.allow_item_overlap(true),
            _ => (),
        }
        unbuilt = unbuilt.frame_padding(true)
            .tree_push_on_open(false)
            .leaf(is_leaf.unwrap_or(true));
        let mut framed = false;
        match is_leaf {
            Some(false) => if !is_decorative {
                framed = true;
            },
            Some(true) =>
                unbuilt = unbuilt.bullet(true),
            None => (),
        }
        if is_decorative {
            match is_leaf {
                Some(true) =>
                    unbuilt = unbuilt.selected(true),
                Some(false) => {
                    framed = true;
                    unbuilt = unbuilt.bullet(true);
                },
                None => {
                    // needs to stand out more among branches too..?
                    // TODO: less necessary once checkboxes become left-aligned
                    unbuilt = unbuilt.selected(true);
                    // would use this but leaf|framed results in strange text alignment...
                    // framed = true
                },
            }
        }
        if framed {
            unbuilt = unbuilt
                .framed(true)
                .opened(open, if path.is_some() { Condition::Always } else { Condition::Once });
        }
        let tree_token = unbuilt.push(ui);
        tree_token
    }

    pub(super) const NAME_TEMPLATE: &'static str = "Generic Copyable Marker Name";
    /// Draw buttons and tooltips and stuff on top
    pub fn category_header_decorate<'u>(
        &mut self,
        ui: &'u Ui,
        info: &PackInfo,
        path: CategoryPath<PackPath>,
    ) {
        let mut display_name = None;
        let is_copyable = info.categories.copyable.contains(path);
        let hovered = ui.is_item_hovered();
        let mut show_tip = hovered;
        let marker_path = path.map_path(MarkerIndex::with_category);
        let pack_data = self.pack_loader_data.as_ref();
        let pack_data = || pack_data.map(|d| d.borrow());
        if is_copyable {
            ui.same_line();
            if ui.small_button(&fl!("copy")) {
                if let Some((copy_value, copy_message)) = self.category_copy.entry(marker_path)
                    .or_insert_with(|| Self::get_marker_copy(pack_data().as_ref().map(|d| &**d), info, marker_path))
                {
                    Self::copy_copyable(ui, &copy_value[..], &copy_message[..]);
                }
            } else if ui.is_item_hovered() {
                let display_name = display_name.get_or_insert(
                    Self::category_display_name(&self.pack_loader_data, &mut self.category_names, info, path)
                        .map(ToOwned::to_owned)
                ).as_ref().and_then(|s| match s {
                    s if !s.is_empty() => Some(&s[..]),
                    _ => None,
                });
                let tip = self.category_tips.entry(marker_path)
                    .or_insert_with(|| Self::get_marker_tip(pack_data().as_ref().map(|d| &**d), info, marker_path));
                Self::draw_tooltip(ui, display_name.unwrap_or(Self::NAME_TEMPLATE), || {
                    if let Some((title, desc)) = tip {
                        Self::draw_tooltip_category(ui, display_name.unwrap_or_default(), &title[..], &desc[..]);
                    }
                    if let Some((copy_value, copy_message)) = self.category_copy.entry(marker_path)
                        .or_insert_with(|| Self::get_marker_copy(pack_data().as_ref().map(|d| &**d), info, marker_path))
                    {
                        Self::draw_tooltip_copyable(
                            ui,
                            display_name.unwrap_or_default(),
                            &copy_value[..],
                            &copy_message[..],
                        );
                    }
                });
                show_tip = false;
            }
        }
        if let Some(Some((title, desc))) = show_tip.then(||
            self.category_tips.entry(marker_path)
                .or_insert_with(|| Self::get_marker_tip(pack_data().as_ref().map(|d| &**d), info, marker_path))
        ) {
            let display_name = display_name.get_or_insert(
                Self::category_display_name(&self.pack_loader_data, &mut self.category_names, info, path)
                    .map(ToOwned::to_owned)
            ).as_ref().and_then(|s| match s {
                s if !s.is_empty() => Some(&s[..]),
                _ => None,
            });
            Self::draw_tooltip(ui, display_name.unwrap_or(Self::NAME_TEMPLATE), || {
                Self::draw_tooltip_category(ui, display_name.unwrap_or_default(), &title[..], &desc[..]);
            });
        }
    }

    #[cfg(todo)]
    fn set_open_item(&mut self, info: &PackInfo, path: CategoryPath<PackPath>) {
        let open = self.open_items.entry(path.root).or_default();
        let mut next = Some(CategoryPath::with_path(path.path));
        loop {
            let Some(cat) = next else { break };
            Self::set_bit(open, None, cat.path as usize, true);
            next = info.categories.parent_of(cat);
        }
    }

    pub fn category_name_finish<'u>(
        ui: &'u Ui,
    ) {
        ui.table_next_column();
    }
    pub fn category_toggle<'u>(
        ui: &'u Ui,
        mut state: bool,
    ) -> Option<bool> {
        let mut toggled = None;
        if ui.checkbox("", &mut state) {
            toggled = Some(state);
        }
        toggled
    }
    pub fn category_finish<'u>(
        _ui: &'u Ui,
        tree: Option<TreeNodeToken<'u>>,
    ) {
        drop(tree);
    }

    pub(super) fn draw_tooltip_category(ui: &Ui, display_name: &str, title: &str, desc: &str) {
        let desc = match desc {
            desc if !desc.is_empty() => Some(&desc[..]),
            _ => None,
        };
        let title = match title {
            title if !title.is_empty() && !display_name.starts_with(title) =>
                Some(&title[..]),
            _ => None,
        };

        if let Some(title) = title {
            let _title_font = desc.map(|_| RenderState::push_font("big", ui));
            ui.text(title);
        }

        if let Some(tip) = desc {
            ui.text_wrapped(tip);
        }
    }

    pub(super) fn draw_title_text_truncate(ui: &Ui, text: &str) {
        let header = text.split_once(['\n', '.'])
            .map(|(header, _rest)| header)
            .unwrap_or(text);
        let header = match header.len() {
            0 => text,
            _ => header,
        };
        #[cfg(todo)]
        let sz = ui.calc_text_size(header);
        let _wrap = ui.push_text_wrap_pos_with_pos(-1.0);
        ui.text_wrapped(header);
    }

    pub fn menu_pack_unloaded(&mut self, ui: &Ui, path: PackPath) {
        let action_remove = with_i18n!("remove", |label| Selectable::new(&label).build(ui));
        let action_reload = with_i18n!("reload-pack", |label| Selectable::new(&label).build(ui));
        if action_reload {
            PathingEvent::ReloadPack(path, true).try_send();
        } else if action_remove {
            PathingEvent::UnloadPack(path, true).try_send();
        }
    }
    pub fn menu_pack(&mut self, ui: &Ui, path: PackPath) {
        //with_i18n!("pack", |header| ui.text(&header));
        let action_later = Selectable::new("later")
            .build(ui);
        let is_loaded = match self.act_selected_pack_active {
            Some(active) => active,
            ref mut active @ None => match PathingController::packs().try_read() {
                Ok(packs) => *active.insert(packs.lookup_ref(&path)
                    .and_then(|pack| pack.active.as_ref())
                    .is_some()
                ),
                Err(..) => false,
            },
        };
        let action_unload = match is_loaded {
            true => with_i18n!("unload-pack", |msg| Selectable::new(msg).build(ui)),
            false => false,
        };
        let action_reload = with_i18n!("reload-pack", |msg| Selectable::new(msg)
            .build(ui)
        );
        if action_unload || action_later {
            PathingEvent::UnloadPack(path, action_unload).try_send();
        } else if action_reload {
            PathingEvent::ReloadPack(path, false).try_send();
        }
    }
    pub fn menu_category(&mut self, ui: &Ui, info: &PackInfo, path: CategoryPath<PackPath>, state: Option<bool>, open: bool) {
        if let Some(_state) = state {
            let action_toggle = with_i18n!("toggle", |msg| Selectable::new(msg).build(ui));
            let action_enable_all = with_i18n!("enable-all", |msg| Selectable::new(msg).build(ui));
            let action_disable_all = with_i18n!("disable-all", |msg| Selectable::new(msg).build(ui));
            let action_reset_all = with_i18n!("reset-all", |msg| Selectable::new(msg).build(ui));
            ui.separator();
            let action_isolate = with_i18n!("isolate", |msg| Selectable::new(msg).build(ui));
            let action_unisolate = with_i18n!("unisolate", |msg| Selectable::new(msg).build(ui));
            ui.separator();
            let action_enable_to = with_i18n!("enable-to", |msg| Selectable::new(msg).build(ui));
            let action_disable_to = with_i18n!("disable-to", |msg| Selectable::new(msg).build(ui));
            let action_all = if action_enable_all {
                Some(Some(true))
            } else if action_disable_all {
                Some(Some(false))
            } else if action_reset_all {
                Some(None)
            } else {
                None
            };
            let action_parents = if action_enable_to {
                Some(true)
            } else if action_disable_to {
                Some(false)
            } else {
                None
            };
            let action_isolate = if action_isolate {
                Some(Some(None))
            } else if action_unisolate {
                Some(None)
            } else {
                None
            };

            if action_toggle {
                #[cfg(todo = "deleteme")]
                PathingEvent::CategorySetToggle(path, None).try_send();
                self.act_categories_select(&info, path.root, iter::once(path.unscope()), None);
            } else if let Some(action_all) = action_all {
                let categories = &info.categories;
                let cat_path = path.unscope();
                let paths = categories.descendents_of(cat_path)
                    .chain(iter::once(cat_path));
                match action_all {
                    Some(enable) =>
                        self.act_categories_select(&info, path.root, paths, Some(enable)),
                    None =>
                        self.act_categories_reset(&info, path.root, paths),
                }
            } else if let Some(parents_enable) = action_parents {
                let categories = &info.categories;
                let cat_path = path.unscope();
                let paths = categories.parents_of(cat_path)
                    .chain(iter::once(cat_path));
                self.act_categories_select(&info, path.root, paths, Some(parents_enable));
            } else if let Some(isolate) = action_isolate {
                let categories = &info.categories;
                let cat_path = path.unscope();
                let cat_info = categories.info_of(cat_path)
                    .map(|cat| {
                        let oldest = cat.parent()
                            .map(CategoryPath::with_path)
                            .and_then(|parent| categories.firstborn_of(parent));
                        (cat, oldest)
                    });
                let oldest = cat_info.and_then(|(cat, oldest)|
                    oldest.or(cat.sibling()
                        .map(CategoryPath::with_path)
                    )
                );
                let paths = oldest.into_iter()
                    .flat_map(|oldest| categories.younger_siblings_of(oldest)
                        .chain(iter::once(oldest))
                        .filter(|&s| s != cat_path)
                    );
                match isolate {
                    Some(state) =>
                        self.act_categories_isolate(&info, path.root, paths, state.ok_or(cat_path)),
                    None =>
                        self.act_categories_reset(&info, path.root, paths),
                }
            }
            ui.separator();
        }

        // TODO
        let any_collapsed = true;
        let action_expand_all = if any_collapsed {
            with_i18n!("expand-all", |msg| Selectable::new(msg).build(ui))
        } else { false };
        let action_collapse_all = if open {
            with_i18n!("collapse-all", |msg| Selectable::new(msg).build(ui))
        } else { false };
        // TODO
        let hidden = false;
        let action_hide = with_i18n(if hidden { "unhide" } else { "hide"}, |msg| Selectable::new(msg).build(ui));

        if action_hide {
            log::debug!("TODO: cat:{}", if hidden { "unhide" } else { "hide" });
        } else if action_expand_all {
            log::debug!("TODO: cat:expand");
        } else if action_collapse_all {
            log::debug!("TODO: cat:collapse");
        }

        ui.separator();
        // TODO: category stats
    }
    pub fn menu_category_item(&mut self, ui: &Ui, info: &PackInfo, (path, state, open, copyable): (CategoryPath<PackPath>, Option<bool>, bool, bool)) {
        self.menu_category(ui, info, path, state, open);
    }

    /// enable=None to toggle paths
    pub fn act_categories_select<I>(&self, info: &PackInfo, pack_path: PackPath, paths: I, enable: Option<bool>) where
        I: IntoIterator<Item = CategoryPath>,
    {
        let paths = paths.into_iter().map(|path|
            (path, enable)
        );
        self.act_categories_set(info, pack_path, paths)
    }

    pub fn act_categories_reset<I>(&self, info: &PackInfo, pack_path: PackPath, paths: I) where
        I: IntoIterator<Item = CategoryPath>,
    {
        let paths = paths.into_iter().map(|path|
            (path, Some(!info.categories.disabled.contains(path)))
        );
        self.act_categories_set(info, pack_path, paths)
    }

    /// enable=Err(target) to toggle all paths to the *opposite* state of target
    pub fn act_categories_isolate<I>(&self, info: &PackInfo, pack_path: PackPath, paths: I, enable: Result<bool, CategoryPath>) where
        I: IntoIterator<Item = CategoryPath>,
    {
        let enable = match enable {
            Ok(enable) => enable,
            Err(target) => !self.item_config_toggle(target.pivot(pack_path), info),
        };
        self.act_categories_select(info, pack_path, paths, Some(enable))
    }

    pub fn act_categories_set<I>(&self, info: &PackInfo, pack_path: PackPath, paths: I) where
        I: IntoIterator<Item = (CategoryPath, Option<bool>)>,
    {
        let config = self.pack_configs.get(&pack_path);
        let Some(config) = config else {
            log::warn!("cannot toggle categories for unloaded pack {info}");
            return
        };
        #[cfg(todo = "unnecessary")]
        let Some(config) = &config.cached else { return default };
        let mut dirty_cats = CategorySet::default();
        config.watch.sender().send_if_modified(|config| {
            let mut config_mut = None::<&mut PackConfig>;
            let mut config = Some(config);
            for (path, target_state) in paths {
                let default = !info.categories.disabled.contains(path);
                let mut dev = match (&mut config, &mut config_mut) {
                    (Some(config), None) => &***config,
                    (None, Some(config)) => &*config,
                    #[cfg(todo)]
                    _ => unsafe { unreachable_unchecked() },
                    _ => continue,
                }.visibility_deviation_for(path);
                let dev_prev_state = dev.is_visible();
                let dev_target_state = match target_state {
                    Some(target) => target ^ default,
                    None => !dev_prev_state,
                };
                if dev_prev_state == dev_target_state {
                    continue
                } else {
                    dev.toggle(VisibilityFlags::TOGGLE);
                }
                let config = match (config.take(), &mut config_mut) {
                    (Some(config), out @ None) =>
                        out.insert(Arc::make_mut(config)),
                    (None, Some(ref mut config)) => config,
                    #[cfg(todo)]
                    _ => unsafe { unreachable_unchecked() },
                    _ => continue,
                };
                #[cfg(todo = "unnecessary")]
                dev.set(VisibilityFlags::TOGGLE, dev_target_state);
                config.set_visibility_deviation(path, dev);
                dirty_cats.insert(path);
            }
            // return !dirty_cats.is_empty()
            config_mut.is_some()
        });
        if !dirty_cats.is_empty() {
            PathingEvent::CategoryCommitVisibility(pack_path, dirty_cats).try_send();
        }
    }
}
