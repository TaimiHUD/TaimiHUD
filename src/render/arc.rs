use {
    crate::{
        controller::{Controller, ControllerEvent},
        exports::{
            arcdps::{self as exports, KeyIntercept},
            runtime::{self as rt, imgui},
        },
        game_language_id,
        settings::{ArcSettings, ArcUpdatePreference, ArcVk, Settings},
    },
    std::collections::HashMap,
    windows::Win32::UI::Input::KeyboardAndMouse,
};
#[cfg(feature = "space")]
use {
    crate::space::engine::{Engine, SpaceEvent},
    taimi_meta::ui::MapContext,
};

#[derive(Debug, Clone, Default)]
pub struct ArcRenderState {
    binding_buffers: HashMap<&'static str, BindingState>,
}

impl ArcRenderState {
    pub fn ui_options(&mut self, ui: &imgui::Ui) {
        ui.text("WORK IN PROGRESS");

        if let Ok(pref) = exports::update_preference() {
            self.ui_options_update(ui, pref);
        }

        ui.new_line();
        self.ui_options_keybinds(ui);

        self.ui_options_language(ui);
    }

    fn ui_options_update(&mut self, ui: &imgui::Ui, pref: ArcUpdatePreference) {
        let mut index = ArcUpdatePreference::OPTIONS.iter().position(|opt| opt == &pref.as_option())
            .unwrap_or(0);
        let auto_update = ui.combo("Auto-update", &mut index, &ArcUpdatePreference::OPTIONS, |option| {
            option.as_str().into()
        });
        let mut new_pref = None;
        if auto_update {
            new_pref = ArcUpdatePreference::OPTIONS.get(index).cloned();
        }
        if ui.button("Check now") {
            log::debug!("TODO: update check");
            let _ = exports::update_url();
        }
        let blanket_auth = pref.blanket_authorization();
        let mut authorized = blanket_auth.unwrap_or(false);
        let auth_toggled = Settings::try_read().and_then(|s| s.arc().update_remote_version.as_ref().map(|latest| {
            ui.same_line();
            if latest == rt::CRATE_VERSION {
                ui.text("Up-to-date");
                None
            } else if blanket_auth.is_none() {
                authorized = pref.authorizes_version(latest).unwrap_or(false);
                ui.checkbox(format!("Allow update to {latest}"), &mut authorized)
                    .then(|| latest.clone())
            } else {
                ui.text("Update available: {latest}");
                None
            }
        })).flatten();
        if auth_toggled.is_some() || new_pref.is_some() {
            let _ = Settings::write_with_blocking(|settings| {
                let arc = settings.arc_mut();
                let pref = match new_pref {
                    Some(pref) =>
                        arc.update_preference.insert(pref),
                    None =>
                        arc.update_preference.get_or_insert_with(|| exports::default_update_preference()),
                };
                if let Some(latest) = auth_toggled {
                    pref.authorize_update(latest, authorized);
                }
            });
        }
    }

    fn ui_options_keybinds(&mut self, ui: &imgui::Ui) {
        for &binding in ArcSettings::VK_WINDOWS {
            self.keybind_ui(ui, binding, Some(|vk: &ArcVk| if let Some(window) = vk.window_name() {
                crate::control_window(window, None);
            }));
        }
        #[cfg(feature = "space")]
        if Engine::is_available() {
            ui.separator();
            self.keybind_ui(ui, &ArcSettings::VK_RENDER_TOGGLE_PATHING, Some(|_vk: &ArcVk| Engine::try_send(SpaceEvent::PathingToggle)));
            ui.separator();
            self.keybind_ui(ui, &ArcSettings::VK_RENDER_TOGGLE_PATHING_MINIMAP, Some(|_vk: &ArcVk| Engine::try_send(SpaceEvent::MapToggle(MapContext::Minimap))));
            ui.separator();
            self.keybind_ui(ui, &ArcSettings::VK_RENDER_TOGGLE_PATHING_MAP, Some(|_vk: &ArcVk| Engine::try_send(SpaceEvent::MapToggle(MapContext::Global))));
        }
        ui.separator();
        for binding in &ArcSettings::VK_TIMER_TRIGGERS {
            self.keybind_ui(ui, binding, Some(|vk: &ArcVk| Controller::try_send(ControllerEvent::TimerKeyTrigger(vk.id.into(), false))));
        }
    }

