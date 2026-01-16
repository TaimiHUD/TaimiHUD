use {
    bvh::{aabb, bvh::Bvh, bounding_hierarchy::BHShape},
    crate::{
        controller::pathing::{
            registry::{PoiMapPath, LoadedPoiPath, PackMapPath, LoadedMarkerPath},
            shared::{PathingReceiver, InteractReceiver},
            state::{
                hidden::{AutoReset, HideContext},
                interactive::{InteractionEvent, InteractionEventAction, InteractivePoi},
                filter::FilterState,
                LoadedPoi, LoadedMapPack,
                LoadedMaps, LoadedMapInfo,
            },
            shared::LocDisplay,
            info::MapPackInfo,
            PathingController, PathingEvent,
        },
        controller::{runtime::WallInstant, Controller},
        exports::runtime as rt,
        render::{RenderEvent, RenderState},
        settings::pathing::{TriggerKind, PathingSettings},
        Interruption,
    },
    futures::future::{self, Either},
    std::{cmp, collections::BinaryHeap, num::NonZero, sync::{Arc, LazyLock}, time::Duration},
    std::task::{Poll, Context},
    std::future::Future,
    taimi_meta::{
        packs::{id::{MarkerId, MarkerIndex, MarkerPath}, MapIndex, PoiPath},
        spatial::{BvhShape, TriggerBoundsInfo},
    },
    taimi_pack::attributes::{keys, AttrString, InteractionAttributes},
    taimi_hoard::loc::LocationRef,
    taimi_hoard::flags::BitSet,
    taimi_hoard::time::Timestamp,
    tokio::sync::RwLock,
    tokio::sync::broadcast::{self, error::RecvError as BroadcastError},
    tokio::pin,
};

#[derive(Debug, Clone)]
pub struct SpaceInteraction {
    #[cfg(todo = "unnecessary")]
    pub id: MarkerId,
    pub path: PoiMapPath,
    pub bounds: TriggerBoundsInfo,
}
impl SpaceInteraction {
    pub fn with_poi(path: PoiMapPath, poi: &super::state::LoadedPoi) -> Self {
        let position = poi.position();
        let (radius, auto) = {
            let interaction = poi.interaction_attrs();

            (interaction.trigger_range(), interaction.auto_trigger())
        };
        let bounds = TriggerBoundsInfo::new(position, radius, auto);
        Self {
            path,
            bounds,
        }
    }
    pub fn is_interactive(poi: &super::state::LoadedPoi) -> bool {
        !poi.info().get_interaction_attrs().map(|i| Self::interaction_is_empty(i)).unwrap_or(true)
    }
    pub fn poi_path(&self) -> PoiMapPath {
        self.path
    }

    fn interaction_is_empty(i: &InteractionAttributes) -> bool {
        if i.info.is_some() { return false }
        if i.bounce_behavior.is_some() { return false }
        if i.copy_value.is_some() { return false }
        if i.reset_guids.is_some() { return false }
        if i.toggle_category.is_some() { return false }
        if i.show_category.is_some() { return false }
        if i.hide_category.is_some() { return false }
        if i.taco_behavior.is_some() { return false }
        #[cfg(todo)]
        if i.auto_trigger() { return false }

        true
    }
}
impl<const D: usize> aabb::Bounded<f32, D> for SpaceInteraction where
    TriggerBoundsInfo: aabb::Bounded<f32, D>,
{
    fn aabb(&self) -> aabb::Aabb<f32, D> {
        self.bounds.aabb()
    }
}

pub type TriggerBvh = Bvh<f32, {MapInteractState::TRIGGER_DIMENSION}>;
#[derive(Debug, Clone)]
pub struct MapInteractState {
    entities: Vec<BvhShape<SpaceInteraction>>,
    /// TODO: moving this whole struct into the arc may work better idk
    trigger_bvh: Arc<RwLock<TriggerBvh>>,
    trigger_bvh_dirty: bool,
    nearby: BitSet,
}
impl MapInteractState {
    /// POI trigger bvh traversal doesn't really need 3D?
    const TRIGGER_DIMENSION: usize = 2;
    #[cfg(todo = "unnecessary")]
    const TRIGGER_DIMENSION: usize = 3;
    pub fn new() -> Self {
        Self {
            entities: Default::default(),
            trigger_bvh: Arc::new(RwLock::new(Bvh { nodes: Vec::new() })),
            trigger_bvh_dirty: false,
            nearby: Default::default(),
        }
    }
    pub fn map_id(&self) -> Option<MapIndex> {
        // XXX: may track multiple maps in future, but they should be split across instances right?
        self.entities.first().map(|e| e.path.root.path)
    }

