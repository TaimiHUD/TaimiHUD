use {
    super::TimerWindowState,
    crate::{
        controller::timers::{ProgressBarStyleChange, TimersController, TimersEvent},
        exports::runtime::{self as rt, bindings},
        fl,
        render::{
            element::{keys::KeyBindSelection, language::LanguageSelection, token::ApiTokenInput},
            machine::RenderMachine,
            RenderEvent,
            TextFont,
        },
        settings::{
            state::{BootstrapState, SaveState},
            MarkerAutoPlaceSettings,
            Settings,
            SquadCondition,
        },
        with_i18n,
        Controller,
        ControllerEvent,
        MarkersController,
        MarkersEvent,
    },
    anyhow::Context,
    nexus::imgui::{ComboBox, Condition, Selectable, Slider, TreeNode, TreeNodeFlags, Ui},
    strum::IntoEnumIterator,
    tokio::sync::watch,
};

#[cfg(feature = "extension-nexus")]
use crate::exports::runtime::bindings::TaimiControls;
#[cfg(feature = "updates")]
use crate::{
    exports::runtime::update::ResolvedVersion,
    settings::state::{AddonHostName, UpdatePreference},
};

pub struct ConfigTabState {
    pub bindings: KeyBindSelection,
    pub language: LanguageSelection,
    pub save_changed: watch::Receiver<SaveState>,
    pub marker_autoplace: MarkerAutoPlaceSettings,
    pub marker_autoplace_inner: Option<SquadCondition>,
    pub dpi_scaling: Option<f32>,
    pub gamebind_invoke: Option<bool>,
    #[cfg(feature = "extension-nexus")]
    pub quick_access_icons_visible: TaimiControls,
    #[cfg(feature = "updates")]
    pub update_state: ConfigUpdateState,
}

impl ConfigTabState {
    pub fn new() -> Self {
        Self {
            bindings: Default::default(),
            language: Default::default(),
            save_changed: SaveState::get().subscribe(),
            dpi_scaling: Default::default(),
            marker_autoplace: Default::default(),
            marker_autoplace_inner: Default::default(),
            gamebind_invoke: Default::default(),
            #[cfg(feature = "extension-nexus")]
            quick_access_icons_visible: TaimiControls::default_quick_access(),
            #[cfg(feature = "updates")]
            update_state: ConfigUpdateState::new(),
        }
    }

