use crate::controller::pathing::registry::{LoadedMarkerPath, LoadedTrailPath};

#[cfg(todo)]
use {
    taimi_meta::{packs::MapIndex, spatial::BvhShape},
    bvh::{aabb, bvh::Bvh},
};
use taimi_sync::watched::watch;
use super::{SpacePackCollection, TrailGeometryRequests, TrailGeometryRequestsTx, TextureLoadRequests, TextureLoadRequestsTx};
use std::{fmt, sync::Arc, collections::BTreeSet};
use taimi_meta::packs::{id::{MarkerId, MarkerIndex}, PackMapPath};

#[derive(Clone)]
pub struct SpacePackShared {
    #[cfg(todo)]
    pub map_id: Option<MapIndex>,
    #[cfg(todo)]
    pub shapes: BvhShape<aabb::Aabb<f32, 3>>,
    pub collection: watch::Sender<Arc<SpacePackCollection>>,
    pub trail_geometry: TrailGeometryRequestsTx,
    pub texture_loads: TextureLoadRequestsTx,
}
impl SpacePackShared {
    pub fn new() -> Self {
        Self {
            collection: Default::default(),
            trail_geometry: TrailGeometryRequests::new_sender(),
            texture_loads: TextureLoadRequests::new_sender(),
        }
    }
    pub fn trail_geometry_id(path: &LoadedTrailPath<PackMapPath>) -> MarkerId {
        let path = path.map_path(|path| MarkerIndex::with_trail_section(path, 0));
        MarkerId::for_marker(path)
    }
}
impl Default for SpacePackShared {
    fn default() -> Self {
        Self::new()
    }
}
impl fmt::Debug for SpacePackShared {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("SpacePackShared").finish()
    }
}
