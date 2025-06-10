use {
    super::TimerWindowState,
    crate::{
        controller::ProgressBarStyleChange,
        fl,
        render::TextFont,
        settings::{MarkerAutoPlaceSettings, SquadCondition},
        ControllerEvent, Controller, SETTINGS,
    },
    nexus::imgui::{ComboBox, Condition, Selectable, Slider, TreeNode, TreeNodeFlags, Ui},
    strum::IntoEnumIterator,
};

pub struct ConfigTabState {
    pub katrender: bool,
    pub marker_autoplace: MarkerAutoPlaceSettings,
    pub marker_autoplace_inner: Option<SquadCondition>,
}

impl ConfigTabState {
    pub fn new() -> Self {
        Self {
            katrender: false,
            marker_autoplace: Default::default(),
            marker_autoplace_inner: Default::default(),
        }
    }

    pub fn draw(&mut self, ui: &Ui, timer_window_state: &mut TimerWindowState) {
        if let Some(settings) = SETTINGS.get().and_then(|settings| settings.try_read().ok()) {
            self.katrender = settings.enable_katrender;
        };
        ui.text_wrapped(&fl!("imgui-notice"));
        ui.dummy([4.0, 4.0]);
        ui.text_wrapped(&fl!("keybind-triggers"));
        ui.dummy([4.0, 4.0]);
        #[cfg(feature = "space")]
        if ui.checkbox("Experimental KatRender", &mut self.katrender) {
            Controller::try_send(ControllerEvent::ToggleKatRender);
        };
        #[cfg(feature = "goggles")]
        if self.katrender {
            crate::render::goggles::options_ui(ui);
        }
        let markers_window_closure = || {
            if let Some(settings) = SETTINGS.get().and_then(|settings| settings.try_read().ok()) {
                self.marker_autoplace = settings.marker_autoplace.clone();
                self.marker_autoplace_inner = match &self.marker_autoplace {
                    MarkerAutoPlaceSettings::OpenWindow(t) => Some(t.clone()),
                    MarkerAutoPlaceSettings::Place(t) => Some(t.clone()),
                    _ => None,
                };
            }
            ui.dummy([4.0, 4.0]);
            ui.text_wrapped(&fl!("autoplace-warning"));
            ui.dummy([4.0, 4.0]);
            let autoplace_closure = || {
                let mut selected = None;
                for autoplace in MarkerAutoPlaceSettings::iter() {
                    if Selectable::new(autoplace.to_string())
                        .selected(autoplace == self.marker_autoplace)
                        .build(ui)
                    {
                        selected = Some(autoplace);
                    }
                }
                selected
            };
            if let Some(Some(selection)) = ComboBox::new(&fl!("marker-trigger"))
                .preview_value(&self.marker_autoplace.to_string())
                .build(ui, autoplace_closure)
            {
                self.marker_autoplace = selection;
                Controller::try_send(ControllerEvent::MarkerAutoPlaceSettings(
                    self.marker_autoplace.clone(),
                ));
            }
            if let Some(inner) = &self.marker_autoplace_inner {
                let autoplace_inner_closure = || {
                    let mut selected = None;
                    for autoplace_inner in SquadCondition::iter() {
                        if Selectable::new(autoplace_inner.to_string())
                            .selected(autoplace_inner == *inner)
                            .build(ui)
                        {
                            selected = Some(autoplace_inner);
                        }
                    }
                    selected
                };
                if let Some(Some(selection)) = ComboBox::new(&fl!("marker-condition"))
                    .preview_value(inner.to_string())
                    .build(ui, autoplace_inner_closure)
                {
                    match &mut self.marker_autoplace {
                        MarkerAutoPlaceSettings::OpenWindow(ref mut t) => {
                            *t = selection.clone();
                        }
                        MarkerAutoPlaceSettings::Place(ref mut t) => {
                            *t = selection.clone();
                        }
                        _ => (),
                    };
                    Controller::try_send(ControllerEvent::MarkerAutoPlaceSettings(
                        self.marker_autoplace.clone(),
                    ));
                }
            }
        };
        let timers_window_closure = || {
            ui.dummy([4.0, 4.0]);
            if let Some(settings) = SETTINGS.get().and_then(|settings| settings.try_read().ok()) {
                timer_window_state.progress_bar.stock = settings.progress_bar.stock;
            };
            if ui.checkbox(
                &fl!("stock-imgui-progress-bar"),
                &mut timer_window_state.progress_bar.stock,
            ) {
                Controller::try_send(ControllerEvent::ProgressBarStyle(
                    ProgressBarStyleChange::Stock(timer_window_state.progress_bar.stock),
                ));
            };
            if ui.checkbox(&fl!("shadow"), &mut timer_window_state.progress_bar.shadow) {
                Controller::try_send(ControllerEvent::ProgressBarStyle(
                    ProgressBarStyleChange::Shadow(timer_window_state.progress_bar.shadow),
                ));
            }
            if ui.checkbox(
                &fl!("centre-text-after-icon"),
                &mut timer_window_state.progress_bar.centre_after,
            ) {
                Controller::try_send(ControllerEvent::ProgressBarStyle(
                    ProgressBarStyleChange::Centre(timer_window_state.progress_bar.centre_after),
                ));
            }
            if Slider::new(&fl!("height"), 8.0, 256.0)
                .display_format("%.0f")
                .build(ui, &mut timer_window_state.progress_bar.height)
            {
                Controller::try_send(ControllerEvent::ProgressBarStyle(
                    ProgressBarStyleChange::Height(timer_window_state.progress_bar.height),
                ));
            }
            let font_closure = || {
                let mut selected = timer_window_state.progress_bar.font.clone();
                for font in TextFont::iter() {
                    if Selectable::new(font.to_string())
                        .selected(font == selected)
                        .build(ui)
                    {
                        Controller::try_send(ControllerEvent::ProgressBarStyle(
                            ProgressBarStyleChange::Font(font.clone()),
                        ));
                        selected = font;
                    }
                }
                selected
            };
            if let Some(_selection) = ComboBox::new(&fl!("font"))
                .preview_value(&timer_window_state.progress_bar.font.to_string())
                .build(ui, font_closure)
            {}
        };
        let _timers_window = TreeNode::new(&fl!("timer-window"))
            .flags(TreeNodeFlags::FRAMED)
            .opened(true, Condition::Once)
            .tree_push_on_open(true)
            .build(ui, timers_window_closure);
        let _markers_window = TreeNode::new(&fl!("marker-window"))
            .flags(TreeNodeFlags::FRAMED)
            .opened(true, Condition::Once)
            .tree_push_on_open(true)
            .build(ui, markers_window_closure);
    }
}
