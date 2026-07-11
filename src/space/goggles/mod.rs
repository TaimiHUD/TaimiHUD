use {
    crate::{exports::runtime::log::DeferredLogger, render::machine::frame_log},
    anyhow::anyhow,
    core::{ffi::c_void, mem, mem::transmute, num::NonZero, ops, slice},
    retour::GenericDetour,
    std::sync::OnceLock,
    taimi_d3d::dx11::{buffer::ResourceDimension, DepthState, DepthView, RenderTargetView},
    taimi_hoard::lazyfmt,
    //taimi_d3d::prelude::*,
    windows::{
        core::{IUnknown, Interface, InterfaceRef},
        Win32::Graphics::Direct3D11::{
            ID3D11Buffer,
            ID3D11DepthStencilState,
            ID3D11DepthStencilView,
            ID3D11DepthStencilView_Vtbl,
            ID3D11DeviceContext,
            ID3D11DeviceContext_Vtbl,
            ID3D11RenderTargetView,
            ID3D11Resource,
            ID3D11UnorderedAccessView,
            D3D11_BOX,
        },
    },
};

pub(crate) use self::tracking::{g2, GogglesShared as FerretResource};
pub use self::{
    class::{D3dNn, D3dPtr},
    tracking::{GogglesFlags, GogglesShared, GogglesState},
};

#[cfg(feature = "goggles2-camera")]
pub mod camera;
pub mod class;
pub mod d3d;
pub mod lens;
#[cfg(feature = "goggles2-project")]
pub mod project;
pub(super) mod tracking;

