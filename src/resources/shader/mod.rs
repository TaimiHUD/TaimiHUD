pub mod description;
pub mod loader;
pub mod pair;

pub use {
    description::ShaderDescription,
    loader::{PixelShaders, ShaderLoader, VertexShaders},
    pair::ShaderPair,
};