    pub fn draw(
        &mut self,
        ui: &Ui,
        machine: &mut RenderMachine,
        timer_window_state: &mut TimerWindowState,
    ) {
        if self.save_changed.has_changed().ok() == Some(true) {
            self.bindings.clear_dirty();
        }

        ui.text_wrapped(&fl!("imgui-notice"));
        ui.dummy([4.0, 4.0]);
        ui.text_wrapped(&fl!("keybind-triggers"));
        ui.dummy([4.0, 4.0]);

        if ui.button(&fl!("quit")) {
            // XXX: this will wipe all of render state, rather than just katrender
            let mut render_sender = crate::RENDER_SENDER.write().unwrap();
            let render_quit = render_sender
                .as_ref()
                .map(|sender| sender.try_send(RenderEvent::Quit));
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
            use crate::exports::nexus::{quick_access_add, quick_access_button_id, quick_access_remove};

            if let Some(settings) = Settings::try_read() {
                self.quick_access_icons_visible = settings.quick_access_visible.clone();
            }

            with_i18n("nexus-quick-access", |msg| ui.text(msg));
            let prior_visible = self.quick_access_icons_visible;
            let mut changed = false;
            for (i, icon) in TaimiControls::QUICK_ACCESS_ICONS.into_iter().enumerate() {
                let Some((_, _, _, _, keybind)) = quick_access_button_id(icon) else {
                    continue
                };
                if i > 0 && i % 4 != 0 {
                    ui.same_line();
                }
                changed |= with_i18n!(keybind, |name| ui.checkbox_flags(
                    name,
                    &mut self.quick_access_icons_visible,
                    icon
                ));
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
        #[cfg(feature = "extension-nexus")]
        let _nexus_ui = TreeNode::new(&fl!("nexus"))
            .flags(TreeNodeFlags::FRAMED)
            .opened(crate::exports::nexus::available(), Condition::Once)
            .tree_push_on_open(true)
            .build(ui, nexus_ui);
        #[cfg(feature = "updates")]
        let _update = TreeNode::new(&fl!("update"))
            .flags(TreeNodeFlags::FRAMED)
            .opened(
                self.update_state.preference.will_authorize() != Some(false),
                Condition::Once,
            )
            .tree_push_on_open(true)
            .build(ui, || self.update_state.draw(ui));

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
                        },
                        MarkerAutoPlaceSettings::Place(ref mut t) => {
                            *t = selection.clone();
                        },
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
                TimersController::try_send(TimersEvent::ProgressBarStyle(ProgressBarStyleChange::Stock(
                    timer_window_state.progress_bar.stock,
                )));
            };
            if ui.checkbox(&fl!("shadow"), &mut timer_window_state.progress_bar.shadow) {
                TimersController::try_send(TimersEvent::ProgressBarStyle(ProgressBarStyleChange::Shadow(
                    timer_window_state.progress_bar.shadow,
                )));
            }
            if ui.checkbox(
                &fl!("centre-text-after-icon"),
                &mut timer_window_state.progress_bar.centre_after,
            ) {
                TimersController::try_send(TimersEvent::ProgressBarStyle(ProgressBarStyleChange::Centre(
                    timer_window_state.progress_bar.centre_after,
                )));
            }
            if Slider::new(&fl!("height"), 8.0, 256.0)
                .display_format("%.0f")
                .build(ui, &mut timer_window_state.progress_bar.height)
            {
                TimersController::try_send(TimersEvent::ProgressBarStyle(ProgressBarStyleChange::Height(
                    timer_window_state.progress_bar.height,
                )));
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
        let _language = TreeNode::new(&fl!("language"))
            .flags(TreeNodeFlags::FRAMED)
            .opened(!self.language.is_default(), Condition::Once)
            .tree_push_on_open(true)
            .build(ui, || self.draw_language(ui));
        let _gamebinds = with_i18n!("gamebinds", |msg| TreeNode::new(&msg)
            .flags(TreeNodeFlags::FRAMED)
            .opened(true, Condition::Once)
            .tree_push_on_open(true)
            .build(ui, || self.draw_gamebinds(ui)));
    }

    pub fn draw_language(&mut self, ui: &Ui) {
        if let Some(language_id) = self.language.draw(ui) {
            let res = crate::load_language(language_id)
                .map_err(anyhow::Error::msg)
                .with_context(|| format!("loading i18n for {language_id}"));
            if let Err(e) = res {
                log::error!("{e:#}");
            }
            BootstrapState::write_with(|state| state.language = Some(language_id.into()));
        }
    }

    pub fn draw_gamebinds(&mut self, ui: &Ui) {
        with_i18n!("gamebind-notice", |msg| ui.text_wrapped(msg));

        self.bindings.do_gamebinds(ui, bindings::interesting_controls());
        ui.separator();
        #[cfg(feature = "extension-nexus")]
        if self.gamebind_invoke.is_none() {
            self.gamebind_invoke = Settings::try_read().map(|s| s.arc().gamebind_invoke.is_some());
        }
        #[cfg(feature = "extension-nexus")]
        if let Some(gamebind_invoke) = &mut self.gamebind_invoke {
            // TODO: InvokeMethod dropdown
            if crate::exports::runtime::nexus_available() && ui.checkbox("Precise Markers", gamebind_invoke)
            {
                let _ = Settings::write_with_blocking(|settings| {
                    settings.arc_mut().gamebind_invoke = gamebind_invoke.then_some(Default::default())
                });
            }
        }
        if self.gamebind_invoke == Some(true) || !rt::nexus_available() {
            self.bindings.do_gamebinds(ui, bindings::interesting_keybinds());
        }
    }
}

#[cfg(feature = "updates")]
pub struct ConfigUpdateState {
    host_preference: Option<AddonHostName>,
    preference: UpdatePreference,
    remote_version: Option<String>,
    remote_version_release: Option<ResolvedVersion>,
    gh_api_token: ApiTokenInput,
    changed: watch::Receiver<BootstrapState>,
}

#[cfg(feature = "updates")]
impl ConfigUpdateState {
    pub fn new() -> Self {
        let mut state = Self {
            host_preference: None,
            preference: UpdatePreference::ASK,
            remote_version: Default::default(),
            remote_version_release: Default::default(),
            gh_api_token: ApiTokenInput::new(),
            changed: BootstrapState::get().subscribe(),
        };
        state.sync_state();
        state
    }

