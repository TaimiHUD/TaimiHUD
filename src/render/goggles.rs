use {
    crate::{
        exports::runtime as rt,
        render::element::prelude::*,
        space::goggles::{self, LensClass, LENSES, LENS_PTR},
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

pub fn options_ui_lenses<'ui, U>(ui: &mut U)
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
        if let Some(combo) = ui.begin_combo("Lens", preview) {
            let mut new_lens = None;
            for (&key, &clss) in lenses.iter() {
                let selected = ui.selectable(
                    format_args!("{clss:?} ({key:08x})"),
                    selected_lens as usize == key,
                );
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
        thread_local! {
            static BUF: std::cell::RefCell<String> = std::cell::RefCell::default();
        }
        BUF.with_borrow_mut(|buf| {
            if ui.input_text("ferret", buf).enter_returns_true(true).build() {
                let f = if let Some(rest) = buf.strip_prefix("0x") {
                    u64::from_str_radix(rest, 16).ok()
                } else {
                    buf.parse().ok()
                };
                if let Some(f) = f {
                    goggles::ferret(f)
                } else {
                    log::warn!("ferret invalid");
                }
            }
        });
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
