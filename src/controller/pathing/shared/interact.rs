use bvh::bvh::Bvh;
use crate::controller::pathing::state::interactive::InteractionEvent;
use crate::controller::pathing::registry::{PoiMapPath, LoadedPoiPath};
use crate::controller::runtime::WallInstant;
use crate::exports::runtime::MumblePtr;
use glamour::Point3;
use tokio::{
    sync::broadcast,
    time::{Sleep, Instant},
};
use taimi_meta::coords::vec_eq;
use taimi_meta::coords::LocalSpace;
use taimi_meta::packs::{MapIndex, PackPath, PoiPath};
use taimi_sync::watched;
use futures::ready;
use futures::stream::Stream;
use std::future::Future;
use std::collections::{BTreeMap, BTreeSet};
use std::task::Poll;
use std::task::Context;
use std::{ptr, mem};
use std::time::Duration;
use std::fmt;
use std::pin::Pin;
use std::sync::{LazyLock, Arc};
use tokio::sync::RwLock;
use taimi_hoard::iters::IterExt as _;
use taimi_hoard::loc::Locator;

pub type InteractShared = InteractSender;

pub type PlayerPosition = Point3<LocalSpace>;

#[derive(Debug, Clone)]
pub struct InteractSender {
    pub events: broadcast::Sender<InteractionEvent>,
    pub trigger_bvh: watched::Tx<SharedTriggerBvh>,
    pub nearby: watched::Tx<SharedNearbyMarkers>,
}
impl InteractSender {
    pub fn new() -> Self {
        Self {
            events: broadcast::Sender::new(Self::EVENT_CAPACITY),
            trigger_bvh: watched::Tx::new(empty_trigger_bvh().clone()),
            nearby: watched::Tx::new(Default::default()),
        }
    }
    pub const EVENT_CAPACITY: usize = 48;
}

#[derive(Debug)]
pub struct InteractReceiver {
    pub event_tx: broadcast::Sender<InteractionEvent>,
    pub event_rx: broadcast::Receiver<InteractionEvent>,
    pub trigger_bvh_tx: watched::Tx<SharedTriggerBvh>,
    pub nearby_tx: watched::Tx<SharedNearbyMarkers>,
    pub player_pos: FollowPlayer,
}
impl InteractReceiver {
    pub fn new(tx: &InteractSender) -> Self {
        Self {
            event_rx: tx.events.subscribe(),
            event_tx: tx.events.clone(),
            trigger_bvh_tx: tx.trigger_bvh.clone(),
            nearby_tx: tx.nearby.clone(),
            player_pos: FollowPlayer::new(),
        }
    }
}

/// POI trigger bvh traversal doesn't really need 3D?
pub const TRIGGER_DIMENSION: usize = 2;
#[cfg(todo = "unnecessary")]
pub const TRIGGER_DIMENSION: usize = 3;
pub type TriggerBvh = Bvh<f32, {TRIGGER_DIMENSION}>;
/// don't write thanks, no I'm not going to bother with a newtype
pub type SharedTriggerBvh = Arc<RwLock<TriggerBvh>>;
pub fn empty_trigger_bvh() -> &'static SharedTriggerBvh {
    static EMPTY_BVH_RW: LazyLock<SharedTriggerBvh> = LazyLock::new(||
        Arc::new(RwLock::new(Bvh { nodes: Vec::new() }))
    );
    &EMPTY_BVH_RW
}

#[cfg(todo = "unnecessary")]
pub type SharedNearbyMarkers = Arc<NearbyMarkers>;
pub type SharedNearbyMarkers = NearbyMarkers;
pub type NearbyPoiPath = Locator<PackPath, LoadedPoiPath>;
#[derive(Debug, Clone, Default)]
pub struct NearbyMarkers {
    pub map_id: Option<MapIndex>,
    pub pois: BTreeMap<NearbyPoiPath, PoiPath>,
}
impl NearbyMarkers {
    pub fn empty() -> Self {
        Self::default()
    }
    pub fn new_on_map(map_id: MapIndex) -> Self {
        Self {
            map_id: Some(map_id),
            pois: Default::default(),
        }
    }

