#![allow(irrefutable_let_patterns)]

use {
    self::api::{ApiController, ApiMessage, ApiReceiver, ApiSender},
    crate::{
        exports::runtime::{
            self as rt,
            bindings::{TaimiControls, TaimiReceiver, CONTROLS},
        },
        log_join_error,
        render::{machine::MumblelinkTick, RenderEvent, RenderState},
        settings::{
            state::{BootstrapState, SaveState},
            DeserializedSource,
            RemoteState,
            Settings,
            SettingsLock,
            SettingsSave,
            SourceKind,
            SourcesFile,
        },
        timer::{CombatState, Position},
        Interruption,
        InterruptionSignal,
        SETTINGS,
        SOURCES,
    },
    anyhow::{anyhow, Context},
    arcdps::{evtc::event::Event as arcEvent, AgentOwned},
    glam::f32::Vec3,
    relative_path::RelativePathBuf,
    std::{ffi::OsStr, future::Future, mem, path::PathBuf, sync::Arc, time::SystemTime},
    strum::Display,
    taimi_meta::ui::gameplay::GameplayState,
    taimi_sync::watched,
    tokio::{
        select,
        sync::{
            mpsc::{self, Receiver, Sender},
            watch,
            Mutex,
        },
        task::JoinSet,
        time::{interval, timeout, Duration},
    },
};

#[cfg(any(feature = "markers", feature = "space"))]
use crate::render::machine::MumbleIdentityUpdate;

mod generic;

#[cfg(feature = "timers")]
pub(crate) mod timers;

#[cfg(feature = "timers")]
use timers::{TimersController, TimersEvent};

#[cfg(feature = "markers")]
pub(crate) mod markers;

#[cfg(feature = "markers")]
use markers::{MarkersController, MarkersEvent};

#[cfg(feature = "space")]
pub(crate) mod pathing;

#[cfg(feature = "space")]
use pathing::{PathingController, PathingEvent, PathingReceiver, PathingSender};

pub(crate) mod api;
pub(crate) mod runtime;

pub(crate) type MapId = Option<u32>;
pub(crate) type RtSender = Sender<RenderEvent>;

#[derive(Debug)]
pub struct Controller {
    receiver: Receiver<ControllerEvent>,
    gameplay_rx: watch::Receiver<GameplayState>,
    pub agent: Option<AgentOwned>,
    pub previous_combat_state: bool,
    pub rt_sender: RtSender,
    pub map_id: MapId,
    pub player_position: Option<Vec3>,
    // TODO: remove!
    alert_sem: Arc<Mutex<()>>,
    settings: SettingsLock,
    #[cfg(feature = "extension-nexus")]
    state_quick_access_current: TaimiControls,
    #[cfg(feature = "extension-nexus")]
    state_quick_access: watch::Receiver<TaimiControls>,
    state_bootstrap: watch::Receiver<BootstrapState>,
    state_bootstrap_throttle: watched::WatchThrottleDelay,
    state_save: watch::Receiver<SaveState>,
    state_save_throttle: watched::WatchThrottleDelay,
    save_interval: tokio::time::Interval,
    keybinds: TaimiReceiver,

    timers: TimersController,
    markers: MarkersController,
}

impl Controller {
    pub fn player_position(&self) -> Option<Position> {
        self.player_position.map(Position::Vec3)
    }

