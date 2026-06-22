use {
    crate::{
        controller::pathing::PathingEvent,
        render::{
            element::{pack::PackVisibility, prelude::*},
            machine::RenderMachine,
            PathingConfig,
            RenderState,
        },
        settings::{
            state::ui::{
                AnchorPosition,
                PathingWindowState as UiState,
                PathingWindowTab,
                UiVec2,
                WindowOpen,
            },
            Settings,
        },
        space::engine::Engine,
        with_i18n,
    },
    std::mem,
    taimi_sync::watched::Watched,
};

pub use self::filter::PathingSearchState;
#[cfg(feature = "paths-interact")]
use crate::controller::pathing::shared::interact::InteractMessage;

mod filter;
mod menu;

pub struct PathingWindowState {
    pub search_state: PathingSearchState,
    search_show_options: bool,
    search_focus_latch: bool,
    ui_state: Watched<UiState>,
    ui_state_pending: bool,
    pub ui_tab_pending: Option<usize>,
}

impl PathingWindowState {
    pub fn new() -> Self {
        Self {
            search_state: Default::default(),
            search_show_options: false,
            search_focus_latch: false,
            ui_state: Watched::empty_with(Default::default()),
            ui_state_pending: false,
            ui_tab_pending: None,
        }
    }

    pub fn pre_render(&mut self) {
        if !self.ui_state.is_watching() {
            if let Some(settings) = Settings::try_read() {
                self.ui_state
                    .restart_watching(settings.ui_state.pathing_window.sender());
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

    pub fn draw<'ui, U>(
        &mut self,
        ui: &mut U,
        machine: &mut RenderMachine,
        engine: Option<&mut anyhow::Result<Engine>>,
    ) where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        if self.ui_state.window.open.is_active() {
            self.draw_window(ui, machine, engine);
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
        if !self.ui_state.is_watching() {
            return
        }
        let state = &*self.ui_state;
        if state.window.open.is_closed() {
            return
        }
        let mut size = state.window_size().clone();
        let state = &state.window;
        let open = state.open;
        let mut pos = state.position_abs.get().copied();
        let pivot = match &mut pos {
            Some(..) => state.anchor,
            pos @ None => {
                *pos = Some(ui.with_io_dyn(|io| io.display_size() * 0.5).into());
                AnchorPosition::Centre
            },
        };
        if let Some(pos) = pos.take() {
            ui.window_prepare_pos(pos.to_point(), ImCondition::Startup, pivot.into());
        }
        ui.window_prepare_collapsed(open.is_collapsed(), ImCondition::Startup);
        let mut open = open.into();
        let window = with_i18n!("pathing-window", |title| ui.begin_taimi_window(
            "pathing-window",
            title,
            ImCondition::startup(size.to_size()),
            &mut open,
        ));
        let visible = window.is_some();
        if let Some(_token) = window {
            size = ui.window_size().into();
            let appearing = ui.window_is_appearing();
            if appearing && self.ui_tab_pending.is_none() {
                self.ui_tab_pending = Some(self.ui_state.tab.index());
            }
            pos = (!appearing).then_some(ui.window_pos().into());
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
        }
        let open = match open {
            true if !visible => WindowOpen::Collapsed,
            open => WindowOpen::new(open),
        };
        let ui_state = &mut *self.ui_state;
        if let Some(pos) = pos {
            ui_state.window.position_abs = pos;
            ui_state.window.position_rel =
                ui_state.window.position_abs / ui.with_io_dyn(|io| io.display_size()).to_raw();
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
    ///
    /// XXX: beware https://github.com/ocornut/imgui/issues/6681 ?
    fn draw_tab<'ui, U>(&mut self, ui: &mut U, tab: usize) -> Option<UiTokenDyn<'ui>>
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let prev = self.ui_state.tab.selected(tab);
        let token = {
            let label = match tab {
                #[cfg(feature = "paths-interact")]
                PathingWindowTab::INDEX_POIS => c"poiz",
                #[cfg(feature = "paths-edit")]
                PathingWindowTab::INDEX_EDIT => c"editz",
                _ => c"packz",
            };
            let flags = match self.ui_tab_pending {
                Some(i) if i == tab => {
                    self.ui_tab_pending = None;
                    match ui.imgui_version_num() {
                        #[cfg(taimi_imgui = "180")]
                        Some(im180::VERSION_NUM) => Some(im180::sys::ImGuiTabItemFlags_SetSelected),
                        #[cfg(taimi_imgui = "192")]
                        Some(im192::VERSION_NUM) => Some(im192::sys::ImGuiTabItemFlags_SetSelected),
                        _ => Default::default(),
                    }
                },
                _ => Default::default(),
            };
            ui.begin_tab_dyn(&mut { label }, None, flags)
        };

        if !prev & token.is_some() {
            if ui.window_is_appearing() {
                // first frame is wonky :<
                return None
            }
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
    pub fn draw_content<'ui, U>(
        &mut self,
        ui: &mut U,
        machine: &mut RenderMachine,
        engine: Option<&mut anyhow::Result<Engine>>,
    ) where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let mut rendered_err = if let Some(Ok(_engine)) = engine {
            None
        } else {
            Some(engine.map(|e| e.as_ref().err()))
        };
        let bookmark_tl = ui.item_rect_min();
        let bookmark_br = ui.item_rect_max();
        let draw_content = rendered_err.is_none() || machine.pack_ui_state.any_loaded();
        let tabs = draw_content.then(|| ui.tab_bar("packs")).flatten();
        for &tab_index in Self::TABS {
            let Some(_draw_tab) = tabs.as_ref().and_then(|_| self.draw_tab(ui, tab_index)) else {
                continue
            };
            if let Some(e) = rendered_err.take() {
                PathingConfig::draw_space_error(ui, machine, e.flatten());
            }
            match tab_index {
                #[cfg(feature = "paths-interact")]
                PathingWindowTab::INDEX_POIS => {
                    let act = machine.pack_ui_state.draw_interact(ui);
                    if act.navigate_packs {
                        self.ui_tab_pending = Some(PathingWindowTab::INDEX_PACKS);
                        #[cfg(deleteme)]
                        {
                            self.ui_state.tab.focus(PathingWindowTab::INDEX_PACKS);
                            self.ui_state_pending = true;
                        }
                    }
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
                    let content = ui.begin_content(c"pathing_subwindow", true);
                    if let Some(_content) = content {
                        self.draw_categories_content(ui, machine);
                    }
                },
            }
        }
        let bookmark = ui.cursor_screen_pos();
        ui.set_cursor_screen_pos([bookmark_br[0], bookmark_tl[1]]);
        match self.ui_state.tab.index() {
            #[cfg(feature = "paths-interact")]
            PathingWindowTab::INDEX_POIS =>
                if ui.button("rebuild") {
                    if let Some(..) = machine.pathing.as_ref() {
                        PathingEvent::InteractControl(InteractMessage::RequestRebuild).try_send();
                    }
                },
            #[cfg(feature = "paths-edit")]
            PathingWindowTab::INDEX_EDIT =>
                if machine.pack_ui_state.pack_edit.is_open() {
                    if ui.button("close") {
                        machine.pack_ui_state.pack_edit.close();
                    }
                },
            _ => {
                self.draw_categories_header(ui, machine);
            },
        }
        ui.set_cursor_screen_pos(bookmark);
        drop(tabs);
    }
    pub fn draw_categories_header<'ui, U>(&mut self, ui: &mut U, machine: &mut RenderMachine)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
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
                if ui.button(fl!("expand-all")) {
                    machine
                        .pack_ui_state
                        .act_expand_all(!Self::FILTER_EXPAND_COLLAPSE);
                }
            }
        }
        if machine.pack_ui_state.can_collapse() {
            if drawn {
                ui.same_line();
            }
            if ui.button(fl!("collapse-all")) {
                machine
                    .pack_ui_state
                    .act_collapse_all(!Self::FILTER_EXPAND_COLLAPSE);
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
    pub fn draw_categories_content<'ui, U>(&mut self, ui: &mut U, machine: &mut RenderMachine)
    where
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
        #[cfg(deleteme)]
        {
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
    pub fn draw_filter_content<'ui, U>(&mut self, ui: &mut U, machine: &mut RenderMachine)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
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
            machine
                .pack_ui_state
                .filter_query
                .set_flags(ui_state.filter.flags);
            match self.search_state.query_str() {
                Some(Some(query)) if !query.is_empty() && search_dirty != Some(true) => (),
                Some(query) => ui_state.search.query = query.cloned().unwrap_or_default(),
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
    #[cfg(feature = "paths-interact")]
    pub fn pois_visibility(&self) -> PackVisibility {
        PackVisibility::visible_or_pending(self.ui_state.tab.selected_pois())
    }
    #[cfg(feature = "paths-edit")]
    pub fn edit_visibility(&self) -> PackVisibility {
        PackVisibility::visible_or_pending(self.ui_state.tab.selected_edit())
    }
}
