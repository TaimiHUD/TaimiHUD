use anyhow::anyhow;
use windows::{
    core::{Interface, InterfaceRef, IUnknown},
    Win32::Graphics::Direct3D11::{ID3D11DepthStencilState, ID3D11DepthStencilView, ID3D11DeviceContext, ID3D11RenderTargetView, D3D11_COMPARISON_LESS, D3D11_COMPARISON_LESS_EQUAL, D3D11_DEPTH_WRITE_MASK_ZERO, D3D11_VIEWPORT},
};
use core::{ffi::c_void, mem::transmute, ptr::{self, NonNull}, slice::from_raw_parts};
use std::{collections::BTreeMap, sync::{OnceLock, RwLock, atomic::{AtomicPtr, Ordering}}};
use retour::GenericDetour;
#[cfg(feature = "space")]
use crate::{space::Engine, ENGINE};

pub type Lenses = BTreeMap<usize, LensClass>;

pub struct Goggles {
    pub set_targets: GenericDetour<SetTargets>,
    pub release_depth_view: Option<GenericDetour<Release>>,
    //pub set_depth_state: SetDepthState,
    //pub clear_depth: ClearDepth,
}

//type SetDepthState = unsafe extern "system" fn(this: InterfaceRef<'static, ID3D11DeviceContext>, buffer: Option<InterfaceRef<'static, ID3D11DepthStencilState>>, u32);
type SetTargets = unsafe extern "system" fn(this: InterfaceRef<'static, ID3D11DeviceContext>, count: u32, views: *const Option<InterfaceRef<'static, ID3D11RenderTargetView>>, depth_view: Option<InterfaceRef<'static, ID3D11DepthStencilView>>);
//type ClearDepth = unsafe extern "system" fn(this: InterfaceRef<'static, ID3D11DeviceContext>, view: Option<InterfaceRef<'static, ID3D11DepthStencilView>>, flags: u32, depth: f32, fill_value: u8);
type Release = unsafe extern "system" fn(this: InterfaceRef<'static, IUnknown>) -> u32;

pub(crate) static LENS_PTR: AtomicPtr<ID3D11DepthStencilView> = AtomicPtr::new(ptr::null_mut());
pub(crate) static GOGGLES: OnceLock<Goggles> = OnceLock::new();
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
        Some(lens) if lens_valid(lens.as_ptr()) => Some(unsafe {
            InterfaceRef::from_raw(lens.cast())
        }),
        _ => None,
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LensClass {
    Unknown,
    //Imgui,
    Space,
    World,
    Test,
    Dummy,
    UI,
    Overlay,
}

/*
unsafe extern "system" fn taimi_set_depth_state(
    this: InterfaceRef<'static, ID3D11DeviceContext>,
    state: Option<InterfaceRef<'static, ID3D11DepthStencilState>>,
    stencil_ref: u32,
) {
    log::trace!("D3D11DeviceContext::OMSetDepthStencilState({this:?}, {state:?}, {stencil_ref:?})");
    #[cfg(todo)]
    let orig = match GOGGLES.get() {
        Some(orig) => orig.set_depth_state,
        None => {
            log::warn!("set_depth_state in place without original?");
            return
        },
    };

    orig(this, state, stencil_ref)
}*/

unsafe extern "system" fn taimi_set_targets(
    this: InterfaceRef<'static, ID3D11DeviceContext>,
    count: u32,
    views_ptr: *const Option<InterfaceRef<'static, ID3D11RenderTargetView>>,
    depth_view: Option<InterfaceRef<'static, ID3D11DepthStencilView>>,
) {
    let views = match count as usize {
        0 => &[],
        count => from_raw_parts(views_ptr, count),
    };

    //log::trace!("D3D11DeviceContext::OMSetRenderTargets({this:?}, {views:?}, {depth_view:?})");

    if let Some(view) = depth_view {
        let key = view.as_raw() as usize;
        let known = LENSES.read().map_err(drop).map(|l| l.get(&key).copied());
        match known {
            Ok(Some(lens)) => {
                //log::trace!("recognized as {lens:?}");
            },
            Ok(None) => {
                log::debug!("unknown buffer, attempting classification...");
                let mut viewports = [D3D11_VIEWPORT::default(); 4];
                let mut count = viewports.len() as u32;
                this.RSGetViewports(&mut count, Some(viewports.as_mut_ptr()));
                //log::debug!("viewports: {:?}", viewports.get(..count as usize));
                let cls = {
                    let mut desc_view = Default::default();
                    let mut desc_state = Default::default();
                    let mut state = None;
                    let mut stencil_ref = 0u32;
                    view.GetDesc(&mut desc_view);
                    this.OMGetDepthStencilState(Some(&mut state), Some(&mut stencil_ref));
                    if let Some(state) = &state {
                        state.GetDesc(&mut desc_state);
                    }
                    match &state {
                        Some(state) => {
                            log::trace!("{view:?} was ref=0x{stencil_ref:08x}, {:?}", state);
                            log::trace!("{desc_state:?}");
                            match desc_state.DepthEnable.0 != 0 {
                                false if desc_state.DepthWriteMask != D3D11_DEPTH_WRITE_MASK_ZERO => {
                                    Some(LensClass::UI)
                                },
                                true if desc_state.DepthWriteMask == D3D11_DEPTH_WRITE_MASK_ZERO => {
                                    log::trace!("skipping for now (read-only bind)");
                                    None
                                },
                                true if desc_state.DepthFunc == D3D11_COMPARISON_LESS => Some(match stencil_ref {
                                    0 => LensClass::World,
                                    _ => LensClass::Dummy,
                                }),
                                true if desc_state.DepthFunc == D3D11_COMPARISON_LESS_EQUAL =>
                                    Some(LensClass::Test),
                                true =>
                                    Some(LensClass::Unknown),
                                false =>
                                    Some(LensClass::Overlay),
                            }
                        },
                        None => {
                            log::warn!("failed to get state, maybe it doesn't exist?");
                            Some(LensClass::Unknown)
                        }
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
                }
            },
            Err(()) => {
                // poisoned???
            },
        }
    }

    match GOGGLES.get() {
        Some(orig) => orig.set_targets
            .call(this, count, views_ptr, depth_view),
        None => {
            log::warn!("set_targets in place without original?");
        },
    };
}

unsafe extern "system" fn taimi_release_depth_view(
    this: InterfaceRef<'static, IUnknown>,
) -> u32 {
    //log::trace!("IUnknown::Release({this:?}, {views:?}, {depth_view:?})");

    if let Some(release) = GOGGLES.get().and_then(|o| o.release_depth_view.as_ref()) {
        let key = match this.cast::<ID3D11DepthStencilView>().ok() {
            None => None,
            Some(view) => {
                let key = view.as_raw() as usize;
                let view_ref = IUnknown::from(view).into_raw();
                let _refcount = release.call(unsafe {
                    InterfaceRef::from_raw(NonNull::new_unchecked(view_ref))
                });

                Some(key)
            },
        };

        let refcount = release.call(this);

        match key {
            Some(key) if refcount == 0 => {
                let removed = if let Ok(mut lenses) = LENSES.write() {
                    lenses.remove(&key).is_some()
                } else {
                    false
                };
                if removed {
                    log::trace!("released depth view {key:08x}");
                }
            },
            _ => (),
        }
        refcount
    } else {
        log::warn!("taimi_release_depth_view called without hook?");
        1
    }
}

/*unsafe extern "system" fn taimi_clear_depth(
    this: InterfaceRef<ID3D11DeviceContext>,
    view: Option<InterfaceRef<ID3D11DepthStencilView>>,
    flags: u32,
    depth: f32,
    fill_value: u8,
) {
    log::trace!("D3D11DeviceContext::ClearDepthStencilView({this:?}, {view:?}, {flags:?}, {depth:?}, {fill_value:?})");
    let orig = match GOGGLES.get() {
        Some(orig) => orig.clear_depth,
        None => {
            log::warn!("clear_depth in place without original?");
            return
        },
    };
    orig(this, view, flags, depth, fill_value)
}*/

pub fn setup(ctx: InterfaceRef<ID3D11DeviceContext>) -> anyhow::Result<()> {
    /*let set_depth_state: unsafe extern "system" fn (*mut c_void, *mut c_void, u32) = ctx.vtable().OMSetDepthStencilState;
    let set_depth_state: SetDepthState = unsafe { transmute(set_depth_state) };*/
    /*let clear_depth: unsafe extern "system" fn (*mut c_void, *mut c_void, u32, f32, u8) = ctx.vtable().ClearDepthStencilView;
    let clear_depth: ClearDepth = unsafe { transmute(clear_depth) };*/
    let set_targets: unsafe extern "system" fn (*mut c_void, u32, *const *mut c_void, *mut c_void) = ctx.vtable().OMSetRenderTargets;
    let set_targets: SetTargets = unsafe { transmute(set_targets) };
    let release_depth_view: Option<unsafe extern "system" fn (*mut c_void) -> u32> = ENGINE.with_borrow(|e| match e {
        Some(e) => Some(e.render_backend.depth_handler.depth_stencil_view.vtable().base__.base__.base__.Release),
        None => None,
    });
    let release_depth_view: Option<Release> = unsafe { transmute(release_depth_view) };

    let orig = unsafe {
        Goggles {
            //set_depth_state: GenericDetour::new(set_depth_state, taimi_set_depth_state)?,
            set_targets: GenericDetour::new(set_targets, taimi_set_targets)?,
            release_depth_view: release_depth_view.map(|f| GenericDetour::new(f, taimi_release_depth_view))
                .transpose()?,
        }
    };
    GOGGLES.set(orig)
        .map_err(|_| anyhow!("goggles already set up?"))
}

pub fn enable() -> anyhow::Result<()> {
    let orig = GOGGLES.get()
        .ok_or_else(|| anyhow!("can't enable what hasn't been set up first"))?;

    unsafe {
        orig.set_targets.enable()?;
        if let Some(release_depth_view) = &orig.release_depth_view {
            release_depth_view.enable()?;
        }
    }

    Ok(())
}

pub fn disable() -> anyhow::Result<()> {
    let orig = GOGGLES.get()
        .ok_or_else(|| anyhow!("can't disable what hasn't been set up first"))?;

    let mut res: anyhow::Result<()> = Ok(());

    unsafe {
        if let Err(e) = orig.set_targets.disable() {
            res = Err(e.into());
        }
        if let Some(Err(e)) = orig.release_depth_view.as_ref().map(|r| r.disable()) {
            res = Err(e.into());
        }
    }

    res
}

/*pub fn needs_classification(cls: LensClass) -> bool {
    match cls {
        LensClass::Space if ENGINE.with_borrow(|e| e.is_none()) =>
            false,
        LensClass::Imgui =>
            false,
        _ => true,
    }
}*/

pub fn has_classification(cls: LensClass) -> Option<bool> {
    LENSES.try_read().ok()
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
    let dsview = engine.render_backend.depth_handler.depth_stencil_view.as_raw();
    classify_lens(dsview as *mut _, LensClass::Space);
}
