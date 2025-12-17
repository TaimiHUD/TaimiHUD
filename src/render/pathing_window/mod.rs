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
                Some(query) if self.search_state.buffer.is_empty() =>
                    self.search_state.buffer = query.into(),
                _ => (),
            }
        }
    }
    pub fn pre_draw(&mut self, _machine: &mut RenderMachine) {}

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
        if let Some(_window) = window {
            let pathing_dir = crate::ADDON_DIR.join("pathing");
            RenderState::draw_open_path_button(ui, fl!("open-button", kind = "folder"), &pathing_dir);
            self.draw_content(ui, machine, engine)
        }
        self.open = open;
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
            self.draw_categories_header(ui, machine);
            if self.filter_open {
                self.draw_filter_content(ui, machine);
            }
            None
        } else {
            Some(engine.map(|e| e.as_ref().err()))
        };
        if let Some(e) = rendered_err {
            PathingConfig::draw_space_error(ui, machine, e.flatten());
        }
        let content = ui.begin_content(c"pathing_subwindow", true);
        if let Some(_content) = content {
            self.draw_categories_content(ui, machine);
        }
    }
    pub fn draw_categories_header<'ui, U>(
        &mut self,
        ui: &mut U,
        machine: &mut RenderMachine,
    ) where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        if machine.pack_ui_state.any_loaded() {
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

            if machine.pack_ui_state.can_expand() {
                ui.same_line();
                if ui.button(fl!("expand-all")) {
                    machine.pack_ui_state.act_expand_all();
                }
            }
        }
        if machine.pack_ui_state.can_collapse() {
            ui.same_line();
            if ui.button(fl!("collapse-all")) {
                machine.pack_ui_state.act_collapse_all();
            }
        }
        ui.same_line();
        if with_i18n!("reload-packs", |msg| ui.button(msg)) {
            PathingEvent::ReloadAll(true).try_send();
        }
        ui.same_line();
        if with_i18n!("deactivate-packs", |msg| ui.button(msg)) {
            PathingEvent::UnloadAll(false).try_send();
        }
        if with_i18n!("remove-packs", |msg| ui.button(msg)) {
            PathingEvent::UnloadAll(true).try_send();
        }
    }
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
            machine.pack_ui_state.draw(ui);
            ui.table_next_column();
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
            let packs = machine
                .pack_ui_state
                .pack_state
                .values()
                .filter_map(|pack| pack.state.pack_data());
            self.search_state.commit(packs);
        }
    }
}
