use {
    crate::{
        control_window,
        exports::runtime::{
            bindings::TaimiControls,
            imgui::{MenuItem, MouseButton, Selectable, Ui},
        },
        render::RenderState,
        with_i18n,
    },
    std::cell::Cell,
};

thread_local! {
    static CONTEXT_PRIMARY_CONTROL: Cell<TaimiControls> = Cell::new(TaimiControls::empty());
}
impl RenderState {
    pub const MENU_PRIMARY_ID: &'static str = "taimi-context";

    pub fn open_context_menu(ui: &Ui, controls: TaimiControls) {
        if !controls.is_empty() {
            CONTEXT_PRIMARY_CONTROL.set(controls);
        }
        ui.open_popup(Self::MENU_PRIMARY_ID)
    }
    pub(super) fn open_context(&mut self, ui: &Ui, menus: TaimiControls) {
        let controls = match menus {
            TaimiControls::MENU_PRIMARY => TaimiControls::WINDOW_TOGGLES,
            _ => TaimiControls::WINDOW_PRIMARY,
        };
        Self::open_context_menu(ui, controls);
    }
    pub(super) fn draw_context_menu(&mut self, ui: &Ui) {
        let popup = ui.begin_popup(Self::MENU_PRIMARY_ID);
        if let Some(popup) = popup {
            self.draw_context_popup(ui, CONTEXT_PRIMARY_CONTROL.get());
            popup.end();
        } else {
            CONTEXT_PRIMARY_CONTROL.set(TaimiControls::empty());
        }
    }
    pub fn render_context_popup(ui: &Ui, control: TaimiControls) {
        if !Self::is_running() {
            return
        }

        let mut lock = Self::lock();
        let Some(state) = &mut *lock else { return };

        state.draw_context_popup(ui, control);
    }

