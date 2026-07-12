#[cfg(feature = "space")]
pub mod model;
#[cfg(feature = "space")]
pub mod obj_format;
#[cfg(feature = "space")]
pub mod shader;
#[cfg(feature = "texture-loader")]
pub mod texture;

#[cfg(feature = "texture-loader")]
pub use texture::Texture;
#[cfg(feature = "space")]
pub use {
    model::{Model, ModelKind},
    obj_format::{ObjFile, ObjInstance, ObjMaterial},
    shader::{PixelShaders, ShaderLoader, ShaderPair, VertexShaders},
    taimi_space::legacy::Vertex,
};
