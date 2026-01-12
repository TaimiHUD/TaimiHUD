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
            None
        } else {
            Some(engine.map(|e| e.as_ref().err()))
        };
        let bookmark_tl = ui.item_rect_min();
        let bookmark_br = ui.item_rect_max();
        let draw_content = rendered_err.is_none() || machine.pack_ui_state.any_loaded();
        let tabs = draw_content.then(|| ui.tab_bar("packs")).flatten();
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
    pub fn draw_categories_header<'ui, U>(&mut self, ui: &mut U, machine: &mut RenderMachine)
    where
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
                    machine.pack_ui_state.act_expand_all();
                }
            }
        }
        if machine.pack_ui_state.can_collapse() {
            if drawn {
                ui.same_line();
            }
            if ui.button(fl!("collapse-all")) {
                machine.pack_ui_state.act_collapse_all();
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
    pub fn draw_filter_content<'ui, U>(&mut self, ui: &mut U, machine: &mut RenderMachine)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
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

use {
    crate::{
        controller::pathing::{
            info::EMPTY_INTERACTION_ATTRS,
            registry::{LoadedPoiPath, PoiMapPath},
            shared::{interact::InteractMessage, LocDisplay, SharedGameplayMap},
        },
        render::element::pack::interact::RenderInteractivePoi,
    },
    std::borrow::Cow,
    taimi_hoard::loc::LocationRef,
    taimi_meta::packs::{CategoryIndex, CategoryPath, PoiIndex, PoiPath},
};
impl PathingWindowState {
    pub fn draw_interact_content(&mut self, ui: &Ui, machine: &mut RenderMachine) {
        let Some(pathing) = machine.pathing.as_ref() else { return };
        let table = RenderInteractivePoi::draw_table_start(ui, "pois-nearby");
        if let Some(_table) = table {
            let nearby = pathing.interact.nearby.borrow().clone();
            let maps = pathing.gameplay.borrow().clone();
            for (lpath, path) in nearby.iter_pois() {
                self.draw_one_poi(ui, &maps, lpath, Some(path));
            }
        }
        if let Some(_table) = RenderInteractivePoi::draw_table_start(ui, "pois-map") {
            let maps = pathing.gameplay.borrow().clone();
            let entities = pathing.interact.entities.borrow();
            // TODO: use bvh to sort by distance bleh
            let bvh = &entities.trigger_bvh;
            for e in entities.entities.iter() {
                let e = &e.value;
                let lpath = e.poi_path();
                self.draw_one_poi(ui, &maps, lpath, None);
            }
        }
        let _ = RenderInteractivePoi::draw_table_start(ui, "pois-hidden");
    }
    fn draw_one_poi(
        &mut self,
        ui: &Ui,
        maps: &SharedGameplayMap,
        lpath: PoiMapPath,
        mut poi_path: Option<PoiPath>,
    ) {
        let lpoi_path: LoadedPoiPath = lpath.unscope();
        let (attrs, lguid, name, category_path, lcat_path) =
            if let Some((_map_path, map_info)) = maps.get_info_for(lpath.root.root) {
                let linfo = map_info.pois().lookup_ref(&lpoi_path);
                // find is incorrect!
                let TODO = ();
                // TODO: map_info.marker_guid(lpath);
                if poi_path.is_none() {
                    poi_path = map_info.poi_path(lpoi_path);
                }
                let lguid = map_info.poi_guids().find(|(p, ..)| Some(*p) == poi_path);
                let cat_path: Option<CategoryPath> = linfo.map(|i| i.category_path);
                let lcat_path = cat_path.and_then(|p| map_info.category_index(p));
                let attrs = linfo.map(|li| li.interaction_attrs());
                let name = linfo.and_then(|li| li.get_marker_attrs()).and_then(|ma| {
                    ma.tip_name
                        .as_ref()
                        .or(ma.tip_description.as_ref())
                        .map(|name| &name[..])
                });
                (attrs, lguid, name, cat_path, lcat_path)
            } else {
                (None, None, None, None, None)
            };
        let (position, visibility, category_visibility) = if let Some(map) = maps.get_state(lpath.root) {
            let lpoi = map.pois().lookup_ref(&lpoi_path);
            let vis = lpoi.map(|lpoi| lpoi.visibility);
            let lcat = lcat_path.and_then(|p| map.categories().lookup_ref(&p));
            let cat_vis = lcat.map(|lcat| lcat.visibility);
            let pos = lpoi.map(|lpoi| lpoi.position);
            (pos, vis, cat_vis)
        } else {
            (None, None, None)
        };
        let guid = lguid.and_then(|(_, g)| g.cloned());
        let display_name = name.map(Cow::Borrowed).unwrap_or_else(|| {
            let n = LocDisplay(lpath.root.rel(lpoi_path));
            Cow::Owned(n.to_string())
        });
        let attrs = attrs.unwrap_or(&EMPTY_INTERACTION_ATTRS);
        let context_opened = RenderInteractivePoi {
            path: poi_path.unwrap_or(PoiPath::with_path(PoiIndex::MAX)),
            category_path: category_path.unwrap_or(CategoryPath::with_path(CategoryIndex::MAX)),
            map_path: lpath.root,
            loaded_index: lpath.path,
            guid,
            visibility: visibility.unwrap_or_default(),
            category_visibility: category_visibility.unwrap_or_default(),
            #[cfg(todo)]
            hidden: visibility.map(|v| v.is_visible() ^ config_vis).unwrap_or(false),
            hidden: false,
            position: Default::default(),
            nearby: Default::default(),
        }
        .draw(ui, attrs, &display_name);
    }
}
