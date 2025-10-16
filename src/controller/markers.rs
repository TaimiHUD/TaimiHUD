use super::RtSender;

use {
    crate::{
        account_name_canon,
        exports::runtime::{
            mouse::{send_input, MouseInput},
            keyboard::KeyState,
        },
        marker::format::{MarkerSet, RuntimeMarkers},
        ACCOUNT_NAME_CELL,
    },
    arcdps::extras::{UserInfoOwned, UserRole},
    taimi_meta::{
        coords::{LocalSpace, ScreenPoint},
        ui::{
            gameplay::{GameplayState, GameplayTransition},
            MapCalibration, MapOpen,
        },
    },
    tokio::{
        task::JoinHandle,
        time::timeout,
    },
    futures::FutureExt,
};
use {
    crate::{
        exports::runtime as rt,
        marker::format::{MarkerEntry, MarkerFiletype},
        render::{
            machine::MumblelinkTick,
            TextFont,
        },
        settings::{MarkerAutoPlaceSettings, RemoteState, RemoteSource, Settings, SettingsSave, SettingsLock, SourcesFile},
        timer::{CombatState, Position, TimerFile, TimerMachine},
        RenderEvent, SETTINGS, SOURCES, TIMERS_DIR,
    },
    anyhow::{anyhow, Context},
    arcdps::{evtc::event::Event as arcEvent, AgentOwned},
    glam::f32::Vec3,
    relative_path::RelativePathBuf,
    std::{
        collections::{HashMap, HashSet},
        ffi::OsStr,
        fs::exists,
        path::PathBuf,
        sync::{Arc, RwLock},
        time::SystemTime,
    },
    tokio::{
        fs::create_dir_all,
        select,
        sync::{
            mpsc::{Receiver, Sender},
            Mutex,
        },
        time::{interval, sleep, Duration},
    },
};

use {
    super::MapId, crate::{
        controller::{
            Controller, ControllerEvent,
        }, render::machine::RenderMachine, MumbleIdentityUpdate
    }, glamour::{
        Box2,
        Point2, Point3,
        TransformMap,
    }, rand::Rng, strum_macros::{Display, FromRepr}, taimi_meta::{
        coords::{FakeSpace, ScreenSpace},
        ui::{UiMap, UiState},
    }
};

#[cfg(feature = "extension-nexus")]
use nexus::rtapi::GroupMemberOwned;

pub use SquadUpdateType as SquadState;
use SquadRank as SquadRoleState;

#[derive(Debug, Clone, Display)]
pub(crate) enum MarkerSaveEvent {
    Append(MarkerSet, PathBuf),
    Create(MarkerSet, PathBuf, MarkerFiletype),
    Edit(MarkerSet, PathBuf, Option<String>, usize),
}

#[derive(Debug, Clone, Display)]
pub(crate) enum MarkersEvent {
    #[cfg(feature = "extension-nexus")]
    RTAPISquadUpdate(SquadState, GroupMemberOwned),
    ClearSpentAutoplace,
    ExtrasSquadUpdate(Vec<UserInfoOwned>),
    ClearMarkers,
    MarkerAutoPlaceSettings(MarkerAutoPlaceSettings),
    SetMarker(Arc<MarkerSet>),
    SaveMarker(MarkerSaveEvent),
    DeleteMarker {
        path: PathBuf,
        category: Option<String>,
        idx: usize,
    },
    GetMarkerPaths,
    ReloadMarkers,
    #[strum(to_string = "Toggled {0}")]
    MarkerToggle(String),
    #[allow(dead_code)]
    MarkerEnable(String),
    #[allow(dead_code)]
    MarkerDisable(String),
    UiResize(MapCalibration),
    UiMapOpened(MapOpen),
    MumbleIdentityUpdated {
        role: SquadRank,
    },
}

