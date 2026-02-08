#[doc(no_inline)]
pub use taimi_meta::coords::LocalSpace as PackSpace;
use {
    self::{
        registry::{PackLoader, PackMapPath, LoadedMarkerPath},
        space::{SpaceContext, SpacePackShared},
        state::{LoadedMapInfo, LoadedMaps, LoadedPacks},
    },
    crate::{
        controller::{
            runtime::WallInstant,
            Controller,
        },
        exports::runtime::{
            self as rt,
            bindings::{
                ControlsReceiver,
                GameControl,
                GameControls,
                TaimiControls,
                TaimiReceiver,
                CONTROLS,
            },
        },
        settings::SettingsLock,
        space::{engine::SpaceEvent, Engine},
        Interruption,
        InterruptionSignal,
    },
    anyhow::Context,
    futures::{
        future::{self, Either},
        stream,
        StreamExt,
    },
    std::{collections::VecDeque, future::Future, mem, pin::Pin, sync::Arc, time::Duration},
    strum_macros::Display,
    taimi_hoard::{
        loc::LocationRef,
        time::Timestamp,
    },
    taimi_meta::{
        packs::{
            collections::{CategorySet, PackSet},
            id::{MarkerId, MarkerPath},
            CategoryPath,
            PackPath,
        },
        ui::{
            gameplay::{GameplayState, GameplayTransition},
            MapContext,
            UiState,
        },
    },
    taimi_pack::attributes::{AttrString, Festivals},
    taimi_meta::packs::MapIndex,
    taimi_sync::watched,
    tokio::{select, sync::Semaphore, task::JoinSet},
    tokio_util::sync::ReusableBoxFuture,
};
#[cfg(feature = "paths-interact")]
use self::interact::InteractReactor;
#[cfg(feature = "scripts")]
use crate::controller::script;

#[allow(unused_imports)]
pub use self::{
    config::PackConfig,
    festivals::FestivalFixup,
    registry::{LoaderBox, UnloadedReason},
    shared::{PathingEnables, PathingReceiver, PathingSender, PathingShared},
    state::VisibilityFlagsExt,
};
#[cfg(feature = "paths-interact")]
pub use self::interact::InteractMessage;

mod config;
mod festivals;
#[cfg(feature = "paths-filter")]
mod filter;
pub mod info;
#[cfg(feature = "paths-interact")]
mod interact;
pub mod reactor;
pub mod registry;
mod setup;
pub mod shared;
pub mod space;
pub mod state;

#[cfg(todo = "unused")]
pub type ExternalFilterState = (Festivals, Arc<RaidState>, Arc<AchievementState>);

#[derive(Debug, Display, Default)]
pub(crate) enum PathingEvent {
    VisibleToggle {
        context: Option<MapContext>,
        set: Option<bool>,
    },
    ReloadPack(PackPath, bool),
    LoadPack(PackPath),
    /// like Unload except will reactivate on its own when needed
    OffloadPack(PackPath),
    /// explicit request to keep pack and its resources unloaded
    /// (and optionally remove from registry)
    UnloadPack(PackPath, bool),
    Refresh { include_datasources: bool },
    ReloadAll(bool),
    LoadAll,
    UnloadAll(bool),
    PackLock { path: PackPath },
    PackUnlock { path: PackPath },
    /// toggle or set category state
    CategoryEnableSet(PackPath, CategoryPath, Option<bool>),
    /// until we maintain the mapping for interaction attrs that toggle/show/hide...
    CategoryEnableById(PackPath, taimi_pack::category::id::IdNameBox, Option<bool>),
    /// act upon a batch of changes to [shared::SharedPackLoad::config]
    CategoryEnableCommit(PackPath, CategorySet),
    /// taco guid reset
    #[cfg(feature = "paths-filter")]
    ResetMarkerIds(Vec<MarkerId>),
    /// same as id
    #[cfg(feature = "paths-filter")]
    ResetMarkerPath(MarkerPath<PackPath>),
    #[cfg(feature = "paths-interact")]
    DismissMarker {
        path: MarkerPath,
        loaded_path: LoadedMarkerPath<PackMapPath>,
        until: Option<Either<Timestamp, Duration>>,
        contexts: Vec<state::hidden::HideContext>,
        reset: Option<state::hidden::AutoReset>,
    },
    #[cfg(feature = "paths-interact")]
    TriggerMarkerCopy {
        path: MarkerPath,
        loaded_path: LoadedMarkerPath<PackMapPath>,
        value: AttrString,
        message: Option<AttrString>,
    },
    #[cfg(feature = "paths-interact")]
    TriggerMarkerInfo {
        path: MarkerPath,
        loaded_path: LoadedMarkerPath<PackMapPath>,
        message: AttrString,
    },
    #[cfg(feature = "paths-interact")]
    #[strum(to_string = "InteractControl {0}")]
    InteractControl(interact::InteractMessage),
    ToggleKatRender,
    ApiBypass(Option<bool>),
    #[cfg(feature = "paths-lua")]
    ScriptsEnable(Option<bool>),
    ReportResourceLoaded(shared::LoadReport),
    CollectGarbage {
        tick: u32,
        aggressive: bool,
    },
    #[cfg(todo = "unused")]
    SpawnTask(PathingTaskBox),
    #[cfg(todo = "unused")]
    #[cfg(feature = "paths-filter")]
    Scheduled(WallInstant, Vec<Self>),
    FanOut(Vec<PathingEvent>),
    Exit(Interruption),
    #[default]
    Nop,
    // Debug and diagnostics commands
    RequestRebuildSpace { entities: Option<bool>, bvh: Option<bool> },
    RequestRebuildVis { pack_path: Option<PackPath>, partial: bool, notify: Option<bool> },
    RequestResourceRelease { pack_path: Option<PackPath> },
    RequestResourceReport { pack_path: Option<PackPath> },
}
pub type PathingTaskBox = Pin<Box<dyn Future<Output = Option<PathingEvent>> + Send + 'static>>;