pub struct Goggles {
    pub set_targets: GenericDetour<SetTargets>,
    pub set_targets_uavs: GenericDetour<SetTargetsAndUAVs>,
    #[cfg(todo)]
    pub release_depth_view: Option<GenericDetour<Release>>,
    pub update_subresource: GenericDetour<UpdateSubresource>,
    pub set_depth_state: GenericDetour<SetDepthState>,
    pub clear_depth: GenericDetour<ClearDepth>,
    pub clear_colour: GenericDetour<ClearColour>,
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
type SetTargetsAndUAVs = unsafe extern "system" fn(
    this: InterfaceRef<'static, ID3D11DeviceContext>,
    count: u32,
    views: *const Option<InterfaceRef<'static, ID3D11RenderTargetView>>,
    depth_view: Option<InterfaceRef<'static, ID3D11DepthStencilView>>,
    uav_slot: u32,
    uav_count: u32,
    uavs: *const Option<InterfaceRef<'static, ID3D11UnorderedAccessView>>,
    uav_initial_counts: *const u32,
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
type ClearColour = unsafe extern "system" fn(
    this: InterfaceRef<'static, ID3D11DeviceContext>,
    view: Option<InterfaceRef<'static, ID3D11RenderTargetView>>,
    colour: *const [f32; 4],
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

unsafe extern "system" fn taimi_set_depth_state(
    this: InterfaceRef<'static, ID3D11DeviceContext>,
    state: Option<InterfaceRef<'static, ID3D11DepthStencilState>>,
    stencil_ref: u32,
) {
    if GogglesShared::is_game_dx11(&*this) {
        let state = state.as_ref().map(|s| DepthState::from_d3d_ref(s));
        frame_log!("D3D11DeviceContext::OMSetDepthStencilState({state:?}, {stencil_ref:?})");
        #[cfg(feature = "goggles2-project")]
        let project2 = project::ProjectShared::on_set_state_prior();
        class::set_state(this.as_ref(), state, stencil_ref);
        #[cfg(feature = "goggles2-project")]
        if let Some(cont) = project2 {
            project::ProjectShared::on_set_state_pre(this.as_ref(), cont);
        }
    }

    let res = match GOGGLES.get() {
        Some(orig) => orig.set_depth_state.call(this, state, stencil_ref),
        None => {
            log::warn!(logger: DeferredLogger::BEST_EFFORT, "set_depth_state in place without original?");
            return
        },
    };

    res
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
        frame_log!(;"D3D11DeviceContext::SetBuffers({slot}, {buffers:?})");
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
            log::warn!(logger: DeferredLogger::BEST_EFFORT, "set_buffers in place without original?");
        },
    }
}
unsafe extern "system" fn taimi_set_targets(
    this: InterfaceRef<'static, ID3D11DeviceContext>,
    count: u32,
    views_ptr: *const Option<InterfaceRef<'static, ID3D11RenderTargetView>>,
    depth_view: Option<InterfaceRef<'static, ID3D11DepthStencilView>>,
) {
    if GogglesShared::is_game_dx11(&*this) {
        let depth = depth_view.as_ref().map(|v| DepthView::from_d3d_ref(v));
        let views = match count as usize {
            0 => &[],
            count => core::slice::from_raw_parts(views_ptr as *const Option<RenderTargetView>, count),
        };
        frame_log!("D3D11DeviceContext::OMSetRenderTargets({views:?}, {depth:?})");
        if let Some(_lock) = GogglesShared::acquire_write() {
            #[cfg(feature = "goggles2-project")]
            let project2 = project::ProjectShared::on_set_targets_prior();
            class::set_targets(this.as_ref(), views, depth, &[]);
            #[cfg(feature = "goggles2-project")]
            if let Some(cont) = project2 {
                project::ProjectShared::on_set_targets_pre(this.as_ref(), views, depth, &[], cont);
            }
        }
    }
    match GOGGLES.get() {
        Some(orig) => orig.set_targets.call(this, count, views_ptr, depth_view),
        None => {
            log::warn!(logger: DeferredLogger::BEST_EFFORT, "set_targets in place without original?");
        },
    };
}
unsafe extern "system" fn taimi_set_targets_uavs(
    this: InterfaceRef<'static, ID3D11DeviceContext>,
    count: u32,
    views_ptr: *const Option<InterfaceRef<'static, ID3D11RenderTargetView>>,
    depth_view: Option<InterfaceRef<'static, ID3D11DepthStencilView>>,
    uav_slot: u32,
    uav_count: u32,
    uavs_ptr: *const Option<InterfaceRef<'static, ID3D11UnorderedAccessView>>,
    uav_initial_counts: *const u32,
) {
    if GogglesShared::is_game_dx11(&*this) {
        let depth = depth_view.as_ref().map(|v| DepthView::from_d3d_ref(v));
        let views = match count as usize {
            0 => &[],
            count => core::slice::from_raw_parts(views_ptr as *const Option<RenderTargetView>, count),
        };
        let uavs = match uav_count as usize {
            0 => &[],
            count =>
                core::slice::from_raw_parts(uavs_ptr as *const Option<ID3D11UnorderedAccessView>, count),
        };
        frame_log!(
            "D3D11DeviceContext::OMSetRenderTargetsAndUAVs({views:?}, {depth:?}, {uav_slot}, {uavs:?})"
        );
        if let Some(_lock) = GogglesShared::acquire_write() {
            #[cfg(feature = "goggles2-project")]
            let project2 = project::ProjectShared::on_set_targets_prior();
            class::set_targets(this.as_ref(), views, depth, uavs);
            #[cfg(feature = "goggles2-project")]
            if let Some(cont) = project2 {
                project::ProjectShared::on_set_targets_pre(this.as_ref(), views, depth, uavs, cont);
            }
        }
    }
    match GOGGLES.get() {
        Some(orig) => orig.set_targets_uavs.call(
            this,
            count,
            views_ptr,
            depth_view,
            uav_slot,
            uav_count,
            uavs_ptr,
            uav_initial_counts,
        ),
        None => {
            log::warn!(logger: DeferredLogger::BEST_EFFORT, "set_targets_uavs in place without original?");
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
    if GogglesShared::is_game_dx11(&*this) {
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
            _ if src_depth_pitch > 0 => Some((0u32, Some(src_depth_pitch))),
            _ => Some((0u32, None)),
        };
        #[cfg(feature = "goggles2-camera")]
        let camera_wants = GogglesShared::acquire_read().map(|_lock| {
            camera::wants_anything(&resource).then(|| {
                (
                    camera::wants_update_camera(resource.as_raw()),
                    camera::wants_update_perspective(resource.as_raw()),
                )
            })
        });

        let mut buffer = None;
        let mut datasize = 0;
        let mut region = 0..0;

        let mut lock = None;
        #[cfg(feature = "goggles2-camera")]
        let camera_matched = match camera_wants {
            Some(Some((wants_cam, wants_persp))) => {
                let (mut matched_cam, mut matched_persp) = (false, false);
                if wants_cam {
                    if let Some((offset, len)) = bounds() {
                        if let Some(_) = lock.get_or_insert_with(|| GogglesShared::acquire_write()) {
                            let len = len.and_then(|l| NonZero::new(l));
                            camera::update_camera(r, subresource, data as *const _, offset, len);
                            matched_cam = true;
                        }
                    }
                }
                if wants_persp {
                    if let Some((offset, len)) = bounds() {
                        if let Some(_) = lock.get_or_insert_with(|| GogglesShared::acquire_write()) {
                            let len = len.and_then(|l| NonZero::new(l));
                            camera::update_perspective(r, subresource, data as *const _, offset, len);
                            matched_persp = true;
                        }
                    }
                }
                Some((matched_cam, matched_persp))
            },
            Some(None) => Some((false, false)),
            None => None,
        };
        #[cfg(feature = "goggles2-camera")]
        let camera_wants = camera_matched
            .map(|matched| {
                if let Some(_) = lock.get_or_insert_with(|| GogglesShared::acquire_write()) {
                    camera::wants_update_subresource_pre(resource.as_raw(), matched)
                } else {
                    false
                }
            })
            .unwrap_or(false);
        #[cfg(not(feature = "goggles2-camera"))]
        let camera_wants = false;
        let log_wants = match frame_log!(::is_enabled()) {
            #[cfg(todo)]
            e => e,
            _ => false,
        };
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
                #[cfg(todo)]
                frame_log!(;
                    "D3D11DeviceContext::UpdateSubresource({:p}, {:p}[{}{:#x}])",
                    resource.as_raw(),
                    data,
                    lazyfmt::or_empty((offset > 0).then_some(format_args!("+{offset:#x}..{end:#x}/"))),
                    datasize,
                );
            }
            #[cfg(feature = "goggles2-camera")]
            let camera_matched = match (camera_wants, camera_matched) {
                (true, Some(matched)) => {
                    debug_assert!(lock.is_some());
                    camera::wants_update_subresource(datasize, region.clone(), matched).then_some(matched)
                },
                _ => None,
            };
            #[cfg(feature = "goggles2-camera")]
            if let Some(matched) = camera_matched {
                let offset = region.start as usize;
                let size = region.end as usize - offset;
                let len = size / mem::size_of::<u32>();
                let data = slice::from_raw_parts(data as *const u32, len);
                camera::update_subresource(r, buffer, data, region.start, matched);
            }
        }
        let _ = lock;
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
            log::warn!(logger: DeferredLogger::BEST_EFFORT, "update_subresource in place without original?");
        },
    };
}

