use {
    super::Alignment,
    crate::{
        controller::ControllerEvent,
        fl,
        marker::{
            atomic::{LocalPoint, MarkerInputData, ScreenPoint, SignObtainer},
            format::MarkerSet,
        },
        render::RenderState,
        settings::MarkerSettings,
        RenderEvent, Controller, SETTINGS,
    },
    glam::{Vec2, Vec3},
    indexmap::IndexMap,
    nexus::imgui::{
        ChildWindow, Condition, PopupModal, Selectable, TableColumnSetup, TableFlags, TreeNode,
        TreeNodeFlags, Ui, WindowFlags,
    },
    std::{
        collections::{HashMap, HashSet},
        sync::Arc,
    },
};

pub struct MarkerTabState {
    markers: IndexMap<String, Vec<Arc<MarkerSet>>>,
    pub marker_selection: Option<Arc<MarkerSet>>,
    category_status: HashSet<String>,
    formatted_name: String,
}

impl MarkerTabState {
    pub fn new() -> Self {
        MarkerInputData::create();

        Self {
            markers: Default::default(),
            marker_selection: Default::default(),
            category_status: Default::default(),
            formatted_name: Default::default(),
        }
    }

    pub fn draw(&mut self, ui: &Ui, state_errors: &mut HashMap<String, anyhow::Error>) {
        ui.columns(2, "marker_tab_start", true);
        self.draw_sidebar(ui, state_errors);
        ui.next_column();
        self.draw_main(ui);
        ui.columns(1, "marker_tab_end", false)
    }

    fn draw_sidebar(&mut self, ui: &Ui, state_errors: &mut HashMap<String, anyhow::Error>) {
        self.draw_sidebar_header(ui, state_errors);
        self.draw_sidebar_child(ui);
    }
    fn draw_sidebar_header(&mut self, ui: &Ui, state_errors: &mut HashMap<String, anyhow::Error>) {
        let markers_dir = crate::ADDON_DIR.join("markers");
        let markers_dir = markers_dir.to_string_lossy().to_string();
        RenderState::draw_open_button(
            state_errors,
            ui,
            fl!("open-button", kind = "folder"),
            markers_dir,
        );
        ui.same_line();
        #[cfg(feature = "markers-edit")]
        if ui.button(fl!("marker-set-create")) {
            RenderState::try_send(RenderEvent::OpenEditMarkers(None));
        }
        ui.same_line();
        if ui.button(fl!("reload-markers")) {
            Controller::try_send(ControllerEvent::ReloadMarkers);
        }
        #[allow(clippy::collapsible_if)]
        if self.category_status.len() != self.markers.keys().len() {
            if ui.button(&fl!("expand-all")) {
                self.category_status.extend(self.markers.keys().cloned());
            }
        }
        if self.category_status.len() != self.markers.keys().len()
            && !self.category_status.is_empty()
        {
            ui.same_line();
        }
        #[allow(clippy::collapsible_if)]
        if !self.category_status.is_empty() {
            if ui.button(&fl!("collapse-all")) {
                self.category_status.clear();
            }
        }
    }
    fn draw_sidebar_child(&mut self, ui: &Ui) {
        let child_window_flags = WindowFlags::HORIZONTAL_SCROLLBAR;
        ChildWindow::new("marker_sidebar")
            .flags(child_window_flags)
            .size([0.0, 0.0])
            .build(ui, || {
                let header_flags = TreeNodeFlags::FRAMED;
                let height = Vec2::from_array(ui.calc_text_size("U\nI"));
                let height = height.y;
                // interface design is my passion
                for idx in 0..self.markers.len() {
                    self.draw_category(ui, header_flags, idx, height);
                }
            });
    }

    fn draw_category(&mut self, ui: &Ui, header_flags: TreeNodeFlags, idx: usize, height: f32) {
        let (category_name, category) = self
            .markers
            .get_index(idx)
            .expect("given an incorrect index for the category");
        let category_closure = || {
            ui.dummy([0.0, 4.0]);
            for marker in category {
                let mut selected = false;
                if let Some(selected_marker) = &self.marker_selection {
                    selected = Arc::ptr_eq(selected_marker, marker);
                }
                let element_selected =
                    Self::draw_marker_set_in_sidebar(ui, marker, selected, height);
                if element_selected && element_selected != selected {
                    self.marker_selection = Some(marker.clone());
                }
            }
        };
        let tree_node = TreeNode::new(category_name)
            .flags(header_flags)
            .opened(
                self.category_status.contains(category_name),
                Condition::Always,
            )
            .tree_push_on_open(false)
            .build(ui, category_closure);
        match tree_node {
            Some(_) => {
                self.category_status.insert(category_name.to_string());
            }
            None => {
                self.category_status.remove(category_name);
            }
        }
    }

