pub mod abi;
pub mod legacy;

pub type DrawSpace = taimi_meta::coords::LocalSpace;
pub type ScreenSpace = taimi_meta::coords::ScreenSpace;

taimi_meta::coords::coord_newtype! {
/// UV coords 0.0 to 1.0
pub struct TextureSpace([f32; 2]);
}
