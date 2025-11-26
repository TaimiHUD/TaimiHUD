use {
    crate::{
        controller::{
            pathing::{
                registry::{CategoryPath, MapIndex, MarkerId, MarkerPath, PackConfig, PackLoader, PackMapPath, PackPath, PoiPath, SharedLoaderPackInfo, TrailPath}, setup::{SetupPoi, SetupTrail}, state::{hidden::HideContext, shared::SharedMapPackInfo}, visible::{InteractionEvent, LoadedTrail}, FestivalState, PathingController
            }, Controller
        }, exports::runtime::{self as rt, bindings::{ControlsReceiver, GameControl, GameControls, TaimiControls, TaimiReceiver, CONTROLS}, watched::{Watched, Watcher}}, render::{machine::{MumbleIdentityUpdate, RenderTaskPriority}, RenderState}, space::pack::PackSpace, Interruption
    },
    anyhow::Context, futures::{future, stream::{self, FusedStream}, FutureExt, StreamExt}, glamour::Point3, std::{collections::BTreeMap, future::Future, iter, pin::Pin, sync::Arc, time::SystemTime}, strum_macros::Display, taimi_meta::ui::{gameplay::GameplayTransition, GameplayState, MapContext, UiState}, taimi_pack::attributes::keys::Guid,
    tokio::{
        select, sync::{broadcast, mpsc, watch}, task::{AbortHandle, JoinSet}, time::{interval, sleep, sleep_until, Duration, Instant, Interval, Sleep}
    },
};

impl PathingController {
    pub async fn run(&mut self, ctx: &mut PathingEventContext) -> anyhow::Result<()> {
        self.setup(ctx).await;

        while ctx.active {
            let int = self.turn(ctx).await;
            if let Some(reason) = int {
                let res = self.exit(ctx, reason).await;
                ctx.active = false;
                return res
            }
        }

        Ok(())
    }

    async fn turn(&mut self, ctx: &mut PathingEventContext) -> Option<Interruption> {
        if ctx.rx.is_closed() {
            return Some(self.exit_drain(ctx));
        }
        let mut gameplay_prev = ctx.gameplay.get().into_owned();
        select! {
            e = ctx.rx.recv() => match e {
                None =>
                    return Some(Interruption::Unspecified),
                Some(e) => match self.handle_event(e, ctx).await.context("Pathing controller") {
                    Ok(Some(int)) => {
                        ctx.rx.close();
                        return Some(int)
                    },
                    Ok(None) => (),
                    Err(e) => {
                        log::error!("{e:#}");
                    },
                },
            },
            e = ctx.rx_interactions.recv() => match e {
                Ok(e) => {
                    self.handle_interaction(ctx, e).await;
                },
                Err(broadcast::error::RecvError::Lagged(amt)) => {
                    log::warn!("interactions lagged behind by {amt}");
                },
                Err(broadcast::error::RecvError::Closed) => (),
            },
            gameplay = ctx.gameplay.when_changed(), if self.enabled => {
                let gameplay = gameplay.clone();
                let map_id = gameplay.gameplay_map();
                let trans = map_id.and_then(|map| gameplay_prev.commit_ingame(map.get()))
                    .or_else(|| gameplay_prev.commit_intermission())
                    .unwrap_or_else(|| gameplay.latest_transition());
                self.update_filter_state(ctx);
                self.handle_gameplay(ctx, gameplay, trans).await;
                if map_id.is_some() {
                    ctx.update_tick.reset_after(Self::UPDATE_INTERVAL_RESPONSIVE);
                    //ctx.filter_state_signal = true;
                }
            },
            controls = ctx.controls.wait() => if let Ok((controls_state, controls_changed)) = controls {
                let state = *controls_state;
                self.handle_presses(ctx, state, controls_changed).await;
            },
            keybinds = ctx.keybinds.wait() => if let Ok((binds_state, binds_changed)) = keybinds {
                self.handle_keybinds(binds_state, binds_changed).await;
            },
            () = ctx.festivals.when_changed() => {
                self.filter_state.festival = ctx.festivals.read_update().clone();
                ctx.filter_state_signal = true;
            },
            _ = ctx.next_schedule.as_mut() => {
                // self.update_filter_state_schedule(ctx);
                ctx.next_schedule.as_mut().reset(Instant::now() + PathingEventContext::SCHEDULE_TIMEOUT);
                ctx.filter_state_signal = true;
            },
            Ok(..) = ctx.mumble_identity.changed() => {
                if let Some(identity) = &*ctx.mumble_identity.borrow_and_update() {
                    self.filter_state.character.update_from_mumblelink(identity);
                    ctx.filter_state_signal = true;
                }
            },
            _ = future::ready(()), if ctx.filter_state_signal && ctx.gameplay_map().is_some() => {
                if let Some(map_id) = ctx.gameplay_map() {
                    ctx.filter_state_signal = false;
                    self.update_filter_state(ctx);
                    if self.update_loaded_visibility() {
                        self.visibility_send(map_id).await;
                        self.mark_map_state_dirty(ctx, map_id);
                    }
                }
            },
            _ = ctx.loader_pack_info.changed() => {
                // XXX: check if anything actually changed idk...
                ctx.pack_configs = Box::new(Self::packs().read().await.watch_config_changes());
            },
            Some((pack, _)) = ctx.pack_configs.next() => {
                self.handle_config_change(ctx, pack).await
            },
            _ = ctx.update_tick.tick() => {
                if let Some(map_id) = ctx.gameplay_map() {
                    self.handle_update_tick(ctx, map_id).await;
                } else {
                    ctx.update_tick.reset_after(Self::UPDATE_INTERVAL_SLOW);
                }
            },
            Some(res) = ctx.tasks.join_next(), if !ctx.tasks.is_empty() => match res {
                Ok(Some(e)) => {
                    let res = self.handle_event(e, ctx).await.context("Pathing controller");
                    if let Some(Some(int)) = rt::log::error_ok(res) {
                        ctx.rx.close();
                        return Some(int)
                    };
                },
                Ok(None) => (),
                Err(e) =>
                    crate::log_join_error("pathing", e),
            },
        }

        None
    }

