pub mod loader;
pub mod material;
pub mod model;

pub use {
    loader::{ObjFile, ObjInstance},
    material::{ObjMaterial, ObjMaterials},
    model::ObjModel,
};
