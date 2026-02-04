use {
    crate::{
        controller::pathing::PathingEvent,
        fl,
        render::{machine::RenderMachine, PathingConfig, RenderState},
        settings::{
            state::ui::{pathing::PathingFilterFlags, AnchorPosition, WindowOpen, UiVec2, PathingWindowTab, PathingWindowState as UiState},
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
    std::mem,
};

pub use self::filter::PathingSearchState;

mod filter;
mod menu;

pub struct PathingWindowState {
    pub search_state: PathingSearchState,
    search_show_options: bool,
    search_focus_latch: bool,
    ui_state: Watched<UiState>,
    ui_state_pending: bool,
}

impl PathingWindowState {
    pub fn new() -> Self {
        Self {
            search_state: Default::default(),
            search_show_options: false,
            search_focus_latch: false,
            ui_state: Watched::empty_with(Default::default()),
            ui_state_pending: false,
        }
    }

    pub fn pre_render(&mut self) {
        if !self.ui_state.is_watching() {
            if let Some(settings) = Settings::try_read() {
                self.ui_state.restart_watching(settings.ui_state.pathing_window.sender());
            }
        };
        if let Some(ui_state) = self.ui_state.try_read_if_changed() {
            self.search_state.flags = ui_state.search.flags;
            match ui_state.search.query() {
                Some(query) if self.search_state.buffer.is_empty() => {
                    self.search_state.buffer = query.into();
                    self.search_state.commit(true);
                },
                _ => (),
            }
        }
        if !self.ui_state.window.open.is_visible() {
            self.search_focus_latch = false;
        }
    }
    pub fn pre_draw(&mut self, machine: &mut RenderMachine) {
        let filter_query = &mut machine.pack_ui_state.filter_query;
        let prev_flags = filter_query.flags;
        filter_query.set_flags(self.ui_state.filter.flags);
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
        if self.ui_state.window.open.is_active() {
            self.draw_window(ui, machine, engine);
        }
    }
    pub fn draw_window(
        &mut self,
        ui: &Ui,
        machine: &mut RenderMachine,
        engine: Option<&mut anyhow::Result<Engine>>,
    ) {
        let state = &*self.ui_state;
        let mut size = state.window_size().clone();
        let state = &state.window;
        let open = state.open;
        let mut opened = state.open.is_active();
        let mut pos = state.position_abs.get().copied();
        let pivot = match &mut pos {
            Some(..) => state.anchor,
            pos @ None => {
                *pos = Some(UiVec2::from(ui.io().display_size) * 0.5);
                AnchorPosition::Centre
            },
        };
        let has_pos = pos.is_some().then_some(Condition::Once).unwrap_or(Condition::Never);
        let visible = Window::new(fl!("pathing-window"))
            .size(size.into(), Condition::Appearing)
            .collapsed(open.is_collapsed(), Condition::Once)
            .position(pos.take().unwrap_or_default().into(), has_pos)
            .position_pivot(pivot.into())
            .nav_focus(false)
            .opened(&mut opened)
            .build(ui, || {
                size = ui.window_size().into();
                pos = (!ui.is_window_appearing()).then_some(ui.window_pos().into());
                #[cfg(todo = "unnecessary")]
                {
                    // XXX: imgui-rs abstracts this away from us and makes it impossible to discern .-.
                    visible &= !ui.is_window_collapsed();
                }
                let pathing_dir = crate::ADDON_DIR.join("pathing");
                RenderState::draw_open_path_button(ui, fl!("open-button", kind = "folder"), &pathing_dir);
                ui.same_line();
                ui.dummy([4.0; 2]);
                self.draw_content(ui, machine, engine)
            }).is_some();
        let open = match opened {
            true if !visible => WindowOpen::Collapsed,
            open => WindowOpen::new(open),
        };
        let ui_state = &mut *self.ui_state;
        if let Some(pos) = pos {
            ui_state.window.position_abs = pos;
            ui_state.window.position_rel = ui_state.window.position_abs / UiVec2::from(ui.io().display_size);
            ui_state.set_window_size(size);
        }
        let open_prev = mem::replace(&mut ui_state.window.open, open);
        self.ui_state_pending |= open_prev != open;
        if mem::take(&mut self.ui_state_pending) && self.ui_state.is_watching() {
            self.ui_state.commit_cloned();
        }
    }
    /// TODO: i18n and reconsider whether to even use tabs
    /// (edit should be a window anyway)
    fn draw_tab<'ui>(&mut self, ui: &'ui Ui, tab: usize) -> Option<imgui::TabItemToken<'ui>> {
        let prev = self.ui_state.tab.selected(tab);
        let mut opened = prev;
        let token = {
            let label = match tab {
                #[cfg(feature = "paths-interact")]
                PathingWindowTab::INDEX_POIS => "poiz",
                #[cfg(feature = "paths-edit")]
                PathingWindowTab::INDEX_EDIT => "editz",
                _ => "packz",
            };
            ui.tab_item_with_opened(label, &mut opened)
        };
        if !prev & opened {
            self.ui_state.tab.focus(tab);
            self.ui_state_pending = true;
        }
        token
    }
    const TABS: &[usize] = &[
        PathingWindowTab::INDEX_PACKS,
        #[cfg(feature = "paths-interact")]
        PathingWindowTab::INDEX_POIS,
        #[cfg(feature = "paths-edit")]
        PathingWindowTab::INDEX_EDIT,
    ];
    pub fn draw_content(
        &mut self,
        ui: &Ui,
        machine: &mut RenderMachine,
        engine: Option<&mut anyhow::Result<Engine>>,
    ) {
        let mut rendered_err = if let Some(Ok(_engine)) = engine {
            None
        } else {
            Some(engine.map(|e| e.as_ref().err()))
        };
        let bookmark_tl = ui.item_rect_min();
        let bookmark_br = ui.item_rect_max();
        let draw_content = rendered_err.is_none() || machine.pack_ui_state.any_loaded();
        let mut tabs_dirty = false;
        let tabs = draw_content.then(|| {
            ui.tab_bar("packs")
        }).flatten();
        for &tab_index in Self::TABS {
            let Some(_draw_tab) = tabs.as_ref().and_then(|_| self.draw_tab(ui, tab_index)) else { continue };
            if let Some(e) = rendered_err.take() {
                PathingConfig::draw_space_error(ui, machine, e.flatten());
            }
            match tab_index {
                #[cfg(feature = "paths-interact")]
                PathingWindowTab::INDEX_POIS => {
                    machine.pack_ui_state.draw_interact(ui);
                },
                #[cfg(feature = "paths-edit")]
                PathingWindowTab::INDEX_EDIT => {
                    machine.pack_ui_state.draw_dynamic(ui);
                },
                _ => {
                    if self.ui_state.search.open {
                        //ui.separator();
                        self.draw_filter_content(ui, machine);
                    }
                    ChildWindow::new("pathing_subwindow")
                        .flags(WindowFlags::ALWAYS_VERTICAL_SCROLLBAR)
                        .size([0.0; 2])
                        .build(ui, || {
                            self.draw_categories_content(ui, machine);
                        });
                },
            }
        }
        let bookmark = ui.cursor_screen_pos();
        ui.set_cursor_screen_pos([bookmark_br[0], bookmark_tl[1]]);
        match self.ui_state.tab.index() {
            #[cfg(feature = "paths-interact")]
            PathingWindowTab::INDEX_POIS => {
                if ui.button("rebuild") {
                    if let Some(pathing) = machine.pathing.as_ref() {
                        PathingEvent::InteractControl(InteractMessage::RequestRebuild).try_send();
                    }
                }
            },
            #[cfg(feature = "paths-edit")]
            PathingWindowTab::INDEX_EDIT => {
                if machine.pack_ui_state.pack_edit.is_open() {
                    if ui.button("close") {
                        machine.pack_ui_state.pack_edit.close();
                    }
                }
            },
            _ => {
                self.draw_categories_header(ui, machine);
            },
        }
        ui.set_cursor_screen_pos(bookmark);
        drop(tabs);
    }
    pub fn draw_categories_header(&mut self, ui: &Ui, machine: &mut RenderMachine) {
        let mut drawn = false;
        ui.dummy([4.0; 2]);
        ui.same_line();
        drawn = true;
        if machine.pack_ui_state.any_loaded() {
            let button_text = match self.ui_state.search.open {
                true => fl!("hide-filter"),
                false => fl!("show-filter"),
            };
            if drawn {
                ui.same_line();
            }
            if ui.button(button_text) {
                self.ui_state.search.open ^= true;
                self.ui_state_pending = true;
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
        let filter_prev = self.ui_state.filter.flags;
        let search_dirty = self.draw_filters(ui, machine);
        ui.separator();

        let query_dirty = match search_dirty {
            Some(hard) => self.search_state.commit(!hard),
            None => false,
        };
        let ui_state = &mut *self.ui_state;
        let filter_dirty = filter_prev != ui_state.filter.flags;
        self.ui_state_pending |= filter_dirty;
        if query_dirty || filter_dirty {
            self.ui_state_pending |= ui_state.search.flags != self.search_state.flags;
            ui_state.search.flags = self.search_state.flags;
            machine.pack_ui_state.filter_query.set_flags(ui_state.filter.flags);
            match self.search_state.query_str() {
                Some(Some(query)) if !query.is_empty() && search_dirty != Some(true) => (),
                Some(query) =>
                    ui_state.search.query = query.cloned().unwrap_or_default(),
                _ => (),
            }
        }
        if query_dirty {
            machine.pack_ui_state.filter_query.search = self.search_state.to_query();
            machine.pack_ui_state.apply_search_filter();
        }
    }

    pub fn window_visibility(&self) -> PackVisibility {
        self.ui_state.window.open.into()
    }
    pub fn packs_visibility(&self) -> PackVisibility {
        PackVisibility::visible_or_pending(self.ui_state.tab.selected_packs())
    }
    pub fn pois_visibility(&self) -> PackVisibility {
        PackVisibility::visible_or_pending(self.ui_state.tab.selected_pois())
    }
    #[cfg(feature = "paths-edit")]
    pub fn edit_visibility(&self) -> PackVisibility {
        PackVisibility::visible_or_pending(self.ui_state.tab.selected_edit())
    }
}
