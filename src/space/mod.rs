pub mod dx11;
pub mod engine;
pub mod object;
pub mod pack;
pub mod render_list;
#[cfg(feature = "goggles")]
pub mod goggles;
#[deprecated = "crate::resources"]
pub(crate) use crate::resources;

pub use engine::Engine;

pub const M_TO_UNIT: f32 = 3.28084 * 2.0;
pub const MAX_DEPTH: f32 = 10_000.0 / M_TO_UNIT;
pub const MIN_DEPTH: f32 = M_TO_UNIT / 10.0;

#[cfg(not(feature = "goggles"))]
pub const fn max_depth() -> f32 {
    MAX_DEPTH
}
#[cfg(not(feature = "goggles"))]
pub const fn min_depth() -> f32 {
    MIN_DEPTH
}

#[cfg(feature = "goggles")]
use std::sync::atomic::{AtomicU32, Ordering};

#[cfg(feature = "goggles")]
pub static MAX_DEPTH_: AtomicU32 = AtomicU32::new(
    u32::from_le_bytes(
        MAX_DEPTH
            .to_le_bytes()
    )
);

#[cfg(feature = "goggles")]
pub fn max_depth() -> f32 {
    f32::from_le_bytes(
        MAX_DEPTH_.load(Ordering::Relaxed)
            .to_le_bytes()
    )
}

#[cfg(feature = "goggles")]
pub fn set_max_depth(v: f32) {
    let v = u32::from_le_bytes(v.to_le_bytes());
    MAX_DEPTH_.store(v, Ordering::Relaxed)
}

#[cfg(feature = "goggles")]
pub static MIN_DEPTH_: AtomicU32 = AtomicU32::new(
    u32::from_le_bytes(
        MIN_DEPTH
            .to_le_bytes()
    )
);

#[cfg(feature = "goggles")]
pub fn min_depth() -> f32 {
    f32::from_le_bytes(
        MIN_DEPTH_.load(Ordering::Relaxed)
            .to_le_bytes()
    )
}

#[cfg(feature = "goggles")]
pub fn set_min_depth(v: f32) {
    let v = u32::from_le_bytes(v.to_le_bytes());
    MIN_DEPTH_.store(v, Ordering::Relaxed)
}
