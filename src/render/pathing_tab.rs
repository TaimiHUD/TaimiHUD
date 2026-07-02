use {
    crate::{
        controller::{
            api::ApiController,
            pathing::{PathingController, PathingEnables, PathingEvent},
            Controller,
        },
        render::{element::prelude::*, machine::RenderMachine, RenderEvent, RenderState},
        settings::{
            pathing::{CameraSource, PathingSettings, SpaceSettings, TriggerKind},
            Settings,
        },
        with_i18n,
    },
    anyhow::Context,
    std::collections::HashMap,
    strum::VariantArray,
    taimi_pack::attributes::Festival,
    taimi_sync::watched::Watched,
};

#[cfg(feature = "paths-interact")]
use crate::controller::pathing::InteractMessage;
#[cfg(feature = "paths")]
use crate::space::engine::{Engine, SpaceEvent};
#[cfg(feature = "goggles")]
use {
    crate::{
        settings::goggles::{GogglesEnables, GogglesMapDepth},
        render::goggles as render_goggles,
    },
    std::{borrow::Cow, ops::Range},
    taimi_meta::map::MapProjectionDepth,
};

pub struct PathingConfig {
    enables: Watched<PathingEnables>,
    arcrender_enabled: bool,
    #[cfg(feature = "goggles")]
    goggles: render_goggles::GogglesConfig,
}

impl PathingConfig {
    pub fn new() -> Self {
        let mut state = Self {
            enables: Watched::EMPTY,
            arcrender_enabled: false,
            #[cfg(feature = "goggles")]
            goggles: Default::default(),
        };
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
    fn arcrender(&self) -> bool {
        self.arcrender_enabled
    }

    pub fn draw<'ui, U>(
        &mut self,
        ui: &mut U,
        machine: &mut RenderMachine,
        _state_errors: &mut HashMap<String, anyhow::Error>,
    ) where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let _ = self.enables.try_read_mut();
        ui.columns(2, "pathing_tab_start", true);

        self.draw_header(ui);

        let mut available = None;
        if let Some(_active) = ui.begin_mainbar(c"pathing_main") {
            let available = *available.insert(Engine::is_available());
            if !available && self.katrender() {
                Self::draw_space_error(ui, machine, None);
            }

            self.draw_pathing_opts(ui, machine, available);
            if available && self.katrender() {
                ui.separator();
                let label = fl!("pathing-window");
                if ui.button(&label) {
                    crate::control_window(crate::WINDOW_PATHING, None);
                }
            }

            with_i18n!("experimental-notice", |msg| ui.text_wrapped(&msg));
        }

        ui.next_column();

        if let Some(_container) = ui.begin_sidebar(c"pathing_secondary") {
            self.draw_map_opts(ui);

            #[cfg(feature = "goggles")]
            if let Some(true) = available {
                let tree_goggles = with_i18n!("pathing-config-goggles", |label| ui
                    .begin_sidebar_tree_node(
                        ImCondition::initial(true),
                        c"pathing-config-goggles",
                        label,
                    ));
                if let Some(_tree) = tree_goggles {
                    let _id = ui.push_id(c"goggles");
                    self.draw_goggles_opts(ui, machine);
                }
            }
        }