    fn exit_drain(&mut self, ctx: &mut PathingEventContext) -> Interruption {
        while let Ok(e) = ctx.rx.try_recv() {
            match e {
                PathingEvent::Exit(reason) =>
                    return reason,
                _ => (),
            }
        }

        Interruption::Unspecified
    }

    async fn exit(&mut self, ctx: &mut PathingEventContext, reason: Interruption) -> anyhow::Result<()> {
        #[cfg(todo = "unnecessary")]
        for handle in ctx.filter_expiry.values() {
            handle.abort()
        }
        ctx.filter_expiry.clear();
        ctx.tasks.abort_all();
        ctx.rx_interactions = broadcast::Sender::new(1).subscribe();

        match reason {
            Interruption::Abort => return Ok(()),
            _ => (),
        }

        ctx.pack_info.send_if_modified(|shared| {
            shared.shared_loader = Some(self.loader.clone());
            // TODO: true (once things consider this a shutdown state)
            false
        });

        ctx.tasks.detach_all();

        Ok(())
    }

    pub(crate) async fn handle_event(&mut self, event: PathingEvent, ctx: &mut PathingEventContext) -> anyhow::Result<Option<Interruption>> {
        use PathingEvent::*;
        match event {
            Exit(reason) => return Ok(Some(reason)),
            ReloadAll(remove) => self.reload_all(ctx, remove).await,
            ReloadPack(path, remove) => self.reload_pack(ctx, path, remove).await,
            LoadAll => self.load_all(ctx).await,
            UnloadAll(remove) => self.unload_all(ctx, remove).await,
            UnloadPack(path, remove) =>
                self.unload_pack(ctx, path, remove).await,
            LoadPack(path) =>
                return self.load_pack(ctx, path).await.map(|_| None),
            #[cfg(deleteme)]
            PreparePack(path) => {
                if let Some(map_id) = ctx.gameplay_map() {
                    log::debug!("TODO: change prepare type to include mapid?");
                    self.prepare_pack(ctx, path.rel(map_id)).await;
                } else {
                    log::warn!("TODO: can't prepare {path}, no map loaded");
                }
            },
            UpdateMapTrails { path, updates } => {
                log::debug!("TODO: UpdateMapTrails");
            },
            SetupTrails { path, trails, pois } => {
                if ctx.gameplay_map().map(|map| map != path.path).unwrap_or(false) {
                    log::debug!("discarding outdated setup for {path}");
                    return Ok(None)
                }
                return self.setup_pack(ctx, path, trails, pois).await.map(|()| None)
            },
            RequestDisabledPaths => {
                if let Some(map_id) = ctx.gameplay_map() {
                    self.update_loaded_visibility();
                    self.visibility_send(map_id).await;
                    self.mark_map_state_dirty(ctx, map_id);
                }
            },
            #[cfg(todo)]
            CategoryVisibility(path, state) => {
                self.handle_vis(ctx, path, state).await;
            },
            CategorySetToggle(path, state) => {
                self.handle_toggle(path, state).await;
            },
            GuidReset(guids) => {
                self.handle_guid_reset(ctx, guids);
            },
            ResetMarker(path) => {
                let map_path = ctx.gameplay_map()
                    .map(|map_id| path.map_root(|r| r.rel(map_id)));
                let marker_ids = iter::once(MarkerId::from(path))
                    .chain(map_path.map(MarkerId::from));
                let mut dirty = false;
                for marker_id in marker_ids {
                    dirty |= self.filter_state.hidden.reset(MarkerId::from(path));
                    dirty |= ctx.unexpire(&marker_id);
                }
                if dirty {
                    self.mark_hidden_dirty(ctx, map_path.map(|p| p.root));
                    ctx.filter_state_signal = true;
                }
            },
            DismissMarker(path, delay, contexts) => {
                if let Some(expiry) = delay.map(|delay| SystemTime::now().checked_add(delay)) {
                    self.handle_dismiss(ctx, path, delay, expiry, contexts).await;
                } else {
                    log::error!("unable to determine expiry time for {path} of {delay:?}");
                }
            },
            ToggleKatRender => self.toggle_katrender(ctx).await,
            LowMemory => self.lowmem_activate(ctx).await,
            VisibleToggle { context, set } => self.set_visible(context, set).await,
            FanOut(events) => for e in events {
                let res = Box::pin(self.handle_event(e, ctx)).await
                    .context("Pathing controller");
                if let Some(Some(int)) = rt::log::error_ok(res) {
                    return Ok(Some(int))
                }
            },
        }

        Ok(None)
    }

