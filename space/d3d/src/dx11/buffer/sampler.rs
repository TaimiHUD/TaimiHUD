pub use crate::dx11::d3d11::{
    ID3D11SamplerState,
    D3D11_FILTER,
    D3D11_SAMPLER_DESC,
    D3D11_TEXTURE_ADDRESS_MODE,
};
use crate::{dx11::prelude::*, D3dContextBindable, D3dContextBindableSlot};

impl_d3d! {
    unsafe impl Dx11Child for ID3D11SamplerState;

    @[transparent(Dx11Child <= ID3D11SamplerState)]
    pub struct SamplerState.state;
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
            ..Self::DESC_DEFAULT
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
        Self { state }
    }

    pub fn new_snapshot_full(context: &Dx11Context) -> [Option<Self>; Self::MAX_COUNT] {
        Self::new_snapshot::<{ Self::MAX_COUNT }>(context, 0)
    }
    pub fn new_snapshot_vec(context: &Dx11Context, slot: ops::Range<u32>) -> Vec<Option<Self>> {
        let mut states = vec![None::<Self>; slot.len()];
        Self::new_snapshot_in(context, slot.start, &mut states[..]);
        states
    }
    pub fn new_snapshot<const N: usize /*= Self::MAX_COUNT*/>(
        context: &Dx11Context,
        slot: u32,
    ) -> [Option<Self>; N] {
        let mut states = [const { None::<Self> }; N];
        Self::new_snapshot_in(context, slot, &mut states);
        states
    }
    pub fn new_snapshot_in<'s>(context: &Dx11Context, slot: u32, out: &'s mut [Option<Self>]) {
        unsafe {
            let out: &'s mut [Option<ID3D11SamplerState>] = mem::transmute(out);
            context.PSGetSamplers(slot, Some(out));
        }
    }

    pub fn bind_set<S>(context: &Dx11Context, slot: u32, states: S)
    where
        S: ID3D11ResourceOf<ID3D11SamplerState>,
    {
        let states = states.as_params_of();
        unsafe {
            context.PSSetSamplers(slot, Some(states));
        }
    }
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

impl D3dContextBindable<Dx11Context> for [Option<SamplerState>; SamplerState::MAX_COUNT] {
    fn set(&self, context: &Dx11Context) {
        SamplerState::bind_set(context, 0, self)
    }
}

#[cfg(todo)]
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

