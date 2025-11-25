use {
    crate::{
        controller::pathing::{registry::{CategoryIndex, CategoryPath, MapIndex, PackConfig, PackIndex, PackInfo, PackLoader, PackMapPath, PackPath, SharedLoaderPackConfig, SharedLoaderPackData, SharedLoaderPackInfo}, visible::VisibilityFlags, PathingController, SharedMapPackInfo}, exports::runtime::{self as rt, Watched, imgui::{
            sys as imgui_sys, Condition, StyleVar, Ui, Window,
        }}, fl, render::{machine::RenderMachine, PathingConfig, RenderState}, settings::Settings, space::engine::Engine, with_i18n, Controller, ControllerEvent
    }, bitvec::{slice::BitSlice, vec::BitVec}, std::{collections::BTreeMap, sync::Arc}, taimi_pack::attributes::{AttrString, MarkerAttributes},
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
    pub current_map: BTreeMap<PackMapPath, BitVec>,
    pub category_names: BTreeMap<CategoryPath<PackPath>, Option<Arc<str>>>,
    pub category_tips: BTreeMap<CategoryPath<PackPath>, Option<(AttrString, AttrString)>>,
    pub category_copy: BTreeMap<CategoryPath<PackPath>, Option<(AttrString, AttrString)>>,
    pub pack_configs: BTreeMap<PackPath, Watched<Arc<PackConfig>>>,
    pub search_state: PathingSearchState,
    pub pack_info: Option<watch::Receiver<SharedMapPackInfo>>,
    pub pack_loader: Option<Arc<PackLoader>>,
    pub pack_loader_info: Option<watch::Receiver<SharedLoaderPackInfo>>,
    pub pack_loader_data: Option<watch::Receiver<SharedLoaderPackData>>,
    pub pack_loader_config: Option<watch::Receiver<SharedLoaderPackConfig>>,
    // draw stack state...
    act_selected_pack_active: Option<bool>,
    act_selected_category: Option<(CategoryPath<PackPath>, Option<bool>, bool, bool)>,
    act_selected_category_open: bool,
}

