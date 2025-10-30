use {
    crate::{dx11::prelude::*, D3dContextBindable},
    std::ffi::CStr,
};

pub use crate::dx11::d3d11::{ID3D11InputLayout, D3D11_INPUT_CLASSIFICATION, D3D11_INPUT_ELEMENT_DESC};

impl_d3d! {
    unsafe impl Dx11Child for ID3D11InputLayout;

    @[transparent(Dx11Child <= ID3D11InputLayout)]
    pub struct InputLayout.layout;
}

impl_d3d! { impl enum for
    #[derive(Default)]
    pub enum InputClassification: D3D11_INPUT_CLASSIFICATION{i32} {
        #[default]
        Vertex(const PER_VERTEX) = d3d11::D3D11_INPUT_PER_VERTEX_DATA,
        Instance(const PER_INSTANCE) = d3d11::D3D11_INPUT_PER_INSTANCE_DATA,
    }
}

impl InputLayout {
    pub fn new_snapshot(context: &Dx11Context) -> anyhow::Result<Self> {
        unsafe { context.IAGetInputLayout() }
            .map(Into::into)
            .context("IAGetInputLayout")
    }

    pub fn new_with_desc<B: AsRef<[u8]>>(
        device: &Dx11Device,
        desc: impl AsRef<[D3D11_INPUT_ELEMENT_DESC]>,
        bytecode: B,
    ) -> anyhow::Result<Self> {
        let bytecode = bytecode.as_ref();
        let desc = desc.as_ref();
        let mut out: Option<ID3D11InputLayout> = None;
        unsafe { device.CreateInputLayout(desc, bytecode, Some(&mut out)) }
            .map_err(anyhow::Error::from)
            .and_then(move |()| out.ok_or_else(|| anyhow!("failed to produce input layout pointer")))
            .context("CreateInputLayout")
            .map(Into::into)
    }

    pub const OFFSET_ALIGNED: u32 = d3d11::D3D11_APPEND_ALIGNED_ELEMENT;

    pub const fn offset_or_aligned(offset: Option<usize>) -> u32 {
        match offset {
            Some(offset) => offset as u32,
            None => Self::OFFSET_ALIGNED,
        }
    }

    pub const fn for_instance(
        slot: u32,
        name: &CStr,
        index: u32,
        format: dxgi::DXGI_FORMAT,
        offset: Option<usize>,
    ) -> D3D11_INPUT_ELEMENT_DESC {
        D3D11_INPUT_ELEMENT_DESC {
            InstanceDataStepRate: 1,
            InputSlotClass: InputClassification::PER_INSTANCE,
            InputSlot: slot,
            SemanticName: PCSTR(name.as_ptr() as *const _),
            SemanticIndex: index,
            Format: format,
            AlignedByteOffset: Self::offset_or_aligned(offset),
        }
    }

