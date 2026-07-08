use {
    crate::{
        fl,
        render::{
            element::prelude::*,
            machine::{RenderMachine, RenderSlot},
            ConfigTabState,
            DataSourceTabState,
            InfoTabState,
            TimerTabState,
            TimerWindowState,
        },
        settings::{state::AddonHostName, Settings},
        Controller,
        ControllerEvent,
    },
    std::collections::HashMap,
};

#[cfg(feature = "markers")]
use super::MarkerTabState;
#[cfg(feature = "markers")]
use super::PathingConfig;
#[cfg(feature = "api")]
use crate::render::api_tab::ApiTabState;
#[cfg(feature = "scripts")]
use crate::render::plug::{PlugConfig, PlugConfigCache, PlugConfigDesc, PlugConfigState};

pub struct PrimaryWindowState {
    pub config_tab: ConfigTabState,
    #[cfg(feature = "api")]
    pub api_tab: ApiTabState,
    pub timer_tab: TimerTabState,
    pub data_sources_tab: DataSourceTabState,
    pub info_tab: InfoTabState,
    #[cfg(feature = "markers")]
    pub marker_tab: MarkerTabState,
    #[cfg(feature = "space")]
    pub pathing_tab: PathingConfig,
    #[cfg(feature = "extension-arcdps")]
    pub arc_tab: super::ArcRenderState,
    /// TODO: deleteme
    pub(super) open: bool,
    pub(super) state: elem::window::WindowState,
    pub(super) scratch: elem::window::WindowScratch,
    #[cfg(feature = "scripts")]
    pub(crate) plug_state: PlugConfigState,
    #[cfg(feature = "scripts")]
    pub(super) plug_scratch: PlugConfigCache,
}

impl PrimaryWindowState {
    pub fn new() -> Self {
        Self {
            config_tab: ConfigTabState::new(),
            #[cfg(feature = "api")]
            api_tab: ApiTabState::new(),
            timer_tab: TimerTabState::new(),
            data_sources_tab: DataSourceTabState::new(),
            info_tab: InfoTabState::new(),
            #[cfg(feature = "markers")]
            marker_tab: MarkerTabState::new(),
            #[cfg(feature = "space")]
            pathing_tab: PathingConfig::new(),
            #[cfg(feature = "extension-arcdps")]
            arc_tab: super::ArcRenderState::new(),
            open: false,
            state: Default::default(),
            scratch: Default::default(),
            #[cfg(feature = "scripts")]
            plug_state: Default::default(),
            #[cfg(feature = "scripts")]
            plug_scratch: Default::default(),
        }
    }

