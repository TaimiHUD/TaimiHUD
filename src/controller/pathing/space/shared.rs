#[cfg(todo)]
use {
    taimi_meta::{packs::MapIndex, spatial::BvhShape},
    bvh::{aabb, bvh::Bvh},
};
use std::fmt;

#[derive(Clone)]
pub struct SpacePackShared {
    #[cfg(todo)]
    pub map_id: Option<MapIndex>,
    #[cfg(todo)]
    pub shapes: BvhShape<aabb::Aabb<f32, 3>>,
    pub collection: super::SpacePackCollection,
}
impl Default for SpacePackShared {
    fn default() -> Self {
        Self {
            collection: super::SpacePackCollection::new(),
        }
    }
}
impl fmt::Debug for SpacePackShared {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("SpacePackShared").finish()
    }
}
