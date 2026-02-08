use {
    crate::{
        controller::{
            pathing::{
                info::MapPackInfo,
                registry::{LoadedMarkerPath, LoadedPoiPath, PackMapPath, PoiMapPath},
                shared::{
                    interact::{
                        empty_trigger_bvh,
                        NearbyMarkers,
                        PlayerPosition,
                        SharedTriggerBvh,
                        TRIGGER_DIMENSION,
                    },
                    InteractReceiver,
                    LocDisplay,
                    PathingEnables,
                    PathingReceiver,
                },
                state::{
                    filter::{self, FilterState, MarkerFilter},
                    hidden::{AutoReset, HideContext},
                    interactive::{InteractionEvent, InteractionEventAction},
                    LoadedMapInfo,
                    LoadedMapPack,
                    LoadedMaps,
                    LoadedPoi,
                },
                PathingController,
                PathingEvent,
            },
            runtime::WallInstant,
            Controller,
        },
        exports::runtime as rt,
        render::{RenderEvent, RenderState},
        settings::{
            pathing::{PathingSettings, TriggerKind},
            SettingsLock,
        },
        Interruption,
    },
    bvh::{aabb, bounding_hierarchy::BHShape, bvh::Bvh},
    futures::{
        future::{self, Either},
        stream::StreamExt,
    },
    glamour::{Contains, Point2, Point3},
    std::{
        cmp,
        collections::BinaryHeap,
        future::Future,
        num::NonZero,
        sync::Arc,
        task::{Context, Poll},
        time::Duration,
    },
    taimi_hoard::{cmp::CmpIgnore, flags::BitSet, loc::LocationRef, time::Timestamp},
    taimi_meta::{
        coords::LocalSpace,
        packs::{
            id::{MarkerId, MarkerIndex, MarkerPath},
            MapIndex,
            PoiPath,
        },
        spatial::{BvhShape, MintConv, TriggerBoundsInfo},
        ui::gameplay::GameplayState,
    },
    taimi_pack::attributes::{keys, AttrString, InteractionAttributes, MarkerAttributes},
    taimi_sync::arcs::ArcPtrCmp,
    tokio::sync::{broadcast, RwLock},
    tokio_stream::wrappers::errors::BroadcastStreamRecvError as BroadcastError,
};

