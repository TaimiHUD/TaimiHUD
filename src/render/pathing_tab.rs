use {
    crate::{
        controller::{
            api::ApiController,
            pathing::{PathingController, PathingEnables, PathingEvent},
            Controller,
        },
        fl,
        render::{machine::RenderMachine, RenderEvent, RenderState},
        settings::{
            pathing::{CameraSource, PathingSettings, SpaceSettings, TriggerKind},
            Settings,
        },
        with_i18n,
        LANGUAGE_LOADER,
    },
    anyhow::Context,
    nexus::imgui::{
        self,
        ChildWindow,
        ComboBox,
        Condition,
        Selectable,
        Slider,
        TreeNode,
        TreeNodeFlags,
        Ui,
        WindowFlags,
    },
    std::collections::HashMap,
    strum::VariantArray,
    taimi_pack::attributes::Festival,
    taimi_sync::watched::Watched,
};
#[cfg(feature = "paths-interact")]
use crate::controller::pathing::InteractMessage;

#[cfg(feature = "goggles")]
use crate::space::engine::{Engine, SpaceEvent};

pub struct PathingConfig {
    enables: Watched<PathingEnables>,
}

impl PathingConfig {
    pub fn new() -> Self {
        let mut state = Self { enables: Watched::EMPTY };
        Controller::with_sender(|s| {
            if let Some(p) = &s.pathing {
                state.enables.restart_watching(&p.enables);
            }
        });
        state
    }

    fn katrender(&self) -> bool {
        self.enables.get().contains(PathingEnables::KATRENDER)
    }

    pub fn draw(
        &mut self,
        ui: &Ui,
        machine: &mut RenderMachine,
        _state_errors: &mut HashMap<String, anyhow::Error>,
    ) {
        let _ = self.enables.try_read_mut();
        ui.columns(2, "pathing_tab_start", true);

        self.draw_header(ui);

        let opts_primary = || {
            let available = Engine::is_available();
            if !available && self.katrender() {
                Self::draw_space_error(ui, machine, None);
            }

            self.draw_pathing_opts(ui, machine);
            if available && self.katrender() {
                ui.separator();
                let label = fl!("pathing-window");
                if ui.button(&label) {
                    crate::control_window(crate::WINDOW_PATHING, None);
                }
            }

            with_i18n!("experimental-notice", |msg| ui.text_wrapped(&msg));

            available.then_some(())
        };

        let child_window_flags = WindowFlags::HORIZONTAL_SCROLLBAR;
        let _active = ChildWindow::new("pathing_main")
            .flags(child_window_flags)
            .size([0.0, 0.0])
            .build(ui, opts_primary);

        ui.next_column();

        let opts_secondary = || {
            self.draw_map_opts(ui);

            #[cfg(feature = "goggles")]
            if let Some(Some(..)) = _active {
                let _goggles = TreeNode::new(&fl!("pathing-config-goggles"))
                    .flags(TreeNodeFlags::FRAMED)
                    .opened(false, Condition::Once)
                    .tree_push_on_open(false)
                    .build(ui, || {
                        let _id = ui.push_id("goggles");
                        Self::draw_goggles_opts(ui, machine)
                    });
            }
        };

        ChildWindow::new("pathing_secondary")
            .size([0.0, 0.0])
            .build(ui, opts_secondary);

        ui.columns(1, "pathing_tab_end", false)
    }

    pub fn draw_space_error(ui: &Ui, machine: &RenderMachine, e: Option<&anyhow::Error>) {
        let _font = RenderState::push_font("big", ui);
        let e = match e {
            None if !Settings::try_read().map(|s| s.enable_katrender).unwrap_or(true) => {
                {
                    let _notice = RenderState::push_font("ui", ui);
                    ui.text_wrapped(&fl!("experimental-notice"));
                }
                if ui.button(&fl!("enable")) {
                    PathingController::try_send(PathingEvent::ToggleKatRender);
                }
                None
            },
            None => {
                if machine.gameplay.is_initial() {
                    with_i18n!("render-notice-gameplay-initial", |msg| ui.text_wrapped(&msg));
                } else if !Engine::is_available() {
                    with_i18n!("render-notice-gameplay", |msg| ui.text_wrapped(&msg));
                } else {
                    // shouldn't happen?
                }
                None
            },
            Some(e) => {
                if !Engine::is_available() {
                    ui.text_wrapped(&fl!("render-notice-error"));
                }
                Some(e)
            },
        };
        if let Some(e) = e {
            ui.text_wrapped(format!("{e:#}"));
        }
    }