pub(crate) struct PathingController {
    loader: Arc<PackLoader>,
    rx: PathingReceiver,
    tasks: JoinSet<anyhow::Result<PathingEvent>>,
    #[cfg(feature = "paths-filter")]
    scheduled_events: reactor::ScheduledEvents,
    controls: ControlsReceiver,
    keybinds: TaimiReceiver,
    settings: SettingsLock,
    active: bool,
    packs: LoadedPacks,
    map_info: LoadedMapInfo,
    maps: LoadedMaps,
    space: SpaceContext,
    filter_state: state::filter::FilterState,
    /// replace this with a real signal or something?
    filter_state_signal: Option<bool>,
    #[cfg(feature = "paths-filter")]
    filter_expiry: reactor::FilterExpiryMap,
    #[cfg(feature = "paths-filter")]
    filter_next_schedule: ReusableBoxFuture<'static, ()>,
    /// whether `filter_next_schedule` indicates dirty state after
    #[cfg(feature = "paths-filter")]
    filter_next_schedule_dirty: bool,
    #[cfg(feature = "paths-interact")]
    interact: InteractReactor,
    // watchers...
    pack_configs: watched::WatchStreamBox<PackPath, shared::SharedPackConfig>,
    /// we only need to regen if a new pack slot is allocated
    pack_configs_sig: PackPath,
    packs_rx: watched::Rx<shared::SharedLoaderPacksInfo>,
}

