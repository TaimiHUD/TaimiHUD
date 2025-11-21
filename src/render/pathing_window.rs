use {
    crate::{
        controller::pathing::{registry::{CategoryIndex, CategoryPath, LoadedPack, MapIndex, PackConfig, PackIndex, PackInfo, PackLoader, PackMapPath, PackPath, PackRoot, SharedLoaderPackConfig, SharedLoaderPackData, SharedLoaderPackInfo, UnloadedReason}, visible::VisibilityFlags, PathingController, PathingEvent, SharedMapPackInfo}, exports::runtime::{self as rt, Watched, imgui::{
            sys as imgui_sys, ChildWindow, Condition, Id, IdStackToken, MouseButton, StyleVar, TableColumnFlags, TableColumnSetup, TableFlags, TreeNode, TreeNodeFlags, TreeNodeToken, Ui, Window, WindowFlags
        }, locator::LocationRef, Locator}, fl, render::{machine::RenderMachine, PathingConfig, RenderState}, settings::Settings, space::engine::Engine, with_i18n, Controller, ControllerEvent
    }, bitflags::bitflags, bitvec::{slice::BitSlice, vec::BitVec}, regex::{Regex, RegexBuilder}, std::{borrow::Cow, collections::{btree_map, BTreeMap, BTreeSet, HashSet}, num::NonZero, str::FromStr, sync::Arc}, taimi_pack::{Category, attributes::AttrString, MarkerAttributes, Pack},
    tokio::sync::watch,
};

bitflags! {
    #[derive(PartialEq, Copy, Clone)]
    pub struct PathingFilterState: u8 {
        const Enabled = 1;
        const Disabled = 1 << 1;
        const CurrentMap = 1 << 2;
        const IgnoreRoot = 1 << 3;
        const IgnoreLeaves = 1 << 4;
        const IgnoreBranches = 1 << 5;
        const ShowHidden = 1 << 6;
    }
}

impl Default for PathingFilterState {
    fn default() -> Self {
        Self::Enabled | Self::Disabled | Self::IgnoreRoot
    }
}

impl FromStr for PathingFilterState {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "enabled" => Self::Enabled,
            "disabled" => Self::Disabled,
            "ignore-root" => Self::IgnoreRoot,
            "ignore-leaf" => Self::IgnoreLeaves,
            "ignore-branch" => Self::IgnoreBranches,
            "show-hidden" => Self::ShowHidden,
            "current-map" => Self::CurrentMap,
            _ => anyhow::bail!("unsupported filter option `{s}`"),
        })
    }
}

impl PathingFilterState {
    pub fn bit_as_str(self) -> Option<&'static str> {
        Some(match self.into_iter().next()? {
            Self::Enabled => "enabled",
            Self::Disabled => "disabled",
            Self::CurrentMap => "current-map",
            Self::IgnoreRoot => "ignore-root",
            Self::IgnoreLeaves => "ignore-leaf",
            Self::IgnoreBranches => "ignore-branch",
            Self::ShowHidden => "show-hidden",
            _ => return None,
        })
    }
}

#[derive(Clone)]
pub struct PathingSearchState {
    pub buffer: String,
    matcher: Option<Regex>,
    search_candidates: HashSet<String>,
    pub candidate_mask: BTreeMap<PackPath, BitVec>,
    pub ignore_space: bool,
    pub ignore_case: bool,
}