#[derive(Debug, Clone)]
pub struct SpaceInteraction {
    #[cfg(todo = "unnecessary")]
    pub id: MarkerId,
    pub path: PoiMapPath,
    pub bounds: TriggerBoundsInfo,
    pub radius_radius: f32,
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
            radius_radius: radius * radius,
        }
    }
    pub fn poi_path(&self) -> PoiMapPath {
        self.path
    }
    pub fn is_auto(&self) -> bool {
        self.bounds.is_auto()
    }
    pub fn is_passive(&self, attrs: &InteractionAttributes) -> bool {
        self.is_auto() || attrs.info().is_some() || attrs.copy_value().is_some()
    }
    /// may display an unintrusive popup or notification, even if not allowed to auto-trigger
    pub const PASSIVE_NEARBY: TriggerKind =
        TriggerKind::from_bits_retain(TriggerKind::INFO.bits() | TriggerKind::COPY.bits());

    pub fn dist_dist(&self, point: &Point3<LocalSpace>) -> f32 {
        self.bounds.position.distance_squared(*point)
    }
    pub fn dist_dist2(&self, point: &Point2<LocalSpace>) -> f32 {
        self.bounds.position2().distance_squared(*point)
    }
    pub fn dist_dist_inrange(&self, point: &Point3<LocalSpace>) -> Option<f32> {
        let dist_dist = self.dist_dist(point);
        (dist_dist <= self.radius_radius).then_some(dist_dist)
    }
    pub fn point_query(pos: Point3<LocalSpace>) -> impl aabb::IntersectsAabb<f32, { TRIGGER_DIMENSION }> {
        let pos2 = LocalSpace::to2(pos);
        let point: nalgebra::Point<f32, { TRIGGER_DIMENSION }> = pos2.into_nalg();
        point
    }
    /// TODO: this probably obsoletes [Self::point_query_postprocess_filter],
    /// just set [TRIGGER_DIMENSION]=3 and use in place of [Self::point_query]
    ///
    /// also maybe `radius_radius` field won't need to exist?
    pub fn point_query3(pos: Point3<LocalSpace>) -> impl aabb::IntersectsAabb<f32, 3> {
        struct RoundPointQuery(Point3<LocalSpace>);
        impl aabb::IntersectsAabb<f32, 3> for RoundPointQuery {
            fn intersects_aabb(&self, aabb: &aabb::Aabb<f32, 3>) -> bool {
                const DIFF_EPSILON: f32 = 0.0001;
                let is_leaf = match aabb.size() {
                    sz => (sz.x - sz.y).abs() < DIFF_EPSILON,
                };
                match is_leaf {
                    true => {
                        let midpoint: Point3<LocalSpace> = MintConv::from_nalg(aabb.center());
                        let radius = (aabb.max.x - aabb.min.x) / 2.0;
                        midpoint.distance(self.0) <= radius
                    },
                    false => {
                        // branch nodes contain union of bounds, so radius would be incorrect?
                        aabb.contains(&self.0.into_nalg())
                    },
                }
            }
        }
        RoundPointQuery(pos)
    }
    #[inline]
    pub fn point_query_postprocess_filter(&self, point: Point3<LocalSpace>) -> bool {
        self.contains(&point)
    }

    pub fn poi_interest(poi: &LoadedPoi) -> (TriggerKind, bool) {
        let Some(attrs) = poi.get_interaction_attrs() else {
            return (TriggerKind::empty(), false)
        };

        let (mut interest, auto) = Self::interest_for(attrs);
        let script = poi.info().get_marker_attrs().map(|m| m.script.is_some());
        if script.unwrap_or(false) {
            interest.insert(TriggerKind::SCRIPT);
        }
        (interest, auto)
    }
    pub fn interest_for_marker(attrs: &MarkerAttributes) -> (TriggerKind, bool) {
        let (mut interest, auto) = match attrs.interaction.as_ref() {
            Some(i) => Self::interest_for(i),
            None => (TriggerKind::empty(), false),
        };
        if attrs.script.is_some() {
            interest.insert(TriggerKind::SCRIPT);
        }
        (interest, auto)
    }
    pub fn interest_for(attrs: &InteractionAttributes) -> (TriggerKind, bool) {
        let mut interest = TriggerKind::empty();
        if attrs.copy_value().is_some() {
            interest.insert(TriggerKind::COPY);
        }
        if attrs.info().is_some() {
            interest.insert(TriggerKind::INFO);
        }
        if !attrs.reset_guids().is_empty() {
            interest.insert(TriggerKind::RESET);
        }
        if attrs.taco_behavior.is_some() {
            interest.insert(TriggerKind::BEHAVIOUR);
        }
        if attrs.bounce_behavior.is_some() {
            interest.insert(TriggerKind::BOUNCE);
        }
        #[cfg(todo)]
        if !lpoi.guid.is_nil() {
            interest.insert(TriggerKind::BEHAVIOUR);
        }
        if attrs.show_category.is_some() {
            interest.insert(TriggerKind::SHOW);
        }
        if attrs.hide_category.is_some() {
            interest.insert(TriggerKind::HIDE);
        }
        if attrs.toggle_category.is_some() {
            interest.insert(TriggerKind::TOGGLE);
        }
        (interest, attrs.auto_trigger())
    }
    pub fn is_interactive(poi: &LoadedPoi) -> bool {
        !poi.get_interaction_attrs()
            .map(|i| Self::interaction_is_empty(i))
            .unwrap_or(true)
    }
    pub fn interaction_is(i: &InteractionAttributes, mask: TriggerKind) -> bool {
        Self::interest_for(i).0.intersects(mask)
    }
    fn interaction_is_empty(i: &InteractionAttributes) -> bool {
        Self::interaction_is(i, TriggerKind::all())
    }
    /// short-circuiting version, do we care?
    #[cfg(todo)]
    fn interaction_is_empty(i: &InteractionAttributes) -> bool {
        if i.info.is_some() {
            return false
        }
        if i.bounce_behavior.is_some() {
            return false
        }
        if i.copy_value.is_some() {
            return false
        }
        if i.reset_guids.is_some() {
            return false
        }
        if i.toggle_category.is_some() {
            return false
        }
        if i.show_category.is_some() {
            return false
        }
        if i.hide_category.is_some() {
            return false
        }
        if i.taco_behavior.is_some() {
            return false
        }
        #[cfg(todo)]
        if i.auto_trigger() {
            return false
        }

        true
    }
}
impl<const D: usize> aabb::Bounded<f32, D> for SpaceInteraction
where
    TriggerBoundsInfo: aabb::Bounded<f32, D>,
{
    fn aabb(&self) -> aabb::Aabb<f32, D> {
        self.bounds.aabb()
    }
}
/// Since the aabb is a box and not a sphere, an additional check is required
impl Contains<Point3<LocalSpace>> for SpaceInteraction {
    fn contains(&self, point: &Point3<LocalSpace>) -> bool {
        matches!(self.dist_dist_inrange(point), Some(..))
    }
}
/// Since the aabb is a box and not a sphere, an additional check is required
impl Contains<Point2<LocalSpace>> for SpaceInteraction {
    fn contains(&self, point: &Point2<LocalSpace>) -> bool {
        self.dist_dist2(point) <= self.radius_radius
    }
}

#[derive(Debug, Clone)]
pub struct MapInteractState {
    entities: Vec<BvhShape<SpaceInteraction>>,
    /// TODO: moving this whole struct into the arc may work better idk
    trigger_bvh: SharedTriggerBvh,
    trigger_bvh_dirty: bool,
    nearby: BitSet,
    interest_auto: TriggerKind,
    interest_nearby: TriggerKind,
}
impl MapInteractState {
    pub fn new() -> Self {
        Self {
            entities: Default::default(),
            trigger_bvh: Arc::new(RwLock::new(Bvh { nodes: Vec::new() })),
            trigger_bvh_dirty: false,
            nearby: Default::default(),
            interest_auto: TriggerKind::empty(),
            interest_nearby: TriggerKind::empty(),
        }
    }
    pub fn map_id(&self) -> Option<MapIndex> {
        // XXX: may track multiple maps in future, but they should be split across instances right?
        self.entities.first().map(|e| e.path.root.path)
    }

