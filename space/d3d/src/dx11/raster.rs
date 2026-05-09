pub use crate::dx11::d3d11::{
    ID3D11RasterizerState,
    ID3D11RenderTargetView,
    D3D11_CULL_MODE,
    D3D11_FILL_MODE,
    D3D11_RASTERIZER_DESC,
    D3D11_RENDER_TARGET_VIEW_DESC,
};
use crate::{
    dx11::{
        buffer::{Resource, Texture2, View},
        d3d11::ID3D11Texture2D,
        depth::{ClearFlags, DepthView},
        prelude::*,
    },
    D3dContextBindable,
};

pub fn get_swap_chain_framebuffer(swap_chain: &IDXGISwapChain) -> anyhow::Result<ID3D11Texture2D> {
    let fb = unsafe { swap_chain.GetBuffer(0) };
    fb.context("IDXGISwapChain::GetBuffer")
}

impl_d3d! {
    unsafe impl Dx11Child for ID3D11RasterizerState;

    @[transparent(Dx11Child <= ID3D11RasterizerState)]
    pub struct RasterizerState.state;
}

impl RasterizerState {
    pub fn with_state(state: ID3D11RasterizerState) -> Self {
        Self { state }
    }

    pub fn new_snapshot(context: &Dx11Context) -> anyhow::Result<Self> {
        unsafe { context.RSGetState() }
            .context("RSGetState")
            .map(Self::with_state)
    }

    pub const DESC_DEFAULT: D3D11_RASTERIZER_DESC = D3D11_RASTERIZER_DESC {
        FillMode: FillMode::SOLID,
        CullMode: CullMode::BACK,
        FrontCounterClockwise: BOOL(0),
        DepthBias: 0,
        DepthBiasClamp: 0.0,
        SlopeScaledDepthBias: 0.0,
        DepthClipEnable: BOOL(1),
        ScissorEnable: BOOL(0),
        MultisampleEnable: BOOL(0),
        AntialiasedLineEnable: BOOL(0),
    };

    pub fn new_with_desc(device: &Dx11Device, desc: &D3D11_RASTERIZER_DESC) -> anyhow::Result<Self> {
        let mut out: Option<ID3D11RasterizerState> = None;
        unsafe { device.CreateRasterizerState(desc, Some(&mut out)) }
            .map_err(anyhow::Error::from)
            .and_then(move |()| out.ok_or_else(|| anyhow!("failed to produce state pointer")))
            .context("CreateRasterizerState")
            .map(Self::with_state)
    }
}

