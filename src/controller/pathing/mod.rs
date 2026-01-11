#[doc(no_inline)]
pub use taimi_meta::coords::LocalSpace as PackSpace;
use {
    self::{
        registry::PackLoader,
        space::{SpaceContext, SpacePackShared},
        state::{LoadedMapInfo, LoadedMaps, LoadedPacks},
    },
    crate::{
        controller::{
            api::{AchievementState, RaidState},
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
        settings::{pathing::PathingSettings, Settings, SettingsLock},
        space::{engine::SpaceEvent, Engine},
        Interruption,
        InterruptionSignal,
    },
    anyhow::Context,
    futures::{
        future::Either,
        stream::{self, FusedStream},
        StreamExt,
    },
    std::{collections::VecDeque, future::Future, mem, pin::Pin, sync::Arc},
    strum_macros::Display,
    taimi_hoard::loc::LocationRef,
    taimi_meta::{
        packs::{
            collections::{CategorySet, PackSet},
            id::MarkerId,
            CategoryPath,
            PackPath,
        },
        ui::{
            gameplay::{GameplayState, GameplayTransition},
            MapContext,
        },
    },
    taimi_pack::attributes::Festivals,
    taimi_sync::watched,
    tokio::{select, sync::Semaphore, task::JoinSet},
};

#[allow(unused_imports)]
pub use self::{
    config::PackConfig,
    festivals::FestivalFixup,
    registry::{LoaderBox, UnloadedReason},
    shared::{PathingEnables, PathingReceiver, PathingSender, PathingShared},
    state::VisibilityFlagsExt,
};

mod config;
mod festivals;
pub mod info;
pub mod registry;
mod setup;
pub mod shared;
pub mod space;
pub mod state;

pub type ExternalFilterState = (Festivals, Arc<RaidState>, Arc<AchievementState>);

#[derive(Debug, Display)]
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
    ReloadAll(bool),
    LoadAll,
    UnloadAll(bool),
    /// toggle or set category state
    CategoryEnableSet(PackPath, CategoryPath, Option<bool>),
    /// act upon a batch of changes to [shared::SharedPackLoad::config]
    CategoryEnableCommit(PackPath, CategorySet),
    ToggleKatRender,
    ApiBypass(Option<bool>),
    ReportResourceLoaded(shared::LoadReport),
    FanOut(Vec<PathingEvent>),
    Exit(Interruption),
    Nop,
}
pub type PathingTaskBox = Pin<Box<dyn Future<Output = Option<PathingEvent>> + Send + 'static>>;

