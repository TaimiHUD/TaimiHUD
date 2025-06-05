use {
    crate::{
        engine_initialized, fl, ControllerEvent, Controller, ENGINE, SETTINGS
    }, bitflags::bitflags, nexus::imgui::{ComboBox, Id, TableColumnFlags, TableColumnSetup, TableFlags, Ui, Window}, std::sync::Arc
};

bitflags! {
    #[derive(PartialEq, Copy, Clone)]
    pub struct PathingFilterState: u8 {
        const CurrentMap = 1;
        const Enabled = 1 << 1;
        const Disabled = 1 << 2;
    }
}

impl Default for PathingFilterState {
    fn default() -> Self {
        Self::Enabled | Self::Disabled
    }
}

impl PathingFilterState {
    pub fn filter_string_to_flag(str: &str) -> Self {
        match str {
            "Enabled" => Self::Enabled,
            "Disabled" => Self::Disabled,
            "Current Map" => Self::CurrentMap,
            _ => unreachable!("no"),
        }
    }
}

pub struct PathingWindowState {
    pub open: bool,
    pub filter_open: bool,
    pub filter_state: PathingFilterState,
}

impl PathingWindowState {
    pub fn new() -> Self {
        Self {
            open: false,
            filter_open: false,
            filter_state: Default::default(),

        }
    }

    pub fn draw(&mut self, ui: &Ui) {
        let mut open = self.open;
        if let Some(settings) = SETTINGS.get().and_then(|settings| settings.try_read().ok()) {
            open = settings.pathing_window_open;
        };
        if open {
            Window::new(fl!("pathing-window"))
                .size([300.0, 200.0], nexus::imgui::Condition::FirstUseEver)
                .opened(&mut open)
                .build(ui, || {
                    if engine_initialized() {
                        ENGINE.with_borrow(|e| {
                            if let Some(Ok(engine)) = e {
                                let root = &engine.test_pack.categories.root_categories;
                                let all_categories = &engine.test_pack.categories.all_categories;
                                let filter_options = vec![
                                    "Enabled",
                                    "Disabled",
                                    "Current Map",
                                ];
                                let button_text = match self.filter_open {
                                    true => "Hide filter options",
                                    false => "Show filter options",
                                };
                                if ui.button(button_text) {
                                    self.filter_open = !self.filter_open;
                                }
                                ui.same_line();
                                if ui.button("Expand All") {
                                }
                                ui.same_line();
                                if ui.button("Collapse All") {
                                }
                                ui.dummy([4.0; 2]);
                                if self.filter_open {
                                    ui.separator();
                                    ui.dummy([4.0; 2]);
                                    ui.text("Filter Options");
                                    for filter in filter_options {
                                        ui.checkbox_flags(filter, &mut self.filter_state, PathingFilterState::filter_string_to_flag(filter));
                                    }
                                    ui.dummy([4.0; 2]);
                                    ui.separator();
                                    ui.dummy([4.0; 2]);
                                }


                                let table_flags =
                                    TableFlags::RESIZABLE | TableFlags::ROW_BG | TableFlags::BORDERS;
                                let table_name = format!("pathing");
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
                                            name: &fl!("actions"),
                                            flags: TableColumnFlags::WIDTH_FIXED,
                                            init_width_or_weight: 0.0,
                                            user_id: Id::Str("actions"),
                                        },
                                    ],
                                    table_flags,
                                );
                                ui.table_next_column();
                                for cat_name in root {
                                    all_categories[cat_name].draw(ui, all_categories);
                                }
                                if let Some(token) = table_token {
                                    token.end();
                                }
                            }
                        });
                    }
                });
        }

        if open != self.open {
            Controller::try_send(ControllerEvent::WindowState(
                crate::WINDOW_PATHING.into(),
                Some(open),
            ));
            self.open = open;
        }
    }
}