    pub fn len(&self) -> usize { self.pois.len() }
    pub fn is_empty(&self) -> bool { self.map_id.is_none() || self.pois.is_empty() }
    pub fn contains_loaded_poi(&self, path: PoiMapPath) -> bool {
        self.lpoi_path(path).map(|lpath|
            self.pois.contains_key(&lpath)
        ).unwrap_or(false)
    }
    pub fn iter_pois(&self) -> impl Iterator<Item = (PoiMapPath, PoiPath)> + '_ {
        self.map_id.map(move |map_id|
            self.pois.iter()
                .lazy_map(move |(lpath, poi_path)| {
                    let lpath =
                        lpath.map_root(|root| root.rel(map_id))
                        .map_path(|p| p.path);
                    (lpath, *poi_path)
                })
            ).into_iter().flatten()
    }
    pub fn iter_poi_lpaths(&self) -> impl Iterator<Item = NearbyPoiPath> + '_ {
        self.pois.keys().copied()
    }

    pub fn set_map_id(&mut self, map_id: Option<MapIndex>) {
        if map_id == self.map_id { return }
        self.clear();
        self.map_id = map_id;
    }
    pub fn clear(&mut self) {
        self.pois.clear();
        self.map_id = None;
    }
    pub fn insert_poi(&mut self, loaded_path: PoiMapPath, path: PoiPath) {
        if self.map_id != Some(loaded_path.root.path) {
            log::warn!("bug? {}={} not on {:?}", loaded_path, path.path, self.map_id);
            return
        }
        let lpoi: LoadedPoiPath = loaded_path.unscope();
        let lpath = loaded_path.root.root
            .rel(lpoi);
        self.pois.insert(lpath, path);
    }
    pub fn remove_poi(&mut self, loaded_path: PoiMapPath) -> Option<PoiPath> {
        self.lpoi_path(loaded_path).and_then(|lpath|
            self.pois.remove(&lpath)
        )
    }
    pub fn append_take_from(&mut self, incoming: &mut Self) {
        self.pois.append(&mut incoming.pois);
    }
    #[inline]
    pub fn remove_pois_sorted<I>(&mut self, pois: I) where
        I: IntoIterator<Item = NearbyPoiPath>,
    {
        self.remove_pois_sorted_dyn(&mut pois.into_iter())
    }
    pub fn remove_pois_sorted_dyn(&mut self, pois: &mut dyn Iterator<Item = NearbyPoiPath>) {
        let mut pois = pois.peekable();
        self.pois.retain(|key, _| {
            pois.next_if(|up| up == key)
                .is_some()
        });
    }
    pub fn remove_pois(&mut self, pois: &BTreeSet<PoiMapPath>) {
        let map_id = self.map_id;
        let pois = pois.iter().filter_map(move |&loaded_path|
            Self::lpoi_path_with(map_id, loaded_path)
        );
        self.remove_pois_sorted(pois)
    }

    fn lpoi_path(&self, loaded_path: PoiMapPath) -> Option<NearbyPoiPath> {
        Self::lpoi_path_with(self.map_id, loaded_path)
    }
    fn lpoi_path_with(map_id: Option<MapIndex>, loaded_path: PoiMapPath) -> Option<NearbyPoiPath> {
        if map_id != Some(loaded_path.root.path) { return None }
        let lpoi: LoadedPoiPath = loaded_path.unscope();
        Some(loaded_path.root.root.rel(lpoi))
    }
}

pub struct FollowPlayer {
    pub ml: Option<MumblePtr>,
    pub update_throttle: Pin<Box<Sleep>>,
    update_throttle_ready: bool,
    threshold_time: Duration,
    /// don't bother triggering if player hasn't moved at least `sqrt(distance)` [metres](LocalSpace)
    threshold_distance_squared: f32,
    /// latest emitted position
    last_seen: PlayerPosition,
    /// latest emitted tick
    last_tick: u32,
    last_emitted: Instant,
    /// last read pos (likely pending if thresholds not met)
    cached_pos: PlayerPosition,
    cached_tick: u32,
    /// once we can count the monotonic player-pos ticks
    #[cfg(todo)]
    update_throttle_tick: u32,
    #[cfg(todo)]
    threshold_time_ticks: u32,
}
impl FollowPlayer {
    /// 40ms (25 fps)
    pub const MUMBLELINK_POS_INTERVAL: Duration = Duration::from_millis(1000 / 25);
    /// throttle events to occur every few updates (~4fps)
    pub const DEFAULT_THRESHOLD_TIME: Duration = Duration::from_millis(Self::MUMBLELINK_POS_INTERVAL.as_millis() as u64 * 6);
    /// even if [self.threshold_distance_squared] is unmet, eventually synchronize anyway
    pub const IDLE_TIMEOUT_TICKS: u32 = 64;
    /// [Self::MUMBLELINK_POS_INTERVAL] * [Self::IDLE_TIMEOUT_TICKS]
    pub const IDLE_TIMEOUT: Duration = Duration::from_millis(Self::MUMBLELINK_POS_INTERVAL.as_millis() as u64 * Self::IDLE_TIMEOUT_TICKS as u64);
    /// ~0.07m
    pub const DEFAULT_THRESHOLD_DIST_DIST: f32 = 0.005;

