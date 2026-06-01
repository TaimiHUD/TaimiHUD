use {
    super::RenderState,
    crate::{
        control_window,
        controller::timers::{TimersController, TimersEvent},
        render::element::prelude::*,
        settings::{ProgressBarSettings, Settings},
        timer::{PhaseState, TimerAlert, TimerFile},
    },
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

    pub fn draw<'ui, U>(&mut self, ui: &mut U)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let mut open = self.open;
        if let Some(settings) = Settings::try_read() {
            open = settings.timers_window_open;
            self.progress_bar = settings.progress_bar.clone();
        };
        if open {
            let window = with_i18n!("timer-window", |label| ui.begin_taimi_window(
                "timer-window",
                label,
                ImCondition::initial(ImSize2::new(300.0, 200.0)),
                &mut open,
            ));

            if let Some(_window) = window {
                if !self.phase_states.is_empty() {
                    if ui.button(fl!("reset-timers")) {
                        TimersController::try_send(TimersEvent::TimerReset);
                        self.reset_phases();
                    }
                    ui.dummy([2.0; 2]);
                    ui.separator();
                    ui.dummy([4.0; 2]);
                } else {
                    ui.text_wrapped(fl!("no-phases-active"));
                }
                for ps in &self.phase_states {
                    for alert in ps.phase.get_alerts() {
                        if self.progress_bar.stock {
                            Self::stock_progress_bar(&self.progress_bar, &alert, ui, ps);
                        } else {
                            Self::progress_bar(&self.progress_bar, &alert, ui, ps);
                        }
                    }
                }
            }
        }

        if open != self.open {
            control_window(crate::WINDOW_TIMERS, Some(open));
            self.open = open;
        }
    }

    fn progress_bar<'ui, U>(settings: &ProgressBarSettings, alert: &TimerAlert, ui: &mut U, ps: &PhaseState)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let start = ps.start;
        let height = settings.height;
        if let Some(percent) = alert.percentage(&ps.offsets, start) {
            let mut widget_pos = ImPos2::new(0.0, 0.0);
            if !settings.centre_after {
                widget_pos = ui.cursor_pos();
            }
            RenderState::icon(
                ui,
                Some(height),
                alert.icon.as_ref(),
                ps.timer().path.as_ref().and_then(|p| p.parent()),
            );
            if settings.centre_after {
                widget_pos = ui.cursor_pos();
            }
            let _fill_colour = alert
                .fill_colour()
                .map(|c| ui.push_colour(ImColourIndex::PlotHistogram, c.imgcolor()));
            let _colour = alert
                .colour()
                .map(|c| ui.push_colour(ImColourIndex::Text, c.imgcolor()));
            let size = imw::ProgressBar::prepare_height(height);
            ui.progress_bar(percent, Some(c""), Some(size));
            let window_size = ui.window_size();
            let widget_size = ui.units().map(window_size.with_y(height));
            let text = alert.progress_bar_text(&ps.offsets, start);
            //let font = ui.push_font_opt(settings.font.to_nexus());
            RenderState::offset_font_text(
                settings.font.to_nexus(),
                ui,
                widget_pos,
                widget_size,
                settings.shadow,
                &text,
            );
            ui.dummy([0.0, height / 4.0]);
        }
    }

    fn stock_progress_bar<'ui, U>(
        settings: &ProgressBarSettings,
        alert: &TimerAlert,
        ui: &mut U,
        ps: &PhaseState,
    ) where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let start = ps.start;
        let height = settings.height;
        if let Some(percent) = alert.percentage(&ps.offsets, start) {
            RenderState::icon(
                ui,
                Some(height),
                alert.icon.as_ref(),
                ps.timer().path.as_ref().and_then(|p| p.parent()),
            );
            let _fill_colour = alert
                .fill_colour()
                .map(|c| ui.push_colour(ImColourIndex::PlotHistogram, c.imgcolor()));
            let _text_colour = alert
                .colour()
                .map(|c| ui.push_colour(ImColourIndex::Text, c.imgcolor()));
            let overlay = alert.progress_bar_text(&ps.offsets, start);
            let size = imw::ProgressBar::prepare_height(height);
            ui.progress_bar(percent, Some(overlay), Some(size));
        }
    }

    pub fn new_phase(&mut self, phase_state: PhaseState) {
        self.phase_states.push(phase_state);
    }
    pub fn remove_phase(&mut self, timer: &Arc<TimerFile>) {
        self.phase_states.retain(|p| !Arc::ptr_eq(p.timer(), timer));
    }
    pub fn reset_phases(&mut self) {
        self.phase_states.clear();
    }
}