impl PathingController {
    pub fn new(rx: PathingReceiver, settings: SettingsLock) -> Self {
        let loader = rx.make_loader(settings.clone());
        Self {
            space: SpaceContext::subscribe_to(&loader.shared),
            packs_rx: {
                let mut packs_rx = loader.shared.packs.packs.subscribe();
                packs_rx.mark_changed();
                packs_rx
            },
            rx,
            loader,
            controls: CONTROLS.subscribe_controls(),
            keybinds: CONTROLS.subscribe_taimi(),
            tasks: Default::default(),
            #[cfg(feature = "paths-filter")]
            scheduled_events: Default::default(),
            settings,
            active: true,
            packs: Default::default(),
            map_info: Default::default(),
            maps: Default::default(),
            pack_configs: Box::new(stream::pending()),
            pack_configs_sig: PackPath::default(),
            filter_state: Default::default(),
            filter_state_signal: None,
            #[cfg(feature = "paths-filter")]
            filter_expiry: Default::default(),
            #[cfg(feature = "paths-filter")]
            filter_next_schedule: ReusableBoxFuture::new(future::pending()),
            // would rather avoid touching these APIs until main async loop, extra allocation is acceptable
            #[cfg(todo = "unnecessary")]
            filter_next_schedule: ReusableBoxFuture::new(WallInstant::big_sleep()),
            #[cfg(feature = "paths-filter")]
            filter_next_schedule_dirty: true,
            #[cfg(feature = "paths-interact")]
            interact: Default::default(),
        }
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        self.setup().await;

        while self.active {
            let int = self.turn().await;
            if let Some(reason) = int {
                let res = self.exit(reason).await;
                self.active = false;
                return res
            }
        }

        Ok(())
    }
    pub async fn turn(&mut self) -> Option<Interruption> {
        if self.rx.command.is_closed() {
            return Some(
                Interruption::try_drain_signals(&mut self.rx.command).unwrap_or(Interruption::Unspecified),
            )
        }
        let gameplay_prev = self.rx.gameplay.get_mut().clone();
        let enables_prev = self.rx.enables.get_mut().clone();
        let load_throttle_prev = self
            .rx
            .load_throttle
            .get_mut().clone();
        let (scheduled_events, filter_next_schedule) = match () {
            #[cfg(feature = "paths-filter")]
            _ => (self.scheduled_events.infinite_mut().next(), &mut self.filter_next_schedule),
            #[cfg(not(feature = "paths-filter"))]
            _ => (future::pending::<Option<((), ())>>(), future::pending::<()>()),
        };
        let interact_rx = match () {
            #[cfg(feature = "paths-interact")]
            _ => self.interact.with_rx(&mut self.rx.interact),
            #[cfg(not(feature = "paths-interact"))]
            _ => future::pending::<()>(),
        };
        select! {
            e = self.rx.command.recv() => {
                let res = match e {
                    None => Some(Interruption::Unspecified),
                    Some(PathingEvent::Nop) => None,
                    Some(e) =>
                        self.handle_message(e).await,
                };
                match res {
                    Some(int) => return Some(int),
                    None => (),
                }
            },
            Ok(_) = self.packs_rx.changed() => {
                let (pack_count, packs_dirty, configs_dirty) = {
                    let packs = self.packs_rx.borrow_and_update();
                    if let Some(last) = packs.paths().last() {
                        // extend pack state to same length...
                        let _ = self.packs.write(last);
                    }
                    let mut packs_dirty = PackSet::default();
                    for ((path, dest), info) in self.packs.packs.iter_mut().zip(packs.values()) {
                        if dest.update_with(info) {
                            packs_dirty.insert(path);
                        }
                    }
                    let pack_count = packs.end_path();
                    // XXX: avoid two locks please?
                    let config_sigs = packs.values().map(|p| p.config.borrow().info_sig);
                    #[cfg(todo = "unnecessary")]
                    let info_sigs = packs.values().map(|p| p.info.sig);
                    let configs_dirty = self.packs.sigs_dirty(config_sigs);
                    (pack_count, packs_dirty, configs_dirty)
                };
                if self.pack_configs_sig < pack_count {
                    // we only need to resubscribe when length changes...
                    let additions = mem::replace(&mut self.pack_configs_sig, pack_count);
                    self.pack_configs = self.loader.shared.watch_config_changes(Either::Right(additions..));
                }
                let map_id = gameplay_prev.gameplay_map();
                #[cfg(feature = "paths-filter")]
                let hidden_guids = (map_id.is_some() && !packs_dirty.is_empty())
                    .then(Self::clone_hidden_guids)
                    .flatten();
                #[cfg(feature = "paths-filter")]
                let hidden_ctx = hidden_guids.as_ref().map(|h| (&**h, WallInstant::now_timestamp_mono()));
                #[cfg(not(feature = "paths-filter"))]
                let hidden_ctx = None;
                for path in &packs_dirty {
                    if let Some(map_path) = map_id.map(|map| path.rel(map)) {
                        if let Some(pack) = self.packs.lookup_ref(&map_path.root) {
                            if !pack.info.has_map(map_path.path) {
                                continue
                            }
                        }
                        let _ = self.prepare_for_pack_map(map_path, true, hidden_ctx);
                    }
                }
                for path in &configs_dirty {
                    self.reload_config_for(path);
                }
            },
            enables = self.rx.enables.when_changed() => {
                let changed = *enables ^ enables_prev;
                if changed.contains(PathingEnables::ENGINE) && enables.contains(PathingEnables::ENGINE) {
                    log::debug!("engine online, let's go!");
                }
            },
            load_throttle = self.rx.load_throttle.when_changed() => {
                let new_amt = (*load_throttle).max(1).min(Semaphore::MAX_PERMITS / 2);
                let change = new_amt as isize - load_throttle_prev as isize;
                if change != 0 {
                    log::trace!("adjusting load throttle by {change} to {new_amt}");
                }
                match self.loader.adjust_load_throttle_by(change, new_amt) {
                    Ok(()) => (),
                    Err(()) => log::debug!("refreshed loader throttle due to outstanding permits"),
                }
            },
            gameplay = self.rx.gameplay.when_changed() => {
                let gameplay = *gameplay;
                let trans = gameplay.latest_transition_from(gameplay_prev);
                self.handle_gameplay(gameplay, trans).await;
            },
            Some(res) = self.tasks.join_next(), if !self.tasks.is_empty() => match res {
                Ok(res) => match rt::log::error_ok(res.context("pathing task")) {
                    None | Some(PathingEvent::Nop) => (),
                    Some(m) =>
                        return self.handle_message(m).await,
                },
                Err(e) => crate::log_join_error("pathing", e),
            },
            Some((_when, _m)) = scheduled_events => {
                #[cfg(feature = "paths-filter")]
                return self.handle_message(_m).await
            },
            _ = filter_next_schedule => {
                #[cfg(feature = "paths-filter")]
                {
                    // self.update_filter_state_schedule(ctx);
                    self.filter_next_schedule.set(WallInstant::soon(Self::SCHEDULE_TIMEOUT).to_future());
                    match self.filter_next_schedule_dirty {
                        true => self.filter_state_signal = Some(true),
                        false => {
                            self.filter_state_signal.get_or_insert(false);
                        },
                    }
                }
            },
            _ = self.rx.festivals.changed() => {
                let new = self.rx.festivals.borrow_and_update().get();
                if new != self.filter_state.festival {
                    self.filter_state.festival = new;
                    self.filter_state_signal = Some(true);
                }
            },
            _ = self.rx.achievements.changed() => {
                let new = self.rx.achievements.borrow_and_update();
                if self.filter_state.achievements.update_with(&*new) {
                    self.filter_state_signal = Some(true)
                }
            },
            _ = self.rx.raids.changed() => {
                let new = self.rx.raids.borrow_and_update();
                if self.filter_state.raids.update_with(&*new) {
                    self.filter_state_signal = Some(true)
                }
            },
            Ok(..) = self.rx.mumble_identity.changed() => {
                if let Some(identity) = &*self.rx.mumble_identity.borrow_and_update() {
                    if self.filter_state.character.update_from_mumblelink(identity) {
                        self.filter_state_signal = Some(true);
                    }
                }
            },
            _ = future::ready(()), if self.filter_state_signal.is_some() && gameplay_prev.gameplay_map().is_some() => {
                let mut dirty = self.filter_state_signal.take().unwrap_or(false);
                #[cfg(feature = "paths-filter")]
                {
                    self.filter_next_schedule_dirty = false;
                    let now = WallInstant::now_timestamp_mono();
                    dirty |= self.update_filter_hidden_state(Some(now));
                }
                dirty |= self.update_filter_state();
                if dirty {
                    self.update_loaded_visibility(!Self::ALLOW_INCOMPLETE_VIS_UPDATE);
                }
            },
            _event = interact_rx => {
                #[cfg(feature = "paths-interact")]
                {
                    let cx = (&self.maps, &self.map_info, &self.filter_state, &self.settings);
                    let followup = self.interact.process_event(&mut self.rx, cx, _event).await;
                    self.process_or_spawn_message(followup).await;
                }
            },
            trail_reqs = SpaceContext::recv_trail_requests(&mut self.space.trail_geometry, &self.space.inflight) => {
                for trail in trail_reqs {
                    log::trace!("processing trail req {trail}");
                    let id = SpacePackShared::trail_geometry_id(&trail);
                    self.request_trail_load(id);
                }
            },
            texture_reqs = SpaceContext::recv_texture_requests(&mut self.space.texture_loads, &self.space.inflight) => {
                for path in texture_reqs {
                    let id = MarkerId::for_marker(path);
                    if self.request_texture_load(id) {
                        log::trace!("processing tex req {path}");
                    } else {
                        log::trace!("reserving tex req {path}");
                    }
                }
            },
            Ok(_) = self.space.maps_rx.changed() => {
                self.space_pack_updates().await;
            },
            controls = self.controls.wait() => match controls {
                Err(e) => log::error!("Control bindings error! {e:#}"),
                Ok((&controls_state, controls_changed)) => {
                    self.handle_presses(controls_state, controls_changed).await;
                },
            },
            keybinds = self.keybinds.wait() => match keybinds {
                Err(e) => log::error!("Keybind receive error! {e:#}"),
                Ok((binds_state, binds_changed)) => {
                    self.handle_keybinds(binds_state, binds_changed).await;
                },
            },
            Some((pack, config)) = self.pack_configs.next() => {
                self.handle_config_change(pack, &config).await
            },
        }
        None
    }

