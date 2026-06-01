use {
    super::Alignment,
    crate::{
        marker::format::MarkerSet,
        render::{element::prelude::*, machine::RenderMachine, RenderState},
        settings::{MarkerSettings, Settings},
        MarkersController,
        MarkersEvent,
        RenderEvent,
    },
    glamour::TransformMap,
    indexmap::IndexMap,
    std::{
        collections::{HashMap, HashSet},
        sync::Arc,
    },
    taimi_meta::coords::{LocalPoint, LocalSpace, MapLocalScale, ScreenPoint},
};

pub struct MarkerTabState {
    markers: IndexMap<String, Vec<Arc<MarkerSet>>>,
    pub marker_selection: Option<Arc<MarkerSet>>,
    category_status: HashSet<String>,
    formatted_name: Option<String>,
}

impl MarkerTabState {
    pub fn new() -> Self {
        Self {
            markers: Default::default(),
            marker_selection: Default::default(),
            category_status: Default::default(),
            formatted_name: Default::default(),
        }
    }

    pub fn draw<'ui, U>(
        &mut self,
        ui: &mut U,
        machine: &mut RenderMachine,
        state_errors: &mut HashMap<String, anyhow::Error>,
    ) where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        ui.columns(2, "marker_tab_start", true);
        self.draw_sidebar(ui, state_errors);
        ui.next_column();
        self.draw_main(ui, machine);
        ui.columns(1, "marker_tab_end", false)
    }

    fn draw_sidebar<'ui, U>(&mut self, ui: &mut U, state_errors: &mut HashMap<String, anyhow::Error>)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        self.draw_sidebar_header(ui, state_errors);
        self.draw_sidebar_child(ui);
    }
    fn draw_sidebar_header<'ui, U>(
        &mut self,
        ui: &mut U,
        _state_errors: &mut HashMap<String, anyhow::Error>,
    ) where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let markers_dir = crate::ADDON_DIR.join("markers");
        RenderState::draw_open_path_button(ui, fl!("open-button", kind = "folder"), &markers_dir);
        ui.same_line();
        #[cfg(feature = "markers-edit")]
        if ui.button(fl!("marker-set-create")) {
            RenderState::try_send(RenderEvent::OpenEditMarkers(None));
        }
        ui.same_line();
        if ui.button(fl!("reload-markers")) {
            MarkersController::try_send(MarkersEvent::ReloadMarkers);
        }
        #[allow(clippy::collapsible_if)]
        if self.category_status.len() != self.markers.keys().len() {
            if ui.button(fl!("expand-all")) {
                self.category_status.extend(self.markers.keys().cloned());
            }
        }
        if self.category_status.len() != self.markers.keys().len() && !self.category_status.is_empty() {
            ui.same_line();
        }
        #[allow(clippy::collapsible_if)]
        if !self.category_status.is_empty() {
            if ui.button(fl!("collapse-all")) {
                self.category_status.clear();
            }
        }
    }
    fn draw_sidebar_child<'ui, U>(&mut self, ui: &mut U)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        if let Some(_token) = ui.begin_sidebar(c"marker_sidebar") {
            let ImSize2 { height, .. } = ui.calc_text_size("U\nI");
            // interface design is my passion
            for idx in 0..self.markers.len() {
                self.draw_category(ui, idx, height);
            }
        }
    }

    fn draw_category<'ui, U>(&mut self, ui: &mut U, idx: usize, height: f32)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let (category_name, category) = self
            .markers
            .get_index(idx)
            .expect("given an incorrect index for the category");
        let tree_node = ui.begin_sidebar_tree_node(
            ImCondition::always(self.category_status.contains(category_name)),
            idx,
            category_name,
        );
        if let Some(_tree_token) = tree_node {
            ui.dummy([0.0, 4.0]);
            for marker in category {
                let mut selected = false;
                if let Some(selected_marker) = &self.marker_selection {
                    selected = Arc::ptr_eq(selected_marker, marker);
                }
                let element_selected = Self::draw_marker_set_in_sidebar(ui, marker, selected, height);
                if element_selected && element_selected != selected {
                    self.marker_selection = Some(marker.clone());
                }
            }
            self.category_status.insert(category_name.to_string());
        } else {
            self.category_status.remove(category_name);
        }
    }

    fn draw_marker_set_in_sidebar<'ui, U>(
        ui: &mut U,
        marker: &Arc<MarkerSet>,
        selected_in: bool,
        height: f32,
    ) -> bool
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let mut selected = selected_in;
        let widget_pos = ui.cursor_pos();
        let window_size = ui.window_region_size();
        let widget_size = window_size.with_y(height);
        let group_token = ui.begin_group();
        if ui.selectable(marker.combined(), selected) {
            selected = true;
        }
        if let Some(settings) = Settings::try_read() {
            let settings_for_marker = settings.markers.get(&marker.id());
            ui.same_line();
            let (color, text) = match settings_for_marker {
                Some(MarkerSettings { disabled: true, .. }) => ([1.0, 0.0, 0.0, 1.0], "Disabled"),
                _ => ([0.0, 1.0, 0.0, 1.0], "Enabled"),
            };
            let text_size = ui.calc_text_size(text);
            Alignment::set_cursor(ui, Alignment::RIGHT_MIDDLE, widget_pos, widget_size, text_size);
            ui.text_colored(color, text);
        }
        ui.dummy([0.0, 4.0]);
        group_token.end();
        selected
    }

    fn draw_main<'ui, U>(&mut self, ui: &mut U, machine: &mut RenderMachine)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let Some(_main_token) = ui.begin_mainbar(c"timer_main") else { return };
        ui.text_wrapped(fl!("experimental-notice"));
        ui.dummy([4.0; 2]);
        ui.separator();
        ui.dummy([4.0; 2]);
        if !machine.map.is_empty() {
            let sign = machine.map.calibration.local_space().scale;
            let meep = MapLocalScale::METRES_PER_FEET;
            let sign_unity = sign / meep;
            let sign_x = format!("{:.2}", sign.x);
            let sign_y = format!("{:.2}", sign.y);
            ui.text_wrapped(fl!("current-scaling-factor", x = &sign_x, y = &sign_y));
            let sign_unity_x = format!("{:.2}", sign_unity.x);
            let sign_unity_y = format!("{:.2}", sign_unity.y);
            ui.text_wrapped(fl!(
                "current-scaling-factor-multiple",
                x = &sign_unity_x,
                y = &sign_unity_y
            ));
            if ui.button(fl!("scaling-factor-reset")) {
                machine.map_sign.clear();
                machine.map.calibration.local_space = None;
            }
            ui.dummy([4.0; 2]);
            ui.separator();
            ui.dummy([4.0; 2]);
        }
        if let Some(selected_marker_set) = &self.marker_selection {
            let _pushy = ui.push_id(&selected_marker_set.name);
            ui.text_with_font(NexusLinkFont::Big, &selected_marker_set.name);
            if let Some(author) = &selected_marker_set.author {
                ui.text_with_font(NexusLinkFont::Ui, fl!("author-arg", author = author));
            }
            if let Some(path) = &selected_marker_set.path {
                let path_display = format!("{}", path.display());
                ui.text_wrapped(fl!("location", path = &path_display));
            }
            ui.text_with_font(NexusLinkFont::Ui, &selected_marker_set.description);
            ui.text(fl!("map-id-arg", id = selected_marker_set.map_id));
            ui.text(fl!("markers-arg", count = selected_marker_set.markers.len()));
            #[cfg(feature = "markers-edit")]
            if ui.button(fl!("marker-set-edit")) {
                let raw_inner = Arc::<MarkerSet>::unwrap_or_clone(selected_marker_set.clone());
                RenderState::try_send(RenderEvent::OpenEditMarkers(Some(raw_inner)));
            }
            ui.same_line();
            // TODO: add confirm ^^;
            #[cfg(feature = "markers-edit")]
            if selected_marker_set.idx.is_some() && selected_marker_set.path.is_some() {
                if ui.button(fl!("marker-set-delete")) {
                    let name = self
                        .formatted_name
                        .insert(fl!("delete-item", item = selected_marker_set.name.clone()).into());
                    ui.open_popup(&*name);
                }
            }
            let modal = self
                .formatted_name
                .as_ref()
                .map(|name| ui.begin_popup_modal(name, Default::default(), None));
            #[cfg(feature = "markers-edit")]
            if let Some(_token) = modal {
                ui.text_colored([1.0, 0.0, 0.0, 1.0], fl!("delete-markerset-warning"));
                if ui.button(fl!("delete")) {
                    MarkersController::try_send(MarkersEvent::DeleteMarker {
                        path: selected_marker_set.path.clone().unwrap(),
                        category: selected_marker_set.category.clone(),
                        idx: selected_marker_set.idx.unwrap(),
                    });
                }
                ui.same_line();
                if ui.button(fl!("cancel")) {
                    ui.close_current_popup();
                }
            }
            let screen_positions: Vec<ScreenPoint> = selected_marker_set
                .markers
                .iter()
                .flat_map(|x| {
                    if let Some(map) = machine.map.get() {
                        let position = LocalSpace::to2(x.position.into());
                        let global = machine.map.calibration.map(position);

                        let context = map.context;
                        map.clip_screen(
                            map.map_to_worldmap_for(context)
                                .then(map.worldmap_to_fake_for(context))
                                .then(map.calibration.to_screen())
                                .map(global),
                        )
                    } else {
                        None
                    }
                })
                .collect();
            ui.dummy([4.0; 2]);
            let cols = [
                "marker-type",
                "description",
                "local-header",
                "map-header",
                "screen-header",
            ];
            let table_flags = match ui.imgui_version_num() {
                #[cfg(taimi_imgui = "180")]
                Some(im180::VERSION_NUM) => imw::Table::IM180_ARGS_PRESET,
                #[cfg(taimi_imgui = "192")]
                Some(im192::VERSION_NUM) => imw::Table::IM192_ARGS_PRESET,
                _ => Default::default(),
            };
            let Some(table_token) = ui.begin_table_with_flags(
                format_args!("markers_for_{}", selected_marker_set.name),
                cols.len(),
                table_flags,
            ) else {
                return
            };
            for id in cols {
                let user_id = 0;
                with_i18n!(id, |label| ui.table_column_setup_untyped(
                    Some(label),
                    Default::default(),
                    None,
                    user_id
                ));
            }
            ui.table_header_row();
            ui.table_next_column();
            for marker in &selected_marker_set.markers {
                // marker marker on the table
                marker.marker.icon(ui);
                ui.table_next_column();
                if let Some(description) = &marker.id {
                    if !description.is_empty() {
                        ui.text_wrapped(description);
                    } else {
                        ui.text_wrapped(fl!("not-applicable"));
                    }
                } else {
                    ui.text_wrapped(fl!("not-applicable"));
                }
                ui.table_next_column();
                let position: LocalPoint = marker.position.into();
                ui.text_wrapped(im_fmt!(
                    "({:.2}, {:.2}, {:.2})",
                    position.x,
                    position.y,
                    position.z
                ));
                ui.table_next_column();
                if let Some(map) = machine.map.get() {
                    let map_position = map.calibration.map(LocalSpace::to2(position));
                    ui.text_wrapped(im_fmt!("({:.2}, {:.2})", map_position.x, map_position.y));
                    ui.table_next_column();
                    let trans = map
                        .map_to_worldmap_for(map.context)
                        .then(map.worldmap_to_fake_for(map.context));
                    if let Some(take_position) = map.clip(trans.map(map_position)) {
                        let screen_position = map.calibration.map(map_position);
                        ui.text_wrapped(im_fmt!("({:.2}, {:.2})", screen_position.x, screen_position.y));
                    } else {
                        ui.text_wrapped(fl!("marker-not-on-screen"));
                    }
                    ui.table_next_column();
                } else {
                    ui.text_wrapped(fl!("not-applicable"));
                    ui.table_next_column();
                    ui.text_wrapped(fl!("not-applicable"));
                    ui.table_next_column();
                }
            }
            table_token.end();
            ui.dummy([4.0; 2]);
            let button_text = match selected_marker_set.status() {
                true => fl!("autoplacement-disable"),
                false => fl!("autoplacement-enable"),
            };
            if ui.button(button_text) {
                MarkersController::try_send(MarkersEvent::MarkerToggle(selected_marker_set.id()));
            }
            ui.dummy([4.0; 2]);
            if ui.button(fl!("markers-place")) {
                MarkersController::try_send(MarkersEvent::SetMarker(selected_marker_set.clone()));
            }
        } else {
            ui.text(fl!("select-a-marker"));
        }
    }
    pub fn marker_update(&mut self, markers: HashMap<String, Vec<Arc<MarkerSet>>>) {
        self.markers.clear();
        for (category, markers) in markers {
            self.markers.insert(category, markers);
        }
        self.markers.sort_keys();
    }
}
