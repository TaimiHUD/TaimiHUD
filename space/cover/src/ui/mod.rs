pub use taimi_ui::im;

#[cfg(feature = "imgui180")]
#[path = "180/mod.rs"]
pub mod imp180;

#[cfg(feature = "imgui180")]
pub use self::imp180::{imui as im180, ImRenderer as ImRenderer180};

#[cfg(feature = "imgui192")]
#[path = "192/mod.rs"]
pub mod imp192;

#[cfg(feature = "imgui192")]
pub use self::imp192::{imui as im192, ImRenderer as ImRenderer192};

pub type ImDrawVtx = [f32; 5];
pub type ImDrawIdx = u16;
