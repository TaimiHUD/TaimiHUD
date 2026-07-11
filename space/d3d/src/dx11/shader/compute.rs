pub use crate::dx11::d3d11::ID3D11ComputeShader;
use crate::{dx11::prelude::*, state::D3dStateSnapshot, D3dContextBindable};

impl_d3d! {
    unsafe impl Dx11Child for ID3D11ComputeShader;

    @[transparent(Dx11Child <= ID3D11ComputeShader)]
    pub struct ShaderC.shader;
}

impl ShaderC {
    pub fn new_snapshot(context: &Dx11Context) -> Option<Self> {
        let mut out = None;
        // TODO: instances?
        let mut instances = [None; 0];
        let mut instance_count = instances.len() as u32;
        unsafe { context.CSGetShader(&mut out, Some(instances.as_mut_ptr()), Some(&mut instance_count)) }
        out.map(Into::into)
    }

    pub fn new_with_bytecode<B: AsRef<[u8]>>(device: &Dx11Device, bytecode: B) -> anyhow::Result<Self> {
        let bytecode = bytecode.as_ref();
        let mut out: Option<ID3D11ComputeShader> = None;
        unsafe { device.CreateComputeShader(bytecode, None, Some(&mut out)) }
            .map_err(anyhow::Error::from)
            .and_then(move |()| out.ok_or_else(|| anyhow!("failed to produce shader pointer")))
            .context("CreateComputeShader")
            .map(Into::into)
    }
}

impl D3dContextBindable<Dx11Context> for ShaderC {
    fn set(&self, context: &Dx11Context) {
        unsafe {
            context.CSSetShader(&self.shader, None);
        }
    }
}
impl D3dContextBindable<Dx11Context> for Option<ShaderC> {
    fn set(&self, context: &Dx11Context) {
        match self {
            Some(shader) => shader.set(context),
            None => unsafe {
                context.CSSetShader(None, None);
            },
        }
    }
}

impl_d3d! {
    impl{D3DC} D3dState<D3DC> for ShaderC;
    impl{D3DC} D3dState<D3DC> for Option<ShaderC>;
}
impl D3dStateSnapshot<Dx11Context> for Option<ShaderC> {
    fn empty_state(_: &Dx11Device) -> anyhow::Result<Self> {
        Ok(None)
    }
    fn snapshot_state(context: &Dx11Context) -> Self {
        ShaderC::new_snapshot(context)
    }
}