    pub const fn for_vertex(
        slot: u32,
        name: &CStr,
        index: u32,
        format: dxgi::DXGI_FORMAT,
        offset: Option<usize>,
    ) -> D3D11_INPUT_ELEMENT_DESC {
        D3D11_INPUT_ELEMENT_DESC {
            InstanceDataStepRate: 0,
            InputSlotClass: InputClassification::PER_VERTEX,
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

#[cfg(feature = "arcffi")]
#[derive(Debug, Clone, PartialEq, Eq)]
#[repr(C)]
pub struct InputLayoutElement {
    pub semantic_name: cstr::CStrBox,
    pub semantic_index: u32,
    pub format: dxgi::DXGI_FORMAT,
    pub input_slot: u32,
    pub aligned_byte_offset: u32,
    pub class: D3D11_INPUT_CLASSIFICATION,
    pub instance_step: u32,
}

#[cfg(feature = "arcffi")]
impl InputLayoutElement {
    pub fn byte_offset(&self) -> Option<usize> {
        match self.aligned_byte_offset {
            InputLayout::OFFSET_ALIGNED => None,
            offset => Some(offset as _),
        }
    }

    pub fn format(&self) -> DxgiFormat {
        DxgiFormat::try_from(self.format).unwrap_or(DxgiFormat::Unknown)
    }

    pub fn class(&self) -> InputClassification {
        InputClassification::try_from(self.class).unwrap_or(InputClassification::Vertex)
    }

    pub fn slice_as_desc(inputs: &[Self]) -> &[D3D11_INPUT_ELEMENT_DESC] {
        unsafe { mem::transmute(inputs) }
    }
}

#[cfg(feature = "arcffi")]
impl_d3d! {
    unsafe impl AsD3d<Interface = D3D11_INPUT_ELEMENT_DESC, @transparent> for InputLayoutElement;
}

#[cfg(feature = "serde")]
pub(crate) mod serde_imp {
    use {
        super::{InputClassification, D3D11_INPUT_CLASSIFICATION},
        crate::{dxgi, DxgiFormat},
    };

    pub(crate) fn default_dxgi() -> dxgi::DXGI_FORMAT {
        DxgiFormat::R32G32B32A32_FLOAT
    }
    pub(crate) fn is_default_dxgi(f: &dxgi::DXGI_FORMAT) -> bool {
        *f == DxgiFormat::R32G32B32A32_FLOAT
    }

    pub(crate) fn is_zero(v: &u32) -> bool {
        *v == 0
    }

    pub(crate) fn default_instance_step(class: D3D11_INPUT_CLASSIFICATION) -> u32 {
        match class {
            InputClassification::PER_INSTANCE => 1,
            InputClassification::PER_VERTEX | _ => 0,
        }
    }

    pub mod input_classification {
        use {
            super::super::{InputClassification, D3D11_INPUT_CLASSIFICATION},
            serde::{Deserialize, Deserializer, Serialize, Serializer},
        };

        pub fn serialize<S: Serializer>(
            class: &D3D11_INPUT_CLASSIFICATION,
            serializer: S,
        ) -> Result<S::Ok, S::Error> {
            match InputClassification::try_from_d3d(*class) {
                Ok(class) => class.serialize(serializer),
                Err(..) => class.0.serialize(serializer),
            }
        }
        pub fn deserialize<'de, D: Deserializer<'de>>(
            deserializer: D,
        ) -> Result<D3D11_INPUT_CLASSIFICATION, D::Error> {
            #[derive(Deserialize)]
            #[serde(untagged)]
            enum InputClassificationDe {
                Class(InputClassification),
                Raw(i32),
            }
            InputClassificationDe::deserialize(deserializer).map(|c| match c {
                InputClassificationDe::Class(class) => class.to_d3d(),
                InputClassificationDe::Raw(class) => D3D11_INPUT_CLASSIFICATION(class),
            })
        }

        pub fn is_default_d3d(c: &D3D11_INPUT_CLASSIFICATION) -> bool {
            *c == InputClassification::PER_VERTEX
        }
    }

    pub mod input_layout_element {
        use {
            super::super::{
                InputLayout,
                InputLayoutElement as InputLayoutElementDesc,
                D3D11_INPUT_CLASSIFICATION,
            },
            crate::prelude::*,
            arcffi::cstr::{CStrBox, CStrRef},
            serde::{Deserialize, Deserializer, Serialize, Serializer},
            std::borrow::Cow,
        };

        #[derive(Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        #[serde(bound(deserialize = "'de: 'c"))]
        struct InputLayoutElement<'c> {
            #[serde(
                rename = "name",
                default = "crate::macros::serde::cstr_box::cow::empty",
                skip_serializing_if = "crate::macros::serde::cstr_box::cow::is_empty",
                with = "crate::macros::serde::cstr_box::cow"
            )]
            semantic_name: Cow<'c, CStrRef>,
            #[serde(rename = "index", default, skip_serializing_if = "super::is_zero")]
            semantic_index: u32,
            #[serde(
                default = "super::default_dxgi",
                skip_serializing_if = "super::is_default_dxgi",
                with = "crate::macros::serde::dxgi_format"
            )]
            format: dxgi::DXGI_FORMAT,
            #[serde(rename = "slot", default, skip_serializing_if = "super::is_zero")]
            input_slot: u32,
            #[serde(rename = "offset", default, skip_serializing_if = "Option::is_none")]
            aligned_byte_offset: Option<u32>,
            #[serde(
                default,
                skip_serializing_if = "super::input_classification::is_default_d3d",
                with = "crate::macros::serde::dx11::input_classification"
            )]
            class: D3D11_INPUT_CLASSIFICATION,
            #[serde(rename = "step", default, skip_serializing_if = "Option::is_none")]
            instance_step: Option<u32>,
        }

        impl Serialize for InputLayoutElementDesc {
            fn serialize<S: serde::Serializer>(
                &self,
                serializer: S,
            ) -> ::core::result::Result<S::Ok, S::Error> {
                self::serialize(self, serializer)
            }
        }
        impl<'de> serde::Deserialize<'de> for InputLayoutElementDesc {
            fn deserialize<D: serde::Deserializer<'de>>(
                deserializer: D,
            ) -> ::core::result::Result<Self, D::Error> {
                self::deserialize(deserializer)
            }
        }

        #[inline]
        pub fn serialize<S: Serializer>(
            v: &InputLayoutElementDesc,
            serializer: S,
        ) -> Result<S::Ok, S::Error> {
            self::InputLayoutElement::serialize(&v.into(), serializer)
        }
        #[inline]
        pub fn deserialize<'de, D: Deserializer<'de>>(
            deserializer: D,
        ) -> Result<InputLayoutElementDesc, D::Error> {
            self::InputLayoutElement::deserialize(deserializer).map(Into::into)
        }

        impl From<InputLayoutElement<'_>> for InputLayoutElementDesc {
            fn from(input: InputLayoutElement) -> Self {
                InputLayoutElementDesc {
                    semantic_name: CStrBox::with_cstring(input.semantic_name.into_owned()),
                    semantic_index: input.semantic_index,
                    format: input.format,
                    input_slot: input.input_slot,
                    aligned_byte_offset: input.aligned_byte_offset.unwrap_or(InputLayout::OFFSET_ALIGNED),
                    instance_step: input
                        .instance_step
                        .unwrap_or(super::default_instance_step(input.class)),
                    class: input.class,
                }
            }
        }
        impl<'s> From<&'s InputLayoutElementDesc> for InputLayoutElement<'s> {
            fn from(input: &'s InputLayoutElementDesc) -> Self {
                InputLayoutElement {
                    semantic_name: Cow::Borrowed(input.semantic_name.as_c_ref()),
                    semantic_index: input.semantic_index,
                    format: input.format,
                    input_slot: input.input_slot,
                    aligned_byte_offset: match input.aligned_byte_offset {
                        InputLayout::OFFSET_ALIGNED => None,
                        o => Some(o),
                    },
                    instance_step: match input.instance_step {
                        step if step == super::default_instance_step(input.class) => None,
                        step => Some(step),
                    },
                    class: input.class,
                }
            }
        }
    }
}
