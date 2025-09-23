use {
    core::time::Duration,
    crate::{
        controller::{
            Controller, ControllerEvent,
        },
        marker::format::MarkerEntry,
        render::machine::RenderMachine,
        MumbleIdentityUpdate,
    },
    glamour::{
        Box2,
        Point2, Point3,
        TransformMap,
    },
    rand::Rng,
    strum_macros::{Display, FromRepr},
    taimi_meta::{
        coords::{FakeSpace, LocalSpace, ScreenSpace},
        ui::{UiMap, UiState},
    },
};

impl Controller {
    pub const MARKERS_NOTABLE_STATE: UiState = UiState::from_bits_retain(
        UiState::MapOpen.bits() | UiState::InCombat.bits()
        | UiState::TextInput.bits() | UiState::Focused.bits()
    );
    pub const TIMERS_NOTABLE_STATE: UiState = UiState::from_bits_retain(
        UiState::InCombat.bits()
    );

    pub fn receive_mumble_identity(id: &MumbleIdentityUpdate) {
        let role = match id.is_commander {
            true => SquadRank::Commander,
            false => SquadRank::Member,
        };
        Controller::try_send(ControllerEvent::MumbleIdentityUpdated {
            role,
        })
    }

    #[cfg(feature = "markers")]
    pub(crate) async fn place_marker_from_map(
        wait_duration: Duration,
        place_duration: Duration,
        point: Point3<LocalSpace>,
        marker: &MarkerEntry,
    ) {
        let map = RenderMachine::shared_map_state().lock().await.clone();
        if map.is_empty() {
            log::error!("cannot place marker while missing map calibration");
            return
        }
        let point = LocalSpace::to2(point);
        let trans = map.calibration.local_to_map()
            .then(map.map_to_worldmap_for(map.context))
            .then(map.worldmap_to_fake_for(map.context));
        if let Some(point) = map.clip(trans.map(point)) {
            let point = map.calibration.map(point);
            Self::place_marker(wait_duration, place_duration, point, marker).await;
        } else {
            log::info!("marker out of bounds");
        }
    }

    pub fn random_map_screen_coordinate(map: &UiMap) -> Point2<ScreenSpace> {
        // TODO: if coord is close to playerpos, skip and try again
        let mut rng = rand::rng();
        let bounds: Box2<FakeSpace> = map.interaction_bounds().to_box2();
        map.calibration.map(Point2::<FakeSpace>::new(
            rng.random_range(bounds.min.x..bounds.max.x),
            rng.random_range(bounds.min.y..bounds.max.y),
        )).floor()
    }
}

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Display, FromRepr)]
#[repr(u8)]
pub enum SquadRank {
    #[default]
    Member = 1,
    Lieutenant = 2,
    Commander = 3,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Display)]
pub enum SquadUpdateType {
    Update,
    Joined,
    Left,
}
