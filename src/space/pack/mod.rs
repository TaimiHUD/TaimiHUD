pub use self::{
    pack::{
        ArcrenderSettings,
        PackRender,
        PackRenderData,
        PackRenderList,
        PackRenderResources,
        PackRenderState,
        STATS_ENTITY_COUNT,
        STATS_ENTITY_DRAW,
        STATS_ENTITY_DRAW_ALL,
        STATS_ENTITY_DRAW_MAP,
        STATS_ENTITY_DRAW_PASS,
        STATS_ENTITY_INSTANCE_SIZE,
        STATS_ENTITY_INSTANCE_SIZE_MAP,
    },
    poi::{PoiCommonRenderData, PoiRender, STATS_POI_INSTANCE_SIZE},
    trail::{TrailRender, STATS_TRAIL_VERTEX_SIZE},
};

#[deprecated = "taimi_space"]
pub mod instance {
    pub use taimi_space::abi::*;
}
mod pack;
mod poi;
pub mod render;
mod trail;
