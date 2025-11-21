use taimi_pack::category::id::FullIdRef;

use {
    self::{registry::{CategoryIndex, LoadedPack, PackLoader, PackPath, PackRegistry, PoiIndex, RecentlyUsed, TrailIndex, UnloadedReason}, visible::LoadedMapPack}, crate::{controller::Controller, exports::runtime::{self as rt, bindings::{ControlsReceiver, GameControl, GameControls, TaimiControls, TaimiReceiver, CONTROLS}, locator::{LocationMut, LocationRef}, watched::{Watched, Watcher}, Locator}, render::{machine::RenderTaskPriority, RenderEvent, RenderState}, settings::{pathing::{FestivalPreference, TriggerKind}, state::SaveState, PathingSettings, Settings, SourceKind}, space::{
            engine::SpaceEvent, pack::{trail::TrailParams, PackSpace}, Engine
        }, Interruption}, anyhow::{anyhow, Context}, bitvec::vec::BitVec, filter::{FilterState, MarkerFilter, AutoReset, HideContext}, futures::{future, stream::{self, FusedStream}, FutureExt, StreamExt}, glamour::Point3, registry::{CategoryPath, MapIndex, PackConfig, PackInfo, PackMapPath, PoiPath, SharedLoaderPackInfo, TrailPath}, state::{MarkerIndex, MarkerPath}, std::{cmp, collections::{btree_map, BTreeMap, BTreeSet, BinaryHeap}, fmt, future::Future, iter, num::NonZero, ops, path::{Path, PathBuf}, pin::Pin, sync::Arc, time::{SystemTime, UNIX_EPOCH}}, strum_macros::Display, taimi_meta::ui::{gameplay::GameplayTransition, GameplayState, MapContext, UiState}, taimi_pack::attributes::{keys::Guid, Festival, Festivals},
    tokio::{
        fs::create_dir_all, select, sync::{broadcast, mpsc, watch, RwLock}, task::{AbortHandle, JoinSet}, time::{interval, sleep, sleep_until, Duration, Instant, Interval, Sleep}
    }, visible::{InteractionEvent, InteractionEventAction, InteractivePoi, LoadedCategory, LoadedPoi, LoadedTrail, SpaceLoader, SpacePoiBuilder, SpaceTrailBuilder, VisibilityFlags}
};
use crate::render::machine::MumbleIdentityUpdate;
pub use self::state::shared::{SharedMapPackInfo, SharedMapPackState};

pub mod registry;
pub mod festivals;
pub mod visible;
pub mod filter;
pub mod state;

#[derive(Debug, Clone, Display)]
pub enum PathingEvent {
    VisibleToggle { context: Option<MapContext>, set: Option<bool> },
    ReloadAll,
    LoadAll,
    UnloadAll,
    LoadPack(PackPath),
    /// Post-load
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
    Exit(Interruption),
    FanOut(Vec<PathingEvent>)
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
    pub filter_expiry: BTreeMap<Result<Guid, MarkerPath<PackPath>>, AbortHandle>,
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