    #[cfg(feature = "markers")]
    const MENU_CONTROLS_MARKERS: TaimiControls = TaimiControls::from_bits_retain(
        TaimiControls::WINDOW_PRIMARY.bits() | TaimiControls::WINDOW_MARKERS.bits(),
    );
    #[cfg(feature = "timers")]
    const MENU_CONTROLS_TIMERS: TaimiControls = TaimiControls::from_bits_retain(
        TaimiControls::WINDOW_PRIMARY.bits()
            | TaimiControls::WINDOW_TIMERS.bits()
            | TaimiControls::TIMER_TRIGGERS.bits()
            | TaimiControls::TIMER_RESET.bits(),
    );
    #[cfg(feature = "space")]
    const MENU_CONTROLS_PATHING: TaimiControls = TaimiControls::from_bits_retain(
        TaimiControls::WINDOW_PRIMARY.bits()
            | TaimiControls::WINDOW_PATHING.bits()
            | TaimiControls::PATHING_TOGGLES.bits(),
    );
    /// Quick access right-click menu
    pub fn draw_context_popup(&mut self, ui: &Ui, control: TaimiControls) {
        let need_sep = false;
        #[cfg(feature = "timers")]
        let need_sep = if control.intersects(Self::MENU_CONTROLS_TIMERS) {
            if need_sep {
                ui.separator()
            }
            self.draw_context_timers(ui, control != TaimiControls::WINDOW_PRIMARY);
            true
        } else {
            false
        };
        #[cfg(feature = "space")]
        let need_sep = if control.intersects(Self::MENU_CONTROLS_PATHING) {
            if need_sep {
                ui.separator()
            }
            self.draw_context_pathing(ui, control != TaimiControls::WINDOW_PRIMARY);
            true
        } else {
            false
        };
        #[cfg(feature = "markers")]
        let need_sep = if control.intersects(Self::MENU_CONTROLS_MARKERS) {
            if need_sep {
                ui.separator()
            }
            self.draw_context_markers(ui, control != TaimiControls::WINDOW_PRIMARY);
            true
        } else {
            false
        };
        if control.intersects(TaimiControls::WINDOW_PRIMARY) {
            if need_sep {
                ui.separator()
            }
            if with_i18n!("primary-window", |label| Selectable::new(&label)
                .selected(self.primary_window.open)
                .build(ui))
            {
                control_window(crate::WINDOW_PRIMARY, None);
            }
        }
    }
    #[cfg(feature = "space")]
    pub fn draw_context_pathing(&mut self, ui: &Ui, inline: bool) {
        use {crate::controller::pathing::PathingEvent, taimi_meta::ui::LocalContext};

        let (window_label, inline) = match inline {
            false => ("pathing-window", true),
            true => ("pathing-window-toggle", false),
        };

        let mut engine = match &mut self.engine {
            Some(Ok(engine)) => Some(engine),
            _ => None,
        };
        // TODO: just use katrender toggle state instead
        let pathing_enabled = engine.is_some();

        let mut toggled = None;
        let toggle_with = |label: &str, mut value: Option<&mut bool>, inline: Option<bool>| -> bool {
            let v = value.as_mut().map(|v| **v).unwrap_or(false);
            let toggled = match (&mut value, inline) {
                (Some(value), Some(true)) => MenuItem::new(label).build_with_ref(ui, *value),
                (Some(value), Some(false)) => ui.checkbox(label, *value),
                #[cfg(todo)]
                (None, Some(false)) => ui.button(label),
                (Some(value), _) => Selectable::new(label)
                    .selected(v)
                    .close_popups(ui.io().key_ctrl || ui.io().key_shift)
                    //.close_popups(false)
                    .build_with_ref(ui, *value),
                _ => Selectable::new(label).selected(v).build(ui),
            };
            if !toggled && ui.is_item_clicked_with_button(MouseButton::Right) {
                if let Some(value) = value {
                    *value ^= true;
                }
                true
            } else {
                toggled
            }
        };
        let toggle =
            |label: &str, value: Option<&mut bool>| -> bool { toggle_with(label, value, Some(inline)) };
        let visibility = engine.as_mut().map(|e| {
            e.map_settings(|s| {
                (
                    s.space.visible_space(),
                    s.space.visible_minimap(),
                    s.space.visible_worldmap(),
                )
            })
        });
        if let Some((mut visible_space, mut visible_minimap, mut visible_worldmap)) = visibility {
            if with_i18n!("pathing-render-toggle", |label| toggle(
                &label,
                Some(&mut visible_space)
            )) {
                toggled = Some((LocalContext::World, Some(visible_space)));
            }
            if with_i18n!("pathing-render-minimap-toggle", |label| toggle(
                &label,
                Some(&mut visible_minimap)
            )) {
                toggled = Some((LocalContext::MINIMAP, Some(visible_minimap)));
            }
            if with_i18n!("pathing-render-map-toggle", |label| toggle(
                &label,
                Some(&mut visible_worldmap)
            )) {
                toggled = Some((LocalContext::GLOBAL, Some(visible_worldmap)));
            }
        } else {
            if with_i18n!("pathing-render-toggle", |label| toggle(&label, None)) {
                toggled = Some((LocalContext::World, None));
            }
            if with_i18n!("pathing-render-minimap-toggle", |label| toggle(&label, None)) {
                toggled = Some((LocalContext::MINIMAP, None));
            }
            if with_i18n!("pathing-render-map-toggle", |label| toggle(&label, None)) {
                toggled = Some((LocalContext::GLOBAL, None));
            }
        }
        match toggled {
            Some((ctx, set)) => {
                PathingEvent::VisibleToggle { context: ctx.into(), set }.try_send();
            },
            None => (),
        }

        let window_open = self.pathing_window.open;
        let submenu_id = "context-popup-pathing";
        let mut submenu = Some(|| {
            if pathing_enabled {
                if with_i18n!("reload-packs", |msg| Selectable::new(&msg).build(ui)) {
                    PathingEvent::ReloadAll(true).try_send();
                }
                if with_i18n!("deactivate-packs", |msg| Selectable::new(&msg).build(ui)) {
                    PathingEvent::UnloadAll(false).try_send();
                }
                #[cfg(todo = "unnecessary")]
                if with_i18n!("remove-packs", |msg| Selectable::new(&msg).build(ui)) {
                    PathingEvent::UnloadAll(true).try_send();
                }
            }
            if with_i18n!("toggle", |msg| Selectable::new(&msg).build(ui)) {
                PathingEvent::ToggleKatRender.try_send();
            }
            if let Some(_menu) = with_i18n!("advanced", |msg| ui.begin_menu(&msg)) {
                self.machine.pack_ui_state.draw_menu_advanced(ui);
            }
            if let Some(_menu) = ui.begin_menu("some") {
                self.pathing_menu_open = true;
                if pathing_enabled {
                    MenuItem::new("body").enabled(false).build(ui);
                    self.pathing_window.draw_context_menu(ui, &mut self.machine);
                } else {
                    MenuItem::new("where").enabled(false).build(ui);
                }
            }
        });
        if !inline {
            ui.popup(submenu_id, || {
                if let Some(mut f) = submenu.take() {
                    f()
                }
            });
        } else {
            ui.separator();
        }
        let submenu_open = submenu.is_none();

        if with_i18n!(window_label, |label| Selectable::new(&label)
            .selected(window_open)
            .build(ui))
        {
            control_window(crate::WINDOW_PATHING, None);
        }
        if pathing_enabled {
            if inline {
                if let Some(mut f) = submenu.take() {
                    f();
                }
            } else if ui.is_item_clicked_with_button(MouseButton::Right) {
                ui.open_popup(submenu_id);
            } else if !submenu_open && ui.is_item_hovered() {
                with_i18n!("context-click-notice", |msg| ui.tooltip_text(&msg));
                // TODO: just steal nexus "((000102))"? could be useful to have a way to load those if available...
            }
        }
    }
    #[cfg(feature = "markers")]
    pub fn draw_context_markers(&mut self, ui: &Ui, inline: bool) {
        use crate::controller::markers::{MarkersController, MarkersEvent};

        let window_label = match inline {
            false => "marker-window",
            true => "marker-window-toggle",
        };

        let window_open = self.marker_window.open;
        let submenu_id = "context-popup-markers";
        let mut submenu = Some(|| {
            // TODO: temporary autoplace setting toggle?
            if with_i18n!("clear-spent-autoplace", |msg| Selectable::new(&msg).build(ui)) {
                MarkersController::try_send(MarkersEvent::ClearSpentAutoplace);
            }
            if with_i18n!("clear-markers", |msg| Selectable::new(&msg).build(ui)) {
                MarkersController::try_send(MarkersEvent::ClearMarkers);
            }
            if with_i18n!("reload-markers", |msg| Selectable::new(&msg).build(ui)) {
                MarkersController::try_send(MarkersEvent::ReloadMarkers);
            }
        });

        if !inline {
            ui.popup(submenu_id, || {
                if let Some(f) = submenu.take() {
                    f()
                }
            });
        }
        let submenu_open = submenu.is_none();

        if with_i18n!(window_label, |label| Selectable::new(&label)
            .selected(window_open)
            .build(ui))
        {
            control_window(crate::WINDOW_MARKERS, None);
        }
        if inline {
            if let Some(f) = submenu.take() {
                f();
            }
        } else if ui.is_item_clicked_with_button(MouseButton::Right) {
            ui.open_popup(submenu_id);
        } else if !submenu_open && ui.is_item_hovered() {
            with_i18n!("context-click-notice", |msg| ui.tooltip_text(&msg));
        }
    }
    #[cfg(feature = "timers")]
    pub fn draw_context_timers(&mut self, ui: &Ui, inline: bool) {
        use crate::{
            controller::timers::{TimersController, TimersEvent},
            fl,
        };

        let window_label = match inline {
            false => "timer-window",
            true => "timer-window-toggle",
        };

        let window_open = self.timer_window.open;
        let submenu_id = "context-popup-timers";
        let mut submenu = Some(|| {
            if with_i18n!("timer-key-reset", |msg| Selectable::new(&msg).build(ui)) {
                TimersController::try_send(TimersEvent::TimerReset);
            }
            for id in 0..=4 {
                // TODO: link this to keybind system and show state?
                if Selectable::new(&fl!("timer-key-trigger", id = id)).build(ui) {
                    use std::{thread, time::Duration};
                    let id = format!("{id}");
                    TimersController::try_send(TimersEvent::TimerKeyTrigger(id.clone(), false));
                    thread::spawn(move || {
                        // TODO: lol delayed release event...
                        thread::sleep(Duration::from_millis(500));
                        TimersController::try_send(TimersEvent::TimerKeyTrigger(id, true));
                    });
                }
            }
            if with_i18n!("reload-timers", |msg| Selectable::new(&msg).build(ui)) {
                TimersController::try_send(TimersEvent::ReloadTimers);
            }
        });

        if !inline {
            ui.popup(submenu_id, || {
                if let Some(f) = submenu.take() {
                    f()
                }
            });
        }
        let submenu_open = submenu.is_none();

        if with_i18n!(window_label, |label| Selectable::new(&label)
            .selected(window_open)
            .build(ui))
        {
            control_window(crate::WINDOW_TIMERS, None);
        }
        if inline {
            if let Some(f) = submenu.take() {
                f();
            }
        } else if ui.is_item_clicked_with_button(MouseButton::Right) {
            ui.open_popup(submenu_id);
        } else if !submenu_open && ui.is_item_hovered() {
            with_i18n!("context-click-notice", |msg| ui.tooltip_text(&msg));
        }
    }
}
