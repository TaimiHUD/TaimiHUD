use {
    super::RenderState,
    crate::{
        control_window,
        fl,
        settings::ProgressBarSettings,
        timer::{PhaseState, TimerAlert, TimerFile},
        ControllerEvent, CONTROLLER_SENDER, SETTINGS, WINDOW_TIMERS,
    },
    glam::Vec2,
    nexus::imgui::{ProgressBar, StyleColor, Ui, Window},
    std::sync::Arc,
};

pub struct TimerWindowState {
    pub open: bool,
    pub progress_bar: ProgressBarSettings,
    pub phase_states: Vec<PhaseState>,
}

impl TimerWindowState {
    pub fn new() -> Self {
        Self {
            open: false,
            progress_bar: Default::default(),
            phase_states: Default::default(),
        }
    }

    pub fn draw(&mut self, ui: &Ui) {
        let mut open = self.open;
        if let Some(settings) = SETTINGS.get().and_then(|settings| settings.try_read().ok()) {
            open = settings.timers_window_open;
            self.progress_bar = settings.progress_bar.clone();
        };
        if open {
            Window::new(fl!("timer-window"))
                .size([300.0, 200.0], nexus::imgui::Condition::FirstUseEver)
                .opened(&mut open)
                .build(ui, || {
                    if !self.phase_states.is_empty() {
                        if ui.button(fl!("reset-timers")) {
                            let sender = CONTROLLER_SENDER.get().unwrap();
                            let event_send = sender.try_send(ControllerEvent::TimerReset);
                            drop(event_send);
                            self.reset_phases();
                        }
                        ui.dummy([2.0; 2]);
                        ui.separator();
                        ui.dummy([4.0; 2]);
                    } else {
                        ui.text_wrapped(&fl!("no-phases-active"));
                    }
                    for ps in &self.phase_states {
                        for alert in ps.alerts.iter() {
                            if self.progress_bar.stock {
                                Self::stock_progress_bar(&self.progress_bar, alert, ui, ps);
                            } else {
                                Self::progress_bar(&self.progress_bar, alert, ui, ps);
                            }
                        }
                    }
                });
        }

        if open != self.open {
            control_window(WINDOW_TIMERS, Some(open));
            self.open = open;
        }
    }

    fn progress_bar(settings: &ProgressBarSettings, alert: &TimerAlert, ui: &Ui, ps: &PhaseState) {
        let start = ps.start;
        let height = settings.height;
        if let Some(percent) = alert.percentage(start) {
            let mut widget_pos: Vec2 = Vec2::new(0.0, 0.0);
            if !settings.centre_after {
                widget_pos = Vec2::from(ui.cursor_pos());
            }
            RenderState::icon(
                ui,
                Some(height),
                alert.icon.as_ref(),
                ps.timer.path.as_ref(),
            );
            if settings.centre_after {
                widget_pos = Vec2::from(ui.cursor_pos());
            }
            let mut colour_tokens = Vec::new();
            if let Some(fill_colour) = alert.fill_colour {
                colour_tokens
                    .push(ui.push_style_color(StyleColor::PlotHistogram, fill_colour.imgcolor()));
            }
            if let Some(colour) = alert.colour {
                colour_tokens.push(ui.push_style_color(StyleColor::Text, colour.imgcolor()));
            }
            ProgressBar::new(percent)
                .size([-1.0, height])
                .overlay_text("")
                .build(ui);
            let window_size = Vec2::from(ui.window_size());
            let widget_size = window_size.with_y(height);
            let text = alert.progress_bar_text(start);
            RenderState::offset_font_text(
                &settings.font.to_string(),
                ui,
                widget_pos,
                widget_size,
                settings.shadow,
                &text,
            );
            ui.dummy([0.0, height / 4.0]);
            for token in colour_tokens {
                token.pop();
            }
        }
    }

    fn stock_progress_bar(
        settings: &ProgressBarSettings,
        alert: &TimerAlert,
        ui: &Ui,
        ps: &PhaseState,
    ) {
        let start = ps.start;
        let height = settings.height;
        if let Some(percent) = alert.percentage(start) {
            RenderState::icon(
                ui,
                Some(height),
                alert.icon.as_ref(),
                ps.timer.path.as_ref(),
            );
            let mut colour_tokens = Vec::new();
            if let Some(fill_colour) = alert.fill_colour {
                colour_tokens
                    .push(ui.push_style_color(StyleColor::PlotHistogram, fill_colour.imgcolor()));
            }
            if let Some(colour) = alert.colour {
                colour_tokens.push(ui.push_style_color(StyleColor::Text, colour.imgcolor()));
            }
            ProgressBar::new(percent)
                .size([-1.0, height])
                .overlay_text(alert.progress_bar_text(start))
                .build(ui);
            for token in colour_tokens {
                token.pop();
            }
        }
    }

    pub fn new_phase(&mut self, phase_state: PhaseState) {
        self.phase_states.push(phase_state);
    }
    pub fn remove_phase(&mut self, timer: Arc<TimerFile>) {
        self.phase_states.retain(|p| !Arc::ptr_eq(&p.timer, &timer));
    }
    pub fn reset_phases(&mut self) {
        self.phase_states.clear();
    }
}
