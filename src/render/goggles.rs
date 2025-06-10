use anyhow::{anyhow, Context};
use crate::{
    exports::runtime as rt,
    space::goggles::{self, LensClass, LENSES, LENS_PTR},
};
use nexus::imgui;
use std::{ptr, sync::atomic::Ordering};
use windows::core::Interface;

pub fn options_ui(ui: &imgui::Ui) {
    let (mut enabled, needs_setup) = match goggles::GOGGLES.get() {
        Some(orig) => (orig.set_targets.is_enabled(), false),
        None => (false, true),
    };

    if ui.checkbox("Goggles", &mut enabled) {
        match enabled {
            true => {
                if needs_setup {
                    let ctx = rt::d3d11_device()
                        .and_then(|dev| dev.ok_or("device not found"))
                        .map_err(|e| anyhow!("d3d11 device unavailable: {e}"))
                        .and_then(|dev| unsafe {
                            dev.GetImmediateContext()
                        }.context("GetImmediateContext"));

                    let ctx = match ctx {
                        Ok(d) => d,
                        Err(e) => {
                            log::error!("goggles requires device context, but: {e}");
                            return
                        },
                    };
                    if let Err(e) = goggles::setup(ctx.to_ref()) {
                        log::error!("goggles failure: {e}");
                        return
                    }
                }

                if let Err(e) = goggles::enable() {
                    log::error!("failed to enable goggles: {e}");
                    let _ = goggles::disable();
                } else {
                    let _ = LENS_PTR.compare_exchange(ptr::null_mut(), ptr::dangling_mut(), Ordering::Relaxed, Ordering::Relaxed);
                }
            },
            false => {
                if let Err(e) = goggles::disable() {
                    log::error!("failed to disable goggles: {e}");
                } else {
                    let _ = LENS_PTR.store(ptr::null_mut(), Ordering::Relaxed);
                }
            },
        }
    }

    let mut depth_value = crate::space::max_depth();
    if imgui::Slider::new("depth", 25.0f32, 15000.0).build(ui, &mut depth_value) {
        crate::space::set_max_depth(depth_value);
    }
    let mut near_value = crate::space::min_depth();
    if imgui::Slider::new("near", 0.001f32, 10.0).build(ui, &mut near_value) {
        crate::space::set_min_depth(near_value);
    }

    if let Ok(lenses) = LENSES.read() {
        let selected_lens = LENS_PTR.load(Ordering::Relaxed);
        let preview = match selected_lens {
            l if l.is_null() => "Default".into(),
            key => match lenses.get(&(key as usize)) {
                Some(clss) => format!("{clss:?} ({key:?})"),
                None => format!("{key:?}"),
            },
        };
        if let Some(combo) = ui.begin_combo("Lens", preview) {
            let mut new_lens = None;
            for (&key, &clss) in lenses.iter() {
                let selected = imgui::Selectable::new(format!("{clss:?} ({key:08x})"))
                    .selected(selected_lens as usize == key)
                    .build(ui);
                if selected {
                    new_lens = Some((key, clss));
                }
            }
            combo.end();

            match new_lens {
                None => (),
                Some((_, LensClass::Space)) => {
                    LENS_PTR.store(ptr::null_mut(), Ordering::Relaxed);
                },
                Some((key, _)) => {
                    LENS_PTR.store(key as *mut _, Ordering::Relaxed);
                },
            }
        }
    }
}