    pub fn draw<'ui, U, C>(
        &mut self,
        ui: &mut U,
        context: &mut C,
        machine: &mut RenderMachine,
        slot: RenderSlot,
        timer_window_state: &mut TimerWindowState,
        state_errors: &mut HashMap<String, anyhow::Error>,
    ) where
        U: ?Sized + ImDrawWindow<'ui>,
        C: ?Sized + DrawContext<'ui>,
    {
        let desc = elem::window::WindowDesc {
            id: cstr!(0"primary-window"),
            size: Some(ImSize2::new(300.0, 200.0)),
            ..Default::default()
        };
        if let Some(settings) = Settings::try_read() {
            if settings.primary_window_open != self.state.open_state() {
                self.state.set_state(settings.primary_window_open);
            }
        }
        let mut state = self.state.clone();
        let mut scratch = self.scratch.clone();
        let mut draw = elem::window::WindowDraw {
            desc: &desc,
            state: &mut state,
            scratch: &mut scratch,
        };
        if let Some(mut window) = draw.begin_draw(ui, context) {
            self.draw_tabs(
                ui,
                &mut window.context,
                None,
                machine,
                slot,
                timer_window_state,
                state_errors,
                true,
            )
        }
        self.state = state;
        self.scratch = scratch;
        if self.state.was_closed() {
            Controller::try_send(ControllerEvent::WindowState(
                crate::WINDOW_PRIMARY.into(),
                Some(false),
            ));
        }
    }

    pub fn keybind_handler(&mut self) {
        Controller::try_send(ControllerEvent::WindowState(
            crate::WINDOW_PRIMARY.into(),
            Some(!self.open),
        ));
    }

    pub fn draw_tabs<'ui, U, C>(
        &mut self,
        ui: &mut U,
        context: &'_ mut C,
        host: Option<AddonHostName>,
        machine: &mut RenderMachine,
        slot: RenderSlot,
        timer_window_state: &mut TimerWindowState,
        state_errors: &mut HashMap<String, anyhow::Error>,
        standalone: bool,
    ) where
        U: ?Sized + ImDrawWindow<'ui>,
        C: ?Sized + DrawContext<'ui>,
    {
        let Some(_tabs) = ui.tab_bar(if standalone { "modules" } else { "modules-settings" }) else {
            return
        };
        if standalone {
            if let Some(_token) = ui.tab_item(fl!("timer-tab")) {
                self.timer_tab.draw(ui, state_errors);
            };
            #[cfg(feature = "markers")]
            if let Some(_token) = ui.tab_item(fl!("marker-tab")) {
                self.marker_tab.draw(ui, machine, state_errors);
            }
            #[cfg(feature = "space")]
            if let Some(_token) = ui.tab_item(fl!("pathing-tab")) {
                self.pathing_tab.draw(ui, machine, state_errors);
            }
            if let Some(_token) = ui.tab_item(fl!("data-sources-tab")) {
                self.data_sources_tab.draw(ui, state_errors);
            }
            #[cfg(feature = "scripts")]
            let scripts = self
                .plug_state
                .applicable
                .then(|| ui.tab_item("scripts"))
                .flatten();
            #[cfg(feature = "scripts")]
            if let Some(_token) = scripts {
                PlugConfig {
                    desc: &PlugConfigDesc { ..Default::default() },
                    state: &mut self.plug_state,
                    scratch: &mut self.plug_scratch,
                }
                .draw_on_window(ui, context);
            }
            #[cfg(feature = "api")]
            if let Some(_token) = ui.tab_item(fl!("api-tab")) {
                self.api_tab.draw(ui, state_errors);
            }
            if let Some(_token) = ui.tab_item(fl!("config-tab")) {
                self.config_tab.draw(ui, machine, timer_window_state);
            }
            if let Some(_token) = ui.tab_item(fl!("info-tab")) {
                self.info_tab.draw(ui, timer_window_state, slot);
            } else {
                self.info_tab.regen_authors();
            }
        } else {
            if let Some(_token) = ui.tab_item(fl!("config-tab")) {
                self.config_tab.draw(ui, machine, timer_window_state);
            }
            if let Some(_token) = ui.tab_item(fl!("data-sources-tab")) {
                self.data_sources_tab.draw(ui, state_errors);
            }
            #[cfg(feature = "scripts")]
            let scripts = self
                .plug_state
                .applicable
                .then(|| ui.tab_item("scripts"))
                .flatten();
            #[cfg(feature = "scripts")]
            if let Some(_token) = scripts {
                PlugConfig {
                    desc: &PlugConfigDesc { ..Default::default() },
                    state: &mut self.plug_state,
                    scratch: &mut self.plug_scratch,
                }
                .draw_on_window(ui, context);
            }
            #[cfg(feature = "api")]
            if let Some(_token) = ui.tab_item(fl!("api-tab")) {
                self.api_tab.draw(ui, state_errors);
            }
            #[cfg(feature = "space")]
            if let Some(_token) = ui.tab_item(fl!("pathing-tab")) {
                self.pathing_tab.draw(ui, machine, state_errors);
            }
            #[cfg(feature = "extension-arcdps")]
            if let Some(_token) = ui.tab_item(fl!("arcdps-tab")) {
                // only relevant when drawing embedded...
                let host = host.unwrap_or(AddonHostName::All);
                self.arc_tab.ui_options(ui, context, host);
            }
            if let Some(_token) = ui.tab_item(fl!("info-tab")) {
                self.info_tab.draw(ui, timer_window_state, slot);
            }
        }
    }
}
