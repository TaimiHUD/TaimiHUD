use {
    crate::render::{machine::frame_log, RenderEvent, RenderState},
    anyhow::anyhow,
    core::{
        ffi::c_void,
        mem::{self, transmute},
        ptr::{self, NonNull},
        slice,
    },
    retour::GenericDetour,
    std::{
        collections::BTreeMap,
        sync::{
            atomic::{AtomicPtr, Ordering},
            OnceLock,
            RwLock,
        },
    },
    taimi_d3d::dx11::buffer::ResourceDimension,
    //taimi_d3d::prelude::*,
    windows::{
        core::{IUnknown, Interface, InterfaceRef},
        Win32::Graphics::Direct3D11::{
            ID3D11Buffer,
            ID3D11DepthStencilState,
            ID3D11DepthStencilView,
            ID3D11DeviceContext,
            ID3D11DeviceContext_Vtbl,
            ID3D11RenderTargetView,
            ID3D11Resource,
            D3D11_BOX,
            D3D11_COMPARISON_LESS,
            D3D11_COMPARISON_LESS_EQUAL,
            D3D11_DEPTH_WRITE_MASK_ZERO,
            D3D11_VIEWPORT,
        },
    },
};

#[cfg(feature = "space")]
use crate::space::Engine;

pub type Lenses = BTreeMap<usize, LensClass>;

pub struct Goggles {
    pub set_targets: GenericDetour<SetTargets>,
    pub release_depth_view: Option<GenericDetour<Release>>,
    pub update_subresource: GenericDetour<UpdateSubresource>,
    pub set_depth_state: GenericDetour<SetDepthState>,
    pub clear_depth: GenericDetour<ClearDepth>,
    pub set_buffers: GenericDetour<SetBuffers>,
}

type SetDepthState = unsafe extern "system" fn(
    this: InterfaceRef<'static, ID3D11DeviceContext>,
    buffer: Option<InterfaceRef<'static, ID3D11DepthStencilState>>,
    u32,
);
type SetTargets = unsafe extern "system" fn(
    this: InterfaceRef<'static, ID3D11DeviceContext>,
    count: u32,
    views: *const Option<InterfaceRef<'static, ID3D11RenderTargetView>>,
    depth_view: Option<InterfaceRef<'static, ID3D11DepthStencilView>>,
);
type SetBuffers = unsafe extern "system" fn(
    this: InterfaceRef<'static, ID3D11DeviceContext>,
    slot: u32,
    count: u32,
    buffers: *const Option<InterfaceRef<'static, ID3D11Buffer>>,
);
type ClearDepth = unsafe extern "system" fn(
    this: InterfaceRef<'static, ID3D11DeviceContext>,
    view: Option<InterfaceRef<'static, ID3D11DepthStencilView>>,
    flags: u32,
    depth: f32,
    fill_value: u8,
);
type Release = unsafe extern "system" fn(this: InterfaceRef<'static, IUnknown>) -> u32;
type UpdateSubresource = unsafe extern "system" fn(
    this: InterfaceRef<'static, ID3D11DeviceContext>,
    resource: InterfaceRef<'static, ID3D11Resource>,
    subresource: u32,
    dst_box: *const D3D11_BOX,
    data: *const c_void,
    src_row_pitch: u32,
    src_depth_pitch: u32,
);

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
        if let Some((&world_key, _cls)) = lenses.iter().find(|(_, cls)| matches!(cls, LensClass::World)) {
            LENS_PTR.store(world_key as *mut _, Ordering::Relaxed);
        }
    }
}

