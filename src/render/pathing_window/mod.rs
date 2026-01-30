use {
    crate::{
        controller::pathing::PathingEvent,
        fl,
        render::{machine::RenderMachine, PathingConfig, RenderState},
        settings::{
            state::ui::{pathing::PathingFilterFlags, PathingWindowState as UiState},
            Settings,
        },
        space::engine::Engine,
        with_i18n,
        Controller,
        ControllerEvent,
    },
    crate::render::element::pack::PackVisibility,
    crate::controller::pathing::shared::interact::InteractMessage,
    crate::exports::runtime::imgui::{self, ChildWindow, Condition, TableFlags, Ui, Window, WindowFlags, TreeNode, TreeNodeFlags, MouseButton},
    std::collections::HashSet,
    taimi_pack::category::CategoryId,
    taimi_sync::watched::Watched,
};

pub use self::filter::PathingSearchState;

mod filter;
mod menu;

pub struct PathingWindowState {
    pub open: bool,
    /// window can be `open` but minimized or collapsed
    ///
    /// (or even dragged off-screen?)
    pub visible: bool,
    pub filter_open: bool,
    pub filter_state: PathingFilterFlags,
    pub search_state: PathingSearchState,
    pub search_show_options: bool,
    pub search_focus_latch: bool,
    pub ui_state: Watched<UiState>,
}

impl PathingWindowState {
    pub fn new() -> Self {
        Self {
            open: false,
            visible: false,
            filter_open: false,
            filter_state: Default::default(),
            search_state: Default::default(),
            search_show_options: false,
            search_focus_latch: false,
            ui_state: Watched::empty_with(Default::default()),
        }
    }

    pub fn pre_render(&mut self) {
        if let Some(settings) = Settings::try_read() {
            self.open = settings.pathing_window_open;
            if self.ui_state.watch.get_receiver().is_none() {
                self.ui_state.restart_watching(&settings.ui_state.pathing_window);
            }
        };
        if let Some(ui_state) = self.ui_state.try_read_if_changed() {
            self.filter_open = ui_state.search.open;
            self.filter_state = ui_state.filter.flags;
            self.search_state.flags = ui_state.search.flags;
            match ui_state.search.query() {
                Some(query) if self.search_state.buffer.is_empty() => {
                    self.search_state.buffer = query.into();
                    self.search_state.commit(true);
                },
                _ => (),
            }
        }
        if !self.visible {
            self.search_focus_latch = false;
        }
    }
    pub fn pre_draw(&mut self, machine: &mut RenderMachine) {
        let filter_query = &mut machine.pack_ui_state.filter_query;
        let prev_flags = filter_query.flags;
        filter_query.set_flags(self.filter_state);
        if prev_flags != filter_query.flags {
            machine.pack_ui_state.filter_query.search = self.search_state.to_query();
        }
    }