#[cfg(todo)]
unsafe extern "system" fn taimi_release_depth_view(this: InterfaceRef<'static, IUnknown>) -> u32 {
    //log::trace!("IUnknown::Release({this:?}, {views:?}, {depth_view:?})");

    if let Some(release) = GOGGLES.get().and_then(|o| o.release_depth_view.as_ref()) {
        // TODO: GogglesShared::acquire?
        let lens_key = lens::release_depth_view_pre(this, release);

        let refcount = release.call(this);

        if let Some(key) = lens_key {
            lens::release_depth_view(refcount, key);
        }
        refcount
    } else {
        log::warn!(logger: DeferredLogger::BEST_EFFORT, "taimi_release_depth_view called without hook?");
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
    if GogglesShared::is_game_dx11(&*this) {
        let view = view.as_ref().map(|v| DepthView::from_d3d_ref(&*v));
        frame_log!(
            "D3D11DeviceContext::ClearDepthStencilView({view:?}, {flags:?}, {depth:?}, {fill_value:?})"
        );
        if let Some(view) = view {
            if let Some(_lock) = GogglesShared::acquire_write() {
                class::clear_depth(&this, view, flags, depth, fill_value);
            }
        }
    }

    let res = match GOGGLES.get() {
        Some(orig) => orig.clear_depth.call(this, view, flags, depth, fill_value),
        None => {
            log::warn!(logger: DeferredLogger::BEST_EFFORT, "clear_depth in place without original?");
            return
        },
    };

    res
}
unsafe extern "system" fn taimi_clear_colour(
    this: InterfaceRef<'static, ID3D11DeviceContext>,
    view: Option<InterfaceRef<'static, ID3D11RenderTargetView>>,
    colour: *const [f32; 4],
) {
    if GogglesShared::is_game_dx11(&*this) {
        let colour = &*colour;
        let view = view.as_ref().map(|v| RenderTargetView::from_d3d_ref(&*v));
        frame_log!("D3D11DeviceContext::ClearRenderTargetView({view:?}, {colour:?})");
        if let Some(view) = view {
            if let Some(_lock) = GogglesShared::acquire_write() {
                class::clear_colour(&this, view, colour);
            }
        }
    }

    let res = match GOGGLES.get() {
        Some(orig) => orig.clear_colour.call(this, view, colour),
        None => {
            log::warn!(logger: DeferredLogger::BEST_EFFORT, "clear_colour in place without original?");
            return
        },
    };

    res
}

#[inline(always)]
pub fn needs_setup() -> bool {
    GOGGLES.get().is_none()
}
pub fn setup(
    vtable: &ID3D11DeviceContext_Vtbl,
    _vtable_dv: Option<&ID3D11DepthStencilView_Vtbl>,
) -> anyhow::Result<()> {
    let set_depth_state: unsafe extern "system" fn(*mut c_void, *mut c_void, u32) =
        vtable.OMSetDepthStencilState;
    let set_depth_state: SetDepthState = unsafe { transmute(set_depth_state) };

    let clear_depth: unsafe extern "system" fn(*mut c_void, *mut c_void, u32, f32, u8) =
        vtable.ClearDepthStencilView;
    let clear_depth: ClearDepth = unsafe { transmute(clear_depth) };

    let clear_colour: unsafe extern "system" fn(*mut c_void, *mut c_void, *const f32) =
        vtable.ClearRenderTargetView;
    let clear_colour: ClearColour = unsafe { transmute(clear_colour) };

    let set_buffers: unsafe extern "system" fn(*mut c_void, u32, u32, *const *mut c_void) =
        vtable.VSSetConstantBuffers;
    let set_buffers: SetBuffers = unsafe { transmute(set_buffers) };

    let set_targets: unsafe extern "system" fn(*mut c_void, u32, *const *mut c_void, *mut c_void) =
        vtable.OMSetRenderTargets;
    let set_targets: SetTargets = unsafe { transmute(set_targets) };

    let set_targets_uavs: unsafe extern "system" fn(
        *mut c_void,
        u32,
        *const *mut c_void,
        *mut c_void,
        u32,
        u32,
        *const *mut c_void,
        *const u32,
    ) = vtable.OMSetRenderTargetsAndUnorderedAccessViews;
    let set_targets_uavs: SetTargetsAndUAVs = unsafe { transmute(set_targets_uavs) };

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

    #[cfg(todo)]
    let release_depth_view: Option<Release> = {
        let release_depth_view: Option<unsafe extern "system" fn(*mut c_void) -> u32> =
            vtable_dv.map(|vtbl| vtbl.base__.base__.base__.Release);
        release_depth_view.map(|release| unsafe { transmute(release) })
    };

    let orig = unsafe {
        Goggles {
            set_depth_state: GenericDetour::new(set_depth_state, taimi_set_depth_state)?,
            clear_depth: GenericDetour::new(clear_depth, taimi_clear_depth)?,
            clear_colour: GenericDetour::new(clear_colour, taimi_clear_colour)?,
            set_buffers: GenericDetour::new(set_buffers, taimi_set_buffers)?,
            set_targets: GenericDetour::new(set_targets, taimi_set_targets)?,
            set_targets_uavs: GenericDetour::new(set_targets_uavs, taimi_set_targets_uavs)?,
            update_subresource: GenericDetour::new(update_subresource, taimi_update_subresource)?,
            #[cfg(todo)]
            release_depth_view: {
                let release_depth_view = release_depth_view.map(|release_depth_view| {
                    GenericDetour::new(release_depth_view, taimi_release_depth_view)
                });
                if release_depth_view.is_none() {
                    log::debug!("missing ID3D11DepthStencilView template");
                }
                rt::log::debug_ok(release_depth_view.transpose()).flatten()
            },
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
        #[cfg(todo)]
        if let Some(release_depth_view) = &orig.release_depth_view {
            release_depth_view.enable()?;
        }
        orig.clear_depth.enable()?;
        orig.clear_colour.enable()?;
        orig.set_buffers.enable()?;
        orig.set_depth_state.enable()?;
        orig.update_subresource.enable()?;
    }

    Ok(())
}

pub fn disable() -> anyhow::Result<()> {
    let Some(orig) = GOGGLES.get() else { return Ok(()) };

    let mut res: anyhow::Result<()> = Ok(());

    unsafe {
        if let Err(e) = orig.set_targets.disable() {
            res = Err(e.into());
        }
        #[cfg(todo)]
        if let Some(Err(e)) = orig.release_depth_view.as_ref().map(|r| r.disable()) {
            res = Err(e.into());
        }
        if let Err(e) = orig.set_depth_state.disable() {
            res = Err(e.into());
        }
        if let Err(e) = orig.clear_depth.disable() {
            res = Err(e.into());
        }
        if let Err(e) = orig.clear_colour.disable() {
            res = Err(e.into());
        }
        if let Err(e) = orig.set_buffers.disable() {
            res = Err(e.into());
        }
        if let Err(e) = orig.update_subresource.disable() {
            res = Err(e.into());
        }
    }

    res
}

pub fn shutdown() -> anyhow::Result<()> {
    if GOGGLES.get().is_none() {
        return Ok(())
    }

    disable()
}
