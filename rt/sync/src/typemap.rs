use {
    core::{any::{Any, TypeId}, mem},
    rustc_hash::FxHashMap,
};

pub use type_map::{self, TypeMap as AnyMap, concurrent::TypeMap as AnyMapSync};

pub const fn empty_any_map() -> AnyMap {
    let empty: Option<FxHashMap<TypeId, Box<dyn Any>>> = None;
    unsafe {
        mem::transmute(empty)
    }
}
pub const fn empty_any_map_sync() -> AnyMapSync {
    let empty: Option<FxHashMap<TypeId, Box<dyn Any + Send + Sync>>> = None;
    unsafe {
        mem::transmute(empty)
    }
}