#[inline]
pub fn is_enabled() -> bool {
    !read_lens().is_null()
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

unsafe extern "system" fn taimi_set_depth_state(
    this: InterfaceRef<'static, ID3D11DeviceContext>,
    state: Option<InterfaceRef<'static, ID3D11DepthStencilState>>,
    stencil_ref: u32,
) {
    if frame_log!(::is_game()) {
        frame_log!(;"D3D11DeviceContext::OMSetDepthStencilState({this:?}, {state:?}, {stencil_ref:?})");
    }
    let mut trigger = false;
    if let Some(state) = state {
        if state.as_raw() as usize == FerretResource::get_buffer_ferret() as usize {
            trigger = true;
        }
    }
    if trigger {
        #[cfg(feature = "goggles2")]
        if FerretResource::get_ferret_draw() {
            if !frame_log!(::is_taimi()) && !FerretResource::get_ferret_drawn() {
                FerretResource::set_ferret_drawn(true);
                let mut state = crate::render::RenderState::lock();
                if let Some(state) = &mut *state {
                    if let Some(Ok(engine)) = &mut state.engine {
                        engine.render_carefully(&mut state.machine, &this);
                    }
                }
                drop(state);
                //log::debug!("careful'd");
            }
        } else {
            return
        }
    }
    match GOGGLES.get() {
        Some(orig) => orig.set_depth_state.call(this, state, stencil_ref),
        None => {
            log::warn!("set_depth_state in place without original?");
            return
        },
    }
}

unsafe extern "system" fn taimi_set_buffers(
    this: InterfaceRef<'static, ID3D11DeviceContext>,
    slot: u32,
    count: u32,
    buffers_ptr: *const Option<InterfaceRef<'static, ID3D11Buffer>>,
) {
    // if buflen == 144 or 224, check it for a matrix btw!
    if frame_log!(::is_game()) {
        let buffers = match count as usize {
            0 => &[],
            count => core::slice::from_raw_parts(buffers_ptr, count),
        };
        frame_log!(;"D3D11DeviceContext::SetBuffers({slot}, {this:?}, {buffers:?})");
        for (i, buffer) in buffers.iter().enumerate() {
            let Some(buffer) = buffer else { continue };
            let mut desc = Default::default();
            buffer.GetDesc(&mut desc);
            frame_log!(;" #{i}: {:p} {} (x{}) bind={:#x} use={:#x} cpu={:#x}", buffer.as_raw(), desc.ByteWidth, desc.StructureByteStride, desc.BindFlags, desc.Usage.0, desc.CPUAccessFlags);
        }
    }
    match GOGGLES.get() {
        Some(orig) => orig.set_buffers.call(this, slot, count, buffers_ptr),
        None => {
            log::warn!("set_buffers in place without original?");
        },
    }
}
unsafe extern "system" fn taimi_set_targets(
    this: InterfaceRef<'static, ID3D11DeviceContext>,
    count: u32,
    views_ptr: *const Option<InterfaceRef<'static, ID3D11RenderTargetView>>,
    depth_view: Option<InterfaceRef<'static, ID3D11DepthStencilView>>,
) {
    if frame_log!(::is_game()) {
        let views = match count as usize {
            0 => &[],
            count => core::slice::from_raw_parts(views_ptr, count),
        };
        frame_log!(;"D3D11DeviceContext::OMSetRenderTargets({this:?}, {views:?}, {depth_view:?})");
    }

    if let Some(view) = depth_view {
        let key = view.as_raw() as usize;
        let known = LENSES.read().map_err(drop).map(|l| l.get(&key).copied());
        match known {
            Ok(Some(_lens)) => {
                //log::trace!("recognized as {lens:?}");
            },
            Ok(None) => {
                //log::debug!("unknown buffer, attempting classification...");
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
                        Some(_state) => {
                            //log::trace!("{view:?} was ref=0x{stencil_ref:08x}, {:?}", state);
                            //log::trace!("{desc_state:?}");
                            match desc_state.DepthEnable.0 != 0 {
                                false if desc_state.DepthWriteMask != D3D11_DEPTH_WRITE_MASK_ZERO =>
                                    Some(LensClass::UI),
                                true if desc_state.DepthWriteMask == D3D11_DEPTH_WRITE_MASK_ZERO => {
                                    //log::trace!("skipping for now (read-only bind)");
                                    None
                                },
                                true if desc_state.DepthFunc == D3D11_COMPARISON_LESS =>
                                    Some(match stencil_ref {
                                        0 => LensClass::World,
                                        _ => LensClass::Dummy,
                                    }),
                                true if desc_state.DepthFunc == D3D11_COMPARISON_LESS_EQUAL =>
                                    Some(LensClass::Test),
                                true => Some(LensClass::Unknown),
                                false => Some(LensClass::Overlay),
                            }
                        },
                        None => {
                            log::warn!("failed to get state, maybe it doesn't exist?");
                            Some(LensClass::Unknown)
                        },
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
            },
            Err(()) => {
                // poisoned???
            },
        }
    }

    if count > 0 {
        let mut trigger = false;
        if let Some(v) = depth_view {
            if v.as_raw() as usize == FerretResource::get_buffer_ferret() as usize {
                trigger = true;
            }
        }
        if let Some(v) = *views_ptr {
            if v.as_raw() as usize == FerretResource::get_buffer_ferret() as usize {
                trigger = true;
            }
        }
        if trigger {
            if let Some(v) = *views_ptr {
                #[cfg(feature = "goggles2")]
                if FerretResource::get_ferret_draw() {
                    if !frame_log!(::is_taimi()) && !FerretResource::get_ferret_drawn() {
                        FerretResource::set_ferret_drawn(true);
                        let mut state = crate::render::RenderState::lock();
                        if let Some(state) = &mut *state {
                            if let Some(Ok(engine)) = &mut state.engine {
                                engine.render_carefully(&mut state.machine, &this);
                            }
                        }
                        drop(state);
                        //log::debug!("careful'd");
                    }
                } else {
                    this.ClearRenderTargetView(v, &[0.5, 0.7, 0.2, 0.5]);
                }
            }
        }
    }
    match GOGGLES.get() {
        Some(orig) => orig.set_targets.call(this, count, views_ptr, depth_view),
        None => {
            log::warn!("set_targets in place without original?");
        },
    };
}
unsafe extern "system" fn taimi_update_subresource(
    this: InterfaceRef<'static, ID3D11DeviceContext>,
    resource: InterfaceRef<'static, ID3D11Resource>,
    subresource: u32,
    dst_box: *const D3D11_BOX,
    data: *const c_void,
    src_row_pitch: u32,
    src_depth_pitch: u32,
) {
    let r = taimi_d3d::dx11::Resource::from_d3d_ref(&resource);
    let mut buffer = None;
    let mut datasize = 0;
    match !frame_log!(::is_taimi()) {
        false => (),
        true if subresource != 0 => (),
        true if !dst_box.is_null() => (),
        true if data.is_null() => (),
        true if src_depth_pitch != 0 && src_depth_pitch != src_row_pitch => (),
        true if r.get_type_d3d() != ResourceDimension::BUFFER => (),
        true => {
            let b = taimi_d3d::dx11::buffer::Buffer::from_d3d_ref(
                &*(r.as_d3d() as *const ID3D11Resource as *const ID3D11Buffer),
            );
            let desc = b.desc();
            let binds = taimi_d3d::dx11::buffer::BindFlags::CONSTANT.bits()
                /*| taimi_d3d::dx11::buffer::BindFlags::VERTEX.0*/;
            if (desc.BindFlags & binds) != 0 {
                buffer = Some(b);
                datasize = match src_row_pitch {
                    0 => desc.ByteWidth as u32,
                    pitch => pitch,
                };
                if datasize % 4 != 0 {
                    buffer = None;
                }
                #[cfg(todo)]
                if !FerretResource::get_size_range().contains(datasize) {
                    buffer = None;
                }
            }
        },
    }
    if buffer.is_some() {
        frame_log!(
            "D3D11DeviceContext::UpdateSubresource({:p}, {:p}[{:#x}])",
            resource.as_raw(),
            data,
            datasize
        );
        if datasize >= 0x10000 || !FerretResource::get_size_range().contains(&(datasize as u16)) {
            buffer = None;
        }
    }
    if !frame_log!(::is_enabled()) {
        buffer = None;
    }
    if let Some(..) = buffer {
        let data = slice::from_raw_parts(data as *const u32, (datasize / 4) as usize);
        let gran = FerretResource::get_granularity() as usize;
        if let Some(m) = FerretResource::get_perspective().search(data, gran) {
            //print?
        }
        if let Some(m) = FerretResource::get_camera().search(data, gran) {}
    }
    match GOGGLES.get() {
        Some(orig) => orig.update_subresource.call(
            this,
            resource,
            subresource,
            dst_box,
            data,
            src_row_pitch,
            src_depth_pitch,
        ),
        None => {
            log::warn!("update_subresource in place without original?");
        },
    };
}

unsafe extern "system" fn taimi_release_depth_view(this: InterfaceRef<'static, IUnknown>) -> u32 {
    //log::trace!("IUnknown::Release({this:?}, {views:?}, {depth_view:?})");

    if let Some(release) = GOGGLES.get().and_then(|o| o.release_depth_view.as_ref()) {
        let key = match this.cast::<ID3D11DepthStencilView>().ok() {
            None => None,
            Some(view) => {
                let key = view.as_raw() as usize;
                let view_ref = IUnknown::from(view).into_raw();
                let _refcount =
                    release.call(unsafe { InterfaceRef::from_raw(NonNull::new_unchecked(view_ref)) });

                Some(key)
            },
        };

        let refcount = release.call(this);

        match key {
            Some(key) if refcount == 0 => {
                let removed = if let Ok(mut lenses) = LENSES.write() {
                    let removed = lenses.remove(&key);
                    if let Some(LensClass::World) = removed {
                        RenderState::try_send(RenderEvent::UiDepthReleased());
                        let _ = LENS_PTR.compare_exchange(
                            key as *mut _,
                            ptr::dangling_mut(),
                            Ordering::Relaxed,
                            Ordering::Relaxed,
                        );
                    }
                    removed.is_some()
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

unsafe extern "system" fn taimi_clear_depth(
    this: InterfaceRef<'static, ID3D11DeviceContext>,
    view: Option<InterfaceRef<'static, ID3D11DepthStencilView>>,
    flags: u32,
    depth: f32,
    fill_value: u8,
) {
    if frame_log!(::is_game()) {
        frame_log!(;"D3D11DeviceContext::ClearDepthStencilView({this:?}, {view:?}, {flags:?}, {depth:?}, {fill_value:?})");
    }
    match GOGGLES.get() {
        Some(orig) => orig.clear_depth.call(this, view, flags, depth, fill_value),
        None => {
            log::warn!("clear_depth in place without original?");
            return
        },
    };
}

// TODO: pass ID3D11DepthStencilView_Vtbl .-.
pub fn setup(vtable: &ID3D11DeviceContext_Vtbl) -> anyhow::Result<()> {
    let set_depth_state: unsafe extern "system" fn(*mut c_void, *mut c_void, u32) =
        vtable.OMSetDepthStencilState;
    let set_depth_state: SetDepthState = unsafe { transmute(set_depth_state) };

    let clear_depth: unsafe extern "system" fn(*mut c_void, *mut c_void, u32, f32, u8) =
        vtable.ClearDepthStencilView;
    let clear_depth: ClearDepth = unsafe { transmute(clear_depth) };

    let set_buffers: unsafe extern "system" fn(*mut c_void, u32, u32, *const *mut c_void) =
        vtable.VSSetConstantBuffers;
    let set_buffers: SetBuffers = unsafe { transmute(set_buffers) };

    let set_targets: unsafe extern "system" fn(*mut c_void, u32, *const *mut c_void, *mut c_void) =
        vtable.OMSetRenderTargets;
    let set_targets: SetTargets = unsafe { transmute(set_targets) };

    let update_subresource: unsafe extern "system" fn(
        *mut c_void,
        *mut c_void,
        u32,
        *const D3D11_BOX,
        *const c_void,
        u32,
        u32,
    ) = vtable.UpdateSubresource;
    let update_subresource: UpdateSubresource = unsafe { transmute(update_subresource) };

    let release_depth_view: unsafe extern "system" fn(*mut c_void) -> u32 =
        crate::space::dx11::DepthHandler::depth_stencil_view_vtbl()
            .map(|vtbl| vtbl.base__.base__.base__.Release)
            .ok_or_else(|| anyhow!("can't find ID3D11DepthStencilView template"))?;
    let release_depth_view: Release = unsafe { transmute(release_depth_view) };

    let orig = unsafe {
        Goggles {
            set_depth_state: GenericDetour::new(set_depth_state, taimi_set_depth_state)?,
            clear_depth: GenericDetour::new(clear_depth, taimi_clear_depth)?,
            set_buffers: GenericDetour::new(set_buffers, taimi_set_buffers)?,
            set_targets: GenericDetour::new(set_targets, taimi_set_targets)?,
            update_subresource: GenericDetour::new(update_subresource, taimi_update_subresource)?,
            release_depth_view: Some(GenericDetour::new(release_depth_view, taimi_release_depth_view)?),
        }
    };
    GOGGLES.set(orig).map_err(|_| anyhow!("goggles already set up?"))
}

pub fn enable() -> anyhow::Result<()> {
    let orig = GOGGLES
        .get()
        .ok_or_else(|| anyhow!("can't enable what hasn't been set up first"))?;

    unsafe {
        orig.set_targets.enable()?;
        if let Some(release_depth_view) = &orig.release_depth_view {
            release_depth_view.enable()?;
        }
        orig.clear_depth.enable()?;
        orig.set_buffers.enable()?;
        orig.set_depth_state.enable()?;
        orig.update_subresource.enable()?;
    }

    Ok(())
}

pub fn disable() -> anyhow::Result<()> {
    let orig = GOGGLES
        .get()
        .ok_or_else(|| anyhow!("can't disable what hasn't been set up first"))?;

    let mut res: anyhow::Result<()> = Ok(());

    unsafe {
        if let Err(e) = orig.set_targets.disable() {
            res = Err(e.into());
        }
        if let Some(Err(e)) = orig.release_depth_view.as_ref().map(|r| r.disable()) {
            res = Err(e.into());
        }
        if let Err(e) = orig.set_depth_state.disable() {
            res = Err(e.into());
        }
        if let Err(e) = orig.clear_depth.disable() {
            res = Err(e.into());
        }
        if let Err(e) = orig.set_buffers.disable() {
            res = Err(e.into());
        }
        if let Err(e) = orig.update_subresource.disable() {
            res = Err(e.into());
        }
    }

    #[cfg(todo = "unnecessary")]
    #[cfg(feature = "goggles")]
    {
        if let Ok(mut lenses) = LENSES.try_write() {
            lenses.clear();
        }
    }

    res
}

pub fn shutdown() -> anyhow::Result<()> {
    if GOGGLES.get().is_none() {
        return Ok(())
    }
    LENS_PTR.store(ptr::null_mut(), Ordering::SeqCst);

    disable()
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
        let dsview = view.view.as_raw();
        classify_lens(dsview as *mut _, LensClass::Space);
    }
}

pub fn ferret(value: u64) {
    FerretResource::set_buffer_ferret(value)
}

use {
    bitvec::{array::BitArray, order::Lsb0},
    core::ops,
};

pub struct FerretResource {
    pub size_range: ops::Range<u16>,
    pub perspective: PerspectiveFerret,
    pub camera: CameraFerret,
    pub granularity: u8,
    pub buffer_ferret: u64,
    pub ferret_draw: bool,
    pub ferret_drawn: bool,
}
impl FerretResource {
    pub const DEFAULT: Self = Self {
        perspective: PerspectiveFerret::EMPTY,
        camera: CameraFerret::EMPTY,
        granularity: Self::DEFAULT_GRANULARITY,
        size_range: 0u16..0u16,
        buffer_ferret: 0,
        ferret_draw: false,
        ferret_drawn: true,
    };
    const DEFAULT_GRANULARITY: u8 = 2;
    pub fn get() -> *mut Self {
        static FERRET: sync_unsafe_cell::SyncUnsafeCell<FerretResource> =
            sync_unsafe_cell::SyncUnsafeCell::new(FerretResource::DEFAULT);
        FERRET.get()
    }
    pub fn get_buffer_ferret() -> u64 {
        unsafe { ptr::read_volatile(&raw const (*Self::get()).buffer_ferret) }
    }
    pub fn set_buffer_ferret(v: u64) {
        unsafe { ptr::write_volatile(&raw mut (*Self::get()).buffer_ferret, v) }
    }
    pub fn get_ferret_draw() -> bool {
        unsafe { ptr::read_volatile(&raw const (*Self::get()).ferret_draw) }
    }
    pub fn get_ferret_drawn() -> bool {
        unsafe { ptr::read_volatile(&raw const (*Self::get()).ferret_drawn) }
    }
    pub fn set_ferret_drawn(v: bool) {
        unsafe { ptr::write_volatile(&raw mut (*Self::get()).ferret_drawn, v) }
    }
    pub fn set_ferret_draw(v: bool) {
        unsafe { ptr::write_volatile(&raw mut (*Self::get()).ferret_draw, v) }
    }
    pub fn get_granularity() -> u8 {
        unsafe { ptr::read_volatile(&raw const (*Self::get()).granularity) }.max(1)
    }
    pub fn set_granularity(v: u8) {
        unsafe { ptr::write_volatile(&raw mut (*Self::get()).granularity, v) }
    }
    pub fn get_perspective() -> PerspectiveFerret {
        unsafe { ptr::read_volatile(&raw const (*Self::get()).perspective) }
    }
    pub fn set_perspective(v: PerspectiveFerret) {
        unsafe { ptr::write_volatile(&raw mut (*Self::get()).perspective, v) }
    }
    pub fn get_camera() -> CameraFerret {
        unsafe { ptr::read_volatile(&raw const (*Self::get()).camera) }
    }
    pub fn set_camera(v: CameraFerret) {
        unsafe { ptr::write_volatile(&raw mut (*Self::get()).camera, v) }
    }
    pub fn set_size_range(v: ops::Range<u16>) {
        unsafe { ptr::write_volatile(&raw mut (*Self::get()).size_range, v) }
    }
    pub fn get_size_range() -> ops::Range<u16> {
        unsafe { ptr::read_volatile(&raw const (*Self::get()).size_range) }
    }
}
pub struct PerspectiveFerret {
    pub expected_w: f32,
    pub expected_h: f32,
}
impl PerspectiveFerret {
    pub const EMPTY: Self = Self { expected_w: 0.0, expected_h: 0.0 };
    pub fn new(fov_y: f32, aspect_ratio: f32) -> Self {
        let mut ferret = Self::EMPTY;
        ferret.set_expected_perspective(fov_y, aspect_ratio);
        ferret
    }
    const ZERO32: u32 = 0.0f32.to_bits();
    const ONE32: u32 = 1.0f32.to_bits();
    const NEG32: u32 = (-1.0f32).to_bits();
    pub const fn is_empty(&self) -> bool {
        self.expected_h.to_bits() == Self::ZERO32
    }

    pub fn set_expected_perspective(&mut self, fov_y: f32, aspect_ratio: f32) {
        let fov = 0.5 * fov_y;
        let fov_sin = fov.sin();
        let fov_cos = fov.cos();
        self.expected_h = fov_cos / fov_sin;
        self.expected_w = self.expected_h / aspect_ratio;
    }
    const M4_LEN32: usize = 4 * 4;
    /// column-major
    #[cfg(todo)]
    const M4_PERSP_MASK: BitArray<[u32; 1], Lsb0> = bitvec::bitarr![
        const u32, Lsb0;
        1, 0, 0, 0,
        0, 1, 0, 0,
        0, 0, 1, 0,
        0, 0, 1, 0,
    ];
    /// row-major
    const M4_PERSP_MASK: BitArray<[u32; 1], Lsb0> = bitvec::bitarr![
        const u32, Lsb0;
        1, 0, 0, 0,
        0, 1, 0, 0,
        0, 0, 1, 1,
        0, 0, 0, 0,
    ];
    /// 1.0 @ (2,2)
    const M4_PERSP_ONE: usize = 8;
    const M4_PERSP_EPSILON: f32 = 0.05;
    const M4_ZERO_EPSILON: f32 = 0.0005;
    pub fn matches_pre(&self, data: &[u32]) -> bool {
        if data.len() < Self::M4_LEN32 {
            return false
        }
        let mut nonzero = true;
        let checks = Self::M4_PERSP_MASK
            .iter()
            .zip(data)
            .filter_map(|(mask, &v)| match *mask {
                false => Some(v),
                true => {
                    if v == Self::ZERO32 {
                        nonzero = false;
                    }
                    #[cfg(todo)]
                    if v != Self::ZERO32 {
                        nonzero = true;
                    }
                    None
                },
            });
        for (i, v) in checks.enumerate() {
            let f = f32::from_bits(v);
            let expectedf = match i {
                Self::M4_PERSP_ONE => -1.0,
                _ => 0.0,
            };
            if (f/*.abs()*/ - expectedf).abs() > Self::M4_ZERO_EPSILON {
                return false
            }
            #[cfg(todo)]
            let expected = match i {
                #[cfg(todo = "unnecessary")]
                Self::M4_PERSP_ONE if v == Self::ONE32 => {
                    // left-handled...
                    continue
                },
                Self::M4_PERSP_ONE => Self::NEG32,
                _ => Self::ZERO32,
            };
            #[cfg(todo)]
            if v != expected {
                return false
            }
        }
        nonzero
    }
    /// post-filter used after checking [Self::matches_pre]
    pub unsafe fn matches(&self, data: &[u32]) -> bool {
        if self.is_empty() {
            return true
        }
        let exp = [self.expected_w, self.expected_h];
        let checks = Self::M4_PERSP_MASK
            .iter_ones()
            .map(|i| f32::from_bits(*unsafe { data.get_unchecked(i) }))
            .zip(exp);
        for (v, e) in checks {
            let delta = (v - e).abs();
            if delta > Self::M4_PERSP_EPSILON {
                return false
            }
        }
        true
    }
}
impl FerretPattern for PerspectiveFerret {
    fn search<'d>(&self, data: &'d [u32], granularity: usize) -> Option<&'d [u32]> {
        search_ferret(
            data,
            granularity,
            |data| self.matches_pre(data),
            |data| unsafe { self.matches(data) }.then_some(Self::M4_LEN32),
        )
    }
}

pub struct CameraFerret {
    pub expected_dir: glam::Vec3,
}
impl CameraFerret {
    pub const EMPTY: Self = Self { expected_dir: glam::Vec3::INFINITY };
    pub fn new(dir: glam::Vec3) -> Self {
        let mut ferret = Self::EMPTY;
        ferret.set_expected(dir);
        ferret
    }
    const ZERO32: u32 = 0.0f32.to_bits();
    const ONE32: u32 = 1.0f32.to_bits();
    const NEG32: u32 = (-1.0f32).to_bits();
    pub const fn is_empty(&self) -> bool {
        self.expected_dir.x.is_infinite()
    }

    pub fn set_expected(&mut self, dir: glam::Vec3) {
        self.expected_dir = dir;
    }
    const M4_LEN32: usize = 4 * 4;
    /// column-major
    #[cfg(todo)]
    const M4_CAM_MASK: BitArray<[u32; 1], Lsb0> = bitvec::bitarr![
        const u32, Lsb0;
        1, 1, 1, 0,
        1, 1, 1, 0,
        1, 1, 1, 0,
        1, 1, 1, 0,
    ];
    /// row-major
    const M4_CAM_MASK: BitArray<[u32; 1], Lsb0> = bitvec::bitarr![
        const u32, Lsb0;
        1, 1, 1, 1,
        1, 1, 1, 1,
        1, 1, 1, 1,
        0, 0, 0, 0,
    ];
    /// 1.0 @ (2,2)
    const M4_CAM_ONE: usize = 3;
    const M4_CAM_EPSILON: f32 = 0.05;
    const M4_ZERO_EPSILON: f32 = 0.0005;
    pub fn matches_pre(&self, data: &[u32]) -> bool {
        if data.len() < Self::M4_LEN32 {
            return false
        }
        let mut nonzero = true;
        let checks = Self::M4_CAM_MASK
            .iter()
            .zip(data)
            .filter_map(|(mask, &v)| match *mask {
                false => Some(v),
                true => {
                    if v == Self::ZERO32 {
                        nonzero = false;
                    }
                    None
                },
            });
        for (i, v) in checks.enumerate() {
            let f = f32::from_bits(v);
            let expectedf = match i {
                Self::M4_CAM_ONE => 1.0,
                _ => 0.0,
            };
            if (f/*.abs()*/ - expectedf).abs() > Self::M4_ZERO_EPSILON {
                return false
            }
            #[cfg(todo)]
            let expected = match i {
                #[cfg(todo = "unnecessary")]
                Self::M4_CAM_ONE if v == Self::ONE32 => {
                    // left-handled...
                    continue
                },
                Self::M4_CAM_ONE => Self::NEG32,
                _ => Self::ZERO32,
            };
            #[cfg(todo)]
            if v != expected {
                return false
            }
        }
        nonzero
    }
    /// post-filter used after checking [Self::matches_pre]
    pub unsafe fn matches(&self, data: &[u32]) -> bool {
        if self.is_empty() {
            return true
        }
        /*
        let exp = [self.expected_w, self.expected_h];
        let checks = Self::M4_CAM_MASK.iter_ones()
            .map(|i| f32::from_bits(*unsafe { data.get_unchecked(i) })).zip(exp);
        for (v, e) in checks {
            let delta = (v - e).abs();
            if delta > Self::M4_CAM_EPSILON {
                return false
            }
        }*/
        true
    }
}
impl FerretPattern for CameraFerret {
    fn search<'d>(&self, data: &'d [u32], granularity: usize) -> Option<&'d [u32]> {
        search_ferret(
            data,
            granularity,
            |data| self.matches_pre(data),
            |data| unsafe { self.matches(data) }.then_some(Self::M4_LEN32),
        )
    }
}

pub trait FerretPattern {
    fn search<'d>(&self, data: &'d [u32], granularity: usize) -> Option<&'d [u32]>;
}