    fn draw_marker_set_in_sidebar(
        ui: &Ui,
        marker: &Arc<MarkerSet>,
        selected_in: bool,
        height: f32,
    ) -> bool {
        let mut selected = selected_in;
        let widget_pos = Vec2::from(ui.cursor_pos());
        let window_size = Vec2::from(ui.window_content_region_max());
        let widget_size = window_size.with_y(height);
        let group_token = ui.begin_group();
        if Selectable::new(&marker.combined())
            .selected(selected)
            .build(ui)
        {
            selected = true;
        }
        if let Some(settings) = SETTINGS.get().and_then(|settings| settings.try_read().ok()) {
            let settings_for_marker = settings.markers.get(&marker.id());
            ui.same_line();
            let (color, text) = match settings_for_marker {
                Some(MarkerSettings { disabled: true, .. }) => ([1.0, 0.0, 0.0, 1.0], "Disabled"),
                _ => ([0.0, 1.0, 0.0, 1.0], "Enabled"),
            };
            let text_size = Vec2::from(ui.calc_text_size(text));
            Alignment::set_cursor(
                ui,
                Alignment::RIGHT_MIDDLE,
                widget_pos,
                widget_size,
                text_size,
            );
            ui.text_colored(color, text);
        }
        ui.dummy([0.0, 4.0]);
        group_token.end();
        selected
    }