#[derive(Default, Debug)]
pub(crate) struct MarkersController {
    #[cfg(feature = "extension-nexus")]
    pub rtapi_squad: HashMap<String, GroupMemberOwned>,
    pub extras_squad: HashMap<String, UserInfoOwned>,
    pub markers: HashMap<String, Vec<Arc<MarkerSet>>>,
    pub spent_markers: HashSet<Arc<MarkerSet>>,
    pub map_id_to_markers: HashMap<u32, HashSet<Arc<MarkerSet>>>,
    pub marker_autoplace: Option<MarkerAutoPlaceSettings>,
    pub mumble_role: Option<SquadRank>,
}

impl MarkersController {
    async fn load(&mut self, rt_sender: RtSender) -> anyhow::Result<()> {
        let markers_dir = crate::ADDON_DIR.join("markers");
        if !exists(&markers_dir).expect("Can't check if directory exists") {
            create_dir_all(&markers_dir).await?;
        }
        let markers = RuntimeMarkers::load_many(&markers_dir, 100).await?;
        let markers = RuntimeMarkers::markers(markers).await;
        let _ = rt_sender
            .send(RenderEvent::MarkerData(markers.clone()))
            .await;
        self.markers = markers;
        Ok(())
    }

    pub async fn setup(&mut self, settings: SettingsLock, rt_sender: RtSender) {
        let settings_lock = settings.read().await;
        self.marker_autoplace = Some(settings_lock.marker_autoplace.clone());
        drop(settings_lock);
        match self.load(rt_sender.clone()).await {
            Ok(()) => (),
            Err(err) => log::error!("Error loading markers: {}", err),
        }
        let mut map_id_to_markers: HashMap<u32, HashSet<Arc<MarkerSet>>> = HashMap::new();
        let marker_sets: Vec<_> = self.markers.values().flatten().collect();
        for set in marker_sets {
            let entry = map_id_to_markers.entry(set.map_id).or_default();
            entry.insert(set.clone());
        }
        self.map_id_to_markers = map_id_to_markers;
    }

    async fn open_window(&self) {
        let mut settings_lock = Settings::async_write().await.expect("Settings unitialized, impossible");
        settings_lock.set_window_state("markers", Some(true));
        drop(settings_lock);
    }

    pub(crate) async fn handle_map_event(&mut self, map_id: u32, rt_sender: RtSender) {
        let markers_for_map = self.map_id_to_markers.get(&map_id);
        let markers_for_map = match markers_for_map {
            Some(s) => s.clone(),
            None => Default::default(),
        };
        let event_markers = markers_for_map.into_iter().collect::<Vec<_>>();
        let _ = rt_sender
            .send(RenderEvent::MarkerMap(event_markers))
            .await;
        self.spent_markers = Default::default();
    }

    async fn handle_marker_autoplace(&self, marker: &MarkerSet, rt_sender: RtSender) -> anyhow::Result<()> {
        if marker.status() {
            use crate::settings::SquadCondition;
            let role = self.get_role().await;
            log::info!("Role detected: {:?}", role);

            if let Some(t) = &self.marker_autoplace {
                match t {
                    MarkerAutoPlaceSettings::OpenWindow(s) => match s {
                        SquadCondition::Never => (),
                        SquadCondition::IfCommander => {
                            if let Some(role) = role {
                                if role == SquadRoleState::Commander {
                                    self.open_window().await;
                                }
                            }
                        }
                        SquadCondition::IfLieutenantOrAbove => {
                            if let Some(role) = role {
                                if role >= SquadRoleState::Lieutenant {
                                    self.open_window().await;
                                }
                            }
                        }
                        SquadCondition::Always => self.open_window().await,
                    },
                    MarkerAutoPlaceSettings::Place(s) => match s {
                        SquadCondition::Never => (),
                        SquadCondition::IfCommander => {
                            if let Some(role) = role {
                                if role == SquadRoleState::Commander {
                                    self.set_marker(marker, rt_sender.clone()).await??;
                                }
                            }
                        }
                        SquadCondition::IfLieutenantOrAbove => {
                            if let Some(role) = role {
                                if role >= SquadRoleState::Lieutenant {
                                    self.set_marker(marker, rt_sender.clone()).await??;
                                }
                            }
                        }
                        SquadCondition::Always => self.set_marker(marker, rt_sender.clone()).await??,
                    },
                    MarkerAutoPlaceSettings::DoNothing => (),
                }
            }
        }
        Ok(())
    }

