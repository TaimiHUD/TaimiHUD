#[cfg(feature = "space")]
use {crate::controller::pathing::PathingEvent, taimi_meta::ui::MapContext};
use {
    crate::{
        controller::timers::{TimersController, TimersEvent},
        exports::runtime::{
            bindings::CONTROLS,
            imgui::{self, Condition, MouseButton, TreeNode, TreeNodeFlags},
        },
        render::{
            element::{addons::AddonHostSelection, keys::KeyBindSelection},
            RenderEvent,
            RenderState,
        },
        settings::{
            state::{AddonHostName, BootstrapState},
            ArcSettings,
            ArcVk,
        },
        with_i18n,
    },
    tokio::sync::watch,
};

#[cfg(feature = "extension-arcdps")]
use crate::exports::{arcdps as exports, runtime as rt};

#[derive(Debug, Clone)]
pub struct ArcRenderState {
    boot_changes: watch::Receiver<BootstrapState>,
    load_host: AddonHostSelection,
    update_host: AddonHostSelection,
    bindings: KeyBindSelection,
    detected: bool,
}

impl ArcRenderState {
    pub fn new() -> Self {
        let mut state = Self {
            boot_changes: BootstrapState::get().subscribe(),
            load_host: Default::default(),
            update_host: Default::default(),
            bindings: Default::default(),
            detected: false,
        };
        state.sync_boot();
        state
    }

    pub fn sync_boot(&mut self) {
        self.detected = AddonHostName::ArcDPS.is_detected() == Some(true);
        let state = self.boot_changes.borrow_and_update();
        self.load_host.host = Some(state.addon_host_preference());
        self.update_host.host = state.update_host_preference();
    }

    pub fn ui_options(&mut self, ui: &imgui::Ui, host: AddonHostName) {
        if self.boot_changes.has_changed().ok() == Some(true) {
            self.sync_boot();
        }

        let load_host_preference = match self.load_host.draw(ui, "preferred-loader") {
            None if ui.is_item_clicked_with_button(MouseButton::Right) => Some(None),
            host => host.map(Some),
        };
        if let Some(host) = load_host_preference {
            BootstrapState::write_with(|s| s.addon_host_preference = host);
        }

        let update_host_preference = match self.update_host.draw_opt(ui, "preferred-updater") {
            None if ui.is_item_clicked_with_button(MouseButton::Right) => Some(None),
            host => host.map(Some),
        };
        if let Some(host) = update_host_preference {
            BootstrapState::write_with(|s| s.set_update_host_preference(host));
        }

        let mut drawn = false;
        if RenderState::is_host(host) != Some(true) && host.is_active() {
            drawn = true;
            if ui.button("take over") {
                // RenderState::select_host();
                RenderState::set_host(host);
            }
        }
        #[cfg(feature = "extension-arcdps")]
        if host != AddonHostName::ArcDPS {
            if exports::loaded() {
                if drawn {
                    ui.same_line();
                }
                if ui.button("un-arcdps") {
                    Self::un_arcdps();
                }
            } else if self.detected && !rt::arcdps_available() {
                if drawn {
                    ui.same_line();
                }
                if with_i18n!("arcdps", |label| ui.button(&label)) {
                    if let Err(e) = exports::enter() {
                        log::error!("arc unavailable? {e}");
                    }
                }
            }
        }
        let _ = drawn;

        let _keybinds = with_i18n!("addonbinds", |msg| TreeNode::new(&msg)
            .flags(TreeNodeFlags::FRAMED)
            .opened(true, Condition::Once)
            .tree_push_on_open(true)
            .build(ui, || self.ui_options_keybinds(ui)));
    }