    pub fn new() -> Self {
        Self {
            ml: None,
            update_throttle: Box::pin(WallInstant::no_sleep()),
            update_throttle_ready: true,
            threshold_time: Self::DEFAULT_THRESHOLD_TIME,
            threshold_distance_squared: Self::DEFAULT_THRESHOLD_DIST_DIST,
            last_seen: Point3::INFINITY,
            last_tick: 0,
            last_emitted: WallInstant::passed_instant().into(),
            cached_pos: Point3::INFINITY,
            cached_tick: 0,
            #[cfg(todo)]
            update_throttle_tick: 0,
            #[cfg(todo)]
            threshold_time_ticks: Self::threshold_interval_ticks_for(Self::DEFAULT_THRESHOLD_TIME),
        }
    }
    pub fn set_ml(&mut self, ml: Option<MumblePtr>) {
        self.ml = ml;
    }
    pub fn reset(&mut self) {
        if self.last_tick != 0 {
            self.clear_throttle();
        }
        self.last_tick = match self.cached_tick {
            #[cfg(todo = "unnecessary")]
            t => t.wrapping_sub(1),
            t => t,
        };
        self.last_seen.x = f32::INFINITY;
        self.cached_pos.x = f32::INFINITY;
    }
    pub fn set_threshold_distance(&mut self, metres: f32) {
        self.threshold_distance_squared = metres * metres;
    }
    pub fn threshold_distance(&self) -> f32 {
        self.threshold_distance_squared.sqrt()
    }
}
impl FollowPlayer {
    #[inline]
    pub fn update_throttle_ready(&self) -> bool {
        self.update_throttle_ready
    }
    #[inline]
    fn set_update_throttle_ready(&mut self) {
        self.update_throttle_ready = true;
    }
    #[inline]
    fn clear_update_throttle_ready(&mut self) {
        self.update_throttle_ready = false;
    }
}
#[cfg(todo)]
impl FollowPlayer {
    /// fuse semantics and whether [Sleep::is_elapsed] returns true prior to
    /// [Poll::Ready] are unclear, so fuse it ourselves
    pub fn update_throttle_ready(&self) -> bool {
        self.update_throttle_remaining() == 0
    }
    fn set_update_throttle_ready(&mut self) {
        self.update_throttle_tick = self.next_update_tick();
    }
    fn clear_update_throttle_ready(&mut self, ticks: u32) {
        self.read_update_throttle_tick();
        self.next_tick = self.update_throttle_tick.wrapping_add(ticks);
    }
    pub fn update_throttle_elapsed(&self) -> u32 {
        self.update_throttle_tick.wrapping_sub(self.last_tick)
    }
    pub fn update_throttle_remaining(&self) -> u32 {
        self.threshold_time_ticks.saturating_sub(self.update_throttle_elapsed())
    }
    pub fn next_update_tick(&self) -> u32 {
        let delay = match self.cached_tick {
            #[cfg(todo = "unnecessary")]
            0 => Self::IDLE_TIMEOUT_TICKS,
            _ => self.threshold_time_ticks,
        };
        self.last_tick.wrapping_add(delay)
    }
    pub fn threshold_interval_ticks_for(interval: Duration) -> u32 {
        const ML_INT: u32 = Self::MUMBLELINK_POS_INTERVAL.as_millis() as u32;
        let ms = interval.as_millis() as u32;
        ms.div_ceil(ML_INT)
    }
    fn read_update_throttle_tick(&mut self) -> u32 {
        let Some(ml) = self.ml else { return 0 };
        let prev_tick = mem::replace(&mut self.update_throttle_tick, ml.read_ui_tick());
        let delta = self.update_throttle_tick.wrapping_sub(prev_tick);
        delta
    }
    fn read_remaining_throttle_ticks(&mut self) -> u32 {
        match update_throttle_remaining() {
            0 => 0,
            rem =>
                rem.saturating_sub(self.read_update_throttle_tick()),
        }
    }
    pub fn poll_ready(&mut self, cx: &mut Context) -> Poll<()> {
        let rem = self.read_remaining_throttle_ticks();

        if rem > 0 {
            ready!(self.update_throttle.as_mut().poll(cx));
            // fuse/mark it as ready even if ml may not have ticked yet
            self.update_throttle_tick = self.update_throttle_tick.wrapping_add(rem);
        }
        Poll::Ready(())
    }
}
impl FollowPlayer {
    /// allow next event to be emitted immediately
    pub fn clear_throttle(&mut self) {
        #[cfg(todo = "unnecessary")]
        {
            self.last_emitted = WallInstant::past_instant().clone().into();
            self.update_throttle.as_mut().reset(self.last_emitted);
        }
        self.set_update_throttle_ready();
    }
    pub fn set_threshold_timeout(&mut self, interval: Duration) {
        if self.threshold_time == interval { return }
        self.threshold_time = interval;
        if !self.update_throttle_ready() && self.last_tick != 0 {
            self.reset_throttle_at(self.last_emitted + interval);
        }
        #[cfg(todo)]
        {
            self.threshold_time_ticks = Self::threshold_interval_ticks_for(self.threshold_time);
        }
    }
    pub fn readjust_now(&mut self) {
        self.readjust_after(Instant::now());
    }
    pub fn readjust_after(&mut self, when: Instant) {
        let Some(adj) = when.checked_duration_since(self.last_emitted) else {
            // went back in time?
            return
        };
        let deadline = self.update_throttle.deadline().max(when);
        self.reset_throttle_at(deadline + adj);
    }
    /// returns elapsed ticks since last read,
    /// so 0 indicates no change detected
    fn read_ml_tick(ml: MumblePtr, dest: &mut u32) -> u32 {
        let initial = *dest;
        for _ in 0..Self::ML_TICK_RETRY {
            let prev_tick = mem::replace(dest, ml.read_ui_tick());
            if prev_tick == *dest { break }
        }
        dest.wrapping_sub(initial)
    }
    /// just enough to double-check really
    const ML_TICK_RETRY: usize = 3;