    /// TODO: partial updates and whatnot
    pub fn update_entities(&mut self, maps: &LoadedMaps, map_info: &LoadedMapInfo, map_id: MapIndex) {
        self.clear_active();

        let mut interest_auto = self.interest_auto;
        let mut interest_nearby = self.interest_nearby;
        for (map_path, map, _map_info) in maps.iter_with_info(map_info, Some(map_id)) {
            let pois = map.lpois().iter().filter_map(|(lpath, poi)| {
                let (interest, auto) = SpaceInteraction::poi_interest(poi);
                interest_nearby.insert(interest);
                if auto {
                    interest_auto.insert(interest & TriggerKind::AUTO_TRIGGER_MASK);
                }
                match interest.is_empty() {
                    true => None,
                    false => Some(BvhShape::new(SpaceInteraction::with_poi(
                        map_path.rel(lpath.path),
                        poi,
                    ))),
                }
            });
            self.entities.extend(pois);
        }
        self.trigger_bvh_dirty = !self.entities.is_empty();
        self.interest_auto = interest_auto;
        self.interest_nearby = interest_nearby;
    }
    /// TODO: rebuild with executor
    ///
    /// TODO: waiting on mutex is unnecessary if you just replace it with a new one if a try_lock fails...
    pub async fn rebuild_trigger_bvh(&mut self) {
        #[cfg(todo = "unnecessary")]
        if !self.trigger_bvh_dirty {
            return
        }
        #[cfg(todo = "unnecessary")]
        if self.entities.is_empty() {
            self.trigger_bvh = empty_trigger_bvh().clone();
            self.trigger_bvh_dirty = false;
            return
        }

        self.ensure_bvh_rw();

        let mut shapes = self.entities.clone();
        let res = Controller::try_run_blocking("POI interaction hierarchy", {
            let mut trigger_bvh = self.trigger_bvh.clone().write_owned().await;
            move || {
                *trigger_bvh = Bvh::build(&mut shapes[..]);
                trigger_bvh.nodes.shrink_to_fit();
                Ok(shapes)
            }
        })
        .await;
        match rt::log::error_ok(res) {
            None => {
                // what do now..?
                self.trigger_bvh = empty_trigger_bvh().clone();
            },
            Some(shapes) =>
                for (dest, shape) in self.entities.iter_mut().zip(&shapes) {
                    let bh_index = BHShape::<f32, { TRIGGER_DIMENSION }>::bh_node_index(shape);
                    BHShape::<f32, { TRIGGER_DIMENSION }>::set_bh_node_index(dest, bh_index);
                },
        };

        self.trigger_bvh_dirty = false;
    }
    pub fn clear_active(&mut self) {
        self.entities.clear();
        self.nearby.clear();
        self.interest_auto = TriggerKind::empty();
        self.interest_nearby = TriggerKind::empty();
        let empty_bvh_rw = empty_trigger_bvh();
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
        self.trigger_bvh = empty_trigger_bvh().clone();
        self.trigger_bvh_dirty = false;
    }

