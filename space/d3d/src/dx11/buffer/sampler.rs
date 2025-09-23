use crate::{
    dx11::{
        impl_d3d_ext11,
        prelude::*,
        buffer::{
            D3D11_TEXTURE_ADDRESS_MODE,
            Filter, TextureAddressMode,
        },
    },
    state::D3dState,
    D3dContextBindableSlot,
};

pub use crate::dx11::d3d11::{
    D3D11_SAMPLER_DESC,
    ID3D11SamplerState,
};

#[derive(Debug, Clone, PartialEq, Eq)]
#[repr(transparent)]
pub struct SamplerState {
    pub state: ID3D11SamplerState,
}

impl Unit for TextureAddressMode {
    type Scalar = i32;
}

impl SamplerState {
    /// 16
    pub const MAX_COUNT: usize = d3d11::D3D11_COMMONSHADER_SAMPLER_SLOT_COUNT as usize;

    pub const DESC_DEFAULT: D3D11_SAMPLER_DESC = D3D11_SAMPLER_DESC {
        Filter: Filter::MIN_MAG_MIP_LINEAR.to_d3d(),
        AddressU: TextureAddressMode::CLAMP.to_d3d(),
        AddressV: TextureAddressMode::CLAMP.to_d3d(),
        AddressW: TextureAddressMode::CLAMP.to_d3d(),
        MinLOD: f32::MIN,
        MaxLOD: f32::MAX,
        MipLODBias: 0.0,
        MaxAnisotropy: 1,
        ComparisonFunc: d3d11::D3D11_COMPARISON_NEVER,
        BorderColor: [1.0; 4],
    };

    pub const fn desc_with_address(address: Vector3<TextureAddressMode>) -> D3D11_SAMPLER_DESC {
        D3D11_SAMPLER_DESC {
            AddressU: D3D11_TEXTURE_ADDRESS_MODE(address.x),
            AddressV: D3D11_TEXTURE_ADDRESS_MODE(address.y),
            AddressW: D3D11_TEXTURE_ADDRESS_MODE(address.z),
            .. Self::DESC_DEFAULT
        }
    }

    pub fn new_with_desc(device: &Dx11Device, desc: &D3D11_SAMPLER_DESC) -> anyhow::Result<Self> {
        let mut ptr: Option<ID3D11SamplerState> = None;
        unsafe { device.CreateSamplerState(desc, Some(&mut ptr)) }
            .map_err(anyhow::Error::from)
            .and_then(move |()| ptr.ok_or_else(|| anyhow!("failed to produce pointer")))
            .map(Self::with_state)
            .context("Dx11::CreateSamplerState")
    }

    pub fn with_state(state: ID3D11SamplerState) -> Self {
        Self {
            state,
        }
    }

    pub fn new_snapshot(context: &Dx11Context) -> [Option<Self>; Self::MAX_COUNT] {
        Self::new_snapshot_from::<{Self::MAX_COUNT}>(context, 0)
    }

    pub fn new_snapshot_from<const N: usize /*= Self::MAX_COUNT*/>(context: &Dx11Context, slot: u32) -> [Option<Self>; N] {
        let mut states = [const { None::<Self> }; N];
        let count = (states.len()).saturating_sub(slot as usize);
        unsafe {
            let states: &mut [Option<Self>] = states.get_unchecked_mut(..count);
            let states: &mut [Option<ID3D11SamplerState>] = mem::transmute(states);
            context.PSGetSamplers(slot, Some(states));
        }
        states
    }

    pub fn bind_set<S>(context: &Dx11Context, slot: u32, states: S) where
        S: ID3D11ResourceOf<ID3D11SamplerState>,
    {
        let states = states.as_params_of();
        unsafe {
            context.PSSetSamplers(slot, Some(states));
        }
    }
}

impl_d3d_ext11! {
    unsafe impl ID3D11ResourceExt<Output=ID3D11SamplerState,@transparent> for SamplerState,
        @field(&this => &this.state);
}

impl D3dContextBindableSlot<Dx11Context> for SamplerState {
    fn set(&self, context: &Dx11Context, slot: u32) {
        SamplerState::bind_set(context, slot, self)
    }
}

impl D3dContextBindableSlot<Dx11Context> for Option<SamplerState> {
    fn set(&self, context: &Dx11Context, slot: u32) {
        SamplerState::bind_set(context, slot, self)
    }
}

impl D3dContextBindableSlot<Dx11Context> for [Option<SamplerState>] {
    fn set(&self, context: &Dx11Context, slot: u32) {
        SamplerState::bind_set(context, slot, self)
    }
}

impl<const N: usize> D3dState<Dx11Context> for [Option<SamplerState>; N] {
    fn empty_state(_: &Dx11Device) -> anyhow::Result<Self> {
        Ok([const { None }; N])
    }

    fn snapshot_state(context: &Dx11Context) -> Self {
        SamplerState::new_snapshot_from::<N>(context, 0)
    }

    fn restore_state(&self, context: &Dx11Context) {
        self.set(context, 0);
    }

    fn discard_state_mut(&mut self) {
        *self = [const { None }; N];
    }
}
