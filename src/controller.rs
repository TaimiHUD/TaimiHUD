#[cfg(feature = "markers")]
use {
    crate::marker::{
        atomic::{CurrentPerspective, MarkerInputData, MinimapPlacement, ScreenPoint},
        format::{MarkerSet, RuntimeMarkers},
    },
    arcdps::extras::UserInfoOwned,
    tokio::task::JoinHandle,
    windows::Win32::{
        Foundation::POINT,
        Graphics::Gdi::ClientToScreen,
        UI::WindowsAndMessaging::{GetCursorPos, GetForegroundWindow},
    },
};
use {
    crate::{
        exports::runtime as rt,
        marker::{
            atomic::ScreenVector,
            format::{MarkerEntry, MarkerFiletype},
        },
        render::TextFont,
        settings::{MarkerAutoPlaceSettings, RemoteSource, Settings, SettingsLock, SourcesFile},
        timer::{CombatState, Position, TimerFile, TimerMachine},
        MumbleIdentityUpdate, RenderEvent, SETTINGS, SOURCES, TIMERS_DIR,
    },
    anyhow::anyhow,
    arcdps::{evtc::event::Event as arcEvent, AgentOwned},
    glam::{f32::Vec3, Vec2},
    nexus::{
        data_link::mumble::{MumblePtr, UiState},
        rtapi::GroupMemberOwned,
    },
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
        runtime, select,
        sync::{
            mpsc::{Receiver, Sender},
            Mutex,
        },
        time::{interval, sleep, Duration},
    },
    windows::Win32::{
        Foundation::GetLastError,
        UI::{
            Input::KeyboardAndMouse::{
                SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_LEFTDOWN,
                MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MOVE, MOUSEINPUT, MOUSE_EVENT_FLAGS,
            },
            WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN},
        },
    },
};

#[cfg(feature = "space")]
use crate::space::dx11::PerspectiveInputData;

#[derive(Debug, Clone)]
pub struct Controller {
    #[cfg(feature = "markers")]
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
    pub cached_identity: Option<MumbleIdentityUpdate>,
    pub mumble_pointer: Option<MumblePtr>,
    pub map_id: Option<u32>,
    pub player_position: Option<Vec3>,
    alert_sem: Arc<Mutex<()>>,
    pub timers: Vec<Arc<TimerFile>>,
    pub current_timers: Vec<TimerMachine>,
    pub sources_to_timers: HashMap<Arc<RemoteSource>, Vec<Arc<TimerFile>>>,
    pub map_id_to_timers: HashMap<u32, Vec<Arc<TimerFile>>>,
    settings: SettingsLock,
    last_fov: f32,
    scaling: f32,
}

impl Controller {
    pub fn player_position(&self) -> Option<Position> {
        self.player_position.map(Position::Vec3)
    }

