use {
    crate::{
        fl,
        render::{
            machine::RenderMachine,
            ConfigTabState, DataSourceTabState, InfoTabState, TimerTabState, TimerWindowState,
        },
        settings::Settings,
        ControllerEvent, Controller,
    },
    nexus::imgui::{Ui, Window},
    std::collections::HashMap,
};

#[cfg(feature = "markers")]
use super::MarkerTabState;
#[cfg(feature = "markers")]
use super::PathingConfig;

pub struct PrimaryWindowState {
    pub config_tab: ConfigTabState,
    pub timer_tab: TimerTabState,
    pub data_sources_tab: DataSourceTabState,
    pub info_tab: InfoTabState,
    #[cfg(feature = "markers")]
    pub marker_tab: MarkerTabState,
    #[cfg(feature = "space")]
    pub pathing_tab: PathingConfig,
    #[cfg(feature = "extension-arcdps")]
    pub arc_tab: super::ArcRenderState,
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
            #[cfg(feature = "space")]
            pathing_tab: PathingConfig::new(),
            #[cfg(feature = "extension-arcdps")]
            arc_tab: Default::default(),
            open: false,
        }
    }

    pub fn draw(
        &mut self,
        ui: &Ui,
        machine: &mut RenderMachine,
        timer_window_state: &mut TimerWindowState,
        state_errors: &mut HashMap<String, anyhow::Error>,
    ) {
        let mut open = self.open;
        if let Some(settings) = Settings::try_read() {
            open = settings.primary_window_open;
        };
        if open {
            Window::new(&fl!("primary-window"))
                .size([300.0, 200.0], nexus::imgui::Condition::FirstUseEver)
                .opened(&mut open)
                .build(ui, || self.draw_tabs(ui, machine, timer_window_state, state_errors, true));
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

    pub fn draw_tabs(
        &mut self,
        ui: &Ui,
        machine: &mut RenderMachine,
        timer_window_state: &mut TimerWindowState,
        state_errors: &mut HashMap<String, anyhow::Error>,
        standalone: bool,
    ) {
        if let Some(_token) = ui.tab_bar("modules") {
            if let Some(_token) = ui.tab_item(&fl!("timer-tab")) {
                self.timer_tab.draw(ui, state_errors);
            };
            #[cfg(feature = "markers")]
            if let Some(_token) = ui.tab_item(&fl!("marker-tab")) {
                self.marker_tab.draw(ui, machine, state_errors);
            }
            #[cfg(feature = "space")]
            if let Some(_token) = ui.tab_item(&fl!("pathing-tab")) {
                self.pathing_tab.draw(ui, machine, state_errors);
            }
            if let Some(_token) = ui.tab_item(&fl!("data-sources-tab")) {
                self.data_sources_tab.draw(ui, state_errors);
            }
            if let Some(_token) = ui.tab_item(&fl!("config-tab")) {
                self.config_tab.draw(ui, machine, timer_window_state);
            }
            if let Some(_token) = ui.tab_item(&fl!("info-tab")) {
                self.info_tab.draw(ui, timer_window_state);
            }
            if !standalone {
                #[cfg(feature = "extension-arcdps")]
                if let Some(_token) = ui.tab_item(&fl!("arcdps-tab")) {
                    self.arc_tab.ui_options(ui);
                }
            }
        }
    }
}