    pub(crate) async fn handle_keybinds(&mut self, state: TaimiControls, changed: TaimiControls) {
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

    async fn handle_presses(&mut self, ctx: &mut PathingEventContext, state: GameControls, changed: GameControls) {
        let pressed = state & changed;
        if pressed.contains(GameControl::Miscellaneous_Interact) {
            // TODO: might still be possible to use if bound to a mouse button maybe?
            let is_text_input = || rt::mumble_link_ptr().map(|ml| ml.read_ui_state())
                .map(|state| UiState::from(state).contains(UiState::TextInput))
                .unwrap_or(false);
            let map_id = match ctx.gameplay_map() {
                Some(..) if is_text_input() =>
                    None,
                map_id => map_id,
            };
            if let Some(map_id) = map_id {
                self.handle_press_interact(ctx, map_id);
            }
        }
    }

    pub async fn handle_gameplay(&mut self, ctx: &mut PathingEventContext, state: GameplayState, trans: GameplayTransition) {
        match state {
            GameplayState::Gameplay { map_id: Some(map_id) } => {
                let (new_map, instantaneous) = match trans {
                    | GameplayTransition::Map { prev_map_id: Some(prev), .. }
                    | GameplayTransition::Loaded { prev_map_id: Some(prev), .. }
                    if prev != map_id => (
                        true,
                        matches!(trans, GameplayTransition::Map { .. }),
                    ),
                    _ => (false, false),
                };
                if new_map {
                    if instantaneous {
                        // make up for missing the loading screen...
                        self.handle_map_suspend(ctx);
                    }
                    self.handle_map_leave(ctx);
                }
                self.handle_map_enter(map_id, ctx).await
            },
            GameplayState::Intermission { initial: false, .. } =>
                self.handle_map_suspend(ctx),
            _ => (),
        }
    }

    async fn toggle_katrender(&mut self, ctx: &mut PathingEventContext) {
        {
            let mut settings = self.loader.settings.write().await;
            settings.toggle_katrender();
            settings.mark_dirty();
            self.enabled = settings.enable_katrender;
        }
        if self.enabled && ctx.gameplay_map().is_some() {
            let _ = ctx.gameplay.watch.try_mark_changed();
        } else if !self.enabled {
            self.handle_map_suspend(ctx);
            self.handle_map_leave(ctx);
            self.unload_all(ctx, false).await;
        }
    }
}

pub struct PathingEventContext {
    pub active: bool,
    pub rx: mpsc::Receiver<PathingEvent>,
    pub rx_interactions: broadcast::Receiver<InteractionEvent>,
    pub gameplay: Watched<GameplayState>,
    pub controls: ControlsReceiver,
    pub keybinds: TaimiReceiver,
    pub festivals: Watcher<FestivalState>,
    pub pack_info: watch::Sender<SharedMapPackInfo>,
    pub mumble_identity: watch::Receiver<Option<MumbleIdentityUpdate>>,
    pub pack_configs: Box<dyn FusedStream<Item = (PackPath, watch::Receiver<Arc<PackConfig>>)> + Send + Unpin + 'static>,
    pub update_tick: Interval,
    pub tasks: JoinSet<Option<PathingEvent>>,
    pub filter_expiry: BTreeMap<MarkerId, AbortHandle>,
    pub next_schedule: Pin<Box<Sleep>>,