impl PathingSearchState {
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.matcher = None;
        self.search_candidates.clear();
        self.candidate_mask.clear();
    }

    pub fn commit<'p, P: Iterator<Item = (PackPath, &'p Pack)>>(&mut self, packs: P) {
        self.search_candidates.clear();
        if self.buffer.is_empty() {
            return
        }
        self.matcher = {
            let pattern = regex::escape(&self.buffer);
            let matcher = RegexBuilder::new(&pattern)
                .case_insensitive(self.ignore_case)
                .ignore_whitespace(self.ignore_space)
                .build();
            if let Err(e) = &matcher {
                log::warn!("search filter failure: {e:#}");
            }
            matcher.ok()
        };

        for (path, pack) in packs {
            {
                let mask = self.candidate_mask.entry(path).or_default();
                mask.clear();
                mask.resize(pack.categories.all_categories.len(), false);
            }

            for (idx, (full_id, category)) in pack.categories.all_categories.iter().enumerate() {
                if self.matches_name(&category.display_name) || self.matches_name(category.id().as_str()) {
                    let mask = self.candidate_mask.entry(path).or_default();
                    self.search_candidates.insert(full_id.into());
                    mask.set(idx, true);
                    for sub_id in full_id.as_id().ancestors() {
                        self.search_candidates.insert(sub_id.into());
                        if let Some(parent_idx) = pack.categories.all_categories.get_index_of(sub_id) {
                            mask.set(parent_idx, true);
                        }
                    }
                }
            }
        }
    }

    pub fn matches_name(&self, name: &str) -> bool {
        match &self.matcher {
            Some(regex) => regex.is_match(name),
            #[cfg(todo = "unnecessary")]
            None if self.buffer.is_empty() => false,
            None => name.contains(&self.buffer),
        }
    }

    pub fn matches_id(&self, full_id: &str) -> bool {
        match self.buffer.is_empty() {
            false => self.search_candidates.contains(full_id),
            true => true,
        }
    }
}