    fn draw_header(&mut self, ui: &Ui) {
        {
            let _font = (!self.katrender()).then(|| RenderState::push_font("big", ui));
            let enables = self.enables.get_mut();
            if ui.checkbox_flags(&fl!("pathing-config-enable"), enables, PathingEnables::KATRENDER) {
                PathingController::try_send(PathingEvent::ToggleKatRender);
            }
        }

        if self.katrender() {
            ui.same_line();
            if ui.button(&fl!("render-unload")) {
                let _disabled = Settings::write_with_blocking(|settings| {
                    settings.enable_katrender = false;
                });
                if _disabled.is_ok() {
                    RenderState::try_send(RenderEvent::ReloadAll);
                }
            }

            ui.same_line();
            if ui.button(&fl!("render-reload")) {
                RenderState::try_send(RenderEvent::ReloadAll);
            }
        } else {
            //RenderState::font_text("ui", ui, &fl!("pathing-config"));
            ui.text_disabled(&fl!("pathing-notice-space"));
        }
    }

    fn combo_setting<T>(ui: &Ui, label: &str, value: T) -> Option<Option<T>>
    where
        T: VariantArray + Eq + Copy + Into<&'static str>,
    {
        let _token = ui.push_id(label);
        let current = LANGUAGE_LOADER.get(value.into());
        let draw = || {
            let mut selected = None;
            for &opt in T::VARIANTS {
                let name = LANGUAGE_LOADER.get(opt.into());
                let selection = Selectable::new(name).selected(value == opt);
                if selection.build(ui) {
                    selected = Some(opt.clone());
                }
            }
            selected
        };
        let changed = ComboBox::new(label).preview_value(&current).build(ui, draw);
        match changed {
            Some(Some(selected)) => Some(Some(selected)),
            None if ui.is_item_clicked_with_button(imgui::MouseButton::Right) =>
            // reset to default
                Some(None),
            _ => None,
        }
    }

    fn slider_setting(ui: &Ui, label: &str, value: f32, range: (f32, f32)) -> Option<Option<f32>> {
        Self::slider_setting_inner(ui, label, value, range, None)
    }
    fn slider_setting_int(ui: &Ui, label: &str, value: i32, (min, max): (i32, i32)) -> Option<Option<i32>> {
        let range = (min as f32, max as f32);
        match Self::slider_setting_inner(ui, label, value as f32, range, Some("%.0f")) {
            Some(Some(new)) => Some(Some(new as i32)),
            Some(None) => Some(None),
            None => None,
        }
    }
    fn slider_setting_inner(
        ui: &Ui,
        label: &str,
        mut value: f32,
        (min, max): (f32, f32),
        fmt: Option<&str>,
    ) -> Option<Option<f32>> {
        let changed = Slider::new(label, min, max);
        let changed = match fmt {
            Some(fmt) => changed.display_format(fmt),
            None => changed,
        };
        match changed.build(ui, &mut value) {
            true => Some(Some(value)),
            false => {
                if ui.is_item_clicked_with_button(imgui::MouseButton::Right) {
                    // reset to default
                    return Some(None)
                }
                None
            },
        }
    }

    fn slider_opt_setting(
        ui: &Ui,
        label: &str,
        value: Option<f32>,
        range: (f32, f32),
    ) -> Option<Option<f32>> {
        Self::slider_opt_setting_with_initial(ui, label, value, range, None)
    }

    fn slider_opt_setting_or_min(
        ui: &Ui,
        label: &str,
        value: Option<f32>,
        range: (f32, f32),
    ) -> Option<Option<f32>> {
        Self::slider_opt_setting_with_initial(ui, label, value, range, Some(range.0))
    }

