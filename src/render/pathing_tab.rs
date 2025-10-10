use {
    crate::{
        controller::ControllerEvent,
        exports::runtime as rt,
        fl,
        render::{
            machine::RenderMachine,
            RenderState,
        },
        settings::{
            pathing::{
                CameraSource,
                PathingSettings,
                SpaceSettings,
            },
            Settings,
        },
        space,
        Controller,
        LANGUAGE_LOADER,
    },
    anyhow::Context,
    nexus::imgui::{
        self,
        ChildWindow, ComboBox, Condition,
        Selectable, Slider, TreeNode, TreeNodeFlags,
        Ui, WindowFlags
    },
    std::collections::HashMap,
    strum::VariantArray,
};

pub struct PathingConfig {
    katrender: bool,
}

impl PathingConfig {
    pub fn new() -> Self {
        Self {
            katrender: false,
        }
    }

    pub fn draw(&mut self, ui: &Ui, machine: &mut RenderMachine, _state_errors: &mut HashMap<String, anyhow::Error>) {
        if let Some(settings) = Settings::try_read() {
            self.katrender = settings.enable_katrender;
        };

        ui.columns(2, "pathing_tab_start", true);

        self.draw_header(ui);

        let opts_primary = || {
            let active = self.draw_pathing_opts(ui, machine);
            match (&active, self.katrender) {
                (None, true) =>
                    Self::draw_space_error(ui, None),
                (Some(..), true) => {
                    ui.separator();
                    let label = fl!("pathing-window");
                    if ui.button(&label) {
                        crate::control_window(crate::WINDOW_PATHING, None);
                    }
                },
                _ => (),
            }
            ui.text_wrapped(&fl!("experimental-notice"));

            active
        };

        let child_window_flags = WindowFlags::HORIZONTAL_SCROLLBAR;
        let active = ChildWindow::new("pathing_main")
            .flags(child_window_flags)
            .size([0.0, 0.0])
            .build(ui, opts_primary);

        ui.next_column();

        let opts_secondary = || if let Some(Some(..)) = active {
            self.draw_map_opts(ui);

            #[cfg(feature = "goggles")]
            let _goggles = TreeNode::new(&fl!("pathing-config-goggles"))
                .flags(TreeNodeFlags::FRAMED)
                .opened(true, Condition::Once)
                .tree_push_on_open(true)
                .build(ui, || Self::draw_goggles_opts(ui, machine));
        };

        ChildWindow::new("pathing_secondary")
            .size([0.0, 0.0])
            .build(ui, opts_secondary);

        ui.columns(1, "pathing_tab_end", false)
    }

    pub fn draw_space_error(ui: &Ui, e: Option<anyhow::Error>) {
        let _font = RenderState::push_font("big", ui);
        let e = match e {
            None if !Settings::try_read().map(|s| s.enable_katrender).unwrap_or(true) => {
                {
                    let _notice = RenderState::push_font("ui", ui);
                    ui.text_wrapped(&fl!("experimental-notice"));
                }
                if ui.button(&fl!("enable")) {
                    Controller::try_send(ControllerEvent::ToggleKatRender);
                }
                None
            },
            None if rt::mumble_link_ptr().map(|ml| ml.read_map_id()).ok() == Some(0) => {
                ui.text("Select a character to get started");
                None
            },
            None => {
                let res = crate::ENGINE.try_lock().ok()
                    .and_then(|e| e.as_ref()
                        .map(|e| e.as_ref().map(drop)
                            .map_err(Clone::clone)
                        )
                    );
                match res {
                    Some(Err(e)) => {
                        ui.text("Error! See log in Nexus or Taimi addon folder for more details");
                        match e {
                            () => None,
                            #[cfg(todo)]
                            e => Some(e),
                        }
                    },
                    None => {
                        ui.text("Load in to the game to get started");
                        None
                    },
                    Some(Ok(())) => {
                        // shouldn't happen?
                        None
                    },
                }
            },
            Some(e) => Some(e),
        };
        if let Some(e) = e {
            ui.text(format!("{e:#}"));
        }
    }

    fn draw_header(&mut self, ui: &Ui) {
        {
            let _font = (!self.katrender).then(|| RenderState::push_font("big", ui));
            if ui.checkbox(&fl!("pathing-config-enable"), &mut self.katrender) {
                Controller::try_send(ControllerEvent::ToggleKatRender);
            }
        }

        if self.katrender {
            ui.same_line();
            if ui.button("Unload Render") {
                let _disabled = Settings::write_with_blocking(|settings| {
                    settings.enable_katrender = false;
                });
                if _disabled.is_ok() {
                    crate::reload_render(false);
                }
            }

            ui.same_line();
            if ui.button("Reload Render") {
                crate::reload_render(false);
            }
        } else {
            //RenderState::font_text("ui", ui, &fl!("pathing-config"));
            ui.text_disabled("KatRender is required for pathing");
        }
    }

