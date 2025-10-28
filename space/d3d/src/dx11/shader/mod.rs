mod desc;
mod pixel;
mod vertex;

#[cfg(feature = "arcffi")]
pub use self::desc::InputLayoutElement;
pub use self::{
    desc::{
        ID3D11InputLayout,
        InputClassification,
        InputLayout,
        D3D11_INPUT_CLASSIFICATION,
        D3D11_INPUT_ELEMENT_DESC,
    },
    pixel::{ID3D11PixelShader, ShaderP},
    vertex::{ID3D11VertexShader, ShaderV},
};

#[cfg(feature = "serde")]
pub(crate) mod serde_imp {
    pub use super::desc::serde_imp::{input_classification, input_layout_element};
}