    #[cfg(deleteme)]
    pub async fn pack_changes(pack_configs: &mut [(watch::Receiver<PackConfig>, ReusableBoxFuture<'static, Result<(), watch::error::RecvError>>)]) -> PackPath {
        let amt = pack_configs.len();
        #[cfg(todo)]
        loop {
            for (i, c) in pack_configs.iter_mut().enumerate() {
                let r = c.changed().await;
                ready = Some((PackPath::with_path(i as PackIndex), r));
            }
            let mut ready = stream::iter(pack_configs.iter_mut().enumerate().map(|(i, (_, c))|
                c.map(move |c| (PackPath::with_path(i as PackIndex), c))
            )).buffer_unordered(amt);
            #[cfg(deleteme)]
            let ready = Box::pin(ready).next().await;
            match ready.next().await {
                Some((p, Ok(()))) => return p,
                Some((_p, Err(..))) => (),
                None => break,
            }
        }
        future::pending().await
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

    pub fn unexpire(&mut self, item: &Result<Guid, MarkerPath<PackPath>>) -> bool {
        if let Some(handle) = self.filter_expiry.remove(item) {
            handle.abort();
            true
        } else {
            false
        }
    }
    pub fn spawn_expire_at<F>(&mut self, item: Result<Guid, MarkerPath<PackPath>>, expiry: SystemTime, duration: Option<Duration>, f: F) where
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
    pub fn expire_at(&mut self, item: Result<Guid, MarkerPath<PackPath>>, expiry: SystemTime, duration: Option<Duration>) {
        self.spawn_expire_at(item.clone(), expiry, duration, async move {
            match item {
                Ok(guid) => Some(PathingEvent::GuidReset(vec![guid])),
                Err(marker) => Some(PathingEvent::ResetMarker(marker.map_path(Into::into))),
            }
        })
    }

    const SCHEDULE_TIMEOUT: Duration = Duration::from_secs(60 * 60 * 12);
}

#[derive(Debug)]
pub struct PathingController {
    loader: Arc<PackLoader>,

    pub enabled: bool,
    pub map_pack_info: BTreeMap<PackMapPath, MapPackInfoStorage>,
    pub map_packs: BTreeMap<PackMapPath, LoadedMapPack>,
    pub filter_state: FilterState,
}

impl PathingController {
    pub fn new(loader: Arc<PackLoader>) -> Self {
        Self {
            loader,
            enabled: false,
            map_pack_info: Default::default(),
            map_packs: Default::default(),
            filter_state: Default::default(),
        }
    }

    pub fn packs() -> &'static RwLock<PackRegistry> {
        static PACKS: RwLock<PackRegistry> = RwLock::const_new(PackRegistry::new());
        &PACKS
    }

    pub async fn trail_params(&self) -> TrailParams {
        let settings = self.loader.settings.read().await;
        let pathing = settings.pathing();
        let mut params = TrailParams::DEFAULT;
        params.y_offset = pathing.space.trail_y_offset().unwrap_or(0.0);
        params.resolution = Some(pathing.space.trail_resolution());
        params.width = pathing.space.trail_width();

        params
    }

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

    pub async fn setup(&mut self, ctx: &mut PathingEventContext) {
        let settings = &self.loader.settings;
        let mut enabled = false;
        let festivals = async {
            let settings = settings.read().await;
            enabled = settings.enable_katrender;
            let (on, off) = settings.pathing().festival_preferences();
            FestivalState {
                active: festivals::FestivalFixup::current_festivals(),
                on,
                off,
            }
        };
        let achievements = async move {
            use tokio::io::AsyncReadExt;
            if let Ok(mut file) = tokio::fs::File::open(rt::addon_dir().join("achievements.json")).await {
                let mut data = Vec::new();
                file.read_to_end(&mut data).await?;
                serde_json::from_slice::<crate::settings::pathing::PathingAchievementApi>(&data)
                    .map_err(anyhow::Error::from)
                    .map(crate::settings::pathing::PathingAchievementSave::from)
                    .map(Some)
            } else {
                Ok(None)
            }
        };
        let preload = self.preload_all();
        let (_preload, festivals, achievements) = tokio::join!(preload, festivals, achievements);

        ctx.festivals.set(festivals);
        self.enabled = enabled;

        let achievements = achievements
            .context("loading achievements.json");
        if let Some(Some(achievements)) = rt::log::warn_ok(achievements) {
            self.filter_state.achievements.status = achievements.into();
        }

        ctx.pack_info.send_if_modified(|shared| {
            shared.shared_loader = Some(self.loader.clone());
            true
        });
    }

    pub const UPDATE_INTERVAL_SLOW: Duration = Duration::from_secs(10);
    pub const UPDATE_INTERVAL_RESPONSIVE: Duration = Duration::from_millis(350);
    /// Don't bother re-scanning if player hasn't moved at least `sqrt(distance)` [metres](PackSpace}
    pub const UPDATE_DISTANCE_DISTANCE: f32 = 0.005;
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

    pub async fn handle_gameplay(&mut self, ctx: &mut PathingEventContext, state: GameplayState, trans: GameplayTransition) {
        match state {
            GameplayState::Gameplay { map_id: Some(map_id) } => {
                let new_map = match trans {
                    GameplayTransition::Map { prev_map_id: Some(prev), .. } if prev != map_id => true,
                    GameplayTransition::Loaded { prev_map_id: Some(prev), .. } if prev != map_id => true,
                    _ => false,
                };
                if new_map {
                    self.handle_map_leave();
                }
                self.handle_map_enter(map_id, ctx).await
            },
            #[cfg(todo = "unnecessary")]
            _ =>
                self.handle_map_leave(),
            _ => (),
        }
    }

    async fn handle_update_tick(&mut self, ctx: &mut PathingEventContext, map_id: MapIndex) {
        // TODO: skip all processing if feature is disabled in settings
        let maps = self.map_packs.iter_mut()
            .filter(|(path, map)| path.path == map_id && !map.interactive_pois.is_empty());
        let mut playerpos = None;
        let mut nearby_changes = Vec::new();
        for (path, map) in maps {
            let Some(info) = self.map_pack_info.get(path) else { continue };
            let playerpos = playerpos.get_or_insert_with(|| {
                let prev = ctx.player_pos();
                match (PathingEventContext::read_player_pos(), prev) {
                    (Some(pos), Some(prev)) if pos.distance_squared(prev) < Self::UPDATE_DISTANCE_DISTANCE =>
                        None,
                    (Some(pos), _) => Some({
                        ctx.player_pos = pos;
                        pos
                    }),
                    _ => None,
                }
            }).clone();
            let Some(playerpos) = playerpos else { break };
            if map.interactive_pois_nearby.is_empty() {
                map.interactive_pois_nearby.resize(map.interactive_pois.len(), false);
            }

            let mut updated = Vec::new();
            let ipois = map.interactive_pois.iter()
                .zip(map.interactive_pois_nearby.iter_mut())
                .enumerate();
            for (i, (ipoi, mut nearby_bit)) in ipois {
                let prev_nearby = *nearby_bit;
                #[cfg(todo)]
                if !ipoi.is_passive() && !prev_nearby {
                    continue
                }
                let Some(lpoi) = ipoi.loaded_poi(&map.pois) else { continue };
                let nearby = ipoi.is_nearby(lpoi.position, playerpos).is_some();
                if nearby != prev_nearby {
                    *nearby_bit = nearby;
                    let interactive_path = Locator::with_path(i as PoiIndex);
                    let loaded_path = path.rel(ipoi.loaded_index().path);
                    let path =
                        info.pois().nth(loaded_path.path as usize)
                        .unwrap_or(Locator::with_path(PoiIndex::MAX));
                    updated.push(match nearby {
                        true => InteractionEvent::Nearby { path, loaded_path, interactive_path, },
                        false => InteractionEvent::Gone { path, loaded_path, interactive_path },
                    });
                }
            }
            if !updated.is_empty() {
                nearby_changes.push((path, map.interactive_pois_nearby.clone(), updated));
            }
        }
        if !nearby_changes.is_empty() {
            ctx.pack_info.send_if_modified(|shared_info| {
                for (path, nearby, events) in nearby_changes {
                    let Some(shared_map) = shared_info.map_state.get_mut(&path) else { continue };
                    shared_map.interactive_pois_nearby = nearby;
                    for e in events {
                        let _ = shared_info.interactions.send(e);
                    }
                }
                true
            });
        }
    }

    async fn handle_config_change(&mut self, ctx: &mut PathingEventContext, path: PackPath) {
        let mut dirty = false;
        for (map_path, map) in self.map_packs.iter_mut().filter(|(p, _)| p.root == path) {
            let Some(map_info) = self.map_pack_info.get(map_path) else { continue };
            let Some((info, config)) = Self::packs().read().await.packs.get(path.path as usize).and_then(|p| p.get_info()) else { continue };
            {
                let damage = map.update_category_config(&map_info, &info.categories, &config);
                if let Ok(true) = damage {
                    continue
                }
                map.refresh_categories(&map_info, &info.categories, &config, damage.err().as_ref());
            }
            dirty = true;
            ctx.pack_info.send_if_modified(|shared_info| {
                let Some(map) = shared_info.map_state.get_mut(map_path) else { return false };
                map.categories = map.categories.clone();
                false
            });
        }
        if dirty {
            ctx.pack_info.send_if_modified(|_| true);
            //ctx.filter_state_signal = true;
            PathingEvent::RequestDisabledPaths.try_send();
        }
    }

    const USED_THRESHOLD_MAP_INFO: u32 = 4;
    const USED_THRESHOLD_MAP: u32 = 3;
    pub async fn handle_map_enter(&mut self, map_id: MapIndex, ctx: &mut PathingEventContext) {
        self.map_pack_info.retain(|path, map| match path.path == map_id {
            true => {
                map.used.mark_used();
                true
            },
            false => {
                map.used.mark_unused();
                !map.used.is_elderly(Self::USED_THRESHOLD_MAP_INFO)
            },
        });
        self.map_packs.retain(|path, map| match path.path == map_id {
            true => {
                map.used.mark_used();
                true
            },
            false => {
                map.used.mark_unused();
                !map.used.is_elderly(Self::USED_THRESHOLD_MAP)
            },
        });
        self.load_maps_for(map_id, ctx).await;
    }
    async fn load_maps_for(&mut self, map_id: MapIndex, ctx: &mut PathingEventContext) {
        let loaded_pack = self.load_packs_for(map_id, ctx).await;

        let res = Controller::run_render(RenderTaskPriority::High, move |state| {
            let Some(Ok(engine)) = &mut state.engine else { return };
            engine.packs.clear_active();
        }).await.context("pre-clearing render list");
        let _ = rt::log::warn_ok(res);

        for &pack in &loaded_pack {
            self.handle_map_pack(pack, map_id, ctx).await;
        }
    }

    async fn load_packs_for(&mut self, map_id: MapIndex, ctx: &mut PathingEventContext) -> Vec<PackPath> {
        let mut packs = Self::packs().write().await;
        let mut loaded_pack = Vec::new();
        {
            let load_packs = packs.load_packs_for_map(&self.loader, map_id);
            futures::pin_mut!(load_packs);
            while let Some((path, pack)) = load_packs.next().await {
                let key = path.rel(map_id);
                if self.map_packs.contains_key(&key) && self.map_pack_info.contains_key(&key) {
                    continue
                }
                let map_pack_info = MapPackInfo::load_from_pack(pack, map_id, &self.loader).await
                    .map(MapPackInfo::get)
                    .with_context(|| format!("loading map #{map_id} for {pack}"));
                let map_pack_info = match map_pack_info {
                    Ok(None) => {
                        log::debug!("deactivating {pack}, why did we think it was relevant to {map_id}?");
                        pack.deactivate(&self.loader);
                        continue
                    },
                    Ok(Some(map_pack_info)) => map_pack_info,
                    Err(e) => {
                        ctx.pack_info.send_if_modified(|shared_info| {
                            shared_info.update_pack(path.clone(), pack);
                            false
                        });
                        log::error!("{e:#}");
                        continue
                    },
                };
                let map_pack_info = Arc::new(map_pack_info);
                // TODO: swap out for a load_from_pack here?
                let mut map_pack = LoadedMapPack::from_pack(map_id, &map_pack_info, pack);
                if let Ok(info) = &pack.info {
                    pack.with_config(|config| {
                        let _damage = map_pack.update_category_config(&map_pack_info, &info.categories, config)
                            .map_err(drop);
                        if let Ok(true) = _damage {
                            // expecting damage to be empty on a fresh load, but maybe it can skip this idk
                            return
                        }
                        map_pack.refresh_categories(&map_pack_info, &info.categories, config, None);
                    });
                }
                loaded_pack.push(path.clone());
                ctx.pack_info.send_if_modified(|shared_info| {
                    shared_info.update_pack(path.clone(), pack);
                    shared_info.map_info.insert(key.clone(), map_pack_info.clone());
                    shared_info.map_state.insert(key.clone(), SharedMapPackState::with_loaded(&map_pack));
                    // defer update until all are loaded
                    false
                });
                self.map_pack_info.insert(key.clone(), MapPackInfoStorage::new(map_pack_info));
                self.map_packs.insert(key, map_pack);
            }
        }
        if !loaded_pack.is_empty() {
            self.update_loaded_visibility();
            //ctx.filter_state_signal = true;
        }
        // now notify
        ctx.pack_info.send_if_modified(|shared_info| {
            for (path, loaded) in packs.all_packs() {
                if !loaded_pack.contains(&path) {
                    shared_info.update_pack(path, loaded);
                }
            }
            !loaded_pack.is_empty()
        });
        loaded_pack
    }

    pub async fn handle_map_pack(&mut self, path: PackPath, map_id: MapIndex, ctx: &mut PathingEventContext) {
        #[cfg(todo)]
        let key = (path, map_id);
        #[cfg(todo)]
        let Some(info) = self.map_pack_info.get(&key) else {
            log::debug!("handle_map_pack called for missing pack info {path}?");
            return
        };
        #[cfg(todo)]
        let Some(pack) = self.map_packs.get_mut(&key) else {
            log::debug!("handle_map_pack called for missing pack {path}?");
            return
        };

        self.prepare_pack(ctx, path.rel(map_id)).await
    }

    pub fn handle_map_leave(&mut self) {
        #[cfg(deleteme)] {
            self.map_pack_info.clear();
        }
        self.filter_state.hidden.reset_map_leave();
    }

    #[cfg(todo)]
    async fn handle_vis(&mut self, ctx: &mut PathingEventContext, path: CategoryPath<PackPath>, state: VisibilityFlags) {
        let packs = Self::packs().read().await;
        let Some(pack) = packs.lookup_ref(&path.root) else { return };
        let Some(config) = &pack.config else {
            log::error!("can't update {path}={}, no config state?", state.bits());
            return
        };
        let Ok(info) = &pack.pack_info.info else { return };
        let cat_vis = info.categories.visibility.get_for(path)
            .unwrap_or(VisibilityFlags::TOGGLES);
        let state_dev = cat_vis ^ state;

        config.send_if_modified(|config| {
            let path = path.unscope();
            if config.visibility_deviation_for(path) == state_dev {
                return false
            }
            Arc::make_mut(config).set_visibility_deviation(path, state_dev);
            true
        });
    }

    async fn handle_toggle(&mut self, path: CategoryPath<PackPath>, state: Option<bool>) {
        let packs = Self::packs().read().await;
        let Some(pack) = packs.lookup_ref(&path.root) else { return };
        let Some(config) = &pack.config else {
            log::error!("can't update {path}={state:?}, no config state?");
            return
        };
        let Ok(info) = &pack.pack_info.info else { return };
        let cat_vis = !info.categories.disabled.contains(path);
        let toggle_dev = state.map(|state| cat_vis ^ state);

        let mut state = state.unwrap_or(false);
        let changed = config.send_if_modified(|config| {
            let path = path.unscope();
            let prev = config.visibility_deviation_for(path);
            let mut state_dev = prev;
            state = toggle_dev.unwrap_or(!prev.contains(VisibilityFlags::TOGGLE));
            state_dev.set(VisibilityFlags::TOGGLE, state);
            if prev == state_dev {
                return false
            }
            Arc::make_mut(config).set_visibility_deviation(path, state_dev);
            true
        });

        if changed {
            let full_id = Self::packs().read().await.lookup_ref(&path.root)
                .and_then(|loaded|
                    loaded.active.as_ref()
                        .and_then(|active| active.pack.categories.all_categories.get_index(path.path as usize))
                        .map(|(_id, cat)| cat.full_id.clone()),
                );
            if let Some(full_id) = full_id {
                let mut settings = self.loader.settings.write().await;
                PathingSettings::pathing_state_update(&mut settings, full_id.to_string(), cat_vis ^ state).await;
            } else {
                log::warn!("{path} not found for toggle state update");
            }
        }
    }

    #[cfg(deleteme)]
    async fn handle_vis(&mut self, ctx: &mut PathingEventContext, path: CategoryPath<PackPath>, state: VisibilityFlags) {
        let map_path = ctx.gameplay.borrow().gameplay_map()
            .map(|map_id| path.swap(map_id));
        if let Some((map_path, map, info)) = map_path.and_then(|map_path|
            self.map_pack_info.get(&map_path).and_then(|info|
                self.map_packs.get_mut(&map_path).map(|map| (map_path, map, info))
            )
        ) {
            let mut state_dirty = false;
            if let Some(pack_info) = Self::packs().read().await
                .lookup_ref(&path.root)
                .and_then(|pack| pack.info.as_ref().ok())
            {
                if let Some(damage) = map.update_visibility(info, &pack_info.categories, path.unscope(), state) {
                    state_dirty = !damage.is_empty();
                    map.apply_category_damage(info, &pack_info.categories, &damage);
                }
            }

            if state_dirty {
                // or self.prepare_pack().await?
                PathingEvent::PreparePack(path.root).try_send();
                ctx.pack_info.send_modify(|pack_info| {
                    if let Some(shared_map) = pack_info.map_state.get_mut(&map_path) {
                        shared_map.categories = map.categories.clone();
                    }
                });
            }
        }

        // TODO: if state_dirty?
        let full_id = Self::packs().read().await.lookup_ref(&path.root)
            .and_then(|loaded|
                loaded.active.as_ref()
                    .and_then(|active| active.pack.categories.all_categories.get_index(path.path as usize))
                    .map(|(_id, cat)| cat.full_id.clone()),
            );
        if let Some(full_id) = full_id {
            let mut settings = self.settings.write().await;
            crate::settings::PathingSettings::pathing_state_update(&mut settings, full_id, state.is_visible()).await;
        } else {
            log::warn!("{path} not found for toggle state update");
        }
    }

    fn handle_guid_reset(&mut self, ctx: &mut PathingEventContext, guids: Vec<Guid>) {
        SaveState::try_write_with(|save| {
            let mut dirty = false;
            for guid in guids {
                if save.pathing().hidden_guid_expiry_get(&guid).is_some() {
                    save.pathing_mut().hidden_guid_expire(&guid);
                    dirty = true;
                }
                if self.filter_state.hidden.reset(&guid) {
                    ctx.filter_state_signal = true;
                }
                if ctx.unexpire(&Ok(guid)) {
                    ctx.filter_state_signal = true;
                }
            }
            dirty
        });
    }
    async fn handle_dismiss(&mut self, ctx: &mut PathingEventContext, path: PoiPath<PackMapPath>, delay: Option<Duration>, expiry: Option<SystemTime>, hide_contexts: Vec<HideContext>) {
        let Some(guid) = ({
            self.map_pack_info.get(&path.root)
                .and_then(|info| self.map_packs.get(&path.root)
                    .map(|map| (map, info))
                ).and_then(|(map, info)| map.poi_guids(info)
                    .find(|(p, ..)| p.path == path.path)
                    .map(|(_, guid)| guid.clone())
                )
            /*Self::packs().read().await.lookup_ref(&path.root)
                .and_then(|pack| pack.active.as_ref())
                .and_then(|active| active.pack.pois.get(path.path as usize))
                .and_then(|poi| match poi.guid {
                    guid if guid == Uuid::nil() => None,
                    guid => Some(guid),
                })*/
        }) else {
            // TODO: ctx.expire_at(Err(path.into()), expiry);
            log::warn!("no GUID on {path} to dismiss");
            return
        };
        let hidden = if let Some(expiry) = expiry {
            ctx.expire_at(Ok(guid.clone().into()), expiry, delay);
            let expiry_now = std::time::Instant::now();
            let expiry_std = expiry_now + if let Some(delay) = delay {
                delay
            } else {
                log::warn!("TODO: expiry to instant");
                Duration::from_secs(2)
            };
            self.filter_state.hidden.expire_at(guid.clone(), expiry_std)
        } else {
            self.filter_state.hidden.marker_mut(guid.clone())
        };
        if !hide_contexts.iter().all(|hide| match hide {
            HideContext::Local(map) if map.shard.is_none() =>
                false,
            _ => true,
        }) && matches!(&hidden.reset, AutoReset::Never) {
            hidden.reset = AutoReset::MapChange;
        }
        let has_context = !hide_contexts.is_empty();
        if has_context {
            hidden.contexts.extend(hide_contexts);
        }
        let expiry = match (expiry, delay) {
            (Some(e), ..) => Some(Some(e)),
            (None, Some(delay)) =>
                SystemTime::now().checked_add(delay).map(Some),
            (None, None) if has_context =>
                Some(None),
            (None, None) =>
                SystemTime::now().checked_add(Duration::MAX).map(Some),
        }.unwrap_or_else(|| {
            log::error!("when is the future?");
            Some(SystemTime::now() + Duration::from_secs(3600 * 24 * 365 * 2))
        });
        if let Some(expiry) = expiry {
            SaveState::write_with(|save| {
                save.pathing_mut().hidden_guid_expire_at(guid.into(), expiry)
            });
        }
        ctx.filter_state_signal = true;
    }

    pub(crate) async fn reload_all(&mut self, ctx: &mut PathingEventContext) {
        self.unload_all(ctx, false).await;
        self.load_all(ctx).await;
    }

    async fn load_all(&mut self, ctx: &mut PathingEventContext) {
        self.preload_all().await;
        if let Some(map_id) = ctx.gameplay_map() {
            self.load_maps_for(map_id, ctx).await
        }
        //tokio::spawn(Self::load_all_inner(self.loader.clone()));
    }

    #[cfg(todo)]
    async fn load_all_inner(loader: Arc<PackLoader>) {
        let mut loaders = JoinSet::new();
        {
            let mut packs = Self::packs().write().await;
            for (path, pack) in packs.all_packs_mut() {
                match pack.activate_start() {
                    Err(e) => {
                        log::error!("{e:#}");
                    },
                    Ok(Some(activate)) => {
                        let loader = loader.clone();
                        let pack_path = pack.path.clone();
                        loaders.spawn(async move {
                            (path, LoadedPack::activate_load(activate, pack_path, &loader).await)
                        });
                    },
                    Ok(None) => (),
                }
            }
        }
        while let Some(res) = loaders.join_next().await {
            let res = res.context("Pack load panicked");
            let res = match res {
                Ok((path, res)) => {
                    let mut packs = Self::packs().write().await;
                    if let Some(loaded) = packs.lookup_mut(&path) {
                        loaded.activate_finish(res).map(|()| path)
                    } else {
                        Err(anyhow!("pack {path} disappeared???"))
                    }
                },
                Err(e) => Err(e),
            };
            match res {
                Ok(path) => {
                    Self::try_send(PathingEvent::PreparePack(path))
                },
                Err(e) =>
                    log::error!("{e:#}"),
            }
        }
    }

    async fn preload_all(&self) {
        let _ = create_dir_all(SourceKind::Pathing.get_user_dir()).await;

        let found_packs = {
            let mut found_packs = Vec::new();
            let dir = Settings::read_source_dir(self.loader.settings.clone(), SourceKind::Pathing).await;
            futures::pin_mut!(dir);
            while let Some(entry) = dir.next().await {
                let (path, source) = match entry {
                    Ok(e) => e,
                    Err(e) => {
                        log::error!("Failed to list pathing files: {e}");
                        continue
                    },
                };
                let datasource = source.map(Locator::with_path);
                found_packs.push((path, datasource));
            }
            found_packs
        };
        let mut packs = Self::packs().write().await;
        for (path, datasource) in found_packs {
            packs.preload(path, datasource, &self.loader);
        }
    }

    #[cfg(deleteme)]
    async fn load_all_inner(settings: SettingsLock) -> anyhow::Result<()> {
        let _ = create_dir_all(SourceKind::Pathing.get_user_dir()).await;

        let mut path_loads = tokio::task::JoinSet::new();

        log::info!("Pre-loading all paths...");
        let dir = Settings::read_source_dir(settings, SourceKind::Pathing).await;
        futures::pin_mut!(dir);
        while let Some(entry) = dir.next().await {
            let (path, datasource) = match entry {
                Ok(e) => e,
                Err(e) => {
                    log::error!("Failed to list pathing files: {e}");
                    continue
                },
            };
            // TODO: name could be source? what do we actually use that for, and is it meant to be user-facing or a unique id?
            let name = path
                .file_name()
                .unwrap_or(path.as_ref())
                .to_string_lossy()
                .into_owned();
            let context = format!("Loading pathing pack {name}");
            log::debug!("{context}...");
            let is_taco = path
                .extension()
                .map(|e| e.eq_ignore_ascii_case("taco") || e.eq_ignore_ascii_case("zip"));
            let is_taco = path.is_file() || is_taco.unwrap_or(false);
            let loader = move || {
                match is_taco {
                    true => Self::pathing_load_taco(path),
                    false => Self::pathing_load_dir(path),
                }
                .context(context)
            };
            let loader = async move {
                let res = tokio::task::spawn_blocking(loader)
                    .await
                    .context("Path load panicked");
                match res {
                    Ok(Ok((pack, loader))) => {
                        Self::pathing_load_pack(pack, loader, name).await;
                        Ok(())
                    },
                    Err(e) | Ok(Err(e)) => {
                        Self::pathing_notify_pack_error(
                            name,
                            UnloadedReason::LoadingFailed(anyhow!("{e:#}")),
                        )
                        .await;
                        Err(e)
                    },
                }
            };
            path_loads.spawn(loader);
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
                                Self::try_send(PathingEvent::RequestDisabledPaths);
                                disabled_paths_dirty = false;
                            },
                        }
                    }
                } else {
                    pack_load.await
                }
                .map(|r| r.context("Path load panicked"));
                match res {
                    None => break,
                    Some(Err(e) | Ok(Err(e))) => log::error!("{e:#}"),
                    Some(Ok(Ok(()))) => disabled_paths_dirty = true,
                }
            }

            // TODO: sender+await, or ideally just make this unnecessary

            if disabled_paths_dirty {
                Self::try_send(PathingEvent::RequestDisabledPaths);
            }
        });

        Ok(())
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
            self.handle_map_leave();
            self.unload_all(ctx, false).await;
        }
    }

    #[cfg(deleteme)]
    fn pathing_load_taco(path: PathBuf) -> anyhow::Result<(Pack, LoaderBox)> {
        use taimi_pack::loader::ZipLoader;
        let mut loader = ZipLoader::new(&path)?;
        let pack = Pack::load(&mut loader)?;
        Ok((pack, Box::new(loader)))
    }

    #[cfg(deleteme)]
    fn pathing_load_dir(path: PathBuf) -> anyhow::Result<(Pack, LoaderBox)> {
        use taimi_pack::loader::DirectoryLoader;
        let mut loader = DirectoryLoader::new(path);
        let pack = Pack::load(&mut loader)?;
        Ok((pack, Box::new(loader)))
    }

    #[cfg(deleteme)]
    async fn pathing_load_pack(mut pack: Pack, loader: LoaderBox, name: String) {
        let context = format!("Loading pack {name} onto engine");
        if pack.name.is_empty() {
            pack.name = name;
        }
        let res = Controller::run_render(RenderTaskPriority::High, move |state| {
            let engine = match &mut state.engine {
                Some(res) => res.as_mut().map_err(|e| anyhow!("{e:#}")),
                None => return Ok(()),
            }?;
            let pack = Arc::new(pack);
            let pack_idx = engine.packs.add_pack(pack, loader);
            engine.packs.load_pack(&engine.render_backend.device, pack_idx)
        })
        .await;
        let res = res
            .map(|res| res.context(context))
            .context("Submitting pack to engine");
        if let Err(e) | Ok(Err(e)) = res {
            log::error!("{e:#}");
        }
    }
    #[cfg(deleteme)]
    async fn pathing_notify_pack_error(name: String, reason: UnloadedReason) {
        let _ = Controller::run_render(RenderTaskPriority::Normal, move |state| {
            let engine = match &mut state.engine {
                Some(Ok(e)) => e,
                _ => return,
            };
            engine.packs.load_failed(name, reason);
        })
        .await;
    }

    async fn unload_all(&mut self, ctx: &mut PathingEventContext, remove: bool) {
        log::info!("Unloading all paths...");
        if let Err(e) = Self::unload_all_inner(self.loader.clone(), remove).await {
            log::error!("{e:#}");
        }
        self.map_packs.clear();
        if remove {
            self.map_pack_info.clear();
        }
        {
            let remove_packs = match remove {
                false => None,
                true => Some(Self::packs().read().await),
            };
            ctx.pack_info.send_modify(|pack_info| {
                pack_info.map_info.clear();
                pack_info.map_state.clear();
                if let Some(packs) = remove_packs {
                    for (path, pack) in packs.all_packs() {
                        pack_info.update_pack(path, pack);
                    }
                } else {
                    pack_info.pack_info.clear();
                }
            });
        }
    }

    async fn load_pack(&mut self, path: PackPath, ctx: &mut PathingEventContext) -> anyhow::Result<()> {
        let mut packs = Self::packs().write().await;
        let pack = packs.lookup_mut(&path).ok_or_else(|| anyhow!("pack {path} does not exist"))?;
        let res = pack.activate(&self.loader).await;
        if let Ok(Some(())) | Err(..) = &res {
            let pack = &*pack;
            let is_loaded = matches!(&res, Ok(Some(())));
            ctx.pack_info.send_if_modified(|shared_info| {
                shared_info.update_pack(path.clone(), pack);
                is_loaded
            });
        }
        if let Ok(Some(())) = &res {
            if let Some(..) = ctx.gameplay_map() {
                PathingEvent::PreparePack(path).try_send();
            }
        }
        res.map(drop)
    }

    #[cfg(deleteme)]
    async fn prepare_pack(&mut self, path: PackPath, ctx: &mut PathingEventContext) -> anyhow::Result<()> {
        let gameplay = ctx.gameplay.watch.receiver().clone();
        let setup = {
            let Some(map_id) = ctx.gameplay_map() else {
                log::warn!("no active map to prepare pack {path} for");
                return Ok(())
            };
            let trail_params = self.trail_params().await;
            let key = path.rel(map_id);
            let Some(map_pack_info) = self.map_pack_info.get(&key) else {
                anyhow::bail!("map pack data for {path} on {map_id} not loaded?");
            };
            let Some(map_pack) = self.map_packs.get_mut(&key) else {
                anyhow::bail!("map pack data for {path} on {map_id} not loaded?");
            };
            let packs = Self::packs().read().await;
            let Some(pack) = packs.lookup_ref(&path) else {
                anyhow::bail!("pack {path} disappeared???")
            };
            let Some(active) = &pack.active else {
                anyhow::bail!("can't prepare pack {path} if it's not loaded")
            };

            let mut pois = Vec::with_capacity(map_pack_info.poi_count());
            for (poi_path, poi) in map_pack.pois(map_pack_info) {
                let pack_poi = active.pack.pois.get(poi_path.path as usize)
                    .and_then(|poi| poi.icon_name().map(|icon| (poi, icon)));
                let setup = pack_poi.map(|(poi, icon)| SpacePoiBuilder {
                    icon_file: icon.into(),
                    scale: poi.attributes.icon_size.map(Into::into).unwrap_or_default(),
                    scale_map: poi.attributes.map_display_size.map(Into::into).unwrap_or_default(),
                    tint: poi.attributes.tint.map(Into::into).unwrap_or_default(),
                    opacity: poi.attributes.alpha.map(Into::into).unwrap_or_default(),
                });
                let is_copy = pack_poi.and_then(|(poi, _)| poi.attributes.copy_value.as_ref()).is_some();
                pois.push((poi_path, poi.clone(), setup, is_copy));
            }
            pois.shrink_to_fit();

            let mut trails = Vec::with_capacity(map_pack_info.trail_count());
            for (trail_path, trail) in map_pack.trails_mut(map_pack_info) {
                let Some(texture_name) = active.pack.trails.get(trail_path.path as usize).and_then(|pack_trail|
                    pack_trail.texture_name().map(String::from)
                ) else {
                    log::info!("trail#{trail_path} missing texture");
                    trails.push((trail_path, trail.clone(), None));
                    continue;
                };
                let trail_data = match active.load_trail_data(trail_path.path).await {
                    Ok(trail_data) => trail_data,
                    Err(e) => {
                        log::error!("{e:#}");
                        trails.push((trail_path, trail.clone(), None));
                        continue
                    },
                };
                trail.populate_data(&trail_data);
                // TODO: spawn_blocking all this and parallelize, also dispatch to render thread incrementally as data comes in
                let geometry = trail.vertices_for(&trail_data, &trail_params);
                let setup = SpaceTrailBuilder {
                    geometry,
                    texture_file: texture_name.into(),
                };
                trails.push((trail_path, trail.clone(), Some(setup)));
            }
            trails.shrink_to_fit();

            (map_id, pois, trails)
        };
        let res = Controller::run_render(RenderTaskPriority::Normal, move |state| -> anyhow::Result<()> {
            let Some(Ok(engine)) = &mut state.engine else { return Ok(()) };
            let (setup_map_id, setup_pois, setup_trails) = setup;
            let map_id = {
                let map = gameplay.borrow().gameplay_map();
                drop(gameplay);
                map
            };
            match map_id {
                Some(map_id) if map_id == setup_map_id => (),
                map_id => {
                    log::warn!("prepared pack {path} for map#{setup_map_id}, but now {map_id:?}");
                    return Ok(())
                },
            }
            let packs = Self::packs().blocking_read();
            let Some(pack) = packs.lookup_ref(&path) else {
                anyhow::bail!("pack {path} disappeared???")
            };
            let Some(active) = &pack.active else {
                anyhow::bail!("can't prepare pack {path} if it's not loaded")
            };
            // TODO: sanity check that we still want to load this?
            let spacepack = engine.packs.pack_mut(&path);
            #[cfg(deleteme)]
            if spacepack.enabled_categories.is_empty() {
                spacepack.init_enabled_categories(
                    active.pack.categories.all_categories.values().map(|c| c.default_toggle)
                );
            }
            let needs_rebuild = spacepack.render_list_bookmark.is_some();
            spacepack.clear();

            let mut loader = active.loader.blocking_lock();
            let mut loader = SpaceLoader {
                active_pack: spacepack,
                loader: &mut *loader,
                device: &engine.render_backend.device,
            };
            let mut copyable_pois = BTreeSet::new();
            let mut copyable_categories = BTreeSet::new();
            // XXX: this whole PackCollection type needs a rework, so collect to vec for now...
            let pois = setup_pois.into_iter().map(|(path, poi, setup, is_copy)| {
                if is_copy {
                    copyable_pois.insert(path.path);
                    copyable_categories.insert(poi.category);
                }
                let poi = setup.map(|setup| setup.build(path, &mut loader, &poi));
                match poi {
                    Some(Ok(poi)) => poi,
                    Some(Err(e)) => {
                        log::warn!("Preparing PoI#{path}: {e:#}");
                        SpacePoiBuilder::build_empty()
                    },
                    None =>
                        SpacePoiBuilder::build_empty(),
                }
            }).collect::<Vec<_>>();
            let trails = setup_trails.into_iter().map(|(path, trail, setup)| {
                let trail = setup.map(|setup| setup.build(path, &mut loader, &trail));
                if let Some(Err(e)) = &trail {
                    log::warn!("Preparing trail#{path}: {e:#}");
                }
                match trail {
                    Some(Ok(trail)) => trail,
                    _ =>
                        SpaceTrailBuilder::build_empty(),
                }
            }).collect::<Vec<_>>();
            #[cfg(deleteme)] {
            spacepack.copyable_pois = copyable_pois;
            spacepack.copyable_categories = copyable_categories;
            }
            if needs_rebuild {
                spacepack.active_pois = pois;
                spacepack.active_trails = trails;
                engine.packs.rebuild_active(&engine.render_backend.device)
            } else {
                engine.packs.load_pack(&engine.render_backend.device, path.path, pois, trails)
            }
        })
        .await;

        match res.map_err(anyhow::Error::from) {
            Err(e) | Ok(Err(e)) => Err(e),
            Ok(Ok(())) => Ok(())
        }.context("Preparing pack for render")
    }

    pub async fn setup_pack(&mut self,
        ctx: &mut PathingEventContext,
        path: PackMapPath,
        setup_trails: Vec<SetupTrail>,
        setup_pois: Option<Vec<SetupPoi>>,
    ) -> anyhow::Result<()> {
        let setup_map_id = path.path;
        let setup = move |state: &mut RenderState| -> anyhow::Result<()> {
            let Some(Ok(engine)) = &mut state.engine else { return Ok(()) };
            let map_id = {
                log::debug!("TODO: check map_id before render setup");
                Some(setup_map_id)
            };
            match map_id {
                None => (),
                Some(map_id) if map_id == setup_map_id => (),
                map_id => {
                    log::info!("prepared pack {path} for map#{setup_map_id}, but now {map_id:?}");
                    return Ok(())
                },
            }
            // TODO: sanity check that we still want to load this?
            let spacepack = engine.packs.pack_mut(&path.root);
            let needs_rebuild = spacepack.render_list_bookmark.is_some();
            spacepack.clear();

            let loader = {
                let packs = Self::packs().blocking_read();
                let Some(pack) = packs.lookup_ref(&path.root) else {
                    anyhow::bail!("pack {path} disappeared???")
                };
                let Some(active) = &pack.active else {
                    anyhow::bail!("can't prepare pack {path} if it's not loaded")
                };
                active.loader.clone()
            };
            let mut loader = loader.blocking_lock();
            let mut loader = SpaceLoader {
                active_pack: spacepack,
                loader: &mut *loader,
                device: &engine.render_backend.device,
            };
            // XXX: this whole PackCollection type needs a rework, so collect to vec for now...
            let pois = setup_pois.map(|p| p.into_iter().map(|(path, setup, poi)| {
                let poi = setup.map(|setup| setup.build(path, &mut loader, &poi));
                match poi {
                    Some(Ok(poi)) => poi,
                    Some(Err(e)) => {
                        log::warn!("Preparing PoI#{path}: {e:#}");
                        SpacePoiBuilder::build_empty()
                    },
                    None =>
                        SpacePoiBuilder::build_empty(),
                }
            }).collect::<Vec<_>>());
            let trails = setup_trails.into_iter().map(|(path, setup, trail)| {
                let trail = setup.map(|setup| setup.build(path, &mut loader, &trail));
                if let Some(Err(e)) = &trail {
                    log::warn!("Preparing trail#{path}: {e:#}");
                }
                match trail {
                    Some(Ok(trail)) => trail,
                    _ =>
                        SpaceTrailBuilder::build_empty(),
                }
            }).collect::<Vec<_>>();
            if needs_rebuild {
                if let Some(pois) = pois {
                    spacepack.active_pois = pois;
                }
                spacepack.active_trails = trails;
                engine.packs.rebuild_active(&engine.render_backend.device)
            } else {
                engine.packs.load_pack(&engine.render_backend.device, path.root.path, pois.unwrap_or_default(), trails)
            }
        };
        ctx.spawn_render(RenderTaskPriority::Normal, move |state| {
            let res = setup(state);
            rt::log::error_ok(res);
        });
        Ok(())
    }

    pub async fn prepare_pack(&mut self, ctx: &mut PathingEventContext, path: PackMapPath) {
        let Some(info) = self.map_pack_info.get(&path) else { return };
        let Some(map) = self.map_packs.get(&path) else { return };
        let Some(active) = Self::packs().read().await.lookup_ref(&path.root).and_then(|p| p.active.clone()) else { return };

        let pois = map.pois(info).map(move |(poi_path, poi)| {
            let setup = active.pack.pois.get(poi_path.path as usize)
                .and_then(SpacePoiBuilder::from_pack);
            (poi_path, setup, poi.clone())
        });

        let pois = pois.collect();
        self.prepare_trails(ctx, path, Some(pois)).await
    }

    pub async fn prepare_trails(&mut self, ctx: &mut PathingEventContext, path: PackMapPath, pois: Option<Vec<SetupPoi>>) {
        let Some(info) = self.map_pack_info.get(&path) else { return };
        let Some(map) = self.map_packs.get(&path) else { return };
        let Some(active) = Self::packs().read().await.lookup_ref(&path.root).and_then(|p| p.active.clone()) else { return };
        let params = self.trail_params().await;

        let trails = map.trails(info).map(move |(trail_path, trail)|
            SpaceTrailBuilder::load_from_pack(trail_path, active.clone(), trail.clone(), params.clone())
                .map(move |(setup, trail, updated)|
                    (trail_path, setup, trail, updated)
                )
        );

        Self::prepare_trails_spawn(ctx, path, trails, pois)
    }

    const LOAD_TRAIL_PARALLEL: usize = 12;
    pub fn prepare_trails_spawn<F, T>(ctx: &mut PathingEventContext, path: PackMapPath, trails: T, pois: Option<Vec<SetupPoi>>) where
        F: Future<Output = (TrailPath, anyhow::Result<SpaceTrailBuilder>, LoadedTrail, bool)> + Send + 'static,
        T: IntoIterator<Item = F>,
    {
        let trails: Vec<_> = trails.into_iter().collect();
        ctx.tasks.spawn(async move {
            let trails = stream::iter(trails).buffered(Self::LOAD_TRAIL_PARALLEL);
            tokio::pin!(trails);
            let mut out = Vec::new();
            let mut map_updates = Vec::new();
            while let Some((trail_path, setup, trail, changed)) = trails.next().await {
                if changed {
                    map_updates.push((trail_path, trail.clone()));
                }
                let setup = rt::log::warn_ok(setup);
                out.push((trail_path, setup, trail));
            }
            let map_updates = (!map_updates.is_empty()).then_some(PathingEvent::UpdateMapTrails {
                path,
                updates: map_updates,
            });
            // TODO: actually submit these to renderer incrementally, don't wait for full vec!
            let trails = (!out.is_empty() || pois.is_some()).then_some(PathingEvent::SetupTrails {
                path,
                trails: out,
                pois,
            });
            Some(PathingEvent::FanOut(
                map_updates.into_iter().chain(trails)
                .collect()
            ))
        });
    }

    async fn unload_all_inner(loader: Arc<PackLoader>, remove: bool) -> anyhow::Result<()> {
        let context = "Unloading packs from engine";
        let res = Controller::run_render(RenderTaskPriority::High, move |state| -> anyhow::Result<()> {
            let engine = match &mut state.engine {
                Some(res) => res.as_mut().map_err(|e| anyhow!("{e:#}")),
                None => return Ok(()),
            }?;
            engine.packs.clear();
            Ok(())
        })
        .await;
        {
            let mut packs = Self::packs().write().await;
            for pack in &mut packs.packs {
                pack.deactivate(&loader);
                if !remove {
                    pack.mark_reload(&loader);
                }
            }
            if remove {
                packs.packs.clear();
            }
        }
        match res.map(|res| res.context(context)).context(context) {
            Err(e) => Err(e),
            Ok(res) => res,
        }
    }

    #[cfg(todo)]
    async fn update_visibility_with(&mut self, disabled_paths: &HashSet<String>, active_festivals: Festivals) {
        let packs = Self::packs().read().await;
        let mut disabled_categories = BTreeMap::<_, bool>::new();
        for (path, map_pack) in &mut self.map_packs {
            let Some(pack) = packs.lookup_ref(&path.root) else { continue };
            let Some(active) = &pack.active else { continue };

            let pois = map_pack.pois.iter_mut()
                .map(|poi| (poi.category, &mut poi.visibility));
            let trails = map_pack.trails.iter_mut()
                .map(|trail| (trail.category, &mut trail.visibility));
            for (category_index, visibility) in pois.chain(trails) {
                *visibility = visibility.restore_default_toggles();
                if !visibility.is_visible() {
                    continue
                }
                let key = (path, category_index);
                let disabled = disabled_categories.get(&key).copied();
                let disabled = match disabled {
                    Some(d) => d,
                    None => {
                        let Some((full_id, category)) = active.pack.categories.all_categories.get_index(category_index as usize) else { continue };
                        let mut disabled = disabled_paths.contains(full_id);
                        if !disabled {
                            for path in disabled_paths.iter() {
                                if full_id.starts_with(path) {
                                    disabled = true;
                                    break
                                }
                            }
                        }
                        if !disabled {
                            let festivals: Festivals = category.marker_attributes.festivals.as_ref()
                                .into_iter().flatten().copied().collect();
                            if !festivals.is_empty() && !festivals.intersects(active_festivals) {
                                disabled = true;
                            }
                        }
                        disabled_categories.insert(key, disabled);
                        disabled
                    },
                };
                if disabled {
                    visibility.remove(VisibilityFlags::TOGGLE);
                }
            }
        }
    }
    pub fn update_loaded_visibility(&mut self) -> bool {
        let hidden_guids = SaveState::read_with(|s| s.pathing_state.as_ref().map(|p| p.hidden_guid_expiry.clone()));
        if let Some(hidden_guids) = hidden_guids {
            let now = SystemTime::now();
            let now_mono = std::time::Instant::now();
            let all_guids = self.map_packs.values()
                .flat_map(|map| map.poi_guids.iter().chain(map.trail_guids.iter()));

            for guid in all_guids {
                if self.filter_state.hidden.hidden.contains_key(guid.as_ref()) {
                    continue
                }
                let Some(&expiry_timestamp) = hidden_guids.get(guid) else { continue };
                self.filter_state.hidden.expire_at_timestamp(guid.clone(), expiry_timestamp, &now, &now_mono);
            }
            self.filter_state.hidden.reset_expired(&now_mono);
        }
        let filter_state = &self.filter_state;
        let mut dirty = false;
        for (path, map_pack) in &mut self.map_packs {
            let Some(map_info) = self.map_pack_info.get(path) else { continue };

            let map_filters = &map_pack.filters;
            let mut poi_filters = map_filters.pois.iter().peekable();
            let mut trail_filters = map_filters.trails.iter().peekable();

            let mut poi_guids = map_pack.poi_guids.iter();
            let pois = map_info.pois().zip(
                map_pack.pois.iter_mut()
                .zip(map_info.poi_guid_mask().map(|guid| guid.then(|| poi_guids.next()).flatten()))
            );
            let pois = pois
                .map(|(poi_path, (poi, guid))| {
                    let filters = loop {
                        match poi_filters.peek() {
                            Some((fp, ..)) if fp.path < poi_path.path => (),
                            Some((fp, ..)) if fp.path == poi_path.path => break poi_filters.next(),
                            _ => break None,
                        };
                    }.map(|(_p, f)| f);
                    let marker_path = MarkerPath::with_parts(path, MarkerIndex::from(poi_path));
                    (marker_path, poi.category, &mut poi.visibility, filters, guid)
                });
            let mut trail_guids = map_pack.trail_guids.iter();
            let trails = map_info.trails().zip(
                map_pack.trails.iter_mut()
                .zip(map_info.trail_guid_mask().map(|guid| guid.then(|| trail_guids.next()).flatten()))
            );
            let trails = trails
                .map(|(trail_path, (trail, guid))| {
                    let filters = loop {
                        match trail_filters.peek() {
                            Some((fp, ..)) if fp.path < trail_path.path => (),
                            Some((fp, ..)) if fp.path == trail_path.path => break trail_filters.next(),
                            _ => break None,
                        };
                    }.map(|(_p, f)| f);
                    let marker_path = MarkerPath::with_parts(path, MarkerIndex::from(trail_path));
                    (marker_path, trail.category, &mut trail.visibility, filters, guid)
                });
            for (marker_path, category_index, visibility, filter, guid) in pois.chain(trails) {
                let prev = *visibility & VisibilityFlags::TOGGLES;
                *visibility = visibility.restore_default_toggles();
                let cat_vis = map_info.category_index(CategoryPath::with_path(category_index))
                    .and_then(|i| map_pack.categories.get(i as usize))
                    .map(|cat| cat.visibility);
                if let Some(cat_vis) = cat_vis {
                    // TODO: if cat vis is override, set directly or something!
                    visibility.set_toggles((cat_vis & VisibilityFlags::TOGGLE) | (*visibility & !VisibilityFlags::TOGGLE));
                    visibility.set(VisibilityFlags::TOGGLE, cat_vis.contains(VisibilityFlags::TOGGLE));
                }
                if visibility.is_visible() {
                    if let Some(filter) = &filter {
                        if let filter::FILTER_HIDDEN = filter.is_visible(filter_state) {
                            visibility.remove(VisibilityFlags::TOGGLE);
                        }
                    }
                }
                if visibility.is_visible() {
                    let marker_path: MarkerPath = MarkerPath::with_path(marker_path.path);
                    if let Some(hidden) = guid.and_then(|guid| map_filters.group_filter_for(marker_path, guid)) {
                        if let filter::FILTER_HIDDEN = hidden.is_visible(filter_state) {
                            visibility.remove(VisibilityFlags::TOGGLE);
                        }
                    }
                }
                if *visibility & VisibilityFlags::TOGGLES != prev {
                    dirty = true;
                }
            }
        }
        dirty
    }

    fn visibility_update(&self, map_id: MapIndex) -> impl Iterator<Item = (PackPath, Box<[VisibilityFlags]>, Box<[VisibilityFlags]>)> + '_ {
        self.map_packs.iter()
            .filter(move |(path, _)| path.path == map_id)
            .map(|(path, map_pack)| {
                let pois: Box<[VisibilityFlags]> = map_pack.pois.iter().map(|poi| poi.visibility).collect();
                let trails: Box<[VisibilityFlags]> = map_pack.trails.iter().map(|trail| trail.visibility).collect();
                (path.root.clone(), pois, trails)
            })
    }

    async fn visibility_send(&mut self, map_id: MapIndex) {
        let update: Vec<_> = self.visibility_update(map_id).collect();

        let res = Controller::run_render(RenderTaskPriority::High, move |state| -> anyhow::Result<()> {
            let engine = match &mut state.engine {
                Some(res) => res.as_mut().map_err(|e| anyhow!("{e:#}")),
                None => return Ok(()),
            }?;
            for (path, pois, trails) in update {
                let Some(pack) = engine.packs.loaded_packs.get_mut(path.path as usize) else {
                    continue
                };
                for (active, visibility) in pack.active_pois.iter_mut().zip(pois) {
                    active.visibility = visibility;
                }
                for (active, visibility) in pack.active_trails.iter_mut().zip(trails) {
                    active.visibility = visibility;
                }
            }
            Ok(())
        }).await.context("updating render visibility");
        if let Err(e) | Ok(Err(e)) = res {
            log::error!("{e:#}");
        }
    }

    pub fn update_filter_state(&mut self, ctx: &mut PathingEventContext) {
        #[cfg(todo = "unnecessary")]
        {
            self.filter_state.festival = ctx.festivals.read().clone();
        }
        self.filter_state.achievements.update_from_save();
        if let Ok(ml) = rt::mumble_link_ptr() {
            self.filter_state.map.update_from_mumblelink_context(&ml);
            self.filter_state.avatar.update_from_mumblelink_context(&ml);
            // TODO: self.filter_state.character.update_from_mumblelink(ml);
        }
        self.update_filter_state_schedule(ctx);
    }
    pub fn update_filter_state_schedule(&mut self, ctx: &mut PathingEventContext) {
        #[cfg(feature = "paths-schedule")]
        let next_scheduled = {
            self.filter_state.schedule.update_time();
            let mut next_scheduled = None;
            if let Some(now) = &self.filter_state.schedule.now {
                if let Some(map_id) = ctx.gameplay_map() {
                    let next_update = self.map_packs.iter_mut()
                        .filter(|(path, _)| path.path == map_id)
                        .filter_map(|(_, map)| {
                            map.filters.next_schedule_event(&now)
                        })
                        .min();
                    next_scheduled = next_update.and_then(|next|
                        next.signed_duration_since(now).to_std().ok()
                    );
                }
            }
        };
        let next_expire = self.filter_state.hidden.next_expiry()
            .and_then(|expiry| expiry.checked_duration_since(std::time::Instant::now()));
        let next = [
            #[cfg(feature = "paths-schedule")]
            next_schedule,
            next_expire,
        ].into_iter().flatten().min();
        let next = next.or(if ctx.next_schedule.is_elapsed() {
            Some(PathingEventContext::SCHEDULE_TIMEOUT)
        } else {
            None
        });
        if let Some(next) = next {
            ctx.next_schedule.as_mut().reset(Instant::now() + next);
        }
    }

    fn mark_map_state_dirty(&self, ctx: &mut PathingEventContext, map_id: MapIndex) {
        ctx.pack_info.send_if_modified(|pack_info| {
            pack_info.map_state.retain(|path, _| path.path == map_id);
            for (path, map_pack) in self.map_packs.iter().filter(|(path, _)| path.path == map_id) {
                // TODO: skip update if unchanged etc
                match pack_info.map_state.entry(path.clone()) {
                    btree_map::Entry::Occupied(mut e) => {
                        e.get_mut().update_static(map_pack);
                    },
                    btree_map::Entry::Vacant(e) => {
                        e.insert(SharedMapPackState::with_loaded(map_pack));
                    },
                }
            }
            true
        });
    }

    #[cfg(deleteme)]
    async fn provide_disabled_paths(&self, active_festivals: Festivals) {
        let settings_lock = self.settings.read().await;
        let disabled_paths = settings_lock.disabled_paths.clone();
        drop(settings_lock);

        let context = "Providing disabled paths to engine";
        let res = Controller::run_render(RenderTaskPriority::Normal, move |state| -> anyhow::Result<()> {
            let engine = match &mut state.engine {
                Some(res) => res.as_mut().map_err(|e| anyhow!("{e:#}")),
                None => return Ok(()),
            }?;
            for (path, _loaded, active) in Self::packs().blocking_read().active_packs() {
                let Some(pack) = engine.packs.loaded_packs.get_mut(path.path as usize) else {
                    // TODO: disable all? when does this happen?
                    continue
                };
                pack.disable_paths(&active.pack, &disabled_paths, active_festivals);
            }
            Ok(())
        })
        .await;
        let res = res.map(|res| res.context(context)).context(context);
        if let Err(e) | Ok(Err(e)) = res {
            log::error!("{e:#}");
        }
    }

    pub(crate) async fn handle_event(&mut self, event: PathingEvent, ctx: &mut PathingEventContext) -> anyhow::Result<Option<Interruption>> {
        use PathingEvent::*;
        match event {
            Exit(reason) => return Ok(Some(reason)),
            ReloadAll => self.reload_all(ctx).await,
            LoadAll => self.load_all(ctx).await,
            UnloadAll => self.unload_all(ctx, true).await,
            LoadPack(path) =>
                return self.load_pack(path, ctx).await.map(|()| None),
            #[cfg(deleteme)]
            PreparePack(path) =>
                return self.prepare_pack(path, ctx).await.map(|()| None),
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
                self.filter_state.hidden.reset(state::MarkerId::from(path));
                ctx.unexpire(&Err(path));
            },
            DismissMarker(path, delay, contexts) => {
                if let Some(expiry) = delay.map(|delay| SystemTime::now().checked_add(delay)) {
                    self.handle_dismiss(ctx, path, delay, expiry, contexts).await;
                } else {
                    log::error!("unable to determine expiry time for {path} of {delay:?}");
                }
            },
            ToggleKatRender => self.toggle_katrender(ctx).await,
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

    async fn handle_interaction(&mut self, ctx: &mut PathingEventContext, event: InteractionEvent) {
        let (path, loaded_path, ipoi, action) = match event {
            InteractionEvent::Nearby { path, loaded_path, interactive_path } => {
                let Some(map) = self.map_packs.get(&loaded_path.root) else { return };
                let Some(ipoi) = map.interactive_pois.get(interactive_path.path as usize) else { return };
                let auto_trigger_configured = || {
                    log::debug!("TODO: auto-trigger setting");
                    true
                };
                let action = if ipoi.trigger.auto && auto_trigger_configured() {
                    InteractionEventAction::AutoTrigger
                } else {
                    return
                };
                (path, loaded_path, ipoi, action)
            },
            InteractionEvent::Gone { .. } => {
                // remove on-screen info maybe?
                return
            },
            InteractionEvent::Interact { action, path, loaded_path, interactive_path } => {
                let Some(map) = self.map_packs.get(&loaded_path.root) else { return };
                let Some(ipoi) = map.interactive_pois.get(interactive_path.path as usize) else { return };
                (path, loaded_path, ipoi, action)
            },
        };

        let allowed = {
            let settings = self.loader.settings.read().await;
            let pathing = settings.pathing();
            match action {
                InteractionEventAction::Trigger => TriggerKind::all(),
                InteractionEventAction::Manual(mask) => mask,
                InteractionEventAction::Interact => pathing.trigger_allow_interact,
                InteractionEventAction::AutoTrigger => pathing.trigger_allow_auto,
            }
        };

        let blocked = "trigger settings blocked";
        if let InteractivePoi { info: Some(info), .. } = ipoi {
            if allowed.contains(TriggerKind::INFO) {
                ctx.spawn_alert(info.message.clone()[..].into(), Duration::from_secs(10));
            } else {
                log::info!("{blocked} info popup");
            }
        }
        if let InteractivePoi { behaviour: Some(behaviour), .. } = ipoi {
            if allowed.contains(TriggerKind::BEHAVIOUR) {
                const HOUR: Duration = Duration::from_secs(3600);
                const DAY: Duration = Duration::from_secs(HOUR.as_secs() * 24);
                const WEEK: Duration = Duration::from_secs(DAY.as_secs() * 7);

                use taimi_pack::attributes::keys::{Behaviour, TacoBehaviour, BlishBehaviour};
                let timestamp = rt::log::error_ok(UNIX_EPOCH.elapsed()).unwrap_or_default();
                let mut contexts = None;
                let delay = match behaviour.mode {
                    Behaviour::Taco(TacoBehaviour::ResetDaily) | Behaviour::Taco(TacoBehaviour::ResetDailyPerCharacter) => Some(Duration::from_secs({
                        if let Behaviour::Taco(TacoBehaviour::ResetDailyPerCharacter) = behaviour.mode {
                            contexts = Some(HideContext::for_character(self.filter_state.character.name.clone()));
                        }
                        const SOME_DAY: Duration = Duration::from_secs(1754265600);
                        (SOME_DAY.as_secs() as i64).wrapping_sub(timestamp.as_secs() as i64).wrapping_rem_euclid(DAY.as_secs() as i64)
                    } as u64)),
                    Behaviour::Blish(BlishBehaviour::ResetWeekly) => Some(Duration::from_secs({
                        const SOME_WEEK: Duration = Duration::from_secs(1754265600);
                        (SOME_WEEK.as_secs() as i64).wrapping_sub(timestamp.as_secs() as i64).wrapping_rem_euclid(WEEK.as_secs() as i64)
                    } as u64)),
                    Behaviour::Taco(TacoBehaviour::ResetDelay) => Some(behaviour.reset_delay.duration()),
                    Behaviour::Taco(TacoBehaviour::AlwaysVisible) => Some(Duration::from_secs(0)),
                    Behaviour::Taco(TacoBehaviour::ResetPermanent) => None,
                    Behaviour::Taco(TacoBehaviour::ResetMap) => {
                        contexts = Some(HideContext::for_map(loaded_path.root.path, None));
                        None
                    },
                    Behaviour::Taco(TacoBehaviour::ResetInstance) => {
                        contexts = Some(HideContext::for_map(loaded_path.root.path, NonZero::new(self.filter_state.map.shard_id)));
                        None
                    },
                    Behaviour::Taco(behaviour) => {
                        log::debug!("TODO: {behaviour:?}");
                        Some(HOUR)
                    },
                };
                log::info!("hiding marker for {delay:?}({contexts:?})");
                let contexts = contexts.into_iter().collect();
                PathingEvent::DismissMarker(loaded_path.root.rel(path.path), delay, contexts).try_send();
            } else {
                log::info!("{blocked} dismiss behaviour");
            }
        }
        if let InteractivePoi { copy: Some(copy), .. } = ipoi {
            if allowed.contains(TriggerKind::COPY) {
                RenderState::try_send(RenderEvent::SendClipboard(copy.value[..].into()));
                let msg = copy.message.clone().map(|m| String::from(&m[..]))
                    .unwrap_or_else(|| crate::fl!("copied").into());
                let message = format!("{msg}\n\n{:?}", &copy.value.0[..]);
                ctx.spawn_alert(message, Duration::from_secs(6));
            } else {
                log::info!("{blocked} copy");
            }
        }
        for show_hide in ipoi.show_hide() {
            if allowed.contains(TriggerKind::TOGGLE) {
                let cat_path = show_hide.category().pivot(loaded_path.root.root);
                // TODO: spawn instead to ensure it arrives?
                PathingEvent::CategorySetToggle(cat_path, show_hide.action.tristate()).try_send();
            } else {
                log::info!("{blocked} {}", show_hide.action);
            }
        }
        if let InteractivePoi { reset: Some(reset), .. } = ipoi {
            if allowed.contains(TriggerKind::RESET) {
                PathingEvent::GuidReset(reset.guid.iter().cloned().collect()).try_send();
            } else {
                log::info!("{blocked} reset");
            }
        }
        if let InteractivePoi { script: Some(..), .. } = ipoi {
            if allowed.contains(TriggerKind::SCRIPT) {
                log::debug!("TODO: interact script");
            } else {
                log::info!("{blocked} script");
            }
        }
        if let InteractivePoi { bounce: Some(..), .. } = ipoi {
            if allowed.contains(TriggerKind::BOUNCE) {
                log::debug!("TODO: interact bounce anim");
            } else {
                log::info!("{blocked} animation");
            }
        }
    }

    pub(crate) async fn set_visible(&mut self, context: Option<MapContext>, set: Option<bool>) {
        let set = {
            let mut settings = self.loader.settings.write().await;
            let pathing = settings.pathing_mut();
            let (is_visible, out) = match context {
                Some(MapContext::Global) => (
                    pathing.space.visible_worldmap(),
                    &mut pathing.space.visible_map_world,
                ),
                Some(MapContext::Minimap) => (
                    pathing.space.visible_minimap(),
                    &mut pathing.space.visible_map_mini,
                ),
                None => (pathing.space.visible_space(), &mut pathing.space.visible_space),
            };
            let set = set.unwrap_or(!is_visible);
            *out = Some(set);
            settings.mark_dirty();
            set
        };

        #[cfg(feature = "goggles")]
        match (context, set) {
            (None, true) =>
                Engine::try_send(SpaceEvent::GogglesRefreshLens { force: false, delay_override: Some(2) }),
            (None, false) => Engine::try_send(SpaceEvent::GogglesClearLens),
            _ => (),
        }
        Engine::try_send(SpaceEvent::SettingsDirty);
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

    fn handle_press_interact(&mut self, ctx: &mut PathingEventContext, map_id: MapIndex) {
        self.trigger_interact_action(ctx, map_id, InteractionEventAction::Interact)
    }

    fn trigger_interact_action(&mut self, ctx: &mut PathingEventContext, map_id: MapIndex, action: InteractionEventAction) {
        let maps = self.map_packs.iter_mut()
            .filter(|(path, map)| path.path == map_id && !map.interactive_pois.is_empty());
        let mut playerpos = None;
        let mut nearby_pois = BinaryHeap::new();
        for (path, map) in maps {
            let Some(info) = self.map_pack_info.get(path) else { continue };
            let playerpos = playerpos.get_or_insert_with(|| PathingEventContext::read_player_pos().map(|pos| {
                ctx.player_pos = pos;
                pos
            })).clone();
            let Some(playerpos) = playerpos else { break };
            if map.interactive_pois_nearby.is_empty() {
                map.interactive_pois_nearby.resize(map.interactive_pois.len(), false);
            }

            let ipois = map.interactive_pois.iter()
                .zip(map.interactive_pois_nearby.iter_mut())
                .enumerate();
            for (i, (ipoi, nearby_bit)) in ipois {
                let Some(lpoi) = ipoi.loaded_poi(&map.pois) else { continue };
                let Some(nearby) = ipoi.is_nearby(lpoi.position, playerpos) else { continue };
                // TODO: *nearby_bit = true?
                let nearby_discrete = (nearby * 1_000_000.0)
                    .min(0x40000000u32 as f32) as u32;
                let prev_nearby = *nearby_bit;
                let auto_triggered = ipoi.is_passive() && prev_nearby;
                let interactive_path = Locator::with_path(i as PoiIndex);
                let loaded_path = path.rel(ipoi.loaded_index().path);
                let path =
                    info.pois().nth(loaded_path.path as usize)
                    .unwrap_or(Locator::with_path(PoiIndex::MAX));
                nearby_pois.push(cmp::Reverse((nearby_discrete, !ipoi.trigger.auto, !auto_triggered, (path, loaded_path, interactive_path))));
            }
        }
        if nearby_pois.is_empty() {
            // TODO: fall back to non-interactive pois in case user is trying to dismiss or get info about a marker?
            // (maybe on a different keybind though?)
            return
        }
        ctx.pack_info.send_if_modified(|shared_info| {
            for cmp::Reverse((_distdist, _, _, (path, loaded_path, interactive_path))) in nearby_pois {
                let _ = shared_info.interactions.send(InteractionEvent::Interact {
                    action,
                    path,
                    loaded_path,
                    interactive_path,
                });
            }
            false
        });
    }

    #[inline]
    pub fn try_send(e: PathingEvent) {
        let Ok(sender) = crate::CONTROLLER_SENDER.try_read() else { return };
        sender.pathing_try_send(e);
    }
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

#[derive(Debug, Clone)]
pub struct MapPackInfoStorage {
    pub used: RecentlyUsed,
    pub info: Arc<MapPackInfo>,
}

impl MapPackInfoStorage {
    pub const fn new(info: Arc<MapPackInfo>) -> Self {
        Self {
            used: RecentlyUsed::DEFAULT,
            info,
        }
    }
}

impl ops::Deref for MapPackInfoStorage {
    type Target = MapPackInfo;
    fn deref(&self) -> &Self::Target {
        &self.info
    }
}

#[derive(Debug, Clone)]
pub struct MapPackInfo {
    pub pois: BitVec,
    pub trails: BitVec,
    pub categories: Box<[CategoryIndex]>,
    /// TODO: not all GUIDs are needed at runtime,
    /// if for example the marker can't be interacted with
    #[cfg(todo)]
    pub poi_guid_mask: BitVec,
    #[cfg(todo)]
    pub trail_guid_mask: BitVec,
}

impl MapPackInfo {
    pub fn empty() -> Self {
        Self {
            pois: BitVec::new(),
            trails: BitVec::new(),
            categories: Default::default(),
        }
    }

    pub fn with_pack(pack: &LoadedPack, map_id: MapIndex) -> Self {
        let Some(active) = &pack.active else {
            return Self::empty()
        };

        // TODO: this doesn't need to use the string ids anymore...
        let id32 = map_id.get() as i32;
        let mut categories = {
            let category_estimate = active.pack.categories.all_categories.len() / 32;
            Vec::<CategoryIndex>::with_capacity(category_estimate)
        };
        let mut insert_cat = |category: &FullIdRef| -> bool {
            if let Some(idx) = active.pack.categories.all_categories.get_index_of(category) {
                let idx = idx as CategoryIndex;
                let insert = categories.partition_point(|&i| i < idx);
                match categories.get(insert) {
                    Some(&i) if i == idx => false,
                    _ => {
                        categories.insert(insert, idx);
                        true
                    },
                }
            } else {
                true
            }
        };
        let mut filter_mapid = |map_id: i32, mut category: &FullIdRef| -> bool {
            if map_id == id32 {
                loop {
                    if !insert_cat(category) { break }
                    category = match category.parent() {
                        Some(parent) => parent,
                        None => break,
                    };
                }
                true
            } else {
                false
            }
        };
        let mut pois = BitVec::new();
        let mut active_pois = active.pack.pois.iter().enumerate()
            .filter(|(_i, poi)| filter_mapid(poi.map_id, poi.category.as_ref()))
            .map(|(i, _)| i)
            .rev();
        if let Some(i) = active_pois.next() {
            pois.reserve_exact(i + 1);
            pois.resize(i, false);
            pois.push(true);
        }
        for i in active_pois {
            pois.set(i, true);
        }
        #[cfg(todo)]
        let trails = active.pack.trails.iter().enumerate()
            .filter(|(_i, trail)| filter_mapid(trail.map_id.unwrap_or(0), &trail.category))
            .map(|(i, _)| i)
            .collect::<BitVec>();
        // TODO: use some sort of space-efficient encoding like RLE for these masks
        // even just an initial offset or vec of bit group lengths (pos/neg for 0 vs 1) would help?
        #[cfg(deleteme)]
        let pois = active.pack.pois.iter()
            .map(|poi| filter_mapid(poi.map_id, &poi.category))
            .collect::<BitVec>();
        let mut trails = BitVec::new();
        let mut active_trails = active.pack.trails.iter().enumerate()
            .filter(|(_i, trail)| filter_mapid(trail.map_id.unwrap_or(0), trail.category.as_ref()))
            .map(|(i, _)| i)
            .rev();
        if let Some(i) = active_trails.next() {
            trails.reserve_exact(i + 1);
            trails.resize(i, false);
            trails.push(true);
        }
        for i in active_trails {
            trails.set(i, true);
        }

        let categories = categories.into_boxed_slice();

        Self {
            pois,
            trails,
            categories,
        }
    }

    pub async fn load_from_pack(pack: &mut LoadedPack, map_id: MapIndex, _manager: &PackLoader) -> anyhow::Result<Self> {
        // TODO...
        Ok(Self::with_pack(&*pack, map_id))
    }

    pub fn is_empty(&self) -> bool {
        (self.trails.is_empty() || self.trails[..].not_any())
            && (self.pois.is_empty() || self.pois[..].not_any())
    }

    /// None if ![self.is_empty()]
    pub fn get(self) -> Option<Self> {
        (!self.is_empty()).then_some(self)
    }

    pub fn poi_count(&self) -> usize {
        self.pois.count_ones()
    }
    pub fn pois(&self) -> impl Iterator<Item = PoiPath> + '_ {
        self.pois.iter_ones()
            .map(|i| PoiPath::with_path(i as PoiIndex))
    }
    #[cfg(todo)]
    pub(crate) fn poi_guid_mask(&self) -> impl Iterator<Item = bool> + '_ {
        self.poi_guid_mask.iter()
    }
    pub(crate) fn poi_guid_mask(&self) -> impl Iterator<Item = bool> + '_ {
        iter::repeat(true).take(self.poi_count())
    }
    pub(crate) fn poi_guid_filter<'a, I>(&'a self, iter: I) -> impl Iterator<Item = I::Item> + 'a where
        I: IntoIterator + 'a,
    {
        self.poi_guid_mask().zip(iter)
            .filter_map(|(mask, v)| mask.then_some(v))
    }
    pub fn poi_index(&self, path: PoiPath) -> Option<PoiIndex> {
        let path = path.path as usize;
        self.pois.iter_ones().position(|p| p == path)
            .map(|p| p as PoiIndex)
    }
    pub fn trail_count(&self) -> usize {
        self.trails.count_ones()
    }
    pub fn trails(&self) -> impl Iterator<Item = TrailPath> + '_ {
        self.trails.iter_ones()
            .map(|i| TrailPath::with_path(i as TrailIndex))
    }
    #[cfg(todo)]
    pub(crate) fn trail_guid_mask(&self) -> impl Iterator<Item = bool> + '_ {
        self.trail_guid_mask.iter()
    }
    pub(crate) fn trail_guid_mask(&self) -> impl Iterator<Item = bool> + '_ {
        iter::repeat(true).take(self.trail_count())
    }
    pub(crate) fn trail_guid_filter<'a, I>(&'a self, iter: I) -> impl Iterator<Item = I::Item> + 'a where
        I: IntoIterator + 'a,
    {
        self.trail_guid_mask().zip(iter)
            .filter_map(|(mask, v)| mask.then_some(v))
    }
    pub fn trail_index(&self, path: TrailPath) -> Option<TrailIndex> {
        let path = path.path as usize;
        self.trails.iter_ones().position(|t| t == path)
            .map(|i| i as TrailIndex)
    }
    pub fn category_count(&self) -> usize {
        self.categories.len()
    }
    pub fn category_max(&self) -> Option<CategoryIndex> {
        self.categories.iter().max().copied()
    }
    pub fn category_max_count(&self) -> CategoryIndex {
        self.category_max()
            .map(|c| c + 1)
            .unwrap_or(0)
    }
    pub fn categories(&self) -> impl Iterator<Item = CategoryPath> + '_ {
        self.categories.iter().copied().map(CategoryPath::with_path)
    }
    pub fn category_index(&self, path: CategoryPath) -> Option<CategoryIndex> {
        self.categories[..].iter().position(|&c| c == path.path)
            .map(|i| i as CategoryIndex)
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FestivalState {
    pub active: Festivals,
    pub on: Festivals,
    pub off: Festivals,
}

impl FestivalState {
    pub const DEFAULT: Self = Self {
        active: Festivals::empty(),
        on: Festivals::empty(),
        off: Festivals::empty(),
    };

    pub fn update_preferences(&mut self, (on, off): (Festivals, Festivals)) {
        self.on = on;
        self.off = off;
    }
    pub fn set_preference(&mut self, festival: Festival, pref: Option<FestivalPreference>) {
        let festival = Festivals::from(festival);
        self.on.remove(festival);
        self.off.remove(festival);
        match pref {
            Some(true) =>
                self.on.insert(festival),
            Some(false) =>
                self.off.insert(festival),
            None => (),
        }
    }

    pub fn get_preference(&self, festival: Festival) -> Option<FestivalPreference> {
        if self.off.get(festival) {
            Some(false)
        } else if self.on.get(festival) {
            Some(true)
        } else {
            None
        }
    }

    pub fn get(&self) -> Festivals {
        (self.active | self.on) & !self.off
    }
}

type SetupPoi = (PoiPath, Option<SpacePoiBuilder>, LoadedPoi);
type SetupTrail = (TrailPath, Option<SpaceTrailBuilder>, LoadedTrail);