    pub fn draw(
        &mut self,
        ui: &Ui,
        machine: &mut RenderMachine,
        engine: Option<&mut anyhow::Result<Engine>>,
    ) {
        let open = self.open;
        if open {
            self.draw_window(ui, machine, engine);
        }
        if open != self.open {
            Controller::try_send(ControllerEvent::WindowState(
                crate::WINDOW_PATHING.into(),
                Some(self.open),
            ));
        }
    }
    pub fn draw_window(
        &mut self,
        ui: &Ui,
        machine: &mut RenderMachine,
        engine: Option<&mut anyhow::Result<Engine>>,
    ) {
        let mut open = self.open;
        let visible = Window::new(fl!("pathing-window"))
            .size([300.0, 200.0], Condition::FirstUseEver)
            .nav_focus(false)
            .opened(&mut open)
            .build(ui, || {
                let pathing_dir = crate::ADDON_DIR.join("pathing");
                RenderState::draw_open_path_button(ui, fl!("open-button", kind = "folder"), &pathing_dir);
                self.draw_content(ui, machine, engine)
            });
        self.open = open;
        self.visible = match open {
            false => false,
            _ => visible.is_some(),
        };
    }
    pub fn draw_content(
        &mut self,
        ui: &Ui,
        machine: &mut RenderMachine,
        engine: Option<&mut anyhow::Result<Engine>>,
    ) {
        let rendered_err = if let Some(Ok(_engine)) = engine {
            None
        } else {
            Some(engine.map(|e| e.as_ref().err()))
        };
        let bookmark_tl = ui.item_rect_min();
        let bookmark_br = ui.item_rect_max();
        let draw_content = rendered_err.is_none() || machine.pack_ui_state.any_loaded();
        let tabs = draw_content.then(|| {
            ui.tab_bar("packs")
        }).flatten();
        let draw_packs = tabs.as_ref().and_then(|_| ui.tab_item("packz"));
        if let Some(e) = rendered_err {
            PathingConfig::draw_space_error(ui, machine, e.flatten());
        }
        if let Some(_tab) = draw_packs {
            let bookmark = ui.cursor_screen_pos();
            ui.set_cursor_screen_pos([bookmark_br[0], bookmark_tl[1]]);
            self.draw_categories_header(ui, machine);
            ui.set_cursor_screen_pos(bookmark);
            if self.filter_open {
                //ui.separator();
                self.draw_filter_content(ui, machine);
            }
            ChildWindow::new("pathing_subwindow")
                .flags(WindowFlags::ALWAYS_VERTICAL_SCROLLBAR)
                .size([0.0; 2])
                .build(ui, || {
                    self.draw_categories_content(ui, machine);
                });
        }
        let draw_pois = tabs.as_ref().and_then(|_| ui.tab_item("poiz"));
        if let Some(_tab) = draw_pois {
            #[cfg(deleteme)]
            if let Some(e) = rendered_err {
                PathingConfig::draw_space_error(ui, machine, e.flatten());
            }
            let bookmark = ui.cursor_screen_pos();
            ui.set_cursor_screen_pos([bookmark_br[0], bookmark_tl[1]]);
            if ui.button("rebuild") {
                if let Some(pathing) = machine.pathing.as_ref() {
                    PathingEvent::InteractControl(InteractMessage::RequestRebuild).try_send();
                }
            }
            ui.set_cursor_screen_pos(bookmark);

            machine.pack_ui_state.draw_interact(ui);
        }
        let draw_pe = tabs.as_ref().and_then(|_| ui.tab_item("editz"));
        if let Some(_tab) = draw_pe {
            let bookmark = ui.cursor_screen_pos();
            ui.set_cursor_screen_pos([bookmark_br[0], bookmark_tl[1]]);
            if machine.pack_ui_state.pack_edit.is_open() {
                if ui.button("close") {
                    machine.pack_ui_state.pack_edit.close();
                }
                ui.same_line();
                if ui.button("refresh tex") {
                    machine.pack_ui_state.pack_edit.refresh_textures();
                }
            }
            ui.set_cursor_screen_pos(bookmark);

            machine.pack_ui_state.draw_dynamic(ui);
        }
        drop(tabs);
    }
    pub fn draw_categories_header(&mut self, ui: &Ui, machine: &mut RenderMachine) {
        let mut drawn = false;
        ui.dummy([4.0; 2]);
        ui.same_line();
        drawn = true;
        if machine.pack_ui_state.any_loaded() {
            let button_text = match self.filter_open {
                true => fl!("hide-filter"),
                false => fl!("show-filter"),
            };
            if drawn {
                ui.same_line();
            }
            if ui.button(button_text) {
                self.filter_open = !self.filter_open;
                self.ui_state.write_with(|state| {
                    state.search.open = self.filter_open;
                });
            }
            drawn = true;

            if machine.pack_ui_state.can_expand() {
                ui.same_line();
                if ui.button(&fl!("expand-all")) {
                    machine.pack_ui_state.act_expand_all(!Self::FILTER_EXPAND_COLLAPSE);
                }
            }
        }
        if machine.pack_ui_state.can_collapse() {
            if drawn {
                ui.same_line();
            }
            if ui.button(&fl!("collapse-all")) {
                machine.pack_ui_state.act_collapse_all(!Self::FILTER_EXPAND_COLLAPSE);
            }
            drawn = true;
        }
        if drawn {
            ui.same_line();
        }
        if with_i18n!("reload-packs", |msg| ui.button(msg)) {
            PathingEvent::ReloadAll(true).try_send();
        }
        ui.same_line();
        if with_i18n!("deactivate-packs", |msg| ui.button(msg)) {
            PathingEvent::UnloadAll(false).try_send();
        }
        ui.same_line();
        if with_i18n!("remove-packs", |msg| ui.button(msg)) {
            PathingEvent::UnloadAll(true).try_send();
        }
    }
    const FILTER_EXPAND_COLLAPSE: bool = true;
    pub fn draw_categories_content(&mut self, ui: &Ui, machine: &mut RenderMachine) {
        let table_flags = TableFlags::RESIZABLE | TableFlags::ROW_BG | TableFlags::BORDERS;
        let table_name = format!("pathing");
        let table_token = ui.begin_table_with_flags(&table_name, 1, table_flags);
        ui.table_next_column();
        machine.pack_ui_state.draw(ui);
        if let Some(token) = table_token {
            token.end();
        }
        if !machine.pack_ui_state.any_loaded() {
            {
                let _font = RenderState::push_font("big", ui);
                with_i18n!("packs-empty", |msg| ui.text(msg));
            }
            {
                let _font = RenderState::push_font("ui", ui);
                with_i18n!("packs-empty-notice", |notice| ui.text_wrapped(notice));
            }
        }
    }
    pub fn draw_filter_content(&mut self, ui: &Ui, machine: &mut RenderMachine) {
        let filter_prev = self.filter_state;
        let search_dirty = self.draw_filters(ui, machine);
        ui.separator();

        let query_dirty = match search_dirty {
            Some(hard) => self.search_state.commit(!hard),
            None => false,
        };
        if query_dirty || filter_prev != self.filter_state {
            machine.pack_ui_state.filter_query.set_flags(self.filter_state);
            self.ui_state.write_if(|s| {
                let flags = (self.search_state.flags, self.filter_state);
                let changed = (s.search.flags, s.filter.flags) != flags;
                match self.search_state.query_str() {
                    Some(Some(query)) if !query.is_empty() && search_dirty != Some(true) => (),
                    Some(query) =>
                        s.search.query = query.cloned().unwrap_or_default(),
                    _ => (),
                }
                s.search.flags = flags.0;
                s.filter.flags = flags.1;
                changed.then_some(true)
            });
        }
        if query_dirty {
            machine.pack_ui_state.filter_query.search = self.search_state.to_query();
            machine.pack_ui_state.apply_search_filter();
        }
    }

    pub fn visibility(&self) -> PackVisibility {
        match self.open {
            true if !self.visible => PackVisibility::Pending,
            open => PackVisibility::visible(open),
        }
    }
}
