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
}

use {
    crate::render::element::pack::interact::{RenderInteractivePoi, DrawMenuPoi, DrawPoiInfo},
    crate::controller::pathing::{
        registry::{LoadedPoiPath, PoiMapPath},
        info::EMPTY_INTERACTION_ATTRS,
        shared::{interact::InteractMessage, SharedGameplayMap, LocDisplay},
    },
    taimi_meta::packs::{
        CategoryIndex, PoiIndex, CategoryPath, PoiPath,
    },
    taimi_hoard::loc::LocationRef,
    taimi_pack::attributes::keys::Guid,
    std::borrow::Cow,
    glamour::{Box2, Point2, Size2},
    std::cell::Cell,
};
thread_local! {
    static POI_CONTEXT_OPEN: Cell<bool> = Cell::new(false);
    static POI_CONTEXT: Cell<Option<(PoiPath, PoiMapPath, Option<Guid>)>> = Cell::new(None);
    static POI_DELAY: Cell<Option<f32>> = Cell::new(None);
}
impl PathingWindowState {
    /// TODO
    pub fn pois_visible(&self, machine: &RenderMachine) -> bool {
        self.open
    }

    pub fn draw_interact_content(&mut self, ui: &Ui, machine: &mut RenderMachine) {
        let mut draw = DrawPoiInfo {
            ui,
            state: &mut machine.pack_ui_state.interact,
            pack_state: &mut machine.pack_ui_state.pack_state,
        };
        let was_context = draw.state.context.is_some();
        draw.draw();
        let context_id = "poi-context";
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
        if popup_drawn && draw.state.context.is_some() {
            if !was_context {
                ui.open_popup(context_id);
            }
        } else if popup_drawn != was_context {
            draw.state.context = None;
        }
    }
    #[cfg(deleteme)]
    pub fn draw_interact_content(&mut self, ui: &Ui, machine: &mut RenderMachine) {
        let Some(pathing) = machine.pathing.as_ref() else { return };

        let mut draw = DrawPoiInfo {
            ui,
            state: &mut machine.pack_ui_state.interact,
            pack_state: &mut machine.pack_ui_state.pack_state,
        };
        return draw.draw();
        POI_CONTEXT_OPEN.set(false);

        let table = RenderInteractivePoi::draw_table_start(ui, "pois-nearby");
        if let Some(_table) = table {
            let nearby = pathing.interact.nearby.borrow().clone();
            let maps = pathing.gameplay.borrow().clone();
            for (lpath, path) in nearby.iter_pois() {
                self.draw_one_poi(ui, Some(&*machine), &maps, lpath, Some(path));
                ui.table_next_column();
            }
        }
        let bounds: Box2<f32> = Box2::new(
            Point2::from_array(ui.window_content_region_min()),
            Point2::from_array(ui.window_content_region_max()),
        );
        #[cfg(todo = "unused")]
        let start_pos: Point2<f32> = Point2::from_array(ui.cursor_start_pos());
        let window_size: Size2<f32> = Size2::from_array(ui.window_size());
        let bounds_height = (bounds.max.y - bounds.min.y).max(window_size.height) + ui.text_line_height_with_spacing() * 2.0;
        if let Some(_table) = RenderInteractivePoi::draw_table_start(ui, "pois-map") {
            let maps = pathing.gameplay.borrow().clone();
            let entities = pathing.interact.entities.borrow();
            // TODO: use bvh to sort by distance bleh
            let bvh = &entities.trigger_bvh;
            for e in entities.entities.iter() {
                let e = &e.value;
                let lpath = e.poi_path();
                let lpoi_path: LoadedPoiPath = lpath.unscope();
                let mut poi_path = None;
                let mut guid = None;

                let _id = ui.push_id(imgui::Id::Int((lpath.root.root.path as i32).rotate_left(20) ^ lpath.root.path.get() as i32 ^ lpath.path as i32));
                let pos: Point2<f32> = Point2::from_array(ui.cursor_pos());
                let offset = pos.y + bounds.min.y;
                let is_visible = offset >= 0.0 && offset <= bounds_height;
                let map_info = is_visible.then(|| maps.get_info_for(lpath.root.root)).flatten();
                let linfo = map_info.as_ref().and_then(|(_, i)| i.pois().lookup_ref(&lpoi_path));
                let stor;
                let display_name = match is_visible {
                    true => {
                        let pd;
                        let mut idx = -(lpoi_path.path as i64);
                        let mut name_or_desc = None;
                        let mut cat_path = None;
                        if let (Some(linfo), Some((_map_path, map_info))) = (linfo, map_info) {
                            if poi_path.is_none() {
                                poi_path = map_info.poi_path(lpoi_path);
                                if let Some(pp) = poi_path {
                                    idx = pp.path as i64;
                                }
                            }
                            guid = map_info.poi_guid_by_index(lpoi_path);
                            cat_path = Some(linfo.category_path);
                            name_or_desc = linfo.get_marker_attrs().and_then(|attrs| attrs.tip_name().or(attrs.tip_description()));
                        }
                        let name = if let Some(name) = name_or_desc {
                            Ok(Cow::Borrowed(name))
                        } else if let Some(pack) = machine.pack_ui_state.pack_state.lookup_ref(&lpath.root.root) {
                            pd = pack.state.pack_data();
                            let cat = pd.as_ref().and_then(|pd| cat_path.and_then(|path|
                                pd.categories.all_categories.get_index(path.path as usize)
                            ));
                            if let Some((_, cat)) = cat {
                                Err(Some(Cow::Borrowed(cat.display_name())))
                            } else if let Some(info) = &pack.state.info.info {
                                Err(Some(Cow::Owned(info.to_string())))
                            } else {
                                Err(Some(Cow::Owned(pack.state.info.to_string())))
                            }
                        } else {
                            Err(None)
                        };

                        stor = match name {
                            Ok(name) => name,
                            Err(Some(pack_name)) => {
                                Cow::Owned(format!("{pack_name}#{}", idx))
                            },
                            Err(None) =>
                                Cow::Owned(format!("{}#{}", lpath.root.root, idx)),
                        };
                        &stor[..]
                    },
                    false => "POI",
                };
                let node = TreeNode::new("poi")
                    .flags(TreeNodeFlags::SPAN_FULL_WIDTH)
                    .label::<&str, _>(display_name)
                    .tree_push_on_open(false)
                    .opened(false, Condition::Appearing)
                    .allow_item_overlap(true)
                    .leaf(false);
                let node = node.push(ui);
                let mut right_clicked = ui.is_item_clicked_with_button(MouseButton::Right);
                if let Some(_token) = node {
                    self.draw_one_poi(ui, None, &maps, lpath, None);
                } else {
                    ui.table_next_column();
                }
                right_clicked |= ui.is_item_clicked_with_button(MouseButton::Right);
                ui.table_next_column();
                drop(_id);
                let right_clicked = right_clicked.then(|| {
                    poi_path.map(|path| (path, lpath, guid.cloned()))
                });
                if let Some(Some(paths)) = right_clicked {
                    POI_CONTEXT_OPEN.set(true);
                    POI_CONTEXT.set(Some(paths));
                    POI_DELAY.set(None);
                    ui.open_popup("poi-context");
                }
            }
        }
        let _ = RenderInteractivePoi::draw_table_start(ui, "pois-hidden");
        if POI_CONTEXT_OPEN.get() {
            ui.open_popup("poi-context");
        }
        ui.popup("poi-context", || {
            let Some((path, loaded_path, guid)) = POI_CONTEXT.get() else { return };
            let mut menu = DrawMenuPoi {
                ui,
                hidden: false,
                act_trigger: None,
                act_untrigger: false,
                act_selected_poi_delay: POI_DELAY.get(),
            };
            menu.draw();
            POI_DELAY.set(menu.act_selected_poi_delay);
            menu.action_trigger(path, loaded_path, guid.as_ref());
        });
    }
    fn draw_one_poi(&mut self, ui: &Ui, machine: Option<&RenderMachine>, maps: &SharedGameplayMap, lpath: PoiMapPath, mut poi_path: Option<PoiPath>) {
        let lpoi_path: LoadedPoiPath = lpath.unscope();
        let (attrs, lguid, name, category_path, lcat_path) = if let Some((_map_path, map_info)) = maps.get_info_for(lpath.root.root) {
            let linfo = map_info.pois().lookup_ref(&lpoi_path);
            if poi_path.is_none() {
                poi_path = map_info.poi_path(lpoi_path);
            }
            let lguid = map_info.poi_guid_by_index(lpoi_path);
            let cat_path: Option<CategoryPath> = linfo.map(|i| i.category_path);
            let lcat_path = cat_path.and_then(|p| map_info.category_index(p));
            let attrs = linfo.map(|li| li.interaction_attrs());
            let name = linfo.and_then(|li| li.get_marker_attrs())
                .and_then(|ma| ma.tip_name.as_ref()
                    .or(ma.tip_description.as_ref())
                    .map(|name| &name[..])
                );
            (attrs, lguid, name, cat_path, lcat_path)
        } else { (None, None, None, None, None) };
        let (position, visibility, category_visibility) = if let Some(map) = maps.get_state(lpath.root) {
            let lpoi = map.pois().lookup_ref(&lpoi_path);
            let vis = lpoi.map(|lpoi| lpoi.visibility);
            let lcat = lcat_path.and_then(|p| map.categories().lookup_ref(&p));
            let cat_vis = lcat.map(|lcat| lcat.visibility);
            let pos = lpoi.map(|lpoi| lpoi.position);
            (pos, vis, cat_vis)
        } else { (None, None, None) };
        let pd;
        let display_name = match name.map(Cow::Borrowed) {
            Some(name) => Some(name),
            None => if let Some(pack) = machine.and_then(|m| m.pack_ui_state.pack_state.lookup_ref(&lpath.root.root)) {
                pd = pack.state.pack_data();
                let cat = pd.as_ref().and_then(|pd| category_path.and_then(|path|
                    pd.categories.all_categories.get_index(path.path as usize)
                ));
                Some(if let Some((_, cat)) = cat {
                    Cow::Borrowed(cat.display_name())
                } else if let Some(info) = &pack.state.info.info {
                    Cow::Owned(info.to_string())
                } else {
                    Cow::Owned(pack.state.info.to_string())
                })
            } else { None },
        }.unwrap_or_else(|| {
            let n = LocDisplay(lpath.root.rel(lpoi_path));
            Cow::Owned(n.to_string())
        });
        let attrs = attrs.unwrap_or(&EMPTY_INTERACTION_ATTRS);
        let path = poi_path.unwrap_or(PoiPath::with_path(PoiIndex::MAX));
        let context_opened = RenderInteractivePoi {
            path,
            category_path: category_path.unwrap_or(CategoryPath::with_path(CategoryIndex::MAX)),
            map_path: lpath.root,
            loaded_index: lpath.path,
            guid: lguid.cloned(),
            visibility: visibility.unwrap_or_default(),
            category_visibility: category_visibility.unwrap_or_default(),
            #[cfg(todo)]
            hidden: visibility.map(|v| v.is_visible() ^ config_vis).unwrap_or(false),
            hidden: false,
            position: Default::default(),
            nearby: Default::default(),
        }.draw(ui, attrs, &display_name);
        if context_opened {
            log::debug!("open popup?");
            if let Some(path) = poi_path {
                log::debug!("ya open popup!");
                POI_CONTEXT_OPEN.set(true);
                POI_CONTEXT.set(Some((path, lpath, lguid.cloned())));
                POI_DELAY.set(None);
                ui.open_popup("poi-context");
            }
        }
    }
}
