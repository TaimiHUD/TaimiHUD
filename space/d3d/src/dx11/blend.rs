pub use crate::dx11::d3d11::{
    ID3D11BlendState,
    D3D11_BLEND,
    D3D11_BLEND_DESC,
    D3D11_BLEND_OP,
    D3D11_RENDER_TARGET_BLEND_DESC,
};
use crate::{dx11::prelude::*, state::D3dState, D3dContextBindable};

impl_d3d! {
    unsafe impl Dx11Child for ID3D11BlendState;

    @[transparent(Dx11Child <= ID3D11BlendState)]
    pub struct BlendState.state;
}

impl BlendState {
    pub const DEFAULT_FACTOR: Vec4 = Vec4::ONE;
    pub const DEFAULT_MASK: u32 = u32::MAX;

    pub const TARGET_DESC_DEFAULT_OFF: D3D11_RENDER_TARGET_BLEND_DESC = D3D11_RENDER_TARGET_BLEND_DESC {
        BlendEnable: BOOL(0),
        SrcBlend: BlendFactor::ONE,
        DestBlend: BlendFactor::ZERO,
        BlendOp: BlendOp::ADD,
        SrcBlendAlpha: BlendFactor::ONE,
        DestBlendAlpha: BlendFactor::ZERO,
        BlendOpAlpha: BlendOp::ADD,
        RenderTargetWriteMask: d3d11::D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
    };
    pub const TARGET_DESC_ADDITIVE: D3D11_RENDER_TARGET_BLEND_DESC = D3D11_RENDER_TARGET_BLEND_DESC {
        BlendEnable: BOOL(1),
        SrcBlend: BlendFactor::SRC_ALPHA,
        DestBlend: BlendFactor::INV_SRC_ALPHA,
        ..Self::TARGET_DESC_DEFAULT_OFF
    };

    pub const fn desc_for_target(
        rt_desc: D3D11_RENDER_TARGET_BLEND_DESC,
        alpha_to_coverage: bool,
        independent_blend: bool,
    ) -> D3D11_BLEND_DESC {
        D3D11_BLEND_DESC {
            AlphaToCoverageEnable: BOOL(match alpha_to_coverage {
                true => 1,
                false => 0,
            }),
            IndependentBlendEnable: BOOL(match independent_blend {
                true => 1,
                false => 0,
            }),
            RenderTarget: [rt_desc; 8],
        }
    }