    fn ensure_bvh_rw(&mut self) {
        if Arc::ptr_eq(&self.trigger_bvh, empty_trigger_bvh()) {
            self.trigger_bvh = Arc::new(RwLock::new(Bvh { nodes: Vec::new() }));
        }
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
#[derive(Debug, Clone)]
pub struct InteractReactor {
    pub map_interactions: MapInteractState,
    pub config: InteractSettings,
    pub enables: PathingEnables,
    /// TODO: passive interval as a multiple of this seems dumb
    pub update_interval: Duration,
    event_dirty_bvh_rebuild: bool,
    event_dirty_settings: bool,
    event_dirty_entities: bool,
}
impl InteractReactor {
    pub fn new() -> Self {
        Self {
            map_interactions: Default::default(),
            config: Default::default(),
            enables: Default::default(),
            update_interval: Self::UPDATE_INTERVAL_RESPONSIVE,
            event_dirty_bvh_rebuild: false,
            event_dirty_settings: false,
            event_dirty_entities: false,
        }
    }
    pub(super) fn handle_map_enter(
        &mut self,
        rx: &mut PathingReceiver,
        _maps: &LoadedMaps,
        _map_info: &LoadedMapInfo,
        _map_id: MapIndex,
    ) {
        if rx.interact.player_pos.ml.is_none() {
            // some initial setup...
            rx.interact.player_pos.set_ml(rt::mumble_link_ptr().ok());
            self.event_dirty_settings = true;
        }
        self.event_dirty_entities = true;
    }
    pub(super) fn handle_map_leave(&mut self, rx: &mut PathingReceiver) {
        self.map_interactions.clear_active();
        rx.interact.player_pos.reset();
        rx.interact.nearby_tx.send_if_modified(|nearby| {
            let dirty = !nearby.is_empty();
            nearby.clear();
            dirty
        });
        rx.interact.entities_tx.send_if_modified(|entities| {
            let dirty = !entities.is_empty();
            entities.clear();
            dirty
        });
    }
    pub(super) fn handle_map_suspend(&mut self, rx: &mut PathingReceiver, gameplay: &GameplayState) {
        let (reentering_urgent, next_map_id, prev_map_id) = match *gameplay {
            GameplayState::Intermission { next_map_id, prev_map_id, .. } =>
                (false, next_map_id, prev_map_id),
            GameplayState::Gameplay { map_id } => (true, map_id, None),
        };
        let maybe_cinematic = !reentering_urgent && next_map_id.is_some() && next_map_id == prev_map_id;
        if !maybe_cinematic {
            rx.interact.nearby_tx.send_if_modified(|nearby| {
                let dirty = !nearby.is_empty();
                nearby.clear();
                dirty
            });
        }
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
                events.push(PathingEvent::CategoryEnableById(
                    loaded_path.root.root,
                    cat,
                    show_hide.tristate(),
                ));
            } else {
                log::info!("{blocked} {}", show_hide);
            }
        }
        if let reset @ &[_, ..] = attrs.reset_guids() {
            let allowed = allowed.contains(TriggerKind::RESET);
            *took_action.get_or_insert_default() |= allowed;
            if allowed {
                let ids = reset
                    .iter()
                    .map(|guid| MarkerId::from_uuid_ref(guid.as_ref()).clone())
                    .collect();
                events.push(PathingEvent::ResetMarkerIds(ids));
            } else {
                log::info!("{blocked} reset");
            }
        }
        let script = lpoi.info().get_marker_attrs().and_then(|m| m.script.as_ref());
        if let Some(_script) = script {
            let allowed = allowed.contains(TriggerKind::SCRIPT);
            *took_action.get_or_insert_default() |= allowed;
            if allowed {
                log::debug!("TODO: interact script");
            } else {
                log::info!("{blocked} script");
            }
        }
        if let Some(_bounce) = attrs.bounce_behavior {
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

                use taimi_pack::attributes::keys::{Behaviour, BlishBehaviour, TacoBehaviour};
                let mut now = None;
                let mut now = || *now.get_or_insert_with(WallInstant::now_timestamp_system_checked);
                let mut contexts = None;
                let mut reset = None;
                /// 16:00 UTC
                const SOME_DAY: Timestamp =
                    Timestamp::with_timestamp(1754265600 - MANY_WEEKS.as_secs() * 13);
                let until = match behaviour {
                    Behaviour::Taco(TacoBehaviour::ResetDaily)
                    | Behaviour::Taco(TacoBehaviour::ResetDailyPerCharacter) => {
                        if let Behaviour::Taco(TacoBehaviour::ResetDailyPerCharacter) = behaviour {
                            contexts =
                                Some(HideContext::for_character(filter_state.character.name.clone()));
                        }
                        match now().timestamp() {
                            #[cfg(todo)]
                            now => Some(Either::Left(Timestamp::with_timestamp(
                                now.next_multiple_of(Timestamp::DAY.as_secs()),
                            ))),
                            now => Some(Either::Right(Duration::from_secs({
                                let delta = (SOME_DAY.timestamp() as i64).wrapping_sub(now as i64);
                                let rem = delta.wrapping_rem_euclid(Timestamp::DAY.as_secs() as i64);
                                rem as u64
                            }))),
                        }
                    },
                    Behaviour::Blish(BlishBehaviour::ResetWeekly) => {
                        /// sunday 23:30 UTC
                        const SOME_WEEK: Timestamp = Timestamp::with_timestamp(
                            SOME_DAY.timestamp() + Timestamp::HOUR.as_secs() * 8
                                - Timestamp::MINUTE.as_secs() * 30,
                        );
                        match now().timestamp() {
                            now => Some(Either::Right(Duration::from_secs({
                                let delta = (SOME_WEEK.timestamp() as i64).wrapping_sub(now as i64);
                                let rem = delta.wrapping_rem_euclid(Timestamp::WEEK.as_secs() as i64);
                                rem as u64
                            }))),
                        }
                    },
                    Behaviour::Taco(TacoBehaviour::ResetDelay) =>
                        Some(Either::Right(keys::ResetLength(reset_delay).duration())),
                    Behaviour::Taco(TacoBehaviour::AlwaysVisible) =>
                        Some(Either::Right(Duration::from_secs(0))),
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
                        contexts = Some(HideContext::for_map(
                            loaded_path.root.path,
                            NonZero::new(filter_state.map.shard_id),
                        ));
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
        filter_state: &FilterState,
        (loaded_path, lpoi): (LoadedPoiPath<PackMapPath>, &LoadedPoi),
        (path, guid): (PoiPath, Option<&keys::Guid>),
        action: &InteractionEventAction,
    ) -> TriggerKind {
        let is_filtered = || {
            if !lpoi.visibility.is_visible() {
                return true
            }
            let lmarker_path = loaded_path.map_path(MarkerIndex::with_poi);
            let marker_path = loaded_path.root.root.rel(MarkerIndex::from(path));
            let ids = guid
                .map(|guid| MarkerId::from_uuid_ref(guid.as_ref()).clone())
                .into_iter()
                .chain([
                    MarkerId::for_marker(lmarker_path),
                    MarkerId::for_marker(marker_path),
                ]);
            for id in ids {
                let filter = filter::GroupConfig {
                    guid: keys::Guid::from(id.uuid),
                    inverted: lpoi.info().filter_attrs().invert_behavior(),
                };
                if let filter::FILTER_HIDDEN = filter.is_visible(filter_state) {
                    return true
                }
            }
            log::debug!("TODO: POI autoreset filter");
            false
        };
        match action {
            InteractionEventAction::Trigger => TriggerKind::all(),
            InteractionEventAction::Dismiss(..) => TriggerKind::DISMISS,
            InteractionEventAction::Manual(mask) => *mask,
            action if action.is_natural() && is_filtered() => {
                let display = match lpoi {
                    #[cfg(todo)]
                    _ => LocDisplay((loaded_path, lpoi)),
                    #[cfg(todo)]
                    _ => LocDisplay({
                        let lpoi_path: LoadedPoiPath = loaded_path.unscope();
                        loaded_path.root.rel(lpoi_path)
                    }),
                    _ => loaded_path,
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
        PoiPath,
        LoadedPoiPath<PackMapPath>,
        &'a LoadedPoi,
        Option<&'a keys::Guid>,
        InteractionEventAction,
    )> {
        match event {
            &InteractionEvent::Nearby { path, loaded_path } => {
                let (map, map_info) = maps.lookup_with_info(map_info, &loaded_path.root)?;
                let lpath: LoadedPoiPath = loaded_path.unscope();
                let lpoi = map.lpois().lookup_ref(&lpath)?;
                let guid = map.poi_guid_by_index(map_info, lpath);
                let attrs = lpoi.interaction_attrs();
                let act_auto = match self.config.trigger_allow_auto {
                    _ if !attrs.auto_trigger() => false,
                    allow if allow.is_empty() => false,
                    allow => SpaceInteraction::interest_for(attrs).0.intersects(allow),
                };
                let action = if act_auto { Some(InteractionEventAction::AutoTrigger) } else { None };
                action.map(|action| (path, loaded_path, lpoi, guid, action))
            },
            &InteractionEvent::Interact { ref action, path, loaded_path } => {
                let (map, map_info) = maps.lookup_with_info(map_info, &loaded_path.root)?;
                let lpath: LoadedPoiPath = loaded_path.unscope();
                let guid = map.poi_guid_by_index(map_info, lpath);
                let lpoi = map.lpois().lookup_ref(&lpath)?;
                Some((path, loaded_path, lpoi, guid, action.clone()))
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
            InteractionEvent::Gone { path, loaded_path } =>
                self.process_gone(filter_state, map_info, maps, (path, loaded_path)),
            ref event @ (InteractionEvent::Nearby { .. } | InteractionEvent::Interact { .. }) => {
                let Some((path, lpath, lpoi, guid, action)) = self.prepare_action(&event, map_info, maps)
                else {
                    return PathingEvent::Nop
                };
                let allowed = self.allow_action(filter_state, (lpath, lpoi), (path, guid), &action);
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
        let guid = map.poi_guid_by_index(map_info, lpath.unscope()).cloned();
        let mut msg = if self.handle_interaction_end(filter_state, &MarkerId::for_marker(marker_path)) {
            PathingEvent::ResetMarkerPath(marker_path)
        } else {
            PathingEvent::Nop
        };
        if let Some(guid) = guid {
            // TODO: does ResetMarkerPath already fan this out and make this unnecessary? could be an or...
            if self.handle_interaction_end(filter_state, guid.0.as_ref()) {
                msg.push(PathingEvent::ResetMarkerIds(vec![MarkerId::with_uuid(
                    guid.into(),
                )]));
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
            AutoReset::Never | AutoReset::Expiry { .. } | AutoReset::MapChange => return false,
        }

        true
    }
    pub const UPDATE_INTERVAL_SLOW: Duration = Duration::from_secs(10);
    pub const UPDATE_INTERVAL_PASSIVE: Duration = Duration::from_secs(1);
    pub const UPDATE_INTERVAL_RESPONSIVE: Duration = Duration::from_millis(350);
    /// TODO: measure impact and use spawn_blocking if it gets unreasonable?
    /// also make interval configurable and/or adaptive
    async fn process_movement<'a>(
        &mut self,
        rx: &mut InteractReceiver,
        map_info: &'_ LoadedMapInfo,
        maps: &'a LoadedMaps,
        pos: PlayerPosition,
    ) -> PathingEvent {
        let Some(map_id) = self.map_interactions.map_id() else {
            return PathingEvent::Nop
        };
        let nearby = rx.nearby_tx.borrow().clone();

        let trigger_bvh = self.map_interactions.trigger_bvh.read().await;
        let query = SpaceInteraction::point_query(pos);
        let mut outgoing = nearby;
        #[cfg(todo)]
        if outgoing.map_id != Some(map_id) {
            // do we need to care to emit events for each marker when leaving a map?
        }
        outgoing.set_map_id(Some(map_id));
        let mut incoming = NearbyMarkers::new_on_map(map_id);
        let inrange = trigger_bvh.traverse_iterator(&query, &self.map_interactions.entities[..]);
        for entity in inrange {
            let id = entity.value.poi_path();
            let is_nearby = entity.value.point_query_postprocess_filter(pos);
            if is_nearby {
                if outgoing.remove_poi(id).is_none() {
                    if let Some((lpath, path)) = Self::lookup_lpoi_path(map_info, id) {
                        incoming.insert_poi(lpath, path);
                    } else {
                        let path = id.map_root(LocDisplay);
                        log::debug!("approached {path} which doesn't exist?");
                    }
                }
            }
        }
        drop(trigger_bvh);
        let has_changes = !outgoing.is_empty() || !incoming.is_empty();
        // anything left in outgoing is no longer in range...
        let gone = outgoing
            .iter_pois()
            //.filter_map(|lpath| Self::lookup_lpoi_path(map_info, lpath))
            .map(|(loaded_path, path)| InteractionEvent::Gone { path, loaded_path });
        let mut incoming_move = incoming.clone();
        let incoming = incoming
            .iter_pois()
            .filter(|(lpath, _)| {
                Self::lookup_lpoi_at(map_info, maps, *lpath)
                    .map(|(lpoi, ..)| lpoi.visibility.is_visible())
                    .unwrap_or(false)
            })
            .map(|(loaded_path, path)| InteractionEvent::Nearby { path, loaded_path });
        let nearby_changes = gone.chain(incoming);
        if has_changes {
            rx.nearby_tx.send_if_modified(|nearby| {
                nearby.set_map_id(Some(map_id));
                nearby.remove_pois_sorted(outgoing.iter_poi_lpaths());
                nearby.append_take_from(&mut incoming_move);
                true
            });
            for e in nearby_changes {
                let _ = rx.event_tx.send(e);
            }
        }
        PathingEvent::Nop
    }
    fn lookup_lpoi_path(map_info: &LoadedMapInfo, lpath: PoiMapPath) -> Option<(PoiMapPath, PoiPath)> {
        map_info
            .lookup_ref(&lpath.root)
            .and_then(|i| i.poi_path(lpath.unscope()).map(|path| (lpath, path)))
    }
    fn lookup_lpoi_at<'a, 'i>(
        map_info: &'i LoadedMapInfo,
        maps: &'a LoadedMaps,
        lpath: PoiMapPath,
    ) -> Option<(&'a LoadedPoi, &'a LoadedMapPack, &'i Arc<MapPackInfo>)> {
        maps.lookup_with_info(map_info, &lpath.root).and_then(|(m, i)| {
            let li: LoadedPoiPath = lpath.unscope();
            m.lpois().lookup_ref(&li).map(|lpoi| (lpoi, m, i))
        })
    }
    pub(super) async fn trigger_interact_action(
        &mut self,
        rx: &mut InteractReceiver,
        (map_info, maps, _map_id, filter_state): (&LoadedMapInfo, &LoadedMaps, MapIndex, &FilterState),
        action: InteractionEventAction,
    ) -> PathingEvent {
        let mut res = PathingEvent::Nop;
        let Some(pos) = rx.player_pos.update_now(false) else { return res };
        let nearby = rx.nearby_tx.borrow().clone();
        let mut nearby_pois = BinaryHeap::with_capacity(nearby.len() * 4);

        let trigger_bvh = self.map_interactions.trigger_bvh.read().await;
        let query = SpaceInteraction::point_query(pos);
        let inrange = trigger_bvh
            .traverse_iterator(&query, &self.map_interactions.entities[..])
            .filter_map(|e| e.value.dist_dist_inrange(&pos).map(|dist| (e, dist)));
        for (entity, dist_dist) in inrange {
            let id = entity.value.poi_path();
            let Some((lpoi, map, map_info)) = Self::lookup_lpoi_at(map_info, maps, id) else {
                continue
            };
            if !lpoi.visibility.is_visible() {
                continue
            }
            let nearby_discrete = (dist_dist * 1_000_000.0).min(0x40000000u32 as f32) as u32;
            let prev_nearby = nearby.contains_loaded_poi(id);
            let auto_trigger = entity.value.bounds.is_auto();
            let attrs = lpoi.interaction_attrs();
            let auto_triggered = entity.value.is_passive(attrs) && prev_nearby;
            let (interest, _interest_auto) = SpaceInteraction::interest_for(attrs);
            if _interest_auto != auto_trigger {
                // TODO: deleteme and/or cfg(debug_assertions)
                log::debug!("BUG: auto-trigger mismatch for {id}");
            }
            let interest_consumed = match prev_nearby {
                true if auto_trigger => interest & self.config.trigger_allow_auto,
                _ => TriggerKind::empty(),
            };
            let interest_avail = interest & !interest_consumed;
            let interest_boring = TriggerKind::SCRIPT | TriggerKind::BOUNCE;
            let boring = interest_avail & interest_boring == interest_boring;
            let interesting = !(interest_avail & self.config.trigger_allow_interact).is_empty();
            let sort_id = (
                !interesting,
                boring,
                nearby_discrete,
                auto_trigger,
                auto_triggered,
            );
            let data = CmpIgnore((id, lpoi, map, map_info, interest_avail));
            nearby_pois.push(cmp::Reverse((sort_id, data)));
        }
        drop(trigger_bvh);
        if nearby_pois.is_empty() {
            // TODO: fall back to non-interactive pois in case user is trying to dismiss or get info about a marker?
            // (maybe on a different keybind though?)
            return res
        }
        for cmp::Reverse((_sort, CmpIgnore((lpath, lpoi, map, map_info, _interest_avail)))) in nearby_pois {
            let Some(path) = map_info.poi_path(lpath.unscope()) else {
                log::warn!("{lpath} unknown?");
                continue
            };
            let guid = map.poi_guid_by_index(map_info, lpath.unscope());
            #[cfg(todo = "unnecessary")]
            let _ = rx.event_tx
                .send(InteractionEvent::Interact { action, path, loaded_path });
            let allowed = self.allow_action(filter_state, (lpath, lpoi), (path, guid), &action);
            if !allowed.is_empty() {
                let interact = self.process_interaction(filter_state, (path, lpath, lpoi), action, allowed);
                if !matches!((&res, &interact), (PathingEvent::Nop, PathingEvent::Nop)) {
                    log::debug!("TODO: activating multiple POIs, should've stopped at the closest?");
                }
                match interact {
                    e if e.is_empty() => continue,
                    e => res.push(e),
                };
                #[cfg(todo)]
                {
                    break
                }
            }
        }
        res
    }
    pub(super) const INTERACT_ACTION: InteractionEventAction = InteractionEventAction::Interact;

    fn poll_event_flag(&mut self, _cx: &mut Context) -> Poll<InteractMessage> {
        if self.event_dirty_settings {
            self.event_dirty_settings = false;
            return Poll::Ready(InteractMessage::RefreshSettings)
        }
        if self.event_dirty_entities {
            self.event_dirty_entities = false;
            return Poll::Ready(InteractMessage::UpdateEntities)
        }
        if self.event_dirty_bvh_rebuild {
            self.event_dirty_bvh_rebuild = false;
            return Poll::Ready(InteractMessage::BvhRebuild)
        }

        Poll::Pending
    }
    pub(super) fn poll_event(
        &mut self,
        cx: &mut Context,
        rx: &mut InteractReceiver,
    ) -> Poll<InteractMessage> {
        if let Poll::Ready(m) = self.poll_event_flag(cx) {
            return Poll::Ready(m)
        }
        for _ in 0..Self::EVENT_RX_RETRY {
            match rx.event_rx.poll_next_unpin(cx) {
                Poll::Ready(Some(Ok(e))) => return Poll::Ready(InteractMessage::Event(e)),
                Poll::Ready(Some(Err(BroadcastError::Lagged(amt)))) => {
                    log::warn!("lagged behind by {amt} interactions");
                    // TODO: clear out queue if this recurs?
                },
                Poll::Ready(None) | Poll::Pending => break,
            }
        }
        if let Some((auto, passive)) = self.interest_movement() {
            rx.player_pos.set_threshold_timeout(match (auto, passive) {
                (true, _) => self.update_interval,
                (false, true) => Self::UPDATE_INTERVAL_PASSIVE.max(self.update_interval * 2),
                (false, false) => Self::UPDATE_INTERVAL_SLOW,
            });
            if let Poll::Ready(pos) = rx.player_pos.poll_next_update(cx) {
                return Poll::Ready(InteractMessage::PlayerMoved(pos))
            }
        }
        Poll::Pending
    }
    fn get_interest_movement(&self) -> Option<(bool, bool)> {
        let has_pois = self.map_interactions.interest_nearby & self.config.trigger_allow_interact;
        let needs_auto = self
            .map_interactions
            .interest_auto
            .intersects(self.config.trigger_allow_auto);
        let passive_monitor = has_pois.intersects(SpaceInteraction::PASSIVE_NEARBY);
        (!has_pois.is_empty()).then_some((needs_auto, passive_monitor))
    }
    /// whether we need to check frequently for nearby auto-trigger POIs
    pub fn interest_movement(&self) -> Option<(bool, bool)> {
        let (auto, passive) = self.get_interest_movement()?;
        if !self
            .enables
            .contains(PathingEnables::KATRENDER | PathingEnables::ENGINE)
        {
            Some((false, passive))
        } else {
            Some((auto, passive))
        }
    }
    pub(super) fn with_rx<'a>(
        &'a mut self,
        rx: &'a mut InteractReceiver,
    ) -> impl Future<Output = InteractMessage> + 'a {
        future::poll_fn(move |cx| self.poll_event(cx, rx))
    }
    const EVENT_RX_RETRY: usize = 2;
    pub(super) async fn process_event(
        &mut self,
        rx: &mut PathingReceiver,
        (maps, map_info, filter_state, settings): (
            &LoadedMaps,
            &LoadedMapInfo,
            &FilterState,
            &SettingsLock,
        ),
        msg: InteractMessage,
    ) -> PathingEvent {
        match msg {
            InteractMessage::Nop => PathingEvent::Nop,
            InteractMessage::RefreshSettings => {
                self.reload_config(rx, settings).await;
                PathingEvent::Nop
            },
            InteractMessage::UpdateEntities => {
                let map_id = rx.gameplay.cached.as_ref().and_then(GameplayState::gameplay_map);
                if let Some(map_id) = map_id {
                    self.map_interactions.update_entities(maps, map_info, map_id);
                    if self.map_interactions.trigger_bvh_dirty {
                        self.event_dirty_bvh_rebuild = true;
                    }
                } else {
                    self.map_interactions.clear_active();
                }
                PathingEvent::Nop
            },
            InteractMessage::BvhRebuild => {
                if self.map_interactions.trigger_bvh_dirty {
                    // TODO: spawn/bg this thanks
                    self.map_interactions.rebuild_trigger_bvh().await;
                    rx.interact.entities_tx.send_if_modified(|shared| {
                        let _ptr_dirty = ArcPtrCmp::from_mut(&mut shared.trigger_bvh)
                            .clone_from_arc(&self.map_interactions.trigger_bvh);
                        shared.entities.clone_from(&self.map_interactions.entities);
                        // XXX: notify anyway in case watcher is used to cache data? idk
                        true
                    });
                }
                PathingEvent::Nop
            },
            InteractMessage::Event(event) => self.process_interact(filter_state, map_info, maps, event),
            InteractMessage::PlayerMoved(pos) => {
                let res = self.process_movement(&mut rx.interact, map_info, maps, pos).await;
                rx.interact.player_pos.readjust_now();
                res
            },
            InteractMessage::FanOut(mut events) => {
                let mut out = PathingEvent::Nop;
                while let Some(event) = events.pop() {
                    match event {
                        InteractMessage::FanOut(mut more) => {
                            events.append(&mut more);
                        },
                        m => {
                            let process =
                                self.process_event(rx, (maps, map_info, filter_state, settings), m);
                            let process = Box::pin(process).await;
                            out.push(process);
                        },
                    }
                }
                out
            },
            InteractMessage::RequestRebuild => {
                self.event_dirty_entities = true;
                PathingEvent::Nop
            },
        }
    }
    pub(super) async fn reload_config(&mut self, rx: &mut PathingReceiver, settings: &SettingsLock) {
        let settings = settings.read().await;
        let pathing = settings.pathing();

        self.config = InteractSettings::from_settings(&pathing);
        self.enables = rx.enables();
    }
    pub(super) async fn collect_garbage(
        &mut self,
        _rx: &mut PathingReceiver,
        (_map_info, _maps): (&LoadedMapInfo, &LoadedMaps),
        map_id: Option<MapIndex>,
        aggressive: bool,
    ) {
        let interact_map_id = self.map_interactions.map_id();
        let map_dirty = interact_map_id != map_id;
        match (map_dirty, aggressive) {
            (false, false) => (),
            (true, false) => self.map_interactions.clear_active(),
            (false, true) if interact_map_id.is_some() => {
                self.map_interactions.shrink_to_fit();
            },
            (_, true) => self.map_interactions.clear(),
        }
    }
    pub(super) fn exit(&mut self, rx: &mut InteractReceiver, _reason: Interruption) {
        rx.event_rx = broadcast::Sender::new(1).subscribe().into();
        self.map_interactions.clear();
    }
}
impl Default for InteractReactor {
    fn default() -> Self {
        Self::new()
    }
}
#[derive(Debug, strum::Display)]
pub enum InteractMessage {
    Nop,
    BvhRebuild,
    UpdateEntities,
    RefreshSettings,
    RequestRebuild,
    Event(InteractionEvent),
    PlayerMoved(PlayerPosition),
    FanOut(Vec<Self>),
}
impl InteractMessage {
    pub fn join(self, e: Self) -> Self {
        Self::FanOut(match (self, e) {
            (Self::Nop, e) | (e, Self::Nop) => return e,
            (Self::FanOut(mut events), e) => {
                match e {
                    Self::FanOut(e) => events.extend(e),
                    e => events.push(e),
                }
                events
            },
            (e, Self::FanOut(mut trailing)) => {
                trailing.insert(0, e);
                trailing
            },
            (e0, e1) => vec![e0, e1],
        })
    }

    pub fn try_send(self) {
        PathingEvent::InteractControl(self).try_send()
    }
}

impl PathingController {
    /// TODO
    pub(super) fn can_interact(interaction: &InteractionAttributes) -> bool {
        // if they exist, sure
        true
    }

    pub(super) async fn process_marker_copy(
        &mut self,
        _path: MarkerPath,
        _loaded_path: LoadedMarkerPath<PackMapPath>,
        value: AttrString,
        message: Option<AttrString>,
    ) {
        use {crate::with_i18n, taimi_hoard::lazyfmt};

        let value = &value[..];
        RenderState::try_send(RenderEvent::SendToClipboard(value.into()));
        let message = message.as_ref().map(|m| &m[..]);
        let alert = lazyfmt::fmt_or(
            message,
            lazyfmt::MaybeFmt::new(|f| with_i18n!("copied", |copied| f.write_str(&copied))),
        );
        let alert = format!("{alert}\n\n\"{value}\"");
        let delay = match message.is_some() {
            true => Duration::from_secs(6),
            false => Duration::from_secs(4),
        };
        self.spawn_alert(alert, delay);
    }
    pub(super) async fn process_marker_info(
        &mut self,
        path: MarkerPath,
        loaded_path: LoadedMarkerPath<PackMapPath>,
        message: AttrString,
    ) {
        log::debug!("TODO: marker info {}", &message[..]);
        self.spawn_alert(message[..].into(), Duration::from_secs(7));
    }
    pub(super) fn process_marker_dismiss(
        &mut self,
        _path: MarkerPath,
        loaded_path: LoadedMarkerPath<PackMapPath>,
        until: Option<Either<Timestamp, Duration>>,
        contexts: Vec<HideContext>,
        reset: Option<AutoReset>,
    ) {
        if let MarkerIndex::NS_POI = loaded_path.path.namespace() {
            let lpoi_path: LoadedPoiPath = LoadedPoiPath::with_path(loaded_path.path.index_poi_unchecked());
            self.filter_dismiss_poi(lpoi_path.pivot(loaded_path.root), until, contexts, reset);
        } else {
            log::debug!("TODO: marker dismiss {}", LocDisplay(loaded_path));
        }
    }
}
impl PathingController {
    /// TODO: deleteme
    pub fn spawn_alert(&mut self, message: String, duration: Duration) {
        use tokio::time::sleep;
        static PATHING_ALERT_HACK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

        log::debug!("TODO: replace alert system lol");
        self.tasks.spawn(async move {
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
                }
                .into(),
                message: message.into(),
            };
            let timer = alert.timer.clone();
            crate::render::RenderState::try_send(crate::render::RenderEvent::AlertStart(alert));
            sleep(duration).await;
            crate::render::RenderState::try_send(crate::render::RenderEvent::AlertEnd(timer));
            drop(lock);
            Ok(PathingEvent::Nop)
        });
    }
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InteractSettings {
    pub trigger_allow_interact: TriggerKind,
    pub trigger_allow_auto: TriggerKind,
}
impl InteractSettings {
    pub fn from_settings(settings: &PathingSettings) -> Self {
        let PathingSettings {
            trigger_allow_auto,
            trigger_allow_interact,
            ..
        } = *settings;
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