    fn slider_opt_setting_with_initial(
        ui: &Ui,
        label: &str,
        value: Option<f32>,
        range: (f32, f32),
        initial: Option<f32>,
    ) -> Option<Option<f32>> {
        let mut enabled = value.is_some();
        let _token = ui.push_id(label);
        ui.unindent();
        let mut res = if ui.checkbox("", &mut enabled) {
            Some(match enabled {
                false if initial.is_some() => None,
                false => Some(SpaceSettings::NONE_F32),
                true => initial,
            })
        } else {
            None
        };

        ui.same_line();
        if let Some(value) = value {
            res = res.or(Self::slider_setting(ui, label, value, range));
        } else {
            ui.label_text(label, &fl!("disabled"));
        }
        ui.indent();
        res
    }

    fn set_pathing<F: FnOnce(&mut PathingSettings)>(f: F) {
        let res = Settings::write_with_blocking(|s| f(s.pathing_mut()))
            .context("failed to save pathing settings");
        match res {
            Ok(()) => Engine::try_send(SpaceEvent::SettingsDirty),
            Err(e) => log::warn!("{e:#}"),
        }
    }
    fn get_pathing<R, F: FnOnce(&PathingSettings) -> R>(f: F) -> Option<R> {
        //Settings::try_read().map(|s|
        Settings::read_with_blocking(|s| f(&s.pathing())).ok()
    }

