use {
    crate::{
        controller::pathing::PathingEvent,
        fl,
        render::{machine::RenderMachine, PathingConfig, RenderState},
        settings::{
            state::ui::{pathing::PathingFilterFlags, PathingWindowState as UiState},
            Settings,
        },
        space::{engine::Engine, pack::UnloadedReason},
        with_i18n,
        Controller,
        ControllerEvent,
    },
    nexus::imgui::{
        ChildWindow,
        Condition,
        Id,
        TableColumnFlags,
        TableColumnSetup,
        TableFlags,
        TreeNode,
        TreeNodeFlags,
        Ui,
        Window,
        WindowFlags,
    },
    std::collections::HashSet,
    taimi_pack::category::CategoryId,
    taimi_sync::watched::Watched,
};

pub use self::filter::PathingSearchState;

mod filter;
mod menu;

pub struct PathingWindowState {
    pub open: bool,
    pub filter_open: bool,
    pub filter_state: PathingFilterFlags,
    pub open_items: HashSet<CategoryId>,
    pub search_state: PathingSearchState,
    pub ui_state: Watched<UiState>,
}

impl PathingWindowState {
    pub fn new() -> Self {
        Self {
            open: false,
            filter_open: false,
            filter_state: Default::default(),
            open_items: Default::default(),
            search_state: Default::default(),
            ui_state: Watched::empty_with(Default::default()),
        }
    }

    pub fn draw(
        &mut self,
        ui: &Ui,
        machine: &mut RenderMachine,
        engine: Option<&mut anyhow::Result<Engine>>,
    ) {
        let mut open = self.open;
        if let Some(settings) = Settings::try_read() {
            open = settings.pathing_window_open;
            if self.ui_state.watch.get_receiver().is_none() {
                self.ui_state.restart_watching(&settings.ui_state.pathing_window);
            }
        };
        if self.ui_state.watch.has_changed() {
            let ui_state = self.ui_state.get_mut();
            self.filter_open = ui_state.search.open;
            self.filter_state = ui_state.filter.flags;
            self.search_state.flags = ui_state.search.flags;
            match ui_state.search.query() {
                Some(query) if self.search_state.buffer.is_empty() =>
                    self.search_state.buffer = query.into(),
                _ => (),
            }
        }
        if open {
            Window::new(fl!("pathing-window"))
                .size([300.0, 200.0], Condition::FirstUseEver)
                .opened(&mut open)
                .build(ui, || {
                    let pathing_dir = crate::ADDON_DIR.join("pathing");
                    RenderState::draw_open_path_button(
                        ui,
                        fl!("open-button", kind = "folder"),
                        &pathing_dir,
                    );
                    let rendered_err = if let Some(Ok(engine)) = engine {
                        if !engine.packs.loaded_packs.is_empty() {
                            ui.same_line();
                            let button_text = match self.filter_open {
                                true => fl!("hide-filter"),
                                false => fl!("show-filter"),
                            };
                            if ui.button(button_text) {
                                self.filter_open = !self.filter_open;
                                self.ui_state.write_with(|state| {
                                    state.search.open = self.filter_open;
                                });
                            }

                            ui.same_line();
                            if ui.button(&fl!("expand-all")) {
                                for pack in engine.packs.loaded_packs.values() {
                                    let all_categories = &pack.pack.categories.all_categories;
                                    self.open_items
                                        .extend(all_categories.values().map(|x| x.full_id.clone()));
                                }
                            }
                        }
                        if !self.open_items.is_empty() {
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
                            let filter_prev = self.filter_state;
                            let search_dirty = self.draw_filters(ui);
                            ui.dummy([4.0; 2]);
                            ui.separator();
                            ui.dummy([4.0; 2]);

                            if search_dirty || filter_prev != self.filter_state {
                                self.ui_state.write_if(|s| {
                                    let flags = (self.search_state.flags, self.filter_state);
                                    let changed = (s.search.flags, s.filter.flags) != flags;
                                    s.search.query = self.search_state.buffer.clone();
                                    s.search.flags = flags.0;
                                    s.filter.flags = flags.1;
                                    changed.then_some(true)
                                });
                            }
                            if search_dirty {
                                self.search_state.commit(engine.packs.loaded_packs.values());
                            }
                        }
                        ChildWindow::new("pathing_subwindow")
                            .flags(WindowFlags::ALWAYS_VERTICAL_SCROLLBAR)
                            .size([0.0; 2])
                            .build(ui, || {
                                let table_flags =
                                    TableFlags::RESIZABLE | TableFlags::ROW_BG | TableFlags::BORDERS;
                                let table_name = format!("pathing");
                                let table_token = ui.begin_table_with_flags(&table_name, 1, table_flags);
                                ui.table_next_column();
                                for (name, reason) in &engine.packs.unloaded_packs {
                                    let node = TreeNode::new(name)
                                        .flags(TreeNodeFlags::SPAN_AVAIL_WIDTH)
                                        .frame_padding(true)
                                        .tree_push_on_open(false)
                                        .leaf(true)
                                        .push(ui);
                                    let hovered = ui.is_item_hovered();
                                    match reason {
                                        #[cfg(todo = "unused")]
                                        UnloadedReason::Disabled => compile_error!("TODO"),
                                        UnloadedReason::UnknownFormat => {
                                            ui.same_line();
                                            with_i18n!("unknown", |msg| ui.text(msg));
                                            if hovered {
                                                ui.tooltip_text("taco zip or folder expected");
                                            }
                                        },
                                        UnloadedReason::LoadingFailed(reason) => {
                                            ui.same_line();
                                            with_i18n!("error", |msg| ui.text(msg));
                                            if hovered {
                                                ui.tooltip_text(reason);
                                            }
                                        },
                                    }
                                    ui.table_next_column();
                                    if let Some(node) = node {
                                        node.pop()
                                    }
                                }
                                for pack in engine.packs.loaded_packs.values_mut() {
                                    let mut recompute = false;
                                    pack.draw_categories(
                                        ui,
                                        self.filter_state,
                                        &mut self.open_items,
                                        &mut recompute,
                                        &self.search_state,
                                    );
                                    if recompute {
                                        pack.recompute_enabled(&engine.packs.active_festivals);
                                    }
                                }
                                if let Some(token) = table_token {
                                    token.end();
                                }
                                if engine.packs.loaded_packs.is_empty() {
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
                        None
                    } else {
                        Some(engine.map(|e| e.as_ref().err()))
                    };
                    if let Some(e) = rendered_err {
                        PathingConfig::draw_space_error(ui, machine, e.flatten());
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
