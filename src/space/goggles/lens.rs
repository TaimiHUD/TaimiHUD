use {
    crate::render::{RenderEvent, RenderState},
    core::ptr::{self, NonNull},
    retour::GenericDetour,
    std::{
        cell::Cell,
        collections::BTreeMap,
        sync::{
            atomic::{AtomicPtr, Ordering},
            RwLock,
        },
    },
    windows::{
        core::{IUnknown, Interface, InterfaceRef},
        Win32::Graphics::Direct3D11::{
            ID3D11DepthStencilView,
            D3D11_COMPARISON_LESS,
            D3D11_COMPARISON_LESS_EQUAL,
            D3D11_DEPTH_WRITE_MASK_ZERO,
        },
    },
    taimi_d3d::dx11::{prelude::*, DepthView, DepthState, buffer::{D3D11_TEXTURE2D_DESC, Resource, View}},
    super::{Release, g2},
};
#[cfg(feature = "space")]
use crate::space::Engine;

pub type Lenses = BTreeMap<usize, LensClass>;

pub(crate) static LENS_PTR: AtomicPtr<ID3D11DepthStencilView> = AtomicPtr::new(ptr::null_mut());
pub(crate) static LENSES: RwLock<Lenses> = RwLock::new(BTreeMap::new());

pub fn read_lens() -> *mut ID3D11DepthStencilView {
    LENS_PTR.load(Ordering::Relaxed)
}

pub fn lens_valid(p: *const ID3D11DepthStencilView) -> bool {
    match LENSES.try_read() {
        Ok(lenses) => lenses.contains_key(&(p as usize)),
        _ => false,
    }
}

pub fn current_lens() -> Option<InterfaceRef<'static, ID3D11DepthStencilView>> {
    match NonNull::new(read_lens()) {
        Some(lens) if lens == NonNull::dangling() => None,
        Some(lens) if lens_valid(lens.as_ptr()) => Some(unsafe { InterfaceRef::from_raw(lens.cast()) }),
        _ => None,
    }
}

pub fn clear_lens() {
    LENS_PTR.store(ptr::dangling_mut(), Ordering::Relaxed);
}