        ui.columns(1, "pathing_tab_end", false)
    }

    pub fn draw_space_error<'ui, U>(ui: &mut U, machine: &RenderMachine, e: Option<&anyhow::Error>)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let _font = NexusLinkFont::Big.push_font(ui);
        let e = match e {
            None if !Settings::try_read().map(|s| s.enable_katrender).unwrap_or(true) => {
                ui.with_font(NexusLinkFont::Ui, |ui| {
                    ui.text_wrapped(fl!("experimental-notice"))
                });
                if with_i18n!("enable", |label| ui.button(label)) {
                    PathingController::try_send(PathingEvent::ToggleKatRender);
                }
                None
            },
            None => {
                if machine.gameplay.is_initial() {
                    with_i18n!("render-notice-gameplay-initial", |msg| ui.text_wrapped(msg));
                } else if !Engine::is_available() {
                    with_i18n!("render-notice-gameplay", |msg| ui.text_wrapped(msg));
                } else {
                    // shouldn't happen?
                }
                None
            },
            Some(e) => {
                if !Engine::is_available() {
                    ui.text_wrapped(fl!("render-notice-error"));
                }
                Some(e)
            },
        };
        if let Some(e) = e {
            ui.text_wrapped(im_fmt!("{e:#}"));
        }
    }

    fn draw_header<'ui, U>(&mut self, ui: &mut U)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        {
            let _font = (!self.katrender()).then(|| NexusLinkFont::Big.push_font(ui));
            let enables = self.enables.get_mut();
            if ui.checkbox_flags(fl!("pathing-config-enable"), enables, PathingEnables::KATRENDER) {
                PathingController::try_send(PathingEvent::ToggleKatRender);
            }
        }

        if self.katrender() {
            ui.same_line();
            if ui.button(fl!("render-unload")) {
                self.enables.write_if(|enables| {
                    enables.remove(PathingEnables::KATRENDER);
                    Some(true)
                });
                if let Some(mut settings) = crate::SETTINGS.get().map(|s| s.blocking_write())  {
                    settings.enable_katrender = false;
                }
                RenderState::try_send(RenderEvent::ReloadAll);
            }

            ui.same_line();
            if ui.button(fl!("render-reload")) {
                RenderState::try_send(RenderEvent::ReloadAll);
            }
        } else {
            //ui.text_with_font(NexusLinkFont::Ui fl!("pathing-config"));
            ui.text_disabled(fl!("pathing-notice-space"));
        }
    }

    fn combo_setting<'ui, U, T>(ui: &mut U, mut label: impl ImStrExt, value: T) -> Option<Option<T>>
    where
        U: ?Sized + ImDrawWindow<'ui>,
        T: VariantArray + Eq + Copy + Into<&'static str>,
    {
        let (combo, _token) = label.with_imstr_dyn(|label| {
            let label = label.im_take_cstring();
            let _token = ui.push_id(&label);
            let current_id: &str = value.into();
            let combo = with_i18n!(current_id, |preview| ui.begin_combo(label, preview));
            (combo, _token)
        });
        let mut selected = None;
        if let Some(_combo) = combo {
            for &opt in T::VARIANTS {
                let name: &str = opt.into();
                if with_i18n!(name, |name| ui.selectable(name, value == opt)) {
                    selected = Some(Some(opt.clone()));
                }
            }
        } else if ui.is_item_right_clicked() {
            // reset to default
            selected = Some(None);
        }
        selected
    }

    fn slider_setting<'ui, U>(
        ui: &mut U,
        label: impl ImStrExt,
        value: f32,
        range: (f32, f32),
    ) -> Option<Option<f32>>
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        Self::slider_setting_inner(ui, label, value, range, None)
    }
    fn slider_setting_int<'ui, U>(
        ui: &mut U,
        label: impl ImStrExt,
        value: i32,
        (min, max): (i32, i32),
    ) -> Option<Option<i32>>
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let range = (min as f32, max as f32);
        match Self::slider_setting_inner(ui, label, value as f32, range, Some(c"%.0f")) {
            Some(Some(new)) => Some(Some(new as i32)),
            Some(None) => Some(None),
            None => None,
        }
    }
    fn slider_setting_inner<'ui, U>(
        ui: &mut U,
        label: impl ImStrExt,
        mut value: f32,
        (min, max): (f32, f32),
        format: Option<&'static CStr>,
    ) -> Option<Option<f32>>
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let changed = ui.slider(label, &mut value, min..=max, format);
        // TODO: right-click to reset or something?
        match changed {
            true => Some(Some(value)),
            false => {
                if ui.is_item_right_clicked() {
                    // reset to default
                    return Some(None)
                }
                None
            },
        }
    }

    fn slider_opt_alpha<'ui, U>(
        ui: &mut U,
        label: impl ImStrExt,
        value: f32,
        initial: Option<f32>,
    ) -> Option<Option<f32>> where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        Self::slider_opt_mult(ui, label, value, Self::RANGE_ALPHA, initial)
    }
    fn slider_opt_mult<'ui, U>(
        ui: &mut U,
        label: impl ImStrExt,
        value: f32,
        range: (f32, f32),
        initial: Option<f32>,
    ) -> Option<Option<f32>> where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let value = match value {
            v if v <= 0.0 => None,
            v => Some(v),
        };
        match Self::slider_opt_setting_with_initial(ui, label, value, range, initial) {
            Some(Some(SpaceSettings::NONE_F32)) if range.0 > 0.0 =>
                Some(Some(0.0)),
            res => res,
        }
    }
    fn slider_opt_setting<'ui, U>(
        ui: &mut U,
        label: impl ImStrExt,
        value: Option<f32>,
        range: (f32, f32),
    ) -> Option<Option<f32>>
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        Self::slider_opt_setting_with_initial(ui, label, value, range, None)
    }

    fn slider_opt_setting_with_initial<'ui, U>(
        ui: &mut U,
        mut label: impl ImStrExt,
        value: Option<f32>,
        range: (f32, f32),
        initial: Option<f32>,
    ) -> Option<Option<f32>>
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let mut enabled = value.is_some();
        ui.unindent();
        let _token = label.with_imstr_dyn(|label| {
            let label = label.im_take_cstring();
            ui.push_id(&label)
        });
        let mut res = if ui.checkbox(c"", &mut enabled) {
            Some(match enabled {
                false if matches!(initial, Some(SpaceSettings::NONE_F32)) => None,
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
            ui.label_text(label, fl!("disabled"));
            res = res.or_else(|| if ui.is_item_right_clicked() {
                Some(None)
            } else {
                ui.is_item_clicked().then_some(initial)
            });
        }
        ui.indent();
        res
    }

    fn toggle_setting<'ui, U>(
        ui: &mut U,
        id: &str,
        value: &mut bool,
    ) -> Option<Option<bool>> where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        if with_i18n!(id, |label| ui.checkbox(&label, value)) {
            Some(Some(*value))
        } else if ui.is_item_right_clicked() {
            Some(None)
        } else {
            None
        }
    }

    fn set_pathing<F: FnOnce(&mut PathingSettings)>(f: F) {
        let res = Settings::write_with_blocking(|s| f(s.pathing_mut().into_mut()))
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

    const RANGE_ALPHA: (f32, f32) = (0.01, 1.0);
    const RANGE_SCALE: (f32, f32) = (0.0, 25.0);
    const RANGE_SCALE_MULT5: (f32, f32) = (0.0, 5.0);
    const RANGE_SCALE_MULT10: (f32, f32) = (0.1, 10.0);
    //const RANGE_SCALE_POI: (f32, f32) = (-1.0, 10.0);
    const RANGE_SCALE_POI: (f32, f32) = Self::RANGE_SCALE_MULT5;
    const RANGE_SCALE_MAP: (f32, f32) = Self::RANGE_SCALE;
    fn draw_pathing_opts<'ui, U>(&mut self, ui: &mut U, machine: &mut RenderMachine, available: bool) -> Option<()>
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let (
            load_simultaneous,
            camera_source,
            mut visible_space,
            player_overlap_threshold,
            mut player_overlap_poi,
            mut distance_fade_range,
            distance_fade_intensity,
            mut distance_ordering,
            distance_max,
            trail_y_offset,
            trail_resolution,
            trail_width,
            mut trail_textured_space,
            trail_alpha,
            poi_alpha,
            scale_trail_space,
            scale_poi_space,
            mut scale_poi_limit,
            anim_trail_space,
            mut arcrender,
            edge_feather_scale,
            (edge_scale,),
        ) = Self::get_pathing(|s| {
            (
                s.load_simultaneous(),
                s.space.camera_source(),
                s.space.visible_space(),
                s.space.player_overlap_threshold(),
                s.space.player_overlap_poi(),
                s.space.distance_fade_range(),
                s.space.distance_fade_intensity(),
                s.space.distance_ordering(),
                s.space.distance_max(),
                s.space.trail_y_offset(),
                s.space.trail_resolution(),
                s.space.trail_width(),
                s.space.trail_textured_space(),
                s.space.trail_alpha(),
                s.space.poi_alpha(),
                s.space.trail_scale_space(),
                s.space.poi_scale_space(),
                s.space.poi_limit_size(),
                s.space.trail_anim_space(),
                s.space.goggles.arcrender_enabled(),
                s.space.edge_feather_scale(),
                match () {
                    #[cfg(feature = "goggles")]
                    _ => (s.space.goggles.edge_scale(),),
                    #[cfg(not(feature = "goggles"))]
                    _ => ((),),
                },
            )
        })?;

        if ui.checkbox(fl!("pathing-render-toggle"), &mut visible_space) {
            Self::set_pathing(|s| s.space.visible_space = Some(visible_space));
            #[cfg(deleteme)]
            #[cfg(feature = "goggles")]
            Engine::try_send(match visible_space {
                true => SpaceEvent::GogglesRefreshLens { force: false, delay_override: Some(2) },
                false => SpaceEvent::GogglesClearLens,
            });
        }
        ui.same_line();
        if ui.checkbox(fl!("pathing-config-textured"), &mut trail_textured_space) {
            Self::set_pathing(|s| s.space.trail_textured_space = Some(trail_textured_space));
        }
        if ui.checkbox("arcrender", &mut arcrender) {
            let flag = GogglesEnables::ARCRENDER_ENABLE;
            Self::set_pathing(|s| {
                s.space.goggles.set_enables(arcrender.then_some(flag).unwrap_or_default(), flag);
                machine.goggles.enabled_config.set(flag, arcrender);
            });
        }
        self.arcrender_enabled = arcrender;

        with_i18n!("pathing-config-reset-notice", |msg| ui.text_wrapped(&msg));

        ui.indent();
        if let Some(value) = Self::slider_opt_alpha(
            ui,
            fl!("pathing-config-trail-alpha"),
            trail_alpha,
            None,
        ) {
            Self::set_pathing(|s| s.space.trail_alpha = value);
        }
        if trail_alpha > 0.0 {
            if let Some(value) = Self::slider_setting(
                ui,
                fl!("pathing-config-trail-scale"),
                scale_trail_space,
                Self::RANGE_SCALE,
            ) {
                Self::set_pathing(|s| s.space.scale_trail_space = value);
            }
            if self.arcrender() {
                if let Some(value) = Self::slider_opt_mult(
                    ui,
                    fl!("pathing-config-trail-anim"),
                    anim_trail_space,
                    Self::RANGE_SCALE_MULT10,
                    Some(SpaceSettings::DEFAULT_TRAIL_ANIM),
                ) {
                    Self::set_pathing(|s| s.space.anim_trail_space = value);
                }
            }
        }
        if let Some(value) =
            Self::slider_opt_alpha(ui, fl!("pathing-config-poi-alpha"), poi_alpha, None)
        {
            Self::set_pathing(|s| s.space.poi_alpha = value);
        }
        if poi_alpha > 0.0 {
            ui.indent();
            if let Some(value) = Self::slider_setting(
                ui,
                fl!("pathing-config-poi-scale"),
                scale_poi_space,
                Self::RANGE_SCALE_POI,
            ) {
                Self::set_pathing(|s| s.space.scale_poi_space = value);
            }
            if arcrender {
                if let Some(value) = Self::toggle_setting(ui, "pathing-config-poi-scale-limit", &mut scale_poi_limit) {
                    Self::set_pathing(|s| s.space.poi_limit_size = value);
                }
            }
            ui.unindent();
        }
        if arcrender {
            if with_i18n!("pathing-config-distance-order", |label| ui.checkbox(&label, &mut distance_ordering)) {
                Self::set_pathing(|s| s.space.distance_ordering = Some(distance_ordering));
            }
        }
        if let Some(value) = Self::slider_opt_setting(
            ui,
            fl!("pathing-config-distance-fade-intensity"),
            distance_fade_intensity,
            (1.0, 500.0),
        ) {
            Self::set_pathing(|s| s.space.distance_fade_intensity = value);
        }
        if arcrender {
            if let Some(value) = Self::toggle_setting(ui, "pathing-config-distance-fade-range", &mut distance_fade_range) {
                Self::set_pathing(|s| s.space.distance_fade_range = value);
            }
        }
        if let Some(value) = Self::slider_opt_setting(
            ui,
            fl!("pathing-config-player-overlap-threshold"),
            player_overlap_threshold,
            (0.01, 1000.0),
        ) {
            Self::set_pathing(|s| s.space.player_overlap_threshold = value);
        }
        if poi_alpha > 0.0 && arcrender {
            ui.indent();
            if let Some(value) = Self::toggle_setting(ui, "pathing-config-player-overlap-poi", &mut player_overlap_poi) {
                Self::set_pathing(|s| s.space.player_overlap_poi = value);
            }
            ui.unindent();
        }
        if let Some(value) =
            Self::slider_opt_setting(ui, fl!("pathing-config-edge-feather-scale"), edge_feather_scale, (0.001f32, 5.0))
        {
            Self::set_pathing(|s| s.space.edge_feather_scale = value);
        }
        #[cfg(feature = "goggles")]
        if let Some(value) =
            Self::slider_opt_mult(ui, fl!("pathing-config-corner-boundary-scale"), edge_scale.unwrap_or(0.0), (0.1f32, 5.0), Some(/*GogglesSettings::DEFAULT_EDGE_SCALE*/ 0.5))
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
            fl!("pathing-config-distance-max"),
            distance_max,
            (1.0, 2000.0),
        ) {
            Self::set_pathing(|s| s.space.distance_max = value);
        }
        ui.unindent();
        #[cfg(any(feature = "extension-nexus", feature = "goggles2-camera"))]
        if let Some(value) = Self::combo_setting(ui, fl!("pathing-config-camera-source"), camera_source) {
            Self::set_pathing(|s| s.space.camera_source = value);
            #[cfg(feature = "extension-nexus")]
            if value == Some(CameraSource::RealTimeAPI) {
                match machine.rtapi_init() {
                    Err(e) => log::warn!("{e:#}"),
                    Ok(false) =>
                        log::warn!("RTAPI inactive - make sure the addon is installed and loaded by Nexus"),
                    Ok(true) => (),
                }
            }
            #[cfg(feature = "goggles2-camera")]
            if value == Some(CameraSource::Goggles2) {
                machine.goggles.enabled_config.insert(GogglesEnables::ENABLE | GogglesEnables::CAMERA_ENABLE | GogglesEnables::CAMERA_DIR);
            }
        }
        #[cfg(any(feature = "extension-nexus", feature = "goggles2-camera"))]
        match camera_source {
            CameraSource::MumbleLink => with_i18n!("pathing-notice-mumblelink", |msg| ui.text_wrapped(msg)),
            #[cfg(feature = "extension-nexus")]
            CameraSource::RealTimeAPI => {
                if machine.rtapi.is_none() {
                    with_i18n!("pathing-notice-rtapi-missing", |msg| ui.text_wrapped(&msg));
                }
                with_i18n!("pathing-notice-rtapi", |msg| ui.text_wrapped(&msg));
            },
            #[cfg(feature = "goggles2-camera")]
            CameraSource::Goggles2 => {
                ui.text_wrapped("goggles must also be enabled");
            },
            #[cfg(not(feature = "goggles2-camera"))]
            CameraSource::Goggles2 => {
                #[cfg(not(feature = "goggles2-camera"))]
                {
                    ui.text_wrapped("missing");
                }
            },
        }

        let filters_tree = with_i18n!("pathing-config-filters", |label| ui.begin_tree_node_framed(
            ImCondition::startup(true),
            c"pathing-config-filters",
            label,
            false,
        ));
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

            let festivals_tree = with_i18n!("pathing-config-festivals", |label| ui.begin_tree_node_framed(
                ImCondition::startup(false),
                c"pathing-config-festivals",
                label,
                false,
            ));
            if let Some(_tree) = festivals_tree {
                self.draw_festival_opts(ui)
            }
        }

        let advanced = with_i18n!("pathing-config-advanced", |label| ui.begin_tree_node_framed(
            ImCondition::startup(false),
            c"pathing-config-advanced",
            label,
            false,
        ));
        if let Some(_tree) = advanced {
            if let Some(value) = Self::slider_setting_int(
                ui,
                fl!("pathing-config-load-simultaneous"),
                load_simultaneous as i32,
                (1, 99),
            ) {
                let value = value.map(|v| v.max(1) as usize);
                Self::set_pathing(|s| s.load_simultaneous = value);
            }
            ui.text_wrapped(fl!("pathing-config-trail-notice"));
            if let Some(value) = Self::slider_opt_setting(
                ui,
                fl!("pathing-config-trail-y-offset"),
                trail_y_offset,
                (-1.0, 1.0),
            ) {
                Self::set_pathing(|s| s.space.trail_y_offset = value);
            }
            if let Some(value) = Self::slider_setting(
                ui,
                fl!("pathing-config-trail-resolution"),
                trail_resolution,
                (0.001, 5.0),
            ) {
                Self::set_pathing(|s| s.space.trail_resolution = value);
            }
            if let Some(value) =
                Self::slider_setting(ui, fl!("pathing-config-trail-width"), trail_width, (0.01, 25.0))
            {
                Self::set_pathing(|s| s.space.trail_width = value);
            }
            #[cfg(feature = "paths-lua")]
            {
                let enables = self.enables.get_mut();
                if ui.checkbox_flags("scripts", enables, PathingEnables::SCRIPTING_LUA) {
                    Self::set_pathing(|s| {
                        s.scripting_enable = enables.contains(PathingEnables::SCRIPTING_LUA)
                    });
                    PathingEvent::ScriptsEnable(Some(enables.contains(PathingEnables::SCRIPTING_LUA)))
                        .try_send();
                }
                if ui.item_is_hovered() {
                    ui.tooltip_text("EXPERIMENTAL AND PROBABLY BROKEN");
                }
                if enables.contains(PathingEnables::SCRIPTING_LUA) {
                    let (mut tick_rate, mut autostart) =
                        Self::get_pathing(|s| (s.scripting_tick_rate, s.scripting_auto))?;
                    ui.same_line();
                    if ui.checkbox("autostart", &mut autostart) {
                        Self::set_pathing(|s| s.scripting_auto = autostart);
                    }
                    ui.same_line();
                    if ui.checkbox_flags("unsecured", enables, PathingEnables::SCRIPTING_UNSECURED) {
                        Self::set_pathing(|s| {
                            s.scripting_unsecured = enables.contains(PathingEnables::SCRIPTING_UNSECURED)
                        });
                    }
                    if ui.slider("tickrate", &mut tick_rate, 0.0f32..=2.0f32, IM_STR_NONE) {
                        Self::set_pathing(|s| s.scripting_tick_rate = tick_rate);
                    }
                }
            }
            self.draw_advanced_opts(ui, machine, available);
        }

        Some(())
    }

    fn draw_map_opts<'ui, U>(&mut self, ui: &mut U) -> Option<()>
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let (
            mut visible_minimap,
            mut visible_worldmap,
            mut map_open,
            mut trail_textured_mini,
            mut trail_textured_world,
            map_trail_alpha_mini,
            map_trail_alpha_world,
            map_poi_alpha_mini,
            map_poi_alpha_world,
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
                s.space.poi_alpha_minimap(),
                s.space.poi_alpha_worldmap(),
                s.space.trail_scale_minimap(),
                s.space.trail_scale_worldmap(),
                s.space.poi_scale_minimap(),
                s.space.poi_scale_worldmap(),
            )
        })?;

        let minimap = with_i18n!("pathing-config-minimap", |label| ui.begin_sidebar_tree_node(
            ImCondition::initial(true),
            c"pathing-config-minimap",
            label,
        ));
        if let Some(_tree) = minimap {
            let _id = ui.push_id(c"minimap");
            //RenderState::font_text("ui", ui, fl!("pathing-config-minimap"));
            if ui.checkbox(fl!("pathing-render-minimap-toggle"), &mut visible_minimap) {
                Self::set_pathing(|s| s.space.visible_map_mini = Some(visible_minimap));
            }
            ui.same_line();
            if ui.checkbox(fl!("pathing-config-textured-minimap"), &mut trail_textured_mini) {
                Self::set_pathing(|s| s.space.map_trail_textured_mini = Some(trail_textured_mini));
            }
            if let Some(value) = Self::slider_opt_alpha(
                ui,
                fl!("pathing-config-trail-alpha-minimap"),
                map_trail_alpha_mini,
                None,
            ) {
                Self::set_pathing(|s| s.space.map_trail_alpha_mini = value);
            }
            if map_trail_alpha_mini > 0.0 {
                if let Some(value) = Self::slider_setting(
                    ui,
                    &fl!("pathing-config-trail-scale-minimap"),
                    scale_trail_mini,
                    Self::RANGE_SCALE_MAP,
                ) {
                    Self::set_pathing(|s| s.space.scale_trail_mini = value);
                }
            }
            if let Some(value) = Self::slider_opt_alpha(
                ui,
                fl!("pathing-config-poi-alpha-minimap"),
                map_poi_alpha_mini,
                None,
            ) {
                Self::set_pathing(|s| s.space.map_poi_alpha_mini = value);
            }
            if map_poi_alpha_mini > 0.0 {
                if let Some(value) = Self::slider_setting(
                    ui,
                    fl!("pathing-config-poi-scale-minimap"),
                    scale_poi_mini,
                    Self::RANGE_SCALE_POI,
                ) {
                    Self::set_pathing(|s| s.space.scale_poi_mini = value);
                }
            }
        }

        let worldmap = with_i18n!("pathing-config-worldmap", |label| ui.begin_sidebar_tree_node(
            ImCondition::initial(true),
            c"pathing-config-worldmap",
            label,
        ));
        if let Some(_tree) = worldmap {
            let _id = ui.push_id(c"worldmap");
            if ui.checkbox(fl!("pathing-render-map-toggle"), &mut visible_worldmap) {
                Self::set_pathing(|s| s.space.visible_map_world = Some(visible_worldmap));
            }
            ui.same_line();
            if ui.checkbox(fl!("pathing-config-textured-worldmap"), &mut trail_textured_world) {
                Self::set_pathing(|s| s.space.map_trail_textured_world = Some(trail_textured_world));
            }
            ui.same_line();
            if ui.checkbox(fl!("pathing-config-map-open"), &mut map_open) {
                Self::set_pathing(|s| s.space.map_open = Some(map_open));
            }
            if let Some(value) = Self::slider_opt_alpha(
                ui,
                fl!("pathing-config-trail-alpha-worldmap"),
                map_trail_alpha_world,
                None,
            ) {
                Self::set_pathing(|s| s.space.map_trail_alpha_world = value);
            }
            if map_trail_alpha_world > 0.0 {
                if let Some(value) = Self::slider_setting(
                    ui,
                    &fl!("pathing-config-trail-scale-worldmap"),
                    scale_trail_world,
                    Self::RANGE_SCALE_MAP,
                ) {
                    Self::set_pathing(|s| s.space.scale_trail_world = value);
                }
            }
            if let Some(value) = Self::slider_opt_alpha(
                ui,
                fl!("pathing-config-poi-alpha-worldmap"),
                map_poi_alpha_world,
                None,
            ) {
                Self::set_pathing(|s| s.space.map_poi_alpha_world = value);
            }
            if map_poi_alpha_world > 0.0 {
                if let Some(value) = Self::slider_setting(
                    ui,
                    fl!("pathing-config-poi-scale-worldmap"),
                    scale_poi_world,
                    Self::RANGE_SCALE_POI,
                ) {
                    Self::set_pathing(|s| s.space.scale_poi_world = value);
                }
            }
        }

        Some(())
    }

    fn draw_festival_opts<'ui, U>(&mut self, ui: &mut U)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let Some(festivals) = ApiController::active_festivals() else { return };
        let mut change = None;
        for festival in Festival::all() {
            let selected = festivals.get_preference(festival);
            let active = festivals.active.get(festival);
            let selection = with_i18n!(festival.as_str(), |name| {
                let title = match active {
                    false => name,
                    true => fl!("pathing-config-festival-active", festival = &name[..]).into(),
                };
                ui.selectable(title, selected.unwrap_or(active))
            });
            if selection {
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

    fn draw_advanced_opts<'ui, U>(&mut self, ui: &mut U, _machine: &RenderMachine, available: bool) where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let debug_token = with_i18n!("config-debug-controls", |label| ui.begin_sidebar_tree_node(
            ImCondition::startup(false),
            c"config-debug-controls",
            label,
        ));
        if let Some(_token) = debug_token {
            if available && self.katrender() {
                if ui.button("invalidate shaders") {
                    let all = !self.arcrender_enabled;
                    Engine::try_send(SpaceEvent::ReloadShaders(all));
                }
            } else {
                with_i18n!("inactive", |msg| ui.text_disabled(&msg));
            }
        }
    }

    #[cfg(feature = "paths-interact")]
    fn draw_interaction_opts<'ui, U>(&mut self, ui: &mut U) where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let settings =
            Self::get_pathing(|s| (s.trigger_enable, s.trigger_allow_auto, s.trigger_allow_interact, s.interact_base_responsiveness));
        let Some((mut trigger_enable, trigger_allow_auto, trigger_allow_interact, responsiveness)) = settings else {
            return
        };
        let mut settings_dirty = false;

        ui.unindent();
        let enable_toggled = {
            let _id = ui.push_id("trigger_enable");
            ui.checkbox("", &mut trigger_enable)
        }
        .then(move || trigger_enable);
        let mut trigger_reset = trigger_enable && ui.is_item_right_clicked();
        ui.same_line();

        let interaction_tree = ui.begin_sidebar_tree_node(
            ImCondition::startup(false),
            c"pathing-config-interactions",
            fl!("pathing-config-interactions"),
        );
        trigger_reset |= ui.is_item_right_clicked();
        ui.indent();

        let mut interact_toggled = None;
        let set_interact = if let Some(_tree) = interaction_tree {
            if let Some(v) = Self::slider_setting_inner(ui, fl!("pathing-config-interactions-responsiveness"), responsiveness, (0.01f32, 4.0f32), Some(c"%.03fs")) {
                Self::set_pathing(|s| s.interact_base_responsiveness = v.unwrap_or(PathingSettings::DEFAULT_INTERACT_RESPONSIVENESS));
                settings_dirty = true;
            }

            let _id = ui.push_id("trigger_allow_interact");

            ui.unindent();
            let mut interact_enabled = !trigger_allow_interact.is_empty();
            interact_toggled = {
                let _id = ui.push_id("trigger_any_interact");
                ui.checkbox("", &mut interact_enabled)
            }
            .then(move || interact_enabled);
            ui.indent();
            ui.same_line();

            self.draw_trigger_opts(ui, trigger_allow_interact)
        } else {
            None
        };

        let mut auto_enabled = !trigger_allow_auto.is_empty();
        let auto_toggled = {
            let _id = ui.push_id("trigger_any_auto");
            ui.checkbox("", &mut auto_enabled)
        }
        .then(move || auto_enabled);
        ui.same_line();
        let autotrigger_tree = ui.begin_sidebar_tree_node(
            ImCondition::startup(false),
            c"pathing-config-autotrigger",
            fl!("pathing-config-autotrigger"),
        );
        ui.indent();
        let set_auto = if let Some(_tree) = autotrigger_tree {
            let _id = ui.push_id("trigger_allow_auto");
            with_i18n!("pathing-config-autotrigger-notice", |msg| ui.text_wrapped(msg));
            self.draw_trigger_opts(ui, trigger_allow_auto)
        } else {
            None
        };

        let set_interact = set_interact.or(interact_toggled.map(|reset| match reset {
            true => None,
            false => Some(TriggerKind::empty()),
        }));
        let set_auto = set_auto.or(auto_toggled.map(|reset| match reset {
            true => None,
            false => Some(TriggerKind::empty()),
        }));
        if let Some(set) = set_auto {
            Self::set_pathing(|s| {
                s.trigger_allow_auto = set.unwrap_or(TriggerKind::settings_default_auto())
            });
            settings_dirty = true;
        } else if let Some(set) = set_interact {
            Self::set_pathing(|s| {
                s.trigger_allow_interact = set.unwrap_or(TriggerKind::settings_default_interact())
            });
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
    fn draw_trigger_opts<'ui, U>(&mut self, ui: &mut U, mut setting: TriggerKind) -> Option<Option<TriggerKind>> where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let mut changed = false;
        let mut reset = false;
        for (i, flag) in TriggerKind::SETTINGS_GUI.into_iter().enumerate() {
            changed |= with_i18n!(flag.flag_str().unwrap_or_default(), |msg| {
                if i > 0 {
                    ui.reserve_line_checkbox(&msg);
                }
                ui.checkbox_flags(
                    msg,
                    &mut setting,
                    flag
                )
            });
            if ui.is_item_right_clicked() {
                reset = true;
            }
        }
        setting.set(
            TriggerKind::SETTINGS_TOGGLE_SHOWHIDE,
            setting.contains(TriggerKind::TOGGLE),
        );
        match (reset, changed) {
            (true, _) => Some(None),
            (_, changed) => changed.then_some(setting).map(Some),
        }
    }

    #[cfg(feature = "goggles")]
    fn draw_goggles_opts<'ui, U>(&mut self, ui: &mut U, machine: &mut RenderMachine) -> Option<()>
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let map_id = machine.gameplay.gameplay_map();
        let farz = machine.get_depth_calibration();
        let Range { start: near, end: far } = machine.depth_range();
        let (mut enables, obscured_alpha, obscured_distance, obscured_distance_effective, farz_set, farz_seen) =
            Self::get_pathing(|s| (
                s.space.goggles.enables(), s.space.goggles.obscured_alpha(), s.space.goggles.obscured_distance(), s.space.obscured_distance(),
                map_id.and_then(|map| s.space.goggles.get_map_depth_setting(map.get())),
                map_id.and_then(|map| s.space.goggles.map_proj_seen.get(&map.get()).cloned()),
            ))?;

        ui.text_wrapped(fl!("pathing-config-goggles-notice"));

        ui.indent();
        let mut enables_commit = GogglesEnables::empty();
        for (i, &enable) in GogglesEnables::UI_ENABLES.iter().enumerate() {
            let label = match enable {
                GogglesEnables::ENABLE => Cow::Borrowed("enable"),
                #[cfg(todo = "unnecessary")]
                enable if !GogglesEnables::SUPPORTED_FEATURES.contains(enable) => continue,
                enable if !GogglesEnables::FEATURE_ENABLES.contains(enable) => continue,
                _ => Cow::Owned(format!("pathing-config-goggles-{enable}"))
            };
            let toggled = with_i18n(&label, |label| {
                if i > 0 {
                    ui.reserve_line_checkbox(&label);
                }
                ui.checkbox_flags(&label, &mut enables, enable)
            });
            let toggled = match (toggled, enable) {
                (false, GogglesEnables::ENABLE) if ui.is_item_right_clicked() => {
                    Self::set_pathing(|s| {
                        s.space.goggles.reset_enables();
                        enables = s.space.goggles.enables();
                    });
                    machine.goggles.enabled_config = enables;
                    enable
                },
                (toggled, enable) => {
                    let toggled = toggled.then_some(enable).unwrap_or(GogglesEnables::empty());
                    enables_commit.insert(toggled);
                    toggled
                },
            };
        }

        #[cfg(feature = "goggles2-project")]
        if enables.contains(GogglesEnables::PROJECT_ENABLE) {
            self.goggles.draw_project_options(ui, machine);
            let mut options_commit = Self::goggles_feature_opts(ui, &mut enables, GogglesEnables::PROJECT_ENABLE, GogglesEnables::OPTIONS_PROJECT.difference(GogglesEnables::OPTIONS_PROJECT_COMPAT));
            // split for visual separation/grouping
            options_commit |= Self::goggles_feature_opts(ui, &mut enables, GogglesEnables::PROJECT_ENABLE, GogglesEnables::OPTIONS_PROJECT_COMPAT);
            enables_commit.insert(options_commit);
        }

        if enables.contains(GogglesEnables::LENS_ENABLE) {
            if let Some(value) = Self::slider_opt_alpha(ui, "x-ray opacity", obscured_alpha, None) {
                Self::set_pathing(|s| s.space.goggles.obscured_alpha = value);
            }
            if !matches!(obscured_alpha, 0.0f32 | SpaceSettings::NONE_F32) {
                if let Some(value) = Self::slider_setting(ui, "x-ray distance", obscured_distance, (0.01, 1.0)) {
                    Self::set_pathing(|s| s.space.goggles.obscured_distance = value);
                }
                if ui.is_item_hovered() {
                    if let Some(_tt) = ui.begin_tooltip() {
                        ui.text(im_fmt!("{obscured_distance_effective}m @ {:.01}%", obscured_distance * 100.0));
                        with_i18n!("pathing-config-goggles-distance-notice", |msg| ui.text(&msg));
                    }
                }
            }
        }

        #[cfg(feature = "goggles2-camera")]
        if enables.contains(GogglesEnables::CAMERA_ENABLE) {
            let options_commit = Self::goggles_feature_opts_all(ui, &mut enables, GogglesEnables::CAMERA_ENABLE);
            enables_commit.insert(options_commit);
        }
        if !machine.goggles.is_enabled(GogglesEnables::CAMERA_ENABLE | GogglesEnables::CAMERA_PERSPECTIVE) {
            ui.unindent();
            #[cfg(todo)]
            {
                let _font = NexusLinkFont::Big.push_font(ui);
                ui.text_wrapped(
                    "For good goggles, you will need to adjust the \"near\" slider for each new map you visit.",
                );
            }
            ui.text_wrapped(
                c"Try sliding focus down until paths stop disappearing under the ground"
            );
            ui.indent();
        }
        if enables.intersects(GogglesEnables::PROJECT_ENABLE | GogglesEnables::LENS_ENABLE | GogglesEnables::CAMERA_PERSPECTIVE) {
            if let Some(map_id) = map_id {
                let map_id = map_id.get();
                let map_scale = machine.map.calibration.local_space();

                let v2 = farz_set.and_then(|v| v.as_v2_preset());
                let value = v2.clone()
                    .or_else(|| farz_set.and_then(|v|
                        machine.fov_y().and_then(|fovy| v.reinterpret_v1_as_v2(fovy, &map_scale))
                    ))
                ;
                let has_farz = farz.is_some();
                let mut new_depth = {
                    let farz_set = value.clone().or(farz).unwrap_or(MapProjectionDepth::DEFAULT_FALLBACK);
                    with_i18n!("pathing-config-goggles-depth", |label| Self::slider_setting(ui, &label, farz_set.farz, (GogglesMapDepth::V2_FARZ_SLIDER_START, GogglesMapDepth::V2_FARZ_SLIDER_END)))
                };
                let remove_msg = match (farz_set, v2) {
                    (Some(..), None) => Some("legacy"),
                    (Some(..), Some(..)) => Some("manual"),
                    _ if farz_seen.is_some() => Some("saved"),
                    _ if has_farz && machine.goggles.is_enabled(GogglesEnables::CAMERA_ENABLE | GogglesEnables::CAMERA_PERSPECTIVE) => Some("detected"),
                    _ if has_farz => Some("cached"),
                    _ => None,
                };
                if let Some(msg) = remove_msg {
                    // far slider maybe idk
                    if ui.small_button(format!("remove {msg} map calibration")) {
                        new_depth = Some(None);
                    }
                }
                if let Some(v) = new_depth {
                    let value = v.map(MapProjectionDepth::with_farz)
                        .map(GogglesMapDepth::from);
                    Self::set_pathing(|s| {
                        match value {
                            Some(value) => {
                                machine.set_depth_range(&value);
                                let dest = s.space.goggles.map_depth_calibration_mut();
                                dest.insert(map_id, value.into());
                            },
                            None => {
                                machine.depth_range = None;
                                if farz_set.is_some() {
                                    let dest = s.space.goggles.map_depth_calibration_mut();
                                    dest.remove(&map_id);
                                } else if farz_seen.is_some() {
                                    let dest = s.space.goggles.map_proj_seen_mut();
                                    dest.remove(&map_id);
                                } else {
                                    machine.map_depth_guess = None;
                                    if machine.map_depth.is_some() {
                                        machine.map_depth = None;
                                        machine.goggles.enabled_config.remove(GogglesEnables::CAMERA_PERSPECTIVE);
                                    }
                                }
                            },
                        }
                    });
                }
                let map_scale_i2m = machine.depth_scale_i2m();
                let map_scale_m2i = map_scale_i2m.recip();
                ui.text(format!("{:.04}\"..{:.02}\" / {near:.06}m..{far:.03}m", near * map_scale_m2i, far * map_scale_m2i));
                if let Some((_, _, n, f)) = machine.goggles.camera.perspective_params() {
                    ui.same_line();
                    let farz_det = MapProjectionDepth::with_far_in(f);
                    ui.text(format!(" (detected {:.02}: {n:.04}\"..{f:.02}\" / {:.06}m..{:.03}m)", farz_det.farz, n * map_scale_i2m, f * map_scale_i2m));
                }
            }
        }
        #[cfg(any(feature = "goggles2-project", feature = "goggles2-camera"))]
        if enables.intersects(GogglesEnables::FEATURE_ENABLES) {
            let lenses_tree = ui.begin_sidebar_tree_node(
                ImCondition::startup(false),
                c"pathing-config-goggles-debug",
                c"debug controls",
            );
            if let Some(_tree) = lenses_tree {
                let _id = ui.push_id(c"debug controls");
                self.goggles.draw_debug_toggles(ui, machine);
            }
        }
        if machine.goggles.is_classifying() {
            #[cfg(taimi_debug)]
            let info_tree = ui.begin_sidebar_tree_node(
                ImCondition::startup(false),
                c"pathing-config-goggles-info",
                c"debug lens info",
            );
            #[cfg(taimi_debug)]
            if let Some(_node) = info_tree {
                let _id = ui.push_id(c"debug lens info");
                self.goggles.draw_debug_lens2(ui, machine);
            }
        }

        ui.unindent();

        if !enables_commit.is_empty() {
            Self::set_pathing(|s| {
                s.space.goggles.set_enables(enables, enables_commit);
                enables = s.space.goggles.enables();
            });
            match enables_commit {
                #[cfg(todo)]
                commit =>
                    machine.goggles.enabled_config ^= enables_commit,
                _commit =>
                    machine.goggles.enabled_config = enables,
            }
        }

        Some(())
    }
    #[cfg(feature = "goggles")]
    fn goggles_feature_opts_all<'ui, U>(ui: &mut U, enables: &mut GogglesEnables, enable: GogglesEnables) -> GogglesEnables where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        Self::goggles_feature_opts(ui, enables, enable, enable.options_mask())
    }
    #[cfg(feature = "goggles")]
    fn goggles_feature_opts<'ui, U>(ui: &mut U, enables: &mut GogglesEnables, _enable: GogglesEnables, opts: GogglesEnables) -> GogglesEnables where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let mut commit = GogglesEnables::empty();
        for (i, opt) in opts.iter().enumerate() {
            let label = Cow::Owned(format!("pathing-config-goggles-{opt}"));
            let toggled = with_i18n(&label, |label| {
                if i > 0 {
                    ui.reserve_line_checkbox(&label);
                }
                ui.checkbox_flags(&label, enables, opt)
            });
            let tooltip_id = match opt {
                _ if !ui.is_item_hovered() => None,
                GogglesEnables::CAMERA_PERSPECTIVE =>
                    Some(format!("pathing-config-goggles-{opt}-tip")),
                _ => None,
            };
            if let Some(tip) = tooltip_id {
                with_i18n!(&tip, |msg| match &msg[..] {
                    id if id == &tip[..] => (),
                    msg => ui.tooltip_text(&msg),
                });
            }
            commit |= opt.r#if(toggled)
        }
        commit
    }
}
