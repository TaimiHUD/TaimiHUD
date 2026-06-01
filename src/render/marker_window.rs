use {
    crate::{
        marker::format::MarkerSet,
        render::element::prelude::*,
        settings::Settings,
        Controller,
        ControllerEvent,
        MarkersController,
        MarkersEvent,
    },
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

    pub fn draw<'ui, U>(&mut self, ui: &mut U)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let mut open = self.open;
        if let Some(settings) = Settings::try_read() {
            open = settings.markers_window_open;
        };
        if open {
            let window = with_i18n!("marker-window", |label| ui.begin_taimi_window(
                "marker-window",
                label,
                ImCondition::initial(ImSize2::new(300.0, 200.0)),
                &mut open,
            ));
            if let Some(_window) = window {
                if ui.button(fl!("clear-markers")) {
                    MarkersController::try_send(MarkersEvent::ClearMarkers);
                }
                ui.same_line();
                if ui.button(fl!("clear-spent-autoplace")) {
                    MarkersController::try_send(MarkersEvent::ClearSpentAutoplace);
                }
                let cols = ["name", "category", "description", "actions"];
                let table_flags = match self.markers_for_map.is_empty() {
                    true => {
                        with_i18n!("no-markers-for-map", |msg| ui.text_wrapped(msg));
                        None
                    },
                    false => Some(match ui.imgui_version_num() {
                        #[cfg(taimi_imgui = "180")]
                        Some(im180::VERSION_NUM) => (
                            imw::DynArgsTable::new(Some(imw::Table::IM180_FLAGS_PRESET)),
                            Some(imw::TableColumn::IM180_WIDTH_STRETCH),
                        ),
                        #[cfg(taimi_imgui = "192")]
                        Some(im192::VERSION_NUM) => (
                            imw::DynArgsTable::new(Some(imw::Table::IM192_FLAGS_PRESET)),
                            Some(imw::TableColumn::IM192_WIDTH_STRETCH),
                        ),
                        _ => (Default::default(), None),
                    }),
                };
                let table_token = table_flags.and_then(|(flags, column_flags)| {
                    ui.begin_table_with_flags(c"markers_for_map", cols.len(), flags)
                        .map(|token| (token, column_flags))
                });
                if let Some((_table, column_flags)) = table_token {
                    for id in cols {
                        let user_id = 0;
                        with_i18n(id, |label| {
                            ui.table_column_setup_untyped(Some(label), column_flags, None, user_id)
                        });
                    }
                    ui.table_header_row();
                    ui.table_next_column();
                    for marker in &self.markers_for_map {
                        let id_token = ui.push_id_hash((&marker.name, &marker.author, &marker.category));
                        ui.text(&marker.name);
                        ui.table_next_column();
                        if let Some(category) = &marker.category {
                            ui.text(category);
                        } else {
                            ui.text("");
                        }
                        ui.table_next_column();
                        ui.text_wrapped(&marker.description);
                        ui.table_next_column();
                        if ui.button(fl!("markers-place")) {
                            MarkersController::try_send(MarkersEvent::SetMarker(marker.clone()));
                        }
                        ui.table_next_column();
                        id_token.end();
                    }
                }
            }
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