fn print_ferret(data: &[u32], offset: usize, len: usize) {
    let displen = (len / 4).max(16);
    let prior = offset
        .checked_sub(16)
        .map(|off| {
            core::iter::once_with(move || {
                frame_log!(;"preceding 16:");
                unsafe { data.get_unchecked(off..offset) }
            })
        })
        .into_iter()
        .flatten();
    let found = unsafe { data.get_unchecked(offset..) };
    let chunks = found.chunks(4).take(displen).chain(prior);
    for chunk in chunks {
        use core::fmt::Write;
        let mut line = String::new();
        for &v in chunk {
            let _ = write!(&mut line, "  {:4.03}", f32::from_bits(v));
        }
        frame_log!(;"\t::{line}");
    }
}
fn search_ferret<F, M>(data: &[u32], granularity: usize, mut filter: F, mut matcher: M) -> Option<&[u32]>
where
    F: FnMut(&[u32]) -> bool,
    M: FnMut(&[u32]) -> Option<usize>,
{
    let mut haystack = data;
    let mut pmatch = None;
    while !haystack.is_empty() {
        let next = haystack.get(granularity..).unwrap_or(&[]);
        let search = mem::replace(&mut haystack, next);
        if !filter(search) {
            continue
        }
        let offset = unsafe { search.as_ptr().offset_from(data.as_ptr()) } as usize;
        frame_log!(;"- pre-match @{offset:#x}?");
        if let Some(len) = matcher(search) {
            frame_log!(;"- actual match offset={offset:#x}");
            print_ferret(data, offset, len);
            pmatch = Some(unsafe { search.get_unchecked(..len) });
        }
        #[cfg(todo)]
        break;
    }
    pmatch
}