pub(crate) struct PathingController {
    loader: Arc<PackLoader>,
    rx: PathingReceiver,
    tasks: JoinSet<anyhow::Result<PathingEvent>>,
    controls: ControlsReceiver,
    keybinds: TaimiReceiver,
    settings: SettingsLock,
    active: bool,
    packs: LoadedPacks,
    map_info: LoadedMapInfo,
    maps: LoadedMaps,
    space: SpaceContext,
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
            settings,
            active: true,
            packs: Default::default(),
            map_info: Default::default(),
            maps: Default::default(),
            pack_configs: Box::new(stream::pending()),
            pack_configs_sig: PackPath::default(),
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
        let gameplay_prev = self.rx.gameplay.cached.clone().unwrap_or(GameplayState::INITIAL);
        let load_throttle_prev = self
            .rx
            .load_throttle
            .cached
            .clone()
            .unwrap_or(PathingSettings::DEFAULT_LOAD_SIMULTANEOUS);
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
                for path in &packs_dirty {
                    if let Some(map_path) = map_id.map(|map| path.rel(map)) {
                        if let Some(pack) = self.packs.lookup_ref(&map_path.root) {
                            if !pack.info.has_map(map_path.path) {
                                continue
                            }
                        }
                        let _ = self.prepare_for_pack_map(map_path, true);
                    }
                }
                for path in &configs_dirty {
                    self.reload_config_for(path);
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
            trail_reqs = SpaceContext::recv_trail_requests(&mut self.space.trail_geometry, &self.space.inflight) => {
                for trail in trail_reqs {
                    log::debug!("processing trail req {trail}");
                    let id = SpacePackShared::trail_geometry_id(&trail);
                    self.request_trail_load(id);
                }
            },
            texture_reqs = SpaceContext::recv_texture_requests(&mut self.space.texture_loads, &self.space.inflight) => {
                for path in texture_reqs {
                    let id = MarkerId::for_marker(path);
                    if self.request_texture_load(id) {
                        log::debug!("processing tex req {path}");
                    } else {
                        log::trace!("reserving tex req {path}");
                    }
                }
            },
            Ok(_) = self.space.maps_rx.changed() => {
                self.space_pack_updates().await;
            },
            _ = self.rx.festivals.changed() => {
                self.external_filters_updated().await;
            },
            _ = self.rx.achievements.changed() => {
                self.external_filters_updated().await;
            },
            _ = self.rx.raids.changed() => {
                self.external_filters_updated().await;
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
        self.tasks.abort_all();

        match reason {
            Interruption::Abort => return Ok(()),
            _ => (),
        }

        self.tasks.shutdown().await;

        Ok(())
    }

    async fn setup(&mut self) {
        let get_settings = {
            let enables = self.rx.enables.clone();
            let load_throttle = self.rx.load_throttle.watch.sender();
            let settings = self.settings.clone();
            async move {
                let mut enable_flags = enables.borrow().clone();
                let settings = settings.read().await;
                let load_simultaneous = settings.pathing.as_ref().and_then(|p| p.load_simultaneous);
                enable_flags.set(PathingEnables::KATRENDER, settings.enable_katrender);
                drop(settings);
                enables.send_replace(enable_flags);
                if let Some(load_simultaneous) = load_simultaneous {
                    load_throttle.send_replace(load_simultaneous);
                }
            }
        };
        let preload = self.preload_all();
        let ((), ()) = tokio::join!(preload, get_settings);
    }

    async fn toggle_katrender(&self) {
        let mut settings_lock = Settings::async_write()
            .await
            .expect("Settings unitialized, impossible");
        settings_lock.toggle_katrender();
        let katrender = settings_lock.enable_katrender;
        drop(settings_lock);
        self.rx
            .enables
            .send_modify(|en| en.set(PathingEnables::KATRENDER, katrender));
    }

    fn toggle_api_bypass(&self, set: Option<bool>) {
        self.rx.enables.send_modify(|en| match set {
            Some(set) => en.set(PathingEnables::API_BYPASS, set),
            None => en.toggle(PathingEnables::API_BYPASS),
        });
    }

    /// TODO: this sanely
    async fn external_filters_updated(&mut self) {
        self.update_loaded_visibility();
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
        if let GameplayTransition::Loaded { initial: true, .. } = trans {}
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
                        self.handle_map_suspend(true);
                    }
                    self.handle_map_leave();
                }
                self.handle_map_enter(map_id)
            },
            GameplayState::Intermission { initial: false, .. } => self.handle_map_suspend(false),
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
            ReloadAll(remove) => self.process_pack_reload_all(remove).await,
            UnloadAll(remove) => self.process_pack_unload_all(remove),
            ReloadPack(path, remove) => self.process_pack_reload(path, remove),
            CategoryEnableSet(pack_path, cat, state) =>
                Self::handle_toggle(&self.loader, pack_path.rel(cat.path), state).await,
            CategoryEnableCommit(pack_path, cats) => {
                let changed = cats.into_iter().map(CategoryPath::with_path);
                Self::category_commit_vis(&self.loader, pack_path, changed).await
            },
            ToggleKatRender => self.toggle_katrender().await,
            ApiBypass(set) => self.toggle_api_bypass(set),
            ReportResourceLoaded(loaded) => self.report_load(loaded).await,
            VisibleToggle { context, set } => self.set_visible(context, set).await,
            Nop => (),
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
        let pressed = state & changed;
        if pressed.contains(GameControl::Miscellaneous_Interact) {
            self.handle_press_interact().await;
        }
    }

    pub(crate) async fn handle_press_interact(&mut self) {
        log::trace!("TODO: player interaction");
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

    #[inline]
    pub fn try_send(e: PathingEvent) -> bool {
        Controller::with_sender(|s| s.pathing_try_send(e)).unwrap_or(false)
    }
}

impl PathingEvent {
    #[inline]
    pub fn try_send(self) {
        let _ = PathingController::try_send(self);
    }

    pub const VISIBLE_TOGGLE_SPACE: Self = Self::VisibleToggle { context: None, set: None };
    pub const fn visible_toggle(context: MapContext) -> Self {
        Self::VisibleToggle { context: Some(context), set: None }
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
