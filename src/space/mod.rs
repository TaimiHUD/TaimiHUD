pub mod dx11;
pub mod engine;
#[cfg(feature = "goggles")]
pub mod goggles;
pub mod object;
pub mod pack;
pub mod render_list;
#[deprecated = "crate::resources"]
pub(crate) use crate::resources;

pub type DrawSpace = taimi_meta::coords::LocalSpace;
pub type ScreenSpace = taimi_meta::coords::ScreenSpace;

taimi_meta::coords::coord_newtype! {
/// UV coords 0.0 to 1.0
pub struct TextureSpace([f32; 2]);
}

pub use self::engine::Engine;