    pub(crate) async fn handle_position(&mut self, map_id: MapId, position: Vec3, rt_sender: RtSender) -> anyhow::Result<()> {
        if let Some(map_id) = &map_id {
            if let Some(markers_for_map) = self.map_id_to_markers.get(map_id) {
                let mut new_spent_markers = Vec::new();
                for marker in markers_for_map.difference(&self.spent_markers) {
                    if marker.trigger(position.into()) {
                        new_spent_markers.push(marker.clone());
                    }
                }
                self.spent_markers.extend(new_spent_markers.clone());
                for spent_marker in new_spent_markers {
                    log::debug!("Marker autoplace triggered for {}", spent_marker.name);
                    self.handle_marker_autoplace(&spent_marker, rt_sender.clone()).await?;
                }
            }
        }
        Ok(())
    }

    pub(crate) async fn handle_mumble_identity(&mut self, role: SquadRank) {
        self.mumble_role = Some(role);
    }

    async fn extras_squad_update(&mut self, data: Vec<UserInfoOwned>) {
        self.extras_squad.clear();
        for datum in data {
            if let Some(account_name) = &datum.account_name {
                self.extras_squad.insert(account_name.clone(), datum);
            }
        }
    }

    async fn clear_spent_autoplace(&mut self) {
        self.spent_markers.clear();
    }

    #[cfg(feature = "extension-nexus")]
    async fn rtapi_squad_update(&mut self, change: SquadState, member: GroupMemberOwned) {
        let account_name = ACCOUNT_NAME_CELL.get();
        if let Some(account_name) = account_name {
            let member_name = match account_name_canon(&member.account_name) {
                Some(name) => name,
                None => return,
            };
            match change {
                SquadState::Left => {
                    if member_name == account_name {
                        self.rtapi_squad.clear();
                    } else {
                        self.rtapi_squad.remove(member_name);
                    }
                }
                SquadState::Joined => {
                    self.rtapi_squad.insert(member_name.into(), member);
                }
                SquadState::Update => {
                    if let Some(entry) = self.rtapi_squad.get_mut(member_name) {
                        *entry = member;
                    }
                }
            }
        }
    }

    async fn save(&mut self, e: MarkerSaveEvent, rt_sender: RtSender) -> anyhow::Result<()> {
        match e {
            MarkerSaveEvent::Append(ms, p) => {
                RuntimeMarkers::append(&p, ms).await?;
            }
            MarkerSaveEvent::Create(ms, p, ft) => {
                RuntimeMarkers::create(&p, ft, ms).await?;
            }
            MarkerSaveEvent::Edit(ms, p, oc, idx) => {
                RuntimeMarkers::edit(ms, &p, oc, idx).await?;
            }
        }
        self.reload(rt_sender.clone()).await;
        Ok(())
    }

    async fn delete_marker(
        &mut self,
        path: &PathBuf,
        category: Option<String>,
        idx: usize,
        rt_sender: RtSender
    ) -> anyhow::Result<()> {
        RuntimeMarkers::delete(path, category, idx).await?;
        self.reload(rt_sender.clone()).await;
        Ok(())
    }

    async fn get_marker_paths(&self, rt_sender: RtSender) -> anyhow::Result<()> {
        let markers_dir = crate::ADDON_DIR.join("markers");
        let mut paths: Vec<PathBuf> = Vec::new();
        for path in RuntimeMarkers::get_paths(&markers_dir)? {
            paths.push(path?);
        }
        let _ = rt_sender
            .send(RenderEvent::GiveMarkerPaths(paths))
            .await;

        Ok(())
    }

    async fn set_autoplace_settings(
        &mut self,
        maps: MarkerAutoPlaceSettings,
    ) -> anyhow::Result<()> {
        let mut settings_lock = Settings::async_write().await.expect("Settings unitialized, impossible");
        settings_lock.set_marker_autoplace_settings(&maps)?;
        drop(settings_lock);
        self.marker_autoplace = Some(maps);
        Ok(())
    }

