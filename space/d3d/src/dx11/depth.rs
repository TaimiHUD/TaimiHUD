pub use crate::dx11::d3d11::{
    ID3D11DepthStencilState,
    ID3D11DepthStencilView,
    D3D11_CLEAR_FLAG,
    D3D11_COMPARISON_FUNC,
    D3D11_DEPTH_STENCILOP_DESC,
    D3D11_DEPTH_STENCIL_DESC,
    D3D11_DEPTH_STENCIL_VIEW_DESC,
    D3D11_DEPTH_STENCIL_VIEW_DESC_0,
    D3D11_STENCIL_OP,
    D3D11_TEX2D_DSV,
};
use crate::{
    dx11::{
        buffer::{self, Resource, Texture2, D3D11_TEXTURE2D_DESC},
        prelude::*,
    },
    D3dContextBindable,
};

impl_d3d! {
    unsafe impl Dx11Child for ID3D11DepthStencilState;

    @[transparent(Dx11Child <= ID3D11DepthStencilState)]
    pub struct DepthState.state;
}

impl_d3d! { impl enum for
    #[derive(Default)]
    pub enum StencilOp: D3D11_STENCIL_OP{i32} {
        #[default]
        Keep(const KEEP) = d3d11::D3D11_STENCIL_OP_KEEP,
        Zero(const ZERO) = d3d11::D3D11_STENCIL_OP_ZERO,
        Replace(const REPLACE) = d3d11::D3D11_STENCIL_OP_REPLACE,
        IncrementSaturate(const INCR_SAT) = d3d11::D3D11_STENCIL_OP_INCR_SAT,
        DecrementSaturate(const DECR_SAT) = d3d11::D3D11_STENCIL_OP_DECR_SAT,
        Invert(const INVERT) = d3d11::D3D11_STENCIL_OP_INVERT,
        Increment(const INCREMENT) = d3d11::D3D11_STENCIL_OP_INCR,
        Decrement(const DECREMENT) = d3d11::D3D11_STENCIL_OP_DECR,
    }
}
impl_d3d! { impl enum for
    #[derive(Default)]
    pub enum ComparisonFunc: D3D11_COMPARISON_FUNC{i32} {
        Never(const NEVER) = d3d11::D3D11_COMPARISON_NEVER,
        Lt(const LESS) = d3d11::D3D11_COMPARISON_LESS,
        Eq(const EQUAL) = d3d11::D3D11_COMPARISON_EQUAL,
        Le(const LESS_EQUAL) = d3d11::D3D11_COMPARISON_LESS_EQUAL,
        Gt(const GREATER) = d3d11::D3D11_COMPARISON_GREATER,
        Ne(const NOT_EQUAL) = d3d11::D3D11_COMPARISON_NOT_EQUAL,
        Ge(const GREATER_EQUAL) = d3d11::D3D11_COMPARISON_GREATER_EQUAL,
        #[default]
        Always(const ALWAYS) = d3d11::D3D11_COMPARISON_ALWAYS,
    }
}

impl DepthState {
    /// func=[D3D11_COMPARISON_ALWAYS](ComparisonFunc::Always) op=[D3D11_STENCIL_OP_KEEP](StencilOp::Keep)
    pub const STENCILOP_DEFAULT: D3D11_DEPTH_STENCILOP_DESC = D3D11_DEPTH_STENCILOP_DESC {
        StencilFunc: ComparisonFunc::ALWAYS,
        StencilDepthFailOp: StencilOp::KEEP,
        StencilFailOp: StencilOp::KEEP,
        StencilPassOp: StencilOp::KEEP,
    };
    /// depth=[D3D11_COMPARISON_LESS](ComparisonFunc::Lt), stencil=[off](Self::STENCILOP_DEFAULT)
    pub const DESC_DEFAULT: D3D11_DEPTH_STENCIL_DESC = D3D11_DEPTH_STENCIL_DESC {
        DepthEnable: BOOL(1),
        DepthWriteMask: d3d11::D3D11_DEPTH_WRITE_MASK_ALL,
        DepthFunc: ComparisonFunc::LESS,
        StencilEnable: BOOL(0),
        StencilReadMask: d3d11::D3D11_DEFAULT_STENCIL_READ_MASK as u8,
        StencilWriteMask: d3d11::D3D11_DEFAULT_STENCIL_WRITE_MASK as u8,
        FrontFace: Self::STENCILOP_DEFAULT,
        BackFace: Self::STENCILOP_DEFAULT,
    };