    fn ui_options_keybinds(&mut self, ui: &imgui::Ui) {
        for &binding in ArcSettings::VK_WINDOWS {
            self.bindings.do_keybind(
                ui,
                binding,
                Some(|vk: &ArcVk| {
                    if let Some(window) = vk.window_name() {
                        crate::control_window(window, None);
                    }
                }),
            );
        }
        for &binding in ArcSettings::VK_CONTEXT_MENUS {
            self.bindings.do_keybind(
                ui,
                binding,
                Some(|vk: &ArcVk| {
                    if let Some(control) = vk.control() {
                        CONTROLS.notify_press(control.to_vk_dummy(), control);
                    }
                }),
            );
        }
        #[cfg(feature = "space")]
        {
            ui.separator();
            self.bindings.do_keybind(
                ui,
                &ArcSettings::VK_RENDER_TOGGLE_PATHING,
                Some(|_vk: &ArcVk| PathingEvent::VISIBLE_TOGGLE_SPACE.try_send()),
            );
            ui.separator();
            self.bindings.do_keybind(
                ui,
                &ArcSettings::VK_RENDER_TOGGLE_PATHING_MINIMAP,
                Some(|_vk: &ArcVk| PathingEvent::visible_toggle(MapContext::Minimap).try_send()),
            );
            ui.separator();
            self.bindings.do_keybind(
                ui,
                &ArcSettings::VK_RENDER_TOGGLE_PATHING_MAP,
                Some(|_vk: &ArcVk| PathingEvent::visible_toggle(MapContext::Global).try_send()),
            );
        }
        #[cfg(feature = "timers")]
        {
            ui.separator();
            for binding in &ArcSettings::VK_TIMER_TRIGGERS {
                self.bindings.do_keybind(
                    ui,
                    binding,
                    Some(|vk: &ArcVk| {
                        TimersController::try_send(TimersEvent::TimerKeyTrigger(vk.id.into(), false))
                    }),
                );
            }
            self.bindings.do_keybind(
                ui,
                &ArcSettings::VK_TIMER_RESET,
                Some(|_vk: &ArcVk| TimersController::try_send(TimersEvent::TimerReset)),
            );
        }
    }

    pub fn un_arcdps() {
        if rt::nexus_available() {
            std::thread::spawn(|| {
                let exit = unsafe { exports::ExitHandle::try_exit() };
                if let Ok(Some(exit)) = exit {
                    exit.free_blocking();
                }
            });
        } else {
            RenderState::try_send(RenderEvent::InitiateQuit);
        }
    }

    pub fn ui_options_disabled(ui: &imgui::Ui, host: AddonHostName) -> bool {
        use crate::settings::state::UpdatePreference;
        ui.text("addon offline or disabled via boot.json");

        let (pref_host, pref_updater, pref_update) = BootstrapState::read_with(|s| {
            // not real default but eh when will anyone see this anyway...
            let update_host = s.get_update_host_preference().unwrap_or(s.addon_host_preference);
            (
                s.addon_host_preference,
                update_host,
                s.update_preference().clone(),
            )
        });

        ui.separator();
        let mut new_pref_host =
            match AddonHostSelection::new_minimal(pref_host).draw(ui, "Loader Preference") {
                None if ui.is_item_clicked_with_button(MouseButton::Right) => Some(None),
                host => host.map(Some),
            };
        let mut new_pref_updater =
            match AddonHostSelection::new_minimal(pref_updater).draw_opt(ui, "Update Host Preference") {
                None if ui.is_item_clicked_with_button(MouseButton::Right) => Some(None),
                host => host.map(Some),
            };
        if ui.button("Reset") {
            new_pref_host = None;
            new_pref_updater = None;
        }

        #[cfg(feature = "extension-arcdps")]
        if host != AddonHostName::ArcDPS {
            if rt::arcdps_available() {
                ui.same_line();
                if ui.button("un-arcdps") {
                    Self::un_arcdps();
                }
            } else if !exports::loaded() {
                ui.same_line();
                if ui.button(&"arcdps") {
                    if let Err(e) = exports::enter() {
                        log::error!("arc unavailable? {e}");
                    }
                }
            }
        }

        match host {
            AddonHostName::Nexus => ui.text("reload addon in nexus to apply changes"),
            _ => (),
        }

        let mut new_pref_update = None;
        ui.separator();
        ui.text("update authorization");
        ui.same_line();
        ui.text(&format!("{pref_update}"));
        if ui.is_item_clicked_with_button(MouseButton::Right) {
            new_pref_update = Some(None);
        }
        ui.same_line();
        if !matches!(pref_update, UpdatePreference::Always) {
            if ui.button("Force updates on") {
                new_pref_update = Some(Some(UpdatePreference::Always));
            }
        } else {
            if ui.button("Force updates off") {
                new_pref_update = Some(Some(UpdatePreference::Never));
            }
        }
        if ui.is_item_clicked_with_button(MouseButton::Right) {
            new_pref_update = Some(None);
        }
        let changed = new_pref_host.is_some() | new_pref_updater.is_some() | new_pref_update.is_some();
        if changed {
            BootstrapState::write_with(|s| {
                if let Some(host) = new_pref_host {
                    s.addon_host_preference = host;
                }
                if let Some(updater) = new_pref_updater {
                    s.set_update_host_preference(updater);
                }
                if let Some(update) = new_pref_update {
                    s.update_preference = update;
                }
            });
        }

        changed
    }
}
