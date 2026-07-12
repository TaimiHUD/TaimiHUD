#[cfg(feature = "windows")]
pub mod dx;
#[cfg(feature = "windows")]
pub use self::dx::dx11;

pub mod input;

#[cfg(feature = "ui")]
pub mod ui;

pub type ScreenSpace = taimi_meta::coords::ScreenSpace;

taimi_meta::coords::coord_newtype! {
/// UV coords 0.0 to 1.0
pub struct TextureSpace([f32; 2]);
}
