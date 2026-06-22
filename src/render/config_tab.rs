use {
    super::TimerWindowState,
    crate::{
        controller::timers::{ProgressBarStyleChange, TimersController, TimersEvent},
        exports::runtime::{
            self as rt,
            bindings,
            imgui::{ComboBox, Condition, MouseButton, Selectable, Slider, TreeNode, TreeNodeFlags, Ui},
        },
        fl,
        render::{
            element::{keys::KeyBindSelection, language::LanguageSelection, token::ApiTokenInput},
            machine::RenderMachine,
            RenderEvent,
            RenderState,
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
    strum::IntoEnumIterator,
    tokio::sync::watch,
};
#[cfg(feature = "extension-nexus")]
use {
    crate::{exports::runtime::bindings::TaimiControls, settings::IconStyle},
    strum::VariantArray,
};

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
    pub quick_access_style: IconStyle,
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
            quick_access_style: Default::default(),
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
            RenderState::try_send(RenderEvent::InitiateQuit);
        }
        ui.same_line();
        if ui.button(&fl!("save")) {
            Controller::try_send(ControllerEvent::SaveSettings);
        }
        ui.dummy([4.0, 4.0]);

        #[cfg(feature = "extension-nexus")]
        let nexus_ui = || {
            use crate::{
                exports::nexus::{quick_access_add, quick_access_remove},
                QUICK_ACCESS_STATE,
            };

            if let Some(settings) = Settings::try_read() {
                self.quick_access_icons_visible = settings.quick_access_visible.clone();
                self.quick_access_style = settings.quick_access_style;
            }

            with_i18n("nexus-quick-access", |msg| ui.text(msg));
            let prior_visible = self.quick_access_icons_visible;
            let mut changed = false;
            for (i, icon) in TaimiControls::QUICK_ACCESS_ICONS.into_iter().enumerate() {
                let Some(keybind) = IconStyle::keybind_id(icon) else { continue };
                if i > 0 && i % 4 != 0 {
                    ui.same_line();
                }
                changed |= with_i18n!(keybind, |name| ui.checkbox_flags(
                    name,
                    &mut self.quick_access_icons_visible,
                    icon
                ));
            }
            let mut changed_icons = prior_visible ^ self.quick_access_icons_visible;
            let select_style = || {
                let mut selected = None;
                for &style in IconStyle::VARIANTS {
                    let is_current = style == self.quick_access_style;
                    if with_i18n!(style.name_id(), |label| Selectable::new(&label)
                        .selected(is_current)
                        .build(ui))
                    {
                        selected = Some(style);
                    }
                }
                selected
            };
            let current_style = self.quick_access_style.name_id();
            let style_selection = with_i18n!(current_style, |current| with_i18n!("icon-style", |label| {
                ComboBox::new(&label)
                    .preview_value(&current)
                    .build(ui, select_style)
            }))
            .flatten();
            if let Some(selection) = style_selection {
                self.quick_access_style = selection;
                changed_icons = self.quick_access_icons_visible;
                changed = true;
            }

            if changed {
                let state = QUICK_ACCESS_STATE.borrow().clone();
                let _ = Settings::write_with_blocking(|settings| {
                    settings.quick_access_visible = self.quick_access_icons_visible;
                    settings.quick_access_style = self.quick_access_style;
                });
                for icon in changed_icons {
                    let is_visible = self.quick_access_icons_visible.intersects(icon);
                    if !is_visible || style_selection.is_some() {
                        quick_access_remove(icon);
                    }
                    if is_visible {
                        quick_access_add(icon, state, self.quick_access_style);
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
            .build(ui, || self.draw_markers_window(ui, machine));
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

    fn draw_markers_window(&mut self, ui: &Ui, machine: &mut RenderMachine) {
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
                let select = with_i18n!(autoplace.label_ident(), |label| Selectable::new(label)
                    .selected(autoplace == self.marker_autoplace)
                    .build(ui));
                if select {
                    selected = Some(autoplace);
                }
            }
            selected
        };
        let current = self.marker_autoplace.label_ident();
        let autoplace = with_i18n!(("marker-trigger", current), |(label, current)| {
            ComboBox::new(&label)
                .preview_value(&current)
                .build(ui, autoplace_closure)
        });
        if let Some(Some(selection)) = autoplace {
            self.marker_autoplace = selection;
            MarkersController::try_send(MarkersEvent::MarkerAutoPlaceSettings(
                self.marker_autoplace.clone(),
            ));
        }
        if let Some(inner) = &self.marker_autoplace_inner {
            let autoplace_inner_closure = || {
                let mut selected = None;
                for autoplace_inner in SquadCondition::iter() {
                    let select = with_i18n!(autoplace_inner.label_ident(), |label| Selectable::new(label)
                        .selected(autoplace_inner == *inner)
                        .build(ui));
                    if select {
                        selected = Some(autoplace_inner);
                    }
                }
                selected
            };
            let condition = with_i18n!(("marker-condition", inner.label_ident()), |(label, current)| {
                ComboBox::new(&label)
                    .preview_value(&current)
                    .build(ui, autoplace_inner_closure)
            });
            if let Some(Some(selection)) = condition {
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
        if with_i18n!("dpi-scaling", |label| ui.checkbox(&label, &mut dpi_scaling)) {
            use taimi_meta::ui::MapCalibration;

            // TODO: controller event
            let _ = Settings::write_with_blocking(|settings| {
                settings.dpi_scaling = (!dpi_scaling).then_some(MapCalibration::DPI_REFERENCE);
                self.dpi_scaling = settings.dpi_scaling.clone();
            });
            machine.act_display_size();
        }
        with_i18n!("dpi-notice", |msg| ui.text_wrapped(&msg));
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
            if crate::exports::runtime::nexus_available()
                && with_i18n!("precise-markers", |label| ui.checkbox(&label, gamebind_invoke))
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
    channel_buffer: String,
    version_buffer: String,
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
            channel_buffer: String::new(),
            version_buffer: String::new(),
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
        self.channel_buffer.clone_from(&state.update_override_channel);
        self.version_buffer.clone_from(&state.update_override_version);
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
        let auto_update = with_i18n!("auto-update", |label| ui.combo(
            &label,
            &mut index,
            &UpdatePreference::OPTIONS,
            |option| with_i18n(option.as_str().to_lowercase().as_str(), |msg| msg.to_string()).into(),
        ));
        let mut new_pref = None;
        if auto_update {
            new_pref = UpdatePreference::OPTIONS.get(index).cloned();
        }

        if let Some(channel) = rt::update::crate_channel_build() {
            ui.text(&fl!("source-arg", source = channel));
            ui.same_line();
            ui.dummy([4.0, 0.0]);
            ui.same_line();
        }

        if with_i18n!("check-for-updates", |msg| ui.button(msg)) {
            Controller::try_send(ControllerEvent::CheckAddonUpdate(false));
        }
        let up_to_date = if let Some(latest) = self.remote_version_release.as_ref() {
            !latest.is_update(true)
        } else if let Some(latest) = &self.remote_version {
            rt::CRATE_VERSION == latest || crate::built_info::git_tag_name() == Some(latest)
        } else {
            false
        };
        #[cfg(feature = "extension-nexus")]
        match crate::exports::nexus::is_nexus_updater() {
            false if crate::exports::nexus::loaded() => {
                ui.same_line();
                let label = match up_to_date || self.remote_version_release.is_some() {
                    true => "update",
                    false => "attempt-update",
                };
                if with_i18n!(label, |msg| ui.button(msg)) {
                    Controller::try_send(ControllerEvent::CheckAddonUpdate(true));
                }
            },
            nexus_updater if !up_to_date => {
                ui.same_line();
                with_i18n!("update", |label| ui.text_disabled(&label));
                if ui.is_item_hovered() {
                    let msg = match nexus_updater {
                        true => "update-nexus-provider-notice",
                        false => "update-nexus-notice",
                    };
                    with_i18n!(msg, |msg| ui.tooltip_text(&msg));
                }
            },
            _ => (),
        }
        let blanket_auth = self.preference.blanket_authorization();
        let mut authorized = blanket_auth.unwrap_or(false);
        let auth_toggled = if let Some(latest) = &self.remote_version {
            ui.same_line();
            let up_to_date = if let Some(latest) = self.remote_version_release.as_ref() {
                !latest.is_update(true)
            } else if let Some(latest) = &self.remote_version {
                rt::CRATE_VERSION == latest || crate::built_info::git_tag_name() == Some(latest)
            } else {
                false
            };

            let channel_overridden = !self.channel_buffer.is_empty();
            let latest_version = match &self.remote_version_release {
                Some(release) => release.to_string(),
                _ => latest.clone(),
            };
            if up_to_date {
                ui.text(&latest_version);
                ui.same_line();
                if latest != &latest_version {
                    ui.text(&format!("({latest})"));
                    ui.same_line();
                }
                with_i18n!("update-not-required", |msg| ui.text(msg));
                if channel_overridden && ui.is_item_hovered() {
                    ui.tooltip_text("really want this update? override version to 0.0.1 or something old");
                }
                false
            } else {
                ui.text(fl!("update-available", version = latest_version));
                if blanket_auth.is_none() || channel_overridden {
                    authorized = self.preference.authorizes_version(latest).unwrap_or(false);
                    ui.same_line();
                    with_i18n!("update-allow", |msg| ui.checkbox(msg, &mut authorized))
                } else {
                    false
                }
            }
        } else {
            false
        };
        let advanced_node = with_i18n!("config-advanced", |label| TreeNode::new(&label)
            .flags(TreeNodeFlags::FRAMED)
            .opened(false, Condition::Once)
            .tree_push_on_open(true)
            .push(ui));
        if let Some(_node) = advanced_node {
            let gh_api_token = {
                let changed = self.gh_api_token.draw_input(ui, "gh-api-token");
                if !changed && ui.is_item_hovered() {
                    with_i18n!("gh-api-token-notice", |msg| ui.tooltip_text(&msg));
                }
                self.gh_api_token.draw_finish(ui, changed)
            };
            if let Some(token) = gh_api_token {
                BootstrapState::write_with(|state| {
                    state.gh_api_token = taimi_hoard::str_opt(token);
                });
            }
            ui.separator();
            let mut channel_override = ui
                .input_text("channel override", &mut self.channel_buffer)
                .auto_select_all(true)
                .enter_returns_true(true)
                .hint(rt::update::crate_channel_build().unwrap_or(rt::update::CHANNEL_RELEASE_NAME))
                .build();
            let channel_reset = ui.is_item_clicked_with_button(MouseButton::Right);
            if !channel_override && ui.is_item_hovered() {
                ui.tooltip_text("be careful!");
            }

            let version_override = ui
                .input_text("version override", &mut self.version_buffer)
                .auto_select_all(true)
                .enter_returns_true(true)
                .hint(&rt::update::addon_version_build())
                .build();
            let version_reset = ui.is_item_clicked_with_button(MouseButton::Right);
            if !version_override && ui.is_item_hovered() {
                ui.tooltip_text("be careful!");
            }
            if !self.channel_buffer.is_empty() {
                if with_i18n!("update-revert-mainline", |label| ui.button(&label)) {
                    self.channel_buffer.clear();
                    self.channel_buffer.push_str(rt::update::CHANNEL_RELEASE_NAME);
                    channel_override = true;
                }
                #[cfg(feature = "extension-nexus")]
                if ui.is_item_hovered() {
                    with_i18n!("update-revert-mainline-notice-nexus", |msg| ui.tooltip_text(&msg));
                }
            }
            if channel_reset {
                self.channel_buffer.clear();
            }
            if channel_override || channel_reset {
                rt::update::override_channel(self.channel_buffer.clone());
                if channel_override
                    && blanket_auth == Some(true)
                    && self.channel_buffer != rt::update::CHANNEL_RELEASE_NAME
                {
                    new_pref = new_pref.or(Some(UpdatePreference::ASK));
                }
            }
            if version_reset {
                self.version_buffer.clear();
            }
            if version_override || version_reset {
                let res = rt::log::error_ok(rt::update::try_override_version(&self.version_buffer));
                if res.is_none() {
                    self.version_buffer.clear();
                    self.changed.mark_changed();
                }
            }
        }
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
    }
}
