use {
    crate::{
        exports::runtime as rt,
        render::{
            element::prelude::*,
            machine::RenderMachine,
        },
        settings::goggles::GogglesEnables,
        space::goggles::{self, class::ClassShared, D3dPtr},
    },
    anyhow::Context,
    std::{mem, thread},
    strum::VariantArray,
    taimi_hoard::lazyfmt,
    windows::{core::Interface, Win32::Graphics::Direct3D11::ID3D11DeviceContext_Vtbl},
};

#[derive(Default)]
pub(super) struct GogglesConfig {
    pub view_lens: D3dPtr,
    pub view_lens_info: String,
}
impl GogglesConfig {
    #[cfg(feature = "goggles2-project")]
    pub fn draw_project_options<'ui, U>(ui: &mut U, machine: &mut RenderMachine)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        use {
            crate::goggles::project::ProjectMethod,
            strum::VariantArray,
        };

        if machine.goggles.enabled_config.contains(GogglesEnables::PROJECT_ENABLE) {
            let selected_mode = machine.goggles.project.method();
            let preview: &str = selected_mode.into();
            let mut new_mode = None;
            if let Some(combo) = ui.begin_combo("Project Method", preview) {
                for &mode in ProjectMethod::VARIANTS {
                    let modename: &str = mode.into();
                    let selected = imgui::Selectable::new(modename)
                        .selected(selected_mode == mode)
                        .build(ui);
                    if selected {
                        new_mode = Some(mode)
                    }
                }
                combo.end();
            }
            if ui.is_item_clicked_with_button(MouseButton::Right) {
                new_mode = Some(Default::default());
            }
            if let Some(new_mode) = new_mode {
                machine.goggles.project.set_method(new_mode);
            }
        }
    }
    #[cfg(any(feature = "goggles2-project", feature = "goggles2-camera"))]
    pub fn draw_debug_toggles<'ui, U>(&mut self, ui: &mut U, machine: &mut RenderMachine)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        #[cfg(feature = "goggles2-project")]
        if machine.goggles.enabled_config.contains(GogglesEnables::PROJECT_ENABLE) {
            let _ = ui.checkbox("blen", &mut machine.goggles.project.project_blend_force);
            ui.same_line();
            let _ = ui.checkbox("dfill", &mut machine.goggles.project.project_depth_fill);
            ui.same_line();
            let _ = ui.checkbox("dvp", &mut machine.goggles.project.project_viewport_force);

            let _ = ui.checkbox("detect", &mut machine.goggles.project.debug_detect);
            if machine.goggles.project.debug_detect {
                ui.same_line();
                let _ = ui.checkbox("all", &mut machine.goggles.project.debug_detect_all);
            }
        }
        #[cfg(feature = "goggles2-project")]
        {
            if machine.goggles.enabled_config.contains(GogglesEnables::PROJECT_ENABLE) {
                ui.same_line();
            }
            if ui.checkbox("shad", &mut machine.goggles.project.project_shadow) {
            }
        }

        #[cfg(feature = "goggles2-camera")]
        if machine.goggles.enabled_config.contains(GogglesEnables::CAMERA_ENABLE) {
            if ui.checkbox("up", &mut machine.goggles.camera.debug_toggle_up) {
            }
            #[cfg(taimi_debug)]
            if ui.is_item_hovered() && machine.goggles.camera.perspective_params.0 != 0.0 {
                ui.tooltip(|| {
                    let (_h, aspect, near, far) = machine.goggles.camera.perspective_params;
                    ui.text(format!("zrange={near:?}..{far:?}({aspect})"));
                    let map_id = machine.gameplay.gameplay_map();
                    let map = map_id.and_then(|map_id| taimi_meta::map::MapCache.lookup_map(map_id.get())
                        .map(|map| (map_id, map))
                    );
                    if let Some((map_id, map)) = map {
                        ui.text(format!("map({map_id})={:?} cont={:?}", map.map_rect(), map.continent_rect()));
                        ui.text(format!("mapsz={:?} contsz={:?}", map.map_rect().size(), map.continent_rect().size()));
                        if let (Some(min), Some(max)) = (map.floors.iter().min(), map.floors.iter().max()) {
                            ui.text(format!("floors={min}..={max}"));
                        }
                    }
                });
            }
            if goggles::FerretResource::has_found_perspective() {
                ui.same_line();
                if ui.checkbox("smooth", &mut machine.goggles.camera.debug_smooth) {
                }
            }
            ui.same_line();
            if ui.checkbox("fuzz", &mut machine.goggles.camera.debug_interpolate) {
            }
            if machine.goggles.camera.debug_interpolate {
                ui.same_line();
                if ui.checkbox("unfuzz", &mut machine.goggles.camera.debug_interpolate_off) {
                }
            }
        }

        #[cfg(feature = "goggles2-project")]
        #[cfg(deleteme)]
        if machine.goggles.enabled_config.contains(GogglesEnables::PROJECT_ENABLE) {
            if let Some(mut request) = goggles::FerretResource::project_target_request() {
                let mut delay = *request.delay.start();
                if imgui::Slider::new("delay", 0u32, 64u32).build(ui, &mut delay) {
                    request.delay = delay..=delay;
                    request.manual_delay = true;
                    goggles::FerretResource::project_set_target_request(Some(request));
                }
                let mut miniz = goggles::g2!(*&ferret.project.minimap_depth_offset);
                if imgui::Slider::new("minimap z-offset", 0.0f32, 8000.0f32).build(ui, &mut miniz) {
                    goggles::g2!(*&mut ferret.project.minimap_depth_offset = miniz)
                }
            }
        }

        #[cfg(deleteme)]
        if false {
        ui.same_line();
        if ui.button("search") {
            let min = 144;
            let min = 60;
            let max = 224;
            let max = 320;
            let max = 0x4080;
            goggles::FerretResource::set_granularity(4);
            goggles::FerretResource::set_size_range(min..(max + 1));
        }
        ui.same_line();
        if ui.button("clr") {
            goggles::FerretResource::set_perspective(goggles::PerspectiveFerret::EMPTY);
            goggles::FerretResource::set_camera(goggles::CameraFerret::EMPTY);
            goggles::FerretResource::set_size_range(8..8);
            goggles::FerretResource::clear_camera_found();
            goggles::FerretResource::clear_perspective_found();
            machine.goggles.camera_enabled = false;
        }
        }
        #[cfg(feature = "goggles2-camera")]
        {
            let mut camera_b = 0;
            if let Some((b, off, _is_m43)) = goggles::FerretResource::found_camera() {
                ui.text(format!("cam: {:p}@{off:#x}", b as *mut ()));
                camera_b = b;
            }
            if let Some((b, off)) = goggles::FerretResource::found_perspective() {
                if b != camera_b {
                    if camera_b != 0 {
                        ui.same_line();
                    }
                    ui.text(format!("persp{}@{off:#x}", lazyfmt::or_empty((b != camera_b).then_some(format_args!(": {:p}", b as *mut ())))));
                    #[cfg(taimi_debug)]
                    if !REPORT_HAS_SMOOTH.get() {
                        ui.same_line();
                        ui.text("smoothfail");
                    }
                    #[cfg(taimi_debug)]
                    if ui.button("eye acc") {
                        REPORT_EYE_ACC.set(true);
                    }
                }
            }
        }

        #[cfg(feature = "goggles2-project")]
        {
            use taimi_meta::ui::LocalContext;
            let mut prefix = "drawfail";
            for ctx in machine.goggles.project.undrawn() {
                if !prefix.is_empty() {
                    ui.text(prefix);
                }
                prefix = "";
                ui.same_line();
                let actname;
                let msg = match ctx {
                    Ok(LocalContext::World) => "space",
                    Ok(LocalContext::MINIMAP) => "mini",
                    Ok(LocalContext::GLOBAL) => "worldmap",
                    Err(act) => {
                        actname = format!("{act:?}");
                        &actname[..]
                    },
                };
                ui.text(msg);
            }
        }
    }

    pub fn draw_debug_lens2<'ui, U>(&mut self, ui: &mut U, machine: &mut RenderMachine)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let list = imgui::ListBox::new("lenses");
        let mut selected_lens = None;
        let mut new_selection = None;
        let mut selected_info = None;
        if let Some(_list) = list.begin(ui) {
            selected_lens = self.view_lens;
            for (key, info) in ClassShared::iter_ui() {
                let ty = info.kind.tag();
                let is_selected = selected_lens == Some(key);
                if is_selected && selected_info.is_none() {
                    selected_info = Some(info.clone());
                }
                let winner = lazyfmt::or_empty(info.winner.then_some(" ++"));
                let ptr = key.as_ptr() as usize;
                let name = format!("{ty}={ptr:#08x}");
                let label = format!("{:?}({name}){winner}###{ptr:#08x}", info.classification);
                let mut selected = imgui::Selectable::new(label)
                    .selected(is_selected)
                    .build(ui);
                if ui.is_item_hovered() {
                    if ui.is_mouse_down(imgui::MouseButton::Left) {
                        selected = true;
                    }
                }
                if selected {
                    new_selection = Some(key);
                    selected_info = Some(info.clone());
                }
            }
        }

        selected_lens = new_selection.or(selected_lens);
        if let Some(lens) = new_selection {
            self.view_lens = Some(lens);
            self.view_lens_info.clear();
        } else if ui.is_item_clicked_with_button(MouseButton::Right) {
            self.view_lens = None;
            self.view_lens_info = String::new();
            selected_lens = None;
        }

        let mut new_class = None;
        if let Some(info) = &selected_info {
            let preview: &str = info.classification.into();
            if let Some(combo) = ui.begin_combo("reclassify", preview) {
                for &cls in goggles::class::BufferClass::VARIANTS {
                    let name: &str = cls.into();
                    let selected = imgui::Selectable::new(name)
                        .selected(info.classification == cls)
                        .build(ui);
                    if selected {
                        new_class = Some(Some(cls));
                    }
                }
                combo.end();
            }
        }
        if new_class.is_none() && ui.is_item_clicked_with_button(MouseButton::Right) {
            new_class = Some(None);
        }

        #[cfg(taimi_debug)]
        if self.view_lens_info.is_empty() {
            use taimi_d3d::dx11::{DepthView, RenderTargetView, View};
            let mut rview = None;
            let mut dview = None;
            let mut uview = None;
            let view = if let (Some(lens), Some(info)) = (&selected_lens, &selected_info) {
                let view: Option<&View> = match (&lens, info.kind) {
                    _ if !info.was_seen() || ClassShared::read_frame_count().wrapping_sub(info.last_seen) >= 1 || machine.mumblelink_frame_skip > 0 || machine.is_ingame().is_none() =>
                        None,
                    (p, goggles::class::BufferKind::DepthView) => Some({
                        &*dview.insert(unsafe { DepthView::from_d3d_raw_ref(p) })
                    }),
                    (p, goggles::class::BufferKind::RenderTarget) => Some({
                        &*rview.insert(unsafe { RenderTargetView::from_d3d_raw_ref(p) })
                    }),
                    (p, goggles::class::BufferKind::UnorderedAccessView) => Some({
                        &*uview.insert(unsafe { View::from_d3d_raw_ref(p) })
                    }),
                };
                view.map(|v| (v, info))
            } else { None };
            let desc = view.and_then(|(view, ..)|
                view.get_resource().ok().and_then(|r| r.as_texture2().map(|t2| t2.desc())),
            );
            if let Some(desc) = desc {
                use core::fmt::Write;
                let out = &mut self.view_lens_info;
                let _ = write!(out, "size={}x{} mips={}({})", desc.Width, desc.Height, desc.MipLevels, desc.SampleDesc.Count);
                let _ = write!(out, "format={:#x} usage={:#x} bind={:#x} misc={:#x}", desc.Format.0, desc.Usage.0, desc.BindFlags, desc.MiscFlags);
            }
        }
        if let (Some(lens), Some(info)) = (selected_lens, &selected_info) {
            ui.text(format!("view {:p} resource {:p}", lens.as_ptr(), arcffi::nn::nonnull_ptr(info.resource)));
            if let Some(assoc) = info.associated {
                ui.same_line();
                ui.text(format!("assoc {assoc:p}"));
                goggles::class::ClassShared::with_seen2(assoc, |abuf| {
                    ui.same_line();
                    let cls: &str = abuf.classification.into();
                    ui.text(format!("({cls})"));
                });
                match info.state.associated {
                    Some(assoc2) if assoc2 == assoc => {
                        ui.same_line();
                        ui.text("(weak)");
                    },
                    Some(assocmismatch) => {
                        ui.text(format!("assoc the 2nd: {assocmismatch:p}"));
                        goggles::class::ClassShared::with_seen2(assocmismatch, |abuf| {
                            ui.same_line();
                            let cls: &str = abuf.classification.into();
                            ui.text(format!("({cls})"));
                        });
                    },
                    None => (),
                }
            }
            #[cfg(taimi_debug)]
            {
                ui.text(format!("binds={} dbinds={} ubinds={} state={:#06x} {:?}", info.state.bind_count, info.state.depth_binds.len(), info.state.bind_count_uavs, info.state.flags, info.state.flags));
                let last_bind_generation = ClassShared::read_bind_generation();
                ui.text(format!("final bind@{}/{last_bind_generation} depthstate@{}", info.state.bind_generation, info.state.depth_generation));
                if info.state.is_bound(last_bind_generation) {
                    ui.same_line();
                    ui.text("REMAINS BOUND!");
                }
            }
            let now = ClassShared::read_frame_count();
            let lifetime = info.age();
            let seen = info.seen_since(now);
            #[cfg(taimi_debug)]
            {
                ui.text(format!("seen={seen} age={lifetime} format={:#x}", info.format.0));
                if let Some((w, h)) = info.size() {
                    ui.same_line();
                    ui.text(format!("size={}x{}", w, h));
                }
                if info.state.flags.contains(goggles::class::BufferStateFlags::CLEARED_COLOUR) {
                    ui.text(format!("clear({}): {:?}", goggles::class::BufferKind::RenderTarget.tag(), info.state.clear_colour));
                }
            }
            #[cfg(todo)]
            if info.state.flags.contains(goggles::class::BufferStateFlags::CLEARED_DEPTH) {
                ui.text(format!("clear({}): {:?}", goggles::class::BufferKind::DepthView.tag(), info.state.clears_depth));
            }

            #[cfg(taimi_debug)]
            {
                let mut scores = info.classify_scores().collect::<Vec<_>>();
                scores.sort_by_key(|(_, score)| core::cmp::Reverse(*score));
                for (i, (cls, score)) in scores.iter().enumerate() {
                    let clsname: &str = cls.into();
                    if i > 0 {
                        ui.reserve_line_checkbox(&clsname);
                    }
                    ui.text(format!("{clsname}={score}"));
                }
                for (i, (depth, sref)) in info.state.depth_binds.iter().enumerate() {
                    ui.text(format!(":: dep#{i} sref={sref} depth={depth:?}"));
                }
            }
        }
        if !self.view_lens_info.is_empty() {
            ui.text(&self.view_lens_info);
        }

        if let (Some(key), Some(cls)) = (selected_lens, new_class) {
            goggles::class::ClassShared::manually_classify(key, cls);
        }
    }
}
unsafe impl Sync for GogglesConfig {}
unsafe impl Send for GogglesConfig {}

