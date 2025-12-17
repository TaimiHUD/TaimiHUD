pub use self::{
    pack::{
        STATS_ENTITY_DRAW,
        STATS_ENTITY_COUNT,
        STATS_ENTITY_DRAW_MAP,
    },
    poi::{PoiCommonRenderData, STATS_POI_INSTANCE_SIZE},
    trail::STATS_TRAIL_VERTEX_SIZE,
};

mod pack;
mod poi;
mod trail;
