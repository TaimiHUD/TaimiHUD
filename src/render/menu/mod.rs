use {
    crate::{
        control_window,
        exports::runtime::bindings::TaimiControls,
        render::{element::prelude::*, RenderState},
        with_i18n,
    },
    std::cell::Cell,
};

thread_local! {
    static CONTEXT_PRIMARY_CONTROL: Cell<TaimiControls> = Cell::new(TaimiControls::empty());
}
impl RenderState {
    pub const MENU_PRIMARY_ID: &'static CStr = c"taimi-context";

    pub fn open_context_menu<'ui, U>(ui: &mut U, controls: TaimiControls)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        if !controls.is_empty() {
            CONTEXT_PRIMARY_CONTROL.set(controls);
        }
        ui.open_popup(Self::MENU_PRIMARY_ID)
    }
    pub(super) fn open_context<'ui, U>(&mut self, ui: &mut U, menus: TaimiControls)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let controls = match menus {
            TaimiControls::MENU_PRIMARY => TaimiControls::WINDOW_TOGGLES,
            _ => TaimiControls::WINDOW_PRIMARY,
        };
        Self::open_context_menu(ui, controls);
    }
    pub(super) fn draw_context_menu<'ui, U>(&mut self, ui: &mut U)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let popup = ui.begin_popup(Self::MENU_PRIMARY_ID, Default::default());
        if let Some(popup) = popup {
            self.draw_context_popup(ui, CONTEXT_PRIMARY_CONTROL.get());
            popup.end();
        } else {
            CONTEXT_PRIMARY_CONTROL.set(TaimiControls::empty());
        }
    }
    pub fn render_context_popup<'ui, U>(ui: &mut U, control: TaimiControls)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
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
    pub fn draw_context_popup<'ui, U>(&mut self, ui: &mut U, control: TaimiControls)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
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
            if with_i18n!("primary-window", |label| ui
                .selectable(label, self.primary_window.open,))
            {
                control_window(crate::WINDOW_PRIMARY, None);
            }
        }
    }
    #[cfg(feature = "space")]
    pub fn draw_context_pathing<'ui, U>(&mut self, ui: &mut U, inline: bool)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
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
        let mut toggle_with = |label: &str, mut value: Option<&mut bool>, inline: Option<bool>| -> bool {
            let v = value.as_mut().map(|v| **v).unwrap_or(false);
            let toggled = match (&mut value, inline) {
                (Some(value), Some(true)) =>
                    imw::Interacted::apply_with_bool(*value, |state| ui.menu_item(label, state)),
                (Some(value), Some(false)) => ui.checkbox(label, *value),
                #[cfg(todo)]
                (None, Some(false)) => ui.button(label),
                (Some(value), _) => imw::Interacted::apply_bool(
                    *value,
                    ui.selectable_dismiss(
                        label,
                        v,
                        ui.im_io_mod_keys().intersects(KeyState::CTRL | KeyState::SHIFT),
                    ),
                ),
                _ => ui.selectable(label, v),
            };
            if !toggled && ui.is_item_right_clicked() {
                if let Some(value) = value {
                    *value ^= true;
                }
                true
            } else {
                toggled
            }
        };
        let mut toggle =
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
        let mut submenu = Some(|ui: &mut U| {
            if pathing_enabled {
                if with_i18n!("reload-packs", |msg| ui.pressable(msg)) {
                    PathingEvent::ReloadAll(true).try_send();
                }
                if with_i18n!("deactivate-packs", |msg| ui.pressable(msg)) {
                    PathingEvent::UnloadAll(false).try_send();
                }
                #[cfg(todo = "unnecessary")]
                if with_i18n!("unload-packs", |msg| ui.pressable(msg)) {
                    PathingEvent::UnloadAll(true).try_send();
                }
            }
            if with_i18n!("toggle", |msg| ui.pressable(msg)) {
                PathingEvent::ToggleKatRender.try_send();
            }
            if let Some(_menu) = ui.begin_menu(fl!("advanced")) {
                self.machine.pack_ui_state.draw_menu_advanced(ui);
            }
            if let Some(_menu) = ui.begin_menu(c"some") {
                self.pathing_menu_open = true;
                if pathing_enabled {
                    ui.menu_item_enabled(c"body", false, false);
                    self.pathing_window.draw_context_menu(ui, &mut self.machine);
                } else {
                    ui.menu_item_enabled(c"where", false, false);
                }
            }
        });
        if !inline {
            if let Some(_popup) = ui.begin_popup(submenu_id, Default::default()) {
                if let Some(mut f) = submenu.take() {
                    f(ui)
                }
            }
        } else {
            ui.separator();
        }
        let submenu_open = submenu.is_none();

        if with_i18n!(window_label, |label| ui.selectable(label, window_open)) {
            control_window(crate::WINDOW_PATHING, None);
        }
        if pathing_enabled {
            if inline {
                if let Some(mut f) = submenu.take() {
                    f(ui);
                }
            } else if ui.is_item_right_clicked() {
                ui.open_popup(submenu_id);
            } else if !submenu_open && ui.is_item_hovered() {
                with_i18n!("context-click-notice", |msg| ui.tooltip_text(&msg));
                // TODO: just steal nexus "((000102))"? could be useful to have a way to load those if available...
            }
        }
    }
    #[cfg(feature = "markers")]
    pub fn draw_context_markers<'ui, U>(&mut self, ui: &mut U, inline: bool)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        use crate::controller::markers::{MarkersController, MarkersEvent};

        let window_label = match inline {
            false => "marker-window",
            true => "marker-window-toggle",
        };

        let window_open = self.marker_window.open;
        let submenu_id = c"context-popup-markers";
        let mut submenu = Some(|ui: &mut U| {
            // TODO: temporary autoplace setting toggle?
            if with_i18n!("clear-spent-autoplace", |msg| ui.pressable(msg)) {
                MarkersController::try_send(MarkersEvent::ClearSpentAutoplace);
            }
            if with_i18n!("clear-markers", |msg| ui.pressable(msg)) {
                MarkersController::try_send(MarkersEvent::ClearMarkers);
            }
            if with_i18n!("reload-markers", |msg| ui.pressable(msg)) {
                MarkersController::try_send(MarkersEvent::ReloadMarkers);
            }
        });

        if !inline {
            if let Some(_popup) = ui.begin_popup(submenu_id, Default::default()) {
                if let Some(f) = submenu.take() {
                    f(ui)
                }
            }
        }
        let submenu_open = submenu.is_none();

        if with_i18n!(window_label, |label| ui.selectable(label, window_open)) {
            control_window(crate::WINDOW_MARKERS, None);
        }
        if inline {
            if let Some(f) = submenu.take() {
                f(ui);
            }
        } else if ui.is_item_right_clicked() {
            ui.open_popup(submenu_id);
        } else if !submenu_open && ui.is_item_hovered() {
            with_i18n!("context-click-notice", |msg| ui.tooltip_text(&msg));
        }
    }
    #[cfg(feature = "timers")]
    pub fn draw_context_timers<'ui, U>(&mut self, ui: &mut U, inline: bool)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        use crate::controller::timers::{TimersController, TimersEvent};

        let window_label = match inline {
            false => "timer-window",
            true => "timer-window-toggle",
        };

        let window_open = self.timer_window.open;
        let submenu_id = c"context-popup-timers";
        let mut submenu = Some(|ui: &mut U| {
            if with_i18n!("timer-key-reset", |msg| ui.pressable(msg)) {
                TimersController::try_send(TimersEvent::TimerReset);
            }
            for id in 0..=4 {
                // TODO: link this to keybind system and show state?
                if ui.pressable(fl!("timer-key-trigger", id = id)) {
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
            if with_i18n!("reload-timers", |msg| ui.pressable(msg)) {
                TimersController::try_send(TimersEvent::ReloadTimers);
            }
        });

        if !inline {
            if let Some(_popup) = ui.begin_popup(submenu_id, Default::default()) {
                if let Some(f) = submenu.take() {
                    f(ui)
                }
            }
        }
        let submenu_open = submenu.is_none();

        if with_i18n!(window_label, |label| ui.selectable(label, window_open)) {
            control_window(crate::WINDOW_TIMERS, None);
        }
        if inline {
            if let Some(f) = submenu.take() {
                f(ui);
            }
        } else if ui.is_item_right_clicked() {
            ui.open_popup(submenu_id);
        } else if !submenu_open && ui.is_item_hovered() {
            with_i18n!("context-click-notice", |msg| ui.tooltip_text(&msg));
        }
    }
}