    fn draw_main(&mut self, ui: &Ui) {
        let child_window_flags = WindowFlags::HORIZONTAL_SCROLLBAR;
        ChildWindow::new("timer_main")
            .flags(child_window_flags)
            .size([0.0, 0.0])
            .build(ui, || {
                ui.text_wrapped(&fl!("experimental-notice"));
                ui.dummy([4.0; 2]);
                ui.separator();
                ui.dummy([4.0; 2]);
                let mid = MarkerInputData::read();
                if let Some(mid) = &mid {
                    let sign = mid.sign_obtainer.sign();
                    let meep = SignObtainer::meters_per_feet();
                    let sign_unity = sign / meep;
                    let sign_x = format!("{:.2}", sign.x);
                    let sign_y = format!("{:.2}", sign.y);
                    ui.text_wrapped(&fl!("current-scaling-factor", x = sign_x, y = sign_y));
                    let sign_unity_x = format!("{:.2}", sign_unity.x);
                    let sign_unity_y = format!("{:.2}", sign_unity.y);
                    ui.text_wrapped(&fl!(
                        "current-scaling-factor-multiple",
                        x = sign_unity_x,
                        y = sign_unity_y
                    ));
                    if ui.button(&fl!("scaling-factor-reset")) {
                        MarkerInputData::reset_signobtainer();
                    }
                    ui.dummy([4.0; 2]);
                    ui.separator();
                    ui.dummy([4.0; 2]);
                }
                if let Some(selected_marker_set) = &self.marker_selection {
                    let pushy = ui.push_id(&selected_marker_set.name);
                    RenderState::font_text("big", ui, &selected_marker_set.name);
                    if let Some(author) = &selected_marker_set.author {
                        RenderState::font_text("ui", ui, &fl!("author-arg", author = author));
                    }
                    if let Some(path) = &selected_marker_set.path {
                        let path_display = format!("{}", path.display());
                        ui.text_wrapped(&fl!("location", path = path_display));
                    }
                    RenderState::font_text("ui", ui, &selected_marker_set.description);
                    ui.text(&fl!("map-id-arg", id = selected_marker_set.map_id));
                    ui.text(&fl!(
                        "markers-arg",
                        count = selected_marker_set.markers.len()
                    ));
                    #[cfg(feature = "markers-edit")]
                    if ui.button(fl!("marker-set-edit")) {
                        let raw_inner =
                            Arc::<MarkerSet>::unwrap_or_clone(selected_marker_set.clone());
                        RenderState::try_send(RenderEvent::OpenEditMarkers(Some(raw_inner)));
                    }
                    ui.same_line();
                    // TODO: add confirm ^^;
                    #[cfg(feature = "markers-edit")]
                    if selected_marker_set.idx.is_some() && selected_marker_set.path.is_some() {
                        if ui.button(&fl!("marker-set-delete")) {
                            self.formatted_name =
                                fl!("delete-item", item = selected_marker_set.name.clone());
                            ui.open_popup(&self.formatted_name);
                        }
                    }
                    #[cfg(feature = "markers-edit")]
                    if let Some(_token) = PopupModal::new(&self.formatted_name)
                        .always_auto_resize(true)
                        .begin_popup(ui)
                    {
                        ui.text_colored([1.0, 0.0, 0.0, 1.0], fl!("delete-markerset-warning"));
                        if ui.button(fl!("delete")) {
                            Controller::try_send(ControllerEvent::DeleteMarker {
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
                            if let Some(mid) = &mid {
                                let position: LocalPoint = Vec3::from(x.position.clone()).into();
                                let map = mid.map_local_to_map(position);

                                mid.map_map_to_screen(map)
                            } else {
                                None
                            }
                        })
                        .collect();
                    ui.dummy([4.0; 2]);
                    let table_flags =
                        TableFlags::RESIZABLE | TableFlags::ROW_BG | TableFlags::BORDERS;
                    let table_name = format!("markers_for_{}", selected_marker_set.name);
                    let table_token = ui.begin_table_header_with_flags(
                        &table_name,
                        [
                            TableColumnSetup::new(&fl!("marker-type")),
                            TableColumnSetup::new(&fl!("description")),
                            TableColumnSetup::new(&fl!("local-header")),
                            TableColumnSetup::new(&fl!("map-header")),
                            TableColumnSetup::new(&fl!("screen-header")),
                        ],
                        table_flags,
                    );
                    ui.table_next_column();
                    for marker in &selected_marker_set.markers {
                        // marker marker on the table
                        marker.marker.icon(ui);
                        ui.table_next_column();
                        if let Some(description) = &marker.id {
                            if !description.is_empty() {
                                ui.text_wrapped(description);
                            } else {
                                ui.text_wrapped(&fl!("not-applicable"));
                            }
                        } else {
                            ui.text_wrapped(&fl!("not-applicable"));
                        }
                        ui.table_next_column();
                        let position: LocalPoint = Vec3::from(marker.position.clone()).into();
                        ui.text_wrapped(format!(
                            "({:.2}, {:.2}, {:.2})",
                            position.x, position.y, position.z
                        ));
                        ui.table_next_column();
                        if let Some(mid) = &mid {
                            let map_position = mid.map_local_to_map(position);
                            ui.text_wrapped(format!(
                                "({:.2}, {:.2})",
                                map_position.x, map_position.y
                            ));
                            ui.table_next_column();
                            if let Some(screen_position) = mid.map_map_to_screen(map_position) {
                                ui.text_wrapped(format!(
                                    "({:.2}, {:.2})",
                                    screen_position.x, screen_position.y
                                ));
                            } else {
                                ui.text_wrapped(&fl!("marker-not-on-screen"));
                            }
                            ui.table_next_column();
                        } else {
                            ui.text_wrapped(&fl!("not-applicable"));
                            ui.table_next_column();
                            ui.text_wrapped(&fl!("not-applicable"));
                            ui.table_next_column();
                        }
                    }
                    if let Some(token) = table_token {
                        token.end();
                    }
                    ui.dummy([4.0; 2]);
                    let button_text = match selected_marker_set.status() {
                        true => fl!("autoplacement-disable"),
                        false => fl!("autoplacement-enable"),
                    };
                    if ui.button(button_text) {
                        Controller::try_send(ControllerEvent::MarkerToggle(selected_marker_set.id()));
                    }
                    ui.dummy([4.0; 2]);
                    if ui.button(&fl!("markers-place")) {
                        Controller::try_send(ControllerEvent::SetMarker(selected_marker_set.clone()));
                    }
                    pushy.pop();
                } else {
                    ui.text(&fl!("select-a-marker"));
                }
            });
    }
    pub fn marker_update(&mut self, markers: HashMap<String, Vec<Arc<MarkerSet>>>) {
        self.markers.clear();
        for (category, markers) in markers {
            self.markers.insert(category, markers);
        }
        self.markers.sort_keys();
    }
}
