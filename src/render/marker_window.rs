use {
    crate::{
        fl,
        marker::{atomic::MarkerInputData, format::MarkerSet},
        ControllerEvent, Controller, SETTINGS,
    },
    nexus::imgui::{Id, TableColumnFlags, TableColumnSetup, TableFlags, Ui, Window},
    std::sync::Arc,
};

pub struct MarkerWindowState {
    pub open: bool,
    pub markers_for_map: Vec<Arc<MarkerSet>>,
}

impl MarkerWindowState {
    pub fn new() -> Self {
        Self {
            markers_for_map: Default::default(),
            open: false,
        }
    }

    pub fn new_map_markers(&mut self, markers: Vec<Arc<MarkerSet>>) {
        self.markers_for_map = markers;
    }

    pub fn draw(&mut self, ui: &Ui) {
        let mut open = self.open;
        if let Some(settings) = SETTINGS.get().and_then(|settings| settings.try_read().ok()) {
            open = settings.markers_window_open;
        };
        if open {
            Window::new(fl!("marker-window"))
                .size([300.0, 200.0], nexus::imgui::Condition::FirstUseEver)
                .opened(&mut open)
                .build(ui, || {
                    if ui.button(&fl!("clear-markers")) {
                        Controller::try_send(ControllerEvent::ClearMarkers);
                    }
                    ui.same_line();
                    if ui.button(&fl!("clear-spent-autoplace")) {
                        Controller::try_send(ControllerEvent::ClearSpentAutoplace);
                    }
                    let mid = MarkerInputData::read();
                    if !self.markers_for_map.is_empty() {
                        let table_flags =
                            TableFlags::RESIZABLE | TableFlags::ROW_BG | TableFlags::BORDERS;
                        let table_name = format!("markers_for_map");
                        let table_token = ui.begin_table_header_with_flags(
                            &table_name,
                            [
                                TableColumnSetup {
                                    name: &fl!("name"),
                                    flags: TableColumnFlags::WIDTH_STRETCH,
                                    init_width_or_weight: 0.0,
                                    user_id: Id::Str("name"),
                                },
                                TableColumnSetup {
                                    name: &fl!("category"),
                                    flags: TableColumnFlags::WIDTH_STRETCH,
                                    init_width_or_weight: 0.0,
                                    user_id: Id::Str("category"),
                                },
                                TableColumnSetup {
                                    name: &fl!("description"),
                                    flags: TableColumnFlags::WIDTH_STRETCH,
                                    init_width_or_weight: 0.0,
                                    user_id: Id::Str("description"),
                                },
                                TableColumnSetup {
                                    name: &fl!("actions"),
                                    flags: TableColumnFlags::WIDTH_STRETCH,
                                    init_width_or_weight: 0.0,
                                    user_id: Id::Str("actions"),
                                },
                            ],
                            table_flags,
                        );
                        ui.table_next_column();
                        for marker in &self.markers_for_map {
                            let id_token = ui.push_id(&format!(
                                "{}{:?}{:?}",
                                marker.name, marker.author, marker.category
                            ));
                            ui.text(format!("{}", marker.name));
                            ui.table_next_column();
                            if let Some(category) = &marker.category {
                                ui.text(format!("{}", category));
                            } else {
                                ui.text("");
                            }
                            ui.table_next_column();
                            ui.text_wrapped(format!("{}", marker.description));
                            ui.table_next_column();
                            if ui.button(&fl!("markers-place")) {
                                Controller::try_send(ControllerEvent::SetMarker(marker.clone()));
                            }
                            ui.table_next_column();
                            id_token.end();
                        }
                        if let Some(token) = table_token {
                            token.end();
                        }
                    } else {
                        ui.text_wrapped(fl!("no-markers-for-map"));
                    }
                });
        }

        if open != self.open {
            Controller::try_send(ControllerEvent::WindowState(
                crate::WINDOW_MARKERS.into(),
                Some(open),
            ));
            self.open = open;
        }
    }
}