    pub fn sync_state(&mut self) {
        self.preference = rt::update::Updater::get_preference();
        let state = self.changed.borrow_and_update();
        self.host_preference = state.update_host_preference().clone();
        //self.preference = state.update_preference().clone();
        self.remote_version = state.update_remote_version.clone();
        self.gh_api_token.update_preview(state.gh_api_token.is_some());
        self.remote_version_release = self
            .remote_version
            .clone()
            .and_then(|v| ResolvedVersion::with_version_id(v).ok());
    }

    fn draw(&mut self, ui: &Ui) {
        if self.changed.has_changed().ok() == Some(true) {
            self.sync_state();
        }
        let mut index = UpdatePreference::OPTIONS
            .iter()
            .position(|opt| opt == &self.preference.as_option())
            .unwrap_or(0);
        let auto_update = ui.combo("Auto-update", &mut index, &UpdatePreference::OPTIONS, |option| {
            option.as_str().into()
        });
        let mut new_pref = None;
        if auto_update {
            new_pref = UpdatePreference::OPTIONS.get(index).cloned();
        }
        if let Some(channel) = rt::update::crate_channel() {
            ui.text(&fl!("source-arg", source = channel));
            ui.same_line();
            ui.dummy([4.0, 0.0]);
            ui.same_line();
        }
        if with_i18n!("check-for-updates", |msg| ui.button(msg)) {
            Controller::try_send(ControllerEvent::CheckAddonUpdate(false));
        }
        let up_to_date = if let Some(latest) = self.remote_version_release.as_ref() {
            latest.is_update()
        } else if let Some(latest) = &self.remote_version {
            rt::CRATE_VERSION == latest || crate::built_info::git_tag_name() == Some(latest)
        } else {
            false
        };
        #[cfg(feature = "extension-nexus")]
        if !crate::built_info::IS_TAGGED_RELEASE && !up_to_date && rt::nexus_available() {
            ui.same_line();
            if with_i18n!("update", |msg| ui.button(msg)) {
                Controller::try_send(ControllerEvent::CheckAddonUpdate(true));
            }
        }
        let blanket_auth = self.preference.blanket_authorization();
        let mut authorized = blanket_auth.unwrap_or(false);
        let auth_toggled = if let Some(latest) = &self.remote_version {
            ui.same_line();
            let up_to_date = if let Some(latest) = self.remote_version_release.as_ref() {
                latest.is_update()
            } else if let Some(latest) = &self.remote_version {
                rt::CRATE_VERSION == latest || crate::built_info::git_tag_name() == Some(latest)
            } else {
                false
            };

            if up_to_date {
                with_i18n!("update-not-required", |msg| ui.text(msg));
                false
            } else {
                let latest_version = match &self.remote_version_release {
                    Some(release) => release.to_string(),
                    _ => latest.clone(),
                };
                ui.text(fl!("update-available", version = latest_version));
                if blanket_auth.is_none() {
                    authorized = self.preference.authorizes_version(latest).unwrap_or(false);
                    ui.same_line();
                    with_i18n!("update", |msg| ui.checkbox(msg, &mut authorized))
                } else {
                    false
                }
            }
        } else {
            false
        };
        if auth_toggled || new_pref.is_some() {
            BootstrapState::write_with(|state| {
                let pref = match new_pref {
                    Some(pref) => state.update_preference.insert(pref),
                    None => state
                        .update_preference
                        .get_or_insert_with(|| UpdatePreference::ASK),
                };
                if let Some(latest) = auth_toggled.then_some(self.remote_version.as_ref()).flatten() {
                    pref.authorize_update(latest.clone(), authorized);
                }
            });
        }

        let gh_api_token = {
            let changed = self.gh_api_token.draw_input(ui, "gh-api-token");
            if !changed && ui.is_item_hovered() {
                with_i18n!("gh-api-token-notice", |msg| ui.tooltip_text(&msg));
            }
            self.gh_api_token.draw_finish(ui, changed)
        };
        if let Some(token) = gh_api_token {
            BootstrapState::write_with(|state| {
                state.gh_api_token = match token.is_empty() {
                    false => Some(token),
                    true => None,
                };
            });
        }
    }
}
