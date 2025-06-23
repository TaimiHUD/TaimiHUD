use {
    crate::{
        fl,
        render::{
            ConfigTabState, DataSourceTabState, InfoTabState, TimerTabState, TimerWindowState,
        },
        ControllerEvent, Controller, SETTINGS,
    },
    nexus::imgui::{Ui, Window},
    std::collections::HashMap,
};

#[cfg(feature = "markers")]
use super::MarkerTabState;

pub struct PrimaryWindowState {
    pub config_tab: ConfigTabState,
    pub timer_tab: TimerTabState,
    pub data_sources_tab: DataSourceTabState,
    pub info_tab: InfoTabState,
    #[cfg(feature = "markers")]
    pub marker_tab: MarkerTabState,
    open: bool,
}

impl PrimaryWindowState {
    pub fn new() -> Self {
        Self {
            config_tab: ConfigTabState::new(),
            timer_tab: TimerTabState::new(),
            data_sources_tab: DataSourceTabState::new(),
            info_tab: InfoTabState::new(),
            #[cfg(feature = "markers")]
            marker_tab: MarkerTabState::new(),
            open: false,
        }
    }

    pub fn draw(
        &mut self,
        ui: &Ui,
        timer_window_state: &mut TimerWindowState,
        state_errors: &mut HashMap<String, anyhow::Error>,
    ) {
        let mut open = self.open;
        if let Some(settings) = SETTINGS.get().and_then(|settings| settings.try_read().ok()) {
            open = settings.primary_window_open;
        };
        if open {
            Window::new(&fl!("primary-window"))
                .size([300.0, 200.0], nexus::imgui::Condition::FirstUseEver)
                .opened(&mut open)
                .build(ui, || {
                    if let Some(_token) = ui.tab_bar("modules") {
                        if let Some(_token) = ui.tab_item(&fl!("timer-tab")) {
                            self.timer_tab.draw(ui, state_errors);
                        };
                        #[cfg(feature = "markers")]
                        {
                            if let Some(_token) = ui.tab_item(&fl!("marker-tab")) {
                                self.marker_tab.draw(ui, state_errors);
                            }
                        }
                        if let Some(_token) = ui.tab_item(&fl!("data-sources-tab")) {
                            self.data_sources_tab.draw(ui, state_errors);
                        }
                        if let Some(_token) = ui.tab_item(&fl!("config-tab")) {
                            self.config_tab.draw(ui, timer_window_state);
                        }
                        if let Some(_token) = ui.tab_item(&fl!("info-tab")) {
                            self.info_tab.draw(ui, timer_window_state);
                        }
                    }
                });
        }
        if open != self.open {
            Controller::try_send(ControllerEvent::WindowState(
                crate::WINDOW_PRIMARY.into(),
                Some(open),
            ));
            self.open = open;
        }
    }

    pub fn keybind_handler(&mut self) {
        Controller::try_send(ControllerEvent::WindowState(
            crate::WINDOW_PRIMARY.into(),
            Some(!self.open),
        ));
    }
}
