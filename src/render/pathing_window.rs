use {
    crate::{engine_initialized, fl, ControllerEvent, Controller, ENGINE, SETTINGS}, bitflags::bitflags, indexmap::IndexMap, nexus::imgui::{ComboBox, Id, TableColumnFlags, TableColumnSetup, TableFlags, Ui, Window}, std::{
        collections::{HashMap, HashSet},
        sync::Arc,
    }
};

bitflags! {
    #[derive(PartialEq, Copy, Clone)]
    pub struct PathingFilterState: u8 {
        const Enabled = 1;
        const Disabled = 1 << 1;
        const IgnoreRoot = 1 << 2;
        const IgnoreLeaves = 1 << 3;
        const IgnoreBranches = 1 << 4;
    }
}

impl Default for PathingFilterState {
    fn default() -> Self {
        Self::Enabled | Self::Disabled | Self::IgnoreRoot
    }
}

impl PathingFilterState {
    pub fn filter_string_to_flag(str: &str) -> Self {
        match str {
            "enabled" => Self::Enabled,
            "disabled" => Self::Disabled,
            "ignore-root" => Self::IgnoreRoot,
            "ignore-leaf" => Self::IgnoreLeaves,
            "ignore-branch" => Self::IgnoreBranches,
            _ => unreachable!("no"),
        }
    }
}

#[derive(Default, Clone)]
pub struct PathingSearchState {
    pub buffer: String,
    pub search_candidates: HashSet<String>,
}

pub struct PathingWindowState {
    pub open: bool,
    pub filter_open: bool,
    pub filter_state: PathingFilterState,
    pub open_items: HashSet<String>,
    pub search_state: PathingSearchState,
    pub filter_options: IndexMap<String, String>,
}

impl PathingWindowState {
    pub fn new() -> Self {
        let mut filter_options: IndexMap<String, String> = IndexMap::new();
        filter_options.insert("enabled".to_string(),fl!("enabled"));
        filter_options.insert("disabled".to_string(), fl!("disabled"));
        filter_options.insert("ignore-root".to_string(), fl!("ignore-root"));
        filter_options.insert("ignore-leaf".to_string(), fl!("ignore-leaf"));
        filter_options.insert("ignore-branch".to_string(), fl!("ignore-branch"));
        Self {
            open: false,
            filter_open: false,
            filter_state: Default::default(),
            open_items: Default::default(),
            search_state: Default::default(),
            filter_options,
        }
    }

    pub fn draw(&mut self, ui: &Ui) {
        let mut open = self.open;
        if let Some(settings) = SETTINGS.get().and_then(|settings| settings.try_read().ok()) {
            open = settings.pathing_window_open;
        };
        if open {
            Window::new(fl!("pathing-window"))
                .size([300.0, 200.0], nexus::imgui::Condition::FirstUseEver)
                .opened(&mut open)
                .build(ui, || {
                    if engine_initialized() {
                        ENGINE.with_borrow_mut(|e| {
                            if let Some(Ok(engine)) = e {
                                        let button_text = match self.filter_open {
                                            true => fl!("hide-filter"),
                                            false => fl!("show-filter"),
                                        };
                                        if ui.button(button_text) {
                                            self.filter_open = !self.filter_open;
                                        }
                                        ui.same_line();
                                        if ui.button(&fl!("expand-all")) {
                                            for (name, pack) in &engine.packs.loaded_packs {
                                                let all_categories = &pack.categories.all_categories;
                                                self.open_items.extend(all_categories.values().map(|x| x.full_id.clone()));
                                            }
                                        }
                                        ui.same_line();
                                            if ui.button(&fl!("collapse-all")) {
                                            self.open_items.clear();
                                        ui.separator();
                                        ui.dummy([4.0; 2]);
                                    }
                                        if self.filter_open {
                                            let mut update_search = false;
                                            ui.separator();
                                            let pushy = ui.push_id("pathing-search");
                                            if ui.input_text("", &mut self.search_state.buffer)
                                                .hint("Search")
                                                .build() {
                                                update_search = true;
                                            }
                                            if update_search {
                                                self.search_state.search_candidates.clear();
                                                if !self.search_state.buffer.is_empty() {
                                                    for (_s, pack) in &engine.packs.loaded_packs {
                                                        for (full_id, category) in pack.categories.all_categories.iter() {
                                                            if category.display_name.contains(&self.search_state.buffer) {
                                                                self.search_state.search_candidates.insert(full_id.to_string());
                                                                let separators: Vec<_> = full_id.rmatch_indices(".").collect();
                                                                for (idx, _eu) in separators {
                                                                    let sub_id = &full_id[..idx];
                                                                    self.search_state.search_candidates.insert(sub_id.to_string());
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            ui.same_line();
                                            if ui.button("X") {
                                                self.search_state.buffer.clear();
                                                self.search_state.search_candidates.clear();
                                            }
                                            if ui.is_item_hovered() {
                                                ui.tooltip_text(fl!("searchbar-clear"));
                                            }
                                            pushy.pop();
                                            ui.dummy([4.0; 2]);
                                            ui.text(fl!("filter-options"));
                                            for (filter, filter_name) in &self.filter_options {
                                                ui.checkbox_flags(filter_name, &mut self.filter_state, PathingFilterState::filter_string_to_flag(filter));
                                            }
                                            ui.dummy([4.0; 2]);
                                            ui.separator();
                                            ui.dummy([4.0; 2]);
                                        }

                                    let table_flags = TableFlags::RESIZABLE
                                        | TableFlags::ROW_BG
                                        | TableFlags::BORDERS;
                                    let table_name = format!("pathing");
                                    let table_token = ui.begin_table_header_with_flags(
                                        &table_name,
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
                                    for (name, mut pack) in &mut engine.packs.loaded_packs {
                                        let mut recompute = false;
                                        let root = &mut pack.categories.root_categories;
                                        let all_categories = &pack.categories.all_categories;
                                        let enabled_categories = &mut pack.user_category_state;
                                        for cat_name in root.iter() {
                                            all_categories[cat_name].draw(
                                                ui,
                                                all_categories,
                                                enabled_categories,
                                                self.filter_state,
                                                &mut self.open_items,
                                                true,
                                                &mut recompute,
                                                &self.search_state
                                            );
                                        }
                                        if recompute {
                                            pack.recompute_enabled();
                                        }
                                    }
                                    if let Some(token) = table_token {
                                        token.end();
                                    }
                            }
                        });
                    }
                });
        }

        if open != self.open {
            Controller::try_send(ControllerEvent::WindowState(
                crate::WINDOW_PATHING.into(),
                Some(open),
            ));
            self.open = open;
        }
    }
}
