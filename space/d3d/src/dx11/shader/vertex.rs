pub use crate::dx11::d3d11::ID3D11VertexShader;
use crate::{dx11::prelude::*, state::D3dStateSnapshot, D3dContextBindable};

impl_d3d! {
    unsafe impl Dx11Child for ID3D11VertexShader;

    @[transparent(Dx11Child <= ID3D11VertexShader)]
    pub struct ShaderV.shader;
}

impl ShaderV {
    pub fn new_snapshot(context: &Dx11Context) -> Option<Self> {
        let mut out = None;
        // TODO: instances?
        let mut instances = [None; 0];
        let mut instance_count = instances.len() as u32;
        unsafe { context.VSGetShader(&mut out, Some(instances.as_mut_ptr()), Some(&mut instance_count)) }
        out.map(Into::into)
    }

    pub fn new_with_bytecode<B: AsRef<[u8]>>(device: &Dx11Device, bytecode: B) -> anyhow::Result<Self> {
        let bytecode = bytecode.as_ref();
        let mut out: Option<ID3D11VertexShader> = None;
        unsafe { device.CreateVertexShader(bytecode, None, Some(&mut out)) }
            .map_err(anyhow::Error::from)
            .and_then(move |()| out.ok_or_else(|| anyhow!("failed to produce shader pointer")))
            .context("CreateVertexShader")
            .map(Into::into)
    }
}

impl D3dContextBindable<Dx11Context> for ShaderV {
    fn set(&self, context: &Dx11Context) {
        unsafe {
            context.VSSetShader(&self.shader, None);
        }
    }
}
impl D3dContextBindable<Dx11Context> for Option<ShaderV> {
    fn set(&self, context: &Dx11Context) {
        match self {
            Some(shader) => shader.set(context),
            None => unsafe {
                context.VSSetShader(None, None);
            },
        }
    }
}

impl_d3d! {
    impl{D3DC} D3dState<D3DC> for ShaderV;
    impl{D3DC} D3dState<D3DC> for Option<ShaderV>;
}
impl D3dStateSnapshot<Dx11Context> for Option<ShaderV> {
    fn empty_state(_: &Dx11Device) -> anyhow::Result<Self> {
        Ok(None)
    }
    fn snapshot_state(context: &Dx11Context) -> Self {
        ShaderV::new_snapshot(context)
    }
}
