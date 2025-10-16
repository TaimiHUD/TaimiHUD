#[cfg(feature = "markers")]
use {
    crate::{
        account_name_canon,
        exports::runtime::{
            mouse::{send_input, MouseInput},
            keyboard::KeyState,
        },
        marker::format::{MarkerSet, RuntimeMarkers},
        render::machine::RenderMachine,
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
    strum_macros::Display,
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

#[cfg(feature = "space")]
use {
    crate::space::{
        pack::LoaderBox,
        Engine, engine::SpaceEvent,
    },
    taimi_pack::Pack,
};
#[cfg(all(feature = "markers", feature = "extension-nexus"))]
use nexus::rtapi::GroupMemberOwned;

#[cfg(feature = "markers")]
pub use self::markers::{
    SquadRank,
    SquadUpdateType as SquadState,
};
use SquadRank as SquadRoleState;

#[cfg(feature = "markers")]
mod markers;
mod runtime;

#[derive(Debug)]
pub struct Controller {
    receiver: Receiver<ControllerEvent>,
    #[cfg(all(feature = "markers", feature = "extension-nexus"))]
    pub rtapi_squad: HashMap<String, GroupMemberOwned>,
    #[cfg(feature = "markers")]
    pub extras_squad: HashMap<String, UserInfoOwned>,
    pub agent: Option<AgentOwned>,
    pub previous_combat_state: bool,
    #[cfg(feature = "markers")]
    pub markers: HashMap<String, Vec<Arc<MarkerSet>>>,
    #[cfg(feature = "markers")]
    pub spent_markers: HashSet<Arc<MarkerSet>>,
    #[cfg(feature = "markers")]
    pub map_id_to_markers: HashMap<u32, HashSet<Arc<MarkerSet>>>,
    #[cfg(feature = "markers")]
    pub marker_autoplace: Option<MarkerAutoPlaceSettings>,
    pub rt_sender: Sender<RenderEvent>,
    #[cfg(feature = "markers")]
    pub mumble_role: Option<SquadRank>,
    pub map_id: Option<u32>,
    pub player_position: Option<Vec3>,
    alert_sem: Arc<Mutex<()>>,
    pub timers: Vec<Arc<TimerFile>>,
    pub current_timers: Vec<TimerMachine>,
    pub sources_to_timers: HashMap<String, Vec<Arc<TimerFile>>>,
    pub map_id_to_timers: HashMap<u32, Vec<Arc<TimerFile>>>,
    settings: SettingsLock,
    save_interval: tokio::time::Interval,
}

impl Controller {
    pub fn player_position(&self) -> Option<Position> {
        self.player_position.map(Position::Vec3)
    }

    pub fn new(
        receiver: Receiver<ControllerEvent>,
        rt_sender: Sender<crate::RenderEvent>,
        settings: SettingsLock,
    ) -> Self {
        Self {
            receiver,
            #[cfg(all(feature = "markers", feature = "extension-nexus"))]
            rtapi_squad: Default::default(),
            #[cfg(feature = "markers")]
            extras_squad: Default::default(),
            #[cfg(feature = "markers")]
            marker_autoplace: Default::default(),
            previous_combat_state: Default::default(),
            rt_sender,
            settings,
            #[cfg(feature = "markers")]
            markers: Default::default(),
            #[cfg(feature = "markers")]
            map_id_to_markers: Default::default(),
            #[cfg(feature = "markers")]
            spent_markers: Default::default(),
            #[cfg(feature = "markers")]
            mumble_role: Default::default(),
            agent: Default::default(),
            map_id: Default::default(),
            player_position: Default::default(),
            alert_sem: Default::default(),
            timers: Default::default(),
            current_timers: Default::default(),
            sources_to_timers: Default::default(),
            map_id_to_timers: Default::default(),
            save_interval: interval(Duration::from_secs(60 * 10)),
        }
    }

    pub fn load(
        receiver: Receiver<ControllerEvent>,
        rt_sender: Sender<crate::RenderEvent>,
        addon_dir: PathBuf,
    ) {
        let rt = match Self::new_runtime() {
            Ok(rt) => rt,
            Err(error) => {
                log::error!("Error! {}", error);
                return;
            }
        };
        let evt_loop = async move {
            let settings = Settings::load_access(&addon_dir.clone()).await;
            let mut state = Self::new(receiver, rt_sender, settings);
            state.run().await;
        };
        rt.block_on(evt_loop);
        Self::shutdown(rt);
    }

    pub async fn run(&mut self) {
            let sources = SourcesFile::load()
                .await
                .expect("Couldn't load sources file");
            let sources = Arc::new(RwLock::new(sources));
            let _ = SOURCES.set(sources);
            let state = self;
            let _ = SETTINGS.set(state.settings.clone());
            let settings = SETTINGS.get().unwrap();
            let settings_lock = settings.read().await;
            state.marker_autoplace = Some(settings_lock.marker_autoplace.clone());
            drop(settings_lock);
            state.setup_timers().await;
            #[cfg(feature = "markers")]
            state.setup_markers().await;
            let mut taimi_interval = interval(Duration::from_millis(125));

            loop {
                select! {
                    evt = state.receiver.recv() => match evt {
                        Some(evt) => {
                            match state.handle_event(evt).await {
                                Ok(true) => (),
                                Ok(false) => break,
                                Err(error) => {
                                    log::error!("Error! {}", error)
                                }
                            }
                        },
                        None => {
                            break
                        },
                    },
                    _ = taimi_interval.tick() => {
                        let _ = state.tick().await;
                    },
                    _ = state.save_interval.tick() => {
                        state.commit_settings().await;
                    },
                }
            }
    }

    /*async fn load_markers_file(&mut self) -> anyhow::Result<()> {
        let addon_dir = get_addon_dir("Taimi").expect("Invalid addon dir");
        let markers = MarkerFile::load(&addon_dir.join("Markers.json")).await?;
        let _ = self
            .rt_sender
            .send(RenderEvent::MarkerData(markers.clone()))
            .await;
        self.markers = Some(markers.clone());
        Ok(())
    }*/

    async fn settings_write(&self) -> tokio::sync::RwLockWriteGuard<Settings> {
        let mut settings = self.settings.write().await;
        settings.mark_dirty();
        settings
    }

    #[cfg(feature = "markers")]
    async fn open_marker_window(&self) {
        let mut settings_lock = self.settings_write().await;
        settings_lock.set_window_state("markers", Some(true));
        drop(settings_lock);
    }

    async fn check_sources(&self) -> anyhow::Result<()> {
        SourcesFile::get_sources().await?;
        let mut settings_lock = self.settings_write().await;
        settings_lock.remotes = RemoteState::suggested_sources().unwrap_or_default();
        drop(settings_lock);
        let sources = SourcesFile::load()
            .await?;
        let _ = SOURCES.set(Arc::new(RwLock::new(sources)));
        Ok(())
    }

    #[cfg(feature = "markers")]
    async fn handle_marker_autoplace(&self, marker: &MarkerSet) -> anyhow::Result<()> {
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
                                    self.open_marker_window().await;
                                }
                            }
                        }
                        SquadCondition::IfLieutenantOrAbove => {
                            if let Some(role) = role {
                                if role >= SquadRoleState::Lieutenant {
                                    self.open_marker_window().await;
                                }
                            }
                        }
                        SquadCondition::Always => self.open_marker_window().await,
                    },
                    MarkerAutoPlaceSettings::Place(s) => match s {
                        SquadCondition::Never => (),
                        SquadCondition::IfCommander => {
                            if let Some(role) = role {
                                if role == SquadRoleState::Commander {
                                    self.set_marker(marker).await??;
                                }
                            }
                        }
                        SquadCondition::IfLieutenantOrAbove => {
                            if let Some(role) = role {
                                if role >= SquadRoleState::Lieutenant {
                                    self.set_marker(marker).await??;
                                }
                            }
                        }
                        SquadCondition::Always => self.set_marker(marker).await??,
                    },
                    MarkerAutoPlaceSettings::DoNothing => (),
                }
            }
        }
        Ok(())
    }

    #[cfg(feature = "markers")]
    async fn load_markers_files(&mut self) -> anyhow::Result<()> {
        let markers_dir = crate::ADDON_DIR.join("markers");
        if !exists(&markers_dir).expect("Can't check if directory exists") {
            create_dir_all(&markers_dir).await?;
        }
        let markers = RuntimeMarkers::load_many(&markers_dir, 100).await?;
        let markers = RuntimeMarkers::markers(markers).await;
        let _ = self
            .rt_sender
            .send(RenderEvent::MarkerData(markers.clone()))
            .await;
        self.markers = markers;
        Ok(())
    }

    async fn setup_markers(&mut self) {
        match self.load_markers_files().await {
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

    async fn load_timer_files(&self) -> Vec<Arc<TimerFile>> {
        let settings_lock = self.settings.read().await;
        let mut timers = Vec::new();
        for remote in settings_lock.remotes.iter() {
            timers.extend(remote.load().await);
        }
        drop(settings_lock);
        let timers_len = timers.len();
        log::trace!("Total loaded timers: {}", timers_len);
        timers
    }

    async fn setup_timers(&mut self) {
        log::debug!("Preparing to setup timers");
        self.timers = self.load_timer_files().await;
        if exists(&*TIMERS_DIR).expect("oh no i cant access my own addon dir") {
            let adhoc_timers = TimerFile::load_many_sourceless(&*TIMERS_DIR, 100)
                .await
                .expect("wah");
            self.timers.extend(adhoc_timers);
        } else {
            create_dir_all(&*TIMERS_DIR)
                .await
                .expect("Can't create timers dir");
        }
        for timer in &self.timers {
            if let Some(association) = &timer.association {
                self.sources_to_timers
                    .entry(association.clone())
                    .or_default();
                if let Some(val) = self.sources_to_timers.get_mut(association) {
                    val.push(timer.clone());
                };
            }
            // Handle map to timers
            self.map_id_to_timers.entry(timer.map_id).or_default();
            if let Some(val) = self.map_id_to_timers.get_mut(&timer.map_id) {
                val.push(timer.clone());
            };
            let association = match &timer.association {
                Some(s) => format!("{}", s),
                None => "unassociated".to_string(),
            };
            // Handle id to timer file allocation
            log::trace!(
                "Set up {4} {0}: {3} for map {1}, category {2}",
                timer.id,
                timer.name.replace("\n", " "),
                timer.map_id,
                timer.category,
                association,
            );
        }
        log::info!("Set up {} timers.", self.timers.len());
        let _ = self
            .rt_sender
            .send(RenderEvent::TimerData(self.timers.clone()))
            .await;
    }

    async fn tick(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn mumblelink_tick(&mut self) -> anyhow::Result<()> {
        let Ok(mumble) = rt::mumble_link_ptr() else {
            return Ok(())
        };

        let ui_state = mumble.read_ui_state();

        let playpos = Vec3::from_array(mumble.read_avatar().position);

            #[cfg(feature = "markers")]
            {
                if let Some(map_id) = &self.map_id {
                    if let Some(markers_for_map) = self.map_id_to_markers.get(map_id) {
                        let mut new_spent_markers = Vec::new();
                        for marker in markers_for_map.difference(&self.spent_markers) {
                            if marker.trigger(playpos) {
                                new_spent_markers.push(marker.clone());
                            }
                        }
                        self.spent_markers.extend(new_spent_markers.clone());
                        for spent_marker in new_spent_markers {
                            log::debug!("Marker autoplace triggered for {}", spent_marker.name);
                            self.handle_marker_autoplace(&spent_marker).await?;
                        }
                    }
                }
            }
            self.player_position = Some(playpos);
            let combat_state = ui_state.contains(rt::UiState::IS_IN_COMBAT);
            if combat_state != self.previous_combat_state {
                if combat_state {
                    log::info!("MumbleLink: Combat begins at {:?}!", SystemTime::now());
                    for machine in &mut self.current_timers {
                        machine.set_combat_state(CombatState::Entered);
                    }
                } else {
                    log::info!("MumbleLink: Combat ends at {:?}!", SystemTime::now());
                    for machine in &mut self.current_timers {
                        machine.set_combat_state(CombatState::Exited);
                    }
                }
                self.previous_combat_state = combat_state;
            }
            if let Some(pos) = self.player_position() {
                for machine in &mut self.current_timers {
                    machine.tick(pos).await
                }
            }

            Ok(())
    }

    async fn handle_mumble_identity(&mut self, role: SquadRank) {
        #[cfg(feature = "markers")]
        {
            self.mumble_role = Some(role);
        }
    }

    async fn handle_map_event(&mut self, gameplay: GameplayState, _trans: GameplayTransition) {
        let new_map_id = match gameplay.gameplay_map() {
            None => {
                log::debug!("TODO: clear timers on loading screen? {_trans:?}");
                //self.map_id = None;
                return
            },
            Some(map_id) => map_id.get(),
        };
        if Some(new_map_id) != self.map_id {
            #[cfg(feature = "markers")]
            {
                let markers_for_map = self.map_id_to_markers.get(&new_map_id);
                let markers_for_map = match markers_for_map {
                    Some(s) => s.clone(),
                    None => Default::default(),
                };
                let event_markers = markers_for_map.into_iter().collect::<Vec<_>>();
                let _ = self
                    .rt_sender
                    .send(RenderEvent::MarkerMap(event_markers))
                    .await;
                self.spent_markers = Default::default();
            }
            for timer in &mut self.current_timers {
                timer.cleanup().await;
            }
            self.current_timers.clear();
            if self.map_id_to_timers.contains_key(&new_map_id) {
                let map_timers = &self.map_id_to_timers[&new_map_id];
                for timer in map_timers {
                    let settings_lock = self.settings.read().await;
                    let settings_for_timer = settings_lock.timers.get(&timer.id);
                    let timer_enabled = match settings_for_timer {
                        Some(setting) => !setting.disabled,
                        None => true,
                    };
                    if timer_enabled {
                        self.current_timers.push(TimerMachine::new(
                            timer.clone(),
                            self.alert_sem.clone(),
                            self.rt_sender.clone(),
                        ));
                    }
                    drop(settings_lock);
                }
                for machine in &mut self.current_timers {
                    machine.update_on_map(new_map_id)
                }
            }
            if self.map_id != None {
                self.commit_settings().await;
            }
            self.map_id = Some(new_map_id);
        }
    }

    async fn handle_combat_event(&mut self, src: arcdps::AgentOwned, evt: arcEvent) {
        let is_self = src.is_self != 0;
        if is_self {
            match &mut self.agent {
                Some(agent) if src.name != agent.name => {
                    log::info!("Character changed from {:?} to {:?}!", agent.name, src.name);
                    *agent = src;
                }
                Some(_agent) => (),
                None => {
                    log::info!("Character selected, {:?}!", src.name);
                    self.agent = Some(src);
                }
            };
        }
        use arcdps::StateChange;
        match evt.get_statechange() {
            StateChange::None => {}
            StateChange::EnterCombat => {
                log::info!("ArcDPS: Combat begins at {}!", evt.time);
                for machine in &mut self.current_timers {
                    machine.set_combat_state(CombatState::Entered);
                }
            }
            StateChange::ExitCombat => {
                log::info!("ArcDPS: Combat ends at {}!", evt.time);
                for machine in &mut self.current_timers {
                    machine.set_combat_state(CombatState::Exited);
                }
            }
            _ => (),
        }
    }

    async fn toggle_marker(&mut self, id: &str) {
        let mut settings_lock = self.settings_write().await;
        settings_lock.toggle_marker(id.to_string());
        drop(settings_lock);
    }

    async fn enable_marker(&mut self, id: &str) {
        let mut settings_lock = self.settings_write().await;
        settings_lock.enable_marker(id.to_string());
        drop(settings_lock);
    }

    async fn disable_marker(&mut self, id: &str) {
        let mut settings_lock = self.settings_write().await;
        settings_lock.disable_marker(id.to_string());
        drop(settings_lock);
    }

    async fn toggle_timer(&mut self, id: &str) {
        let mut settings_lock = self.settings_write().await;
        let disabled = settings_lock.toggle_timer(id.to_string());
        drop(settings_lock);
        match disabled {
            false => {
                if let Some(map_id) = self.map_id {
                    if let Some(timers_for_map) = &self.map_id_to_timers.get(&map_id) {
                        let timers = timers_for_map.iter().filter(|t| t.id == id);
                        for timer in timers {
                            log::debug!(
                                "Creating timer machine for {} as it has been enabled.",
                                timer.id
                            );
                            self.current_timers.push(TimerMachine::new(
                                timer.clone(),
                                self.alert_sem.clone(),
                                self.rt_sender.clone(),
                            ));
                        }
                    }
                }
            }
            true => {
                let timers_to_remove = self.current_timers.iter_mut().filter(|t| t.timer.id == id);
                for timer in timers_to_remove {
                    log::debug!(
                        "Starting cleanup for timer {} as it has been disabled.",
                        timer.timer.id
                    );
                    timer.cleanup().await;
                }
            }
        }
    }

    async fn enable_timer(&mut self, id: &str) {
        let mut settings_lock = self.settings_write().await;
        settings_lock.enable_timer(id.to_string());
        drop(settings_lock);
        if let Some(map_id) = self.map_id {
            if let Some(timers_for_map) = &self.map_id_to_timers.get(&map_id) {
                let timers = timers_for_map.iter().filter(|t| t.id == id);
                for timer in timers {
                    log::debug!("Creating timer machine for {}", timer.id);
                    self.current_timers.push(TimerMachine::new(
                        timer.clone(),
                        self.alert_sem.clone(),
                        self.rt_sender.clone(),
                    ));
                }
            }
        }
    }

    async fn disable_timer(&mut self, id: &str) {
        let mut settings_lock = self.settings_write().await;
        settings_lock.disable_timer(id.to_string());
        drop(settings_lock);
        let timers_to_remove = self.current_timers.iter_mut().filter(|t| t.timer.id == id);
        for timer in timers_to_remove {
            log::debug!("Starting cleanup for timer {}", timer.timer.id);
            timer.cleanup().await;
        }
        self.current_timers.retain(|t| t.timer.id != id);
    }

    async fn check_updates(&mut self) {
        let _ = self
            .rt_sender
            .send(RenderEvent::CheckingForUpdates(true))
            .await;
        match Settings::check_for_updates().await {
            Ok(_) => (),
            Err(err) => log::error!("Controller.check_updates(): {}", err),
        }
        let _ = self
            .rt_sender
            .send(RenderEvent::CheckingForUpdates(false))
            .await;
    }

    async fn save_settings(&mut self) {
        // avoid holding on to the lock for too long...
        let settings = self.settings.read().await.start_save().await;
        self.save_settings_internal(settings).await
    }

    async fn save_on_quit(&self) -> anyhow::Result<()> {
        let settings = timeout(Duration::from_secs(2), self.settings.read()).await;
        match settings {
            Ok(s) if s.is_dirty() => {
                log::info!("Saving settings on exit...");
                s.save().await
            },
            Ok(_) => Ok(()),
            Err(..) => Err(anyhow!("Read timeout")),
        }.context("Failed to save settings")
    }

    async fn save_settings_internal(&mut self, settings: anyhow::Result<SettingsSave>) {
        // avoid holding on to the lock for too long...
        let res = match settings {
            Ok(settings) => Settings::save_to(&settings).await,
            Err(e) => Err(e),
        }.context("Saving settings");
        match res {
            Ok(()) =>
                self.save_interval.reset(),
            Err(e) =>
                log::error!("{e:#}"),
        }
    }

    async fn commit_settings(&mut self) {
        let settings = match Settings::try_commit().await {
            Ok(Some(settings)) => Ok(settings),
            Ok(None) => return,
            Err(e) => Err(e),
        };

        self.save_settings_internal(settings).await
    }

    async fn reload_data(&mut self) {
        self.reload_timers().await;
        #[cfg(feature = "markers")]
        self.reload_markers().await;
    }

    async fn reload_timers(&mut self) {
        self.timers.clear();
        self.sources_to_timers.clear();
        self.map_id_to_timers.clear();
        self.setup_timers().await;
        self.reset_timers().await;
    }

    pub const KEY_INVOKE_DURATION: Duration = Duration::from_millis(50);

    #[cfg(feature = "markers")]
    async fn reload_markers(&mut self) {
        self.load_markers_files()
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
    #[cfg(feature = "markers")]
    async fn clear_markers(&self) {
        use crate::marker::format::MarkerType;

        if let Err(e) = rt::invoke_marker_bind(MarkerType::ClearMarkers, false, Self::KEY_INVOKE_DURATION, None).await {
            log::warn!("Failed to clear markers: {e}");
        }
    }

    #[cfg(feature = "markers")]
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

    #[cfg(feature = "markers")]
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

    #[cfg(feature = "markers")]
    fn set_marker(&self, markers: &MarkerSet) -> JoinHandle<anyhow::Result<()>> {
        tokio::spawn(Self::set_marker_task(
            markers.clone(),
            self.rt_sender.clone(),
        ))
    }

    #[cfg(feature = "markers")]
    async fn set_marker_task(
        markers: MarkerSet,
        rt_sender: Sender<crate::RenderEvent>,
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

    async fn do_update(&mut self, state: RemoteState) {
        let name = state.name();
        match Settings::download_latest(state).await {
            Ok(_) => (),
            Err(err) => log::error!("Controller.do_update() error for \"{}\": {}", name, err),
        };
        self.reload_timers().await;
    }

    async fn progress_bar_style(&mut self, style: ProgressBarStyleChange) {
        let mut settings_lock = self.settings_write().await;
        let settings = settings_lock.set_progress_bar(style);
        let _ = self
            .rt_sender
            .send(RenderEvent::ProgressBarUpdate(settings))
            .await;

        drop(settings_lock);
    }

    async fn set_window_state(&mut self, window: String, state: Option<bool>) {
        let mut settings_lock = self.settings_write().await;
        settings_lock.set_window_state(&window, state);
        drop(settings_lock);
    }

    async fn open_openable<T: AsRef<OsStr>>(&self, key: String, uri: T) {
        match open::that(uri) {
            Ok(_) => (),
            Err(err) => {
                let _ = self
                    .rt_sender
                    .send(RenderEvent::OpenableError(key, err.into()))
                    .await;
            }
        }
    }
    async fn toggle_katrender(&mut self) {
        let mut settings_lock = self.settings_write().await;
        settings_lock.toggle_katrender();
        drop(settings_lock);
    }

    async fn uninstall_addon(&mut self, source: &RemoteSource) -> anyhow::Result<()> {
        let mut settings_lock = self.settings_write().await;
        settings_lock.uninstall_remote(source).await?;
        drop(settings_lock);
        Ok(())
    }

    async fn timer_key_trigger(&mut self, id: String, is_release: bool) {
        let idx = id.chars().last().unwrap().to_digit(10).unwrap();
        for timer in &mut self.current_timers {
            timer.key_event(idx, is_release);
        }
    }

    async fn load_texture(&self, rel: RelativePathBuf, base: PathBuf) {
        if let Err(e) = rt::texture_schedule_path(rel.as_str(), &base).await {
            log::warn!("Cannot load texture {rel:?}: {e}");
        }
    }

    async fn reset_timers(&mut self) {
        for timer in &mut self.current_timers {
            timer.do_reset().await;
        }
    }

    #[cfg(feature = "markers-edit")]
    async fn save_marker(&mut self, e: MarkerSaveEvent) -> anyhow::Result<()> {
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
        self.reload_markers().await;
        Ok(())
    }

    #[cfg(feature = "markers-edit")]
    async fn delete_marker(
        &mut self,
        path: &PathBuf,
        category: Option<String>,
        idx: usize,
    ) -> anyhow::Result<()> {
        RuntimeMarkers::delete(path, category, idx).await?;
        self.reload_markers().await;
        Ok(())
    }

    #[cfg(feature = "markers-edit")]
    async fn get_marker_paths(&self) -> anyhow::Result<()> {
        let markers_dir = crate::ADDON_DIR.join("markers");
        let mut paths: Vec<PathBuf> = Vec::new();
        for path in RuntimeMarkers::get_paths(&markers_dir)? {
            paths.push(path?);
        }
        let _ = self
            .rt_sender
            .send(RenderEvent::GiveMarkerPaths(paths))
            .await;

        Ok(())
    }

    #[cfg(feature = "markers")]
    async fn set_marker_autoplace_settings(
        &mut self,
        maps: MarkerAutoPlaceSettings,
    ) -> anyhow::Result<()> {
        let mut settings_lock = self.settings_write().await;
        settings_lock.set_marker_autoplace_settings(&maps)?;
        drop(settings_lock);
        self.marker_autoplace = Some(maps);
        Ok(())
    }

    #[cfg(feature = "markers")]
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

    #[cfg(all(feature = "markers", feature = "extension-nexus"))]
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

    #[cfg(feature = "markers")]
    async fn extras_squad_update(&mut self, data: Vec<UserInfoOwned>) {
        self.extras_squad.clear();
        for datum in data {
            if let Some(account_name) = &datum.account_name {
                self.extras_squad.insert(account_name.clone(), datum);
            }
        }
    }

    #[cfg(feature = "markers")]
    async fn clear_spent_autoplace(&mut self) {
        self.spent_markers.clear();
    }

    async fn load_texture_integrated(&mut self, identifier: String, data: Vec<u8>) {
        if let Err(e) = rt::texture_schedule_bytes(&identifier[..], data).await {
            log::warn!("Cannot load texture {identifier:?}: {e}");
        }
    }

    #[cfg(feature = "space")]
    async fn pathing_state_update(&mut self, path: String, state: bool) {
        let mut settings_lock = self.settings_write().await;
        crate::settings::PathingSettings::pathing_state_update(&mut settings_lock, path, state).await;
        drop(settings_lock);

    }
    #[cfg(feature = "space")]
    async fn provide_disabled_paths(&self) {
        let settings_lock = self.settings.read().await;
        let disabled_paths = settings_lock.disabled_paths.clone();
        drop(settings_lock);
        if let Some(sender) = Engine::sender() {
            let _event_send = sender.send(SpaceEvent::DisabledPaths(disabled_paths)).await;
        }
    }
    #[cfg(feature = "space")]
    async fn pathing_load_all(&self) {
        let res = self.pathing_load_all_inner().await
            .context("Loading all paths");
        if let Err(e) = res {
            log::error!("{e}");
        }
    }

    #[cfg(feature = "space")]
    async fn pathing_load_all_inner(&self) -> anyhow::Result<()> {
        use tokio::fs::read_dir;

        let pathing_dir = crate::ADDON_DIR.join("pathing");
        if !exists(&pathing_dir).unwrap_or(false) {
            create_dir_all(&pathing_dir).await?;
        }

        let mut path_loads = tokio::task::JoinSet::new();

        log::info!("Pre-loading all paths...");
        let mut dir = read_dir(pathing_dir).await?;
        loop {
            let entry = match dir.next_entry().await {
                Ok(Some(e)) => e,
                Ok(None) => break,
                Err(e) => {
                    log::error!("Failed to list pathing files: {e}");
                    continue
                },
            };
            let name = entry.file_name().to_string_lossy().into_owned();
            let context = format!("Loading pathing pack {name}");
            log::debug!("{context}...");
            let path = entry.path();
            let is_taco = path.extension().map(|e| e.eq_ignore_ascii_case("taco") || e.eq_ignore_ascii_case("zip"));
            let loader = move || {
                let res = if path.is_file() || is_taco.unwrap_or(false) {
                    Self::pathing_load_taco(name, path)
                } else {
                    Self::pathing_load_dir(name, path)
                }.context(context);

                if let Err(e) = &res {
                    log::error!("Path load failed: {e:#}");
                }
                res.is_ok()
            };
            path_loads.spawn_blocking(loader);
        }

        tokio::spawn(async move {
            let mut disabled_paths_dirty = false;
            loop {
                let pack_load = path_loads.join_next();
                let res = if disabled_paths_dirty {
                    // throttle repeated state event if packs load quickly enough...
                    let timeout = sleep(Duration::from_millis(174)).fuse();
                    tokio::pin!(timeout);
                    tokio::pin!(pack_load);
                    loop {
                        select! {
                            res = &mut pack_load => break res,
                            _ = &mut timeout => {
                                // this will take a while, so emit the pending update
                                Self::try_send(ControllerEvent::RequestDisabledPaths);
                                disabled_paths_dirty = false;
                            },
                        }
                    }
                } else {
                    pack_load.await
                };
                match res {
                    None => break,
                    Some(Err(e)) =>
                        log::error!("Path load panicked: {e}"),
                    Some(Ok(true)) =>
                        disabled_paths_dirty = true,
                    Some(Ok(..)) => (),
                }
            }

            // TODO: sender+await, or ideally just make this unnecessary

            if disabled_paths_dirty {
                Self::try_send(ControllerEvent::RequestDisabledPaths);
            }
        });

        Ok(())
    }

    #[cfg(feature = "space")]
    fn pathing_load_taco(name: String, path: PathBuf) -> anyhow::Result<()> {
        use taimi_pack::loader::ZipLoader;
        let mut loader = ZipLoader::new(&path)?;
        let pack = Pack::load(&mut loader)?;
        Self::pathing_load_pack(pack, Box::new(loader), name);
        Ok(())
    }

    #[cfg(feature = "space")]
    fn pathing_load_dir(name: String, path: PathBuf) -> anyhow::Result<()> {
        use taimi_pack::loader::DirectoryLoader;
        let mut loader = DirectoryLoader::new(path);
        let pack = Pack::load(&mut loader)?;
        Self::pathing_load_pack(pack, Box::new(loader), name);
        Ok(())
    }

    #[cfg(feature = "space")]
    fn pathing_load_pack(mut pack: Pack, loader: LoaderBox, name: String) {
        if pack.name.is_empty() {
            pack.name = name;
        }
        let event = SpaceEvent::PackLoad {
            pack: Arc::new(pack),
            loader,
        };
        // TODO: await!
        if let Some(sender) = Engine::sender() {
            let _ = sender.blocking_send(event);
        }
    }

    async fn handle_event(&mut self, event: ControllerEvent) -> anyhow::Result<bool> {
        use ControllerEvent::*;
        match &event {
            // omit the worst spam offenders
            ControllerEvent::LoadTextureIntegrated(id, data) =>
                log::trace!("Controller received event: Load texture {id} from {} bytes", data.len()),
            ControllerEvent::CombatEvent { .. } | ControllerEvent::MumbleIdentityUpdated { .. }
            | ControllerEvent::WindowState(..)
            | ControllerEvent::UiTick(..)
                => log::trace!("Controller received event: {}", event),
            event =>
                log::debug!("Controller received event: {}", event),
        }

        match event {
            #[cfg(feature = "space")]
            PathingLoadAll => self.pathing_load_all().await,
            #[cfg(feature = "space")]
            RequestDisabledPaths => self.provide_disabled_paths().await,
            #[cfg(feature = "space")]
            PathingStateUpdate(p, s) => self.pathing_state_update(p, s).await,
            #[cfg(feature = "markers")]
            ClearSpentAutoplace => self.clear_spent_autoplace().await,
            #[cfg(feature = "markers")]
            MarkerEnable(id) => self.enable_marker(&id).await,
            #[cfg(feature = "markers")]
            MarkerDisable(id) => self.disable_marker(&id).await,
            #[cfg(feature = "markers")]
            MarkerToggle(id) => self.toggle_marker(&id).await,
            #[cfg(feature = "markers")]
            ExtrasSquadUpdate(members) => self.extras_squad_update(members).await,
            #[cfg(all(feature = "markers", feature = "extension-nexus"))]
            RTAPISquadUpdate(change, member) => self.rtapi_squad_update(change, member).await,
            #[cfg(feature = "markers")]
            ClearMarkers => self.clear_markers().await,
            ReloadData => self.reload_data().await,
            SaveSettings => self.save_settings().await,
            ReloadTimers => self.reload_timers().await,
            #[cfg(feature = "markers")]
            MarkerAutoPlaceSettings(maps) => self.set_marker_autoplace_settings(maps).await?,
            #[cfg(feature = "markers")]
            ReloadMarkers => self.reload_markers().await,
            ToggleKatRender => self.toggle_katrender().await,
            OpenOpenable(key, uri) => self.open_openable(key, uri).await,
            UninstallAddon(dd) => self.uninstall_addon(&dd).await?,
            MumbleIdentityUpdated { role } => self.handle_mumble_identity(role).await,
            CombatEvent { src, evt } => self.handle_combat_event(src, evt).await,
            TimerEnable(id) => self.enable_timer(&id).await,
            TimerDisable(id) => self.disable_timer(&id).await,
            TimerToggle(id) => self.toggle_timer(&id).await,
            TimerReset => self.reset_timers().await,
            CheckDataSourceUpdates => self.check_updates().await,
            CheckUpdateSources => self.check_sources().await?,
            #[cfg(feature = "markers")]
            SetMarker(t) => {
                self.set_marker(&t);
            }
            TimerKeyTrigger(id, is_release) => self.timer_key_trigger(id, is_release).await,
            DoDataSourceUpdate { state } => self.do_update(state).await,
            ProgressBarStyle(style) => self.progress_bar_style(style).await,
            WindowState(window, state) => self.set_window_state(window, state).await,
            LoadTexture(rel, base) => self.load_texture(rel, base).await,
            LoadTextureIntegrated(identifier, data) => {
                self.load_texture_integrated(identifier, data).await
            }
            #[cfg(feature = "markers-edit")]
            SaveMarker(e) => self.save_marker(e).await?,
            #[cfg(feature = "markers-edit")]
            DeleteMarker {
                path,
                category,
                idx,
            } => self.delete_marker(&path, category, idx).await?,
            #[cfg(feature = "markers-edit")]
            GetMarkerPaths => self.get_marker_paths().await?,
            UiTick(tick) => match tick.is_player() {
                #[cfg(todo)]
                false => (),
                _ => self.mumblelink_tick().await?,
            },
            #[cfg(feature = "markers")]
            UiResize(_calibration) => (),
            #[cfg(feature = "markers")]
            UiMapOpened(_open) => (),
            GameplayStatus {
                gameplay,
                trans,
            } => {
                self.handle_map_event(gameplay, trans).await;
                if gameplay.gameplay_map().is_some() {
                    // force immediate state update
                    self.mumblelink_tick().await?;
                }
            },
            Quit => {
                if let Err(e) = self.save_on_quit().await {
                    log::error!("{e:#}");
                }
                return Ok(false)
            },
            UnloadAll => {
                if let Err(e) = self.save_on_quit().await {
                    log::error!("{e:#}");
                }
                #[cfg(feature = "extension-arcdps")] {
                    use {
                        crate::exports::arcdps as exports,
                        std::thread,
                    };
                    let avail = exports::available();
                    if avail || exports::loaded() {
                        thread::spawn(move || {
                            if avail {
                                // wait a tiny bit to give render thread cleanup a chance
                                thread::sleep(Duration::from_millis(84));
                            }
                            // TODO: synchronize with controller shutdown in case it takes a while...
                            let res = exports::ExitHandle::try_exit()
                                .and_then(|exit| exit.ok_or("unloaded/unaware?"));
                            match res {
                                Err(e) =>
                                    log::error!("Failed to leave arcdps: {e}"),
                                Ok(exit) => {
                                    log::info!("goodbye arc");
                                    exit.spawn_free();
                                },
                            }
                        });
                    }
                }
                return Ok(false)
            },
            // I forget why we needed this, but I think it's a holdover from the buttplug one o:
            //_ => (),
        }
        Ok(true)
    }

    #[cfg(todo = "unused")]
    pub fn sender() -> Option<Sender<ControllerEvent>> {
        crate::CONTROLLER_SENDER.try_read()
            .as_ref().ok()
            .and_then(|s| (*s).clone())
    }

    pub fn try_send(e: ControllerEvent) {
        let sender = crate::CONTROLLER_SENDER.try_read();
        let sender = sender.as_ref().map(|s| &**s);
        if let Ok(Some(sender)) = sender {
            let _ = sender.try_send(e);
        }
    }
}

#[derive(Debug, Clone, Display)]
pub enum ProgressBarStyleChange {
    Centre(bool),
    Stock(bool),
    Shadow(bool),
    Height(f32),
    Font(TextFont),
}

#[derive(Debug, Clone, Display)]
pub enum MarkerSaveEvent {
    Append(MarkerSet, PathBuf),
    Create(MarkerSet, PathBuf, MarkerFiletype),
    Edit(MarkerSet, PathBuf, Option<String>, usize),
}

#[derive(Debug, Clone, Display)]
pub enum ControllerEvent {
    #[cfg(feature = "space")]
    PathingLoadAll,
    #[cfg(feature = "space")]
    RequestDisabledPaths,
    #[cfg(feature = "space")]
    PathingStateUpdate(String, bool),
    #[cfg(feature = "markers")]
    ClearSpentAutoplace,
    #[cfg(feature = "markers")]
    ExtrasSquadUpdate(Vec<UserInfoOwned>),
    #[cfg(all(feature = "markers", feature = "extension-nexus"))]
    RTAPISquadUpdate(SquadState, GroupMemberOwned),
    OpenOpenable(String, String),
    #[cfg(feature = "markers")]
    ClearMarkers,
    #[cfg(feature = "markers")]
    MarkerAutoPlaceSettings(MarkerAutoPlaceSettings),
    #[cfg(feature = "markers")]
    SetMarker(Arc<MarkerSet>),
    #[cfg(feature = "markers-edit")]
    SaveMarker(MarkerSaveEvent),
    #[cfg(feature = "markers-edit")]
    DeleteMarker {
        path: PathBuf,
        category: Option<String>,
        idx: usize,
    },
    #[cfg(feature = "markers-edit")]
    GetMarkerPaths,
    UninstallAddon(RemoteSource),
    MumbleIdentityUpdated {
        role: SquadRank,
    },
    ToggleKatRender,
    CombatEvent {
        src: arcdps::AgentOwned,
        evt: arcEvent,
    },
    DoDataSourceUpdate {
        state: RemoteState,
    },
    ProgressBarStyle(ProgressBarStyleChange),
    WindowState(String, Option<bool>),
    #[strum(to_string = "Id {0}, pressed {1}")]
    TimerKeyTrigger(String, bool),
    LoadTextureIntegrated(String, Vec<u8>),
    #[strum(to_string = "Load texture {0} from {1:?}")]
    LoadTexture(RelativePathBuf, PathBuf),
    CheckDataSourceUpdates,
    ReloadTimers,
    #[cfg(feature = "markers")]
    ReloadMarkers,
    ReloadData,
    SaveSettings,

    #[cfg(feature = "markers")]
    #[strum(to_string = "Toggled {0}")]
    MarkerToggle(String),
    #[cfg(feature = "markers")]
    #[allow(dead_code)]
    MarkerEnable(String),
    #[cfg(feature = "markers")]
    #[allow(dead_code)]
    MarkerDisable(String),

    UiTick(MumblelinkTick),
    #[cfg(feature = "markers")]
    UiResize(MapCalibration),
    #[cfg(feature = "markers")]
    UiMapOpened(MapOpen),
    GameplayStatus {
        gameplay: GameplayState,
        trans: GameplayTransition,
    },

    #[allow(dead_code)]
    TimerEnable(String),
    #[allow(dead_code)]
    TimerDisable(String),
    TimerReset,
    #[strum(to_string = "Toggled {0}")]
    TimerToggle(String),
    CheckUpdateSources,
    Quit,
    /// Like quit but will also request addon release
    /// (if possible)
    UnloadAll,
}