    const RANGE_ALPHA: (f32, f32) = (0.0, 1.0);
    const RANGE_SCALE: (f32, f32) = (0.0, 25.0);
    //const RANGE_SCALE_POI: (f32, f32) = (-1.0, 10.0);
    const RANGE_SCALE_POI: (f32, f32) = (0.0, 5.0);
    const RANGE_SCALE_MAP: (f32, f32) = Self::RANGE_SCALE;
    fn draw_pathing_opts(&mut self, ui: &Ui, machine: &mut RenderMachine) -> Option<()> {
        let (
            load_simultaneous,
            camera_source,
            mut visible_space,
            player_overlap_threshold,
            distance_fade_intensity,
            distance_max,
            trail_y_offset,
            trail_resolution,
            trail_width,
            mut trail_textured_space,
            trail_alpha,
            scale_trail_space,
            scale_poi_space,
            edge_feather_scale,
            (edge_scale,),
        ) = Self::get_pathing(|s| {
            (
                s.load_simultaneous(),
                s.space.camera_source(),
                s.space.visible_space(),
                s.space.player_overlap_threshold(),
                s.space.distance_fade_intensity(),
                s.space.distance_max(),
                s.space.trail_y_offset(),
                s.space.trail_resolution(),
                s.space.trail_width(),
                s.space.trail_textured_space(),
                s.space.trail_alpha(), // s.space.poi_alpha(),
                s.space.trail_scale_space(),
                s.space.poi_scale_space(),
                s.space.edge_feather_scale(),
                match () {
                    #[cfg(feature = "goggles")]
                    _ => (s.space.goggles.edge_scale(),),
                    #[cfg(not(feature = "goggles"))]
                    _ => ((),),
                },
            )
        })?;

        if ui.checkbox(&fl!("pathing-render-toggle"), &mut visible_space) {
            Self::set_pathing(|s| s.space.visible_space = Some(visible_space));
            #[cfg(feature = "goggles")]
            Engine::try_send(match visible_space {
                true => SpaceEvent::GogglesRefreshLens { force: false, delay_override: Some(2) },
                false => SpaceEvent::GogglesClearLens,
            });
        }
        ui.same_line();
        if ui.checkbox(&fl!("pathing-config-textured"), &mut trail_textured_space) {
            Self::set_pathing(|s| s.space.trail_textured_space = Some(trail_textured_space));
        }

        with_i18n!("pathing-config-reset-notice", |msg| ui.text_wrapped(&msg));

        ui.indent();
        if let Some(value) = Self::slider_setting(
            ui,
            &fl!("pathing-config-trail-alpha"),
            trail_alpha,
            Self::RANGE_ALPHA,
        ) {
            Self::set_pathing(|s| s.space.trail_alpha = value);
        }
        if let Some(value) = Self::slider_setting(
            ui,
            &fl!("pathing-config-trail-scale"),
            scale_trail_space,
            Self::RANGE_SCALE,
        ) {
            Self::set_pathing(|s| s.space.scale_trail_space = value);
        }
        #[cfg(todo)]
        if let Some(value) =
            Self::slider_setting(ui, &fl!("pathing-config-poi-alpha"), poi_alpha, Self::RANGE_ALPHA)
        {
            Self::set_pathing(|s| s.space.poi_alpha = value);
        }
        if let Some(value) = Self::slider_setting(
            ui,
            &fl!("pathing-config-poi-scale"),
            scale_poi_space,
            Self::RANGE_SCALE_POI,
        ) {
            Self::set_pathing(|s| s.space.scale_poi_space = value);
        }
        if let Some(value) = Self::slider_opt_setting(
            ui,
            &fl!("pathing-config-distance-fade-intensity"),
            distance_fade_intensity,
            (1.0, 500.0),
        ) {
            Self::set_pathing(|s| s.space.distance_fade_intensity = value);
        }
        if let Some(value) = Self::slider_opt_setting(
            ui,
            &fl!("pathing-config-player-overlap-threshold"),
            player_overlap_threshold,
            (0.01, 1000.0),
        ) {
            Self::set_pathing(|s| s.space.player_overlap_threshold = value);
        }
        if let Some(value) =
            Self::slider_opt_setting(ui, "edge feather scale", edge_feather_scale, (0.001f32, 5.0))
        {
            Self::set_pathing(|s| s.space.edge_feather_scale = value);
        }
        #[cfg(feature = "goggles")]
        if let Some(value) =
            Self::slider_opt_setting_or_min(ui, "corner boundary scale", edge_scale, (0.1f32, 5.0))
        {
            let mut edge_scale = value;
            Self::set_pathing(|s| {
                s.space.goggles.edge_scale = value;
                edge_scale = s.space.goggles.edge_scale();
            });
            Engine::try_send(SpaceEvent::RefreshEdgeScale);
        }
        if let Some(value) = Self::slider_setting(
            ui,
            &fl!("pathing-config-distance-max"),
            distance_max,
            (1.0, 2000.0),
        ) {
            Self::set_pathing(|s| s.space.distance_max = value);
        }
        ui.unindent();
        #[cfg(feature = "extension-nexus")]
        if let Some(value) = Self::combo_setting(ui, &fl!("pathing-config-camera-source"), camera_source) {
            Self::set_pathing(|s| s.space.camera_source = value);
            if value == Some(CameraSource::RealTimeAPI) {
                match machine.rtapi_init() {
                    Err(e) => log::warn!("{e:#}"),
                    Ok(false) =>
                        log::warn!("RTAPI inactive - make sure the addon is installed and loaded by Nexus"),
                    Ok(true) => (),
                }
            }
        }
        #[cfg(feature = "extension-nexus")]
        match camera_source {
            CameraSource::MumbleLink => ui.text_wrapped(
                "if you experience stuttering, try changing Vertical Sync under the in-game graphical settings",
            ),
            CameraSource::RealTimeAPI => {
                if machine.rtapi.is_none() {
                    ui.text_wrapped(
                        "RTAPI is a separate addon that must be installed via Nexus"
                    );
                }
                ui.text_wrapped(
                    "if you experience stuttering, try changing Vertical Sync or switching to MumbleLink",
                );
            },
        }

        let filters_tree = with_i18n!("pathing-config-filters", |label| TreeNode::new(&label)
            .flags(TreeNodeFlags::FRAMED)
            .opened(true, Condition::Once)
            .tree_push_on_open(true)
            .push(ui));
        if let Some(_tree) = filters_tree {
            let enables = self.enables.get_mut();
            if with_i18n!("pathing-config-api-bypass", |label| ui.checkbox_flags(
                &label,
                enables,
                PathingEnables::API_BYPASS
            )) {
                PathingEvent::ApiBypass(Some(enables.contains(PathingEnables::API_BYPASS))).try_send();
            }
            if ui.is_item_hovered() {
                with_i18n!("pathing-config-api-bypass-notice", |msg| ui.tooltip_text(&msg));
            }
            #[cfg(feature = "paths-interact")]
            {
                self.draw_interaction_opts(ui);
            }

            let festivals_tree = with_i18n!("pathing-config-festivals", |label| TreeNode::new(&label)
                .flags(TreeNodeFlags::FRAMED)
                .opened(false, Condition::Once)
                .tree_push_on_open(true)
                .push(ui));
            if let Some(_tree) = festivals_tree {
                self.draw_festival_opts(ui);
            }
        }

        let advanced = || {
            if let Some(value) = Self::slider_setting_int(
                ui,
                &fl!("pathing-config-load-simultaneous"),
                load_simultaneous as i32,
                (1, 99),
            ) {
                let value = value.map(|v| v.max(1) as usize);
                Self::set_pathing(|s| s.load_simultaneous = value);
            }
            ui.text_wrapped(&fl!("pathing-config-trail-notice"));
            if let Some(value) = Self::slider_opt_setting(
                ui,
                &fl!("pathing-config-trail-y-offset"),
                trail_y_offset,
                (-1.0, 1.0),
            ) {
                Self::set_pathing(|s| s.space.trail_y_offset = value);
            }
            if let Some(value) = Self::slider_setting(
                ui,
                &fl!("pathing-config-trail-resolution"),
                trail_resolution,
                (0.001, 5.0),
            ) {
                Self::set_pathing(|s| s.space.trail_resolution = value);
            }
            if let Some(value) =
                Self::slider_setting(ui, &fl!("pathing-config-trail-width"), trail_width, (0.01, 25.0))
            {
                Self::set_pathing(|s| s.space.trail_width = value);
            }
        };
        let _trail_advanced = TreeNode::new(&fl!("pathing-config-advanced"))
            .flags(TreeNodeFlags::FRAMED)
            .opened(false, Condition::Once)
            .tree_push_on_open(true)
            .build(ui, advanced);

        Some(())
    }