pub fn pick_lens(force: bool) {
    let selected_lens = LENS_PTR.load(Ordering::Relaxed);
    if selected_lens.is_null() {
        return
    }

    if let Ok(lenses) = LENSES.read() {
        if !force && lenses.contains_key(&(selected_lens as usize)) {
            return
        }
        let selected = lenses
            .iter()
            .filter(|(_, cls)| matches!(cls, LensClass::World | LensClass::Test))
            .max_by_key(|(_, cls)| matches!(cls, LensClass::World));
        if let Some((&world_key, _cls)) = selected {
            LENS_PTR.store(world_key as *mut _, Ordering::Relaxed);
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LensClass {
    Unknown,
    Unsupported,
    Sampled,
    //Imgui,
    Space,
    World,
    Test,
    Dummy,
    UI,
    Overlay,
}

thread_local! {
    static DEPTH_VIEW_BOUND_KEY: Cell<usize> = Cell::new(0);
}
pub(crate) fn get_view_dims(view: &View) -> Option<D3D11_TEXTURE2D_DESC> {
    let resource = view.get_resource().ok().map(Resource::from_d3d)?;
    if let Some(t2) = resource.as_texture2() {
        Some(t2.desc())
    } else {
        // TODO: texture3d? how2mips idk
        None
    }
}
pub(super) unsafe fn set_targets(
    _this: &Dx11Context,
    depth_view: Option<&DepthView>,
) {
    let key = depth_view.map(|v| v.as_d3d_raw().as_ptr() as usize).unwrap_or(0);
    DEPTH_VIEW_BOUND_KEY.set(match depth_view {
        None =>
            key,
        Some(view) => {
            let known = LENSES.read().map_err(drop).map(|l| l.get(&key).copied());
            match known {
                Err(()) => {
                    // poisoned???
                    0
                },
                Ok(Some(LensClass::Unknown)) => key,
                Ok(Some(_lens)) => {
                    if let LensClass::Unsupported | LensClass::Sampled = _lens {
                        #[cfg(todo)]
                        {
                        DEPTH_VIEW_BOUND.set(0);
                        }
                    }
                    //log::trace!("recognized as {lens:?}");
                    0
                },
                Ok(None) => {
                    let size_ok = if let Some(dims) = get_view_dims(view) {
                        let format_ok = match dims.Format {
                            _ => true,
                        };
                        if !format_ok {
                            log::warn!("DEBUG DELETEME(dv): fmt actually caught something? {}", dims.Format.0);
                            Err(LensClass::Unsupported)
                        } else {
                            let expected = g2!(*&ferret.display_size);
                            if dims.Width == expected.x as u32 && dims.Height == expected.y as u32 {
                                Ok(())
                            } else {
                                let expected_aspect = expected.x / expected.y;
                                let aspect = dims.Width as f32 / dims.Height as f32;
                                match (expected_aspect - aspect).abs() < 2e-4f32 {
                                    true => Err(LensClass::Sampled),
                                    false => Err(LensClass::Unsupported),
                                }
                            }
                        }
                    } else {
                        Err(LensClass::Unsupported)
                    };
                    let (cls, k) = match size_ok {
                        Ok(()) => (LensClass::Unknown, key),
                        Err(c) => (c, 0),
                    };
                    if let Ok(mut lenses) = LENSES.write() {
                        lenses.insert(key, cls);
                    }
                    k
                },
            }
        },
    });
}
pub(super) unsafe fn set_depth_state(
    context: &Dx11Context,
    state: Option<&DepthState>,
    stencil_ref: u32,
) {
    let Some(state) = state else {
        return
    };
    let key = DEPTH_VIEW_BOUND_KEY.get();
    if key == 0 { return }
    //log::debug!("unknown buffer, attempting classification...");
    let cls = {
        #[cfg(todo)]
        let desc_view = unsafe {
            let mut desc_view = Default::default();
            view.GetDesc(&mut desc_view);
            desc_view
        };
        // TODO: desc can't change after state created, so cache ptr in bloom filter maybe?
        let viewport_ok = || {
            let [viewport] = taimi_d3d::dx11::Viewport::new_snapshot(context);
            let expected = g2!(*&ferret.display_size);
            if viewport.viewport.TopLeftX.to_bits() != 0.0f32.to_bits() || viewport.viewport.TopLeftY.to_bits() != 0.0f32.to_bits() { return false }
            let expected_aspect = expected.x / expected.y;
            let viewport_aspect = viewport.viewport.Width / viewport.viewport.Height;
            (expected_aspect - viewport_aspect).abs() < 2e-4f32
        };
        let desc_state = state.get_desc();
        match desc_state.DepthEnable.0 != 0 {
            _ if key == 0 => {
                None
            },
            _ if !viewport_ok() => {
                None
            },
            false => {
                if desc_state.DepthWriteMask != D3D11_DEPTH_WRITE_MASK_ZERO {
                    log::trace!("skipping for now (write-only bind)");
                }
                None
            },
            #[cfg(todo)]
            false if desc_state.DepthWriteMask != D3D11_DEPTH_WRITE_MASK_ZERO =>
                Some(LensClass::UI),
            true if desc_state.DepthWriteMask == D3D11_DEPTH_WRITE_MASK_ZERO => {
                //log::trace!("skipping for now (read-only bind)");
                None
            },
            true if desc_state.DepthFunc == D3D11_COMPARISON_LESS =>
                Some(match stencil_ref {
                    0 => LensClass::World,
                    #[cfg(todo = "unnecessary")]
                    0 if viewport_ok() =>
                        LensClass::World,
                    #[cfg(todo = "unnecessary")]
                    0 => LensClass::Unsupported,
                    _ => LensClass::Dummy,
                }),
            true if desc_state.DepthFunc == D3D11_COMPARISON_LESS_EQUAL =>
                Some(LensClass::Test),
            true => Some(LensClass::Unknown),
            #[cfg(todo)]
            false => Some(LensClass::Overlay),
        }
    };
    if let Some(cls) = cls {
        if let Ok(mut lenses) = LENSES.write() {
            lenses.insert(key, cls);
            if cls == LensClass::World {
                let selected_lens = LENS_PTR.load(Ordering::Relaxed);
                if !selected_lens.is_null() && !lenses.contains_key(&(selected_lens as usize)) {
                    LENS_PTR.store(key as *mut _, Ordering::Relaxed);
                }
            }
        }
        if cls == LensClass::World {
            RenderState::try_send(RenderEvent::UiDepthAcquired());
        }
    }
}

pub(super) fn release_depth_view_pre(this: InterfaceRef<'static, IUnknown>, release: &GenericDetour<Release>) -> Option<usize> {
    let key = match this.cast::<ID3D11DepthStencilView>().ok() {
        None => None,
        Some(view) => {
            let key = view.as_raw() as usize;
            let view_ref = IUnknown::from(view).into_raw();
            let _refcount = unsafe {
                release.call(InterfaceRef::from_raw(NonNull::new_unchecked(view_ref)))
            };

            Some(key)
        },
    };
    key
}
pub(super) fn release_depth_view(refcount: u32, key: usize) {
    if refcount > 0 { return }
    let removed = if let Ok(mut lenses) = LENSES.write() {
        let removed = lenses.remove(&key);
        let selected = LENS_PTR.compare_exchange(
            key as *mut _,
            ptr::dangling_mut(),
            Ordering::Relaxed,
            Ordering::Relaxed,
        );
        if matches!(removed, Some(LensClass::World)) || selected.is_ok() {
            RenderState::try_send(RenderEvent::UiDepthReleased());
        }
        removed.is_some()
    } else {
        false
    };
    if removed {
        //log::trace!("released depth view {key:08x}");
    }
}

pub(super) fn shutdown() {
    LENS_PTR.store(ptr::null_mut(), Ordering::SeqCst);
}

/*pub fn needs_classification(cls: LensClass) -> bool {
    match cls {
        LensClass::Space if Engine::is_available() =>
            false,
        LensClass::Imgui =>
            false,
        _ => true,
    }
}*/

#[cfg(todo = "unused")]
pub fn has_classification(cls: LensClass) -> Option<bool> {
    LENSES
        .try_read()
        .ok()
        .map(|lenses| lenses.values().any(|&c| c == cls))
}

pub fn classify_lens(dsview: *mut ID3D11DepthStencilView, cls: LensClass) {
    if let Ok(mut lenses) = LENSES.write() {
        lenses.insert(dsview as usize, cls);
    }
}

/*
pub fn classify_current_lens(cls: LensClass) {
    let dsview = rt::d3d11_device().ok().flatten()
        .and_then(|d3d11| unsafe { d3d11.GetImmediateContext().ok() })
        .and_then(|ctx| unsafe {
            let mut dsview = None;
            ctx.OMGetRenderTargets(None, Some(&mut dsview));
            dsview.map(|dsview| dsview.as_raw())
        });
    if let Some(dsview) = dsview {
        classify_lens(dsview as *mut _, cls)
    }
}*/

#[cfg(feature = "space")]
pub fn classify_space_lens(engine: &Engine) {
    if let Some(view) = &engine.render_backend.depth_handler.render_target_view.depth {
        let dsview = view.as_d3d().as_raw();
        classify_lens(dsview as *mut _, LensClass::Space);
    }
}
