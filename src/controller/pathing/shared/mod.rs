#[allow(unused_imports)]
pub use {
    self::{
        display::LocDisplay,
        loader::{
            LoadReport,
            SharedLoaderPacksInfo,
            SharedPackConfig,
            SharedPackInfo,
            SharedPackLoad,
            SharedPackLoaded,
            SharedPacks,
            SharedResourceRequests,
            SharedResourceRequestsTx,
        },
        maps::{
            LoadedMarkerRef,
            LoadedPoiRef,
            LoadedPoiShared,
            LoadedTrailRef,
            LoadedTrailShared,
            SharedGameplayMap,
            SharedMapPackLoaded,
            SharedMapPackState,
            SharedMarkerRef,
            SharedPoiRef,
            SharedTrailRef,
        },
        space::{
            SpacePackShared,
            TextureLoadRequests,
            TextureLoadRequestsTx,
            TrailGeometryRequests,
            TrailGeometryRequestsTx,
            TrailGeometrySections,
        },
    },
    crate::{
        controller::pathing::state::{LoadedTrailGeometry, LoadedTrailSection},
        settings::pathing::HiddenGuids,
    },
};
use {
    crate::{
        controller::{
            api::{AchievementState, FestivalState, RaidState},
            pathing::{
                registry::{PackLoader, PackPath},
                ExternalFilterState,
                PathingEvent,
            },
        },
        render::machine::MumbleIdentityUpdate,
        settings::{pathing::PathingSettings, SettingsLock},
    },
    futures::future::Either,
    std::{ops, sync::Arc},
    taimi_meta::ui::GameplayState,
    taimi_sync::watched::{self, Watched},
    tokio::sync::mpsc,
};

#[cfg(feature = "paths-interact")]
pub use self::interact::{InteractReceiver, InteractSender, InteractShared};

mod display;
#[cfg(feature = "paths-interact")]
pub mod interact;
mod loader;
mod maps;
mod space;

#[derive(Debug, Clone)]
pub struct PathingSender {
    pub shared: Arc<PathingShared>,
    pub command: mpsc::Sender<PathingEvent>,
    pub enables: watched::Tx<PathingEnables>,
    pub load_throttle: watched::Tx<usize>,
    /// already exposed via [PathingShared]
    #[cfg(todo = "unnecessary")]
    pub interactions: InteractSender,
}
impl PathingSender {
    pub fn new(
        gameplay: &watched::Tx<GameplayState>,
        mumble_identity: &watched::Tx<Option<MumbleIdentityUpdate>>,
        festivals: &watched::Tx<FestivalState>,
        achievements: &watched::Tx<Arc<AchievementState>>,
        raids: &watched::Tx<Arc<RaidState>>,
    ) -> (Self, PathingReceiver) {
        let (command, command_rx) = mpsc::channel(48);
        let sender = Self {
            shared: Arc::new(PathingShared::new()),
            command,
            enables: watched::Tx::new(PathingEnables::empty()),
            load_throttle: watched::Tx::new(PathingSettings::DEFAULT_LOAD_SIMULTANEOUS),
        };
        let rx = PathingReceiver {
            shared: sender.shared.clone(),
            command: command_rx,
            festivals: {
                let mut festivals = festivals.subscribe();
                festivals.mark_changed();
                festivals
            },
            achievements: {
                let mut achievements = achievements.subscribe();
                achievements.mark_changed();
                achievements
            },
            raids: {
                let mut raids = raids.subscribe();
                raids.mark_changed();
                raids
            },
            gameplay: {
                let mut gameplay = Watched::subscribe_to(gameplay);
                gameplay.cached = Some(GameplayState::INITIAL);
                gameplay
            },
            mumble_identity: {
                let mut identity = mumble_identity.subscribe();
                identity.mark_changed();
                identity
            },
            enables: Watched::subscribe_to(&sender.enables),
            load_throttle: {
                let mut load_throttle = Watched::subscribe_to(&sender.load_throttle);
                let _ = load_throttle.try_read_mut();
                load_throttle
            },
            #[cfg(feature = "paths-interact")]
            interact: InteractReceiver::new(&sender.shared.interact),
        };

        (sender, rx)
    }
}

