use {
    crate::{
        controller::{
            pathing::{
                info::MapPackInfo,
                registry::{LoadedMarkerPath, LoadedPoiPath, PackPath, PackMapPath, PoiMapPath},
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
        render::{message_window::{MessageItemDesc, MessageActionDesc}, RenderEvent, RenderState},
        settings::{
            pathing::{PathingSettings, TriggerKind},
            SettingsLock,
        },
        Interruption,
        fl, with_i18n,
    },
    bvh::{aabb, bounding_hierarchy::BHShape, bvh::Bvh, point_query::PointDistance},
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
        mem,
    },
    taimi_hoard::{lazyfmt, str_opt, cmp::CmpIgnore, flags::BitSet, loc::LocationRef, time::Timestamp},
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
    taimi_pack::{category::id, attributes::{cell::GetAttrDynExt, keys::{self, GetAttr}, AttrString, InteractionAttributes, MarkerAttributes}},
    taimi_sync::arcs::ArcPtrCmp,
    tokio::sync::{broadcast, RwLock},
    tokio_stream::wrappers::errors::BroadcastStreamRecvError as BroadcastError,
    taimi_hoard::vec::vec32_eq,
    ordered_float::OrderedFloat,
    num_traits::AsPrimitive,
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

            (interaction.attr_or_default_into::<keys::TriggerRange, _>(), interaction.attr_or_default_into::<keys::AutoTrigger, _>())
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
    fn is_passive<A>(&self, attrs: &A) -> bool where
        A: GetAttr<keys::Info>
        + GetAttr<keys::CopyValue>
        + GetAttr<keys::ScriptFocus>
        // TODO: bleh
        + GetAttrDynExt,
    {
        let is_passive = self.is_auto() || attrs.has_attr_of::<keys::Info>() || attrs.has_attr_of::<keys::CopyValue>();
        #[cfg(feature = "paths-lua")]
        let is_passive = is_passive || attrs.has_attr_of::<keys::ScriptFocus>();
        is_passive
    }
    /// may display an unintrusive popup or notification, even if not allowed to auto-trigger
    ///
    /// [TriggerKind::SCRIPT] is relevant for `script-focus` events only
    pub const PASSIVE_NEARBY: TriggerKind =
        TriggerKind::from_bits_retain(TriggerKind::INFO.bits() | TriggerKind::COPY.bits() | TriggerKind::SCRIPT.bits());

    pub fn dist_dist(&self, point: &Point3<LocalSpace>) -> f32 {
        self.bounds.position.distance_squared(*point)
    }
    pub fn dist_dist2(&self, point: &Point2<LocalSpace>) -> f32 {
        self.bounds.position2().distance_squared(*point)
    }
    pub fn dist_dist_inrange(&self, point: &Point3<LocalSpace>) -> Option<f32> {
        if self.radius_radius == 0.0 { return None }
        let dist_dist = self.dist_dist(point);
        (dist_dist <= self.radius_radius).then_some(dist_dist)
    }
    pub fn dist_dist2_inrange(&self, point: &Point2<LocalSpace>) -> Option<f32> {
        if self.radius_radius == 0.0 { return None }
        let dist_dist = self.dist_dist2(point);
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
        if attrs.has_attr_of::<keys::CopyValue>() {
            interest.insert(TriggerKind::COPY);
        }
        if attrs.has_attr_of::<keys::Info>() {
            interest.insert(TriggerKind::INFO);
        }
        if attrs.has_attr_of::<keys::ResetGuid>() {
            interest.insert(TriggerKind::RESET);
        }
        if attrs.has_attr_of::<keys::Behaviour>() {
            interest.insert(TriggerKind::BEHAVIOUR);
        }
        if attrs.has_attr_of::<keys::Bounce>() {
            interest.insert(TriggerKind::BOUNCE);
        }
        #[cfg(todo)]
        if !lpoi.guid.is_nil() {
            interest.insert(TriggerKind::BEHAVIOUR);
        }
        if attrs.has_attr_of::<keys::ShowCategory>() {
            interest.insert(TriggerKind::SHOW);
        }
        if attrs.has_attr_of::<keys::HideCategory>() {
            interest.insert(TriggerKind::HIDE);
        }
        if attrs.has_attr_of::<keys::ToggleCategory>() {
            interest.insert(TriggerKind::TOGGLE);
        }
        (interest, attrs.attr_or_default::<keys::AutoTrigger>().into())
    }
    pub fn is_interactive(poi: &LoadedPoi) -> bool {
        let interactive = poi.get_interaction_attrs()
            .map(|i| !Self::interaction_is_empty(i))
            .unwrap_or(false);
        #[cfg(feature = "paths-lua")]
        let interactive = interactive || poi.info().marker_info.has_attr_of::<keys::ScriptTrigger>() || poi.info().marker_info.has_attr_of::<keys::ScriptFocus>();
        interactive
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
impl<const D: usize> PointDistance<f32, D> for SpaceInteraction
where
    TriggerBoundsInfo: PointDistance<f32, D>,
{
    #[inline]
    fn distance_squared(&self, query: nalgebra::Point<f32, D>) -> f32 {
        #[cfg(todo = "unnecessary")]
        if self.bounds.radius() > InteractReactor::CIRCLE_TOO_BIG {
            const BIG_DIST: f32 = taimi_meta::spatial::IRRELEVANT_MAX.powi(2);
            return BIG_DIST
        }
        self.bounds.distance_squared(query)
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
        matches!(self.dist_dist2_inrange(point), Some(..))
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
        #[cfg(todo)]
        let mut interest_auto = self.interest_auto;
        #[cfg(todo)]
        let mut interest_nearby = self.interest_nearby;
        let (mut interest_auto, mut interest_nearby) = (TriggerKind::empty(), TriggerKind::empty());
        let mut pois = maps.iter_with_info(map_info, Some(map_id))
            .flat_map(|(map_path, map, _map_info)|
                    map.lpois().iter()
                    .filter_map(move |(lpath, poi)| {
                        let (interest, auto) = SpaceInteraction::poi_interest(poi);
                        match interest.is_empty() {
                            true => None,
                            false => Some((map_path.rel(lpath.path), poi, interest, auto)),
                        }
                    })
            ).inspect(|(_, poi, interest, auto)| {
                let interest = *interest;
                #[cfg(feature = "paths-lua")]
                let interest = match interest.intersects(TriggerKind::SCRIPT) {
                    // other script attrs are irrelevant for "passive" classification
                    true if !poi.info.marker_info().has_attr_of::<keys::ScriptFocus>() =>
                        interest & !TriggerKind::SCRIPT,
                    _ => interest,
                };
                let nearbyi = match poi.info.marker_info().get_attr_dyn_of::<keys::TriggerRange>() {
                    // discard map-wide bounds, they don't help anyone...
                    Some(range) if f32::from(**range) > InteractReactor::CIRCLE_TOO_BIG =>
                        TriggerKind::empty(),
                    _ => interest,
                };
                interest_nearby.insert(nearbyi);
                if *auto {
                    interest_auto.insert(interest & TriggerKind::AUTO_TRIGGER_MASK);
                }
            });
        let mut entities_dirty = false;
        let mut trunc = None;
        for (i, dest) in self.entities.iter_mut().enumerate() {
            let Some((lpath, poi, _interest, _auto)) = pois.next() else {
                trunc = Some(i);
                break
            };
            let e = SpaceInteraction::with_poi(
                lpath,
                poi,
            );
            let prev = mem::replace(&mut dest.value, e);
            entities_dirty |= (prev.path != lpath) | !vec32_eq(prev.bounds.position, dest.bounds.position) | (prev.bounds.radius.to_bits() != dest.bounds.radius.to_bits());
        }
        if let Some(trunc) = trunc {
            entities_dirty = true;
            unsafe {
                self.entities.set_len(trunc);
            }
        } else {
            let additional = pois.map(|(lpath, poi, ..)| {
                entities_dirty = true;
                BvhShape::new(SpaceInteraction::with_poi(
                    lpath,
                    poi,
                ))
            });
            self.entities.extend(additional);
        }
        entities_dirty |= (self.interest_auto != interest_auto) | (self.interest_nearby != interest_nearby);
        if entities_dirty {
            self.clear_bvh();
            self.clear_nearby();
        }
        self.trigger_bvh_dirty |= entities_dirty && !self.entities.is_empty();
        self.interest_auto = interest_auto;
        self.interest_nearby = interest_nearby;
    }
    pub fn needs_trigger_bvh_rebuild(&self) -> bool {
        self.trigger_bvh_dirty
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

        // TODO: mem::take() instead? this isn't an arc...
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
        self.clear_entities();
        self.clear_nearby();
        self.clear_bvh();
    }
    fn clear_entities(&mut self) {
        self.entities.clear();
        self.interest_auto = TriggerKind::empty();
        self.interest_nearby = TriggerKind::empty();
    }
    fn clear_nearby(&mut self) {
        self.nearby.clear();
    }
    fn clear_bvh(&mut self) {
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

    /// TODO: filtered wrapper around nearest_traverse_iterator?
    #[cfg(todo)]
    pub fn nearest_passive_to(&self, pos: Point3<LocalSpace>) -> Option<(Point3<LocalSpace>, dist2)> {
    }
    /// 2d distance is returned (will not not account for y axis)
    pub fn nearest_to(&self, pos: Point3<LocalSpace>) -> Option<(Point3<LocalSpace>, f32)> {
        let trigger_bvh = self.trigger_bvh.as_ref().try_read().ok()?;
        trigger_bvh.nearest_to(LocalSpace::to2(pos).into_nalg(), &self.entities[..])
            .map(|(e, dist2)| (e.value.bounds.position, dist2))
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
    pub update_interval: f32,
    fallback_progress: usize,
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
            update_interval: PathingSettings::DEFAULT_INTERACT_RESPONSIVENESS,
            fallback_progress: 0,
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
        RenderEvent::MessageDismissMatching {
            filter: Box::new(move |id: &MarkerId| id.marker_path::<PackPath>().map(|p|
                p.path.namespace() == MarkerIndex::NS_POI
            ).unwrap_or(false)) as Box<_>,
        }.try_send();
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
    fn process_interaction(
        &mut self,
        event_tx: &broadcast::Sender<InteractionEvent>,
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

        let mut took_action = None::<TriggerKind>;
        let mut side_effects = TriggerKind::empty();
        let blocked = "trigger settings blocked";
        #[cfg(todo = "unnecessary")]
        if let Some(message) = attrs.info.as_ref() {
            let allowed = allowed & TriggerKind::INFO;
            took_action.get_or_insert_default().insert(allowed);
            if !allowed.is_empty() {
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
            let allowed = allowed & TriggerKind::COPY;
            took_action.get_or_insert_default().insert(allowed);
            if !allowed.is_empty() {
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
        for (id, show_hide) in keys::ShowHideAction::iter_in_attrs(attrs) {
            let allowed = allowed & TriggerKind::from_show_hide_action(show_hide);
            took_action.get_or_insert_default().insert(allowed);
            if !allowed.is_empty() {
                #[cfg(todo)]
                let cat = show_hide.category().pivot(loaded_path.root.root);
                let cat = id::IdNameBox::with_arcbox(id.into_owned());
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
        if let Some(reset) = attrs.get_attr_of::<keys::ResetGuid>() {
            let allowed = allowed & TriggerKind::RESET;
            took_action.get_or_insert_default().insert(allowed);
            if !allowed.is_empty() {
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
            let allowed = allowed & TriggerKind::SCRIPT;
            took_action.get_or_insert_default().insert(allowed);
            if !allowed.is_empty() {
                log::debug!("TODO: interact script");
            } else {
                log::info!("{blocked} script");
            }
        }
        if let Some(_bounce) = attrs.bounce_behavior {
            let allowed = allowed & TriggerKind::BOUNCE;
            side_effects.insert(allowed);
            if !allowed.is_empty() {
                log::debug!("TODO: interact bounce anim");
            } else {
                log::info!("{blocked} animation");
            }
        }

        let behaviour = match &action {
            InteractionEventAction::Dismiss(config) => Some((config.mode, config.reset_delay.into())),
            _ => attrs.clone_attr_of::<keys::Behaviour>().map(|b| (b, attrs.attr_or_default::<keys::ResetLength>())),
        };
        if let Some((behaviour, reset_delay)) = behaviour {
            let organic = match action.is_natural() {
                true => took_action.map(|a| !a.is_empty()).unwrap_or(true),
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
                    Behaviour::Blish(BlishBehaviour::TaimiAchievement) => {
                        log::info!("TODO: dismiss achievement marker immediately");
                        None
                    },
                    Behaviour::Taco(TacoBehaviour::ResetDelay) =>
                        Some(Either::Right(reset_delay.duration())),
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
                took_action.get_or_insert_default().insert(TriggerKind::BEHAVIOUR);
            } else {
                log::info!("{blocked} dismiss behaviour");
            }
        } else if matches!(action, InteractionEventAction::Interact) && took_action.map(|a| !a.is_empty() && a.contains(TriggerKind::COPY) && !a.contains(TriggerKind::SCRIPT)).unwrap_or(false) {
            let contexts = vec![HideContext::for_map(loaded_path.root.path, None)];
            events.push(PathingEvent::DismissMarker {
                path: marker_path,
                loaded_path: lpath,
                until: Some(Either::Right(Self::INTERACT_COOLDOWN)),
                contexts,
                reset: Some(AutoReset::Distance),
            });
            took_action.get_or_insert_default().insert(TriggerKind::DISMISS);
        }
        let aftermath = took_action.unwrap_or_default() | side_effects;
        if !aftermath.is_empty() {
            let action = InteractionEventAction::Report(aftermath);
            let _ = event_tx.send(InteractionEvent::Interact { action, path, loaded_path });
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
                    inverted: lpoi.info().filter_attrs().attr_or_default::<keys::InvertBehaviour>().into(),
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
            InteractionEventAction::Report(..) => {
                // should've been filtered out much sooner...
                TriggerKind::empty()
            },
        }
    }
    fn prepare_action<'a>(
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
                    allow if allow.is_empty() => false,
                    _ if !attrs.attr_or_default_into::<keys::AutoTrigger, bool>() => false,
                    allow => SpaceInteraction::interest_for(attrs).0.intersects(allow),
                };

                let action = if act_auto { Some(InteractionEventAction::AutoTrigger) } else { None };
                action.map(|action| (path, loaded_path, lpoi, guid, action))
            },
            InteractionEvent::Interact { action: InteractionEventAction::Report(..), .. } =>
                // should've been filtered out but just in case...
                None,
            &InteractionEvent::Interact { ref action, path, loaded_path } => {
                let (map, map_info) = maps.lookup_with_info(map_info, &loaded_path.root)?;
                let lpath: LoadedPoiPath = loaded_path.unscope();
                let guid = map.poi_guid_by_index(map_info, lpath);
                let lpoi = map.lpois().lookup_ref(&lpath)?;
                Some((path, loaded_path, lpoi, guid, action.clone()))
            },
            // routes to process_gone() instead
            InteractionEvent::Gone { .. } => None,
        }
    }
    fn process_interact<'a>(
        &mut self,
        event_tx: &broadcast::Sender<InteractionEvent>,
        filter_state: &FilterState,
        map_info: &LoadedMapInfo,
        maps: &LoadedMaps,
        event: InteractionEvent,
    ) -> PathingEvent {
        if let &InteractionEvent::Nearby { path, loaded_path } = &event {
            if let Some(item) = self.process_nearby_info((path, loaded_path), map_info, maps) {
                let key = MarkerId::for_marker(loaded_path.root.root.rel(MarkerIndex::with_poi(path.path)));
                RenderEvent::MessageInfo {
                    key,
                    item,
                }.try_send();
            }
        }
        match event {
            InteractionEvent::Gone { path, loaded_path } =>
                self.process_gone(filter_state, map_info, maps, (path, loaded_path)),
            ref event @ (InteractionEvent::Nearby { .. } | InteractionEvent::Interact { .. }) => {
                let Some((path, lpath, lpoi, guid, action)) = self.prepare_action(&event, map_info, maps)
                else {
                    return PathingEvent::Nop
                };
                let allowed = self.allow_action(filter_state, (lpath, lpoi), (path, guid), &action);
                #[cfg(feature = "paths-lua")]
                if !allowed.is_empty() && matches!(action, InteractionEventAction::AutoTrigger) && matches!(event, InteractionEvent::Nearby { .. }) {
                    // TODO: && allowed.contains(TriggerType::SCRIPT)?
                    // it'll get back to us here momentarily don't worry...
                    let sent = event_tx.send(InteractionEvent::Interact { action, path, loaded_path: lpath });
                    if sent.is_ok() {
                        return PathingEvent::Nop
                    }
                }
                self.process_interaction(event_tx, filter_state, (path, lpath, lpoi), action, allowed)
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
        let marker_path = lpath.root.root.rel(MarkerIndex::with_poi(path.path));
        let marker_id = MarkerId::for_marker(marker_path);
        let dismiss_info = || RenderEvent::MessageDismiss {
            // TODO: make this an interact/pathing event rather than render?
            // also better tracking of active info markers on self?
            key: marker_id,
        }.try_send();
        let Some((map, map_info)) = maps.lookup_with_info(map_info, &lpath.root) else {
            // just in case we lost track of it...
            dismiss_info();
            return PathingEvent::Nop
        };
        let guid = map.poi_guid_by_index(map_info, lpath.unscope()).cloned();
        let had_info = map.lpois().lookup_ref(&lpath.unscope())
            .map(|lpoi| lpoi.interaction_attrs().has_attr_of::<keys::Info>() | lpoi.interaction_attrs().has_attr_of::<keys::CopyValue>());
        if let Some(true) | None = had_info {
            dismiss_info();
        }
        let mut msg = if self.handle_interaction_end(filter_state, &marker_id) {
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

        msg
    }
    fn process_nearby_info<'a>(
        &mut self,
        (path, loaded_path): (PoiPath, PoiMapPath),
        map_info: &'_ LoadedMapInfo,
        maps: &'a LoadedMaps,
    ) -> Option<MessageItemDesc> {
        let (map, map_info) = maps.lookup_with_info(map_info, &loaded_path.root)?;
        let lpath: LoadedPoiPath = loaded_path.unscope();
        let lpoi = map.lpois().lookup_ref(&lpath)?;
        let guid = map.poi_guid_by_index(map_info, lpath);
        let attrs = lpoi.interaction_attrs();
        let (interest, marker_is_auto) = SpaceInteraction::interest_for(attrs);
        let act_auto = marker_is_auto & interest.intersects(self.config.trigger_allow_auto);

        let mut info_item = MessageItemDesc::default();
        let new_action = |action| MessageActionDesc::blank(Box::new(move || {
            let e = InteractionEvent::Interact { action, path, loaded_path };
            Controller::with_sender(|s| s.pathing.as_ref().map(move |p| {
                let _res = p.shared.interact.events.send(e);
                #[cfg(taimi_debug)]
                if let Err(..) = _res {
                    log::debug!("interact queue full?");
                }
            }));
        }) as Box<_>);
        let copy = (!(self.config.trigger_allow_auto | self.config.trigger_allow_interact).is_empty() & interest.contains(TriggerKind::COPY))
            .then(|| attrs.get_attr_of::<keys::CopyValue>())
            .flatten();
        let copy = copy.as_ref().and_then(|s| str_opt(&**s));
        if let Some(copy) = copy {
            let mut act = new_action(InteractionEventAction::Manual(TriggerKind::COPY));
            act.set_id(*fl!("poi-activate-copy").id_name());
            act.set_tooltip_desc(lazyfmt::fmt_fn(|f| {
                write!(f, "{:?}", &copy[..])?;
                if let Some(msg) = attrs.get_attr_of::<keys::CopyMessage>() {
                    write!(f, "\n\n{}", &msg[..])?;
                }
                Ok(())
            }).to_string());
            info_item.actions.push(act);
        }
        let passive_info = ((self.config.trigger_allow_auto & TriggerKind::INFO).intersects(interest) | copy.is_some())
            .then(|| attrs.get_attr_of::<keys::Info>());
        if let Some(Some(info)) = passive_info {
            info_item.set_message(&info[..]);
        }
        if info_item.is_empty() {
            return None
        }
        let linfo = lpoi.info();
        if let Some(tip) = linfo.marker_info.get_attr_of::<keys::TipName>() {
            info_item.set_title(&tip[..]);
        }
        if let Some(tip) = linfo.marker_info.get_attr_of::<keys::TipDescription>() {
            info_item.set_tooltip_desc(&tip[..]);
        }
        if let Some(guid) = guid {
            info_item.set_attribution(guid.to_string());
        }
        if !act_auto {
            if marker_is_auto {
                let mut act = new_action(InteractionEventAction::Manual(self.config.trigger_allow_interact));
                act.mark_context_menu(true);
                act.set_id(*fl!("poi-activate").id_name());
                info_item.actions.push(act);
            } else {
                let mut act = new_action(InteractionEventAction::Interact);
                act.mark_context_menu(true);
                act.set_id(*fl!("Miscellaneous_Interact").id_name());
                info_item.actions.push(act);
            }
        }
        #[cfg(todo)]
        let mut act = new_action(InteractionEventAction::Manual(TriggerKind::all()));
        let mut act = new_action(InteractionEventAction::Trigger);
        act.mark_context_menu(true);
        act.set_id(*fl!("poi-activate-override").id_name());
        with_i18n!("poi-activate-override-notice", |notice| act.set_tooltip_desc(&notice[..]));
        info_item.actions.push(act);

        let mut act = new_action(InteractionEventAction::Manual(TriggerKind::DISMISS));
        act.mark_context_menu(true);
        act.mark_dismiss(true);
        act.set_id(*fl!("trigger-behaviour").id_name());
        info_item.actions.push(act);

        Some(info_item)
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
    /// these are dumb to consider because of packs that use map-wide bounds...
    pub const CIRCLE_TOO_BIG: f32 = 20.0;
    pub const UPDATE_INTERVAL_SLOW_MS: u16 = 10_000u16;
    pub const UPDATE_INTERVAL_PASSIVE_MS: u16 = 1_000u16;
    /// please tell me no one wants frame-perfect interactions...
    /// 60fps *is* more than enough for anyone right guys..?
    pub const UPDATE_INTERVAL_MIN_MS: u16 = 16;
    /// help estimate how close player is to a passive marker, m/s
    const MAX_PLAYER_SPEED: f32 = match 20.0 {
        #[cfg(todo)]
        s => s,
        // prefer to err on introducing slight delay, some underestimating is fine...
        s => s * 0.75,
    };
    const MAX_PLAYER_M_PER_S: f32 = Self::MAX_PLAYER_SPEED.recip();
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

        let mut outgoing = nearby;
        #[cfg(todo)]
        if outgoing.map_id != Some(map_id) {
            // do we need to care to emit events for each marker when leaving a map?
        }
        outgoing.set_map_id(Some(map_id));
        let mut incoming = NearbyMarkers::new_on_map(map_id);
        if !self.map_interactions.trigger_bvh_dirty {
            let trigger_bvh = self.map_interactions.trigger_bvh.read().await;
            let query = SpaceInteraction::point_query(pos);
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
            self.fallback_progress = 0;
        } else if !self.map_interactions.entities.is_empty() {
            // fall back to partial update while still loading...
            let total = self.map_interactions.entities.len();
            let start = self.fallback_progress % total;
            let amt = total.min(Self::FALLBACK_MAX_ITERATIONS);
            self.fallback_progress = start.wrapping_add(amt);
            // TODO: avoid sequential access when overbudget...
            let subset = self.map_interactions.entities.iter()
                .cycle()
                .skip(start)
                .take(amt);
            // TODO: consider spawn_blocking and increase the window?
            for entity in subset {
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
            let deferred = total - amt;
            let deferred_start = self.fallback_progress % total;
            let deferred = self.map_interactions.entities.iter()
                .cycle()
                .skip(deferred_start)
                .take(deferred);
            for entity in deferred {
                let id = entity.value.poi_path();
                if !outgoing.contains_loaded_poi(id) { continue }
                let is_nearby = entity.value.point_query_postprocess_filter(pos);
                if is_nearby {
                    // XXX: could just unconditionally remove the remainder, but
                    // it's probably better to be responsive when it's likely a select few...
                    outgoing.remove_poi(id);
                }
            }
        }
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
    const FALLBACK_MAX_ITERATIONS: usize = 96;
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
        let mut nearby = rx.nearby_tx.borrow().clone();
        let mut nearby_pois = BinaryHeap::with_capacity(nearby.len() * 4);

        let trigger_bvh = self.map_interactions.trigger_bvh.read().await;
        let query = SpaceInteraction::point_query(pos);
        let inrange = trigger_bvh
            .traverse_iterator(&query, &self.map_interactions.entities[..]);
        for entity in inrange {
            let id = entity.value.poi_path();
            let Some(dist_dist) = entity.value.dist_dist_inrange(&pos) else {
                if let Some(path) = nearby.remove_poi(id) {
                    rx.nearby_tx.send_modify(|shared| {
                        shared.remove_poi(id);
                    });
                    let _ = rx.event_tx.send(InteractionEvent::Gone { path: path, loaded_path: id });
                }
                continue
            };
            let Some((lpoi, map, map_info)) = Self::lookup_lpoi_at(map_info, maps, id) else {
                continue
            };
            if !lpoi.visibility.is_visible() {
                continue
            }
            let nearby_discrete = (dist_dist * 1_000_000.0).min(0x40000000u32 as f32) as u32;
            let prev_nearby = nearby.contains_loaded_poi(id);
            let auto_trigger = entity.value.bounds.is_auto();
            if !prev_nearby {
                let path = unsafe { map_info.poi_path_unchecked(id.unscope()) };
                rx.nearby_tx.send_modify(|shared| {
                    shared.insert_poi(id, path);
                });
                let sent = rx.event_tx.send(InteractionEvent::Nearby { path, loaded_path: id });
                if auto_trigger && sent.is_ok() {
                    continue
                }
                #[cfg(todo = "unnecessary")]
                {
                    // maybe there could be duplicates..?
                    nearby.insert_poi(id, path);
                }
            } else if matches!(action, InteractionEventAction::Interact) && auto_trigger {
                if !lpoi.info.marker_info.has_attr_of::<keys::ScriptTrigger>() {
                    // repeated interactions are allowed, so this cannot be consumed even if auto-triggered
                    // TODO: same with copy? especially if it was hidden for a period after having autotriggered...
                    continue
                }
            }
            let is_passive = entity.value.is_passive(lpoi.info().marker_info());
            let auto_triggered = is_passive && prev_nearby;
            let (mut interest, _interest_auto) = SpaceInteraction::interest_for(lpoi.interaction_attrs());
            #[cfg(taimi_debug)]
            if _interest_auto != auto_trigger {
                // TODO: deleteme and/or cfg(debug_assertions)
                log::debug!("BUG: auto-trigger mismatch for {id}");
            }
            let interest_consumed = match prev_nearby {
                true if auto_trigger => interest & self.config.trigger_allow_auto,
                _ => TriggerKind::empty(),
            };
            if lpoi.info.marker_info.has_attr_of::<keys::ScriptTrigger>() {
                interest.insert(TriggerKind::SCRIPT);
            }
            let interest_avail = interest & !interest_consumed;
            let interest_boring = TriggerKind::BOUNCE | TriggerKind::INFO;
            #[cfg(not(feature = "paths-lua"))]
            let interest_boring = interest_boring | TriggerKind::SCRIPT;
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
            let allowed = self.allow_action(filter_state, (lpath, lpoi), (path, guid), &action);
            if allowed.is_empty() {
                continue
            }
            let sent = rx.event_tx
                .send(InteractionEvent::Interact { action, path, loaded_path: lpath });
            if sent.is_ok() {
                continue
            }
            if !allowed.is_empty() {
                let interact = self.process_interaction(&rx.event_tx, filter_state, (path, lpath, lpoi), action, allowed);
                #[cfg(taimi_debug)]
                if !matches!((&res, &interact), (PathingEvent::Nop, _) | (_, PathingEvent::Nop)) {
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
        let rx_tick = 32;
        let mut rx_retry = Self::EVENT_RX_RETRY * rx_tick;
        while rx_retry > 0 {
            let mut rx_tick = rx_tick;
            match rx.event_rx.poll_next_unpin(cx) {
                Poll::Ready(Some(Ok(InteractionEvent::Interact { action: InteractionEventAction::Report(..), .. }))) => {
                    // we emit these, filter them out early...
                    rx_tick = 1
                },
                Poll::Ready(Some(Ok(e))) => return Poll::Ready(InteractMessage::Event(e)),
                Poll::Ready(Some(Err(BroadcastError::Lagged(amt)))) => {
                    log::warn!("lagged behind by {amt} interactions");
                    // TODO: clear out queue if this recurs?
                },
                Poll::Ready(None) | Poll::Pending => break,
            }
            rx_retry = rx_retry.saturating_sub(rx_tick);
        }
        if let Some((auto, passive)) = self.interest_movement() {
            const SLOW_BOUND_UPPER32: u32 = InteractReactor::UPDATE_INTERVAL_SLOW_MS as u32;
            let mut something_is_close = false;
            let (next_mul, next_range) = match (auto, passive) {
                (false, false) => {
                    // map requires explicit interact key events, so we can be very lax with updates...
                    const SLOW_BOUND_LOWER32: u32 = (InteractReactor::UPDATE_INTERVAL_PASSIVE_MS << 1) as u32;
                    (92, SLOW_BOUND_LOWER32..SLOW_BOUND_UPPER32)
                },
                _ if self.map_interactions.needs_trigger_bvh_rebuild() => {
                    // when bvh postponed for higher-prio tasks, don't poll as often
                    (20, Self::UPDATE_INTERVAL_PASSIVE_MS as u32..(Self::UPDATE_INTERVAL_PASSIVE_MS << 3) as u32)
                },
                _ if self.map_interactions.nearby.any() => {
                    // allow slightly tighter timings if a marker is currently inrange
                    something_is_close = true;
                    (2, Self::UPDATE_INTERVAL_MIN_MS as u32..((Self::UPDATE_INTERVAL_PASSIVE_MS << 1) as u32))
                },
                // maybe change strictness depending on what kind of markers are onmap and/or nearby? idk
                #[cfg(todo)]
                (false, true) => todo,
                _ =>
                    (5, (Self::UPDATE_INTERVAL_MIN_MS << 2) as u32..(Self::UPDATE_INTERVAL_PASSIVE_MS << 1) as u32),
            };
            const UPDATE_INTERVAL_SHIFT_AMT: u32 = InteractReactor::UPDATE_INTERVAL_PASSIVE_MS as u32 * 2;
            const UPDATE_INTERVAL_SHIFT_AMT_NEG: isize = -(UPDATE_INTERVAL_SHIFT_AMT.next_power_of_two() as isize);
            if let Poll::Ready(pos) = rx.player_pos.poll_next_update(cx) {
                let base32 = self.config.base_responsiveness_ms as u32;
                let absolute_min_ms = base32.max(Self::UPDATE_INTERVAL_MIN_MS as u32);
                let many = base32 * next_mul;
                let mut next_interval_ms = many.clamp(next_range.start, next_range.end) as u64;
                let current_interval = rx.player_pos.threshold_timeout();
                let current_interval_ms = current_interval.as_millis() as u64;
                let target_int = next_interval_ms as isize - current_interval_ms as isize;

                let mut resync_distance = !something_is_close || target_int > 0;
                if !resync_distance && current_interval_ms < 400 {
                    let occasional = (rx.player_pos.last_tick() & 0x3f) == 1;
                    if occasional {
                        resync_distance = true;
                    }
                }
                let check_pacing = resync_distance.then(|| self.map_interactions.nearest_to(pos)).flatten();
                if let Some((_point, dist2)) = check_pacing {
                    // checking vicinity to determine how slow we can be...
                    let upper_limit = (next_interval_ms + UPDATE_INTERVAL_SHIFT_AMT as u64).min(SLOW_BOUND_UPPER32 as u64);
                    let seconds = dist2 * Self::MAX_PLAYER_M_PER_S;
                    let ms = (seconds * 1000.0f32) as u64;
                    let prev = next_interval_ms;
                    next_interval_ms = ms.clamp(absolute_min_ms as u64, upper_limit);
                }
                rx.player_pos.set_threshold_timeout(Duration::from_millis(next_interval_ms));
                #[cfg(taimi_debug)]
                {
                    STATS_POLL_INTERVAL.reset(rt::statistics::StatsUnit::time_ms(next_interval_ms));
                }
                return Poll::Ready(InteractMessage::PlayerMoved(pos))
            } else {
                #[cfg(todo = "unnecessary")]
                if passive && target_int <= UPDATE_INTERVAL_SHIFT_AMT_NEG {
                    // polling too slow for our tastes, hurry up!
                    next_interval_ms += UPDATE_INTERVAL_SHIFT_AMT as u64;
                    rx.player_pos.set_threshold_timeout(Duration::from_millis(next_interval_ms));
                    #[cfg(taimi_debug)]
                    {
                        STATS_POLL_INTERVAL.reset(rt::statistics::StatsUnit::time_ms(next_interval_ms));
                    }
                }
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
            InteractMessage::RefreshNearby => {
                if let Some(pos) = rx.interact.player_pos.position_last_seen() {
                    let res = self.process_movement(&mut rx.interact, map_info, maps, pos).await;
                    //rx.interact.player_pos.readjust_now();
                    res
                } else {
                    PathingEvent::Nop
                }
            },
            InteractMessage::RefreshInfo(lpath, marker_path) => {
                let poi_path = match lpath.path.namespace() {
                    MarkerIndex::NS_POI => {
                        let poi_path: PoiPath = PoiPath::new_path(marker_path.path.index_poi_unchecked());
                        Some((
                            lpath.map_path(|p| p.index_poi_unchecked()),
                            poi_path,
                        ))
                    },
                    _ => None,
                };
                let item_update = poi_path.and_then(|(lpath, path)| {
                    let is_nearby = rx.interact.nearby_tx.borrow().contains_loaded_poi(lpath);
                    if !is_nearby {
                        // likely dismissed, ignore
                        None
                    } else {
                        self.process_nearby_info((path, lpath), map_info, maps)
                    }
                });
                if let Some(item) = item_update {
                    RenderEvent::MessageInfo {
                        key: MarkerId::for_marker(lpath.root.root.rel(marker_path.path)),
                        item,
                    }.try_send();
                }
                PathingEvent::Nop
            },
            InteractMessage::BvhRebuild => {
                if rx.shared.packs.read_still_waiting().0 {
                    #[cfg(taimi_debug)]
                    log::debug!("DELETEME: delaying interact bvh");
                    return PathingEvent::Nop
                }
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
            InteractMessage::Event(event) => self.process_interact(&rx.interact.event_tx, filter_state, map_info, maps, event),
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
    RefreshNearby,
    RefreshInfo(LoadedMarkerPath<PackMapPath>, MarkerPath),
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
impl From<InteractMessage> for PathingEvent {
    #[inline]
    fn from(e: InteractMessage) -> Self {
        match e {
            #[cfg(todo = "unnecessary")]
            InteractMessage::Nop => PathingEvent::Nop,
            e => PathingEvent::InteractControl(e),
        }
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
        let value = &value[..];
        if value.is_empty() { return }
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
    #[cfg(deleteme)]
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

    pub(super) async fn interact_rebuild_if_needed(&mut self) {
        if self.interact.map_interactions.trigger_bvh_dirty {
            // schedule it for next poll... could try inline if no other events are pending but bleh
            self.interact.event_dirty_bvh_rebuild = true;
        }
    }
    pub(super) fn interact_entity_updates(&mut self) {
        self.interact.event_dirty_entities = true;
    }
}
impl PathingController {
    /// TODO: deleteme
    pub fn spawn_alert(&mut self, message: String, duration: Duration) {
        use tokio::time::sleep;
        static PATHING_ALERT_HACK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

        #[cfg(taimi_debug)]
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
    pub base_responsiveness: OrderedFloat<f32>,
    pub base_responsiveness_ms: u16,
}
impl InteractSettings {
    pub fn from_settings(settings: &PathingSettings) -> Self {
        let PathingSettings {
            trigger_allow_auto,
            trigger_allow_interact,
            interact_base_responsiveness,
            ..
        } = *settings;
        Self {
            trigger_allow_auto,
            trigger_allow_interact,
            base_responsiveness: interact_base_responsiveness.into(),
            base_responsiveness_ms: (interact_base_responsiveness * 1000.0f32).as_(),
        }
    }
}
impl Default for InteractSettings {
    fn default() -> Self {
        Self {
            trigger_allow_auto: TriggerKind::SETTINGS_DEFAULT_AUTO,
            trigger_allow_interact: TriggerKind::SETTINGS_DEFAULT_INTERACT,
            base_responsiveness: PathingSettings::DEFAULT_INTERACT_RESPONSIVENESS.into(),
            base_responsiveness_ms: (PathingSettings::DEFAULT_INTERACT_RESPONSIVENESS * 1000.0f32) as u16,
        }
    }
}

#[cfg(taimi_debug)]
pub(crate) static STATS_POLL_INTERVAL: rt::statistics::StatsCounter = rt::statistics::StatsCounter::new(0);
