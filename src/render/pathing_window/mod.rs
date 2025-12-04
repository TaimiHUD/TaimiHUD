use crate::controller::pathing::registry::UnloadedReason;

use {
    crate::{
        controller::pathing::{
            registry::{PackConfig, PackInfo, MarkerPath},
            visible::VisibilityFlags,
            shared::{PathingShared, SharedGameplayMap, SharedMaps, SharedPacks, SharedLoaderPackConfig, SharedLoaderPackData, SharedLoaderPackInfo},
        }, exports::runtime::{self as rt, Watched, imgui::{
            sys as imgui_sys, Condition, StyleVar, Ui, Window,
        }}, fl, render::{machine::RenderMachine, PathingConfig, RenderState}, settings::Settings, space::engine::Engine, with_i18n, Controller, ControllerEvent
    },
    bitvec::{slice::BitSlice, vec::BitVec},
    std::{collections::BTreeMap, sync::{Arc, Weak}},
    taimi_pack::attributes::AttrString,
    taimi_meta::loc::packs::{CategoryIndex, CategoryPath, MapIndex, PackIndex, PackMapPath, PackPath},
    tokio::sync::watch,
};
pub use self::filter::{PathingFilterState, PathingSearchState};

mod filter;
mod interact;
mod category;

pub struct PathingWindowState {
    pub open: bool,
    pub filter_open: bool,
    pub filter_state: PathingFilterState,
    pub open_items: BTreeMap<PackPath, BitVec>,
    pub current_state: BTreeMap<PackPath, BitVec>,
    pub effective_state: BTreeMap<PackPath, BitVec>,
    pub current_map: BTreeMap<PackMapPath, BitVec>,
    pub category_names: BTreeMap<MarkerPath<PackPath>, Option<Arc<str>>>,
    pub category_tips: BTreeMap<MarkerPath<PackPath>, Option<(AttrString, AttrString)>>,
    pub category_copy: BTreeMap<MarkerPath<PackPath>, Option<(AttrString, AttrString)>>,
    pub cache_info: BTreeMap<MarkerPath<PackPath>, Option<AttrString>>,
    pub pack_configs: BTreeMap<PackPath, Watched<Arc<PackConfig>>>,
    pub search_state: PathingSearchState,
    pub pack_gameplay: Option<watch::Receiver<SharedGameplayMap>>,
    pub pack_maps: Option<watch::Receiver<SharedMaps>>,
    pub pack_loader: Option<Arc<PathingShared>>,
    pub pack_loader_info: Option<watch::Receiver<SharedLoaderPackInfo>>,
    pub pack_loader_data: Option<watch::Receiver<SharedLoaderPackData>>,
    pub pack_loader_config: Option<watch::Receiver<SharedLoaderPackConfig>>,
    // draw stack state...
    act_selected_pack_active: Option<bool>,
    act_selected_category: Option<(CategoryPath<PackPath>, Option<bool>, bool, bool)>,
    act_selected_category_open: bool,
    act_selected_poi: Option<self::interact::RenderInteractivePoi>,
    act_selected_poi_open: bool,
    act_selected_poi_delay: Option<f32>,
}

impl PathingWindowState {
    pub fn new() -> Self {
        Self {
            open: false,
            filter_open: false,
            filter_state: Default::default(),
            open_items: Default::default(),
            search_state: Default::default(),
            pack_maps: Default::default(),
            pack_gameplay: Default::default(),
            pack_configs: Default::default(),
            current_state: Default::default(),
            effective_state: Default::default(),
            current_map: Default::default(),
            category_names: Default::default(),
            category_tips: Default::default(),
            category_copy: Default::default(),
            cache_info: Default::default(),
            pack_loader: Default::default(),
            pack_loader_info: Default::default(),
            pack_loader_data: Default::default(),
            pack_loader_config: Default::default(),
            act_selected_pack_active: Default::default(),
            act_selected_category: Default::default(),
            act_selected_category_open: Default::default(),
            act_selected_poi: Default::default(),
            act_selected_poi_open: Default::default(),
            act_selected_poi_delay: None,
        }
    }