    fn draw_map_opts(&mut self, ui: &Ui) -> Option<()> {
        let (
            mut visible_minimap,
            mut visible_worldmap,
            mut map_open,
            mut trail_textured_mini,
            mut trail_textured_world,
            map_trail_alpha_mini,
            map_trail_alpha_world,
            scale_trail_mini,
            scale_trail_world,
            scale_poi_mini,
            scale_poi_world,
        ) = Self::get_pathing(|s| {
            (
                s.space.visible_minimap(),
                s.space.visible_worldmap(),
                s.space.map_open(),
                s.space.trail_textured_minimap(),
                s.space.trail_textured_worldmap(),
                s.space.trail_alpha_minimap(),
                s.space.trail_alpha_worldmap(),
                //s.space.poi_alpha_minimap(), s.space.poi_alpha_worldmap(),
                s.space.trail_scale_minimap(),
                s.space.trail_scale_worldmap(),
                s.space.poi_scale_minimap(),
                s.space.poi_scale_worldmap(),
            )
        })?;

        let minimap_opts = || {
            //RenderState::font_text("ui", ui, &fl!("pathing-config-minimap"));
            if ui.checkbox(&fl!("pathing-render-minimap-toggle"), &mut visible_minimap) {
                Self::set_pathing(|s| s.space.visible_map_mini = Some(visible_minimap));
            }
            ui.same_line();
            if ui.checkbox(&fl!("pathing-config-textured-minimap"), &mut trail_textured_mini) {
                Self::set_pathing(|s| s.space.map_trail_textured_mini = Some(trail_textured_mini));
            }
            if let Some(value) = Self::slider_setting(
                ui,
                &fl!("pathing-config-trail-alpha-minimap"),
                map_trail_alpha_mini,
                Self::RANGE_ALPHA,
            ) {
                Self::set_pathing(|s| s.space.map_trail_alpha_mini = value);
            }
            #[cfg(todo)]
            if let Some(value) = Self::slider_setting(
                ui,
                &fl!("pathing-config-poi-alpha-minimap"),
                map_poi_alpha_mini,
                Self::RANGE_ALPHA,
            ) {
                Self::set_pathing(|s| s.space.map_poi_alpha_mini = value);
            }
            if let Some(value) = Self::slider_setting(
                ui,
                &fl!("pathing-config-trail-scale-minimap"),
                scale_trail_mini,
                Self::RANGE_SCALE_MAP,
            ) {
                Self::set_pathing(|s| s.space.scale_trail_mini = value);
            }
            if let Some(value) = Self::slider_setting(
                ui,
                &fl!("pathing-config-poi-scale-minimap"),
                scale_poi_mini,
                Self::RANGE_SCALE_POI,
            ) {
                Self::set_pathing(|s| s.space.scale_poi_mini = value);
            }
        };
        let _minimap = TreeNode::new(&fl!("pathing-config-minimap"))
            .flags(TreeNodeFlags::FRAMED)
            .opened(true, Condition::Once)
            .tree_push_on_open(true)
            .build(ui, minimap_opts);

        let worldmap_opts = || {
            if ui.checkbox(&fl!("pathing-render-map-toggle"), &mut visible_worldmap) {
                Self::set_pathing(|s| s.space.visible_map_world = Some(visible_worldmap));
            }
            ui.same_line();
            if ui.checkbox(
                &fl!("pathing-config-textured-worldmap"),
                &mut trail_textured_world,
            ) {
                Self::set_pathing(|s| s.space.map_trail_textured_world = Some(trail_textured_world));
            }
            ui.same_line();
            if ui.checkbox(&fl!("pathing-config-map-open"), &mut map_open) {
                Self::set_pathing(|s| s.space.map_open = Some(map_open));
            }
            if let Some(value) = Self::slider_setting(
                ui,
                &fl!("pathing-config-trail-alpha-worldmap"),
                map_trail_alpha_world,
                Self::RANGE_ALPHA,
            ) {
                Self::set_pathing(|s| s.space.map_trail_alpha_world = value);
            }
            #[cfg(todo)]
            if let Some(value) = Self::slider_setting(
                ui,
                &fl!("pathing-config-poi-alpha-worldmap"),
                map_poi_alpha_world,
                Self::RANGE_ALPHA,
            ) {
                Self::set_pathing(|s| s.space.map_poi_alpha_world = value);
            }
            if let Some(value) = Self::slider_setting(
                ui,
                &fl!("pathing-config-trail-scale-worldmap"),
                scale_trail_world,
                Self::RANGE_SCALE_MAP,
            ) {
                Self::set_pathing(|s| s.space.scale_trail_world = value);
            }
            if let Some(value) = Self::slider_setting(
                ui,
                &fl!("pathing-config-poi-scale-worldmap"),
                scale_poi_world,
                Self::RANGE_SCALE_POI,
            ) {
                Self::set_pathing(|s| s.space.scale_poi_world = value);
            }
        };

        let _worldmap = TreeNode::new(&fl!("pathing-config-worldmap"))
            .flags(TreeNodeFlags::FRAMED)
            .opened(true, Condition::Once)
            .tree_push_on_open(true)
            .build(ui, worldmap_opts);

        Some(())
    }

