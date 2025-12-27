use {
    crate::{
        controller::{
            api::FestivalState,
            pathing::{registry::PackLoader, space::SpacePackShared, PathingEvent},
        },
        render::machine::MumbleIdentityUpdate,
        settings::SettingsLock,
    },
    std::{collections::btree_map, sync::Arc},
    taimi_meta::ui::GameplayState,
    taimi_sync::watched::{Watched, watch},
    tokio::sync::mpsc,
};
pub use self::{
    loader::{SharedPacks, SharedLoaderPackData, SharedLoaderPacksInfo, SharedLoaderPackInfo, SharedLoaderPackConfig, SharedPackInfo, SharedPackConfig, SharedPackLoaded, SharedPackLoad, SharedResourceRequests, SharedResourceRequestsTx},
    maps::{SharedGameplayMap, SharedMapPackLoaded, SharedMapPackState, SharedMarkerRef, SharedPoiRef, LoadedPoiRef, SharedTrailRef, LoadedTrailRef, LoadedTrailShared, LoadedMarkerRef},
    info::{MapPackInfo, MapTrailInfo, LoadedMarkerInfo, LoadedPoiInfo, LoadedTrailInfo},
};
pub(crate) use self::info::EMPTY_RENDER_ATTRS;
#[cfg(todo)]
pub use self::maps::SharedMaps;

mod info;
mod loader;
mod maps;

#[derive(Debug, Clone)]
pub struct PathingSender {
    pub shared: Arc<PathingShared>,
    pub command: mpsc::Sender<PathingEvent>,
    pub enables: watch::Sender<PathingEnables>,
    #[cfg(todo)]
    pub interactions: broadcast::Sender<InteractionEvent>,
}
impl PathingSender {
    pub fn new(
        gameplay: &watch::Sender<GameplayState>,
        mumble_identity: &watch::Sender<Option<MumbleIdentityUpdate>>,
        festivals: &watch::Sender<FestivalState>,
    ) -> (Self, PathingReceiver) {
        let (command, command_rx) = mpsc::channel(48);
        #[cfg(todo)]
        let interactions = broadcast::Sender::new(Self::INTERACTIONS_BUFFER_LEN);
        let sender = Self {
            shared: Arc::new(PathingShared::new()),
            command,
            enables: watch::Sender::new(PathingEnables::empty()),
            #[cfg(todo)]
            interactions: interactions.clone(),
        };
        let rx = PathingReceiver {
            shared: sender.shared.clone(),
            command: command_rx,
            festivals: festivals.subscribe(),
            gameplay: Watched::subscribe_to(gameplay),
            mumble_identity: mumble_identity.subscribe(),
            enables: sender.enables.clone(),
            #[cfg(todo)]
            interactions_rx: interactions.subscribe(),
            #[cfg(todo)]
            interactions,
        };

        (sender, rx)
    }
    #[cfg(todo)]
    const INTERACTIONS_BUFFER_LEN: usize = 48;
}

pub struct PathingReceiver {
    pub shared: Arc<PathingShared>,
    pub command: mpsc::Receiver<PathingEvent>,
    pub enables: watch::Sender<PathingEnables>,
    pub festivals: watch::Receiver<FestivalState>,
    pub gameplay: Watched<GameplayState>,
    pub mumble_identity: watch::Receiver<Option<MumbleIdentityUpdate>>,
    #[cfg(todo)]
    pub interactions: broadcast::Sender<InteractionEvent>,
    #[cfg(todo)]
    pub interactions_rx: broadcast::Receiver<InteractionEvent>,
}
impl PathingReceiver {
    pub(crate) fn make_loader(&self, settings: SettingsLock) -> Arc<PackLoader> {
        let loader = PackLoader::new(self.shared.clone(), settings);
        Arc::new(loader)
    }
}

/// TODO: make a recv side to this?
#[derive(Debug)]
pub struct PathingShared {
    pub packs: SharedPacks,
    #[cfg(todo)]
    pub maps: watch::Sender<SharedMaps>,
    /// current map
    pub gameplay: watch::Sender<SharedGameplayMap>,
    /// rendering
    pub space: SpacePackShared,
}
impl PathingShared {
    pub fn new() -> Self {
        Self {
            packs: SharedPacks::new(),
            #[cfg(todo)]
            maps: watch::Sender::new(Default::default()),
            gameplay: watch::Sender::new(SharedGameplayMap::default()),
            space: SpacePackShared::new(),
        }
    }

    /// TODO: cache as atomic
    pub fn pack_count(&self) -> usize {
        self.packs.packs.borrow().len()
    }

    pub fn clear_for_shutdown(&self) {
        // TODO: consider true once downstream interprets some changes as shutdown
        let notify = false;
        self.gameplay.send_if_modified(|shared_map| {
            *shared_map = SharedGameplayMap::default();
            notify
        });
        self.space.collection.send_if_modified(|shared_space| {
            *Arc::make_mut(shared_space) = Default::default();
            notify
        });
        #[cfg(todo)]
        {
            self.maps.send_if_modified(|shared_maps| {
                *shared_maps = SharedMaps::empty();
                notify
            });
        }
        self.packs.packs.send_if_modified(|shared_packs| {
            for shared_pack in shared_packs.values_mut() {
                shared_pack.clear_for_shutdown(notify);
            }
            notify
        });
        #[cfg(deleteme)]
        {
            self.packs.info.send_if_modified(|info| {
                *info = Default::default();
                notify
            });
            self.packs.config.send_if_modified(|config| {
                *config = Default::default();
                notify
            });
            self.packs.data.send_if_modified(|data| {
                *data = Default::default();
                notify
            });
        }
    }
}

bitflags::bitflags! {
    #[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct PathingEnables: u8 {
        const KATRENDER = 0x01;
        const API_BYPASS = 0x02;
    }
}