    async fn exit(&mut self, reason: Interruption) -> anyhow::Result<()> {
        #[cfg(todo = "unnecessary")]
        for handle in self.filter_expiry.values() {
            handle.abort();
        }
        #[cfg(feature = "paths-filter")]
        {
            self.filter_expiry.clear();
        }
        self.tasks.abort_all();
        #[cfg(feature = "paths-interact")]
        {
            self.interact.exit(&mut self.rx.interact, reason);
        }

        match reason {
            Interruption::Abort => return Ok(()),
            _ => (),
        }

        self.tasks.shutdown().await;

        Ok(())
    }

    async fn setup(&mut self) {
        let get_settings = {
            let enables = self.rx.enables.watch.clone();
            let load_throttle = self.rx.load_throttle.watch.sender();
            let settings = self.settings.clone();
            async move {
                let mut enable_flags = enables.read().clone();
                let settings = settings.read().await;
                let load_simultaneous = settings.pathing.as_ref()
                    .and_then(|p| p.load_simultaneous);
                let interact_config = match &settings.pathing {
                    #[cfg(feature = "paths-interact")]
                    p => p.as_ref().map(|p| interact::InteractSettings::from_settings(p)),
                    #[cfg(not(feature = "paths-interact"))]
                    _ => None::<()>,
                };
                enable_flags.set(PathingEnables::KATRENDER, settings.enable_katrender);
                #[cfg(feature = "paths-lua")]
                if let Some((enable, unsecure)) = settings
                    .pathing
                    .as_ref()
                    .map(|p| (p.scripting_enable, p.scripting_unsecured))
                {
                    enable_flags.set(PathingEnables::SCRIPTING_LUA, enable);
                    enable_flags.set(PathingEnables::SCRIPTING_UNSECURED, unsecure);
                }
                drop(settings);
                enables.replace(enable_flags);
                if let Some(load_simultaneous) = load_simultaneous {
                    load_throttle.send_replace(load_simultaneous);
                }
                (
                    interact_config,
                )
            }
        };
        let preload = self.preload_all();
        let ((), get_settings) = tokio::join!(preload, get_settings);

        let (_settings_interact_config,) = get_settings;
        #[cfg(feature = "paths-interact")]
        if let Some(config) = _settings_interact_config {
            self.interact.config = config;
            self.interact.enables = self.rx.enables();
        }
    }

