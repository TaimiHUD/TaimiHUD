use {
    crate::{
        exports::runtime::{
            self as rt,
            bindings::{ControlsReceiver, TaimiControls, TaimiReceiver, CONTROLS},
        },
        render::machine::MumblelinkTick,
        settings::{
            state::{BootstrapState, SaveState},
            RemoteSource,
            RemoteState,
            Settings,
            SettingsLock,
            SettingsSave,
            SourcesFile,
        },
        timer::{CombatState, Position},
        RenderEvent,
        SETTINGS,
        SOURCES,
    },
    anyhow::{anyhow, Context},
    arcdps::{evtc::event::Event as arcEvent, AgentOwned},
    glam::f32::Vec3,
    relative_path::RelativePathBuf,
    std::{
        ffi::OsStr,
        path::PathBuf,
        sync::{Arc, RwLock},
        time::SystemTime,
    },
    strum_macros::Display,
    tokio::{
        select,
        sync::{
            mpsc::{Receiver, Sender},
            watch,
            Mutex,
        },
        time::{interval, timeout, Duration},
    },
};

mod generic;

#[cfg(feature = "timers")]
pub(crate) mod timers;

use taimi_meta::ui::{gameplay::GameplayTransition, GameplayState};
#[cfg(feature = "timers")]
use timers::{TimersController, TimersEvent};

#[cfg(feature = "markers")]
pub(crate) mod markers;

#[cfg(feature = "markers")]
use markers::{MarkersController, MarkersEvent};

#[cfg(feature = "space")]
pub(crate) mod pathing;

#[cfg(feature = "space")]
use pathing::{PathingController, PathingEvent};

pub(crate) mod runtime;

pub(crate) type MapId = Option<u32>;
pub(crate) type RtSender = Arc<Sender<RenderEvent>>;

#[derive(Debug)]
pub struct Controller {
    receiver: Receiver<ControllerEvent>,
    pub agent: Option<AgentOwned>,
    pub previous_combat_state: bool,
    pub rt_sender: RtSender,
    pub map_id: MapId,
    pub player_position: Option<Vec3>,
    // TODO: remove!
    alert_sem: Arc<Mutex<()>>,
    settings: SettingsLock,
    state_bootstrap: watch::Receiver<BootstrapState>,
    state_bootstrap_throttle: rt::watched::WatchThrottleDelay,
    state_save: watch::Receiver<SaveState>,
    state_save_throttle: rt::watched::WatchThrottleDelay,
    save_interval: tokio::time::Interval,
    controls: ControlsReceiver,
    keybinds: TaimiReceiver,

    timers: TimersController,
    markers: MarkersController,
    pathing: PathingController,
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
            previous_combat_state: Default::default(),
            rt_sender: Arc::new(rt_sender),
            settings,
            state_bootstrap: BootstrapState::get().subscribe(),
            state_bootstrap_throttle: BootstrapState::watch_initial_delay(),
            state_save: SaveState::get().subscribe(),
            state_save_throttle: SaveState::watch_initial_delay(),
            agent: Default::default(),
            map_id: Default::default(),
            player_position: Default::default(),
            alert_sem: Default::default(),
            save_interval: interval(Duration::from_secs(60 * 10)),
            controls: CONTROLS.subscribe_controls(),
            keybinds: CONTROLS.subscribe_taimi(),