    pub fn new(
        receiver: Receiver<ControllerEvent>,
        gameplay_rx: watch::Receiver<GameplayState>,
        rt_sender: Sender<RenderEvent>,
        settings: SettingsLock,
    ) -> Self {
        Self {
            receiver,
            gameplay_rx,
            previous_combat_state: Default::default(),
            rt_sender,
            settings,
            #[cfg(feature = "extension-nexus")]
            state_quick_access_current: TaimiControls::empty(),
            #[cfg(feature = "extension-nexus")]
            state_quick_access: crate::QUICK_ACCESS_STATE.subscribe(),
            state_bootstrap: BootstrapState::get().subscribe(),
            state_bootstrap_throttle: BootstrapState::watch_initial_delay(),
            state_save: SaveState::get().subscribe(),
            state_save_throttle: SaveState::watch_initial_delay(),
            agent: Default::default(),
            map_id: Default::default(),
            player_position: Default::default(),
            alert_sem: Default::default(),
            save_interval: interval(Duration::from_secs(60 * 10)),
            keybinds: CONTROLS.subscribe_taimi(),

            timers: Default::default(),
            markers: Default::default(),
        }
    }

    pub fn load(
        mut receiver: ControllerReceiver,
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
            let critical_failure = || {
                #[cfg(debug_assertions)]
                log::error!("controller broken");
            };
            let mut controllers = JoinSet::new();
            {
                let settings = Settings::load_access(&addon_dir.clone()).await;
                let Some(gameplay) = receiver.gameplay.take() else {
                    critical_failure();
                    return
                };
                #[cfg(any(feature = "markers", feature = "space"))]
                let Some(mumble_identity) = receiver.mumble_identity.take() else {
                    critical_failure();
                    return
                };
                #[cfg(feature = "space")]
                if let Some(rx) = receiver.pathing.take() {
                    let mut pathing = PathingController::new(rx, settings.clone());
                    controllers.spawn(async move {
                        let res = pathing.run().await.context("Pathing control loop");
                        let _ = rt::log::error_ok(res);
                    });
                };
                if let Some(rx) = receiver.api.take() {
                    let mut api = ApiController::new(rx, settings.clone());
                    controllers.spawn(async move {
                        let res = api.run().await.context("API control loop");
                        let _ = rt::log::error_ok(res);
                    });
                };
                if let Some(receiver) = receiver.generic.take() {
                    let mut state = Self::new(receiver, gameplay, rt_sender, settings);
                    #[cfg(feature = "markers")]
                    {
                        state.markers.mumble_identity_rx = Some(mumble_identity);
                    }
                    controllers.spawn(async move { state.run().await });
                } else {
                    critical_failure();
                };
            }
            while let Some(res) = controllers.join_next().await {
                if let Err(e) = res {
                    log_join_error("controller", e);
                }
            }
        };
        rt.block_on(evt_loop);
        Self::shutdown(rt);
    }

    pub async fn late_init(&mut self) {
        let settings = self.settings.read().await;
        #[cfg(feature = "extension-nexus")]
        let qa = if rt::nexus_available() {
            let mut qa_state = TaimiControls::empty();
            crate::QUICK_ACCESS_STATE.send_if_modified(|state| {
                let pathing = settings.pathing();
                state.set(TaimiControls::PATHING_SPACE, pathing.space.visible_space());
                state.set(TaimiControls::PATHING_MAP, pathing.space.visible_worldmap());
                state.set(TaimiControls::PATHING_MINIMAP, pathing.space.visible_minimap());
                qa_state = *state;
                // no need to notify, this is used for initial state after all!
                false
            });
            Some((
                settings.quick_access_visible,
                settings.quick_access_style,
                qa_state,
            ))
        } else {
            None
        };
        drop(settings);

        #[cfg(feature = "extension-nexus")]
        if let Some((qa_icons, qa_style, qa_state)) = qa {
            crate::exports::nexus::quick_access_init(qa_icons, qa_style, qa_state);
        }
    }
    #[cfg(feature = "extension-nexus")]
    async fn quick_access_changed(&mut self) {
        use crate::exports::nexus::{quick_access_add, quick_access_remove};

        let prev = self.state_quick_access_current;
        self.state_quick_access_current = self.state_quick_access.borrow_and_update().clone();
        let state = self.state_quick_access_current;
        let (style, visible) = {
            let settings = self.settings.read().await;
            (settings.quick_access_style, settings.quick_access_visible)
        };
        for changed_icon in state ^ prev {
            if !visible.intersects(changed_icon) {
                continue
            }
            quick_access_remove(changed_icon);
            quick_access_add(changed_icon, state, style);
        }
    }

    pub async fn run(&mut self) {
        let datasources = SourcesFile::load().await.context("Couldn't load sources file");
        let datasources = match datasources {
            Ok(datasources) => datasources,
            Err(e) => {
                log::error!("{e:#}");
                SourcesFile::stock()
            },
        };
        if let Ok(mut sources) = SOURCES.write() {
            *sources = datasources;
        }
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
        state.late_init().await;

        let interruption = loop {
            let state_quick_access = match () {
                #[cfg(feature = "extension-nexus")]
                _ => state.state_quick_access.changed(),
                #[cfg(not(feature = "extension-nexus"))]
                _ => futures::future::pending::<Result<(), ()>>(),
            };
            let mumble_identity_rx = match () {
                #[cfg(feature = "markers")]
                () => state.markers.mumble_identity_rx.as_mut().unwrap().changed(),
                #[cfg(not(feature = "markers"))]
                () => futures::future::pending::<()>(),
            };
            select! {
                evt = state.receiver.recv() => match evt {
                    Some(evt) => match state.handle_event(evt).await {
                        Ok(None) => (),
                        Ok(Some(int)) => break int,
                        Err(error) => {
                            log::error!("Error! {}", error)
                        }
                    },
                    None => {
                        break Interruption::Unspecified
                    },
                },
                Ok(()) = state.gameplay_rx.changed() => {
                    let gameplay = *state.gameplay_rx.borrow_and_update();
                    state.handle_map_event(gameplay).await;
                    if gameplay.gameplay_map().is_some() {
                        // force immediate state update
                        let _ = rt::log::error_ok(state.mumblelink_tick().await);
                    }
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
                Ok(()) = state_quick_access => {
                    #[cfg(feature = "extension-nexus")]
                    if rt::nexus_available() {
                        state.quick_access_changed().await;
                    }
                },
                Ok(()) = SaveState::watch_dirty(&mut state.state_save, &mut state.state_save_throttle) => {
                    let state = state.state_save.borrow_and_update();
                    if let Err(e) = Self::commit_state_save(state).await {
                        log::error!("{e:#}");
                    }
                },
                keybinds = state.keybinds.wait() => match keybinds {
                    Err(e) => log::error!("Keybind receive error! {e:#}"),
                    Ok((binds_state, binds_changed)) => {
                        state.handle_keybinds(binds_state, binds_changed).await;
                    },
                },
                _ = mumble_identity_rx => {
                    #[cfg(feature = "markers")]
                    {
                        state.markers.handle_mumble_identity();
                    }
                },
            }
        };

        state.handle_exit(interruption).await;
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
        let menus = pressed & TaimiControls::MENUS;
        if !menus.is_empty() {
            CONTROLS.notify_handled(menus);
            let _ = self.rt_sender.send(RenderEvent::ContextMenuOpen { menus }).await;
        }

        #[cfg(feature = "timers")]
        {
            if self.timers.handle_keybinds(state, changed).await {
                // timer key was pressed and may influence machine state,
                // but could be missed if released before a normal tick update!
                if let Some(playpos) = self.player_position() {
                    self.timers.handle_position(playpos).await;
                }
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

    async fn settings_write(&self) -> tokio::sync::RwLockWriteGuard<'_, Settings> {
        let mut settings = self.settings.write().await;
        settings.mark_dirty();
        settings
    }

    async fn check_sources(&self) -> anyhow::Result<()> {
        let latest = SourcesFile::get_sources().await?;
        let mut settings_lock = self.settings_write().await;
        for remote in &mut settings_lock.remotes {
            if remote
                .datasource_repo
                .as_ref()
                .map(|r| &r[..] == SourcesFile::FILENAME)
                .unwrap_or(true)
            {
                let source = remote
                    .datasource_name
                    .as_ref()
                    .and_then(|name| latest.lookup(remote.kind, name))
                    .or_else(|| latest.lookup(remote.kind, &remote.source().name()));
                if let Some(source) = source {
                    remote.datasource_repo = Some(SourcesFile::FILENAME.into());
                    remote.datasource_name = Some(source.as_source().name().into_owned());
                    remote.source = source.clone();
                } else {
                    remote.datasource_repo = None;
                    remote.datasource_name = None;
                }
            }
        }
        // clear out unnecessary state from outdated or redundant sources
        settings_lock
            .remotes
            .retain(|remote| remote.datasource_repo.is_none() || !remote.is_empty());
        drop(settings_lock);
        if let Ok(mut sources) = SOURCES.write() {
            *sources = latest;
        }
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
                    log::trace!("MumbleLink: Combat begins at {:?}!", SystemTime::now());
                    CombatState::Entered
                },
                false => {
                    log::trace!("MumbleLink: Combat ends at {:?}!", SystemTime::now());
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

    async fn handle_map_event(&mut self, gameplay: GameplayState) {
        let new_map_id = match gameplay {
            GameplayState::Intermission { next_map_id, .. }
                if next_map_id.map(|id| id.get()) != self.map_id && self.map_id.is_some() =>
            {
                self.map_id = None;
                #[cfg(feature = "timers")]
                self.timers.handle_loading_screen().await;
                return
            },
            GameplayState::Gameplay { map_id: Some(map_id), .. } => map_id.get(),
            _ => return,
        };
        if Some(new_map_id) != self.map_id {
            log::debug!("Map changed from {:?} to {}", self.map_id, new_map_id);
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
                    log::trace!("Character changed from {:?} to {:?}!", agent.name, src.name);
                    *agent = src;
                },
                Some(_agent) => (),
                None => {
                    log::trace!("Character selected, {:?}!", src.name);
                    self.agent = Some(src);
                },
            };
        }
        use arcdps::StateChange;
        let propagate = match evt.get_statechange() {
            StateChange::EnterCombat => {
                log::trace!("ArcDPS: Combat begins at {}!", evt.time);
                Some(CombatState::Entered)
            },
            StateChange::ExitCombat => {
                log::trace!("ArcDPS: Combat ends at {}!", evt.time);
                Some(CombatState::Exited)
            },
            _ => None,
        };
        match propagate {
            Some(cbt) => self.timers.handle_combat_event(cbt).await,
            None => (),
        }
    }

    fn check_updates(&mut self, filter: Result<(SourceKind, String), bool>) {
        let settings = self.settings.clone();
        let rt_sender = self.rt_sender.clone();
        let _ = tokio::spawn(Self::check_for_updates(rt_sender, settings, filter));
    }

    async fn check_for_updates(
        rt_sender: RtSender,
        settings: SettingsLock,
        filter: Result<(SourceKind, String), bool>,
    ) {
        let _ = rt_sender
            .send(RenderEvent::CheckingForUpdates { checking: true, downloading: false })
            .await;
        let res = Settings::check_for_updates(settings, move |remote| match filter {
            Ok((kind, ref name)) => remote.kind == kind && &remote.datasource_name() == name,
            Err(everything) => everything || remote.installed_path.is_some(),
        })
        .await
        .context("Controller.check_updates");
        match res {
            Ok(_) => (),
            Err(err) => log::error!("{err:#}"),
        }
        let _ = rt_sender
            .send(RenderEvent::CheckingForUpdates { checking: false, downloading: false })
            .await;
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

    fn commit_state_bootstrap(
        state: watch::Ref<'_, BootstrapState>,
    ) -> impl Future<Output = anyhow::Result<()>> + Send {
        let save = match state {
            state if !state.has_changed() => Ok(None),
            state => state.start_save().map(Some),
        };
        async move {
            let Some(save) = save? else { return Ok(()) };
            BootstrapState::save_to(&save).await.context("Saving boot state")
        }
    }
    fn commit_state_save(
        state: watch::Ref<'_, SaveState>,
    ) -> impl Future<Output = anyhow::Result<()>> + Send {
        let save = match state {
            state if !state.has_changed() => Ok(None),
            state => state.start_save().map(Some),
        };
        async move {
            let Some(save) = save? else { return Ok(()) };
            SaveState::save_to(&save).await.context("Saving save state")
        }
    }

    async fn reload_data(&mut self) {
        #[cfg(feature = "timers")]
        self.timers
            .reload(self.settings.clone(), self.rt_sender.clone())
            .await;
        #[cfg(feature = "markers")]
        self.markers.reload(self.rt_sender.clone()).await;
        #[cfg(feature = "space")]
        PathingEvent::ReloadAll(true).try_send();
    }

    pub fn with_datasource<R, F: FnOnce(&DeserializedSource) -> Option<R>>(
        kind: SourceKind,
        id: &str,
        f: F,
    ) -> Option<R> {
        match SOURCES.read() {
            Ok(sources) => sources.lookup(kind, &id).and_then(f),
            _ => None,
        }
    }

    fn spawn_source_update(&mut self, kind: SourceKind, id: String) {
        let rt_sender = self.rt_sender.clone();
        let settings = self.settings.clone();
        let _ = tokio::spawn(Self::do_update(rt_sender, settings, kind, id));
    }

    async fn do_update(rt_sender: RtSender, settings: SettingsLock, kind: SourceKind, id: String) {
        let _ = rt_sender
            .send(RenderEvent::CheckingForUpdates { checking: true, downloading: true })
            .await;
        let state = match RemoteState::lookup_datasource(&settings.read().await.remotes, kind, &id) {
            Some(state) => Some(state.clone()),
            None => Self::with_datasource(kind, &id, |source| {
                Some(RemoteState::new_from_source(kind, source.clone()))
            }),
        };
        let Some(state) = state else {
            log::warn!("update requested for unknown {kind} datasource {id}?");
            return
        };
        let res = Settings::download_latest(settings, state)
            .await
            .with_context(|| format!("controller update for {kind} datasource {id} failed"));
        match res {
            Ok(_) => (),
            Err(err) => log::error!("{err:#}"),
        };
        let _ = rt_sender
            .send(RenderEvent::CheckingForUpdates { checking: false, downloading: false })
            .await;
        match kind {
            SourceKind::Timers => TimersController::try_send(TimersEvent::ReloadTimers),
            SourceKind::Markers => MarkersController::try_send(MarkersEvent::ReloadMarkers),
            SourceKind::Pathing => {
                // TODO: if new pack, ReloadAll(false)
                // TODO: if existing pack, ReloadPack(path) instead
                PathingEvent::ReloadAll(true).try_send();
            },
            _ => (),
        }
    }

    async fn set_window_state(&mut self, window: &str, state: Option<bool>) {
        let mut settings_lock = self.settings_write().await;
        settings_lock.set_window_state(window, state);
        drop(settings_lock);
    }

    async fn open_openable<T: AsRef<OsStr>>(&self, key: String, uri: T) {
        use std::path::Path;

        let uri = uri.as_ref();
        let as_path = Path::new(uri);
        if as_path.extension().is_none() && as_path.starts_with(rt::addon_dir()) {
            // TODO: less dumb thanks
            let _ = std::fs::create_dir(as_path);
        }
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

    async fn remove_source(&mut self, kind: SourceKind, id: String) {
        let mut settings = self.settings_write().await;
        settings
            .remotes
            .retain(|remote| remote.kind != kind || !remote.datasource_name_matches(&id));
    }

    async fn uninstall_addon(&mut self, kind: SourceKind, id: String) {
        let settings = self.settings.clone();
        // XXX: consider spawning...
        if let Err(e) = Self::uninstall_source(settings, kind, id).await {
            log::error!("{e:#}");
        }
    }

    async fn uninstall_source(settings: SettingsLock, kind: SourceKind, id: String) -> anyhow::Result<()> {
        let state = RemoteState::lookup_datasource(&settings.read().await.remotes, kind, &id).cloned();
        let Some(mut state) = state else {
            log::warn!("cannot uninstall missing {kind} datasource {id}!");
            return Ok(())
        };
        let res = state.uninstall().await;

        {
            let mut settings = settings.write().await;
            if let Some(remote) = RemoteState::lookup_datasource_mut(&mut settings.remotes, kind, &id) {
                *remote = state;
                settings.mark_dirty();
            }
        }
        res
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

    async fn handle_event(&mut self, event: ControllerEvent) -> anyhow::Result<Option<Interruption>> {
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
            #[cfg(feature = "timers")]
            Timers(..) => (),
            #[cfg(feature = "markers")]
            Markers(..) => (),
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

            ReloadData => self.reload_data().await,
            SaveSettings => self.save_settings().await,
            OpenOpenable(key, uri) => self.open_openable(key, uri).await,
            UninstallAddon { kind, id } => self.uninstall_addon(kind, id).await,
            RemoveDataSource { kind, id } => self.remove_source(kind, id).await,
            CombatEvent { src, evt } => self.handle_combat_event(src, evt).await,
            CheckDataSourceUpdates(everything) => self.check_updates(Err(everything)),
            CheckDataSourceUpdate { kind, id } => self.check_updates(Ok((kind, id))),
            CheckUpdateSources => self.check_sources().await?,
            CheckAddonUpdate(proceed) => self.addon_check_for_updates(proceed).await,
            DoDataSourceUpdate { kind, id } => self.spawn_source_update(kind, id),
            WindowState(window, state) => self.set_window_state(&window, state).await,
            LoadTexture(rel, base) => self.load_texture(rel, base).await,
            LoadTextureIntegrated(identifier, data) => self.load_texture_integrated(identifier, data).await,
            UiTick(tick) => match tick.is_player() {
                #[cfg(todo)]
                false => (),
                _ => self.mumblelink_tick().await?,
            },
            Quit(reason) => return Ok(Some(reason)),
            // I forget why we needed this, but I think it's a holdover from the buttplug one o:
            //_ => (),
        }
        Ok(None)
    }

    pub async fn handle_exit(&mut self, reason: Interruption) {
        if let Interruption::Abort = reason {
            log::debug!("controller skipping shutdown due to abort");
            return
        }
        match reason {
            // no need if we're exiting for good or will be right back...
            Interruption::GameQuit | Interruption::Temporary => (),
            // otherwise unloading for typical reasons and should perform clean-up
            _ => {},
        }
        let _ = rt::log::error_ok(self.save_on_quit().await);
    }

    #[cfg(feature = "extension-arcdps")]
    pub fn arc_spawn_early_exit() {
        use {crate::exports::arcdps as exports, std::thread};

        let avail = exports::available();
        let arc_cleanup = match avail {
            #[cfg(feature = "extension-nexus")]
            false if exports::loaded() && !rt::nexus_available() => true,
            avail => avail,
        };
        if arc_cleanup {
            thread::spawn(move || {
                if avail {
                    // wait a tiny bit to give render thread cleanup a chance
                    thread::sleep(Duration::from_millis(84));
                }
                // TODO: synchronize with controller shutdown in case it takes a while...
                let res = unsafe { exports::ExitHandle::try_exit() }
                    .and_then(|exit| exit.ok_or("unloaded/unaware?"));
                match res {
                    Err(e) => log::error!("Failed to leave arcdps: {e}"),
                    Ok(exit) => {
                        log::info!("goodbye arc");
                        exit.free_blocking();
                    },
                }
            });
        }
    }

    #[cfg(todo = "unused")]
    pub fn try_exit(reason: Interruption) -> Option<Interruption> {
        let mut sender = crate::CONTROLLER_SENDER.try_write().ok()?;
        if let Some(reason) = sender.shutdown {
            return Some(reason)
        }
        match sender.exit(reason) {
            Some(true) => {
                RenderState::try_send(RenderEvent::Quit(reason));
                Some(reason)
            },
            _ => sender.shutdown,
        }
    }
    pub fn send_exit(reason: Interruption) -> Interruption {
        let mut sender = match crate::CONTROLLER_SENDER.write() {
            Ok(s) => s,
            // if this is poisoned that's very bad news
            Err(..) => return Interruption::Abort,
        };
        match sender.exit(reason) {
            Some(true) => {
                RenderState::try_send(RenderEvent::Quit(reason));
                reason
            },
            _ => sender.shutdown.unwrap_or(Interruption::Unspecified),
        }
    }

    pub fn with_sender<R, F: FnOnce(&ControllerSender) -> R>(f: F) -> Option<R> {
        let sender = crate::CONTROLLER_SENDER.try_read().ok()?;

        Some(f(&*sender))
    }

    pub fn try_send(e: ControllerEvent) {
        Self::with_sender(|sender| sender.generic_try_send(e));
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

    // TODO: remove as porting happens - Generic
    WindowState(String, Option<bool>),
    OpenOpenable(String, String),
    CombatEvent {
        src: arcdps::AgentOwned,
        evt: arcEvent,
    },
    CheckDataSourceUpdates(bool),
    CheckDataSourceUpdate {
        kind: SourceKind,
        id: String,
    },
    DoDataSourceUpdate {
        kind: SourceKind,
        id: String,
    },
    RemoveDataSource {
        kind: SourceKind,
        id: String,
    },
    UninstallAddon {
        kind: SourceKind,
        id: String,
    },
    LoadTextureIntegrated(String, Vec<u8>),
    #[strum(to_string = "Load texture {0} from {1:?}")]
    LoadTexture(RelativePathBuf, PathBuf),
    ReloadData,
    SaveSettings,
    UiTick(MumblelinkTick),
    CheckUpdateSources,
    CheckAddonUpdate(bool),
    Quit(Interruption),
}

impl ControllerEvent {
    pub fn try_send(self) {
        Controller::try_send(self)
    }
}

impl InterruptionSignal for ControllerEvent {
    fn interrupted(&self) -> Option<Interruption> {
        match self {
            &Self::Quit(reason) => Some(reason),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ControllerSender {
    pub shutdown: Option<Interruption>,
    pub gameplay: Option<watch::Sender<GameplayState>>,
    #[cfg(any(feature = "markers", feature = "space"))]
    pub mumble_identity: Option<watch::Sender<Option<MumbleIdentityUpdate>>>,
    pub generic: Option<Sender<ControllerEvent>>,
    pub api: Option<ApiSender>,
    #[cfg(feature = "space")]
    pub pathing: Option<PathingSender>,
}

impl ControllerSender {
    pub const EMPTY: Self = Self {
        shutdown: None,
        gameplay: None,
        #[cfg(any(feature = "markers", feature = "space"))]
        mumble_identity: None,
        generic: None,
        api: None,
        #[cfg(feature = "space")]
        pathing: None,
    };

    pub fn new() -> (Self, ControllerReceiver) {
        let (generic, generic_rx) = mpsc::channel(64);
        let gameplay = watch::Sender::new(GameplayState::INITIAL);
        let (api, api_rx) = ApiSender::new(&gameplay);
        #[cfg(any(feature = "markers", feature = "space"))]
        let (mumble_identity_tx, mumble_identity_rx) = watch::channel(None);
        #[cfg(feature = "paths")]
        let (pathing, pathing_rx) =
            PathingSender::new(gameplay.subscribe(), mumble_identity_rx.clone(), &api.festivals);

        let receiver = ControllerReceiver {
            gameplay: Some(gameplay.subscribe()),
            #[cfg(any(feature = "markers", feature = "space"))]
            mumble_identity: Some(mumble_identity_rx),
            generic: Some(generic_rx),
            api: Some(api_rx),
            #[cfg(feature = "space")]
            pathing: Some(pathing_rx),
        };
        let sender = Self {
            shutdown: None,
            gameplay: Some(gameplay),
            #[cfg(any(feature = "markers", feature = "space"))]
            mumble_identity: Some(mumble_identity_tx),
            generic: Some(generic),
            api: Some(api),
            #[cfg(feature = "space")]
            pathing: Some(pathing),
        };

        (sender, receiver)
    }

    pub fn exit(&mut self, reason: Interruption) -> Option<bool> {
        #[cfg(feature = "space")]
        if let Some(sender) = self.pathing.take() {
            let _ = sender.command.try_send(PathingEvent::Exit(reason));
        }
        if let Some(sender) = self.api.take() {
            let _ = sender.command.try_send(ApiMessage::Exit(reason));
        }
        let reason = rt::notify_shutdown(reason);
        let sent = self
            .generic
            .as_ref()?
            .try_send(ControllerEvent::Quit(reason))
            .is_ok();
        if self.shutdown.is_none() {
            self.shutdown = Some(
                sent.then_some(reason)
                    .unwrap_or_else(|| rt::is_shutdown().unwrap_or(Interruption::Unspecified)),
            );
        }
        match (reason, sent) {
            (Interruption::GameQuit, false) => return Some(false),
            _ => (),
        }
        let generic = self.generic.take();
        let sent = match generic {
            Some(generic) if !sent && !reason.is_urgent() =>
                generic.blocking_send(ControllerEvent::Quit(reason)).is_ok(),
            _ => sent,
        };
        Some(sent)
    }

    pub fn take(&mut self) -> Self {
        mem::replace(self, Self::EMPTY)
    }

    pub fn generic_try_send(&self, message: ControllerEvent) -> bool {
        self.generic
            .as_ref()
            .and_then(move |sender| sender.try_send(message).ok())
            .is_some()
    }
    pub fn api_try_send(&self, message: ApiMessage) -> bool {
        self.api
            .as_ref()
            .and_then(move |api| api.command.try_send(message).ok())
            .is_some()
    }

    #[cfg(feature = "timers")]
    pub fn timers_try_send(&self, message: TimersEvent) -> bool {
        self.generic
            .as_ref()
            .and_then(move |sender| sender.try_send(ControllerEvent::Timers(message)).ok())
            .is_some()
    }

    #[cfg(feature = "markers")]
    pub fn markers_try_send(&self, message: MarkersEvent) -> bool {
        self.generic
            .as_ref()
            .and_then(move |sender| sender.try_send(ControllerEvent::Markers(message)).ok())
            .is_some()
    }

    #[cfg(feature = "space")]
    pub fn pathing_try_send(&self, message: PathingEvent) -> bool {
        self.pathing
            .as_ref()
            .and_then(move |pathing| pathing.command.try_send(message).ok())
            .is_some()
    }
    #[cfg(feature = "space")]
    pub fn pathing_blocking_send(&self, message: PathingEvent) -> bool {
        self.pathing
            .as_ref()
            .and_then(move |pathing| pathing.command.blocking_send(message).ok())
            .is_some()
    }
}

pub struct ControllerReceiver {
    pub gameplay: Option<watch::Receiver<GameplayState>>,
    #[cfg(any(feature = "markers", feature = "space"))]
    pub mumble_identity: Option<watch::Receiver<Option<MumbleIdentityUpdate>>>,
    pub generic: Option<Receiver<ControllerEvent>>,
    pub api: Option<ApiReceiver>,
    #[cfg(feature = "space")]
    pub pathing: Option<PathingReceiver>,
}