    /// TODO: partial updates and whatnot
    pub fn update_entities(&mut self, maps: &LoadedMaps, map_info: &LoadedMapInfo, map_id: MapIndex) {
        self.clear_active();

        for (map_path, map, map_info) in maps.iter_with_info(map_info, Some(map_id)) {
            let pois = map.pois(map_info)
                .filter(|(_, poi)| SpaceInteraction::is_interactive(poi))
                .map(|(lpath, poi)| BvhShape::new(SpaceInteraction::with_poi(map_path.rel(lpath.path), poi)));
            self.entities.extend(pois);
        }
        self.trigger_bvh_dirty = !self.entities.is_empty();
    }
    /// TODO: rebuild with executor
    ///
    /// TODO: waiting on mutex is unnecessary if you just replace it with a new one if a try_lock fails...
    pub async fn rebuild_trigger_bvh(&mut self) {
        if !self.trigger_bvh_dirty { return }
        #[cfg(todo = "unnecessary")]
        if self.entities.is_empty() { self.trigger_bvh = Self::empty_bvh_rw().clone(); self.trigger_bvh_dirty = false; return }

        self.ensure_bvh_rw();

        let mut shapes = self.entities.clone();
        let res = Controller::try_run_blocking("POI interaction hierarchy", {
            let mut trigger_bvh = self.trigger_bvh.clone().write_owned().await;
            move || {
                *trigger_bvh = Bvh::build(&mut shapes[..]);
                trigger_bvh.nodes.shrink_to_fit();
                Ok(shapes)
            }
        }).await;
        match rt::log::error_ok(res) {
            None => {
                // what do now..?
                self.trigger_bvh = Self::empty_bvh_rw().clone();
            },
            Some(shapes) => {
                for (dest, shape) in self.entities.iter_mut().zip(&shapes) {
                    let bh_index = BHShape::<f32, {Self::TRIGGER_DIMENSION}>::bh_node_index(shape);
                    BHShape::<f32, {Self::TRIGGER_DIMENSION}>::set_bh_node_index(dest, bh_index);
                }
            },
        };

        self.trigger_bvh_dirty = false;
    }
    pub fn clear_active(&mut self) {
        self.entities.clear();
        self.nearby.clear();
        let empty_bvh_rw = Self::empty_bvh_rw();
        let trigger_bvh_cleared = if Arc::ptr_eq(&self.trigger_bvh, empty_bvh_rw) {
            // nothing to do...
            true
        } else {
            #[cfg(todo = "unnecessary")]
            if let Ok(mut trigger_bvh) = self.trigger_bvh.try_write() {
                // wishful thinking that we'd be able to reuse the vec :<
                trigger_bvh.nodes.clear();
                true
            }
            // not a big deal if we failed to lock it, just ditch the last one
            false
        };
        if !trigger_bvh_cleared {
            self.trigger_bvh = empty_bvh_rw.clone();
        }
        self.trigger_bvh_dirty = false;
    }
    pub fn clear(&mut self) {
        self.entities = Vec::new();
        self.nearby = BitSet::default();
        self.trigger_bvh = Self::empty_bvh_rw().clone();
        self.trigger_bvh_dirty = false;
    }

