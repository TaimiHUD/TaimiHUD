#[cfg(feature = "space")]
pub mod model;
#[cfg(feature = "space")]
pub mod obj_format;
#[cfg(feature = "space")]
pub mod shader;
#[cfg(feature = "texture-loader")]
pub mod texture;
#[cfg(feature = "space")]
pub mod vertex;

#[cfg(feature = "space")]
pub use {
    model::{Model, ModelKind},
    obj_format::{ObjFile, ObjInstance, ObjMaterial},
    shader::{PixelShader, PixelShaders, ShaderLoader, ShaderPair, VertexShader, VertexShaders},
    vertex::Vertex,
};
#[cfg(feature = "texture-loader")]
pub use texture::Texture;