    async fn get_role(&self) -> Option<SquadRoleState> {
        #[cfg(feature = "extension-nexus")]
        if let Ok(Some(rtapi)) = rt::rtapi() {
            if let Some(player) = rtapi.read_player() {
                let account_name = player.account_name;
                if let Some(squad_state) = self.rtapi_squad.get(&account_name) {
                    if squad_state.is_commander {
                        return Some(SquadRoleState::Commander);
                    } else if squad_state.is_lieutenant {
                        return Some(SquadRoleState::Lieutenant);
                    } else {
                        return Some(SquadRoleState::Member);
                    }
                }
            }
        }
        if !self.extras_squad.is_empty() {
            if let Some(account_name) = ACCOUNT_NAME_CELL.get() {
                if let Some(squad_state) = self.extras_squad.get(account_name) {
                    return match squad_state.role {
                        UserRole::SquadLeader => Some(SquadRoleState::Commander),
                        UserRole::Lieutenant => Some(SquadRoleState::Lieutenant),
                        UserRole::Member => Some(SquadRoleState::Member),
                        _ => None,
                    };
                }
            }
        }
        if let Some(role) = self.mumble_role {
            return Some(role)
        }
        None
    }

    pub const KEY_INVOKE_DURATION: Duration = Duration::from_millis(50);

    pub async fn reload(&mut self, rt_sender: RtSender) {
        self.load(rt_sender.clone())
            .await
            .expect("markers load failed");
        let mut map_id_to_markers: HashMap<u32, HashSet<Arc<MarkerSet>>> = HashMap::new();
        let marker_sets: Vec<_> = self.markers.values().flatten().collect();
        for set in marker_sets {
            let entry = map_id_to_markers.entry(set.map_id).or_default();
            entry.insert(set.clone());
        }
        self.map_id_to_markers = map_id_to_markers;
    }
    async fn clear(&self) {
        use crate::marker::format::MarkerType;

        if let Err(e) = rt::invoke_marker_bind(MarkerType::ClearMarkers, false, Self::KEY_INVOKE_DURATION, None).await {
            log::warn!("Failed to clear markers: {e}");
        }
    }

    async fn drag_mouse_abs(from: ScreenPoint, to: ScreenPoint) -> rt::RuntimeResult<()> {
        let wait_duration = Duration::from_millis(10);
        let from = rt::mouse::mouse_position_from_screen(from);
        let to = rt::mouse::mouse_position_from_screen(to);
        sleep(wait_duration).await;
        send_input(MouseInput::from(from))?;
        sleep(wait_duration).await;
        send_input(MouseInput::new(from, KeyState::BUTTON_L, Some(true)))?;
        sleep(wait_duration).await;
        send_input(MouseInput::from(to))?;
        sleep(wait_duration).await;
        send_input(MouseInput::new(to, KeyState::BUTTON_L, Some(false)))?;
        sleep(wait_duration).await;
        Ok(())
    }

    async fn place_marker(
        wait_duration: Duration,
        place_duration: Duration,
        point: ScreenPoint,
        marker: &MarkerEntry,
    ) {
        sleep(wait_duration).await;
        let point = rt::mouse::mouse_position_from_screen(point);
        let res = rt::invoke_marker_bind(marker.marker, false, place_duration, Some(point)).await
            .map_err(anyhow::Error::msg)
            .with_context(|| format!("Failed to place marker {:?}", marker.marker));
        if let Err(e) = res {
            log::warn!("{e:#}");
        }
    }

    fn set_marker(&self, markers: &MarkerSet, rt_sender: RtSender) -> JoinHandle<anyhow::Result<()>> {
        tokio::spawn(Self::set_marker_task(
            markers.clone(),
            rt_sender.clone(),
        ))
    }