    pub fn draw(
        &mut self,
        ui: &Ui,
        machine: &mut RenderMachine,
        engine: Option<&mut anyhow::Result<Engine>>,
    ) {
        if let Some(settings) = Settings::try_read() {
            self.open = settings.pathing_window_open;
        };
        if !self.open {
            return
        }
        self.init_watcher();
        let mut packs_info_changed = false;
        let mut packs_config_changed = false;
        let mut packs_map_changed = false;
        let mut packs_maps_changed = false;
        let mut packs_data_changed = false;
        if let Some(pack_loader) = &self.pack_loader {
            let loader_info = self.pack_loader_info.get_or_insert_with(|| {
                let mut rx = pack_loader.packs.info.subscribe();
                rx.mark_changed();
                rx
            });
            if let Some(loader_info) = loader_info.has_changed().unwrap_or(false).then(|| loader_info.borrow_and_update()) {
                packs_info_changed = true;
            };

            let loader_data = self.pack_loader_data.get_or_insert_with(|| {
                let mut rx = pack_loader.packs.data.subscribe();
                rx.mark_changed();
                rx
            });
            if let Some(loader_data) = loader_data.has_changed().unwrap_or(false).then(|| loader_data.borrow_and_update()) {
                // todo
                packs_data_changed = true;
            };

            let loader_config = self.pack_loader_config.get_or_insert_with(|| {
                let mut rx = pack_loader.packs.config.subscribe();
                rx.mark_changed();
                rx
            });
            if let Some(loader_config) = loader_config.has_changed().unwrap_or(false).then(|| loader_config.borrow_and_update()) {
                packs_config_changed = true;
                self.pack_configs.clear();
                self.pack_configs.extend(loader_config.iter().enumerate()
                    .filter_map(|(i, c)| {
                        let Some(mut c) = c.as_ref().map(Watched::subscribe_to) else { return None };
                        let _ = c.watch.try_mark_changed();
                        Some((PackPath::with_path(i as PackIndex), c))
                    })
                );
            };

            let pack_maps = self.pack_maps.get_or_insert_with(|| {
                let mut rx = pack_loader.maps.subscribe();
                rx.mark_changed();
                rx
            });
            if let Some(pack_maps) = pack_maps.has_changed().unwrap_or(false).then(|| pack_maps.borrow_and_update()) {
                packs_maps_changed = true;
            };

            let gameplay_map = self.pack_gameplay.get_or_insert_with(|| {
                let mut rx = pack_loader.gameplay.subscribe();
                rx.mark_changed();
                rx
            });
            if let Some(gameplay_map) = gameplay_map.has_changed().unwrap_or(false).then(|| gameplay_map.borrow_and_update()) {
                packs_map_changed = true;
            };
        }
        for (_path, pack_config) in &mut self.pack_configs {
            let _ = pack_config.try_read_mut();
        }
        if packs_data_changed {
            self.refresh_packs_data();
        }
        if packs_info_changed {
            self.refresh_packs();
        }
        let loaded_packs = if packs_info_changed || packs_config_changed || packs_maps_changed {
            self.pack_loader_info.as_ref().map(|info|
                SharedPacks::packs(info.borrow().iter().map(|pack| pack.info.clone()))
                .collect::<Vec<_>>()
            )
        } else { None }.unwrap_or_default();
        if packs_info_changed || packs_config_changed {
            for &(path, ref pack) in &loaded_packs {
                // 1. resets current_state to category defaults (fineish, but needs clobbering immediately if map is also active!)
                // 2. ensures open_items exists for the path key but does not populate
                match pack {
                    Ok(pack) =>
                        self.refresh_state_of(path, &pack),
                    Err(reason) =>
                        self.clear_state_of(path, reason)
                }
            }
        }
        if packs_maps_changed {
            self.prune_maps();
            let map_id = self.pack_gameplay.as_ref()
                .and_then(|state| state.borrow().map_id);
            for &(path, ref pack) in &loaded_packs {
                if let Ok(info) = pack {
                    // populates bitvec of categories used by the current map for the filter
                    self.refresh_current_map((path, map_id), info);
                }
            }
            #[cfg(todo)]
            if map_id.is_none() {
                // idk probably unnecessary and annoying to clear this during loading screens...
                self.clear_current_map();
            }
        }
        if packs_map_changed {
            // clobbers current_state with vis(default) from map state
            // seems fine but also just read config directly tbh???
            // a secondary bitvec tracking effective vis would be useful but is a different thing!
            self.refresh_current_map_state();
        }
        let open_prev = self.open;
        let mut open = open_prev;
        Window::new(fl!("pathing-window"))
            .size([300.0, 200.0], Condition::FirstUseEver)
            .opened(&mut open)
            .build(ui, || self.draw_window(ui, machine, engine));

        if open != open_prev {
            ControllerEvent::WindowState(
                crate::WINDOW_PATHING.into(),
                Some(open),
            ).try_send();
            self.open = open;
        }

        if !self.open {
            self.reduce_memory();
        }
    }