    pub fn new_with_desc(device: &Dx11Device, desc: &D3D11_DEPTH_STENCIL_DESC) -> anyhow::Result<Self> {
        let mut out: Option<ID3D11DepthStencilState> = None;
        unsafe { device.CreateDepthStencilState(desc, Some(&mut out)) }
            .map_err(anyhow::Error::from)
            .and_then(move |()| out.ok_or_else(|| anyhow!("failed to produce state pointer")))
            .context("CreateDepthStencilState")
            .map(Into::into)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OMDepthState<S = Option<DepthState>> {
    pub state: S,
    pub stencil_ref: u32,
}

impl<S> OMDepthState<S> {
    pub fn with_state<T: Into<S>>(state: T, stencil_ref: u32) -> Self {
        Self { state: state.into(), stencil_ref }
    }

    pub fn new_snapshot(context: &Dx11Context) -> Self
    where
        S: From<Option<ID3D11DepthStencilState>>,
    {
        let mut state = None;
        let mut stencil_ref = 0;
        unsafe {
            context.OMGetDepthStencilState(Some(&mut state), Some(&mut stencil_ref));
        }

        Self::with_state(state, stencil_ref)
    }
}

impl<S> D3dContextBindable<Dx11Context> for OMDepthState<S>
where
    S: D3dInterfacePtr<Interface = ID3D11DepthStencilState>,
{
    fn set(&self, context: &Dx11Context) {
        let state = self.state.as_d3d_param();
        unsafe {
            context.OMSetDepthStencilState(state.as_ref(), self.stencil_ref);
        }
    }
}

impl<S> From<S> for OMDepthState<S> {
    fn from(state: S) -> Self {
        Self::with_state(state, Default::default())
    }
}

impl_d3d! {
    unsafe impl Dx11Child for ID3D11DepthStencilView;

    @[transparent(Dx11Child <= ID3D11DepthStencilView)]
    pub struct DepthView.view;
}

impl_d3d! { impl bitflags for
    pub struct ClearFlags: D3D11_CLEAR_FLAG{u32} {
        const DEPTH = d3d11::D3D11_CLEAR_DEPTH.0;
        const STENCIL = d3d11::D3D11_CLEAR_STENCIL.0;
    },
}
impl ClearFlags {
    pub const DEPTH_STENCIL: Self = Self::from_bits_retain(Self::DEPTH.bits() | Self::STENCIL.bits());
}

impl DepthView {
    pub fn clear(&self, context: &Dx11Context, flags: ClearFlags, depth: f32, stencil: u8) {
        unsafe { context.ClearDepthStencilView(&self.view, flags.to_raw(), depth, stencil) }
    }

    pub const fn desc_for_texture2(
        desc: D3D11_TEX2D_DSV,
        format: dxgi::DXGI_FORMAT,
        flags: u32,
    ) -> D3D11_DEPTH_STENCIL_VIEW_DESC {
        D3D11_DEPTH_STENCIL_VIEW_DESC {
            Format: format,
            ViewDimension: d3d11::D3D11_DSV_DIMENSION_TEXTURE2D,
            Flags: flags,
            Anonymous: D3D11_DEPTH_STENCIL_VIEW_DESC_0 { Texture2D: desc },
        }
    }

    pub const fn desc_for_buffer2<U: Unit<Scalar = u32>>(display_size: Size2<U>) -> D3D11_TEXTURE2D_DESC {
        D3D11_TEXTURE2D_DESC {
            Width: display_size.width,
            Height: display_size.height,
            MipLevels: 1,
            ArraySize: 1,
            Format: dxgi::DXGI_FORMAT_D24_UNORM_S8_UINT,
            SampleDesc: Texture2::DEFAULT_SAMPLE_DESC,
            Usage: buffer::Usage::DEFAULT.to_d3d(),
            BindFlags: buffer::BindFlags::DEPTH.to_uint(),
            CPUAccessFlags: 0,
            MiscFlags: 0,
        }
    }
    pub const BUFFER2D_DESC_UNSIZED: D3D11_TEXTURE2D_DESC = Self::desc_for_buffer2::<u32>(Size2::ZERO);

    pub fn new_with_texture2(
        device: &Dx11Device,
        buffer: &Texture2,
        desc: D3D11_TEX2D_DSV,
        flags: u32,
    ) -> anyhow::Result<Self> {
        let format = buffer.dxgi_format();
        let desc = Self::desc_for_texture2(desc, format, flags);
        Self::new_with_buffer(device, buffer.as_d3d(), &desc)
    }

    pub fn new_with_buffer<R: AsRef<Resource>>(
        device: &Dx11Device,
        buffer: R,
        desc: &D3D11_DEPTH_STENCIL_VIEW_DESC,
    ) -> anyhow::Result<Self> {
        let buffer = buffer.as_ref();
        let mut out: Option<ID3D11DepthStencilView> = None;
        unsafe { device.CreateDepthStencilView(buffer.as_d3d(), Some(desc), Some(&mut out)) }
            .map_err(anyhow::Error::from)
            .and_then(move |()| out.ok_or_else(|| anyhow!("failed to produce view pointer")))
            .context("CreateDepthStencilView")
            .map(Into::into)
    }

    pub fn get_desc(&self) -> D3D11_DEPTH_STENCIL_VIEW_DESC {
        let mut out = Default::default();
        unsafe {
            self.view.GetDesc(&mut out);
        }
        out
    }

    pub fn get_resource(&self) -> anyhow::Result<Dx11Resource> {
        unsafe { self.view.GetResource() }.context("ID3D11DepthStencilView::GetResource")
    }
}