    fn draw_festival_opts(&mut self, ui: &Ui) {
        let Some(festivals) = ApiController::active_festivals() else { return };
        let mut change = None;
        for festival in Festival::all() {
            let selected = festivals.get_preference(festival);
            let active = festivals.active.get(festival);
            let name = crate::LANGUAGE_LOADER.get(festival.as_str());
            let title = match active {
                true => fl!("pathing-config-festival-active", festival = name),
                false => name,
            };
            let selection = Selectable::new(title).selected(selected.unwrap_or(active));
            if selection.build(ui) {
                change = Some((festival, match (selected, active) {
                    (Some(selected), active) if active == !selected => None,
                    (Some(selected), ..) => Some(!selected),
                    (None, active) => Some(!active),
                }));
            }
        }
        if let Some((festival, change)) = change {
            Self::set_pathing(|s| s.set_festival_preference(festival, change));
        }
    }

    #[cfg(feature = "paths-interact")]
    fn draw_interaction_opts(&mut self, ui: &Ui) {
        let settings = Self::get_pathing(|s| (s.trigger_enable, s.trigger_allow_auto, s.trigger_allow_interact));
        let Some((
            mut trigger_enable,
            trigger_allow_auto,
            trigger_allow_interact,
        )) = settings else {
            return
        };
        let mut settings_dirty = false;

        let indent_token = {
            ui.unindent();
            || ui.indent()
        };
        let enable_toggled = {
            let _id = ui.push_id("trigger_enable");
            ui.checkbox("", &mut trigger_enable)
        }.then(move || trigger_enable);
        let mut trigger_reset = trigger_enable && ui.is_item_clicked_with_button(imgui::MouseButton::Right);
        ui.same_line();

        let interaction_tree = with_i18n!("pathing-config-interactions", |label| TreeNode::new(&label)
            .flags(TreeNodeFlags::FRAMED)
            .opened(false, Condition::Once)
            .tree_push_on_open(false)
            .push(ui));
        trigger_reset |= ui.is_item_clicked_with_button(imgui::MouseButton::Right);
        indent_token();

        let mut interact_toggled = None;
        let set_interact = if let Some(_tree) = interaction_tree {
            let _id = ui.push_id("trigger_allow_interact");

            ui.unindent();
            let mut interact_enabled = !trigger_allow_interact.is_empty();
            interact_toggled = {
                let _id = ui.push_id("trigger_any_interact");
                ui.checkbox("", &mut interact_enabled)
            }.then(move || interact_enabled);
            ui.indent();
            ui.same_line();

            self.draw_trigger_opts(ui, trigger_allow_interact)
        } else { None };

        let mut auto_enabled = !trigger_allow_auto.is_empty();
        let auto_toggled = {
            let _id = ui.push_id("trigger_any_auto");
            ui.checkbox("", &mut auto_enabled)
        }.then(move || auto_enabled);
        ui.same_line();
        let autotrigger_tree = with_i18n!("pathing-config-autotrigger", |label| TreeNode::new(&label)
            .flags(TreeNodeFlags::FRAMED)
            .opened(false, Condition::Once)
            .tree_push_on_open(false)
            .push(ui));
        ui.indent();
        let set_auto = if let Some(_tree) = autotrigger_tree {
            let _id = ui.push_id("trigger_allow_auto");
            with_i18n!("pathing-config-autotrigger-notice", |msg| ui.text_wrapped(msg));
            self.draw_trigger_opts(ui, trigger_allow_auto)
        } else { None };

        let set_interact = set_interact.or(interact_toggled.map(|reset| match reset {
            true => None,
            false => Some(TriggerKind::empty()),
        }));
        let set_auto = set_auto
            .or(auto_toggled.map(|reset| match reset {
                true => None,
                false => Some(TriggerKind::empty()),
            }));
        if let Some(set) = set_auto {
            Self::set_pathing(|s| s.trigger_allow_auto = set.unwrap_or(TriggerKind::settings_default_auto()));
            settings_dirty = true;
        } else if let Some(set) = set_interact {
            Self::set_pathing(|s| s.trigger_allow_interact = set.unwrap_or(TriggerKind::settings_default_interact()));
            settings_dirty = true;
        }
        ui.unindent();

        match (enable_toggled, trigger_reset) {
            (Some(..), _) | (_, true) => {
                Self::set_pathing(|s| {
                    if let Some(enable) = enable_toggled {
                        s.trigger_enable = enable;
                    }
                    if trigger_reset {
                        s.trigger_allow_auto = TriggerKind::settings_default_auto();
                        s.trigger_allow_interact = TriggerKind::settings_default_interact();
                    }
                });
                settings_dirty = true;
            },
            (None, false) => (),
        }
        if settings_dirty {
            InteractMessage::RefreshSettings.try_send();
        }
    }
    fn draw_trigger_opts(&mut self, ui: &Ui, mut setting: TriggerKind) -> Option<Option<TriggerKind>> {
        let mut changed = false;
        let mut reset = false;
        for (i, flag) in TriggerKind::SETTINGS_GUI.into_iter().enumerate() {
            if i % 4 != 0 {
                ui.same_line();
            }
            changed |= with_i18n!(flag.flag_str().unwrap_or_default(), |msg| ui.checkbox_flags(msg, &mut setting, flag));
            if ui.is_item_clicked_with_button(imgui::MouseButton::Right) {
                reset = true;
            }
        }
        setting.set(TriggerKind::SETTINGS_TOGGLE_SHOWHIDE, setting.contains(TriggerKind::TOGGLE));
        match (reset, changed) {
            (true, _) => Some(None),
            (_, changed) =>
                changed.then_some(setting).map(Some),
        }
    }

