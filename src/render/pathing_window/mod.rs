use {
    crate::{
        controller::pathing::PathingEvent,
        render::{element::prelude::*, machine::RenderMachine, PathingConfig, RenderState},
        settings::{
            state::ui::{pathing::PathingFilterFlags, PathingWindowState as UiState},
            Settings,
        },
        space::engine::Engine,
        with_i18n,
        Controller,
        ControllerEvent,
    },
    crate::render::element::pack::{
        interact::DrawPoiInfo,
        PackVisibility,
    },
    crate::controller::pathing::shared::interact::InteractMessage,
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

    pub fn draw<'ui, U>(
        &mut self,
        ui: &mut U,
        machine: &mut RenderMachine,
        engine: Option<&mut anyhow::Result<Engine>>,
    ) where
        U: ?Sized + ImDrawWindow<'ui>,
    {
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
    pub fn draw_window<'ui, U>(
        &mut self,
        ui: &mut U,
        machine: &mut RenderMachine,
        engine: Option<&mut anyhow::Result<Engine>>,
    ) where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let mut open = self.open;
        let window = with_i18n!("pathing-window", |title| ui.begin_taimi_window(
            "pathing-window",
            title,
            ImCondition::initial(ImSize2::new(300.0, 200.0)),
            &mut open,
        ));
        let visible = window.is_some();
        if let Some(_window) = window {
            let pathing_dir = crate::ADDON_DIR.join("pathing");
            RenderState::draw_open_path_button(ui, fl!("open-button", kind = "folder"), &pathing_dir);
            self.draw_content(ui, machine, engine)
        }
        self.open = open;
        self.visible = match open {
            false => false,
            _ => visible,
        };
    }
    pub fn draw_content<'ui, U>(
        &mut self,
        ui: &mut U,
        machine: &mut RenderMachine,
        engine: Option<&mut anyhow::Result<Engine>>,
    ) where
        U: ?Sized + ImDrawWindow<'ui>,
    {
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
            let content = ui.begin_content(c"pathing_subwindow", true);
            if let Some(_content) = content {
                self.draw_categories_content(ui, machine);
            }
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

            self.draw_interact_content(ui, machine);
        }
        drop(tabs);
    }
    pub fn draw_categories_header<'ui, U>(
        &mut self,
        ui: &mut U,
        machine: &mut RenderMachine,
    ) where
        U: ?Sized + ImDrawWindow<'ui>,
    {
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
                if ui.button(fl!("expand-all")) {
                    machine.pack_ui_state.act_expand_all(!Self::FILTER_EXPAND_COLLAPSE);
                }
            }
        }
        if machine.pack_ui_state.can_collapse() {
            if drawn {
                ui.same_line();
            }
            if ui.button(fl!("collapse-all")) {
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
    pub fn draw_categories_content<'ui, U>(
        &mut self,
        ui: &mut U,
        machine: &mut RenderMachine,
    ) where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let table_flags = match ui.imgui_version_num() {
            #[cfg(taimi_imgui = "180")]
            Some(im180::VERSION_NUM) => imw::DynFlagsContainer::new(Some(
                im180::sys::ImGuiTableFlags_Resizable
                    | im180::sys::ImGuiTableFlags_RowBg
                    | im180::sys::ImGuiTableFlags_Borders,
            )),
            #[cfg(taimi_imgui = "192")]
            Some(im192::VERSION_NUM) => imw::DynFlagsContainer::new(Some(
                im192::sys::ImGuiTableFlags_Resizable
                    | im192::sys::ImGuiTableFlags_RowBg
                    | im192::sys::ImGuiTableFlags_Borders,
            )),
            _ => Default::default(),
        };
        let table_token = ui.begin_table_with_flags(c"pathing", 1, table_flags);
        #[cfg(deleteme)] {
            ui.table_next_column();
            for (name, reason) in &engine.packs.unloaded_packs {
                let node = ui.begin_tree_leaf_wide(name, name, true);
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
                node.end();
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
                    let external = PathingController::external_filter_state();
                    pack.recompute_enabled(external.as_ref());
                }
            }
        }
        if let Some(_token) = table_token {
            ui.table_next_column();
            machine.pack_ui_state.draw(ui);
        }
        if !machine.pack_ui_state.any_loaded() {
            with_i18n!("packs-empty", |msg| ui.text_with_font(NexusLinkFont::Big, msg));
            ui.with_font(NexusLinkFont::Ui, |ui| {
                with_i18n!("packs-empty-notice", |notice| ui.text_wrapped(notice))
            });
        }
    }
    pub fn draw_filter_content<'ui, U>(
        &mut self,
        ui: &mut U,
        machine: &mut RenderMachine,
    ) where
        U: ?Sized + ImDrawWindow<'ui>,
    {
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
            let packs = machine
                .pack_ui_state
                .pack_state
                .values()
                .filter_map(|pack| pack.state.pack_data());
            machine.pack_ui_state.filter_query.search = self.search_state.to_query();
            machine.pack_ui_state.apply_search_filter();
        }
    }

    pub fn draw_interact_content(&mut self, ui: &Ui, machine: &mut RenderMachine) {
        let mut draw = DrawPoiInfo::new(
            ui,
            &mut machine.pack_ui_state.interact,
            machine.pack_ui_state.pack_state.map_ref_as_slice(),
        );
        let was_context = draw.state.context.is_some();
        draw.draw();
        let context_id = "poi-context";
        if draw.state.context.is_some() && !was_context {
            ui.open_popup(context_id);
        }
        let popup = ui.begin_popup(context_id);
        let popup_drawn = popup.is_some();
        if let Some(_token) = popup {
            if let Some(context) = &mut draw.state.context {
                let mut menu = context.prepare_draw_menu(ui);
                menu.draw();
                context.finish_draw_menu(menu);
            } else {
                ui.close_current_popup();
            }
        }
        if draw.state.context.is_some() == was_context && popup_drawn != was_context {
            draw.state.context = None;
        }
    }

    pub fn visibility(&self) -> PackVisibility {
        match self.open {
            true if !self.visible => PackVisibility::Pending,
            open => PackVisibility::visible(open),
        }
    }
}