    fn ensure_bvh_rw(&mut self) {
        if Arc::ptr_eq(&self.trigger_bvh, Self::empty_bvh_rw()) {
            self.trigger_bvh = Arc::new(RwLock::new(Bvh { nodes: Vec::new() }));
        }
    }
    fn empty_bvh_rw() -> &'static Arc<RwLock<TriggerBvh>> {
        static EMPTY_BVH_RW: LazyLock<Arc<RwLock<TriggerBvh>>> = LazyLock::new(||
            Arc::new(RwLock::new(Bvh { nodes: Vec::new() }))
        );
        &EMPTY_BVH_RW
    }

    fn shrink_to_fit(&mut self) {
        self.entities.shrink_to_fit();
        #[cfg(todo = "unnecessary")]
        if let Some(trigger_bvh) = self.trigger_bvh.try_write() {
            trigger_bvh.shrink_to_fit();
        }
    }
}
impl Default for MapInteractState {
    fn default() -> Self {
        Self::new()
    }
}
#[derive(Debug, Clone, Default)]
pub struct InteractReactor {
    pub map_interactions: MapInteractState,
    pub config: InteractSettings,
}
impl InteractReactor {
    pub(super) fn handle_map_enter(&mut self, maps: &LoadedMaps, map_info: &LoadedMapInfo, map_id: MapIndex) {
        self.map_interactions.update_entities(maps, map_info, map_id);
    }
    pub(super) fn handle_map_leave(&mut self) {
        self.map_interactions.clear_active();
    }
    pub(super) fn process_interaction(
        &mut self,
        filter_state: &FilterState,
        (path, loaded_path, lpoi): (PoiPath, LoadedPoiPath<PackMapPath>, &LoadedPoi),
        action: InteractionEventAction,
        allowed: TriggerKind,
    ) -> PathingEvent {
        if allowed.is_empty() {
            return PathingEvent::Nop
        }
        let attrs = lpoi.interaction_attrs();
        let mut events = Vec::new();
        let marker_path = path.pivot_from();
        let lpath = loaded_path.map_path(MarkerIndex::with_poi);

        let mut took_action = None;
        let blocked = "trigger settings blocked";
        if let Some(message) = attrs.info.as_ref() {
            let allowed = allowed.contains(TriggerKind::INFO);
            *took_action.get_or_insert_default() |= allowed;
            if allowed {
                events.push(PathingEvent::TriggerMarkerInfo {
                    path: marker_path,
                    loaded_path: lpath,
                    message: message.clone(),
                });
            } else {
                log::info!("{blocked} info popup");
            }
        }
        if let Some(value) = attrs.copy_value.as_ref() {
            let allowed = allowed.contains(TriggerKind::COPY);
            *took_action.get_or_insert_default() |= allowed;
            if allowed {
                events.push(PathingEvent::TriggerMarkerCopy {
                    path: marker_path,
                    loaded_path: lpath,
                    value: value.clone(),
                    message: attrs.copy_message.clone(),
                });
            } else {
                log::info!("{blocked} copy");
            }
        }
        for (id, show_hide) in attrs.category_actions() {
            let allowed = allowed.contains(TriggerKind::TOGGLE);
            *took_action.get_or_insert_default() |= allowed;
            if allowed {
                #[cfg(todo)]
                let cat = show_hide.category().pivot(loaded_path.root.root);
                let cat = id.clone();
                // TODO: spawn instead to ensure it arrives?
                events.push(PathingEvent::CategoryEnableById(loaded_path.root.root, cat, show_hide.tristate()));
            } else {
                log::info!("{blocked} {}", show_hide);
            }
        }
        if let Some(reset @ &[_, ..]) = attrs.reset_guids() {
            let allowed = allowed.contains(TriggerKind::RESET);
            *took_action.get_or_insert_default() |= allowed;
            if allowed {
                let ids = reset.iter().map(|guid| MarkerId::from_uuid_ref(guid.as_ref()).clone()).collect();
                events.push(PathingEvent::ResetMarkerIds(ids));
            } else {
                log::info!("{blocked} reset");
            }
        }
        let script = lpoi.info().get_marker_attrs().and_then(|m| m.script.as_ref());
        if let Some(script) = script {
            let allowed = allowed.contains(TriggerKind::SCRIPT);
            *took_action.get_or_insert_default() |= allowed;
            if allowed {
                log::debug!("TODO: interact script");
            } else {
                log::info!("{blocked} script");
            }
        }
        if let Some(bounce) = attrs.bounce_behavior {
            if allowed.contains(TriggerKind::BOUNCE) {
                log::debug!("TODO: interact bounce anim");
            } else {
                log::info!("{blocked} animation");
            }
        }

        let behaviour = match &action {
            InteractionEventAction::Dismiss(config) => Some((config.mode, config.reset_delay.into())),
            _ => attrs.behaviour().map(|b| (b, attrs.reset_delay())),
        };
        if let Some((behaviour, reset_delay)) = behaviour {
            let organic = match action.is_natural() {
                true => took_action.unwrap_or(true),
                false => true,
            };
            if allowed.contains(TriggerKind::BEHAVIOUR) && organic {
                const MANY_WEEKS: Duration = Duration::from_secs(Timestamp::WEEK.as_secs() * 52);

                use taimi_pack::attributes::keys::{Behaviour, TacoBehaviour, BlishBehaviour};
                let mut now = None;
                let mut now = || *now.get_or_insert_with(WallInstant::now_timestamp_system_checked);
                let mut contexts = None;
                let mut reset = None;
                let until = match behaviour {
                    Behaviour::Taco(TacoBehaviour::ResetDaily) | Behaviour::Taco(TacoBehaviour::ResetDailyPerCharacter) => Some(Either::Left(Timestamp::with_timestamp({
                        if let Behaviour::Taco(TacoBehaviour::ResetDailyPerCharacter) = behaviour {
                            contexts = Some(HideContext::for_character(filter_state.character.name.clone()));
                        }
                        const SOME_DAY: Timestamp = Timestamp::with_timestamp(1754265600 - MANY_WEEKS.as_secs() * 13);
                        (SOME_DAY.timestamp() as i64).wrapping_sub(now().timestamp() as i64).wrapping_rem_euclid(Timestamp::DAY.as_secs() as i64)
                    } as u64))),
                    Behaviour::Blish(BlishBehaviour::ResetWeekly) => Some(Either::Left(Timestamp::with_timestamp({
                        const SOME_WEEK: Timestamp = Timestamp::with_timestamp(1754265600 - MANY_WEEKS.as_secs() * 13);
                        (SOME_WEEK.timestamp() as i64).wrapping_sub(now().timestamp() as i64).wrapping_rem_euclid(Timestamp::WEEK.as_secs() as i64)
                    } as u64))),
                    Behaviour::Taco(TacoBehaviour::ResetDelay) => Some(Either::Right(keys::ResetLength(reset_delay).duration())),
                    Behaviour::Taco(TacoBehaviour::AlwaysVisible) => Some(Either::Right(Duration::from_secs(0))),
                    Behaviour::Taco(TacoBehaviour::ResetPermanent) => {
                        reset = Some(AutoReset::Never);
                        None
                    },
                    Behaviour::Taco(TacoBehaviour::ResetMap) => {
                        contexts = Some(HideContext::for_map(loaded_path.root.path, None));
                        reset = Some(AutoReset::MapChange);
                        None
                    },
                    Behaviour::Taco(TacoBehaviour::ResetInstance) => {
                        contexts = Some(HideContext::for_map(loaded_path.root.path, NonZero::new(filter_state.map.shard_id)));
                        None
                    },
                    Behaviour::Taco(behaviour) => {
                        log::debug!("TODO: {behaviour:?}");
                        Some(Either::Right(Timestamp::HOUR))
                    },
                };
                log::info!("hiding marker for {:?}({contexts:?})", until);
                let contexts = contexts.into_iter().collect();
                events.push(PathingEvent::DismissMarker {
                    path: marker_path,
                    loaded_path: lpath,
                    until,
                    contexts,
                    reset,
                });
            } else {
                log::info!("{blocked} dismiss behaviour");
            }
        } else if action.is_natural() && took_action.unwrap_or(false) {
            let contexts = vec![HideContext::for_map(loaded_path.root.path, None)];
            events.push(PathingEvent::DismissMarker {
                path: marker_path,
                loaded_path: lpath,
                until: Some(Either::Right(Self::INTERACT_COOLDOWN)),
                contexts,
                reset: Some(AutoReset::Distance),
            });
        }
        PathingEvent::FanOut(events).flatten()
    }
    pub const INTERACT_COOLDOWN: Duration = Duration::from_secs(120);

