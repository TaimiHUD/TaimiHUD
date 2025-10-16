mod desc;
mod pixel;
mod vertex;

pub use self::{
    desc::{
        D3D11_INPUT_CLASSIFICATION, D3D11_INPUT_ELEMENT_DESC,
        ID3D11InputLayout,
        InputClassification,
        InputLayout,
    },
    pixel::{ShaderP, ID3D11PixelShader},
    vertex::{ShaderV, ID3D11VertexShader},
};
#[cfg(feature = "arcffi")]
pub use self::desc::InputLayoutElement;

#[cfg(feature = "serde")]
pub(crate) mod serde_imp {
    pub use super::desc::serde_imp::{
        input_classification,
        input_layout_element,
    };
}