    fn combo_setting<T>(ui: &Ui, label: &str, value: T) -> Option<Option<T>> where
        T: VariantArray + Eq + Copy + Into<&'static str>,
    {
        let _token = ui.push_id(label);
        let current = LANGUAGE_LOADER.get(value.into());
        let draw = || {
            let mut selected = None;
            for &opt in T::VARIANTS {
                let name = LANGUAGE_LOADER.get(opt.into());
                let selection = Selectable::new(name)
                    .selected(value == opt);
                if selection.build(ui) {
                    selected = Some(opt.clone());
                }
            }
            selected
        };
        let changed = ComboBox::new(label)
            .preview_value(&current)
            .build(ui, draw);
        match changed {
            Some(Some(selected)) => Some(Some(selected)),
            None if ui.is_item_clicked_with_button(imgui::MouseButton::Right) =>
                // reset to default
                Some(None),
            _ => None,
        }
    }

    fn slider_setting(ui: &Ui, label: &str, mut value: f32, (min, max): (f32, f32)) -> Option<Option<f32>> {
        let changed = Slider::new(label, min, max)
            .build(ui, &mut value);
        // TODO: right-click to reset or something?
        match changed {
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

    fn slider_opt_setting(ui: &Ui, label: &str, value: Option<f32>, range: (f32, f32)) -> Option<Option<f32>> {
        let mut enabled = value.is_some();
        let _token = ui.push_id(label);
        let res = if let Some(value) = value {
            let res = Self::slider_setting(ui, label, value, range);
            res
        } else {
            ui.label_text(label, &fl!("disabled"));
            None
        };
        ui.same_line();
        if ui.checkbox("", &mut enabled) {
            return Some(match enabled {
                false => Some(SpaceSettings::NONE_F32),
                true => None,
            })
        }
        res
    }

    fn set_pathing<F: FnOnce(&mut PathingSettings)>(f: F) {
        let res = crate::engine_mut(|e|
            e.map_settings_mut(|s| f(s))
        ).transpose().context("failed to save pathing settings");
        if let Err(e) = res {
            log::warn!("{e:#}");
        }
    }

    const RANGE_ALPHA: (f32, f32) = (0.0, 1.0);
    const RANGE_SCALE: (f32, f32) = (0.0, 25.0);
    //const RANGE_SCALE_POI: (f32, f32) = (-1.0, 10.0);
    const RANGE_SCALE_POI: (f32, f32) = (0.0, 5.0);
    const RANGE_SCALE_MAP: (f32, f32) = Self::RANGE_SCALE;
    fn draw_pathing_opts(&mut self, ui: &Ui, _machine: &mut RenderMachine) -> Option<()> {
        let (
            camera_source,
            mut visible_space,
            player_overlap_threshold,
            distance_fade_intensity,
            distance_max,
            mut trail_textured_space,
            trail_alpha,
            scale_trail_space, scale_poi_space,
            edge_feather_scale,
            (edge_scale, ),
        ) = crate::engine_ref(|e| e.map_settings_ref(|s| s.map(|s| (
            s.space.camera_source(),
            s.space.visible_space(),
            s.space.player_overlap_threshold(),
            s.space.distance_fade_intensity(),
            s.space.distance_max(),
            s.space.trail_textured_space(),
            s.space.trail_alpha(), // s.space.poi_alpha(),
            s.space.trail_scale_space(), s.space.poi_scale_space(),
            s.space.edge_feather_scale(),
            match () {
                #[cfg(feature = "goggles")]
                _ => (
                    s.space.goggles.edge_scale(),
                ),
                #[cfg(not(feature = "goggles"))]
                _ => ((),),
            },
        )))).flatten()?;

        if ui.checkbox(&fl!("pathing-render-toggle"), &mut visible_space) {
            Self::set_pathing(|s| s.space.visible_space = Some(visible_space));
            #[cfg(feature = "goggles")]
            match visible_space {
                _ if !crate::space::goggles::is_enabled() => (),
                false =>
                    crate::space::goggles::clear_lens(),
                true => {
                    let _ = crate::engine_mut(|e| e.goggles_enter(false));
                },
            }
        }
        ui.same_line();
        if ui.checkbox(&fl!("pathing-config-textured"), &mut trail_textured_space) {
            Self::set_pathing(|s| s.space.trail_textured_space = Some(trail_textured_space));
        }
        if let Some(value) = Self::slider_setting(ui, &fl!("pathing-config-trail-alpha"), trail_alpha, Self::RANGE_ALPHA) {
            Self::set_pathing(|s| s.space.trail_alpha = value);
        }
        if let Some(value) = Self::slider_setting(ui, &fl!("pathing-config-trail-scale"), scale_trail_space, Self::RANGE_SCALE) {
            Self::set_pathing(|s| s.space.scale_trail_space = value);
        }
        #[cfg(todo)]
        if let Some(value) = Self::slider_setting(ui, &fl!("pathing-config-poi-alpha"), poi_alpha, Self::RANGE_ALPHA) {
            Self::set_pathing(|s| s.space.poi_alpha = value);
        }
        if let Some(value) = Self::slider_setting(ui, &fl!("pathing-config-poi-scale"), scale_poi_space, Self::RANGE_SCALE_POI) {
            Self::set_pathing(|s| s.space.scale_poi_space = value);
        }
        if let Some(value) = Self::slider_opt_setting(ui, &fl!("pathing-config-distance-fade-intensity"), distance_fade_intensity, (1.0, 500.0)) {
            Self::set_pathing(|s| s.space.distance_fade_intensity = value);
        }
        if let Some(value) = Self::slider_opt_setting(ui, &fl!("pathing-config-player-overlap-threshold"), player_overlap_threshold, (0.01, 1000.0)) {
            Self::set_pathing(|s| s.space.player_overlap_threshold = value);
        }
        if let Some(value) = Self::slider_opt_setting(ui, "edge feather scale", edge_feather_scale, (0.001f32, 5.0)) {
            Self::set_pathing(|s| s.space.edge_feather_scale = value);
        }
        #[cfg(feature = "goggles")]
        if let Some(value) = Self::slider_opt_setting(ui, "corner boundary scale", edge_scale, (0.1f32, 5.0)) {
            let mut edge_scale = value;
            Self::set_pathing(|s| {
                s.space.goggles.edge_scale = value;
                edge_scale = s.space.goggles.edge_scale();
            });
            let _ = crate::engine_mut(|e| {
                //e.render_backend.depth_handler.regen_edge(&e.render_backend.device, edge_scale);
                e.render_backend.depth_handler.fill_edge.take();
            });
        }
        if let Some(value) = Self::slider_setting(ui, &fl!("pathing-config-distance-max"), distance_max, (1.0, 2000.0)) {
            Self::set_pathing(|s| s.space.distance_max = value);
        }
        #[cfg(feature = "extension-nexus")]
        if let Some(value) = Self::combo_setting(ui, &fl!("pathing-config-camera-source"), camera_source) {
            Self::set_pathing(|s| s.space.camera_source = value);
            if value == Some(CameraSource::RealTimeAPI) {
                match _machine.rtapi_init() {
                    Err(e) =>
                        log::warn!("{e:#}"),
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
                if _machine.rtapi.is_none() {
                    ui.text_wrapped(
                        "RTAPI is a separate addon that must be installed via Nexus"
                    );
                }
                ui.text_wrapped(
                    "if you experience stuttering, try changing Vertical Sync or switching to MumbleLink",
                );
            },
        }

        Some(())
    }

    fn draw_map_opts(&mut self, ui: &Ui) -> Option<()> {
        let (
            mut visible_minimap, mut visible_worldmap,
            mut map_open,
            mut trail_textured_mini, mut trail_textured_world,
            map_trail_alpha_mini, map_trail_alpha_world,
            scale_trail_mini, scale_trail_world,
            scale_poi_mini, scale_poi_world,
        ) = crate::engine_ref(|e| e.map_settings_ref(|s| s.map(|s| (
            s.space.visible_minimap(), s.space.visible_worldmap(),
            s.space.map_open(),
            s.space.trail_textured_minimap(), s.space.trail_textured_worldmap(),
            s.space.trail_alpha_minimap(), s.space.trail_alpha_worldmap(),
            //s.space.poi_alpha_minimap(), s.space.poi_alpha_worldmap(),
            s.space.trail_scale_minimap(), s.space.trail_scale_worldmap(),
            s.space.poi_scale_minimap(), s.space.poi_scale_worldmap(),
        )))).flatten()?;

        let minimap_opts = || {
            //RenderState::font_text("ui", ui, &fl!("pathing-config-minimap"));
            if ui.checkbox(&fl!("pathing-render-minimap-toggle"), &mut visible_minimap) {
                Self::set_pathing(|s| s.space.visible_map_mini = Some(visible_minimap));
            }
            ui.same_line();
            if ui.checkbox(&fl!("pathing-config-textured-minimap"), &mut trail_textured_mini) {
                Self::set_pathing(|s| s.space.map_trail_textured_mini = Some(trail_textured_mini));
            }
            if let Some(value) = Self::slider_setting(ui, &fl!("pathing-config-trail-alpha-minimap"), map_trail_alpha_mini, Self::RANGE_ALPHA) {
                Self::set_pathing(|s| s.space.map_trail_alpha_mini = value);
            }
            #[cfg(todo)]
            if let Some(value) = Self::slider_setting(ui, &fl!("pathing-config-poi-alpha-minimap"), map_poi_alpha_mini, Self::RANGE_ALPHA) {
                Self::set_pathing(|s| s.space.map_poi_alpha_mini = value);
            }
            if let Some(value) = Self::slider_setting(ui, &fl!("pathing-config-trail-scale-minimap"), scale_trail_mini, Self::RANGE_SCALE_MAP) {
                Self::set_pathing(|s| s.space.scale_trail_mini = value);
            }
            if let Some(value) = Self::slider_setting(ui, &fl!("pathing-config-poi-scale-minimap"), scale_poi_mini, Self::RANGE_SCALE_POI) {
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
            if ui.checkbox(&fl!("pathing-config-textured-worldmap"), &mut trail_textured_world) {
                Self::set_pathing(|s| s.space.map_trail_textured_world = Some(trail_textured_world));
            }
            ui.same_line();
            if ui.checkbox(&fl!("pathing-config-map-open"), &mut map_open) {
                Self::set_pathing(|s| s.space.map_open = Some(map_open));
            }
            if let Some(value) = Self::slider_setting(ui, &fl!("pathing-config-trail-alpha-worldmap"), map_trail_alpha_world, Self::RANGE_ALPHA) {
                Self::set_pathing(|s| s.space.map_trail_alpha_world = value);
            }
            #[cfg(todo)]
            if let Some(value) = Self::slider_setting(ui, &fl!("pathing-config-poi-alpha-worldmap"), map_poi_alpha_world, Self::RANGE_ALPHA) {
                Self::set_pathing(|s| s.space.map_poi_alpha_world = value);
            }
            if let Some(value) = Self::slider_setting(ui, &fl!("pathing-config-trail-scale-worldmap"), scale_trail_world, Self::RANGE_SCALE_MAP) {
                Self::set_pathing(|s| s.space.scale_trail_world = value);
            }
            if let Some(value) = Self::slider_setting(ui, &fl!("pathing-config-poi-scale-worldmap"), scale_poi_world, Self::RANGE_SCALE_POI) {
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

    #[cfg(feature = "goggles")]
    fn draw_goggles_opts(ui: &Ui, machine: &mut RenderMachine) -> Option<()> {
        use {
            core::ops::Range,
            crate::render::goggles as render_goggles,
        };

        let mut map_id = None;
        let (
            obscured_alpha,
            Range { start: _near, end: _far },
        ) = crate::engine_ref(|e| e.map_settings_ref(|s| s.map(|s| {
            map_id = e.packs.current_map.map(|id| id as _);
            (
                s.space.goggles.obscured_alpha(),
                machine.get_depth_range().unwrap_or(space::MIN_DEPTH..space::MAX_DEPTH)
            )
        }))).flatten()?;

        let (mut enabled, needs_setup) = render_goggles::get_state();

        ui.text_wrapped("This currently requires setting Render Sampling to Native under Graphics Options.");

        if ui.checkbox(&fl!("enable"), &mut enabled) {
            Self::set_pathing(|s| s.space.goggles.goggles_enabled = Some(enabled));
            match enabled {
                true => {
                    if crate::engine_mut(|e| e.goggles_enter(false)).is_none() {
                        render_goggles::enable(needs_setup);
                    }
                },
                false => {
                    log::info!("Goggles setup: disabling...");
                    render_goggles::disable();
                },
            }
        }

        if !needs_setup {
            let _font = RenderState::push_font("big", ui);
            ui.text_wrapped("For good goggles, you will need to adjust the \"near\" slider for each new map you visit.");
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
            use crate::{settings::pathing::GogglesSettings, space};

            //RenderState::font_text("ui", ui, "Goggles");
            if let Some(Some(value)) = Self::slider_setting(ui, "near", _near, (0.15, 1.2)) {
                Self::set_pathing(|s| {
                    let map_depth_calibration = s.space.goggles.map_depth_calibration_mut();
                    let e = map_depth_calibration.entry(map_id)
                        .or_insert(GogglesSettings::DEFAULT_DEPTH_CALIBRATION);
                    let prev = e.0;
                    e.0 = value / space::MIN_DEPTH;
                    let near = value;
                    let mut far = _far;
                    if e.1 == 1.0 || e.1 == prev {
                        e.1 = e.0;
                        far = e.1 * space::MAX_DEPTH;
                    }
                    machine.depth_range = Some(near..far);
                });
            }
            if let Some(Some(value)) = Self::slider_setting(ui, "far", _far, (500.0, 2500.0)) {
                Self::set_pathing(|s| {
                    let map_depth_calibration = s.space.goggles.map_depth_calibration_mut();
                    let e = map_depth_calibration.entry(map_id)
                        .or_insert(GogglesSettings::DEFAULT_DEPTH_CALIBRATION);
                    e.1 = value / space::MAX_DEPTH;
                    machine.depth_range = Some(_near..value);
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
