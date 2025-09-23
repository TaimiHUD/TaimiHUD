use crate::{
    dx11::{
        buffer::{self, D3D11_TEXTURE2D_DESC, Texture2},
        prelude::*,
        impl_d3d_ext11,
    },
    D3dContextBindable,
};

pub use crate::dx11::d3d11::{
    ID3D11DepthStencilState, ID3D11DepthStencilView,
    D3D11_CLEAR_FLAG,
    D3D11_DEPTH_STENCILOP_DESC, D3D11_DEPTH_STENCIL_DESC, D3D11_DEPTH_STENCIL_VIEW_DESC,
    D3D11_DEPTH_STENCIL_VIEW_DESC_0,
    D3D11_TEX2D_DSV,
};

#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct DepthState {
    pub state: ID3D11DepthStencilState,
}

impl DepthState {
    /// func=D3D11_COMPARISON_ALWAYS op=D3D11_STENCIL_OP_KEEP
    pub const STENCILOP_DEFAULT: D3D11_DEPTH_STENCILOP_DESC = D3D11_DEPTH_STENCILOP_DESC {
        StencilFunc: d3d11::D3D11_COMPARISON_ALWAYS,
        StencilDepthFailOp: d3d11::D3D11_STENCIL_OP_KEEP,
        StencilFailOp: d3d11::D3D11_STENCIL_OP_KEEP,
        StencilPassOp: d3d11::D3D11_STENCIL_OP_KEEP,
    };
    pub const DESC_DEFAULT: D3D11_DEPTH_STENCIL_DESC = D3D11_DEPTH_STENCIL_DESC {
        DepthEnable: BOOL(1),
        DepthWriteMask: d3d11::D3D11_DEPTH_WRITE_MASK_ALL,
        DepthFunc: d3d11::D3D11_COMPARISON_LESS,
        StencilEnable: BOOL(0),
        StencilReadMask: d3d11::D3D11_DEFAULT_STENCIL_READ_MASK as u8,
        StencilWriteMask: d3d11::D3D11_DEFAULT_STENCIL_WRITE_MASK as u8,
        FrontFace: Self::STENCILOP_DEFAULT,
        BackFace: Self::STENCILOP_DEFAULT,
    };

    pub fn new_with_desc(
        device: &Dx11Device,
        desc: &D3D11_DEPTH_STENCIL_DESC,
    ) -> anyhow::Result<Self> {
        let mut out: Option<ID3D11DepthStencilState> = None;
        unsafe {
            device.CreateDepthStencilState(
                desc,
                Some(&mut out),
            )
        }.map_err(anyhow::Error::from)
        .and_then(move |()| out.ok_or_else(|| anyhow!("failed to produce state pointer")))
        .context("CreateDepthStencilState")
        .map(Into::into)
    }
}

impl_d3d_ext11! {
    unsafe impl ID3D11ResourceExt<Output=ID3D11DepthStencilState, @transparent> for DepthState,
        @field(&this => &this.state);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OMDepthState<S = Option<DepthState>> {
    pub state: S,
    pub stencil_ref: u32,
}

impl<S> OMDepthState<S> {
    pub fn with_state<T: Into<S>>(state: T, stencil_ref: u32) -> Self {
        Self {
            state: state.into(),
            stencil_ref,
        }
    }

    pub fn new_snapshot(context: &Dx11Context) -> Self where
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

impl<S> D3dContextBindable<Dx11Context> for OMDepthState<S> where
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

#[derive(Debug, Clone, PartialEq, Eq)]
#[repr(transparent)]
pub struct DepthView {
    pub view: ID3D11DepthStencilView,
}

impl DepthView {
    pub const CLEAR_DEPTH: D3D11_CLEAR_FLAG = d3d11::D3D11_CLEAR_DEPTH;
    pub const CLEAR_STENCIL: D3D11_CLEAR_FLAG = d3d11::D3D11_CLEAR_STENCIL;
    pub const CLEAR_DEPTH_STENCIL: D3D11_CLEAR_FLAG = D3D11_CLEAR_FLAG(Self::CLEAR_DEPTH.0 | Self::CLEAR_STENCIL.0);

    pub fn clear(&self, context: &Dx11Context, flags: D3D11_CLEAR_FLAG, depth: f32, stencil: u8) {
        unsafe {
            context.ClearDepthStencilView(
                &self.view,
                flags.0,
                depth,
                stencil,
            )
        }
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
            Anonymous: D3D11_DEPTH_STENCIL_VIEW_DESC_0 {
                Texture2D: desc,
            },
        }
    }

    pub const fn desc_for_buffer2<U: Unit<Scalar = u32>>(
        display_size: Size2<U>,
    ) -> D3D11_TEXTURE2D_DESC {
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
        Self::new_with_buffer(device, &buffer.resource, &desc)
    }

    pub fn new_with_buffer(
        device: &Dx11Device,
        buffer: &Dx11Resource,
        desc: &D3D11_DEPTH_STENCIL_VIEW_DESC,
    ) -> anyhow::Result<Self> {
        let mut out: Option<ID3D11DepthStencilView> = None;
        unsafe {
            device.CreateDepthStencilView(
                buffer,
                Some(desc),
                Some(&mut out),
            )
        }.map_err(anyhow::Error::from)
        .and_then(move |()| out.ok_or_else(|| anyhow!("failed to produce view pointer")))
        .context("CreateDepthStencilView")
        .map(Into::into)
    }

    pub fn get_resource(&self) -> anyhow::Result<Dx11Resource> {
        unsafe {
            self.view.GetResource()
        }.context("ID3D11DepthStencilView::GetResource")
    }
}

impl_d3d_ext11! {
    unsafe impl ID3D11ResourceExt<Output=ID3D11DepthStencilView,@transparent> for DepthView,
        @field(&this => &this.view);
}