    pub(super) fn allow_action(
        &mut self,
        (loaded_path, lpoi): (LoadedPoiPath<PackMapPath>, &LoadedPoi),
        action: &InteractionEventAction,
    ) -> TriggerKind {
        let is_filtered = || {
            if !lpoi.visibility.is_visible() {
                return true
            }
            log::debug!("TODO: POI autoreset filter");
            false
        };
        match action {
            InteractionEventAction::Trigger => TriggerKind::all(),
            InteractionEventAction::Dismiss(..) =>
                TriggerKind::DISMISS,
            InteractionEventAction::Manual(mask) => *mask,
            action if action.is_natural() && is_filtered() => {
                let display = match lpoi {
                    #[cfg(todo)]
                    _ => LocDisplay((loaded_path, lpoi)),
                    _ => LocDisplay({
                        let lpoi_path: LoadedPoiPath = loaded_path.unscope();
                        loaded_path.root.rel(lpoi_path)
                    }),
                };
                log::debug!("ignoring filtered POI interaction for {display}");
                TriggerKind::empty()
            },
            InteractionEventAction::Interact => self.config.trigger_allow_interact,
            InteractionEventAction::AutoTrigger => self.config.trigger_allow_auto,
        }
    }
    pub(super) fn prepare_action<'a>(
        &mut self,
        event: &InteractionEvent,
        map_info: &'_ LoadedMapInfo,
        maps: &'a LoadedMaps,
    ) -> Option<(
        PoiPath, LoadedPoiPath<PackMapPath>, &'a LoadedPoi, InteractionEventAction,
    )> {
        match event {
            &InteractionEvent::Nearby { path, loaded_path } => {
                let (map, _map_info) = maps.lookup_with_info(map_info, &loaded_path.root)?;
                let lpath: LoadedPoiPath = loaded_path.unscope();
                let lpoi = map.lpois().lookup_ref(&lpath)?;
                let auto_trigger_configured = || {
                    log::debug!("TODO: auto-trigger setting");
                    true
                };
                let action = if lpoi.interaction_attrs().auto_trigger() && auto_trigger_configured() {
                    Some(InteractionEventAction::AutoTrigger)
                } else {
                    None
                };
                action.map(|action| (path, loaded_path, lpoi, action))
            },
            &InteractionEvent::Interact { ref action, path, loaded_path } => {
                let (map, _map_info) = maps.lookup_with_info(map_info, &loaded_path.root)?;
                let lpath: LoadedPoiPath = loaded_path.unscope();
                let lpoi = map.lpois().lookup_ref(&lpath)?;
                Some((path, loaded_path, lpoi, action.clone()))
            },
            InteractionEvent::Gone { .. } => None,
        }
    }
    pub(super) fn process_interact<'a>(
        &mut self,
        filter_state: &FilterState,
        map_info: &LoadedMapInfo,
        maps: &LoadedMaps,
        event: InteractionEvent,
    ) -> PathingEvent {
        match event {
            InteractionEvent::Gone { path, loaded_path } => {
                self.process_gone(filter_state, map_info, maps, (path, loaded_path))
            },
            ref event @ (InteractionEvent::Nearby { .. } | InteractionEvent::Interact { .. }) => {
                let Some((path, lpath, lpoi, action)) = self.prepare_action(&event, map_info, maps) else {
                    return PathingEvent::Nop
                };
                let allowed = self.allow_action((lpath, lpoi), &action);
                self.process_interaction(filter_state, (path, lpath, lpoi), action, allowed)
            },
        }
    }
    pub(super) fn process_gone<'a>(
        &mut self,
        filter_state: &FilterState,
        map_info: &'_ LoadedMapInfo,
        maps: &'a LoadedMaps,
        (path, lpath): (PoiPath, PoiMapPath),
    ) -> PathingEvent {
        let Some((map, map_info)) = maps.lookup_with_info(map_info, &lpath.root) else {
            return PathingEvent::Nop
        };
        let marker_path = lpath.root.root.rel(MarkerIndex::with_poi(path.path));
        // TODO: nth with option variant
        let guid = map.poi_guids(map_info)
            .find(|(p, ..)| p.path == path.path)
            .map(|(_, _, guid)| guid.clone());
        let mut msg = if self.handle_interaction_end(filter_state, &MarkerId::for_marker(marker_path)) {
            PathingEvent::ResetMarkerPath(marker_path)
        } else {
            PathingEvent::Nop
        };
        if let Some(guid) = guid {
            // TODO: does ResetMarkerPath already fan this out and make this unnecessary? could be an or...
            if self.handle_interaction_end(filter_state, guid.0.as_ref()) {
                msg = msg.join(PathingEvent::ResetMarkerIds(vec![MarkerId::with_uuid(guid.into())]));
            }
        }

        // remove on-screen info maybe?
        msg
    }
    fn handle_interaction_end(&mut self, filter_state: &FilterState, marker_id: &MarkerId) -> bool {
        let Some(hidden) = filter_state.hidden.hidden.get(marker_id) else {
            return false
        };
        match hidden.reset {
            AutoReset::Distance => (),
            AutoReset::Never | AutoReset::Expiry { .. } | AutoReset::MapChange =>
                return false,
        }

        true
    }


    pub(super) fn poll_event(&mut self, cx: &mut Context, rx: &mut InteractReceiver) -> Poll<InteractMessage> {
        for _ in 0..Self::EVENT_RX_RETRY {
            let event_rx = rx.event_rx.recv();
            pin!(event_rx);
            match event_rx.poll(cx) {
                Poll::Ready(Ok(e)) => return Poll::Ready(InteractMessage::Event(e)),
                Poll::Ready(Err(BroadcastError::Lagged(amt))) => {
                    log::warn!("lagged behind by {amt} interactions");
                    // TODO: clear out queue if this recurs?
                },
                Poll::Ready(Err(BroadcastError::Closed)) | Poll::Pending => break,
            }
        }
        if self.map_interactions.trigger_bvh_dirty {
            return Poll::Ready(InteractMessage::BvhRebuild)
        }
        Poll::Pending
    }
    pub(super) fn with_rx<'a>(&'a mut self, rx: &'a mut InteractReceiver) -> impl Future<Output = InteractMessage> + 'a {
        future::poll_fn(move |cx| self.poll_event(cx, rx))
    }
    const EVENT_RX_RETRY: usize = 2;
    pub(super) async fn process_event(
        &mut self,
        _rx: &mut PathingReceiver,
        (maps, map_info, filter_state): (&LoadedMaps, &LoadedMapInfo, &FilterState),
        msg: InteractMessage,
    ) -> PathingEvent {
        match msg {
            InteractMessage::BvhRebuild => {
                // TODO: spawn/bg this thanks
                self.map_interactions.rebuild_trigger_bvh().await;
                PathingEvent::Nop
            },
            InteractMessage::Event(event) => {
                self.process_interact(filter_state, map_info, maps, event)
            },
        }
    }
    pub(super) async fn collect_garbage(&mut self, _rx: &mut PathingReceiver, (_map_info, _maps): (&LoadedMapInfo, &LoadedMaps), map_id: Option<MapIndex>, aggressive: bool) {
        let interact_map_id = self.map_interactions.map_id();
        let map_dirty = interact_map_id != map_id;
        match (map_dirty, aggressive) {
            (false, false) => (),
            (true, false) =>
                self.map_interactions.clear_active(),
            (false, true) if interact_map_id.is_some() => {
                self.map_interactions.shrink_to_fit();
            },
            (_, true) =>
                self.map_interactions.clear(),
        }
    }
    pub(super) fn exit(&mut self, rx: &mut InteractReceiver, _reason: Interruption) {
        rx.event_rx = broadcast::Sender::new(1).subscribe();
        self.map_interactions.clear();
    }
}
#[cfg(todo)]
impl Future for InteractReactor {
    type Output = InteractMessage;
    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        self.get_mut().poll_event_fallback(cx)
    }
}
pub enum InteractMessage {
    BvhRebuild,
    Event(InteractionEvent),
}