    pub fn update_now(&mut self, consume: bool) -> Option<PlayerPosition> {
        self.update_at(Instant::now(), consume)
    }
    pub fn update_at(&mut self, when: Instant, consume: bool) -> Option<PlayerPosition> {
        let changed = self.update_from_mumblelink();
        if !self.has_data() { return None }
        let mut pos = None;
        if changed > 0 {
            if consume {
                pos = Some(self.commit_emit(when));
                self.reset_throttle_at(self.last_emitted + self.threshold_time);
            } else {
                self.cached_tick = self.cached_tick.wrapping_sub(changed);
                self.set_update_throttle_ready();
            }
        }
        Some(pos.unwrap_or(self.cached_pos))
    }
    pub(crate) fn update_from_mumblelink(&mut self) -> u32 {
        let Some(ml) = self.ml else { return 0 };
        let changed = Self::read_ml_tick(ml, &mut self.cached_tick);
        if changed > 0 {
            let pos = unsafe {
                let pos = &raw const (*ml.as_ptr()).avatar.position;
                ptr::read_volatile(pos)
            };
            self.cached_pos = Point3::from_array(pos);
        }
        changed
    }
    pub fn reset_throttle_at(&mut self, deadline: Instant) {
        self.update_throttle.as_mut().reset(deadline);
        self.clear_update_throttle_ready();
    }
    pub fn has_moved(&self) -> bool {
        self.cached_tick != self.last_tick && !vec_eq(self.cached_pos, self.last_seen)
            // && !self.cached_pos.x.is_infinite()
    }
    pub fn position_delta_delta_unchecked(&self) -> f32 {
        self.cached_pos.distance_squared(self.last_seen)
    }
    pub fn position_delta_delta(&self) -> Option<f32> {
        if self.cached_pos.x.is_infinite() || self.cached_tick == self.last_tick { return None }
        if self.last_seen.x.is_infinite() { return Some(f32::INFINITY) }
        Some(self.position_delta_delta_unchecked())
    }
    pub(crate) fn delta_is_update(delta: Option<f32>, threshold_distance_squared: f32) -> bool {
        match delta {
            None => return false,
            Some(d) if d.is_infinite() =>
                return true,
            Some(d) if d < threshold_distance_squared => false,
            d => d.is_some(),
        }
    }
    /// if so, make sure to wait for throttle!
    pub fn has_update(&self) -> bool {
        Self::delta_is_update(self.position_delta_delta(), self.threshold_distance_squared)
    }
    fn has_data(&self) -> bool {
        self.position_delta_delta().is_some()
    }
    fn update_overdue(&self, now: Instant) -> bool {
        if !self.has_moved() { return false }
        now.duration_since(self.last_emitted) > Self::IDLE_TIMEOUT
    }
    /// otherwise a recent update emitted means we're waiting for throttle to timeout
    pub fn poll_ready(&mut self, cx: &mut Context) -> Poll<()> {
        if !self.update_throttle_ready() {
            ready!(self.update_throttle.as_mut().poll(cx));
            self.set_update_throttle_ready();
        }
        Poll::Ready(())
    }
    fn poll_ready_ml(&mut self, _cx: &mut Context) -> Poll<()> {
        let changed = self.update_from_mumblelink();
        match changed {
            0 => Poll::Pending,
            _ => Poll::Ready(())
        }
    }
    pub fn commit_emit(&mut self, when: Instant) -> PlayerPosition {
        self.last_emitted = when;
        self.last_tick = self.cached_tick;
        self.last_seen = self.cached_pos;
        self.last_seen
    }
    pub fn poll_next_update(&mut self, cx: &mut Context) -> Poll<PlayerPosition> {
        ready!(self.poll_ready(cx));
        let ready = match self.poll_ready_ml(cx) {
            Poll::Ready(()) => Poll::Ready(self.has_update()),
            Poll::Pending => Poll::Pending,
        };
        let now = Instant::now();
        let overdue = match ready {
            Poll::Pending | Poll::Ready(false) => self.update_overdue(now),
            _ => false,
        };
        let res = match ready {
            _ if overdue => Poll::Ready(true),
            ready => ready,
        };
        let next_delay = match res {
            Poll::Ready(true) if !overdue =>
                self.threshold_time,
            Poll::Ready(..) =>
                Self::MUMBLELINK_POS_INTERVAL,
            Poll::Pending if self.last_tick == 0 =>
                // having never seen an update means we're not yet in-game
                Self::IDLE_TIMEOUT,
            Poll::Pending =>
                // try to catch next tick soon
                Self::MUMBLELINK_POS_INTERVAL / 2,
        };
        let res = match res {
            Poll::Ready(true) =>
                Poll::Ready(self.commit_emit(now)),
            _ => Poll::Pending,
        };
        self.reset_throttle_at(now + next_delay);
        if let Poll::Pending = &res {
            // TODO: is polling ready here necessary?
            // does Sleep::reset register the timer with the runtime/waker or no?
            if self.poll_ready(cx).is_ready() {
                // unlikely since we just reset the interval a moment ago...
                cx.waker().wake_by_ref();
            }
        }
        res
    }
}
/// see [Self::poll_next_update]
impl Stream for FollowPlayer {
    type Item = PlayerPosition;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        self.get_mut().poll_next_update(cx).map(Some)
    }
}
impl fmt::Debug for FollowPlayer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut f = f.debug_struct("FollowPlayer");
        let f = f.field("interval", &self.threshold_time)
            .field("threshold", &self.threshold_distance());
        match self.ml {
            Some(..) => f
                .field("last_update", &self.last_emitted)
                .field("ui_ticks_behind", &(self.cached_tick.wrapping_sub(self.last_tick))),
            None => f.field("last_update", &"uninitialized"),
        }.finish()
    }
}
impl Default for FollowPlayer {
    fn default() -> Self { Self::new() }
}