#[cfg(todo = "unused")]
pub fn options_ui(ui: &imgui::Ui) {
    let (mut enabled, needs_setup) = get_state();

    if ui.checkbox("Goggles", &mut enabled) {
        match enabled {
            true => {
                enable(needs_setup);
            },
            false => {
                disable();
            },
        }
    }

    options_ui_lenses(ui);
}

#[cfg(taimi_debug)]
thread_local! {
    static REPORT_EYE_ACC: std::cell::Cell<bool> = std::cell::Cell::new(false);
    static REPORT_HAS_SMOOTH: std::cell::Cell<bool> = std::cell::Cell::new(false);
}
pub fn blah_pre_render(machine: &mut super::machine::RenderMachine) {
    #[cfg(taimi_debug)]
    {
        if REPORT_EYE_ACC.get() {
            report_eye_acc(&*machine);
            REPORT_EYE_ACC.set(false);
        }
        REPORT_HAS_SMOOTH.set(!goggles::FerretResource::wants_snatch_camera_smooth());
    }
}
#[cfg(taimi_debug)]
fn report_eye_acc(machine: &super::machine::RenderMachine) {
    use taimi_meta::coords::{LocalSpace, GameSpace};
    let persp = goggles::FerretResource::snatch_perspective();
    let smooth = goggles::FerretResource::snatch_camera_smooth();
    let (eye_persp_raw, dir_persp_raw, ..) = smooth.get_as_look_raw();
    let eye_persp_raw = glam::Vec3::from(eye_persp_raw);
    let eye_persp = GameSpace::to_local(eye_persp_raw.into()).to_raw();
    let dir_persp_raw = glam::Vec3::from(dir_persp_raw);
    let dir_persp = GameSpace::norm_to_local(dir_persp_raw.into()).to_raw();

    let (eye_ml, dir_ml, ..) = machine.mumblelink_camera;
    let eye_ml = eye_ml.to_raw();
    let eye_ml_game = LocalSpace::to_game(eye_ml.into()).to_raw();
    let dir_ml = dir_ml.to_raw();
    let dir_ml_game = LocalSpace::norm_to_game(dir_ml.into()).to_raw();

    let (eye_m2, dir_m2, ..) = rt::mumble_link_ptr().map(|ml| super::machine::RenderMachine::read_camera_mumblelink(ml)).unwrap_or(super::machine::RenderMachine::POSITION_EMPTY);
    let eye_m2 = eye_m2.to_raw();
    let eye_m2_game = LocalSpace::to_game(eye_m2.into()).to_raw();
    let dir_m2 = dir_m2.to_raw();
    let dir_m2_game = LocalSpace::norm_to_game(dir_m2.into()).to_raw();
    let eye_m2_frameskip = machine.mumblelink_frame_skip;

    let (eye_rt, dir_rt, ..) = machine.rtapi_state.camera;
    let eye_rt = eye_rt.to_raw();
    let eye_rt_game = LocalSpace::to_game(eye_rt.into()).to_raw();
    let dir_rt = dir_rt.to_raw();
    let dir_rt_game = LocalSpace::norm_to_game(dir_rt.into()).to_raw();
    let cam = goggles::FerretResource::snatch_camera();

    let (eye_cam_raw, dir_cam_raw, ..) = cam.get_as_look_raw();
    let eye_cam_raw = glam::Vec3::from(eye_cam_raw);
    let eye_cam = GameSpace::to_local(eye_cam_raw.into()).to_raw();
    let dir_cam_raw = glam::Vec3::from(dir_cam_raw);
    let dir_cam = GameSpace::norm_to_local(dir_cam_raw.into()).to_raw();

    let cmp = |eye_game: glam::Vec3| {
        let eye_ref = match cam.is_empty() {
            true => eye_persp_raw,
            false => eye_cam_raw,
        };
        let delta = eye_game - eye_ref;
        format!("dist={:?} delta={delta:?} mult={:?}", delta.length(), eye_game / eye_ref)
    };
    log::debug!("ACC EYE\nEYE(ML): {} loc={eye_ml:?} game={eye_ml_game:?}\nEYE(M2): {} loc={eye_m2:?} game={eye_m2_game:?} skip={eye_m2_frameskip}\nEYE(RT): {} loc={eye_rt:?} game={eye_rt_game:?}\nEYE(SP): {} loc={eye_persp:?} read={eye_persp_raw:?}\nEYE(SC)={eye_cam:?} read={eye_cam_raw:?}", cmp(eye_ml_game), cmp(eye_m2_game), cmp(eye_rt_game), cmp(eye_persp_raw));

    let cmpdir = |dir_game: glam::Vec3| {
        let dir_ref = match cam.is_empty() {
            true => dir_persp_raw,
            false => dir_cam_raw,
        };
        let delta = dir_game - dir_ref;
        format!("dist={:?} delta={delta:?} mult={:?}", (dir_game.normalize() - dir_ref.normalize()).length(), dir_game / dir_ref)
    };
    log::debug!("ACC DIR\nDIR(ML): {} loc={dir_ml:?} game={dir_ml_game:?}\nDIR(M2): {} loc={dir_m2:?} game={dir_m2_game:?} skip={eye_m2_frameskip}\nDIR(RT): {} loc={dir_rt:?} game={dir_rt_game:?}\nDIR(SP): {} loc={dir_persp:?} read={dir_persp_raw:?}\nDIR(SC)={dir_cam:?} read={dir_cam_raw:?}", cmpdir(dir_ml_game), cmpdir(dir_m2_game), cmpdir(dir_rt_game), cmpdir(dir_persp_raw));

    let h_persp = persp.get_as_perspective().0;
    let fov_persp = h_persp.recip().atan() * 2.0;
    let fov_rt = machine.rtapi_state.camera_fov_y;
    let fov_ml = machine.fov_y().map(|v| v.to_radians()).unwrap_or(0.0f32);
    let cmpfov = |fov: f32| {
        let fov_ref = fov_persp;
        let delta = fov - fov_ref;
        format!("delta={delta:?} mult={:?}", fov / fov_ref)
    };
    log::debug!("ACC FOV\nFOV(ML)={fov_ml:?} {}\nFOV(RT)={fov_rt:?} {}\nFOV(SP)={fov_persp:?}", cmpfov(fov_ml), cmpfov(fov_rt));
    if (fov_persp - fov_ml).abs() > 0.2 {
        let (fov, range) = persp.get_as_perspective();
        let aspect = persp.perspective_aspect_ratio();
        log::debug!("ACC PERSPDEBUG fov={fov} aspect={aspect} depth={range:?}\n{:?}", persp.data);
    }
    let (eye_ml_prev, ..) = machine.mumblelink_camera_prev;
    let eye_ml_prev = eye_ml_prev.to_raw();
    let eye_ml_prev_game = LocalSpace::to_game(eye_ml_prev.into()).to_raw();
    log::debug!("EYE(PREV): {} prev={eye_ml_prev:?} game={eye_ml_prev_game} prevframe={} camframe={} frame={}", cmp(eye_ml_prev_game), machine.mumblelink_camera_prev_frame, machine.mumblelink_camera_frame, machine.mumblelink_frame);
}
