use {
    crate::render::machine::{frame_log, FrameState},
    anyhow::anyhow,
    core::{
        ffi::c_void,
        mem::transmute,
        slice,
        num::NonZero,
        mem,
        ops,
    },
    retour::GenericDetour,
    std::sync::OnceLock,
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
        },
    },
    taimi_hoard::lazyfmt,
};
pub use self::lens::{
    current_lens, clear_lens, pick_lens, LensClass,
};
#[cfg(feature = "goggles2-camera")]
pub use self::camera::{
    FerretResource, PerspectiveFerret, CameraFerret,
};
#[cfg(feature = "goggles2-camera")]
pub(crate) use self::camera::g2;

pub mod lens;
#[cfg(feature = "goggles2-camera")]
pub mod camera;

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

pub(crate) static GOGGLES: OnceLock<Goggles> = OnceLock::new();

#[inline]
pub fn is_enabled() -> bool {
    !lens::read_lens().is_null()
}

unsafe extern "system" fn taimi_set_depth_state(
    this: InterfaceRef<'static, ID3D11DeviceContext>,
    state: Option<InterfaceRef<'static, ID3D11DepthStencilState>>,
    stencil_ref: u32,
) {
    if frame_log!(::is_game()) {
        frame_log!(;"D3D11DeviceContext::OMSetDepthStencilState({this:?}, {state:?}, {stencil_ref:?})");
    }

    if FrameState::is_game() {
        lens::set_depth_state(this, state, stencil_ref);
    }

    let mut trigger = false;
    if let Some(state) = state {
        if state.as_raw() as usize == g2!(*&ferret.buffer_ferret) as usize {
            trigger = true;
        }
    }
    if trigger {
        #[cfg(feature = "goggles2")]
        if g2!(*&ferret.ferret_draw) {
            if FrameState::is_game() && !g2!(*&ferret.ferret_drawn) {
                g2!(*&mut ferret.ferret_drawn = true);
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
    if FrameState::is_game() {
        lens::set_targets(this, count, views_ptr, depth_view);
    }

    if count > 0 {
        let mut trigger = false;
        if let Some(v) = depth_view {
            if v.as_raw() as usize == g2!(*&ferret.buffer_ferret) as usize {
                trigger = true;
            }
        }
        if let Some(v) = *views_ptr {
            if v.as_raw() as usize == g2!(*&ferret.buffer_ferret) as usize {
                trigger = true;
            }
        }
        if trigger {
            if let Some(v) = *views_ptr {
                #[cfg(feature = "goggles2")]
                if g2!(*&ferret.ferret_draw) {
                    if FrameState::is_game() && !g2!(*&ferret.ferret_drawn) {
                        g2!(*&mut ferret.ferret_drawn = true);
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
    #[cfg(feature = "goggles2-camera")]
    if FrameState::is_game() {
        let bounds = || match () {
            _ if data.is_null() => None,
            _ if !dst_box.is_null() => {
                // TODO...
                //Some((u32::MAX, None))
                None
            },
            _ if src_depth_pitch > 0 && src_depth_pitch != src_row_pitch => {
                // TODO...
                None
            },
            _ if src_depth_pitch > 0 => Some({
                (0u32, Some(src_depth_pitch))
            }),
            _ => Some((0u32, None)),
        };
        let (mut matched_cam, mut matched_persp) = (false, false);
        if camera::wants_update_camera(resource.as_raw()) {
            if let Some((offset, len)) = bounds() {
                let len = len.and_then(|l| NonZero::new(l));
                camera::update_camera(r, subresource, data as *const _, offset, len);
                matched_cam = true;
            }
        }
        if camera::wants_update_perspective(resource.as_raw()) {
            if let Some((offset, len)) = bounds() {
                let len = len.and_then(|l| NonZero::new(l));
                camera::update_perspective(r, subresource, data as *const _, offset, len);
                matched_persp = true;
            }
        }
        let matched = (matched_cam, matched_persp);
        let mut buffer = None;
        let mut datasize = 0;
        let mut region = 0..0;
        let camera_wants = camera::wants_update_subresource_pre(resource.as_raw(), matched);
        let log_wants = frame_log!(::is_game());
        let bounds = (camera_wants | log_wants).then(bounds);
        match bounds {
            _ if subresource != 0 => (),
            None | Some(None) => (),
            Some(..) if r.get_type_d3d() != ResourceDimension::BUFFER => (),
            Some(Some((offset, size))) => {
                let b = taimi_d3d::dx11::buffer::Buffer::from_d3d_ref(
                    &*(r.as_d3d() as *const ID3D11Resource as *const ID3D11Buffer),
                );
                let desc = b.desc();
                datasize = desc.ByteWidth;
                region = offset..size.unwrap_or(datasize);
                let binds = taimi_d3d::dx11::buffer::BindFlags::CONSTANT.bits()
                    /*| taimi_d3d::dx11::buffer::BindFlags::VERTEX.0*/;
                if (desc.BindFlags & binds) != 0 {
                    buffer = Some(b);
                    #[cfg(todo)]
                    if datasize < 4 {
                        buffer = None;
                    }
                }
            },
        }
        if let Some(buffer) = buffer {
            if log_wants {
                let ops::Range { start: offset, end } = region;
                frame_log!(;
                    "D3D11DeviceContext::UpdateSubresource({:p}, {:p}[{}{:#x}])",
                    resource.as_raw(),
                    data,
                    lazyfmt::or_empty((offset > 0).then_some(format_args!("+{offset:#x}..{end:#x}/"))),
                    datasize,
                );
            }
            if camera_wants && camera::wants_update_subresource(datasize, region.clone(), matched) {
                let offset = region.start as usize;
                let size = region.end as usize - offset;
                let len = size / mem::size_of::<u32>();
                let data = slice::from_raw_parts(data as *const u32, len);
                camera::update_subresource(r, buffer, data, region.start, matched);
            }
        }
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
        let lens_key = lens::release_depth_view_pre(this, release);

        let refcount = release.call(this);

        if let Some(key) = lens_key {
            lens::release_depth_view(refcount, key);
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
    lens::shutdown();

    disable()
}