    async fn toggle_katrender(&mut self) {
        let katrender = {
            let latest = self.rx.enables.get_mut().contains(PathingEnables::KATRENDER);
            let mut settings_lock = self.loader.settings.write().await;
            let mismatched = settings_lock.enable_katrender && !latest;
            match mismatched {
                true => {
                    // presumably because we were unloaded without saving settings...
                    log::info!("katrender resync");
                    !latest
                },
                false => {
                    settings_lock.toggle_katrender();
                    let katrender = settings_lock.enable_katrender;
                    if katrender != latest {
                        settings_lock.mark_dirty();
                    }
                    katrender
                },
            }
        };
        let mut engine_online = false;
        self.rx
            .enables
            .write_if(|en| {
                engine_online = en.contains(PathingEnables::ENGINE);
                en.set(PathingEnables::KATRENDER, katrender);
                Some(true)
            });
        #[cfg(todo)]
        if !engine_online && katrender && self.gameplay_map().is_some() {
            RenderEvent::StartEngine.try_send()
        }
    }

    fn toggle_api_bypass(&mut self, set: Option<bool>) {
        self.rx.enables.write_if(|en| {
            match set {
                Some(set) => en.set(PathingEnables::API_BYPASS, set),
                None => en.toggle(PathingEnables::API_BYPASS),
            }
            Some(true)
        });
    }
    #[cfg(feature = "paths-lua")]
    fn toggle_script_enable(&self, set: Option<bool>) {
        self.rx.enables.send_modify(|en| match set {
            Some(set) => en.set(PathingEnables::SCRIPTING_LUA, set),
            None => en.toggle(PathingEnables::SCRIPTING_LUA),
        });
    }