    fn ui_options_language(&mut self, ui: &imgui::Ui) {
        let selected_language = exports::game_language()
            .map(game_language_id)
            .unwrap_or("");
        if let Some(languages) = ui.begin_combo("Language", selected_language) {
            let mut new_language = None;
            for l in crate::LANGUAGES_GAME {
                let id = game_language_id(l);
                let selected = imgui::Selectable::new(id)
                    .selected(selected_language == id)
                    .build(ui);
                if selected {
                    new_language = Some(Ok(l));
                }
            }
            for id in crate::LANGUAGES_EXTRA {
                let selected = imgui::Selectable::new(id)
                    .selected(selected_language == id)
                    .build(ui);
                if selected {
                    new_language = Some(Err(id));
                }
            }
            languages.end();

            if let Some(new_language) = new_language {
                log::warn!("TODO: language selection");
            }
        }
    }

    fn keybind_ui<F: FnOnce(&ArcVk)>(&mut self, ui: &imgui::Ui, vk: &'static ArcVk, action: Option<F>) {
        let _id_token = ui.push_id(vk.id);
        let name = vk.get_name();
        match action {
            Some(action) => if ui.button(name) {
                action(vk)
            },
            None => ui.text(name),
        }
        ui.same_line();

        let default_vk = vk.vkeycode_default();
        let default_name = default_vk.and_then(|vk| rt::keyboard::vk_name(vk).ok());

        let changed = {
            use std::collections::hash_map::Entry;

            let any_configuring = self.binding_buffers.values().any(|b| b.configuring);

            let binding_buffer = self.binding_buffers.entry(vk.id);
            let is_fresh = match &binding_buffer {
                Entry::Vacant(..) => true,
                Entry::Occupied(b) => b.get().name_buffer.is_empty(),
            };
            let binding_buffer = binding_buffer.or_default();
            if is_fresh {
                if let Some(current_vk) = vk.get_setting_vkeycode() {
                    use std::fmt::Write;

                    let current_name = rt::keyboard::vk_name(current_vk);
                    let _ = if let Ok(name) = current_name {
                        write!(&mut binding_buffer.name_buffer, "{name}")
                    } else {
                        write!(&mut binding_buffer.name_buffer, "{}", current_vk.0)
                    };
                } else {
                    binding_buffer.name_buffer = "(unbound)".into();
                }
            }
            let input = ui.input_text("Keybind", &mut binding_buffer.name_buffer)
                .read_only(binding_buffer.configuring)
                .auto_select_all(true)
                .always_insert_mode(true)
                .enter_returns_true(true)
                .no_undo_redo(true)
                .no_horizontal_scroll(true);
            let changed = match (default_name, default_vk) {
                (Some(name), _) => input.hint(name.to_string()),
                (None, Some(vk)) => input.hint(format!("{}", vk.0)),
                (None, None) => input.hint("unbound by default".into()),
            }.build();

            ui.same_line();
            let mut filtered = true;
            if ui.checkbox("mod-filtered", &mut filtered) {
                log::warn!("TODO: unfiltered keybinds");
            }

            match (binding_buffer.configuring, any_configuring) {
                _ if changed => match binding_buffer.name_buffer.parse::<u16>() {
                    Ok(new) => {
                        log::debug!("updating {} keybind to: {new:#x}", vk.id);
                        Some(KeyboardAndMouse::VIRTUAL_KEY(new))
                    },
                    Err(_) => {
                        log::warn!("TODO: update {} keybind to: {binding_buffer:?}", vk.id);
                        None
                    },
                },
                (true, _) => {
                    ui.same_line();
                    ui.text_disabled("press a key");
                    match KeyIntercept::intercept_take() {
                        None => {
                            log::info!("key bind cancelled");
                            binding_buffer.configuring = false;
                            None
                        },
                        Some(KeyIntercept::Pending) => None,
                        Some(KeyIntercept::Intercepted { key }) => {
                            log::debug!("got key bind: {key:?}");
                            match key.down {
                                false => {
                                    KeyIntercept::intercept_restart();
                                    None
                                },
                                true => {
                                    binding_buffer.configuring = false;
                                    binding_buffer.name_buffer.clear();
                                    if !key.mods.is_empty() {
                                        log::info!("TODO: key bind mods");
                                    }
                                    Some(key.vk)
                                },
                            }
                        },
                    }
                },
                (false, false) => {
                    ui.same_line();
                    debug_assert!(!KeyIntercept::intercept_ready());
                    if ui.button("bind") {
                        KeyIntercept::intercept_restart();
                        binding_buffer.configuring = true;
                    }
                    None
                },
                (false, true) => None,
            }
        };

        if let Some(new) = changed {
            if let Err(e) = vk.set_vkeycode(new) {
                log::error!("saving keybind {} failed: {}", vk.id, e);
            }
        }
    }
}

#[derive(Debug, Default, Clone)]
struct BindingState {
    name_buffer: String,
    configuring: bool,
}
