#[cfg(todo)]
use {
    taimi_meta::{packs::MapIndex, spatial::BvhShape},
    bvh::{aabb, bvh::Bvh},
};

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