    async fn load_all(&mut self) {
        let res = Self::do_load_all(self.loader.clone())
            .await
            .context("Loading all paths");
        if let Err(e) = res {
            log::error!("{e:#}");
        }
    }

    async fn handle_gameplay(&mut self, gameplay: GameplayState, trans: GameplayTransition) {
        if let GameplayTransition::Loaded { initial: true, .. } = trans {
            #[cfg(todo)]
            if self.rx.is_katrender_enabled() && !self.rx.is_engine_active() {
                RenderEvent::StartEngine.try_send();
            }
        }
        match gameplay {
            GameplayState::Gameplay { map_id: Some(map_id) } => {
                let (new_map, instantaneous) = match trans {
                    | GameplayTransition::Map { prev_map_id: Some(prev), .. }
                    | GameplayTransition::Loaded { prev_map_id: Some(prev), .. }
                        if prev != map_id =>
                        (true, matches!(trans, GameplayTransition::Map { .. })),
                    _ => (false, false),
                };
                if new_map {
                    if instantaneous {
                        // make up for missing the loading screen...
                        self.handle_map_suspend(&gameplay);
                    }
                    self.handle_map_leave();
                }
                self.handle_map_enter(map_id)
            },
            GameplayState::Intermission { initial: false, .. } => self.handle_map_suspend(&gameplay),
            _ => (),
        }
    }

    async fn handle_message(&mut self, event: PathingEvent) -> Option<Interruption> {
        match event {
            PathingEvent::Nop => None,
            PathingEvent::Exit(interruption) => Some(interruption),
            PathingEvent::FanOut(events) => {
                let mut events = VecDeque::from(events);
                let mut out = None;
                while let Some(e) = events.pop_front() {
                    match e {
                        PathingEvent::Exit(int) => {
                            out = Some(int);
                            break
                        },
                        PathingEvent::FanOut(more) => {
                            let more = more.into_iter();
                            #[cfg(todo = "unnecessary")]
                            let e = match more.next() {
                                Some(e) => e,
                                None => continue,
                            };
                            for more in more.rev() {
                                events.push_front(more);
                            }
                            continue
                        },
                        PathingEvent::Nop => continue,
                        e => self.process_message(e).await,
                    }
                }
                out
            },
            e => {
                self.process_message(e).await;
                None
            },
        }
    }
    async fn process_message(&mut self, event: PathingEvent) {
        use PathingEvent::*;
        match event {
            LoadAll => self.load_all().await,
            LoadPack(path) => self.process_pack_activate(path),
            OffloadPack(path) => self.process_pack_deactivate(path),
            UnloadPack(path, remove) => self.process_pack_unload(path, remove),
            Refresh { include_datasources } => self.process_pack_refresh_all(include_datasources).await,
            ReloadAll(remove) => self.process_pack_reload_all(remove).await,
            UnloadAll(remove) => self.process_pack_unload_all(remove),
            PackLock { path } => self.process_pack_lock(path),
            PackUnlock { path } => self.process_pack_unlock(path),
            ReloadPack(path, remove) => self.process_pack_reload(path, remove),
            CategoryEnableSet(pack_path, cat, state) =>
                self.process_category_set(pack_path.rel(cat.path), state).await,
            CategoryEnableById(pack_path, cat_id, state) =>
                self.process_category_set_id(pack_path, cat_id, state).await,
            CategoryEnableCommit(pack_path, cats) => {
                self.category_commit_vis(pack_path, &mut cats.into_paths()).await
            },
            #[cfg(feature = "paths-filter")]
            ResetMarkerIds(ids) =>
                self.process_filter_clear_ids(ids),
            #[cfg(feature = "paths-filter")]
            ResetMarkerPath(path) =>
                self.process_filter_clear_path(path),
            #[cfg(feature = "paths-interact")]
            TriggerMarkerCopy { path, loaded_path, value, message } =>
                self.process_marker_copy(path, loaded_path, value, message).await,
            #[cfg(feature = "paths-interact")]
            TriggerMarkerInfo { path, loaded_path, message } =>
                self.process_marker_info(path, loaded_path, message).await,
            #[cfg(feature = "paths-interact")]
            DismissMarker { path, loaded_path, until, contexts, reset } =>
                self.process_marker_dismiss(path, loaded_path, until, contexts, reset),
            #[cfg(feature = "paths-interact")]
            InteractControl(msg) => {
                let cx = (&self.maps, &self.map_info, &self.filter_state, &self.settings);
                let followup = self.interact.process_event(&mut self.rx, cx, msg).await;
                self.spawn_message(followup);
            },
            ToggleKatRender => self.toggle_katrender().await,
            ApiBypass(set) => self.toggle_api_bypass(set),
            #[cfg(feature = "paths-lua")]
            ScriptsEnable(en) => self.toggle_script_enable(en),
            ReportResourceLoaded(loaded) => self.report_load(loaded).await,
            CollectGarbage { tick, aggressive } =>
                self.collect_garbage(tick, aggressive, self.gameplay_map()).await,
            VisibleToggle { context, set } => self.set_visible(context, set).await,
            Nop => (),
            RequestRebuildSpace { entities, bvh } => {
                self.debug_req_space_build(entities, bvh).await;
            },
            RequestRebuildVis { pack_path, partial, notify } => {
                self.debug_req_config_vis(pack_path, partial, notify).await;
            },
            RequestResourceRelease { pack_path } => {
                self.debug_req_resource_release(pack_path).await;
            },
            RequestResourceReport { pack_path } => {
                self.debug_req_resource_report(pack_path);
            },
            #[cfg(todo = "unused")]
            SpawnTask(task) => {
                self.tasks.spawn(task);
            },
            #[cfg(todo = "unused")]
            #[cfg(feature = "paths-filter")]
            Scheduled(when, events) => {
                self.scheduled_events.schedule_append(when.instant, events);
            },
            #[cfg(debug_assertions)]
            Exit(..) | FanOut(..) => unreachable!(),
            #[cfg(not(debug_assertions))]
            Exit(..) | FanOut(..) => (),
        }
    }

