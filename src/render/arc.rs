#[cfg(feature = "space")]
use {crate::controller::pathing::PathingEvent, taimi_meta::ui::MapContext};
use {
    crate::{
        controller::timers::{TimersController, TimersEvent},
        exports::runtime::{
            bindings::CONTROLS,
            imgui::{self, Condition, TreeNode, TreeNodeFlags},
        },
        render::element::{addons::AddonHostSelection, keys::KeyBindSelection},
        settings::{state::BootstrapState, ArcSettings, ArcVk},
        with_i18n,
    },
    tokio::sync::watch,
};

#[derive(Debug, Clone)]
pub struct ArcRenderState {
    boot_changes: watch::Receiver<BootstrapState>,
    load_host: AddonHostSelection,
    update_host: AddonHostSelection,
    bindings: KeyBindSelection,
}

impl ArcRenderState {
    pub fn new() -> Self {
        let mut state = Self {
            boot_changes: BootstrapState::get().subscribe(),
            load_host: Default::default(),
            update_host: Default::default(),
            bindings: Default::default(),
        };
        state.sync_boot();
        state
    }

    pub fn sync_boot(&mut self) {
        let state = self.boot_changes.borrow_and_update();
        self.load_host.host = Some(state.addon_host_preference());
        self.update_host.host = state.update_host_preference();
    }

    pub fn ui_options(&mut self, ui: &imgui::Ui) {
        if self.boot_changes.has_changed().ok() == Some(true) {
            self.sync_boot();
        }

        let load_host_preference = match self.load_host.draw(ui, "preferred-loader") {
            None if ui.is_item_clicked_with_button(imgui::MouseButton::Right) => Some(None),
            host => host.map(Some),
        };
        if let Some(host) = load_host_preference {
            BootstrapState::write_with(|s| s.addon_host_preference = host);
        }

        let update_host_preference = match self.update_host.draw_opt(ui, "preferred-updater") {
            None if ui.is_item_clicked_with_button(imgui::MouseButton::Right) => Some(None),
            host => host.map(Some),
        };
        if let Some(host) = update_host_preference {
            BootstrapState::write_with(|s| s.update_host_preference = host);
        }

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
}