    async fn set_marker_task(
        markers: MarkerSet,
        rt_sender: RtSender,
    ) -> anyhow::Result<()> {
        use {
            anyhow::anyhow,
            glamour::TransformMap,
            taimi_meta::coords::LocalPoint,
        };
        let player_position = rt::mumble_link_ptr()
            .map(|ml| LocalPoint::from_array(ml.read_avatar().position)).ok();
        if let Some(player_position) = player_position {
            let mut too_far = false;
            for marker in &markers.markers {
                if player_position.distance(marker.position.into()) >= 127.0 {
                    too_far = true;
                    break;
                }
            }
            if too_far {
                let err =
                    anyhow!("Player is too far away from the markers they are trying to place.");
                let _ = rt_sender
                    .send(RenderEvent::OpenableError(
                        format!("Error setting marker set: {}", &markers.name),
                        err,
                    ))
                    .await;
                return Err(anyhow!(
                    "Player is too far away from the markers they are trying to place."
                ));
            }
        }

        let wait_duration = Duration::from_millis(50);
        let original_position = rt::window_mouse_position()
            .map_err(|e| anyhow!("Getting cursor pos: {e}"))?;
        for marker in &markers.markers {
            // check if it is possible to place immediately
            let local_point: LocalPoint = marker.position.into();
            let map = RenderMachine::shared_map_state().lock().await.clone();
            let (map_point, screen_point) = if let Some(map) = map.get() {
                let map_point = map.calibration.map(LocalSpace::to2(local_point));
                let fake_point = map.map_to_worldmap_for(map.context)
                    .then(map.worldmap_to_fake_for(map.context))
                    .map(map_point);
                let screen_point = map.calibration.map(fake_point);
                (Some(map_point), map.clip_screen(screen_point))
            } else {
                (None, None)
            };
            match screen_point {
                // if the marker is on the map, that's fine, place it
                Some(point) => {
                    Self::place_marker(wait_duration, Self::KEY_INVOKE_DURATION, point, marker).await;
                }
                // if the marker isn't on the map, we need to get our perspective to include
                // the marker
                None => {
                    if let Some(map_point) = map_point {
                        let max_attempts = 10; // inshallah
                        let mut attempts = 0;
                        let map_centre = RenderMachine::shared_map_state()
                            .lock().await
                            .get().map(|map| map.centre());
                        log::debug!("Reached none arm for marker placement");
                        if let Some(mut map_centre) = map_centre {
                            while (map_centre.distance(map_point) > 5.0)
                                && (attempts < max_attempts)
                            {
                                log::debug!("Attempt {}/{}", attempts, max_attempts);
                                let map = RenderMachine::shared_map_state().lock().await.clone();
                                if let Some(map) = map.get() {
                                    let bounds = map.calibration.map(map.bounds());
                                    map_centre = map.centre();
                                    let remaining_distance = map_centre.distance(map_point);
                                    log::debug!("Remaining distance: {}", remaining_distance);
                                    let drag_from = Self::random_map_screen_coordinate(map);
                                    let fake_point = map.map_to_worldmap_for(map.context)
                                        .then(map.worldmap_to_fake_for(map.context))
                                        .map(map_point);
                                    let screen_point = map.calibration.map(fake_point);
                                    #[cfg(todo)]
                                    let difference_screen = map.map_to_worldmap_for(map.context)
                                        .then(map.worldmap_to_fake_for(map.context))
                                        .then(map.calibration.to_screen())
                                        .map(map_point - map_centre);
                                    let difference_screen = screen_point - bounds.center();

                                    // the l
                                    //let (min, max) = (bounds.min(), bounds.max());
                                    let drag_res = drag_from - difference_screen;
                                    //let drag_res = drag_res.clamp(min, max);
                                    let drag_res = drag_res.clamp(glamour::Point2::ZERO, map.calibration.display_size.to_vector().to_point());
                                    log::debug!(
                                        "Map centre: {:?}, destination: {:?}",
                                        map_centre,
                                        map_point
                                    );
                                    //log::debug!("Min: {:?}, max: {:?}", min, max);
                                    log::debug!(
                                        "Attempting a drag from {:?} to {:?}",
                                        drag_from,
                                        drag_res
                                    );
                                    Self::drag_mouse_abs(drag_from, drag_res).await
                                        .map_err(|e| anyhow!("mouse drag failed: {e}"))?;
                                    sleep(wait_duration).await;
                                }
                                attempts += 1;
                            }
                            log::info!("Attempts: {}", attempts);
                            if map_centre.distance(map_point) > 5.0 {
                                let err =
                                    anyhow!("Could not drag map perspective to marker location!");
                                let _ = rt_sender
                                    .send(RenderEvent::OpenableError(
                                        format!("Error setting marker set: {}", &markers.name),
                                        err,
                                    ))
                                    .await;
                                return Err(anyhow!(
                                    "Could not drag map perspective to marker location!"
                                ));
                            } else {
                                Self::place_marker_from_map(
                                    wait_duration,
                                    Self::KEY_INVOKE_DURATION,
                                    marker.position.into(),
                                    marker,
                                )
                                .await;
                            }
                        }
                    }
                }
            }
        }
        sleep(wait_duration).await;
        send_input(original_position)
            .map_err(|e| anyhow!("Failed to restore original cursor position: {e}"))?;
        Ok(())
    }