    #[cfg(feature = "goggles")]
    fn draw_goggles_opts(ui: &Ui, machine: &mut RenderMachine) -> Option<()> {
        use {crate::render::goggles as render_goggles, core::ops::Range};

        let map_id = machine.gameplay.gameplay_map();
        let Range { start: near, end: far } = machine.depth_range();
        let (mut is_enabled, obscured_alpha) =
            Self::get_pathing(|s| (s.space.goggles.enabled(), s.space.goggles.obscured_alpha()))?;

        let (enabled, needs_setup) = render_goggles::get_state();

        ui.text_wrapped(&fl!("pathing-config-goggles-notice"));

        if ui.checkbox(&fl!("enable"), &mut is_enabled) {
            Self::set_pathing(|s| s.space.goggles.goggles_enabled = Some(is_enabled));
            match is_enabled {
                true if !Engine::is_available() => (),
                true => {
                    Engine::try_send(SpaceEvent::GogglesRefreshLens {
                        force: false,
                        delay_override: Some(2),
                    });
                },
                false if !enabled => (),
                false => {
                    log::debug!("Goggles setup: disabling...");
                    render_goggles::disable();
                },
            }
        }

        if !needs_setup {
            let _font = RenderState::push_font("big", ui);
            ui.text_wrapped(
                "For good goggles, you will need to adjust the \"near\" slider for each new map you visit.",
            );
            drop(_font);
            ui.text_wrapped(concat!(
                "Try sliding it down until paths disappear under the ground, then back off a bit.",
                "\n(the sweet spot is usually when you can see the path but grass is slightly covering it)",
                "\nif you see flickering/z-fighting during movement, back off a little more or tweak far",
            ));
        }

        if let Some(value) = Self::slider_setting(ui, "x-ray opacity", obscured_alpha, Self::RANGE_ALPHA) {
            Self::set_pathing(|s| s.space.goggles.obscured_alpha = value);
        }

        if let Some(map_id) = map_id {
            use crate::settings::pathing::GogglesSettings;
            let map_id = map_id.get();

            //RenderState::font_text("ui", ui, "Goggles");
            if let Some(Some(value)) = Self::slider_setting(ui, "near", near, (0.15, 1.2)) {
                Self::set_pathing(|s| {
                    let map_depth_calibration = s.space.goggles.map_depth_calibration_mut();
                    let e = map_depth_calibration
                        .entry(map_id)
                        .or_insert(GogglesSettings::DEFAULT_DEPTH_CALIBRATION);
                    let prev = e.0;
                    e.0 = value / RenderMachine::GOGGLES_DEPTH_RANGE.start;
                    let near = value;
                    let mut far = far;
                    if e.1 == 1.0 || e.1 == prev {
                        e.1 = e.0;
                        far = e.1 * RenderMachine::GOGGLES_DEPTH_RANGE.end;
                    }
                    machine.depth_range = Some(near..far);
                });
            }
            if let Some(Some(value)) = Self::slider_setting(ui, "far", far, (500.0, 2500.0)) {
                Self::set_pathing(|s| {
                    let map_depth_calibration = s.space.goggles.map_depth_calibration_mut();
                    let e = map_depth_calibration
                        .entry(map_id)
                        .or_insert(GogglesSettings::DEFAULT_DEPTH_CALIBRATION);
                    e.1 = value / RenderMachine::GOGGLES_DEPTH_RANGE.end;
                    machine.depth_range = Some(near..value);
                });
            }
            if ui.button("distance reset") {
                Self::set_pathing(|s| {
                    let map_depth_calibration = s.space.goggles.map_depth_calibration_mut();
                    map_depth_calibration.remove(&map_id);
                    machine.depth_range = None;
                });
            }
        }

        let _lenses = TreeNode::new("advanced lens config")
            .flags(TreeNodeFlags::FRAMED)
            .opened(false, Condition::Once)
            .tree_push_on_open(true)
            .build(ui, || render_goggles::options_ui_lenses(ui));

        Some(())
    }
}