    pub(crate) async fn set_visible(&mut self, context: Option<MapContext>, set: Option<bool>) {
        let set = self.set_visible_settings(context, set).await;

        #[cfg(feature = "extension-nexus")]
        crate::QUICK_ACCESS_STATE.send_if_modified(|state| {
            let control = match context {
                Some(MapContext::Global) => TaimiControls::PATHING_MAP,
                Some(MapContext::Minimap) => TaimiControls::PATHING_MINIMAP,
                None => TaimiControls::PATHING_SPACE,
            };
            if state.contains(control) != set {
                state.toggle(control);
                true
            } else {
                false
            }
        });

        #[cfg(feature = "goggles")]
        match (context, set) {
            (None, true) =>
                Engine::try_send(SpaceEvent::GogglesRefreshLens { force: false, delay_override: Some(2) }),
            (None, false) => Engine::try_send(SpaceEvent::GogglesClearLens),
            _ => (),
        }
    }

    async fn handle_keybinds(&mut self, state: TaimiControls, changed: TaimiControls) {
        let pressed = state & changed;
        if pressed.intersects(TaimiControls::PATHING_SPACE) {
            CONTROLS.notify_handled(TaimiControls::PATHING_SPACE);
            self.set_visible(None, None).await;
        }
        if pressed.intersects(TaimiControls::PATHING_MAP) {
            CONTROLS.notify_handled(TaimiControls::PATHING_MAP);
            self.set_visible(Some(MapContext::Global), None).await;
        }
        if pressed.intersects(TaimiControls::PATHING_MINIMAP) {
            CONTROLS.notify_handled(TaimiControls::PATHING_MINIMAP);
            self.set_visible(Some(MapContext::Minimap), None).await;
        }
    }

    pub(crate) async fn handle_presses(&mut self, state: GameControls, changed: GameControls) {
        let _pressed = state & changed;
        #[cfg(feature = "paths-interact")]
        if _pressed.contains(GameControl::Miscellaneous_Interact) {
            if let Some(map_id) = self.filter_press_gameplay(GameControl::Miscellaneous_Interact) {
                self.handle_press_interact(map_id).await;
            }
        }
        #[cfg(feature = "scripts")]
        if let Some(msg) = script::ScriptMessage::gameplay_keybind(state, changed) {
            msg.try_send()
        }
    }