            timers: Default::default(),
            markers: Default::default(),
            pathing: Default::default(),
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
            },
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
        let sources = SourcesFile::load().await.expect("Couldn't load sources file");
        let sources = Arc::new(RwLock::new(sources));
        let _ = SOURCES.set(sources);
        let state = self;
        let _ = SETTINGS.set(state.settings.clone());
        #[cfg(feature = "timers")]
        state
            .timers
            .setup(state.settings.clone(), state.rt_sender.clone())
            .await;
        #[cfg(feature = "markers")]
        state
            .markers
            .setup(state.settings.clone(), state.rt_sender.clone())
            .await;

        state.render_inherit();

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
                _ = state.save_interval.tick() => {
                    state.commit_settings().await;
                },
                Ok(()) = BootstrapState::watch_dirty(&mut state.state_bootstrap, &mut state.state_bootstrap_throttle) => {
                    let state = state.state_bootstrap.borrow_and_update();
                    if let Err(e) = Self::commit_state_bootstrap(state).await {
                        log::error!("{e:#}");
                    }
                },
                Ok(()) = SaveState::watch_dirty(&mut state.state_save, &mut state.state_save_throttle) => {
                    let state = state.state_save.borrow_and_update();
                    if let Err(e) = Self::commit_state_save(state).await {
                        log::error!("{e:#}");
                    }
                },
                controls = state.controls.wait() => match controls {
                    Err(e) => log::error!("Control bindings error! {e:#}"),
                    Ok((&controls_state, controls_changed)) => {
                        #[cfg(feature = "space")]
                        state.pathing.handle_presses(controls_state, controls_changed).await;
                    },
                },
                keybinds = state.keybinds.wait() => match keybinds {
                    Err(e) => log::error!("Keybind receive error! {e:#}"),
                    Ok((binds_state, binds_changed)) => {
                        state.handle_keybinds(binds_state, binds_changed).await;
                    },
                },
            }
        }
    }

    async fn handle_keybinds(&mut self, state: TaimiControls, changed: TaimiControls) {
        let pressed = state & changed;

        if pressed.intersects(TaimiControls::WINDOW_PRIMARY) {
            CONTROLS.notify_handled(TaimiControls::WINDOW_PRIMARY);
            self.set_window_state(crate::WINDOW_PRIMARY, None).await;
        }
        #[cfg(feature = "markers")]
        if pressed.intersects(TaimiControls::WINDOW_MARKERS) {
            CONTROLS.notify_handled(TaimiControls::WINDOW_MARKERS);
            self.set_window_state(crate::WINDOW_MARKERS, None).await;
        }
        #[cfg(feature = "timers")]
        if pressed.intersects(TaimiControls::WINDOW_TIMERS) {
            CONTROLS.notify_handled(TaimiControls::WINDOW_TIMERS);
            self.set_window_state(crate::WINDOW_TIMERS, None).await;
        }
        #[cfg(feature = "space")]
        if pressed.intersects(TaimiControls::WINDOW_PATHING) {
            CONTROLS.notify_handled(TaimiControls::WINDOW_PATHING);
            self.set_window_state(crate::WINDOW_PATHING, None).await;
        }

        #[cfg(feature = "timers")]
        self.timers.handle_keybinds(state, changed).await;
        #[cfg(feature = "space")]
        self.pathing.handle_keybinds(state, changed).await;
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

    async fn check_sources(&self) -> anyhow::Result<()> {
        SourcesFile::get_sources().await?;
        let mut settings_lock = self.settings_write().await;
        settings_lock.remotes = RemoteState::suggested_sources().unwrap_or_default();
        drop(settings_lock);
        let sources = SourcesFile::load().await?;
        let _ = SOURCES.set(Arc::new(RwLock::new(sources)));
        Ok(())
    }

    const ADDON_UPDATE_CHECK_TIMEOUT: Duration = Duration::from_secs(20);
    async fn addon_check_for_updates(&self, proceed: bool) {
        tokio::spawn(async move {
            let res = rt::update::ResolvedVersion::latest_release(Self::ADDON_UPDATE_CHECK_TIMEOUT)
                .await
                .and_then(|release| {
                    rt::update::Updater::notify_latest(&release).map(|auth| (release, auth))
                })
                .context("Checking for addon updates");
            match res {
                Ok((release, auth)) =>
                    if auth && proceed {
                        let res = rt::update::Updater::perform(&release)
                            .await
                            .map_err(anyhow::Error::msg)
                            .context("Updating addon");
                        if let Err(e) = res {
                            log::error!("{e:#}");
                        }
                    },
                Err(e) => log::error!("{e:#}"),
            }
        });
    }

    async fn mumblelink_tick(&mut self) -> anyhow::Result<()> {
        let Ok(mumble) = rt::mumble_link_ptr() else { return Ok(()) };

        let ui_state = mumble.read_ui_state();

        let playpos = Vec3::from_array(mumble.read_avatar().position);
        #[cfg(feature = "markers")]
        self.markers
            .handle_position(self.map_id, playpos.clone(), self.rt_sender.clone())
            .await?;
        self.player_position = Some(playpos);

        let combat_state = ui_state.contains(rt::UiState::IS_IN_COMBAT);
        if combat_state != self.previous_combat_state {
            let cbt = match combat_state {
                true => {
                    log::debug!("MumbleLink: Combat begins at {:?}!", SystemTime::now());
                    CombatState::Entered
                },
                false => {
                    log::debug!("MumbleLink: Combat ends at {:?}!", SystemTime::now());
                    CombatState::Exited
                },
            };
            #[cfg(feature = "timers")]
            self.timers.handle_combat_event(cbt).await;
            self.previous_combat_state = combat_state;
        }
        if let Some(position) = self.player_position() {
            #[cfg(feature = "timers")]
            self.timers.handle_position(position).await;
        }

        Ok(())
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
            log::info!("Map changed from {:?} to {}", self.map_id, new_map_id);
            #[cfg(feature = "markers")]
            self.markers
                .handle_map_event(new_map_id, self.rt_sender.clone())
                .await;
            #[cfg(feature = "timers")]
            self.timers
                .handle_map_event(
                    self.settings.clone(),
                    self.alert_sem.clone(),
                    new_map_id,
                    self.rt_sender.clone(),
                )
                .await;
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
                },
                Some(_agent) => (),
                None => {
                    log::info!("Character selected, {:?}!", src.name);
                    self.agent = Some(src);
                },
            };
        }
        use arcdps::StateChange;
        let propagate = match evt.get_statechange() {
            StateChange::None => None,
            StateChange::EnterCombat => {
                log::debug!("ArcDPS: Combat begins at {}!", evt.time);
                Some(CombatState::Entered)
            },
            StateChange::ExitCombat => {
                log::debug!("ArcDPS: Combat ends at {}!", evt.time);
                Some(CombatState::Exited)
            },
            _ => None,
        };
        match propagate {
            Some(cbt) => self.timers.handle_combat_event(cbt).await,
            None => (),
        }
    }

    async fn check_updates(&mut self, everything: bool) {
        let _ = self.rt_sender.send(RenderEvent::CheckingForUpdates(true)).await;
        let res = Settings::check_for_updates(everything)
            .await
            .context("Controller.check_updates");
        match res {
            Ok(_) => (),
            Err(err) => log::error!("{err:#}"),
        }
        let _ = self.rt_sender.send(RenderEvent::CheckingForUpdates(false)).await;
    }

    async fn save_settings(&mut self) {
        // avoid holding on to the lock for too long...
        let settings = self.settings.read().await.start_save().await;
        self.save_settings_internal(settings).await
    }

    async fn save_on_quit(&self) -> anyhow::Result<()> {
        let state_bootstrap = &self.state_bootstrap;
        let save_state_bootstrap =
            async move { Self::commit_state_bootstrap(state_bootstrap.borrow()).await };
        let state_save = &self.state_save;
        let save_state_save = async move { Self::commit_state_save(state_save.borrow()).await };

        let settings = &self.settings;
        let save_settings = async move {
            let settings = timeout(Duration::from_secs(2), settings.read()).await;
            match settings {
                Ok(s) if s.is_dirty() => {
                    log::info!("Saving settings on exit...");
                    s.save().await
                },
                Ok(_) => Ok(()),
                Err(..) => Err(anyhow!("Read timeout")),
            }
            .context("Failed to save settings")
        };

        tokio::try_join!(save_state_bootstrap, save_state_save, save_settings,).map(drop)
    }

    async fn save_settings_internal(&mut self, settings: anyhow::Result<SettingsSave>) {
        // avoid holding on to the lock for too long...
        let res = match settings {
            Ok(settings) => Settings::save_to(&settings).await,
            Err(e) => Err(e),
        }
        .context("Saving settings");
        match res {
            Ok(()) => self.save_interval.reset(),
            Err(e) => log::error!("{e:#}"),
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

    async fn commit_state_bootstrap(state: watch::Ref<'_, BootstrapState>) -> anyhow::Result<()> {
        let save = match state {
            state if !state.has_changed() => return Ok(()),
            state => {
                let save = state.start_save()?;
                drop(state);
                save
            },
        };
        BootstrapState::save_to(&save).await.context("Saving boot state")
    }
    async fn commit_state_save(state: watch::Ref<'_, SaveState>) -> anyhow::Result<()> {
        let save = match state {
            state if !state.has_changed() => return Ok(()),
            state => {
                let save = state.start_save()?;
                drop(state);
                save
            },
        };
        SaveState::save_to(&save).await.context("Saving save state")
    }

    async fn reload_data(&mut self) {
        #[cfg(feature = "timers")]
        self.timers
            .reload(self.settings.clone(), self.rt_sender.clone())
            .await;
        #[cfg(feature = "markers")]
        self.markers.reload(self.rt_sender.clone()).await;
    }

    async fn do_update(&mut self, state: RemoteState) {
        let name = state.name();
        match Settings::download_latest(state).await {
            Ok(_) => (),
            Err(err) => log::error!("Controller.do_update() error for \"{}\": {}", name, err),
        };
        self.timers
            .reload(self.settings.clone(), self.rt_sender.clone())
            .await;
    }

    async fn set_window_state(&mut self, window: &str, state: Option<bool>) {
        let mut settings_lock = self.settings_write().await;
        settings_lock.set_window_state(window, state);
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
            },
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

    async fn load_texture(&self, rel: RelativePathBuf, base: PathBuf) {
        if let Err(e) = rt::texture_schedule_path(rel.as_str(), &base).await {
            log::warn!("Cannot load texture {rel:?}: {e}");
        }
    }

    async fn load_texture_integrated(&mut self, identifier: String, data: Vec<u8>) {
        if let Err(e) = rt::texture_schedule_bytes(&identifier[..], data).await {
            log::warn!("Cannot load texture {identifier:?}: {e}");
        }
    }

    async fn handle_event(&mut self, event: ControllerEvent) -> anyhow::Result<bool> {
        use ControllerEvent::*;
        match &event {
            // omit the worst spam offenders
            ControllerEvent::LoadTextureIntegrated(id, data) => log::trace!(
                "Controller received event: Load texture {id} from {} bytes",
                data.len()
            ),
            ControllerEvent::CombatEvent { .. }
            | ControllerEvent::WindowState(..)
            | ControllerEvent::UiTick(..) => log::trace!("Controller received event: {}", event),
            event => log::debug!("Controller received event: {}", event),
        }

        match event {
            #[cfg(feature = "timers")]
            Timers(evt) =>
                self.timers
                    .handle_event(evt, &self.settings, &self.alert_sem, self.map_id, &self.rt_sender)
                    .await,
            #[cfg(feature = "markers")]
            Markers(evt) => self.markers.handle_event(evt, &self.rt_sender).await?,
            #[cfg(feature = "markers")]
            Pathing(evt) => self.pathing.handle_event(evt, &self.settings).await,

            ReloadData => self.reload_data().await,
            SaveSettings => self.save_settings().await,
            OpenOpenable(key, uri) => self.open_openable(key, uri).await,
            UninstallAddon(dd) => self.uninstall_addon(&dd).await?,
            CombatEvent { src, evt } => self.handle_combat_event(src, evt).await,
            CheckDataSourceUpdates(everything) => self.check_updates(everything).await,
            CheckUpdateSources => self.check_sources().await?,
            CheckAddonUpdate(proceed) => self.addon_check_for_updates(proceed).await,
            DoDataSourceUpdate { state } => self.do_update(state).await,
            WindowState(window, state) => self.set_window_state(&window, state).await,
            LoadTexture(rel, base) => self.load_texture(rel, base).await,
            LoadTextureIntegrated(identifier, data) => self.load_texture_integrated(identifier, data).await,
            UiTick(tick) => match tick.is_player() {
                #[cfg(todo)]
                false => (),
                _ => self.mumblelink_tick().await?,
            },
            GameplayStatus { gameplay, trans } => {
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
                #[cfg(feature = "extension-arcdps")]
                {
                    use {crate::exports::arcdps as exports, std::thread};
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
                                Err(e) => log::error!("Failed to leave arcdps: {e}"),
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
        crate::CONTROLLER_SENDER
            .try_read()
            .as_ref()
            .ok()
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

/*
*/

/*
enum GenericEvent {
    WindowState(String, Option<bool>),
    OpenOpenable(String, String),
    UninstallAddon(RemoteSource),
    CombatEvent {
        src: arcdps::AgentOwned,
        evt: arcEvent,
    },
    DoDataSourceUpdate {
        state: RemoteState,
    },
    LoadTextureIntegrated(String, Vec<u8>),
    #[strum(to_string = "Load texture {0} from {1:?}")]
    LoadTexture(RelativePathBuf, PathBuf),
    CheckDataSourceUpdates,
    ReloadData,
    SaveSettings,
    UiTick(MumblelinkTick),
    GameplayStatus {
        gameplay: GameplayState,
        trans: GameplayTransition,
    },
    CheckUpdateSources,
    Quit,
    /// Like quit but will also request addon release
    /// (if possible)
    UnloadAll,
}
*/

#[derive(Debug, Clone, Display)]
pub enum ControllerEvent {
    /*Generic(GenericEvent),*/
    #[cfg(feature = "timers")]
    Timers(TimersEvent),
    #[cfg(feature = "markers")]
    Markers(MarkersEvent),
    #[cfg(feature = "space")]
    Pathing(PathingEvent),

    // TODO: remove as porting happens - Generic
    WindowState(String, Option<bool>),
    OpenOpenable(String, String),
    UninstallAddon(RemoteSource),
    CombatEvent {
        src: arcdps::AgentOwned,
        evt: arcEvent,
    },
    DoDataSourceUpdate {
        state: RemoteState,
    },
    LoadTextureIntegrated(String, Vec<u8>),
    #[strum(to_string = "Load texture {0} from {1:?}")]
    LoadTexture(RelativePathBuf, PathBuf),
    CheckDataSourceUpdates(bool),
    ReloadData,
    SaveSettings,
    UiTick(MumblelinkTick),
    GameplayStatus {
        gameplay: GameplayState,
        trans: GameplayTransition,
    },
    CheckUpdateSources,
    CheckAddonUpdate(bool),
    Quit,
    /// Like quit but will also request addon release
    /// (if possible)
    UnloadAll,
}
