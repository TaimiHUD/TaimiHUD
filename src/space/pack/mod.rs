pub use self::{
    pack::{
        PackRender,
        STATS_ENTITY_DRAW,
        STATS_ENTITY_COUNT,
        STATS_ENTITY_DRAW_MAP,
    },
    poi::{PoiRender, PoiCommonRenderData, STATS_POI_INSTANCE_SIZE},
    trail::{TrailRender, STATS_TRAIL_VERTEX_SIZE},
};
use self::pack::PackRenderData;

mod pack;
mod poi;
mod trail;
