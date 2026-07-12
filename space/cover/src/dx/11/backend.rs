use {
    super::{Device, DeviceContext, SwapChain},
    crate::{dx::ShaderLoader, ScreenSpace},
    anyhow::Context,
    glamour::{Rect, Size2},
    taimi_d3d::dx11::{
        blend::{BlendFactor, BlendState, OMBlendState, D3D11_RENDER_TARGET_BLEND_DESC},
        buffer::{SamplerState, TextureAddressMode, D3D11_SAMPLER_DESC},
        prelude::*,
        viewport::Viewport,
        RenderTargetView,
    },
};

pub struct RenderBackend11 {
    pub blend_state: OMBlendState<BlendState>,

    pub shaders: ShaderLoader,
    pub sampler_state: SamplerState,
    pub device: Device,
    pub swap_chain: SwapChain,
    pub viewport: Viewport,

    pub context: Option<DeviceContext>,
    pub feature_level: d3d::D3D_FEATURE_LEVEL,
}

impl RenderBackend11 {
    pub fn new(
        swap_chain: SwapChain,
        device: Device,
        display_size: Size2<ScreenSpace>,
    ) -> anyhow::Result<Self> {
        let viewport = Viewport::with_size(display_size.extend(1.0));

        let sampler_state =
            SamplerState::new_with_desc(&device, &Self::SAMPLER_DESC).context("Sampler setup failed")?;

        let blend_desc = BlendState::desc_for_target(Self::BLEND_STATE_DESC_ALPHA_SRC, false, false);
        let blend_state =
            BlendState::new_with_desc(&device, &blend_desc).context("Blending setup failed")?;

        Ok(Self {
            blend_state: OMBlendState::new(blend_state, None, None),
            device,
            swap_chain,
            shaders: ShaderLoader::new(),
            sampler_state,
            viewport,
            context: None,
            feature_level: d3d::D3D_FEATURE_LEVEL_11_0,
            #[cfg(todo)]
            feature_level: device.get_feature_level(),
        })
    }

    pub fn setup_frame(&mut self) -> anyhow::Result<()> {
        let ctx = match &mut self.context {
            Some(ctx) => &*ctx,
            ctx @ &mut None => &*ctx.insert(self.device.get_immediate_context()?),
        };
        self.viewport.set(ctx);
        Ok(())
    }

    pub fn new_surface(&self) -> anyhow::Result<RenderTargetView> {
        let rt_bb = self.swap_chain.get_buffer::<d3d11::ID3D11Texture2D>(0)?;
        let mut rt = None;
        unsafe { self.device.CreateRenderTargetView(&rt_bb, None, Some(&mut rt)) }
            .map_err(anyhow::Error::from)
            .and_then(move |()| rt.context("NULL"))
            .context("CreateRenderTargetView")
            .map(RenderTargetView::from_d3d)
    }

    /// standard alpha blending
    ///
    /// writing to surface alpha channel is unneeded and frees it up for coverage use and things
    pub const BLEND_STATE_DESC_ALPHA_SRC: D3D11_RENDER_TARGET_BLEND_DESC = D3D11_RENDER_TARGET_BLEND_DESC {
        RenderTargetWriteMask: BlendState::WRITE_RGB.0 as _,
        ..BlendState::TARGET_DESC_ADDITIVE
    };
    /// beware overlap halos etc
    pub const BLEND_STATE_DESC_PREMUL: D3D11_RENDER_TARGET_BLEND_DESC = D3D11_RENDER_TARGET_BLEND_DESC {
        RenderTargetWriteMask: BlendState::WRITE_RGB.0 as _,
        SrcBlend: BlendFactor::ONE,
        #[cfg(todo)]
        SrcBlend: BlendFactor::BLEND_FACTOR,
        #[cfg(todo)]
        SrcBlendAlpha: BlendFactor::BLEND_FACTOR,
        ..Self::BLEND_STATE_DESC_ALPHA_SRC
    };
    /// [Self::BLEND_STATE_DESC_PREMUL] but writes alpha for use in post-compositing
    pub const BLEND_STATE_DESC_PREMUL_A: D3D11_RENDER_TARGET_BLEND_DESC = D3D11_RENDER_TARGET_BLEND_DESC {
        RenderTargetWriteMask: d3d11::D3D11_COLOR_WRITE_ENABLE_ALL.0 as _,
        ..Self::BLEND_STATE_DESC_PREMUL
    };

    const SAMPLER_DESC: D3D11_SAMPLER_DESC = D3D11_SAMPLER_DESC {
        MinLOD: 0.0,
        ComparisonFunc: d3d11::D3D11_COMPARISON_ALWAYS,
        BorderColor: [0.0; 4],
        ..SamplerState::desc_with_address(TextureAddressMode::WRAP.to_vec3())
    };

    #[inline]
    pub fn viewport_rect(&self) -> Rect<ScreenSpace> {
        let r = self.viewport.rect();
        Rect::new(r.origin.cast(), r.size.cast())
    }
    #[inline]
    pub fn display_size(&self) -> Size2<ScreenSpace> {
        self.viewport.size2().cast()
    }
}
#[cfg(todo)]
impl D3dContextBindable<Dx11Context> for RenderBackend11 {
    fn set(&self, context: &Dx11Context) {
        self.viewport.set(context);
        self.blend_state.set(context);
        self.sampler_state.set(context, 0);
    }
}
