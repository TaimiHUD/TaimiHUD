use {
    super::TimerWindowState,
    crate::{
        controller::ProgressBarStyleChange,
        fl,
        render::TextFont,
        settings::{MarkerAutoPlaceSettings, SquadCondition},
        ControllerEvent, Controller, SETTINGS,
    },
    nexus::imgui::{self, ComboBox, Condition, Selectable, Slider, TreeNode, TreeNodeFlags, Ui},
    strum::IntoEnumIterator,
};
#[cfg(feature = "space")]
use {
    anyhow::Context,
    crate::{
        render::RenderState,
        settings::pathing::{PathingSettings, SpaceSettings},
    },
};

pub struct ConfigTabState {
    pub katrender: bool,
    pub marker_autoplace: MarkerAutoPlaceSettings,
    pub marker_autoplace_inner: Option<SquadCondition>,
}

impl ConfigTabState {
    pub fn new() -> Self {
        Self {
            katrender: false,
            marker_autoplace: Default::default(),
            marker_autoplace_inner: Default::default(),
        }
    }

    pub fn draw(&mut self, ui: &Ui, timer_window_state: &mut TimerWindowState) {
        if let Some(settings) = SETTINGS.get().and_then(|settings| settings.try_read().ok()) {
            self.katrender = settings.enable_katrender;
        };
        ui.text_wrapped(&fl!("imgui-notice"));
        ui.dummy([4.0, 4.0]);
        ui.text_wrapped(&fl!("keybind-triggers"));
        ui.dummy([4.0, 4.0]);
        #[cfg(feature = "space")]
        self.draw_space(ui);

        let markers_window_closure = || {
            if let Some(settings) = SETTINGS.get().and_then(|settings| settings.try_read().ok()) {
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
                Controller::try_send(ControllerEvent::MarkerAutoPlaceSettings(
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
                        }
                        MarkerAutoPlaceSettings::Place(ref mut t) => {
                            *t = selection.clone();
                        }
                        _ => (),
                    };
                    Controller::try_send(ControllerEvent::MarkerAutoPlaceSettings(
                        self.marker_autoplace.clone(),
                    ));
                }
            }
        };
        let timers_window_closure = || {
            ui.dummy([4.0, 4.0]);
            if let Some(settings) = SETTINGS.get().and_then(|settings| settings.try_read().ok()) {
                timer_window_state.progress_bar.stock = settings.progress_bar.stock;
            };
            if ui.checkbox(
                &fl!("stock-imgui-progress-bar"),
                &mut timer_window_state.progress_bar.stock,
            ) {
                Controller::try_send(ControllerEvent::ProgressBarStyle(
                    ProgressBarStyleChange::Stock(timer_window_state.progress_bar.stock),
                ));
            };
            if ui.checkbox(&fl!("shadow"), &mut timer_window_state.progress_bar.shadow) {
                Controller::try_send(ControllerEvent::ProgressBarStyle(
                    ProgressBarStyleChange::Shadow(timer_window_state.progress_bar.shadow),
                ));
            }
            if ui.checkbox(
                &fl!("centre-text-after-icon"),
                &mut timer_window_state.progress_bar.centre_after,
            ) {
                Controller::try_send(ControllerEvent::ProgressBarStyle(
                    ProgressBarStyleChange::Centre(timer_window_state.progress_bar.centre_after),
                ));
            }
            if Slider::new(&fl!("height"), 8.0, 256.0)
                .display_format("%.0f")
                .build(ui, &mut timer_window_state.progress_bar.height)
            {
                Controller::try_send(ControllerEvent::ProgressBarStyle(
                    ProgressBarStyleChange::Height(timer_window_state.progress_bar.height),
                ));
            }
            let font_closure = || {
                let mut selected = timer_window_state.progress_bar.font.clone();
                for font in TextFont::iter() {
                    if Selectable::new(font.to_string())
                        .selected(font == selected)
                        .build(ui)
                    {
                        Controller::try_send(ControllerEvent::ProgressBarStyle(
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
    }

    #[cfg(feature = "space")]
    fn draw_pathing_opts(&mut self, ui: &Ui) -> Option<()> {
        #[cfg(feature = "goggles")]
        let mut map_id = None;
        let current = crate::engine_ref(|e| e.map_settings_ref(|s| s.map(|s| (
            s.space.player_overlap_threshold(),
            s.space.distance_fade_intensity(),
            s.space.distance_max(),
                // TODO: visible toggles
            s.space.trail_textured_space(),
            s.space.trail_textured_minimap(), s.space.trail_textured_worldmap(),
            s.space.trail_alpha(), // s.space.poi_alpha(),
            s.space.trail_scale_space(), s.space.poi_scale_space(),
            s.space.trail_alpha_minimap(), s.space.trail_alpha_worldmap(),
            //s.space.poi_alpha_minimap(), s.space.poi_alpha_worldmap(),
            s.space.trail_scale_minimap(), s.space.trail_scale_worldmap(),
            s.space.poi_scale_minimap(), s.space.poi_scale_worldmap(),
            match () {
                #[cfg(feature = "goggles")]
                _ => {
                    map_id = e.gameplay_map.ok().map(|id| id.get());
                    (
                        s.space.goggles.edge_scale(),
                        s.space.goggles.obscured_alpha(),
                        (crate::space::min_depth(), crate::space::max_depth()),
                    )
                },
                #[cfg(not(feature = "goggles"))]
                _ => ((), (), ((), ())),
            },
        )))).flatten();
        if let Some(current) = current {
            let (
                player_overlap_threshold,
                distance_fade_intensity,
                distance_max,
                mut trail_textured_space, mut trail_textured_mini, mut trail_textured_world,
                trail_alpha,
                scale_trail_space, scale_poi_space,
                map_trail_alpha_mini, map_trail_alpha_world,
                scale_trail_mini, scale_trail_world,
                scale_poi_mini, scale_poi_world,
                (_edge_scale, _obscured_alpha, (_near, _far)),
            ) = current;
            let range_alpha = (0.0, 1.0);
            let range_scale = (0.0, 25.0);
            //let range_scale_poi = (-1.0, 10.0);
            let range_scale_poi = (0.0, 5.0);
            let range_scale_map = range_scale;


            if ui.checkbox(&fl!("pathing-config-textured"), &mut trail_textured_space) {
                Self::set_pathing(|s| s.space.trail_textured_space = Some(trail_textured_space));
            }
            if let Some(value) = Self::slider_setting(ui, &fl!("pathing-config-trail-alpha"), trail_alpha, range_alpha) {
                Self::set_pathing(|s| s.space.trail_alpha = value);
            }
            if let Some(value) = Self::slider_setting(ui, &fl!("pathing-config-trail-scale"), scale_trail_space, range_scale) {
                Self::set_pathing(|s| s.space.scale_trail_space = value);
            }
            #[cfg(todo)]
            if let Some(value) = Self::slider_setting(ui, &fl!("pathing-config-poi-alpha"), poi_alpha, range_alpha) {
                Self::set_pathing(|s| s.space.poi_alpha = value);
            }
            if let Some(value) = Self::slider_setting(ui, &fl!("pathing-config-poi-scale"), scale_poi_space, range_scale_poi) {
                Self::set_pathing(|s| s.space.scale_poi_space = value);
            }
            if let Some(value) = Self::slider_opt_setting(ui, &fl!("pathing-config-distance-fade-intensity"), distance_fade_intensity, (1.0, 500.0)) {
                Self::set_pathing(|s| s.space.distance_fade_intensity = value);
            }
            if let Some(value) = Self::slider_opt_setting(ui, &fl!("pathing-config-player-overlap-threshold"), player_overlap_threshold, (0.01, 1000.0)) {
                Self::set_pathing(|s| s.space.player_overlap_threshold = value);
            }
            if let Some(value) = Self::slider_setting(ui, &fl!("pathing-config-distance-max"), distance_max, (1.0, 2000.0)) {
                Self::set_pathing(|s| s.space.distance_max = value);
            }
            //ui.separator();

            let minimap_opts = || {
                //RenderState::font_text("ui", ui, &fl!("pathing-config-minimap"));
                if ui.checkbox(&fl!("pathing-config-textured-minimap"), &mut trail_textured_mini) {
                    Self::set_pathing(|s| s.space.map_trail_textured_mini = Some(trail_textured_mini));
                }
                if let Some(value) = Self::slider_setting(ui, &fl!("pathing-config-trail-alpha-minimap"), map_trail_alpha_mini, range_alpha) {
                    Self::set_pathing(|s| s.space.map_trail_alpha_mini = value);
                }
                #[cfg(todo)]
                if let Some(value) = Self::slider_setting(ui, &fl!("pathing-config-poi-alpha-minimap"), map_poi_alpha_mini, range_alpha) {
                    Self::set_pathing(|s| s.space.map_poi_alpha_mini = value);
                }
                if let Some(value) = Self::slider_setting(ui, &fl!("pathing-config-trail-scale-minimap"), scale_trail_mini, range_scale_map) {
                    Self::set_pathing(|s| s.space.scale_trail_mini = value);
                }
                if let Some(value) = Self::slider_setting(ui, &fl!("pathing-config-poi-scale-minimap"), scale_poi_mini, range_scale_poi) {
                    Self::set_pathing(|s| s.space.scale_poi_mini = value);
                }
            };
            let _minimap = TreeNode::new(&fl!("pathing-config-minimap"))
                .flags(TreeNodeFlags::FRAMED)
                .opened(true, Condition::Once)
                .tree_push_on_open(true)
                .build(ui, minimap_opts);

            let worldmap_opts = || {
                if ui.checkbox(&fl!("pathing-config-textured-worldmap"), &mut trail_textured_world) {
                    Self::set_pathing(|s| s.space.map_trail_textured_world = Some(trail_textured_world));
                }
                if let Some(value) = Self::slider_setting(ui, &fl!("pathing-config-trail-alpha-worldmap"), map_trail_alpha_world, range_alpha) {
                    Self::set_pathing(|s| s.space.map_trail_alpha_world = value);
                }
                #[cfg(todo)]
                if let Some(value) = Self::slider_setting(ui, &fl!("pathing-config-poi-alpha-worldmap"), map_poi_alpha_world, range_alpha) {
                    Self::set_pathing(|s| s.space.map_poi_alpha_world = value);
                }
                if let Some(value) = Self::slider_setting(ui, &fl!("pathing-config-trail-scale-worldmap"), scale_trail_world, range_scale_map) {
                    Self::set_pathing(|s| s.space.scale_trail_world = value);
                }
                if let Some(value) = Self::slider_setting(ui, &fl!("pathing-config-poi-scale-worldmap"), scale_poi_world, range_scale_poi) {
                    Self::set_pathing(|s| s.space.scale_poi_world = value);
                }
            };

            let _worldmap = TreeNode::new(&fl!("pathing-config-worldmap"))
                .flags(TreeNodeFlags::FRAMED)
                .opened(false, Condition::Once)
                .tree_push_on_open(true)
                .build(ui, worldmap_opts);

            #[cfg(feature = "goggles")]
            let goggles_opts = || {
                crate::render::goggles::options_ui(ui);

                if let Some(map_id) = map_id {
                    use crate::{settings::pathing::GogglesSettings, space};

                    //RenderState::font_text("ui", ui, "Goggles");
                    if let Some(Some(value)) = Self::slider_setting(ui, "goggles near", _near, (0.15, 1.2)) {
                        Self::set_pathing(|s| {
                            let e = s.space.goggles.map_depth_calibration.entry(map_id)
                                .or_insert(GogglesSettings::DEFAULT_DEPTH_CALIBRATION);
                            let prev = e.0;
                            e.0 = value / space::MIN_DEPTH;
                            space::set_min_depth(value);
                            if e.1 == 1.0 || e.1 == prev {
                                e.1 = e.0;
                                space::set_max_depth(e.1 * space::MAX_DEPTH);
                            }
                        });
                    }
                    if let Some(Some(value)) = Self::slider_setting(ui, "goggles far", _far, (500.0, 2500.0)) {
                        Self::set_pathing(|s| {
                            let e = s.space.goggles.map_depth_calibration.entry(map_id)
                                .or_insert(GogglesSettings::DEFAULT_DEPTH_CALIBRATION);
                            e.1 = value / space::MAX_DEPTH;
                            space::set_max_depth(value);
                        });
                    }
                    if ui.button("goggles distance reset") {
                        Self::set_pathing(|s| {
                            s.space.goggles.map_depth_calibration.remove(&map_id);
                            space::set_max_depth(space::MAX_DEPTH);
                            space::set_min_depth(space::MIN_DEPTH);
                        });
                    }
                }
                if let Some(value) = Self::slider_setting(ui, "goggles x-ray opacity", _obscured_alpha, range_alpha) {
                    Self::set_pathing(|s| s.space.goggles.obscured_alpha = value);
                }
                if let Some(value) = Self::slider_opt_setting(ui, "edge boundary scale", _edge_scale, (0.1f32, 5.0)) {
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
            };
            #[cfg(feature = "goggles")]
            let _goggles = TreeNode::new("goggles")
                .flags(TreeNodeFlags::FRAMED)
                .opened(true, Condition::Once)
                .tree_push_on_open(true)
                .build(ui, goggles_opts);

            Some(())
        } else {
            None
        }
    }

    #[cfg(feature = "space")]
    fn draw_space(&mut self, ui: &Ui) {
        if ui.checkbox("Experimental KatRender", &mut self.katrender) {
            Controller::try_send(ControllerEvent::ToggleKatRender);
        };

        ui.same_line();
        if ui.button("Unload All") {
            use crate::render::RenderEvent;

            // XXX: this will wipe all of render state, rather than just katrender
            let mut render_sender = crate::RENDER_SENDER.write().unwrap();
            let render_quit = render_sender.as_ref().map(|sender| sender.try_send(RenderEvent::Quit));
            if let Some(Ok(())) = render_quit {
                let _ = render_sender.take();
                let _ = crate::SPACE_SENDER.write().unwrap().take();
                crate::TEXTURES.quit();
            }
            Controller::try_send(ControllerEvent::UnloadAll);
        }
        if self.katrender {
            ui.same_line();
            if ui.button("Reload Render") {
                crate::reload_render(false);
            }

            let pathing_opts = || {
                if let None = self.draw_pathing_opts(ui) {
                    ui.text_disabled("not loaded (yet?)");
                    ui.text_disabled("load a map or check nexus logs for errors");
                }
            };

            let _pathing = TreeNode::new(&fl!("pathing-config"))
                .flags(TreeNodeFlags::FRAMED)
                .opened(true, Condition::Once)
                .tree_push_on_open(true)
                .build(ui, pathing_opts);
        } else {
            RenderState::font_text("ui", ui, &fl!("pathing-config"));
            ui.text_disabled("katrender options disabled");
        }
    }

    #[cfg(feature = "space")]
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

    #[cfg(feature = "space")]
    fn slider_opt_setting(ui: &Ui, label: &str, value: Option<f32>, range: (f32, f32)) -> Option<Option<f32>> {
        let mut enabled = value.is_some();
        let _token = ui.push_id(label);
        let res = if let Some(value) = value {
            let res = Self::slider_setting(ui, label, value, range);
            ui.same_line();
            res
        } else { None };
        if ui.checkbox("enable", &mut enabled) {
            return Some(match enabled {
                false => Some(SpaceSettings::NONE_F32),
                true => None,
            })
        }
        res
    }

    #[cfg(feature = "space")]
    fn set_pathing<F: FnOnce(&mut PathingSettings)>(f: F) {
        let res = crate::engine_mut(|e|
            e.map_settings_mut(|s| f(s))
        ).transpose().context("failed to save pathing settings");
        if let Err(e) = res {
            log::warn!("{e:#}");
        }
    }
}