    pub loader_pack_info: watch::Receiver<SharedLoaderPackInfo>,

    pub player_pos: Point3<PackSpace>,
    pub filter_state_signal: bool,
}

impl PathingEventContext {
    pub fn new(
        loader: &Arc<PackLoader>,
        rx: mpsc::Receiver<PathingEvent>,
        gameplay: watch::Receiver<GameplayState>,
        festivals_tx: watch::Sender<FestivalState>,
        pack_info: watch::Sender<SharedMapPackInfo>,
        mumble_identity: watch::Receiver<Option<MumbleIdentityUpdate>>,
    ) -> Self {
        let mut update_tick = interval(PathingController::UPDATE_INTERVAL_RESPONSIVE);
        update_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let rx_interactions = pack_info.borrow().interactions.subscribe();
        Self {
            rx,
            gameplay: Watched::start_receiving(gameplay),
            controls: CONTROLS.subscribe_controls(),
            keybinds: CONTROLS.subscribe_taimi(),
            active: true,
            festivals: Watcher::with_sender(festivals_tx),
            pack_info,
            mumble_identity,
            pack_configs: Box::new(stream::pending()),
            update_tick,
            loader_pack_info: loader.shared_pack_info.subscribe(),
            tasks: JoinSet::new(),
            filter_expiry: BTreeMap::new(),
            next_schedule: Box::pin(sleep(Self::SCHEDULE_TIMEOUT)),
            player_pos: Point3::INFINITY,
            filter_state_signal: true,
            rx_interactions,
        }
    }

    pub fn spawn<F>(&mut self, f: F) -> AbortHandle where
        F: Future<Output = ()> + Send + 'static,
    {
        self.tasks.spawn(f.map(|()| None))
    }

    pub fn spawn_render<F>(&mut self, prio: RenderTaskPriority, f: F) -> AbortHandle where
        F: FnOnce(&mut RenderState) + Send + 'static,
    {
        self.spawn(Controller::schedule_render(prio, f).then(|rx| rx).map(|res| {
            let (None | Some(())) = rt::log::debug_ok_with("render task lost", res);
        }))
    }

    pub fn gameplay_map(&mut self) -> Option<MapIndex> {
        self.gameplay.try_get_mut()
            .and_then(|state| state.gameplay_map())
    }

    pub fn spawn_alert(&mut self, message: String, duration: Duration) {
        static PATHING_ALERT_HACK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

        log::debug!("TODO: replace alert system lol");
        self.spawn(async move {
            let lock = PATHING_ALERT_HACK.lock().await;
            let alert = crate::timer::TextAlert {
                timer: crate::timer::TimerFile {
                    icon: Default::default(),
                    map_id: Default::default(),
                    reset: crate::timer::TimerTrigger {
                        require_entry: false,
                        require_combat: false,
                        require_departure: false,
                        require_out_of_combat: false,
                        radius: None,
                        antipode: None,
                        position: None,
                        key_bind: None,
                        kind: crate::timer::TimerTriggerType::Key,
                    },
                    phases: Default::default(),
                    author: Default::default(),
                    description: Default::default(),
                    name: Default::default(),
                    category: Default::default(),
                    id: Default::default(),
                    path: None,
                    association: None,
                }.into(),
                message: message.into(),
            };
            let timer = alert.timer.clone();
            crate::render::RenderState::try_send(crate::render::RenderEvent::AlertStart(alert));
            sleep(duration).await;
            crate::render::RenderState::try_send(crate::render::RenderEvent::AlertEnd(timer));
            drop(lock)
        });
    }

