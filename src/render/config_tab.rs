use {
    super::TimerWindowState,
    crate::{
        controller::timers::{ProgressBarStyleChange, TimersController, TimersEvent}, fl, render::{
            machine::RenderMachine,
            RenderEvent, TextFont,
        }, settings::{MarkerAutoPlaceSettings, Settings, SquadCondition}, Controller, ControllerEvent, MarkersController, MarkersEvent
    },
    nexus::imgui::{ComboBox, Condition, Selectable, Slider, TreeNode, TreeNodeFlags, Ui},
    strum::IntoEnumIterator,
};
#[cfg(feature = "extension-nexus")]
use crate::{
    exports::runtime::bindings::TaimiControls,
    with_i18n,
};

pub struct ConfigTabState {
    pub marker_autoplace: MarkerAutoPlaceSettings,
    pub marker_autoplace_inner: Option<SquadCondition>,
    pub dpi_scaling: Option<f32>,
    #[cfg(feature = "extension-nexus")]
    pub quick_access_icons_visible: TaimiControls,
}

impl ConfigTabState {
    pub fn new() -> Self {
        Self {
            dpi_scaling: Default::default(),
            marker_autoplace: Default::default(),
            marker_autoplace_inner: Default::default(),
            #[cfg(feature = "extension-nexus")]
            quick_access_icons_visible: TaimiControls::default_quick_access(),
        }
    }

    pub fn draw(&mut self, ui: &Ui, machine: &mut RenderMachine, timer_window_state: &mut TimerWindowState) {
        ui.text_wrapped(&fl!("imgui-notice"));
        ui.dummy([4.0, 4.0]);
        ui.text_wrapped(&fl!("keybind-triggers"));
        ui.dummy([4.0, 4.0]);

        if ui.button(&fl!("quit")) {
            // XXX: this will wipe all of render state, rather than just katrender
            let mut render_sender = crate::RENDER_SENDER.write().unwrap();
            let render_quit = render_sender.as_ref().map(|sender| sender.try_send(RenderEvent::Quit));
            if let Some(Ok(())) = render_quit {
                let _ = render_sender.take();
                let _ = crate::SPACE_SENDER.write().unwrap().take();
                crate::TEXTURES.quit();
            }
            Controller::try_send(ControllerEvent::UnloadAll);
        }
        ui.same_line();
        if ui.button(&fl!("save")) {
            Controller::try_send(ControllerEvent::SaveSettings);
        }
        ui.dummy([4.0, 4.0]);

        #[cfg(feature = "extension-nexus")]
        let nexus_ui = || {
            use crate::exports::nexus::{quick_access_button_id, quick_access_add, quick_access_remove};

            if let Some(settings) = Settings::try_read() {
                self.quick_access_icons_visible = settings.quick_access_visible.clone();
            }

            with_i18n("nexus-quick-access", |msg| ui.text(msg));
            let prior_visible = self.quick_access_icons_visible;
            let mut changed = false;
            for (i, icon) in TaimiControls::QUICK_ACCESS_ICONS.into_iter().enumerate() {
                let Some((_, _, _, keybind)) = quick_access_button_id(icon) else { continue };
                if i > 0 && i % 4 != 0 { ui.same_line(); }
                changed |= with_i18n!(keybind, |name|
                    ui.checkbox_flags(name, &mut self.quick_access_icons_visible, icon)
                );
            }

            if changed {
                let _ = Settings::write_with_blocking(|settings| {
                    settings.quick_access_visible = self.quick_access_icons_visible;
                });
                for icon in prior_visible ^ self.quick_access_icons_visible {
                    match self.quick_access_icons_visible.intersects(icon) {
                        true => quick_access_add(icon),
                        false => quick_access_remove(icon),
                    }
                }
            }
        };
        let _nexus_ui = TreeNode::new(&fl!("nexus"))
            .flags(TreeNodeFlags::FRAMED)
            .opened(true, Condition::Once)
            .tree_push_on_open(true)
            .build(ui, nexus_ui);

        let markers_window_closure = || {
            if let Some(settings) = Settings::try_read() {
                self.dpi_scaling = settings.dpi_scaling.clone();
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
                MarkersController::try_send(MarkersEvent::MarkerAutoPlaceSettings(
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
                    MarkersController::try_send(MarkersEvent::MarkerAutoPlaceSettings(
                        self.marker_autoplace.clone(),
                    ));
                }
            }
            ui.dummy([4.0, 4.0]);
            let mut dpi_scaling = self.dpi_scaling.is_none();
            if ui.checkbox(&fl!("dpi-scaling"), &mut dpi_scaling) {
                use taimi_meta::ui::MapCalibration;

                // TODO: controller event
                let _ = Settings::write_with_blocking(|settings| {
                    settings.dpi_scaling = (!dpi_scaling).then_some(MapCalibration::DPI_REFERENCE);
                    self.dpi_scaling = settings.dpi_scaling.clone();
                });
                machine.act_display_size();
            }
            ui.text_wrapped(&fl!("dpi-notice"));
        };
        let timers_window_closure = || {
            ui.dummy([4.0, 4.0]);
            if let Some(settings) = Settings::try_read() {
                timer_window_state.progress_bar.stock = settings.progress_bar.stock;
            };
            if ui.checkbox(
                &fl!("stock-imgui-progress-bar"),
                &mut timer_window_state.progress_bar.stock,
            ) {
                TimersController::try_send(TimersEvent::ProgressBarStyle(
                    ProgressBarStyleChange::Stock(timer_window_state.progress_bar.stock),
                ));
            };
            if ui.checkbox(&fl!("shadow"), &mut timer_window_state.progress_bar.shadow) {
                TimersController::try_send(TimersEvent::ProgressBarStyle(
                    ProgressBarStyleChange::Shadow(timer_window_state.progress_bar.shadow),
                ));
            }
            if ui.checkbox(
                &fl!("centre-text-after-icon"),
                &mut timer_window_state.progress_bar.centre_after,
            ) {
                TimersController::try_send(TimersEvent::ProgressBarStyle(
                    ProgressBarStyleChange::Centre(timer_window_state.progress_bar.centre_after),
                ));
            }
            if Slider::new(&fl!("height"), 8.0, 256.0)
                .display_format("%.0f")
                .build(ui, &mut timer_window_state.progress_bar.height)
            {
                TimersController::try_send(TimersEvent::ProgressBarStyle(
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
                        TimersController::try_send(TimersEvent::ProgressBarStyle(
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
