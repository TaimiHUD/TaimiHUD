use {
    crate::{
        controller::pathing::{PathingController, PathingEvent}, fl, render::{machine::RenderMachine, PathingConfig, RenderState}, settings::Settings, space::pack::ActivePack, with_i18n, Controller, ControllerEvent
    },
    bitflags::bitflags,
    nexus::imgui::{ChildWindow, Id, TableColumnFlags, TableColumnSetup, TableFlags, Ui, Window, WindowFlags},
    regex::{Regex, RegexBuilder},
    std::{collections::HashSet, str::FromStr},
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
            _ => return None,
        })
    }
}

#[derive(Clone)]
pub struct PathingSearchState {
    pub buffer: String,
    matcher: Option<Regex>,
    search_candidates: HashSet<String>,
    pub ignore_space: bool,
    pub ignore_case: bool,
}

impl PathingSearchState {
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.matcher = None;
        self.search_candidates.clear();
    }

    pub fn commit<'p, P: Iterator<Item=&'p ActivePack>>(&mut self, packs: P) {
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

        for pack in packs {
            for (full_id, category) in pack.pack.categories.all_categories.iter() {
                if self.matches_name(&category.display_name) || self.matches_name(&category.id) {
                    self.search_candidates.insert(full_id.into());
                    let separators = full_id.rmatch_indices(".");
                    for (idx, _eu) in separators {
                        if let Some(sub_id) = full_id.get(..idx) {
                            self.search_candidates.insert(sub_id.into());
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
            ignore_case: true,
            ignore_space: true,
        }
    }
}

pub struct PathingWindowState {
    pub open: bool,
    pub filter_open: bool,
    pub filter_state: PathingFilterState,
    pub open_items: HashSet<String>,
    pub search_state: PathingSearchState,
}

impl PathingWindowState {
    pub fn new() -> Self {
        Self {
            open: false,
            filter_open: false,
            filter_state: Default::default(),
            open_items: Default::default(),
            search_state: Default::default(),
        }
    }

    pub fn draw(&mut self, ui: &Ui, machine: &mut RenderMachine) {
        let mut state_errors = Default::default();
        let mut open = self.open;
        if let Some(settings) = Settings::try_read() {
            open = settings.pathing_window_open;
        };
        if open {
            Window::new(fl!("pathing-window"))
                .size([300.0, 200.0], nexus::imgui::Condition::FirstUseEver)
                .opened(&mut open)
                .build(ui, || {
                    let pathing_dir = crate::ADDON_DIR.join("pathing");
                    RenderState::draw_open_button(&mut state_errors,
                        ui,
                        fl!("open-button", kind = "folder"),
                        pathing_dir.to_string_lossy(),
                    );
                    let rendered = crate::engine_mut(|engine| {
                                        ui.same_line();
                                        let button_text = match self.filter_open {
                                            true => fl!("hide-filter"),
                                            false => fl!("show-filter"),
                                        };
                                        if ui.button(button_text) {
                                            self.filter_open = !self.filter_open;
                                        }
                                        ui.same_line();
                                        if ui.button(&fl!("expand-all")) {
                                            for pack in engine.packs.loaded_packs.values() {
                                                let all_categories = &pack.pack.categories.all_categories;
                                                self.open_items.extend(all_categories.values().map(|x| x.full_id.clone()));
                                            }
                                        }
                                        ui.same_line();
                                            if ui.button(&fl!("collapse-all")) {
                                            self.open_items.clear();
                                        ui.separator();
                                        ui.dummy([4.0; 2]);
                                    }
                                    ui.same_line();
                                    // TODO? Engine::try_send(SpaceEvent::PackUnloadAll); instead of inline here...
                                    if ui.button("Reload All") {
                                        engine.packs.clear();
                                        PathingController::try_send(PathingEvent::PathingLoadAll);
                                    }
                                    ui.same_line();
                                    if ui.button("Unload All") {
                                        engine.packs.clear();
                                    }
                                        if self.filter_open {
                                            ui.separator();
                                            let pushy = ui.push_id("pathing-search");
                                            let mut search_dirty = ui.input_text("", &mut self.search_state.buffer)
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
                                            search_dirty |= ui.checkbox(&fl!("case-insensitive"), &mut self.search_state.ignore_case);
                                            ui.same_line();
                                            search_dirty |= ui.checkbox(&fl!("ignore-whitespace"), &mut self.search_state.ignore_space);
                                            pushy.pop();
                                            ui.dummy([4.0; 2]);
                                            ui.text(fl!("filter-options"));
                                            let filters = PathingFilterState::all().iter()
                                                .filter_map(|filter| filter.bit_as_str().map(|name| (filter, name)));
                                            for (i, (flag, filter_name)) in filters.enumerate() {
                                                if i > 0 && i % 3 != 0 {
                                                    ui.same_line();
                                                }
                                                with_i18n!(filter_name, |name|
                                                    ui.checkbox_flags(name, &mut self.filter_state, flag)
                                                );
                                            }
                                            ui.dummy([4.0; 2]);
                                            ui.separator();
                                            ui.dummy([4.0; 2]);

                                            if search_dirty {
                                                self.search_state.commit(engine.packs.loaded_packs.values());
                                            }
                                        }
                                    ChildWindow::new("pathing_subwindow")
                                        .flags(WindowFlags::ALWAYS_VERTICAL_SCROLLBAR)
                                        .size([0.0; 2])
                                        .build(ui, || {
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
                                        for pack in engine.packs.loaded_packs.values_mut() {
                                            let mut recompute = false;
                                            pack.draw_categories(
                                                ui,
                                                self.filter_state,
                                                &mut self.open_items,
                                                &mut recompute,
                                                &self.search_state
                                            );
                                            if recompute {
                                                pack.recompute_enabled(&engine.packs.active_festivals);
                                            }
                                        }
                                        if let Some(token) = table_token {
                                            token.end();
                                        }
                                    });
                    });
                    if rendered.is_none() {
                        PathingConfig::draw_space_error(ui, machine, None);
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