    pub const MARKERS_NOTABLE_STATE: UiState = UiState::from_bits_retain(
        UiState::MapOpen.bits() | UiState::InCombat.bits()
        | UiState::TextInput.bits() | UiState::Focused.bits()
    );

    pub fn receive_mumble_identity(id: &MumbleIdentityUpdate) {
        let role = match id.is_commander {
            true => SquadRank::Commander,
            false => SquadRank::Member,
        };
        MarkersController::try_send(MarkersEvent::MumbleIdentityUpdated {
            role,
        })
    }

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
    
    async fn toggle(&mut self, id: &str) {
        let mut settings_lock = Settings::async_write().await.expect("Settings unitialized, impossible");
        settings_lock.toggle_marker(id.to_string());
        drop(settings_lock);
    }

    async fn enable(&mut self, id: &str) {
        let mut settings_lock = Settings::async_write().await.expect("Settings unitialized, impossible");
        settings_lock.enable_marker(id.to_string());
        drop(settings_lock);
    }

    async fn disable(&mut self, id: &str) {
        let mut settings_lock = Settings::async_write().await.expect("Settings unitialized, impossible");
        settings_lock.disable_marker(id.to_string());
        drop(settings_lock);
    }

    pub fn try_send(e: MarkersEvent) {
        let sender = crate::CONTROLLER_SENDER.try_read();
        let sender = sender.as_ref().map(|s| &**s);
        let full_e = ControllerEvent::Markers(e);
        if let Ok(Some(sender)) = sender {
            let _ = sender.try_send(full_e);
        }
    }

    pub(crate) async fn handle_event(&mut self, event: MarkersEvent, rt_sender: &RtSender) -> anyhow::Result<()> {
        use MarkersEvent::*;
        match event {
            MumbleIdentityUpdated { role } => self.handle_mumble_identity(role).await,
            #[cfg(feature = "extension-nexus")]
            RTAPISquadUpdate(change, member) => self.rtapi_squad_update(change, member).await,
            ClearMarkers => self.clear().await,
            ClearSpentAutoplace => self.clear_spent_autoplace().await,
            MarkerEnable(id) => self.enable(&id).await,
            MarkerDisable(id) => self.disable(&id).await,
            MarkerToggle(id) => self.toggle(&id).await,
            ExtrasSquadUpdate(members) => self.extras_squad_update(members).await,
            MarkerAutoPlaceSettings(maps) => self.set_autoplace_settings(maps).await?,
            ReloadMarkers => self.reload(rt_sender.clone()).await,
            SetMarker(t) => {
                self.set_marker(&t, rt_sender.clone());
            }
            SaveMarker(e) => self.save(e, rt_sender.clone()).await?,
            DeleteMarker {
                path,
                category,
                idx,
            } => self.delete_marker(&path, category, idx, rt_sender.clone()).await?,
            GetMarkerPaths => self.get_marker_paths(rt_sender.clone()).await?,
            UiResize(_calibration) => (),
            UiMapOpened(_open) => (),
        }
        Ok(())
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