impl Default for PathingSearchState {
    fn default() -> Self {
        Self {
            buffer: Default::default(),
            matcher: Default::default(),
            search_candidates: Default::default(),
            candidate_mask: Default::default(),
            ignore_case: true,
            ignore_space: true,
        }
    }
}

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
        let mut open = self.open;
        Window::new(fl!("pathing-window"))
            .size([300.0, 200.0], Condition::FirstUseEver)
            .opened(&mut open)
            .build(ui, || self.draw_window(ui, machine, engine));

        if open != self.open {
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
        let rendered_err = if let Some(Ok(engine)) = engine {
            self.draw_content(ui, machine, engine);
            None
        } else {
            Some(engine.map(|e| e.as_ref().err()))
        };
        if let Some(e) = rendered_err {
            Self::draw_folder_button(ui);
            PathingConfig::draw_space_error(ui, machine, e.flatten());
        }
    }
    pub fn draw_content(
        &mut self,
        ui: &Ui,
        machine: &mut RenderMachine,
        engine: &mut Engine,
    ) {
        let Some(_tabs) = ui.tab_bar("pathing") else { return };
        if let Some(_token) = with_i18n!("enable", |msg| ui.tab_item(msg)) {
            self.draw_pack_list(ui, machine, engine);
        }
        if let Some(_token) = with_i18n!("poi", |msg| ui.tab_item(msg)) {
            self.draw_pois(ui, machine, engine);
        }
    }

    pub fn draw_pack_list(
        &mut self,
        ui: &Ui,
        machine: &mut RenderMachine,
        engine: &mut Engine,
    ) {
        Self::draw_folder_button(ui);
        if !engine.packs.loaded_packs.is_empty() {
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
                    if let Some(pack_info) = self.pack_info.as_ref().map(|i| i.borrow()) {
                        for (path, info) in pack_info.pack_info() {
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
        }
        if self.open_items.iter().any(|(_, open)| open.any()) {
            ui.same_line();
            if ui.button(&fl!("collapse-all")) {
                self.open_items.clear();
            }
        }
        ui.same_line();
        if with_i18n!("reload-packs", |msg| ui.button(msg)) {
            PathingEvent::ReloadAll.try_send();
        }
        ui.same_line();
        if with_i18n!("unload-packs", |msg| ui.button(msg)) {
            PathingEvent::UnloadAll.try_send();
        }
        if self.filter_open {
            ui.separator();
            let pushy = ui.push_id("pathing-search");
            let mut search_dirty = ui
                .input_text("", &mut self.search_state.buffer)
                .hint("Search")
                .build();
            ui.same_line();
            if ui.button("X") {
                self.search_state.clear();
            }
            if ui.is_item_hovered() {
                ui.tooltip_text(fl!("searchbar-clear"));
            }
            ui.same_line();
            search_dirty |=
                ui.checkbox(&fl!("case-insensitive"), &mut self.search_state.ignore_case);
            ui.same_line();
            search_dirty |=
                ui.checkbox(&fl!("ignore-whitespace"), &mut self.search_state.ignore_space);
            pushy.pop();
            ui.dummy([4.0; 2]);
            ui.text(fl!("filter-options"));
            let filters = PathingFilterState::all()
                .iter()
                .filter_map(|filter| filter.bit_as_str().map(|name| (filter, name)));
            for (i, (flag, filter_name)) in filters.enumerate() {
                if i > 0 && i % 3 != 0 {
                    ui.same_line();
                }
                with_i18n!(filter_name, |name| ui.checkbox_flags(
                    name,
                    &mut self.filter_state,
                    flag
                ));
            }
            ui.dummy([4.0; 2]);
            ui.separator();
            ui.dummy([4.0; 2]);

            if search_dirty {
                self.refresh_search();
            }
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
                    #[cfg(deleteme)]
                    [
                        TableColumnSetup {
                            name: &fl!("name"),
                            flags: TableColumnFlags::WIDTH_STRETCH,
                            init_width_or_weight: 0.0,
                            user_id: Id::Str("name"),
                        },
                        TableColumnSetup {
                            name: &fl!("toggle"),
                            flags: TableColumnFlags::WIDTH_FIXED,
                            init_width_or_weight: 0.0,
                            user_id: Id::Str("actions"),
                        },
                    ],
                    table_flags,
                );
                ui.table_next_column();
                let mut any_loaded = false;
                let packs: Vec<_> = {
                    let Some(pack_info) = &self.pack_info else {
                        return
                    };
                    let pack_info = pack_info.borrow();
                    pack_info.pack_info.iter()
                        .map(|(&path, info)| {
                            let loaded = pack_info.is_loaded(&path);
                            (path, info.clone(), loaded)
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
                            if self.draw_unloaded_pack(ui, unloaded.to_string(), Some(&unloaded.reason)) {
                                PathingEvent::LoadPack(path).try_send();
                            }
                        },
                    }
                }
                self.refresh_current_state();

                for (path, info, is_loaded) in packs {
                    let info = match (info, is_loaded) {
                        (Ok(info), true) => info,
                        (Ok(info), false) => {
                            if self.draw_unloaded_pack(ui, info.to_string(), None) {
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
                    {
                        let _font = RenderState::push_font("big", ui);
                        with_i18n!("packs-empty", |msg| ui.text(msg));
                    }
                    {
                        let _font = RenderState::push_font("ui", ui);
                        with_i18n!("packs-empty-notice", |notice| ui.text_wrapped(notice));
                    }
                }
            });
    }

    pub fn draw_pois(
        &mut self,
        ui: &Ui,
        machine: &mut RenderMachine,
        engine: &mut Engine,
    ) {
        use crate::controller::pathing::visible::{InteractionEvent, InteractionEventAction};
        use crate::controller::pathing::state::MarkerIndex;
        use crate::settings::pathing::TriggerKind;
        let Some(Some(map_id)) = Controller::with_sender(|s| s.gameplay.as_ref().and_then(|g|
                g.borrow().gameplay_map()
        )) else { return };
        let Some(pack_info) = &machine.pack_info else { return };
        let table_flags =
            TableFlags::RESIZABLE | TableFlags::ROW_BG | TableFlags::BORDERS;
        let table_token = ui.begin_table_header_with_flags(
            "interactive_pois",
            [
                TableColumnSetup {
                    name: &fl!("name"),
                    flags: TableColumnFlags::WIDTH_STRETCH,
                    init_width_or_weight: 0.0,
                    user_id: Id::Str("name"),
                },
                TableColumnSetup {
                    name: &fl!("category"),
                    flags: TableColumnFlags::WIDTH_FIXED,
                    init_width_or_weight: 0.0,
                    user_id: Id::Str("cat"),
                },
            ],
            table_flags,
        );
        ui.table_next_column();
        for (path, map) in &machine.map_pack_state {
            let path = path.rel(map_id);
            let pack_info = pack_info.borrow();
            let Some(info) = pack_info.map_info.get(&path) else { continue };
            let ipois = map.interactive_pois.iter().enumerate()
                .zip(map.interactive_pois_nearby.iter())
                .filter_map(|((i, ipoi), nearby)| match *nearby {
                    true => Some((i, ipoi)),
                    false => None,
                });
            for (ipoii, ipoi) in ipois {
                let loaded_path = ipoi.loaded_index();
                // TODO: cache this in the refresh .-.
                let Some(poi_path) = info.pois().nth(loaded_path.path as usize) else { continue };
                let poi_path = path.rel(poi_path.path);
                let guid_idx = info.poi_guid_mask()
                    .take(loaded_path.path as usize)
                    .filter(|&has| has)
                    .count();
                let guid = map.poi_guids.get(guid_idx).cloned();

                ui.text(guid.unwrap_or_default().to_string());

                ui.same_line();
                if ui.small_button("trigger") {
                    let _ = pack_info.interactions.send(InteractionEvent::Interact {
                        action: InteractionEventAction::Trigger,
                        path: poi_path.unscope(),
                        loaded_path: path.rel(loaded_path.path),
                        interactive_path: Locator::with_path(ipoii as u32),
                    });
                }
                if let Some(r) = &ipoi.reset {
                    ui.same_line();
                    if ui.small_button("reset") {
                        PathingEvent::GuidReset(r.guid.iter().cloned().collect()).try_send();
                    }
                }
                for (i, showhide) in ipoi.show_hide().enumerate() {
                    if i > 0 {
                        ui.same_line();
                    }
                    if ui.small_button(showhide.action.to_string()) {
                        let cat_path = showhide.category().pivot(path.root);
                        PathingEvent::CategorySetToggle(cat_path, showhide.action.tristate()).try_send();
                    }
                }
                if let Some(b) = &ipoi.behaviour {
                    if ui.small_button("dismiss") {
                        log::debug!("TODO: dismiss");
                        //PathingEvent::DismissMarker(poi_path, std::time::Duration::from_secs(5)).try_send();
                        let _ = pack_info.interactions.send(InteractionEvent::Interact {
                            action: InteractionEventAction::Manual(TriggerKind::BEHAVIOUR),
                            path: poi_path.unscope(),
                            loaded_path: path.rel(loaded_path.path),
                            interactive_path: Locator::with_path(ipoii as u32),
                        });
                    }
                }
                if let Some(b) = &ipoi.copy {
                    if ui.small_button("copy") {
                        let _ = pack_info.interactions.send(InteractionEvent::Interact {
                            action: InteractionEventAction::Manual(TriggerKind::COPY),
                            path: poi_path.unscope(),
                            loaded_path: path.rel(loaded_path.path),
                            interactive_path: Locator::with_path(ipoii as u32),
                        });
                    }
                }
                if let Some(b) = &ipoi.info {
                    if ui.small_button("info") {
                        let _ = pack_info.interactions.send(InteractionEvent::Interact {
                            action: InteractionEventAction::Manual(TriggerKind::INFO),
                            path: poi_path.unscope(),
                            loaded_path: path.rel(loaded_path.path),
                            interactive_path: Locator::with_path(ipoii as u32),
                        });
                    }
                }
                if let Some(b) = &ipoi.bounce {
                    if ui.small_button("anim") {
                        let _ = pack_info.interactions.send(InteractionEvent::Interact {
                            action: InteractionEventAction::Manual(TriggerKind::BOUNCE),
                            path: poi_path.unscope(),
                            loaded_path: path.rel(loaded_path.path),
                            interactive_path: Locator::with_path(ipoii as u32),
                        });
                    }
                }
                if let Some(b) = &ipoi.script {
                    if ui.small_button("script") {
                        let _ = pack_info.interactions.send(InteractionEvent::Interact {
                            action: InteractionEventAction::Manual(TriggerKind::SCRIPT),
                            path: poi_path.unscope(),
                            loaded_path: path.rel(loaded_path.path),
                            interactive_path: Locator::with_path(ipoii as u32),
                        });
                    }
                }

                ui.table_next_column();
                // TODO
                let hidden = true;
                if hidden && ui.small_button("unhide") {
                    if let Some(guid) = guid {
                        PathingEvent::GuidReset(vec![guid])
                    } else {
                        PathingEvent::ResetMarker(path.root.rel(MarkerIndex::with_poi(poi_path.path)))
                    }.try_send();
                }
                ui.table_next_column();
            }
        }
        drop(table_token);
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
                #[cfg(deleteme)]
                let vis_effective = cat.visibility.is_visible();
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
        PackLoader::shared_pack_active(packs?, path.root)
            .map(|active| active.pack.categories.all_categories.get_index(path.path as usize)
                .map(|(_id, cat)| cat.display_name.clone())
            )
    }
    fn category_display_name<'a>(packs: &Option<watch::Receiver<SharedLoaderPackData>>, category_names: &'a mut BTreeMap<CategoryPath<PackPath>, Option<Arc<str>>>, info: &PackInfo, path: CategoryPath<PackPath>) -> Option<&'a str> {
        let entry = match category_names.entry(path) {
            btree_map::Entry::Occupied(e) => return e.into_mut().as_ref().map(|s| &s[..]),
            btree_map::Entry::Vacant(e) => e,
        };
        Self::get_category_display_name(packs.as_ref().map(|d| d.borrow()).as_ref().map(|d| &**d), info, path)
            .map(|name| entry.insert(name).as_ref())
            .unwrap_or(None)
            .map(|s| &s[..])
    }

    fn get_category_tip(packs: Option<&SharedLoaderPackData>, _info: &PackInfo, path: CategoryPath<PackPath>) -> Option<(AttrString, AttrString)> {
        let Some(active) = PackLoader::shared_pack_active(packs?, path.root) else { return None };
        let Some((_, cat)) = active.pack.categories.all_categories.get_index(path.path as usize) else { return None };

        let tip_description = match cat.marker_attributes.tip_description.as_ref() {
            Some(desc) if desc.is_empty() => None,
            d => d,
        };
        let tip_name = match cat.marker_attributes.tip_name.as_ref() {
            Some(title) if title.is_empty() => None,
            Some(title) if cat.display_name.starts_with(&title[..]) => None,
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

    fn get_category_copy(packs: Option<&SharedLoaderPackData>, _info: &PackInfo, path: CategoryPath<PackPath>) -> Option<(AttrString, AttrString)> {
        let Some(active) = PackLoader::shared_pack_active(packs?, path.root) else { return None };
        let Some((_, cat)) = active.pack.categories.all_categories.get_index(path.path as usize) else { return None };

        let copy_value = cat.marker_attributes.copy_value.clone()?;
        let copy_message = match cat.marker_attributes.copy_message.as_ref() {
            Some(m) if m.is_empty() => None,
            m => m,
        };
        Some((copy_value, copy_message.cloned().unwrap_or_default()))
    }

    pub fn draw_unloaded_pack(&mut self, ui: &Ui, name: String, reason: Option<&UnloadedReason>) -> bool {
        let is_button = reason.is_some();
        let node = TreeNode::new(name)
            .flags(TreeNodeFlags::SPAN_AVAIL_WIDTH)
            .frame_padding(true)
            .tree_push_on_open(false)
            .opened(false, Condition::Always)
            .leaf(is_button)
            .push(ui);
        let hovered = ui.is_item_hovered();
        // TODO: hovered?
        let pressed = is_button && ui.is_item_clicked() && ui.is_mouse_released(MouseButton::Left);
        match reason {
            #[cfg(todo = "unused")]
            UnloadedReason::Disabled => compile_error!("TODO"),
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
        if let Some(node) = node {
            node.pop();
            !is_button || pressed
        } else {
            pressed
        }
    }

    pub fn draw_pack(&mut self, ui: &Ui, path: PackPath, info: &PackInfo) {
        let _pack_id = ui.push_id(path.path as i32);

        let map_path = Controller::with_sender(|s|
            s.gameplay.as_ref().and_then(|g| g.borrow().gameplay_map())
        ).flatten().map(|map_id| path.rel(map_id));
        if self.filter_state.contains(PathingFilterState::CurrentMap) {
            if let Some(map_path) = map_path {
                self.refresh_current_map(map_path);
            }
        }

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
            let token = self.category_header_start(ui, root_path, &display_name, open, Some(false), false, None);
            if let Some(root_path) = root_path {
                self.category_header_decorate(ui, info, root_path);
            }
            (root_path, token)
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
        if let Some(state) = state {
            ui.unindent();
            if let Some(toggled) = Self::category_toggle(ui, state) {
                self.commit_state(cat_path, toggled);
            }
            ui.same_line();
        }
        let (_id, tree) = self.category_header_start(ui, Some(cat_path), &display_name, open, is_leaf, is_decorative, Some(is_copyable));
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
            }.unwrap_or_else(|| format!("#{}", cat_path.path));
            self.draw_category(ui, info, (child_path, map_path), &display_name);
        }
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
    ) -> (IdStackToken<'u>, Option<TreeNodeToken<'u>>) {
        let push_token = match path {
            Some(path) => ui.push_id(path.path as i32 ^ ((path.root.path as i32) << 20)),
            _ => ui.push_id(display_name),
        };

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
        (push_token, tree_token)
    }

    const NAME_TEMPLATE: &'static str = "Generic Copyable Marker Name";
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
        let pack_data = self.pack_loader_data.as_ref();
        let pack_data = || pack_data.map(|d| d.borrow());
        if is_copyable {
            ui.same_line();
            if ui.small_button(&fl!("copy")) {
                if let Some((copy_value, copy_message)) = self.category_copy.entry(path)
                    .or_insert_with(|| Self::get_category_copy(pack_data().as_ref().map(|d| &**d), info, path))
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
                let tip = self.category_tips.entry(path)
                    .or_insert_with(|| Self::get_category_tip(pack_data().as_ref().map(|d| &**d), info, path));
                Self::draw_tooltip(ui, display_name.unwrap_or(Self::NAME_TEMPLATE), || {
                    if let Some((title, desc)) = tip {
                        Self::draw_tooltip_category(ui, display_name.unwrap_or_default(), &title[..], &desc[..]);
                    }
                    if let Some((copy_value, copy_message)) = self.category_copy.entry(path)
                        .or_insert_with(|| Self::get_category_copy(pack_data().as_ref().map(|d| &**d), info, path))
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
            self.category_tips.entry(path)
                .or_insert_with(|| Self::get_category_tip(pack_data().as_ref().map(|d| &**d), info, path))
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

    #[cfg(todo)]
    pub fn draw_category<'u>() {
        if category.is_hidden {
            return
        }
        let (_id_token, tree_token) = self.draw_category_header();
        if let Some(_token) = tree_token {
            if !open_items.contains(&category.full_id)
                && !category.is_separator
                && !category.sub_categories.is_empty()
            {
                open_items.insert(category.full_id.clone());
            }
            if !category.sub_categories.is_empty() {
                ui.indent(); //_by(1.0);
            }
            for (_local, global) in category.sub_categories.iter() {
                Self::draw_category(
                    ui,
                    path,
                    &all_categories[global],
                    all_categories,
                    state,
                    filter_state,
                    open_items,
                    false,
                    recompute,
                    search_state,
                    category_filter,
                    copyable,
                );
            }
            if !category.sub_categories.is_empty() {
                ui.unindent(); //_by(1.0);
            }
        }
    }

    fn copy_copyable(ui: &Ui, copy_value: &str, copy_message: &str) {
        if !copy_value.is_empty() {
            ui.set_clipboard_text(copy_value);
        }
        if !copy_message.is_empty() {
            let _ = rt::send_alert(ui, copy_message);
        }
    }

    fn draw_tooltip_category(ui: &Ui, display_name: &str, title: &str, desc: &str) {
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

    #[cfg(deleteme)]
    fn category_has_tooltip<N, D>(display_name: &str, tip_name: Option<N>, tip_description: Option<D>) -> bool where
        N: AsRef<str>,
        D: AsRef<str>,
    {
        let tip_description = tip_description.as_ref().map(AsRef::as_ref);
        match tip_description {
            Some(desc) if !desc.is_empty() => return true,
            _ => (),
        }
        let tip_name = tip_name.as_ref().map(AsRef::as_ref);
        match tip_name {
            Some(title) if !title.is_empty() && !display_name.starts_with(title) => return true,
            _ => (),
        }

        false
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