    pub fn draw_window(
        &mut self,
        ui: &Ui,
        machine: &mut RenderMachine,
        engine: Option<&mut anyhow::Result<Engine>>,
    ) {
        let rendered_err = if let Some(Ok(..)) = engine {
            None
        } else {
            Some(engine.as_ref().map(|e| e.as_ref().err()))
        };
        if let Some(e) = rendered_err {
            Self::draw_folder_button(ui);
            PathingConfig::draw_space_error(ui, machine, e.flatten());
        }
        self.draw_content(ui, machine, engine);
    }
    pub fn draw_content(
        &mut self,
        ui: &Ui,
        machine: &mut RenderMachine,
        mut engine: Option<&mut anyhow::Result<Engine>>,
    ) {
        let Some(_tabs) = ui.tab_bar("pathing") else { return };
        if let Some(_token) = with_i18n!("enable", |msg| ui.tab_item(msg)) {
            self.draw_pack_list(ui, machine, engine.take());
        }
        if let Some(_token) = with_i18n!("poi", |msg| ui.tab_item(msg)) {
            self.draw_pois(ui, machine, engine.take());
        }
    }

    fn draw_empty_notice(ui: &Ui) {
        if let _font = RenderState::push_font("big", ui) {
            with_i18n!("packs-empty", |msg| ui.text(msg));
        }
        if let _font = RenderState::push_font("ui", ui) {
            with_i18n!("packs-empty-notice", |notice| ui.text_wrapped(notice));
        }
    }

    pub fn draw_folder_button(ui: &Ui) {
        let pathing_dir = crate::ADDON_DIR.join("pathing");
        RenderState::draw_open_path_button(
            ui,
            fl!("open-button", kind = "folder"),
            &pathing_dir,
        );
    }

    fn init_watcher(&mut self) {
        if self.pack_loader.is_none() {
            self.pack_loader = Controller::with_sender(|s| s.pathing.as_ref().map(|info| {
                info.shared.clone()
            })).map(|l| l.unwrap_or_else(|| {
                log::info!("pathing controller didn't show up for work today");
                Arc::new(PathingShared::new())
            }));
        }
        #[cfg(todo)]
        let was_shutdown = self.pack_loader.is_none();
        let was_shutdown = false;
        if self.pack_loader.is_none() {
            self.reduce_memory();
            // TODO: free all other handles, controller shutdown probably
        }
    }

    pub fn reduce_memory(&mut self) {
        self.current_map.clear();
        self.current_state.clear();
        self.effective_state.clear();
        // TODO: retain any names with open parents that are presumably visible?
        self.category_names.clear();
        self.category_tips.clear();
        self.category_copy.clear();
        self.cache_info.clear();
    }
    /// TODO: lack of map info can just mean it's not loaded yet,
    /// so don't clear anything meaningful here
    pub fn prune_maps(&mut self) {
        let map_info = self.pack_maps.as_ref()
            .map(|info| info.borrow());
        let Some(map_info) = map_info else { return };

        map_info.prune_map(&mut self.current_map);
        map_info.prune_map_of(&mut self.current_state);
        map_info.prune_map_of(&mut self.category_tips);
        map_info.prune_map_of(&mut self.category_copy);
        map_info.prune_map_of(&mut self.cache_info);
    }

    fn refresh_packs(&mut self) {
        self.category_names.clear();
        self.category_tips.clear();
        self.category_copy.clear();
        self.cache_info.clear();
        for current_map in self.current_map.values_mut() {
            current_map.clear();
        }
        for current_state in self.current_state.values_mut() {
            current_state.clear();
        }
        let Some(pack_loader) = &self.pack_loader else { return };
    }
    fn refresh_packs_data(&mut self) {
        self.category_names.retain(|_, v| v.is_some());
        self.category_tips.retain(|_, v| v.is_some());
        self.category_copy.retain(|_, v| v.is_some());
        self.cache_info.retain(|_, v| v.is_some());
    }

