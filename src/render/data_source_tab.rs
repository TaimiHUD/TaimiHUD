use {
    crate::{
        controller::ControllerEvent,
        fl,
        render::RenderState,
        settings::{NeedsUpdate, RemoteState, Source},
        Controller, SETTINGS,
    },
    nexus::imgui::{PopupModal, StyleColor, TableColumnSetup, TableFlags, Ui},
    std::collections::HashMap,
};

pub struct DataSourceTabState {
    pub checking_for_updates: bool,
}

impl DataSourceTabState {
    pub fn new() -> Self {
        Self {
            checking_for_updates: false,
        }
    }

    pub fn draw_uninstall(&self, ui: &Ui, rs: &RemoteState) {
        let source_text = &rs.source.source().repo_string();
        let modal_name = fl!("addon-uninstall-modal-title", source = source_text);
        if ui.button(&fl!("addon-uninstall-modal-button")) {
            ui.open_popup(&modal_name);
        }
        if ui.is_item_hovered() {
            if let Some(path) = &rs.installed_path {
                let path_string = format!("{}", &path.display());
                ui.tooltip_text(fl!("location", path = path_string));
            }
        }
        if let Some(_token) = PopupModal::new(&modal_name)
            .always_auto_resize(true)
            .begin_popup(ui)
        {
            ui.text_wrapped(fl!("addon-uninstall-modal-title"));
            ui.dummy([4.0, 4.0]);
            if let Some(path) = &rs.installed_path {
                let path_string = format!("{}", &path.display());
                ui.text(&fl!("location", path = path_string));
            }
            ui.dummy([4.0, 4.0]);
            let token = ui.push_style_color(StyleColor::Text, [1.0, 0.0, 0.0, 1.0]);
            ui.text(fl!("addon-uninstall-modal-description"));
            token.pop();
            ui.dummy([4.0, 4.0]);
            if ui.button(fl!("addon-uninstall-modal-button")) {
                Controller::try_send(ControllerEvent::UninstallAddon(rs.source.clone()));
                ui.close_current_popup();
            }
            ui.same_line();
            if ui.button(fl!("close")) {
                ui.close_current_popup();
            }
        }
    }

    pub fn draw(&mut self, ui: &Ui, state_errors: &mut HashMap<String, anyhow::Error>) {
        if let Some(settings) = SETTINGS.get().and_then(|settings| settings.try_read().ok()) {
            if self.checking_for_updates {
                ui.text(fl!("checking-for-updates"))
            } else {
                if ui.button(fl!("check-for-updates")) {
                    Controller::try_send(ControllerEvent::CheckDataSourceUpdates);
                }
                if ui.is_item_hovered() {
                    ui.tooltip_text(fl!("check-for-updates-tooltip"));
                }
                ui.same_line();
                if ui.button(fl!("reload-data-sources")) {
                    Controller::try_send(ControllerEvent::ReloadData);
                }
                if ui.is_item_hovered() {
                    ui.tooltip_text(fl!("reload-data-sources-tooltip"));
                }
                ui.same_line();
                if let Some(last_checked) = &settings.last_checked {
                    let time_display = last_checked.format("%F %T %Z").to_string();
                    ui.text(fl!("checked-for-updates-last", time = time_display));
                } else {
                    ui.text(fl!("checked-for-updates-last", time = "Never"));
                }
                ui.dummy([8.0, 8.0]);
                let table_flags = TableFlags::RESIZABLE | TableFlags::ROW_BG | TableFlags::BORDERS;
                let table_token = ui.begin_table_header_with_flags(
                    "remotes",
                    [
                        TableColumnSetup::new(fl!("remote")),
                        TableColumnSetup::new(fl!("description")),
                        TableColumnSetup::new(fl!("update-status")),
                        TableColumnSetup::new(fl!("actions")),
                    ],
                    table_flags,
                );
                ui.table_next_column();
                for download_data in &settings.remotes {
                    let source_arc = download_data.source.clone();
                    let source = source_arc.source();
                    let source_text = source.to_string();
                    let pushy = ui.push_id(&source_text);
                    ui.text(format!("{}", source));
                    ui.table_next_column();
                    if let Some(description) = &source.description {
                        ui.text_wrapped(description);
                    } else {
                        ui.text_wrapped(fl!("no-description"));
                    }
                    ui.table_next_column();
                    if let Some(installed) = &download_data.installed_tag {
                        ui.text_wrapped(fl!("version-installed", version = installed));
                    } else {
                        ui.text_wrapped(fl!("version-not-installed"));
                    }
                    download_data.needs_update.draw(ui);
                    ui.table_next_column();
                    use NeedsUpdate::*;
                    let button_text = match &download_data.needs_update {
                        Unknown => Some(fl!("attempt-update")),
                        Known(true, _id) => Some(fl!("update")),
                        Known(false, _id) => None,
                        Error(_err) => None,
                    };
                    if let Some(button_text) = button_text {
                        if ui.button(button_text) {
                            Controller::try_send(ControllerEvent::DoDataSourceUpdate {
                                source: source_arc,
                            });
                        }
                    }
                    RenderState::draw_open_button(
                        state_errors,
                        ui,
                        fl!("open-button", kind = "repository"),
                        source.view_url(),
                    );
                    if let Some(path) = &download_data.installed_path {
                        if let Some(path) = path.to_str() {
                            RenderState::draw_open_button(
                                state_errors,
                                ui,
                                fl!("open-button", kind = "folder"),
                                path.to_string(),
                            );
                        }
                        self.draw_uninstall(ui, download_data);
                    }

                    ui.table_next_column();
                    pushy.pop();
                }
                drop(table_token);
            }
        } else {
            ui.text(fl!("settings-unloaded"));
        }
    }
}