pub struct PathingReceiver {
    pub shared: Arc<PathingShared>,
    pub command: mpsc::Receiver<PathingEvent>,
    pub enables: Watched<PathingEnables>,
    pub load_throttle: Watched<usize>,
    pub festivals: watched::Rx<FestivalState>,
    /// TODO: cfg(feature = "api")
    pub achievements: watched::Rx<Arc<AchievementState>>,
    /// TODO: cfg(feature = "api")
    pub raids: watched::Rx<Arc<RaidState>>,
    pub gameplay: Watched<GameplayState>,
    pub mumble_identity: watched::Rx<Option<MumbleIdentityUpdate>>,
    #[cfg(feature = "paths-interact")]
    pub interact: InteractReceiver,
}
impl PathingReceiver {
    pub(crate) fn make_loader(&self, settings: SettingsLock) -> Arc<PackLoader> {
        let load_throttle = self.load_throttle.cached.clone();
        let loader = PackLoader::new(self.shared.clone(), settings, load_throttle);
        Arc::new(loader)
    }
    #[inline]
    pub(super) fn enables(&self) -> PathingEnables {
        self.enables.cached.unwrap_or_default()
    }

    /// TODO: with_filter_state borrowing variant to avoid clone?
    /// lock should be fine to hold...
    pub(super) fn get_filter_state(&self) -> ExternalFilterState {
        let festivals = self.festivals.borrow().get();
        let bypass = self.enables().contains(PathingEnables::API_BYPASS);
        let (clears, achievements) = match bypass {
            true => Default::default(),
            false => (self.raids.borrow().clone(), self.achievements.borrow().clone()),
        };
        (festivals, clears, achievements)
    }

    pub(super) fn is_katrender_enabled(&self) -> bool {
        self.enables().contains(PathingEnables::KATRENDER)
    }
    pub(super) fn is_engine_active(&self) -> bool {
        self.enables().contains(PathingEnables::ENGINE)
    }
    pub(super) fn is_online(&self) -> bool {
        self.enables()
            .contains(PathingEnables::KATRENDER | PathingEnables::ENGINE)
    }
}

/// TODO: make a recv side to this?
#[derive(Debug)]
pub struct PathingShared {
    pub packs: SharedPacks,
    #[cfg(todo)]
    pub maps: watched::Tx<SharedMaps>,
    /// current map
    pub gameplay: watched::Tx<SharedGameplayMap>,
    /// rendering
    pub space: SpacePackShared,
    #[cfg(feature = "paths-interact")]
    pub interact: InteractShared,
}
impl PathingShared {
    pub fn new() -> Self {
        Self {
            packs: SharedPacks::new(),
            #[cfg(todo)]
            maps: watched::Tx::new(Default::default()),
            gameplay: watched::Tx::new(SharedGameplayMap::default()),
            space: SpacePackShared::new(),
            #[cfg(feature = "paths-interact")]
            interact: InteractShared::new(),
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
    }

    pub fn watch_config_changes(
        &self,
        mark_changed: Either<bool, ops::RangeFrom<PackPath>>,
    ) -> watched::WatchStreamBox<PackPath, SharedPackConfig> {
        let packs = self.packs.packs.borrow();
        let configs = packs.iter().map(|(path, load)| (path, &load.config));
        match mark_changed {
            Either::Left(mark_changed) => watched::stream::stream_watch_changes_of(configs, mark_changed),
            Either::Right(mark_changed) => {
                let recv = configs.map(|(path, tx)| {
                    let mut rx = tx.subscribe();
                    if mark_changed.contains(&path) {
                        rx.mark_changed();
                    }
                    (path, rx)
                });
                Box::new(watched::stream::stream_watch_changes(recv))
            },
        }
    }
}

bitflags::bitflags! {
    #[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct PathingEnables: u8 {
        const KATRENDER = 0x01;
        const API_BYPASS = 0x02;
        /// signals that katrender is successfully initialized and running
        const ENGINE = 0x04;
        #[cfg(feature = "paths-lua")]
        const SCRIPTING_LUA = 0x08;
        #[cfg(feature = "paths-lua")]
        const SCRIPTING_UNSECURED = 0x10;
    }
}