    /// ignore if not in-game or textbox has focus etc
    ///
    /// TODO: might still be possible to use if bound to a mouse button maybe?
    #[cfg(feature = "paths-interact")]
    fn filter_press_gameplay(&mut self, _control: GameControl) -> Option<MapIndex> {
        let ml = match () {
            #[cfg(todo = "unnecessary")]
            _ => rt::mumble_link_ptr().ok(),
            _ => self.rx.interact.player_pos.ml,
        };
        self.gameplay_map().and_then(|map_id| {
            let is_text_input = ml.map(|ml| ml.read_ui_state())
                .map(|state| UiState::from(state).contains(UiState::TextInput))
                .unwrap_or(true);
            (!is_text_input).then_some(map_id)
        })
    }
    #[cfg(feature = "paths-interact")]
    pub(crate) async fn handle_press_interact(&mut self, map_id: MapIndex) {
        let interact_ctx = (&self.map_info, &self.maps, map_id, &self.filter_state);
        if self.rx.interact.try_throttle_press() {
            let followup = self.interact.trigger_interact_action(&mut self.rx.interact, interact_ctx, InteractReactor::INTERACT_ACTION).await;
            self.process_or_spawn_message(followup).await;
        } else {
            log::debug!("throttling interact handler");
        }
        self.rx.interact.report_throttle_press();
    }

    pub(crate) async fn collect_garbage(&mut self, tick: u32, aggressive: bool, map_id: Option<MapIndex>) {
        let mut map_info_dirty;
        let mut maps_dirty;
        match (tick, aggressive) {
            (_, true) => {
                map_info_dirty = self.map_info.clear(map_id) > 0;
                maps_dirty = self.maps.prune(Some(&self.map_info));
                self.packs.age_tick(Some(&self.map_info), true);
            },
            (tick, _) => {
                for _ in 0..tick {
                    self.map_info.age_tick(map_id);
                    self.maps.age_tick(map_id);
                }
                map_info_dirty = self.map_info.prune(Some(&self.packs));
                maps_dirty = self.maps.prune(Some(&self.map_info));
                for _ in 0..tick {
                    self.packs.age_tick(Some(&self.map_info), false);
                }
            },
        }
        map_info_dirty |= self.map_info.prune(Some(&self.packs));
        maps_dirty |= self.maps.prune(Some(&self.map_info));
        let packs_dirty = {
            let unloaded = self.pack_unload_unused();
            !unloaded.is_empty()
        };
        let dirty = !map_info_dirty && !maps_dirty && !packs_dirty;
        let maps = (&self.map_info, &self.maps);
        if dirty || aggressive {
            self.space.collect_garbage(maps, map_id, aggressive);
            #[cfg(feature = "paths-interact")]
            {
                self.interact.collect_garbage(&mut self.rx, maps, map_id, aggressive).await;
            }
        }
    }

    #[cfg(todo = "unused")]
    pub fn external_filter_state() -> Option<ExternalFilterState> {
        Controller::with_sender(|s| {
            let bypass = s
                .pathing
                .as_ref()
                .map(|p| p.enables.borrow().clone())
                .unwrap_or_default()
                .contains(PathingEnables::API_BYPASS);
            s.api.as_ref().map(|a| {
                let festivals = a.festivals.borrow().get();
                let (clears, achievements) = match bypass {
                    true => Default::default(),
                    false => (a.raids.borrow().clone(), a.achievements.borrow().clone()),
                };
                (festivals, clears, achievements)
            })
        })
        .flatten()
    }

    async fn process_or_spawn_message(&mut self, msg: PathingEvent) {
        match msg {
            e @ (PathingEvent::Nop | PathingEvent::Exit(..) | PathingEvent::FanOut(..)) =>
                self.spawn_message(e),
            #[cfg(feature = "paths-interact")]
            e @ PathingEvent::InteractControl(..) =>
                self.spawn_message(e),
            event => self.process_message(event).await,
        }
    }
    fn spawn_message(&mut self, msg: PathingEvent) {
        match msg {
            PathingEvent::Nop => (),
            #[cfg(feature = "paths-interact")]
            PathingEvent::InteractControl(interact::InteractMessage::Nop) => (),
            #[cfg(feature = "paths-interact")]
            e @ PathingEvent::InteractControl(..) => {
                // TODO: deleteme
                self.tasks.spawn(async move {
                    tokio::time::sleep(Duration::from_millis(7)).await;
                    Ok(e)
                });
            }
            e => {
                self.tasks.spawn(future::ready(Ok(e)));
            },
        }
    }

    #[inline]
    pub fn try_send(e: PathingEvent) -> bool {
        Controller::with_sender(|s| s.pathing_try_send(e)).unwrap_or(false)
    }
}

impl InterruptionSignal for PathingEvent {
    fn interrupted(&self) -> Option<Interruption> {
        match self {
            &Self::Exit(reason) => Some(reason),
            _ => None,
        }
    }
}