    fn refresh_search(&mut self) {
        let packs: Box<[_]> = {
            let Some(packs) = self.pack_loader_data.as_ref().map(|data| data.borrow()) else {
                self.search_state.clear_matches();
                return
            };
            SharedPacks::packs(packs.iter())
                .filter_map(|(path, data)| Weak::upgrade(data).map(|data|
                    (path, data)
                )).collect()
        };
        let packs = packs.iter().map(|(p, d)| (*p, &**d));
        self.search_state.commit(packs);
    }

    pub fn refresh_state_of(&mut self, path: PackPath, info: &PackInfo) {
        let state = self.current_state.entry(path).or_default();
        state.clear();
        state.resize(info.categories.count(), true);
        for cat_path in info.categories.disabled() {
            Self::try_set_bit(state, cat_path.path as usize, false);
        }
        let config = self.pack_configs.get(&path)
            .and_then(|config| config.cached.as_ref());
        if let Some(config) = config {
            for (cat_path, vis) in config.category_visibility.iter() {
                let deviation = vis.contains(VisibilityFlags::TOGGLE);
                if !deviation { continue }
                if let Some(mut bit) = state.get_mut(cat_path.path as usize) {
                    *bit ^= deviation;
                }
            }
        }
        let _ = self.open_items.entry(path).or_default();
    }
    pub fn clear_state_of(&mut self, path: PackPath, _reason: &UnloadedReason) {
        self.current_state.remove(&path);
        self.effective_state.remove(&path);
        self.current_map.retain(|p, _| p.root != path);
        self.cache_info.retain(|p, _| p.root != path);
        self.category_copy.retain(|p, _| p.root != path);
    }

    pub fn refresh_current_map_state(&mut self) {
        let map = self.pack_gameplay.as_ref()
            .map(|map| map.borrow());
        let Some(map) = map else { return };

        for (path, map, info) in map.iter_state() {
            let Some(state) = self.current_state.get_mut(&path.root) else { continue };
            if state.is_empty() { continue }
            let effective_state = self.effective_state.entry(path.root)
                .or_insert_with(|| state.clone());
            if effective_state.len() != state.len() {
                effective_state.resize(state.len(), false);
            }
            effective_state[..].copy_from_bitslice(&state[..]);
            for (category_path, cat) in map.categories(info) {
                let index = category_path.path as usize;
                if index >= state.len() {
                    log::error!("category count ({}) mismatch! {category_path}", state.len());
                    continue
                }
                #[cfg(deleteme)]
                let vis_configured = cat.visibility.contains(VisibilityFlags::DEFAULT_TOGGLE);
                #[cfg(deleteme)]
                state.set(index, vis_configured);
                let vis_effective = cat.visibility.contains(VisibilityFlags::TOGGLE);
                effective_state.set(index, vis_effective);
            }
        }
    }

    pub fn refresh_current_map(&mut self, (path, map_id): (PackPath, Option<MapIndex>), info: &PackInfo) {
        let path = match map_id {
            Some(map_id) => path.rel(map_id),
            None => return,
        };

        let filter = self.current_map.entry(path).or_default();
        #[cfg(deleteme)]
        if !filter.is_empty() {
            return
        }

        filter.resize(info.categories.count(), false);

        let map_info = self.pack_maps.as_ref()
            .map(|info| info.borrow());
        let map_info = map_info.as_ref().and_then(|info|
            info.map_info.get(&path)
        );
        let Some(map_info) = map_info else { return };

        #[cfg(todo = "unnecessary")]
        {
            // map categories now contains all this info...
            // TODO: guard against cycles?
            let mut parents = Vec::new();
            for category in map_info.categories() {
                parents.extend(pack_info.categories.children_of(category));
                let mut parent = category;
                loop {
                    if filter.replace(parent.path as usize, true) {
                        break
                    }
                    let Some(next) = pack_info.categories.parent_of(parent) else { break };
                    parent = next;
                }
            }
            while let Some(parent) = parents.pop() {
                Self::try_set_bit(filter, parent.path as usize, true);
                parents.extend(pack_info.categories.children_of(parent));
            }
        }
        filter.fill(false);
        for category in map_info.categories() {
            Self::try_set_bit(filter, category.path as usize, true);
        }
    }

