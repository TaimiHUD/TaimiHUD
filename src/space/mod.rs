pub mod dx11;
pub mod engine;
pub mod object;
pub mod pack;
pub mod render_list;
#[deprecated = "crate::resources"]
pub(crate) use crate::resources;

pub use engine::Engine;

pub const MAX_DEPTH: f32 = 1000.0;
pub const MIN_DEPTH: f32 = 0.1;

pub const fn max_depth() -> f32 {
    MAX_DEPTH
}
pub const fn min_depth() -> f32 {
    MIN_DEPTH
}
