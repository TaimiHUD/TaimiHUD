#[cfg(feature = "space")]
use {crate::controller::pathing::PathingEvent, taimi_meta::ui::MapContext};
use {
    crate::{
        controller::timers::{TimersController, TimersEvent},
        exports::runtime::bindings::CONTROLS,
        render::{
            element::{addons::AddonHostSelection, keys::KeyBindSelection, prelude::*},
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
            load_host: AddonHostSelection::new(fl!("preferred-loader"), Err(None)),
            update_host: AddonHostSelection::new(fl!("preferred-updater"), Ok(fl!("disabled"))),
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

    pub fn ui_options<'ui, U, C>(&mut self, ui: &mut U, context: &mut C, host: AddonHostName)
    where
        U: ?Sized + ImDrawWindow<'ui>,
        C: ?Sized + DrawContext<'ui>,
    {
        if self.boot_changes.has_changed().ok() == Some(true) {
            self.sync_boot();
        }

        let load_host_preference = match self.load_host.draw(ui, context) {
            false if ui.is_item_right_clicked() => Some(None),
            false => None,
            true => Some(self.load_host.host),
        };
        if let Some(host) = load_host_preference {
            BootstrapState::write_with(|s| s.addon_host_preference = host);
        }

        let update_host_preference = match self.update_host.draw(ui, context) {
            false if ui.is_item_right_clicked() => Some(None),
            false => None,
            true => Some(Some(self.update_host.host)),
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
            if with_i18n!("arcdps", |label| ui.button(label)) {
                if let Err(e) = exports::enter() {
                    log::error!("arc unavailable? {e}");
                }
            }
        }
        let _ = drawn;

        let args = match ui.imgui_version_num() {
            #[cfg(taimi_imgui = "180")]
            Some(im180::VERSION_NUM) => imw::TreeNode::IM180_ARGS_FRAMED,
            #[cfg(taimi_imgui = "192")]
            Some(im192::VERSION_NUM) => imw::TreeNode::IM192_ARGS_FRAMED,
            _ => Default::default(),
        };
        let keybinds = with_i18n!("addonbinds", |msg| ui.begin_tree_node(
            Some(ImCondition::INITIAL),
            c"addonbinds",
            msg,
            args
        ));
        if let Some(_keybinds) = keybinds {
            self.ui_options_keybinds(ui);
        }
    }

    fn ui_options_keybinds<'ui, U>(&mut self, ui: &mut U)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
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

    pub fn ui_options_disabled<'ui, U, C>(ui: &mut U, context: &mut C, host: AddonHostName) -> bool
    where
        U: ?Sized + ImDrawWindow<'ui>,
        C: ?Sized + DrawContext<'ui>,
    {
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
        let mut new_pref_host = AddonHostSelection::with_host("Loader Preference", Err(None), pref_host);
        let mut new_pref_host = match new_pref_host.draw(ui, context) {
            true => Some(new_pref_host.host),
            false => None,
        };
        let mut new_pref_updater =
            AddonHostSelection::with_host("Update Host Preference", Err(Some("Disabled")), pref_updater);
        let mut new_pref_updater = match new_pref_updater.draw(ui, context) {
            true => Some(Some(new_pref_updater.host)),
            false => None,
        };
        if ui.button("Reset") {
            new_pref_host = Some(None);
            new_pref_updater = Some(None);
        }

        #[cfg(feature = "extension-arcdps")]
        if rt::arcdps_available() {
            ui.same_line();
            if ui.button("un-arcdps") {
                Self::un_arcdps();
            }
        } else if !exports::loaded() && AddonHostName::ArcDPS.is_detected() == Some(true) {
            ui.same_line();
            if ui.button(&"arcdps") {
                if let Err(e) = exports::enter() {
                    log::error!("arc unavailable? {e}");
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
        ui.text(im_to_s!(pref_update));
        if ui.is_item_right_clicked() {
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
        if ui.is_item_right_clicked() {
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
