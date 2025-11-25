use {
    self::{
        state::{
            MapPackInfoStorage,
            info::MapPackInfo,
        },
        registry::{PackMapPath, PackLoader, PackRegistry},
        visible::LoadedMapPack,
        filter::FilterState,
    },
    crate::space::pack::trail::TrailParams,
    std::{collections::BTreeMap, sync::Arc},
    tokio::sync::RwLock,
};
pub use self::{
    state::{
        shared::{SharedMapPackInfo, SharedMapPackState},
        festival::FestivalState,
    },
    setup::{PathingEvent, PathingEventContext},
};

pub mod registry;
pub mod festivals;
pub mod visible;
pub mod filter;
pub mod state;
pub mod setup;

#[derive(Debug)]
pub struct PathingController {
    loader: Arc<PackLoader>,

    pub enabled: bool,
    pub map_pack_info: BTreeMap<PackMapPath, MapPackInfoStorage>,
    pub map_packs: BTreeMap<PackMapPath, LoadedMapPack>,
    pub filter_state: FilterState,
}

impl PathingController {
    pub fn new(loader: Arc<PackLoader>) -> Self {
        Self {
            loader,
            enabled: false,
            map_pack_info: Default::default(),
            map_packs: Default::default(),
            filter_state: Default::default(),
        }
    }

    pub fn packs() -> &'static RwLock<PackRegistry> {
        static PACKS: RwLock<PackRegistry> = RwLock::const_new(PackRegistry::new());
        &PACKS
    }

    pub async fn trail_params(&self) -> TrailParams {
        let settings = self.loader.settings.read().await;
        let pathing = settings.pathing();
        let mut params = TrailParams::DEFAULT;
        params.y_offset = pathing.space.trail_y_offset().unwrap_or(0.0);
        params.resolution = Some(pathing.space.trail_resolution());
        params.width = pathing.space.trail_width();

        params
    }

    #[inline]
    pub fn try_send(e: PathingEvent) {
        let Ok(sender) = crate::CONTROLLER_SENDER.try_read() else { return };
        sender.pathing_try_send(e);
    }
}