    fn current_map_filter(&self, path: PackPath) -> Option<&BitSlice> {
        Self::current_map_filter_of(&self.current_map, path)
    }
    fn current_map_filter_of(current_map: &BTreeMap<PackMapPath, BitVec>, path: PackPath) -> Option<&BitSlice> {
        let map_id = Controller::with_sender(|s| s.gameplay.as_ref().and_then(|g| g.borrow().gameplay_map()));
        match map_id {
            Some(Some(map_id)) => current_map.get(&path.rel(map_id))
                .map(|b| &b[..]),
            _ => None,
        }
    }

    fn set_bit(dest: &mut BitVec, init: Option<bool>, idx: usize, value: bool) {
        if idx >= CategoryIndex::MAX as usize {
            log::error!("invalid category bit");
            return
        }
        if idx >= dest.len() {
            dest.resize(idx + 1, init.unwrap_or(false));
        }
        dest.set(idx, value);
    }

    fn try_set_bit(dest: &mut BitVec, idx: usize, value: bool) {
        if idx < dest.len() {
            dest.set(idx, value);
        }
    }

    #[cfg(todo)]
    pub fn draw_category_header<'u>(
        &mut self,
        ui: &'u Ui,
        path: CategoryPath<PackPath>,
        category: &Category,
        state: &mut BitVec,
        category_filter: Option<&BitVec>,
        copyable: (&BTreeSet<CategoryIndex>, &BTreeSet<PoiIndex>, &[Poi]),
    ) -> (IdStackToken<'u>, Option<TreeNodeToken<'u>>) {
        let push_token = ui.push_id(path.path as i32 ^ (path.root.path as i32) << 20);
        let category_idx = path.path as usize;
        let mut tree_token = None;
        let is_copyable = match &category.marker_attributes.copy_value {
            Some(value) if category.sub_categories.is_empty() && !category.is_separator => Some(value),
            _ => None,
        };
        if let Some(..) = is_copyable {
            ui.indent();
            if ui.small_button(&fl!("copy-arg", arg = (&category.display_name[..]))) {
                Self::copy_copyable(ui, &category.marker_attributes);
            }
            if ui.is_item_hovered() {
                Self::draw_tooltip(ui, &category.display_name, || {
                    Self::draw_tooltip_category(ui, category);
                    Self::draw_tooltip_copyable(
                        ui,
                        &category.marker_attributes,
                        Some(&category.display_name),
                    );
                });
            }
            ui.unindent();
            ui.table_next_column();
            ui.table_next_column();
        } else {
            let (copyable_categories, copyable_pois, pois) = copyable;
            let has_copyable_pois = copyable_categories.contains(&path.path);

            let mut unbuilt = TreeNode::new(&category.display_name);
            if (category.is_separator || category.sub_categories.is_empty())
                && category.marker_attributes.copy_value.is_none()
                && !has_copyable_pois
            {
                unbuilt = unbuilt.flags(TreeNodeFlags::SPAN_AVAIL_WIDTH);
            }
            unbuilt = unbuilt.frame_padding(true).tree_push_on_open(false);
            let open = self.open_items.entry(path.root.clone()).or_default();
            let is_open = open.get(category_idx).map(|b| *b);
            if category.is_separator {
                unbuilt = unbuilt.leaf(true);
            } else if category.sub_categories.is_empty() {
                unbuilt = unbuilt.bullet(true);
            } else {
                unbuilt = unbuilt
                    .framed(true)
                    .opened(is_open.unwrap_or(false), Condition::Always);
            }
            tree_token = unbuilt.push(ui);
            if ui.is_item_hovered() && Self::category_has_tooltip(&category.display_name, category.marker_attributes.tip_name.as_ref(), category.marker_attributes.tip_description.as_ref()) {
                Self::draw_tooltip(ui, &category.display_name, || {
                    Self::draw_tooltip_category(ui, category);
                });
            }
            if category.marker_attributes.copy_value.is_some() {
                ui.same_line();
                if with_i18n!("copy", |copy| ui.small_button(copy)) {
                    Self::copy_copyable(ui, &category.marker_attributes);
                }
                if ui.is_item_hovered() {
                    Self::draw_tooltip(ui, &category.display_name, || {
                        Self::draw_tooltip_copyable(
                            ui,
                            &category.marker_attributes,
                            Some(&category.display_name),
                        );
                    });
                }
            }
            if has_copyable_pois {
                // TODO: revisit or remove once trigger radius and interaction is working
                let pois = copyable_pois
                    .iter()
                    .filter_map(|&poi_idx| pois.get(poi_idx as usize))
                    //.filter(|poi| poi.category_idx == idx);
                    .filter(|poi| poi.category == category.full_id);
                for (i, copyable) in pois.enumerate() {
                    if i % 4 != 3 {
                        ui.same_line();
                    }
                    let copied = match &copyable.attributes.tip_name {
                        Some(name) => ui.small_button(&fl!("copy-arg", arg = name)),
                        None => with_i18n!("copy", |copy| ui.small_button(copy)),
                    };
                    if copied {
                        Self::copy_copyable(ui, &copyable.attributes);
                    }
                    if ui.is_item_hovered() {
                        let template = copyable
                            .attributes
                            .tip_name
                            .as_ref()
                            .map(|n| &n[..])
                            .unwrap_or("Generic Copyable Marker Name");
                        Self::draw_tooltip(ui, template, || {
                            Self::draw_tooltip_poi(ui, &copyable.attributes);
                            Self::draw_tooltip_copyable(ui, &copyable.attributes, None);
                        });
                    }
                }
            }
            ui.table_next_column();
            if !category.is_separator {
                if let Some(mut substate) = state.get_mut(category_idx) {
                    if ui.checkbox("", &mut substate) {
                        PathingEvent::PathingStateUpdate(
                            path,
                            *substate,
                        ).try_send();
                    };
                }
            }
            ui.table_next_column();
            if is_open != Some(tree_token.is_some()) {
                if open.len() <= category_idx {
                    open.resize(category_idx + 1, false);
                }
                open.set(category_idx, false);
            }
        }
        (push_token, tree_token)
    }

    fn copy_copyable(ui: &Ui, copy_value: &str, copy_message: &str) {
        if !copy_value.is_empty() {
            ui.set_clipboard_text(copy_value);
        }
        if !copy_message.is_empty() {
            let _ = rt::send_alert(ui, copy_message);
        }
    }

    #[cfg(todo)]
    fn draw_tooltip_poi(ui: &Ui, attributes: &MarkerAttributes) {
        let desc = match &attributes.tip_description {
            Some(desc) if !desc.is_empty() => Some(&desc[..]),
            _ => None,
        };

        if let Some(title) = &attributes.tip_name {
            let _title_font = desc.map(|_| RenderState::push_font("big", ui));
            ui.text(&title[..]);
        }
        if let Some(desc) = &attributes.tip_description {
            ui.text_wrapped(&desc[..]);
        }
    }

    /// since these aren't intended to be displayed, there's no canon name to use...
    /// if it looks like more than just a location link, we'll try to preview it
    fn copyable_value_has_message(copy_value: &str) -> bool {
        if copy_value.is_empty() { return false };
        if !copy_value.starts_with('[') || !copy_value.ends_with(']') {
            return true
        }
        false
    }

    fn draw_tooltip<F: FnOnce()>(ui: &Ui, title_template: &str, f: F) {
        let _id = ui.push_id("category_tooltip");
        let [minwidth, lineheight] = ui.calc_text_size(title_template);
        unsafe {
            imgui_sys::igSetNextWindowSize([0.0, lineheight * 1.5].into(), Condition::Appearing as _);
        };
        let _size = ui.push_style_var(StyleVar::WindowMinSize([minwidth, lineheight]));
        ui.tooltip(|| {
            {
                let _padding = ui.push_style_var(StyleVar::ItemSpacing([f32::EPSILON, f32::EPSILON]));
                ui.dummy([minwidth, f32::EPSILON]);
            }
            f()
        })
    }

    fn draw_tooltip_copyable(ui: &Ui, display_name: &str, copy_value: &str, copy_message: &str) {
        if (display_name.is_empty() || copy_message.is_empty()) && Self::copyable_value_has_message(copy_value) {
            ui.text_wrapped(&format!("\"{copy_value}\""));
        }
        if !copy_message.is_empty() {
            ui.text_wrapped(copy_message);
        }
    }
}