impl PathingController {
    /// TODO
    pub(super) fn can_interact(interaction: &InteractionAttributes) -> bool {
        // if they exist, sure
        true
    }

    pub(super) async fn process_marker_copy(&mut self, path: MarkerPath, loaded_path: LoadedMarkerPath<PackMapPath>, value: AttrString, message: Option<AttrString>) {
        log::debug!("TODO: marker copy {} {message:?}", &value[..]);
    }
    pub(super) async fn process_marker_info(&mut self, path: MarkerPath, loaded_path: LoadedMarkerPath<PackMapPath>, message: AttrString) {
        log::debug!("TODO: marker info {}", &message[..]);
    }
    pub(super) fn process_marker_dismiss(&mut self, path: MarkerPath, loaded_path: LoadedMarkerPath<PackMapPath>, until: Option<Either<Timestamp, Duration>>, contexts: Vec<HideContext>, reset: Option<AutoReset>) {
        if let MarkerIndex::NS_POI = loaded_path.path.namespace() {
            let lpoi_path: LoadedPoiPath = LoadedPoiPath::with_path(loaded_path.path.index_poi_unchecked());
            self.filter_dismiss_poi(lpoi_path.pivot(loaded_path.root), until, contexts, reset);
        } else {
            log::debug!("TODO: marker dismiss {}", LocDisplay(loaded_path));
        }
    }
}
#[cfg(todo)]
impl PathingController {
    pub(super) async fn handle_interaction(&mut self, ctx: &mut PathingEventContext, event: InteractionEvent) {
        let (path, loaded_path, ipoi, lpoi, action) = match event {
            InteractionEvent::Nearby { path, loaded_path, interactive_path } => {
                let Some(map) = self.map_packs.get(&loaded_path.root) else { return };
                let Some(ipoi) = map.interactive_pois.get(interactive_path.path as usize) else { return };
                let lpoi = map.pois.get(loaded_path.path as usize);
                let auto_trigger_configured = || {
                    log::debug!("TODO: auto-trigger setting");
                    true
                };
                let action = if ipoi.trigger.auto && auto_trigger_configured() {
                    InteractionEventAction::AutoTrigger
                } else {
                    return
                };
                (path, loaded_path, ipoi, lpoi, action)
            },
            InteractionEvent::Gone { path, loaded_path, interactive_path: _ } => {
                let Some(map_info) = self.map_pack_info.get(&loaded_path.root) else { return };
                let Some(map) = self.map_packs.get(&loaded_path.root) else { return };
                let marker_path = loaded_path.root.root.rel(MarkerIndex::with_poi(path.path));
                // TODO: nth with option variant
                let guid = map.poi_guids(map_info)
                    .find(|(p, _)| p.path == path.path)
                    .map(|(_, guid)| guid.clone());
                let mut removed = self.handle_interaction_end(ctx, &MarkerId::for_marker(marker_path));
                if let Some(guid) = guid {
                    removed |= self.handle_interaction_end(ctx, guid.0.as_ref());
                }
                if removed {
                    ctx.filter_state_signal = true;
                    self.mark_hidden_dirty(ctx, Some(loaded_path.root));
                }

                // remove on-screen info maybe?
                return
            },
            InteractionEvent::Interact { action, path, loaded_path, interactive_path } => {
                let Some(map) = self.map_packs.get(&loaded_path.root) else { return };
                let Some(ipoi) = map.interactive_pois.get(interactive_path.path as usize) else { return };
                let lpoi = map.pois.get(loaded_path.path as usize);
                (path, loaded_path, ipoi, lpoi, action)
            },
        };

        let mut behaviour = ipoi.behaviour.as_ref();
        let allowed = {
            let settings = self.loader.settings.read().await;
            let pathing = settings.pathing();
            let is_filtered = || {
                if lpoi.as_ref().map(|lpoi| !lpoi.visibility.is_visible()).unwrap_or(false) {
                    return true
                }
                log::debug!("TODO: POI autoreset filter");
                false
            };
            match action {
                InteractionEventAction::Trigger => TriggerKind::all(),
                InteractionEventAction::Dismiss(ref config) => {
                    behaviour = Some(config);
                    TriggerKind::DISMISS
                },
                InteractionEventAction::Manual(mask) => mask,
                action if action.is_natural() && is_filtered() => {
                    log::debug!("ignoring filtered POI interaction for {loaded_path}");
                    return
                },
                InteractionEventAction::Interact => pathing.trigger_allow_interact,
                InteractionEventAction::AutoTrigger => pathing.trigger_allow_auto,
            }
        };

        let mut took_action = None;
        let blocked = "trigger settings blocked";
        if let InteractivePoi { info: Some(info), .. } = ipoi {
            let allowed = allowed.contains(TriggerKind::INFO);
            *took_action.get_or_insert_default() |= allowed;
            if allowed {
                ctx.spawn_alert(info.message.clone()[..].into(), Duration::from_secs(10));
            } else {
                log::info!("{blocked} info popup");
            }
        }
        if let InteractivePoi { copy: Some(copy), .. } = ipoi {
            let allowed = allowed.contains(TriggerKind::COPY);
            *took_action.get_or_insert_default() |= allowed;
            if allowed {
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
            let allowed = allowed.contains(TriggerKind::TOGGLE);
            *took_action.get_or_insert_default() |= allowed;
            if allowed {
                let cat_path = show_hide.category().pivot(loaded_path.root.root);
                // TODO: spawn instead to ensure it arrives?
                PathingEvent::CategorySetToggle(cat_path, show_hide.action.tristate()).try_send();
            } else {
                log::info!("{blocked} {}", show_hide.action);
            }
        }
        if let InteractivePoi { reset: Some(reset), .. } = ipoi {
            let allowed = allowed.contains(TriggerKind::RESET);
            *took_action.get_or_insert_default() |= allowed;
            if allowed {
                PathingEvent::GuidReset(reset.guid.iter().cloned().collect()).try_send();
            } else {
                log::info!("{blocked} reset");
            }
        }
        if let InteractivePoi { script: Some(..), .. } = ipoi {
            let allowed = allowed.contains(TriggerKind::SCRIPT);
            *took_action.get_or_insert_default() |= allowed;
            if allowed {
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

        if let Some(behaviour) = behaviour {
            let organic = match action.is_natural() {
                true => took_action.unwrap_or(true),
                false => true,
            };
            if allowed.contains(TriggerKind::BEHAVIOUR) && organic {
                const HOUR: Duration = Duration::from_secs(3600);
                const DAY: Duration = Duration::from_secs(HOUR.as_secs() * 24);
                const WEEK: Duration = Duration::from_secs(DAY.as_secs() * 7);
                const MANY_WEEKS: Duration = Duration::from_secs(WEEK.as_secs() * 52);

                use taimi_pack::attributes::keys::{Behaviour, TacoBehaviour, BlishBehaviour};
                let timestamp = rt::log::error_ok(UNIX_EPOCH.elapsed()).unwrap_or_default();
                let mut contexts = None;
                let mut reset = None;
                let delay = match behaviour.mode {
                    Behaviour::Taco(TacoBehaviour::ResetDaily) | Behaviour::Taco(TacoBehaviour::ResetDailyPerCharacter) => Some(Duration::from_secs({
                        if let Behaviour::Taco(TacoBehaviour::ResetDailyPerCharacter) = behaviour.mode {
                            contexts = Some(HideContext::for_character(self.filter_state.character.name.clone()));
                        }
                        const SOME_DAY: Duration = Duration::from_secs(1754265600 - MANY_WEEKS.as_secs() * 13);
                        (SOME_DAY.as_secs() as i64).wrapping_sub(timestamp.as_secs() as i64).wrapping_rem_euclid(DAY.as_secs() as i64)
                    } as u64)),
                    Behaviour::Blish(BlishBehaviour::ResetWeekly) => Some(Duration::from_secs({
                        const SOME_WEEK: Duration = Duration::from_secs(1754265600 - MANY_WEEKS.as_secs() * 13);
                        (SOME_WEEK.as_secs() as i64).wrapping_sub(timestamp.as_secs() as i64).wrapping_rem_euclid(WEEK.as_secs() as i64)
                    } as u64)),
                    Behaviour::Taco(TacoBehaviour::ResetDelay) => Some(behaviour.reset_delay.duration()),
                    Behaviour::Taco(TacoBehaviour::AlwaysVisible) => Some(Duration::from_secs(0)),
                    Behaviour::Taco(TacoBehaviour::ResetPermanent) => {
                        reset = Some(AutoReset::Never);
                        None
                    },
                    Behaviour::Taco(TacoBehaviour::ResetMap) => {
                        contexts = Some(HideContext::for_map(loaded_path.root.path, None));
                        reset = Some(AutoReset::MapChange);
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
                PathingEvent::DismissMarker(loaded_path.root.rel(path.path), delay, contexts, reset).try_send();
            } else {
                log::info!("{blocked} dismiss behaviour");
            }
        } else if action.is_natural() && took_action.unwrap_or(false) {
            let context = vec![HideContext::for_map(loaded_path.root.path, None)];
            PathingEvent::DismissMarker(loaded_path.root.rel(path.path), Some(Self::INTERACT_COOLDOWN), context, Some(AutoReset::Distance)).try_send();
        }
    }

    pub(super) fn handle_press_interact(&mut self, ctx: &mut PathingEventContext, map_id: MapIndex) {
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
        for cmp::Reverse((_distdist, _, _, (path, loaded_path, interactive_path))) in nearby_pois {
            let _ = ctx.interactions.send(InteractionEvent::Interact {
                action,
                path,
                loaded_path,
                interactive_path,
            });
        }
    }

    pub const UPDATE_INTERVAL_SLOW: Duration = Duration::from_secs(10);
    pub const UPDATE_INTERVAL_RESPONSIVE: Duration = Duration::from_millis(350);
    /// Don't bother re-scanning if player hasn't moved at least `sqrt(distance)` [metres](PackSpace)
    pub const UPDATE_DISTANCE_DISTANCE: f32 = 0.005;
    pub(super) async fn handle_update_tick(&mut self, ctx: &mut PathingEventContext, map_id: MapIndex) {
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
                nearby_changes.push((path, Arc::new(map.interactive_pois_nearby.clone()), updated));
            }
        }
        if !nearby_changes.is_empty() {
            let mut all_events = Vec::new();
            let mut dirty = false;
            ctx.shared.gameplay.send_if_modified(|shared_map| {
                for (path, nearby, events) in nearby_changes {
                    all_events.extend(events);
                    let Some(shared_state) = shared_map.get_state_mut(*path) else { continue };
                    shared_state.interactive_pois_nearby = nearby;
                    dirty |= true;
                }
                false
            });
            for e in all_events {
                ctx.interactions.send(e);
            }
            if dirty {
                ctx.shared.update_gameplay_notify(map_id);
            }
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InteractSettings {
    pub trigger_allow_interact: TriggerKind,
    pub trigger_allow_auto: TriggerKind,
}
impl InteractSettings {
    pub fn from_settings(settings: &PathingSettings) -> Self {
        let PathingSettings { trigger_allow_auto, trigger_allow_interact, .. } = *settings;
        Self {
            trigger_allow_auto,
            trigger_allow_interact,
        }
    }
}
impl Default for InteractSettings {
    fn default() -> Self {
        Self {
            trigger_allow_auto: TriggerKind::SETTINGS_DEFAULT_AUTO,
            trigger_allow_interact: TriggerKind::SETTINGS_DEFAULT_INTERACT,
        }
    }
}
