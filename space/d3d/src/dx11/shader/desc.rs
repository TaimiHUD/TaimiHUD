use {
    crate::{
        dx11::{
            impl_d3d_ext11,
            prelude::*,
        },
        D3dContextBindable,
    },
    std::ffi::CStr,
};
pub use crate::dx11::d3d11::{
    D3D11_INPUT_CLASSIFICATION, D3D11_INPUT_ELEMENT_DESC,
    ID3D11InputLayout,
};

#[derive(Debug, Clone, PartialEq, Eq)]
#[repr(transparent)]
pub struct InputLayout {
    pub layout: ID3D11InputLayout,
}

impl InputLayout {
    pub fn new_snapshot(context: &Dx11Context) -> anyhow::Result<Self> {
        unsafe {
            context.IAGetInputLayout()
        }.map(Into::into)
        .context("IAGetInputLayout")
    }

    pub fn new_with_desc<B: AsRef<[u8]>>(
        device: &Dx11Device,
        desc: &[D3D11_INPUT_ELEMENT_DESC],
        bytecode: B,
    ) -> anyhow::Result<Self> {
        let bytecode = bytecode.as_ref();
        let mut out: Option<ID3D11InputLayout> = None;
        unsafe {
            device.CreateInputLayout(desc, bytecode, Some(&mut out))
        }.map_err(anyhow::Error::from)
        .and_then(move |()| out.ok_or_else(|| anyhow!("failed to produce input layout pointer")))
        .context("CreateInputLayout")
        .map(Into::into)
    }

    pub const INPUT_PER_INSTANCE: D3D11_INPUT_CLASSIFICATION = d3d11::D3D11_INPUT_PER_INSTANCE_DATA;
    pub const INPUT_PER_VERTEX: D3D11_INPUT_CLASSIFICATION = d3d11::D3D11_INPUT_PER_VERTEX_DATA;
    pub const OFFSET_ALIGNED: u32 = d3d11::D3D11_APPEND_ALIGNED_ELEMENT;

    pub const fn offset_or_aligned(offset: Option<usize>) -> u32 {
        match offset {
            Some(offset) => offset as u32,
            None => Self::OFFSET_ALIGNED,
        }
    }

    pub const fn for_instance(slot: u32, name: &CStr, index: u32, format: dxgi::DXGI_FORMAT, offset: Option<usize>) -> D3D11_INPUT_ELEMENT_DESC {
        D3D11_INPUT_ELEMENT_DESC {
            InstanceDataStepRate: 1,
            InputSlotClass: Self::INPUT_PER_INSTANCE,
            InputSlot: slot,
            SemanticName: PCSTR(name.as_ptr() as *const _),
            SemanticIndex: index,
            Format: format,
            AlignedByteOffset: Self::offset_or_aligned(offset),
        }
    }

    pub const fn for_vertex(slot: u32, name: &CStr, index: u32, format: dxgi::DXGI_FORMAT, offset: Option<usize>) -> D3D11_INPUT_ELEMENT_DESC {
        D3D11_INPUT_ELEMENT_DESC {
            InstanceDataStepRate: 0,
            InputSlotClass: Self::INPUT_PER_VERTEX,
            InputSlot: slot,
            SemanticName: PCSTR(name.as_ptr() as *const _),
            SemanticIndex: index,
            Format: format,
            AlignedByteOffset: Self::offset_or_aligned(offset),
        }
    }
}

impl D3dContextBindable<Dx11Context> for InputLayout {
    fn set(&self, context: &Dx11Context) {
        unsafe {
            context.IASetInputLayout(&self.layout);
        }
    }
}

impl_d3d_ext11! {
    unsafe impl ID3D11ResourceExt<Output=ID3D11InputLayout,@transparent> for InputLayout,
        @field(&this => &this.layout);
}
