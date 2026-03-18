use {
    crate::{
        exports::runtime as rt,
        render::element::prelude::*,
        space::goggles::{self, lens::{LensClass, LENSES, LENS_PTR}},
    },
    anyhow::Context,
    std::{mem, ptr, sync::atomic::Ordering, thread},
    windows::{core::Interface, Win32::Graphics::Direct3D11::ID3D11DeviceContext_Vtbl},
};

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

pub fn options_ui_lenses<'ui, U>(ui: &mut U, machine: &mut super::machine::RenderMachine)
where
    U: ?Sized + ImDrawWindow<'ui>,
{
    if let Ok(lenses) = LENSES.read() {
        let selected_lens = LENS_PTR.load(Ordering::Relaxed);
        let preview = match selected_lens {
            l if l.is_null() => "Default".into(),
            key => match lenses.get(&(key as usize)) {
                Some(clss) => format!("{clss:?} ({key:?})"),
                None => format!("{key:?}"),
            },
        };
        let mut new_lens = None;
        if let Some(combo) = ui.begin_combo("Lens", preview) {
            for (&key, &clss) in lenses.iter() {
                if matches!(clss, LensClass::Unknown) {
                    continue
                }
                let selected = imgui::Selectable::new(format!("{clss:?} ({key:08x})"))
                    .selected(selected_lens as usize == key)
                    .build(ui);
                if selected {
                    new_lens = Some((key, clss));
                }
            }
            combo.end();
        }
        match new_lens {
            None if ui.is_item_clicked_with_button(imgui::MouseButton::Right) => {
                LENS_PTR.store(ptr::null_mut(), Ordering::Relaxed);
            },
            None => (),
            Some((_, LensClass::Space)) => {
                LENS_PTR.store(ptr::null_mut(), Ordering::Relaxed);
            },
            Some((key, _)) => {
                LENS_PTR.store(key as *mut _, Ordering::Relaxed);
            },
        }

        let selected_proj = goggles::FerretResource::project_target_buffer();
        let mut new_proj = None;
        if let Some(selected_proj) = selected_proj {
            let preview = match selected_proj {
                None => "Default".into(),
                Some(key) =>
                    format!("{key:p}"),
            };
            if let Some(_combo) = ui.begin_combo("Projection", preview) {
                let selected_off = imgui::Selectable::new("Off")
                    .selected(selected_proj == None)
                    .build(ui);
                if selected_off {
                    new_proj = Some((None, None));
                }
                for (&key, &clss) in lenses.iter() {
                    if !matches!(clss, LensClass::World | LensClass::Test | LensClass::UI | LensClass::Dummy) {
                        continue
                    }
                    let selected = imgui::Selectable::new(format!("LENS={clss:?} ({key:08x})"))
                        .selected(selected_proj.map(|p| p.as_ptr() as usize) == Some(key))
                        .build(ui);
                    if selected {
                        new_proj = Some((core::ptr::NonNull::new(key as *mut _), None));
                    }
                }
                for (key, info) in goggles::FerretResource::project_iter_ui(goggles::project::ProjectClassification::DEFAULT_TARGET) {
                    let ty = match info.kind {
                        goggles::project::ProjectBufferKind::DepthView => "DV",
                        goggles::project::ProjectBufferKind::RenderTarget => "RT",
                    };
                    let mut selected = imgui::Selectable::new(format!("{ty}={:?} ({:#08x})", info.classification, key.as_ptr() as usize))
                        .selected(selected_proj == Some(key))
                        .build(ui);
                    if ui.is_item_hovered() {
                        if ui.is_mouse_down(imgui::MouseButton::Left) {
                            selected = true;
                        }
                        let now = goggles::g2!(*&ferret.project.frame_count);
                        let lifetime = info.age();
                        let seen = info.seen_since(now);
                        ui.tooltip_text(format!("seen={seen} age={lifetime}"));
                    }
                    if selected {
                        new_proj = Some((Some(key), Some(info.classification)));
                    }
                }
            }
            if ui.is_item_clicked_with_button(imgui::MouseButton::Right) {
                new_proj = Some((None, Some(goggles::project::ProjectClassification::DEFAULT_TARGET)));
            }
            // TODO: use this for target and enable, then no need to unwrap
            let mut request = goggles::FerretResource::project_target_request().unwrap();
            let selected_mode = request.cond;
            let preview: &str = selected_mode.into();
            let mut new_mode = None;
            let mut request_dirty = false;
            if let Some(combo) = ui.begin_combo("pmode", preview) {
                for &mode in <goggles::project::ProjectCondition as strum::VariantArray>::VARIANTS {
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
            if ui.is_item_clicked_with_button(imgui::MouseButton::Right) {
                new_mode = Some(goggles::project::ProjectCondition::DEFAULT_TARGET);
            }
            if let Some(new_mode) = new_mode {
                request.cond = new_mode;
                request_dirty = true;
            }

            if ui.checkbox("patient", &mut request.patient) {
                request.manual_delay = false;
                request_dirty = true;
            }
            ui.same_line();
            if ui.checkbox("empty", &mut request.empty) {
                request_dirty = true;
            }

            if request_dirty {
                goggles::FerretResource::project_set_target_request(Some(request));
            }
        } else {
            let mut proj = false;
            if ui.checkbox("Projection", &mut proj) {
                debug_assert!(proj);
                new_proj = Some((None, Some(goggles::project::ProjectClassification::DEFAULT_TARGET)));
            }
        }
        match new_proj {
            None => (),
            Some((None, None)) => {
                goggles::FerretResource::project_set_target_request(None);
            },
            Some((key, classification)) => {
                goggles::FerretResource::project_set_target(key, classification);
            },
        }

        let selected_shadowbox = goggles::FerretResource::project_shadowbox_buffer();
        let mut new_shadowbox = None;
        if let Some(selected_shadowbox) = selected_shadowbox {
            let preview = match selected_shadowbox {
                None => "Default".into(),
                Some(key) =>
                    format!("{key:p}"),
            };
            if let Some(_combo) = ui.begin_combo("Shadowboxing", preview) {
                let selected_off = imgui::Selectable::new("Off")
                    .selected(selected_shadowbox == None)
                    .build(ui);
                if selected_off {
                    new_shadowbox = Some((None, None));
                }
                for (key, info) in goggles::FerretResource::project_iter_ui(goggles::project::ProjectClassification::DEFAULT_SHADOWBOX) {
                    let selected = imgui::Selectable::new(format!("RT={:?} ({:#08x})", info.classification, key.as_ptr() as usize))
                        .selected(selected_shadowbox == Some(key))
                        .build(ui);
                    if selected {
                        new_shadowbox = Some((Some(key), Some(info.classification)));
                    }
                }
            }
            if ui.is_item_clicked_with_button(imgui::MouseButton::Right) {
                new_shadowbox = Some((None, Some(goggles::project::ProjectClassification::DEFAULT_SHADOWBOX)));
            }
        } else {
            let mut shadowbox = false;
            if ui.checkbox("Shadowboxing", &mut shadowbox) {
                debug_assert!(shadowbox);
                new_shadowbox = Some((None, Some(goggles::project::ProjectClassification::DEFAULT_SHADOWBOX)));
            }
        }
        match new_shadowbox {
            None => (),
            Some((None, None)) => {
                goggles::FerretResource::project_set_shadowbox_request(None);
            },
            Some((key, classification)) => {
                goggles::FerretResource::project_set_shadowbox(key, classification);
            },
        }

        if ui.checkbox("dfill", &mut machine.goggles.project_depth_fill) {
        }
        ui.same_line();
        if ui.checkbox("dvp", &mut machine.goggles.project_viewport_force) {
        }
        if let Some(target) = unsafe { &mut *goggles::g2!(&raw mut ferret.project.target) } {
            ui.same_line();
            let mut debug = matches!(target.action, goggles::project::ProjectAction::DebugDetect);
            if ui.checkbox("debug", &mut debug) {
                match debug {
                    true => target.action = goggles::project::ProjectAction::DebugDetect,
                    false => target.action = goggles::project::ProjectAction::Draw,
                }
            } else if ui.is_item_hovered() {
                use taimi_d3d::dx11::{DepthView, RenderTargetView, View};

                let selected_proj = selected_proj.flatten();
                let selected_buf = goggles::FerretResource::project_iter_ui(goggles::project::ProjectClassification::DEFAULT_TARGET).find(|(k, _)| Some(*k) == selected_proj)
                    .map(|(_, b)| b);
                let view = selected_proj.as_ref().and_then(|p| selected_buf.as_ref().map(move |buf| (p, buf.kind)));
                let mut rview = None;
                let mut dview = None;
                let view: Option<&View> = match view {
                    _ if selected_buf.as_ref().map(|b| b.last_seen) != Some(goggles::g2!(*&ferret.project.frame_count).wrapping_sub(1)) =>
                        None,
                    Some((p, goggles::project::ProjectBufferKind::DepthView)) => Some({
                        &*dview.insert(unsafe { DepthView::from_d3d_raw_ref(p) })
                    }),
                    Some((p, goggles::project::ProjectBufferKind::RenderTarget)) => Some({
                        &*rview.insert(unsafe { RenderTargetView::from_d3d_raw_ref(p) })
                    }),
                    None => None,
                };
                if let (Some(view), Some(info)) = (view, selected_buf) {
                    ui.tooltip(|| {
                        ui.text(format!("view {:p}", view.as_d3d_raw().as_ptr()));
                        if let Some(buf_desc) = goggles::lens::get_view_dims(view) {
                            ui.text(format!("size=({},{}) mips={}({})", buf_desc.Width, buf_desc.Height, buf_desc.MipLevels, buf_desc.SampleDesc.Count));
                            ui.text(format!("format={:#x} usage={:#x} bind={:#x} misc={:#x}", buf_desc.Format.0, buf_desc.Usage.0, buf_desc.BindFlags, buf_desc.MiscFlags));
                        }
                        if machine.goggles.perspective_params.0 != 0.0 {
                            let (_h, aspect, near, far) = machine.goggles.perspective_params;
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
                        }
                    });
                }
            }
        }

        ui.same_line();
        ui.checkbox("uninherit", &mut machine.goggles.inherit_render);

        ui.same_line();
        if ui.checkbox("blen", &mut machine.goggles.project_blend_force) {
        }
        ui.same_line();
        if ui.checkbox("flush", &mut machine.goggles.project_flush) {
        }

        ui.same_line();
        if ui.checkbox("shad", &mut machine.goggles.project_shadow) {
        }

        if let Some(mut request) = goggles::FerretResource::project_target_request() {
            let mut delay = *request.delay.start();
            if imgui::Slider::new("delay", 0u32, 64u32).build(ui, &mut delay) {
                request.delay = delay..=delay;
                request.manual_delay = true;
                goggles::FerretResource::project_set_target_request(Some(request));
            }
        }

        let mut cam = machine.goggles.camera_enabled;
        if ui.checkbox("camera", &mut cam) {
            match cam {
                true => machine.goggles.camera_enable(),
                false => machine.goggles.camera_disable(),
            }
        }
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
        let mut camera_b = 0;
        if let Some((b, off, _is_m43)) = goggles::FerretResource::found_camera() {
            ui.text(format!("cam: {:p}@{off:#x}", b as *mut ()));
            camera_b = b;
        }
        if let Some((b, off)) = goggles::FerretResource::found_perspective() {
            use taimi_hoard::lazyfmt;
            if b != camera_b {
                if camera_b != 0 {
                    ui.same_line();
                }
                ui.text(format!("persp{}@{off:#x}", lazyfmt::or_empty((b != camera_b).then_some(format_args!(": {:p}", b as *mut ())))));
            }
        }
    }

    ui.same_line();
    if let Some(report) = goggles::FerretResource::project_report_target() {
        let count = report.count;
        ui.text(format!("draws={count}"));
        if !report.acted {
            ui.same_line();
            ui.text("drawfail")
        }
    }
}

pub fn get_state() -> (bool, bool) {
    match goggles::GOGGLES.get() {
        Some(orig) => (orig.set_targets.is_enabled(), false),
        None => (false, true),
    }
}

pub fn enable(needs_setup: bool) {
    let vtbl = if needs_setup {
        let ctx = rt::d3d11_device()
            .and_then(|(dev, _)| dev.get_immediate_context())
            .context("goggles requires device context");

        let ctx = match ctx {
            Ok(d) => d,
            Err(e) => {
                log::error!("{e:#}");
                return
            },
        };
        let vtbl = ctx.vtable();
        Some(unsafe {
            mem::transmute::<&ID3D11DeviceContext_Vtbl, &'static ID3D11DeviceContext_Vtbl>(vtbl)
        })
    } else {
        None
    };
    // avoid deadlocks...
    thread::spawn(move || {
        if let Some(vtbl) = vtbl {
            let res = goggles::setup(vtbl).context("goggles failure");
            if let Err(e) = res {
                log::error!("{e:#}");
                return
            }
        }

        let res = goggles::enable().context("failed to enable goggles");
        if let Err(e) = res {
            log::error!("{e:#}");
            let _ = goggles::disable();
        } else {
            let _ = LENS_PTR.compare_exchange(
                ptr::null_mut(),
                ptr::dangling_mut(),
                Ordering::Relaxed,
                Ordering::Relaxed,
            );
        }
    });
}

pub fn disable() {
    let res = goggles::disable().context("failed to disable goggles");
    if let Err(e) = res {
        log::error!("{e:#}");
    } else {
        let _ = LENS_PTR.store(ptr::null_mut(), Ordering::Relaxed);
    }
    if let Ok(mut lenses) = LENSES.try_write() {
        lenses.clear();
    }
}