    pub fn new_with_desc(device: &Dx11Device, desc: &D3D11_BLEND_DESC) -> anyhow::Result<Self> {
        let mut ptr: Option<ID3D11BlendState> = None;
        unsafe { device.CreateBlendState(desc, Some(&mut ptr)) }
            .map_err(anyhow::Error::from)
            .and_then(move |()| ptr.ok_or_else(|| anyhow!("failed to produce pointer")))
            .map(Into::into)
            .context("Dx11::CreateBlendState")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OMBlendState<B = BlendState> {
    pub state: B,
    pub factor: Option<Vec4>,
    pub sample_mask: Option<u32>,
}

impl<B> OMBlendState<B> {
    pub fn with_state<S: Into<B>>(state: S) -> Self {
        Self::new(state.into(), None, None)
    }

    pub const fn new(state: B, factor: Option<Vec4>, sample_mask: Option<u32>) -> Self {
        Self {
            state,
            factor,
            sample_mask: Some(match sample_mask {
                Some(m) => m,
                None => BlendState::DEFAULT_MASK,
            }),
        }
    }

    pub fn new_snapshot(context: &Dx11Context) -> Self
    where
        B: From<Option<ID3D11BlendState>>,
    {
        let mut state = None;
        let mut factor = BlendState::DEFAULT_FACTOR.to_array();
        let mut sample_mask = BlendState::DEFAULT_MASK;
        unsafe { context.OMGetBlendState(Some(&mut state), Some(&mut factor), Some(&mut sample_mask)) }
        let factor = Vec4::from_array(factor);
        //let factor = (factor != Self::DEFAULT_FACTOR).then(factor);
        Self::new(state.into(), Some(factor), Some(sample_mask))
    }

    pub fn factor(&self) -> Option<&[f32; 4]> {
        self.factor
            .as_ref()
            .map(|f| unsafe { &*(&raw const f.x as *const [f32; 4]) })
    }

    pub fn clear(&mut self)
    where
        B: Default,
    {
        self.sample_mask.take();
        mem::take(&mut self.state);
    }
}

impl<B> D3dContextBindable<Dx11Context> for OMBlendState<B>
where
    //B: ID3D11ResourceExt<Output = ID3D11BlendState>,
    B: D3dInterfacePtr<Interface = ID3D11BlendState>,
{
    fn set(&self, device_context: &Dx11Context) {
        if let Some(sample_mask) = self.sample_mask {
            let state = self.state.as_d3d_param(); // .as_param()?
            unsafe {
                device_context.OMSetBlendState(state.as_ref(), self.factor(), sample_mask);
            }
        }
    }
}

impl<B> D3dState<Dx11Context> for OMBlendState<B>
where
    B: From<Option<ID3D11BlendState>>,
    Self: D3dContextBindable<Dx11Context>,
{
    fn empty_state(_: &Dx11Device) -> anyhow::Result<Self> {
        Ok(Self::with_state(None))
    }

    fn snapshot_state(context: &Dx11Context) -> Self {
        Self::new_snapshot(context)
    }

    fn restore_state(&self, context: &Dx11Context) {
        self.set(context);
    }

    fn discard_state_mut(&mut self) {
        self.sample_mask.take();
        self.state = None.into();
    }
}

impl<B> From<B> for OMBlendState<B> {
    fn from(state: B) -> Self {
        Self::with_state(state)
    }
}

impl_d3d! { impl enum for
    #[derive(Default)]
    pub enum BlendOp: D3D11_BLEND_OP{i32} {
        #[default]
        Add(const ADD) = d3d11::D3D11_BLEND_OP_ADD,
        Sub(const SUBTRACT) = d3d11::D3D11_BLEND_OP_SUBTRACT,
        ReverseSub(const REV_SUBTRACT) = d3d11::D3D11_BLEND_OP_REV_SUBTRACT,
        Min(const MIN) = d3d11::D3D11_BLEND_OP_MIN,
        Max(const MAX) = d3d11::D3D11_BLEND_OP_MAX,
    },
    #[derive(Default)]
    pub enum BlendFactor: D3D11_BLEND{i32} {
        #[default]
        Zero(const ZERO) = d3d11::D3D11_BLEND_ZERO,
        One(const ONE) = d3d11::D3D11_BLEND_ONE,
        Colour(const SRC_COLOR) = d3d11::D3D11_BLEND_SRC_COLOR,
        ColourInv(const INV_SRC_COLOR) = d3d11::D3D11_BLEND_INV_SRC_COLOR,
        Alpha(const SRC_ALPHA) = d3d11::D3D11_BLEND_SRC_ALPHA,
        AlphaInv(const INV_SRC_ALPHA) = d3d11::D3D11_BLEND_INV_SRC_ALPHA,
        AlphaDest(const DEST_ALPHA) = d3d11::D3D11_BLEND_DEST_ALPHA,
        AlphaDestInv(const INV_DEST_ALPHA) = d3d11::D3D11_BLEND_INV_DEST_ALPHA,
        ColourDest(const DEST_COLOR) = d3d11::D3D11_BLEND_DEST_COLOR,
        ColourDestInv(const INV_DEST_COLOR) = d3d11::D3D11_BLEND_INV_DEST_COLOR,
        AlphaSaturated(const SRC_ALPHA_SAT) = d3d11::D3D11_BLEND_SRC_ALPHA_SAT,
        StateFactor(const BLEND_FACTOR) = d3d11::D3D11_BLEND_BLEND_FACTOR,
        StateFactorInv(const INV_BLEND_FACTOR) = d3d11::D3D11_BLEND_INV_BLEND_FACTOR,
        Colour1(const SRC1_COLOR) = d3d11::D3D11_BLEND_SRC1_COLOR,
        Colour1Inv(const INV_SRC1_COLOR) = d3d11::D3D11_BLEND_INV_SRC1_COLOR,
        Alpha1(const SRC1_ALPHA) = d3d11::D3D11_BLEND_SRC1_ALPHA,
        Alpha1Inv(const INV_SRC1_ALPHA) = d3d11::D3D11_BLEND_INV_SRC1_ALPHA,
    },
}