impl PathingWindowState {
    pub fn new() -> Self {
        Self {
            open: false,
            filter_open: false,
            filter_state: Default::default(),
            open_items: Default::default(),
            search_state: Default::default(),
            pack_info: Default::default(),
            pack_configs: Default::default(),
            current_state: Default::default(),
            current_map: Default::default(),
            category_names: Default::default(),
            category_tips: Default::default(),
            category_copy: Default::default(),
            pack_loader: Default::default(),
            pack_loader_info: Default::default(),
            pack_loader_data: Default::default(),
            pack_loader_config: Default::default(),
            act_selected_pack_active: Default::default(),
            act_selected_category: Default::default(),
            act_selected_category_open: Default::default(),
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
        let mut packs_changed = false;
        if let Some(pack_loader) = &self.pack_loader {
            let loader_info = self.pack_loader_info.get_or_insert_with(|| {
                let mut rx = pack_loader.shared_pack_info.subscribe();
                rx.mark_changed();
                rx
            });
            if let Some(loader_info) = loader_info.has_changed().unwrap_or(false).then(|| loader_info.borrow_and_update()) {
                packs_changed = true;
            };

            let loader_data = self.pack_loader_data.get_or_insert_with(|| {
                let mut rx = pack_loader.shared_pack_data.subscribe();
                rx.mark_changed();
                rx
            });
            if let Some(loader_data) = loader_data.has_changed().unwrap_or(false).then(|| loader_data.borrow_and_update()) {
                // todo
            };

            let loader_config = self.pack_loader_config.get_or_insert_with(|| {
                let mut rx = pack_loader.shared_pack_config.subscribe();
                rx.mark_changed();
                rx
            });
            if let Some(loader_config) = loader_config.has_changed().unwrap_or(false).then(|| loader_config.borrow_and_update()) {
                self.pack_configs.clear();
                self.pack_configs.extend(loader_config.iter().enumerate()
                    .filter_map(|(i, c)| {
                        let Some(mut c) = c.as_ref().map(Watched::subscribe_to) else { return None };
                        let _ = c.watch.try_mark_changed();
                        Some((PackPath::with_path(i as PackIndex), c))
                    })
                );
            };
        }
        for (_path, pack_config) in &mut self.pack_configs {
            let _ = pack_config.try_read_mut();
        }
        if packs_changed {
            self.refresh_packs();
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
        if self.pack_info.is_none() {
            self.pack_info = Controller::with_sender(|s| s.pack_info.as_ref().map(|info| {
                let mut info = info.clone();
                info.mark_changed();
                info
            })).flatten();
        }
        if let Some(pack_info) = &self.pack_info {
            if self.pack_loader.is_none() || matches!(pack_info.has_changed(), Ok(true) | Err(..)) {
                let pack_info = pack_info.borrow();
                if self.pack_loader.is_some() != pack_info.shared_loader.is_some() {
                    self.pack_loader = pack_info.shared_loader.clone();
                }
            }
        }
        if self.pack_loader.is_none() {
            self.reduce_memory();
            // TODO: free all other handles, controller shutdown probably
        }
    }

    pub fn reduce_memory(&mut self) {
        self.current_map.clear();
        self.current_state.clear();
        // TODO: retain any names with open parents that are presumably visible?
        self.category_names.clear();
        self.category_tips.clear();
        self.category_copy.clear();
    }

    fn refresh_packs(&mut self) {
        self.category_names.clear();
        self.category_tips.clear();
        self.category_copy.clear();
        for current_map in self.current_map.values_mut() {
            current_map.clear();
        }
        for current_state in self.current_state.values_mut() {
            current_state.clear();
        }
        let Some(pack_info) = &self.pack_info else { return };
    }

    fn refresh_search(&mut self) {
        let packs = PathingController::packs().blocking_read();
        let packs = packs.active_packs().map(|(path, _, pack)| (path, &*pack.pack));
        self.search_state.commit(packs);
    }

    pub fn refresh_state_of(&mut self, path: PackPath, info: &PackInfo) {
        let state = self.current_state.entry(path).or_default();
        state.clear();
        state.resize(info.categories.count(), true);
        for cat_path in info.categories.disabled() {
            Self::try_set_bit(state, cat_path.path as usize, false);
        }
        let _ = self.open_items.entry(path).or_default();
    }

    pub fn refresh_current_state(&mut self) {
        let map_id = Controller::with_sender(|s| s.gameplay.as_ref().and_then(|g|
            g.borrow().gameplay_map()
        ));
        if let Some(Some(map_id)) = map_id {
            self.refresh_current_map_state(map_id);
        }
    }

    pub fn refresh_current_map_state(&mut self, map_id: MapIndex) {
        let Some(pack_info) = &self.pack_info else { return };
        let pack_info = pack_info.borrow();

        let maps = pack_info.map_info.iter()
            .filter(|(path, _)| path.path == map_id);
        for (path, info) in maps {
            let Some(state) = self.current_state.get_mut(&path.root) else { continue };
            if state.is_empty() { continue }
            let Some(map) = pack_info.map_state.get(&path) else { continue };
            for (category_path, cat) in map.categories(info) {
                let index = category_path.path as usize;
                if index >= state.len() {
                    log::error!("category count ({}) mismatch! {category_path}", state.len());
                    continue
                }
                let vis_configured = cat.visibility.contains(VisibilityFlags::DEFAULT_TOGGLE);
                state.set(index, vis_configured);
            }
        }
    }

    pub fn refresh_current_map(&mut self, path: PackMapPath) {
        let filter = self.current_map.entry(path).or_default();
        if !filter.is_empty() {
            return
        }

        let Some(pack_info) = &self.pack_info else { return };
        let pack_info = pack_info.borrow();
        let (Some(Ok(pack_info)), Some(map_info)) = (
            pack_info.pack_info.get(&path.root),
            pack_info.map_info.get(&path),
        ) else { return };
        filter.resize(pack_info.categories.count(), false);

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
