use crate::{
    dx11::{
        impl_d3d_ext11,
        prelude::*,
    },
    D3dContextBindable,
};
pub use crate::dx11::d3d11::ID3D11PixelShader;

#[derive(Debug, Clone, PartialEq, Eq)]
#[repr(transparent)]
pub struct ShaderP {
    pub shader: ID3D11PixelShader,
}

impl ShaderP {
    pub fn new_snapshot(context: &Dx11Context) -> Option<Self> {
        let mut out = None;
        // TODO: instances?
        let mut instances = [None; 0];
        let mut instance_count = instances.len() as u32;
        unsafe {
            context.PSGetShader(&mut out, Some(instances.as_mut_ptr()), Some(&mut instance_count))
        }
        out.map(Into::into)
    }

    pub fn new_with_bytecode<B: AsRef<[u8]>>(
        device: &Dx11Device,
        bytecode: B,
    ) -> anyhow::Result<Self> {
        let bytecode = bytecode.as_ref();
        let mut out: Option<ID3D11PixelShader> = None;
        unsafe {
            device.CreatePixelShader(bytecode, None, Some(&mut out))
        }.map_err(anyhow::Error::from)
        .and_then(move |()| out.ok_or_else(|| anyhow!("failed to produce shader pointer")))
        .context("CreatePixelShader")
        .map(Into::into)
    }
}

impl D3dContextBindable<Dx11Context> for ShaderP {
    fn set(&self, context: &Dx11Context) {
        unsafe {
            context.PSSetShader(&self.shader, None);
        }
    }
}

impl_d3d_ext11! {
    unsafe impl ID3D11ResourceExt<Output=ID3D11PixelShader,@transparent> for ShaderP,
        @field(&this => &this.shader);
}