impl_d3d! { impl enum for
    #[derive(Default)]
    pub enum TextureAddressMode: D3D11_TEXTURE_ADDRESS_MODE{i32} {
        #[default]
        const CLAMP = d3d11::D3D11_TEXTURE_ADDRESS_CLAMP;
        const WRAP = d3d11::D3D11_TEXTURE_ADDRESS_WRAP;
        const BORDER = d3d11::D3D11_TEXTURE_ADDRESS_BORDER;
        const MIRROR = d3d11::D3D11_TEXTURE_ADDRESS_MIRROR;
        const MIRROR_ONCE = d3d11::D3D11_TEXTURE_ADDRESS_MIRROR_ONCE;
    },
    #[derive(Default)]
    pub enum Filter: D3D11_FILTER{u32} {
        #[default]
        const MIN_MAG_MIP_POINT = d3d11::D3D11_FILTER_MIN_MAG_MIP_POINT;
        const MIN_MAG_POINT_MIP_LINEAR = d3d11::D3D11_FILTER_MIN_MAG_POINT_MIP_LINEAR;
        const MIN_POINT_MAG_LINEAR_MIP_POINT = d3d11::D3D11_FILTER_MIN_POINT_MAG_LINEAR_MIP_POINT;
        const MIN_POINT_MAG_MIP_LINEAR = d3d11::D3D11_FILTER_MIN_POINT_MAG_MIP_LINEAR;
        const MIN_LINEAR_MAG_MIP_POINT = d3d11::D3D11_FILTER_MIN_LINEAR_MAG_MIP_POINT;
        const MIN_LINEAR_MAG_POINT_MIP_LINEAR = d3d11::D3D11_FILTER_MIN_LINEAR_MAG_POINT_MIP_LINEAR;
        const MIN_MAG_LINEAR_MIP_POINT = d3d11::D3D11_FILTER_MIN_MAG_LINEAR_MIP_POINT;
        const MIN_MAG_MIP_LINEAR = d3d11::D3D11_FILTER_MIN_MAG_MIP_LINEAR;

        const ANISOTROPIC = d3d11::D3D11_FILTER_ANISOTROPIC;

        const COMPARISON_MIN_MAG_MIP_POINT = d3d11::D3D11_FILTER_COMPARISON_MIN_MAG_MIP_POINT;
        const COMPARISON_MIN_MAG_POINT_MIP_LINEAR = d3d11::D3D11_FILTER_COMPARISON_MIN_MAG_POINT_MIP_LINEAR;
        const COMPARISON_MIN_POINT_MAG_LINEAR_MIP_POINT = d3d11::D3D11_FILTER_COMPARISON_MIN_POINT_MAG_LINEAR_MIP_POINT;
        const COMPARISON_MIN_POINT_MAG_MIP_LINEAR = d3d11::D3D11_FILTER_COMPARISON_MIN_POINT_MAG_MIP_LINEAR;
        const COMPARISON_MIN_LINEAR_MAG_MIP_POINT = d3d11::D3D11_FILTER_COMPARISON_MIN_LINEAR_MAG_MIP_POINT;
        const COMPARISON_MIN_LINEAR_MAG_POINT_MIP_LINEAR = d3d11::D3D11_FILTER_COMPARISON_MIN_LINEAR_MAG_POINT_MIP_LINEAR;
        const COMPARISON_MIN_MAG_LINEAR_MIP_POINT = d3d11::D3D11_FILTER_COMPARISON_MIN_MAG_LINEAR_MIP_POINT;
        const COMPARISON_MIN_MAG_MIP_LINEAR = d3d11::D3D11_FILTER_COMPARISON_MIN_MAG_MIP_LINEAR;
        const COMPARISON_ANISOTROPIC = d3d11::D3D11_FILTER_COMPARISON_ANISOTROPIC;

        const MINIMUM_MIN_MAG_MIP_POINT = d3d11::D3D11_FILTER_MINIMUM_MIN_MAG_MIP_POINT;
        const MINIMUM_MIN_MAG_POINT_MIP_LINEAR = d3d11::D3D11_FILTER_MINIMUM_MIN_MAG_POINT_MIP_LINEAR;
        const MINIMUM_MIN_POINT_MAG_LINEAR_MIP_POINT = d3d11::D3D11_FILTER_MINIMUM_MIN_POINT_MAG_LINEAR_MIP_POINT;
        const MINIMUM_MIN_POINT_MAG_MIP_LINEAR = d3d11::D3D11_FILTER_MINIMUM_MIN_POINT_MAG_MIP_LINEAR;
        const MINIMUM_MIN_LINEAR_MAG_MIP_POINT = d3d11::D3D11_FILTER_MINIMUM_MIN_LINEAR_MAG_MIP_POINT;
        const MINIMUM_MIN_LINEAR_MAG_POINT_MIP_LINEAR = d3d11::D3D11_FILTER_MINIMUM_MIN_LINEAR_MAG_POINT_MIP_LINEAR;
        const MINIMUM_MIN_MAG_LINEAR_MIP_POINT = d3d11::D3D11_FILTER_MINIMUM_MIN_MAG_LINEAR_MIP_POINT;
        const MINIMUM_MIN_MAG_MIP_LINEAR = d3d11::D3D11_FILTER_MINIMUM_MIN_MAG_MIP_LINEAR;
        const MINIMUM_ANISOTROPIC = d3d11::D3D11_FILTER_MINIMUM_ANISOTROPIC;

        const MAXIMUM_MIN_MAG_MIP_POINT = d3d11::D3D11_FILTER_MAXIMUM_MIN_MAG_MIP_POINT;
        const MAXIMUM_MIN_MAG_POINT_MIP_LINEAR = d3d11::D3D11_FILTER_MAXIMUM_MIN_MAG_POINT_MIP_LINEAR;
        const MAXIMUM_MIN_POINT_MAG_LINEAR_MIP_POINT = d3d11::D3D11_FILTER_MAXIMUM_MIN_POINT_MAG_LINEAR_MIP_POINT;
        const MAXIMUM_MIN_POINT_MAG_MIP_LINEAR = d3d11::D3D11_FILTER_MAXIMUM_MIN_POINT_MAG_MIP_LINEAR;
        const MAXIMUM_MIN_LINEAR_MAG_MIP_POINT = d3d11::D3D11_FILTER_MAXIMUM_MIN_LINEAR_MAG_MIP_POINT;
        const MAXIMUM_MIN_LINEAR_MAG_POINT_MIP_LINEAR = d3d11::D3D11_FILTER_MAXIMUM_MIN_LINEAR_MAG_POINT_MIP_LINEAR;
        const MAXIMUM_MIN_MAG_LINEAR_MIP_POINT = d3d11::D3D11_FILTER_MAXIMUM_MIN_MAG_LINEAR_MIP_POINT;
        const MAXIMUM_MIN_MAG_MIP_LINEAR = d3d11::D3D11_FILTER_MAXIMUM_MIN_MAG_MIP_LINEAR;
        const MAXIMUM_ANISOTROPIC = d3d11::D3D11_FILTER_MAXIMUM_ANISOTROPIC;
    },
}

impl TextureAddressMode {
    pub const fn to_vec3(self) -> Vector3<Self> {
        Vector3::new(self.to_raw(), self.to_raw(), self.to_raw())
    }
}