impl D3dContextBindable<Dx11Context> for RasterizerState {
    fn set(&self, context: &Dx11Context) {
        unsafe {
            context.RSSetState(&self.state);
        }
    }
}
impl D3dContextBindable<Dx11Context> for Option<RasterizerState> {
    fn set(&self, context: &Dx11Context) {
        match self {
            Some(state) => state.set(context),
            None => unsafe {
                context.RSSetState(None);
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderTargetViews<V = [Option<RenderTargetView>; MAX_RENDER_TARGETS], D = DepthView> {
    pub views: V,
    pub depth: Option<D>,
}

impl<V, D> RenderTargetViews<V, D> {
    pub fn with_views(views: V, depth: Option<D>) -> Self {
        Self { views, depth }
    }

    pub fn new_snapshot(context: &Dx11Context) -> Self
    where
        V: Default + AsMut<[Option<RenderTargetView>]>,
        D: From<d3d11::ID3D11DepthStencilView>,
    {
        let mut views = V::default();
        let mut depth_view = None;
        unsafe {
            let views = RenderTargetView::slice_as_raw_mut(views.as_mut());
            context.OMGetRenderTargets(Some(views), Some(&mut depth_view));
        }
        Self {
            views,
            depth: depth_view.map(Into::into),
        }
    }

    pub fn to_ref(&self) -> RenderTargetViews<&V, &D> {
        RenderTargetViews::with_views(&self.views, self.depth.as_ref())
    }

    pub fn map_views<T, F: FnOnce(V) -> T>(self, f: F) -> RenderTargetViews<T, D> {
        let Self { views, depth } = self;
        RenderTargetViews::with_views(f(views), depth)
    }
    pub fn map_depth<T, F: FnOnce(Option<D>) -> Option<T>>(self, f: F) -> RenderTargetViews<V, T> {
        let Self { views, depth } = self;
        RenderTargetViews::with_views(views, f(depth))
    }
    pub fn without_depth(self) -> RenderTargetViews<V, D> {
        self.map_depth(|_| None)
    }

    pub fn views(&self) -> &[Option<ID3D11RenderTargetView>]
    where
        V: ID3D11ResourceOf<ID3D11RenderTargetView>,
    {
        self.views.as_params_of()
    }

    pub fn views_mut(&mut self) -> &mut [Option<ID3D11RenderTargetView>]
    where
        V: AsMut<[Option<RenderTargetView>]>,
    {
        RenderTargetView::slice_as_raw_mut(self.views.as_mut())
    }

    pub fn clear_colour<C: Into<Vec4>>(&self, context: &Dx11Context, colour: C)
    where
        V: ID3D11ResourceOf<ID3D11RenderTargetView>,
    {
        let colour = colour.into();
        for view in self.views().iter().flatten() {
            RenderTargetView::from_d3d_ref(view).clear_rgba(context, colour);
        }
    }

    pub fn clear_depth(&self, context: &Dx11Context, flags: ClearFlags, depth: f32, stencil: u8)
    where
        D: AsRef<DepthView>,
    {
        if let Some(depth_view) = &self.depth {
            let depth_view = depth_view.as_ref();
            depth_view.clear(context, flags, depth, stencil)
        }
    }

    pub fn bind_set(context: &Dx11Context, views: &V, depth: Option<&D>)
    where
        V: ID3D11ResourceOf<ID3D11RenderTargetView>,
        D: AsRef<DepthView>,
    {
        let views = views.as_params_of();
        let views = match views.is_empty() {
            #[cfg(todo = "unnecessary")]
            true => None,
            _ => Some(views),
        };
        let depth = depth.map(AsRef::as_ref);
        let depth = depth.as_param();
        unsafe { context.OMSetRenderTargets(views, depth.as_ref()) }
    }
}

impl<V, D> D3dContextBindable<Dx11Context> for RenderTargetViews<V, D>
where
    V: ID3D11ResourceOf<ID3D11RenderTargetView>,
    D: AsRef<DepthView>,
{
    fn set(&self, context: &Dx11Context) {
        Self::bind_set(context, &self.views, self.depth.as_ref())
    }
}

pub const MAX_RENDER_TARGETS: usize = d3d11::D3D11_SIMULTANEOUS_RENDER_TARGET_COUNT as usize;

impl_d3d! {
    unsafe impl Dx11Child for ID3D11RenderTargetView;

    @[transparent(Dx11Child <= ID3D11RenderTargetView)]
    pub struct RenderTargetView {
        pub view: View,
    }
    @into()
    @deref(View);
}

impl RenderTargetView {
    pub const MAX_RENDER_TARGETS: usize = MAX_RENDER_TARGETS;

    pub fn new_with_buffer2(device: &Dx11Device, framebuffer: &Texture2) -> anyhow::Result<Self> {
        Self::new_with_desc(device, framebuffer, None)
    }
    pub fn new_with_desc(
        device: &Dx11Device,
        resource: &Resource,
        desc: Option<&D3D11_RENDER_TARGET_VIEW_DESC>,
    ) -> anyhow::Result<Self> {
        let mut out: Option<ID3D11RenderTargetView> = None;
        let desc = desc.map(|d| d as *const _);
        unsafe { device.CreateRenderTargetView(resource, desc, Some(&mut out)) }
            .map_err(anyhow::Error::from)
            .and_then(move |()| out.ok_or_else(|| anyhow!("failed to produce view pointer")))
            .context("CreateRenderTargetView")
            .map(Into::into)
    }
    pub fn slice_as_raw_mut(views: &mut [Option<Self>]) -> &mut [Option<ID3D11RenderTargetView>] {
        unsafe { mem::transmute(views) }
    }

    pub fn get_desc(&self) -> D3D11_RENDER_TARGET_VIEW_DESC {
        let mut out = Default::default();
        unsafe {
            self.as_d3d().GetDesc(&mut out);
        }
        out
    }

    pub fn clear_rgba(&self, context: &Dx11Context, colour: Vec4) {
        let colour = colour.to_array();
        self.clear_rgba_ref(context, &colour)
    }
    #[inline]
    pub fn clear_rgba_ref(&self, context: &Dx11Context, colour: &[f32; 4]) {
        unsafe {
            context.ClearRenderTargetView(self, &colour);
        }
    }
}

impl_d3d! { impl enum for
    #[derive(Default)]
    pub enum CullMode: D3D11_CULL_MODE{i32} {
        None(const NONE) = d3d11::D3D11_CULL_NONE,
        Front(const FRONT) = d3d11::D3D11_CULL_FRONT,
        #[default]
        Back(const BACK) = d3d11::D3D11_CULL_BACK,
    },
    #[derive(Default)]
    pub enum FillMode: D3D11_FILL_MODE{i32} {
        Wireframe(const WIREFRAME) = d3d11::D3D11_FILL_WIREFRAME,
        #[default]
        Solid(const SOLID) = d3d11::D3D11_FILL_SOLID,
    },
}
