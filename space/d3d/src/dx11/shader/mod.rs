mod compute;
mod desc;
mod domain;
mod geometry;
mod hull;
mod pixel;
mod vertex;

#[cfg(feature = "arcffi")]
pub use self::desc::InputLayoutElement;
pub use self::{
    compute::{ID3D11ComputeShader, ShaderC},
    desc::{
        ID3D11InputLayout,
        InputClassification,
        InputLayout,
        D3D11_INPUT_CLASSIFICATION,
        D3D11_INPUT_ELEMENT_DESC,
    },
    domain::{ID3D11DomainShader, ShaderD},
    geometry::{ID3D11GeometryShader, ShaderG},
    hull::{ID3D11HullShader, ShaderH},
    pixel::{ID3D11PixelShader, ShaderP},
    vertex::{ID3D11VertexShader, ShaderV},
};

#[cfg(feature = "serde")]
pub(crate) mod serde_imp {
    pub use super::desc::serde_imp::{input_classification, input_layout_element};
}