    pub fn load(
        mut controller_receiver: Receiver<ControllerEvent>,
        rt_sender: Sender<crate::RenderEvent>,
        addon_dir: PathBuf,
    ) {
        let mumble_link = match rt::mumble_link_ptr() {
            Ok(p) => Some(p),
            Err(e) => {
                log::warn!("No MumbleLink? {e}");
                None
            },
        };
        let evt_loop = async move {
            let sources = SourcesFile::load()
                .await
                .expect("Couldn't load sources file");
            let sources = Arc::new(RwLock::new(sources));
            let _ = SOURCES.set(sources);
            let settings = Settings::load_access(&addon_dir.clone()).await;
            let mut state = Controller {
                #[cfg(feature = "markers")]
                rtapi_squad: Default::default(),
                #[cfg(feature = "markers")]
                extras_squad: Default::default(),
                #[cfg(feature = "markers")]
                marker_autoplace: Default::default(),
                last_fov: 0.0,
                previous_combat_state: Default::default(),
                rt_sender,
                settings,
                #[cfg(feature = "markers")]
                markers: Default::default(),
                #[cfg(feature = "markers")]
                map_id_to_markers: Default::default(),
                #[cfg(feature = "markers")]
                spent_markers: Default::default(),
                agent: Default::default(),
                cached_identity: Default::default(),
                mumble_pointer: mumble_link,
                map_id: Default::default(),
                player_position: Default::default(),
                alert_sem: Default::default(),
                timers: Default::default(),
                current_timers: Default::default(),
                sources_to_timers: Default::default(),
                map_id_to_timers: Default::default(),
                scaling: 0.0f32,
            };
            let _ = SETTINGS.set(state.settings.clone());
            let settings = SETTINGS.get().unwrap();
            let mut settings_lock = settings.write().await;
            settings_lock.handle_sources_changes();
            drop(settings_lock);
            let settings_lock = settings.read().await;
            state.marker_autoplace = Some(settings_lock.marker_autoplace.clone());
            drop(settings_lock);
            state.setup_timers().await;
            #[cfg(feature = "markers")]
            state.setup_markers().await;
            let mut taimi_interval = interval(Duration::from_millis(125));
            let mut mumblelink_interval = interval(Duration::from_millis(20));
            loop {
                select! {
                    evt = controller_receiver.recv() => match evt {
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
                    _ = mumblelink_interval.tick() => {
                        let _ = state.mumblelink_tick().await;
                    },
                    _ = taimi_interval.tick() => {
                        let _ = state.tick().await;
                    },
                }
            }
        };
        let rt = match runtime::Builder::new_current_thread().enable_all().build() {
            Ok(rt) => rt,
            Err(error) => {
                log::error!("Error! {}", error);
                return;
            }
        };
        rt.block_on(evt_loop);
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

    #[cfg(feature = "markers")]
    async fn open_marker_window(&self) {
        let mut settings_lock = self.settings.write().await;
        settings_lock.set_window_state("markers", Some(true)).await;
        drop(settings_lock);
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
        log::info!("Total loaded timers: {}", timers_len,);
        timers
    }

    async fn setup_timers(&mut self) {
        log::info!("Preparing to setup timers");
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
            log::info!(
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
        if let Some(mumble) = self.mumble_pointer {
            let playpos = Vec3::from_array(mumble.read_avatar().position);
            #[cfg(feature = "space")]
            {
                let camera = mumble.read_camera();
                let front = Vec3::from_array(camera.front);
                let pos = Vec3::from_array(camera.position);
                PerspectiveInputData::swap_camera(front, pos, playpos);
            }
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
                if let Ok(nexus_link) = rt::read_nexus_link() {
                    let scaling = nexus_link.scaling;
                    if self.scaling != scaling {
                        MarkerInputData::from_nexus(scaling);
                        self.scaling = scaling;
                    }
                }
                let ui_state = mumble.read_ui_state();
                let global_player_pos = Vec2::from(mumble.read_player_position());
                let global_map = Vec2::from(mumble.read_map_center());
                let compass_width = mumble.read_compass_width() as f32;
                let compass_height = mumble.read_compass_height() as f32;
                let compass_size = Vec2::new(compass_width, compass_height);
                let compass_rotation = mumble.read_compass_rotation();
                let map_scale = mumble.read_map_scale();
                let perspective = CurrentPerspective::from(ui_state.contains(UiState::IS_MAP_OPEN));
                let minimap_placement =
                    MinimapPlacement::from(ui_state.contains(UiState::IS_COMPASS_TOP_RIGHT));
                let rotation_enabled =
                    ui_state.contains(UiState::DOES_COMPASS_HAVE_ROTATION_ENABLED);
                MarkerInputData::from_tick(
                    playpos,
                    global_player_pos,
                    global_map,
                    compass_size,
                    compass_rotation,
                    map_scale,
                    perspective,
                    minimap_placement,
                    rotation_enabled,
                );
            }
            self.player_position = Some(playpos);
            let combat_state = mumble
                .read_context()
                .ui_state
                .contains(UiState::IS_IN_COMBAT);
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
        }
        Ok(())
    }

    async fn handle_mumble(&mut self, identity: MumbleIdentityUpdate) {
        #[cfg(feature = "space")]
        {
            if self.last_fov != identity.fov {
                PerspectiveInputData::swap_fov(identity.fov);
                self.last_fov = identity.fov;
            }
        }
        let new_map_id = identity.map_id;
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
                MarkerInputData::from_mapchange(new_map_id);
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
            self.map_id = Some(new_map_id);
        }
        self.cached_identity = Some(identity);
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
        let mut settings_lock = self.settings.write().await;
        let disabled = settings_lock.toggle_marker(id.to_string()).await;
        drop(settings_lock);
    }

    async fn enable_marker(&mut self, id: &str) {
        let mut settings_lock = self.settings.write().await;
        settings_lock.enable_marker(id.to_string()).await;
        drop(settings_lock);
    }

    async fn disable_marker(&mut self, id: &str) {
        let mut settings_lock = self.settings.write().await;
        settings_lock.disable_marker(id.to_string()).await;
        drop(settings_lock);
    }

    async fn toggle_timer(&mut self, id: &str) {
        let mut settings_lock = self.settings.write().await;
        let disabled = settings_lock.toggle_timer(id.to_string()).await;
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
        let mut settings_lock = self.settings.write().await;
        settings_lock.enable_timer(id.to_string()).await;
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
        let mut settings_lock = self.settings.write().await;
        settings_lock.disable_timer(id.to_string()).await;
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

        if let Err(e) = rt::invoke_marker_bind(MarkerType::ClearMarkers, false, 10i32) {
            log::warn!("Failed to clear markers: {e}");
        }
    }

    #[cfg(feature = "markers")]
    fn get_viewport_point(rel: Vec2) -> POINT {
        let hwnd = unsafe { GetForegroundWindow() };
        let mut abs: POINT = POINT {
            x: rel.x as i32,
            y: rel.y as i32,
        };
        unsafe {
            let _ = ClientToScreen(hwnd, &mut abs);
        }
        abs
    }

    #[cfg(feature = "markers")]
    fn get_viewport_coord(rel: Vec2) -> (i32, i32) {
        let point = Self::get_viewport_point(rel);
        (point.x, point.y)
    }

    #[cfg(feature = "markers")]
    fn get_abs_coord(rel: Vec2) -> (i32, i32) {
        let (x, y) = Self::get_viewport_coord(rel);
        let dx = (x * 65536) / unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let dy = (y * 65536) / unsafe { GetSystemMetrics(SM_CYSCREEN) };
        (dx, dy)
    }

    #[cfg(feature = "markers")]
    fn mouse_event(coords: (i32, i32), flags: MOUSE_EVENT_FLAGS) -> anyhow::Result<()> {
        let (dx, dy) = coords;
        let mousey = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx,
                    dy,
                    mouseData: 0,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        let inputs = [mousey];
        let result = unsafe { SendInput(&inputs, size_of_val(&inputs) as i32) };
        let error = unsafe { GetLastError() }.to_hresult();
        if error.0 != 0 {
            return Err(anyhow!("Error code: {}", error.0));
        }
        match result {
            0 => Err(anyhow!("mouse event blocked by another thread")),
            _ => Ok(()),
        }
    }

    #[cfg(feature = "markers")]
    fn move_cursor_pos(goal: Vec2) -> anyhow::Result<()> {
        let coords = Self::get_abs_coord(goal);
        Self::mouse_event(coords, MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE)
    }

    #[cfg(feature = "markers")]
    async fn drag_mouse_abs(from: Vec2, to: Vec2) -> anyhow::Result<()> {
        let wait_duration = Duration::from_millis(10);
        let from_abs = Self::get_abs_coord(from);
        let to_abs = Self::get_abs_coord(to);
        sleep(wait_duration).await;
        Self::mouse_event(from_abs, MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE)?;
        sleep(wait_duration).await;
        Self::mouse_event(from_abs, MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_LEFTDOWN)?;
        sleep(wait_duration).await;
        Self::mouse_event(to_abs, MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE)?;
        sleep(wait_duration).await;
        Self::mouse_event(to_abs, MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_LEFTUP)?;
        sleep(wait_duration).await;
        Ok(())
    }

    #[cfg(feature = "markers")]
    async fn drag_mouse_rel(from: ScreenPoint, amount: ScreenVector) -> anyhow::Result<()> {
        let wait_duration = Duration::from_millis(30);
        let from_abs = Self::get_abs_coord(from.into());

        let [amt_x, amt_y] = amount.as_array();
        let amount = (*amt_x as i32, *amt_y as i32);
        // bounds appear to mean that only y is actually capable of being subtracted, presumably
        // the distance from max_move in the x is negative
        // make sure the mouse is in the right place, and then put the mouse down
        Self::mouse_event(from_abs, MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE)?;
        sleep(wait_duration).await;
        Self::mouse_event(from_abs, MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_LEFTDOWN)?;
        sleep(wait_duration).await;
        Self::mouse_event(amount, MOUSEEVENTF_MOVE)?;
        sleep(wait_duration).await;
        Self::mouse_event((0i32, 0i32), MOUSEEVENTF_LEFTUP)?;
        sleep(wait_duration * 10).await;
        Ok(())
    }

    #[cfg(feature = "markers")]
    async fn place_marker(
        wait_duration: Duration,
        place_duration: i32,
        point: ScreenPoint,
        marker: &MarkerEntry,
    ) {
        sleep(wait_duration).await;
        match Self::move_cursor_pos(point.into()) {
            Ok(_) => (),
            Err(e) => log::error!("{}", e),
        }
        sleep(wait_duration).await;
        if let Err(e) = rt::invoke_marker_bind(marker.marker, false, place_duration) {
            log::warn!("Failed to place marker {:?}: {e}", marker.marker);
        }
    }

    #[cfg(feature = "markers")]
    async fn place_marker_from_map(
        wait_duration: Duration,
        place_duration: i32,
        point: Vec3,
        marker: &MarkerEntry,
    ) {
        use crate::marker::atomic::LocalPoint;
        let mid = MarkerInputData::read();
        if let Some(mid) = mid {
            let point: LocalPoint = Vec3::from(marker.position.clone()).into();
            let point = mid.map_local_to_map(point);
            let point = mid.map_map_to_screen(point);
            if let Some(point) = point {
                Self::place_marker(wait_duration, 10i32, point, marker).await;
            }
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
            crate::marker::atomic::{LocalPoint, MapPoint},
            anyhow::anyhow,
            glamour::TransformMap,
            windows::Win32::Graphics::Gdi::ScreenToClient,
        };
        if let Some(mid) = MarkerInputData::read() {
            let player_position = mid.local_player_pos;
            let mut too_far = false;
            for marker in &markers.markers {
                if player_position.distance(marker.position.clone().into()) >= 127.0 {
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
        let mut pos_ptr: POINT = POINT::default();
        let original_position = unsafe {
            let hwnd = GetForegroundWindow();
            let pos = GetCursorPos(&mut pos_ptr);
            let _ = ScreenToClient(hwnd, &mut pos_ptr);
            pos
        }
        .map_err(anyhow::Error::from)
        .map(|()| pos_ptr)?;
        for marker in &markers.markers {
            // check if it is possible to place immediately
            let local_point: LocalPoint = Vec3::from(marker.position.clone()).into();
            let (map_point, screen_point) = if let Some(mid) = MarkerInputData::read() {
                let map_point = mid.map_local_to_map(local_point);
                let screen_point = mid.map_map_to_screen(map_point);
                (Some(map_point), screen_point)
            } else {
                (None, None)
            };
            match screen_point {
                // if the marker is on the map, that's fine, place it
                Some(point) => {
                    Self::place_marker(wait_duration, 10i32, point, marker).await;
                }
                // if the marker isn't on the map, we need to get our perspective to include
                // the marker
                None => {
                    if let Some(map_point) = map_point {
                        let max_attempts = 10; // inshallah
                        let mut attempts = 0;
                        let map_centre: Option<MapPoint> =
                            MarkerInputData::read().map(|mid| mid.global_map.into());
                        log::debug!("Reached none arm for marker placement");
                        if let Some(mut map_centre) = map_centre {
                            while (map_centre.distance(map_point) > 5.0)
                                && (attempts < max_attempts)
                            {
                                log::debug!("Attempt {}/{}", attempts, max_attempts);
                                if let Some(mid) = MarkerInputData::read() {
                                    let bounds = mid.screen_bound();
                                    map_centre = mid.global_map.into();
                                    let remaining_distance = map_centre.distance(map_point);
                                    log::debug!("Remaining distance: {}", remaining_distance);
                                    let drag_from = mid.random_map_screen_coordinate();
                                    let difference_map = map_point - map_centre;
                                    let difference_fake = mid.map_to_fake_tf().map(difference_map);
                                    let difference_screen =
                                        mid.screen_to_fake().inverse().map(difference_fake);

                                    // the l
                                    let (min, max) = (bounds.min(), bounds.max());
                                    let drag_res = drag_from - difference_screen;
                                    let drag_res = drag_res.clamp(min, max);
                                    log::debug!(
                                        "Map centre: {:?}, destination: {:?}",
                                        map_centre,
                                        map_point
                                    );
                                    log::debug!("Min: {:?}, max: {:?}", min, max);
                                    log::debug!(
                                        "Attempting a drag from {:?} to {:?}",
                                        drag_from,
                                        drag_res
                                    );
                                    Self::drag_mouse_abs(drag_from.into(), drag_res.into()).await?;
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
                                break;
                            } else {
                                Self::place_marker_from_map(
                                    wait_duration,
                                    10i32,
                                    marker.position.clone().into(),
                                    marker,
                                )
                                .await;
                            }
                        }
                    }
                }
                _ => unreachable!("set_marker: this should not happen!"),
            }
        }
        sleep(wait_duration).await;
        let original_position = Vec2::new(original_position.x as f32, original_position.y as f32);
        Self::move_cursor_pos(original_position)?;
        Ok(())
    }

    async fn do_update(&mut self, source: &RemoteSource) {
        match Settings::download_latest(source).await {
            Ok(_) => (),
            Err(err) => log::error!("Controller.do_update() error for \"{}\": {}", source, err),
        };
        self.reload_timers().await;
    }

    async fn progress_bar_style(&mut self, style: ProgressBarStyleChange) {
        let mut settings_lock = self.settings.write().await;
        let settings = settings_lock.set_progress_bar(style).await;
        let _ = self
            .rt_sender
            .send(RenderEvent::ProgressBarUpdate(settings))
            .await;

        drop(settings_lock);
    }

    async fn set_window_state(&mut self, window: String, state: Option<bool>) {
        let mut settings_lock = self.settings.write().await;
        settings_lock.set_window_state(&window, state).await;
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
        let mut settings_lock = self.settings.write().await;
        settings_lock.toggle_katrender().await;
        drop(settings_lock);
    }

    async fn uninstall_addon(&mut self, source: &RemoteSource) -> anyhow::Result<()> {
        let mut settings_lock = self.settings.write().await;
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
        if let Some(base) = base.parent() {
            let abs = rel.to_path(base);
            if let Err(e) = rt::texture_schedule_path(rel.as_str(), &abs) {
                log::warn!("Cannot load texture {rel:?}: {e}");
            }
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
        let mut settings_lock = self.settings.write().await;
        settings_lock.set_marker_autoplace_settings(&maps).await?;
        self.marker_autoplace = Some(maps);
        drop(settings_lock);
        Ok(())
    }

    #[cfg(feature = "markers")]
    async fn get_role(&self) -> Option<SquadRoleState> {
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
            use {crate::ACCOUNT_NAME_CELL, arcdps::extras::user::UserRole};
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
        if let Some(identity) = &self.cached_identity {
            if identity.is_commander {
                return Some(SquadRoleState::Commander);
            }
        }
        None
    }

    #[cfg(feature = "markers")]
    async fn rtapi_squad_update(&mut self, change: SquadState, member: GroupMemberOwned) {
        use crate::ACCOUNT_NAME_CELL;

        let account_name = ACCOUNT_NAME_CELL.get();
        if let Some(account_name) = account_name {
            match change {
                SquadState::Left => {
                    if member.account_name == *account_name {
                        self.rtapi_squad.clear();
                    } else {
                        self.rtapi_squad.remove(&member.account_name);
                    }
                }
                SquadState::Joined => {
                    self.rtapi_squad.insert(member.account_name.clone(), member);
                }
                SquadState::Update => {
                    if let Some(entry) = self.rtapi_squad.get_mut(&member.account_name) {
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
        if let Err(e) = rt::texture_schedule_bytes(&identifier, data) {
            log::warn!("Cannot load texture {identifier:?}: {e}");
        }
    }

    async fn handle_event(&mut self, event: ControllerEvent) -> anyhow::Result<bool> {
        use ControllerEvent::*;
        log::debug!("Controller received event: {}", event);
        match event {
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
            #[cfg(feature = "markers")]
            RTAPISquadUpdate(change, member) => self.rtapi_squad_update(change, member).await,
            #[cfg(feature = "markers")]
            ClearMarkers => self.clear_markers().await,
            ReloadData => self.reload_data().await,
            ReloadTimers => self.reload_timers().await,
            #[cfg(feature = "markers")]
            MarkerAutoPlaceSettings(maps) => self.set_marker_autoplace_settings(maps).await?,
            #[cfg(feature = "markers")]
            ReloadMarkers => self.reload_markers().await,
            ToggleKatRender => self.toggle_katrender().await,
            OpenOpenable(key, uri) => self.open_openable(key, uri).await,
            UninstallAddon(dd) => self.uninstall_addon(&dd).await?,
            MumbleIdentityUpdated(identity) => self.handle_mumble(identity).await,
            CombatEvent { src, evt } => self.handle_combat_event(src, evt).await,
            TimerEnable(id) => self.enable_timer(&id).await,
            TimerDisable(id) => self.disable_timer(&id).await,
            TimerToggle(id) => self.toggle_timer(&id).await,
            TimerReset => self.reset_timers().await,
            CheckDataSourceUpdates => self.check_updates().await,
            #[cfg(feature = "markers")]
            SetMarker(t) => {
                self.set_marker(&t);
            }
            TimerKeyTrigger(id, is_release) => self.timer_key_trigger(id, is_release).await,
            DoDataSourceUpdate { source } => self.do_update(&source).await,
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
            Quit => return Ok(false),
            // I forget why we needed this, but I think it's a holdover from the buttplug one o:
            //_ => (),
        }
        Ok(true)
    }
}

#[derive(Ord, PartialOrd, Eq, PartialEq, Debug, Clone, Display)]
pub enum SquadRoleState {
    Member,
    Lieutenant,
    Commander,
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
pub enum SquadState {
    Update,
    Joined,
    Left,
}

#[derive(Debug, Clone, Display)]
pub enum ControllerEvent {
    #[cfg(feature = "markers")]
    ClearSpentAutoplace,
    #[cfg(feature = "markers")]
    ExtrasSquadUpdate(Vec<UserInfoOwned>),
    #[cfg(feature = "markers")]
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
    UninstallAddon(Arc<RemoteSource>),
    MumbleIdentityUpdated(MumbleIdentityUpdate),
    ToggleKatRender,
    CombatEvent {
        src: arcdps::AgentOwned,
        evt: arcEvent,
    },
    DoDataSourceUpdate {
        source: Arc<RemoteSource>,
    },
    ProgressBarStyle(ProgressBarStyleChange),
    WindowState(String, Option<bool>),
    #[strum(to_string = "Id {0}, pressed {1}")]
    TimerKeyTrigger(String, bool),
    LoadTextureIntegrated(String, Vec<u8>),
    LoadTexture(RelativePathBuf, PathBuf),
    CheckDataSourceUpdates,
    ReloadTimers,
    #[cfg(feature = "markers")]
    ReloadMarkers,
    ReloadData,

    #[cfg(feature = "markers")]
    #[strum(to_string = "Toggled {0}")]
    MarkerToggle(String),
    #[cfg(feature = "markers")]
    #[allow(dead_code)]
    MarkerEnable(String),
    #[cfg(feature = "markers")]
    #[allow(dead_code)]
    MarkerDisable(String),

    #[allow(dead_code)]
    TimerEnable(String),
    #[allow(dead_code)]
    TimerDisable(String),
    TimerReset,
    #[strum(to_string = "Toggled {0}")]
    TimerToggle(String),
    Quit,
}