    pub fn player_pos(&self) -> Option<Point3<PackSpace>> {
        let player_pos = self.player_pos;
        (!player_pos.x.is_infinite()).then_some(player_pos)
    }

    pub fn read_player_pos() -> Option<Point3<PackSpace>> {
        match rt::mumble_link_ptr() {
            Ok(ml) => Some(Point3::from_array(ml.read_avatar().position)),
            _ => None,
        }
    }

    pub fn unexpire(&mut self, item: impl AsRef<MarkerId>) -> bool {
        if let Some(handle) = self.filter_expiry.remove(item.as_ref()) {
            handle.abort();
            true
        } else {
            false
        }
    }
    pub fn spawn_expire_at<F>(&mut self, item: MarkerId, expiry: SystemTime, duration: Option<Duration>, f: F) where
        F: Future<Output = Option<PathingEvent>> + Send + 'static,
    {
        let now = Instant::now();
        let deadline = expiry.duration_since(SystemTime::now()).ok()
            .map(|expiry|
                now.checked_add(expiry)
                .or_else(|| duration.and_then(|d| now.checked_add(d)))
            );
        let handle = self.tasks.spawn(async move {
            match deadline {
                Some(Some(deadline)) => sleep_until(deadline).await,
                // duration too big means forever...
                #[cfg(todo = "unnecessary")]
                Some(None) => future::pending().await,
                Some(None) => return None,
                // otherwise expiry is earlier than now, so continue immediately
                None => (),
            }
            f.await
        });
        // TODO: there's probably an entry replace api for this...
        self.unexpire(&item);
        self.filter_expiry.insert(item, handle);
    }
    pub fn expire_at(&mut self, item: MarkerId, expiry: SystemTime, duration: Option<Duration>) {
        self.spawn_expire_at(item.clone(), expiry, duration, async move {
            match item.marker_path() {
                Some(marker) => Some(PathingEvent::ResetMarker(marker)),
                _ => Some(PathingEvent::GuidReset(vec![item.into()])),
            }
        })
    }

    pub(super) const SCHEDULE_TIMEOUT: Duration = Duration::from_secs(60 * 60 * 12);
}

#[derive(Debug, Clone, Display)]
pub enum PathingEvent {
    VisibleToggle { context: Option<MapContext>, set: Option<bool> },
    ReloadAll(bool),
    ReloadPack(PackPath, bool),
    LoadAll,
    UnloadAll(bool),
    UnloadPack(PackPath, bool),
    LoadPack(PackPath),
    /// Post-load
    #[cfg(deleteme)]
    PreparePack(PackPath),
    SetupTrails {
        path: PackMapPath,
        trails: Vec<SetupTrail>,
        pois: Option<Vec<SetupPoi>>,
    },
    UpdateMapTrails {
        path: PackMapPath,
        updates: Vec<(TrailPath, LoadedTrail)>,
    },
    RequestDisabledPaths,
    CategorySetToggle(CategoryPath<PackPath>, Option<bool>),
    GuidReset(Vec<Guid>),
    ResetMarker(MarkerPath<PackPath>),
    DismissMarker(PoiPath<PackMapPath>, Option<Duration>, Vec<HideContext>),
    #[cfg(todo)]
    CategoryVisibility(CategoryPath<PackPath>, VisibilityFlags),
    ToggleKatRender,
    LowMemory,
    Exit(Interruption),
    FanOut(Vec<PathingEvent>)
}

impl PathingEvent {
    #[inline]
    pub fn try_send(self) {
        PathingController::try_send(self);
    }

    pub const VISIBLE_TOGGLE_SPACE: Self = Self::VisibleToggle { context: None, set: None };
    pub const fn visible_toggle(context: MapContext) -> Self {
        Self::VisibleToggle { context: Some(context), set: None }
    }
}
