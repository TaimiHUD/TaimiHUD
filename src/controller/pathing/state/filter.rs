use std::{collections::BTreeSet, mem, fmt, num::NonZero, ops, hash::Hash, sync::{Arc, LazyLock}};
use std::marker::PhantomData;
use crate::{
    controller::pathing::{
        state::hidden::MarkerState,
        registry::PackInfoSignature,
        info::MapPackInfo,
    },
    controller::api::{AchievementState as ApiAchievementState, SharedAchievementState, SharedRaidState},
    render::machine::MumbleIdentityUpdate,
    exports::runtime as rt,
};
use taimi_meta::packs::{collections::MarkerSet, PoiPath, TrailPath, MapIndex, MarkerId, MarkerIndex, MarkerPath};
use taimi_pack::attributes::{self as attr, keys::{self, Guid}, FilterAttributes, Festivals};
use taimi_pack::Pack;
use taimi_hoard::collections::TaimiSet;
use taimi_sync::arcs::ArcPtrCmp;
#[cfg(feature = "paths-schedule")]
use {
    chrono::{DateTime, TimeDelta},
    croner::errors::CronError,
    std::time::Duration,
};

pub const FILTER_HIDDEN: Option<bool> = Some(false);
pub const FILTER_ALLOWED: Option<bool> = None;
#[cfg(todo = "unused")]
pub const FILTER_VISIBLE_OVERRIDE: Option<bool> = Some(true);
pub type FilterAllow = Option<bool>;

/// TODO: hopefully can replacing this with a collection/typeset of [keys] soon
#[derive(Debug, Clone, Default)]
pub struct FilterConfig {
    /// TODO: probably make this field Arc on upstream [attr::MarkerAttributes]
    filters: FilterAttributes,
    #[cfg(feature = "paths-schedule")]
    schedule: Option<ScheduleConfig>,
}
impl FilterConfig {
    pub fn from_attributes(filters: FilterAttributes) -> Self {
        #[cfg(todo = "unnecessary")]
        if Self::filters_is_empty(&filters) { return None }
        #[cfg(feature = "paths-schedule")]
        let schedule = rt::log::warn_ok(ScheduleConfig::from_attributes(&filters)).flatten();
        Self {
            filters,
            #[cfg(feature = "paths-schedule")]
            schedule,
        }
    }

    pub fn is_empty(&self) -> bool {
        Self::filters_is_empty(&self.filters)
    }
    pub(crate) fn filters_is_empty(filters: &FilterAttributes) -> bool {
        if !filters.festivals().is_empty() { return false }
        if !filters.mounts().is_empty() { return false }
        if !filters.professions().is_empty() { return false }
        if !filters.races().is_empty() { return false }
        if !filters.specializations().is_empty() { return false }
        if !filters.map_types().is_empty() { return false }
        #[cfg(feature = "paths-schedule")]
        if !filters.schedule().is_empty() { return false }
        if !filters.raids().is_empty() { return false }
        if !filters.achievement_id().is_none() { return false }
        true
    }
    fn filters_achievement(filters: &FilterAttributes) -> Option<AchievementConfig> {
        filters.achievement_id().map(|id| AchievementConfig {
            id: id.into(),
            bit: filters.achievement_bit().map(Into::into),
        })
    }
    /// may not include full functionality of [FilterConfig] that requires
    /// pre-processing (atm this means cron schedule and achievementid)
    fn filters_from<'a, S: 'a>(filters: &'a FilterAttributes) -> impl Iterator<Item = &'a (dyn MarkerFilter<State = S> + 'a)> where
        S: AsRef<Festivals>,
        S: AsRef<AvatarMetadata>,
        S: AsRef<CharacterMetadata>,
        S: AsRef<MapMetadata>,
        S: AsRef<RaidState>,
        //S: AsRef<AchievementState>,
    {
        let specializations = filters.specializations();
        let map_types = filters.map_types();
        let raids = filters.raids();
        IntoIterator::into_iter([
            filters.festivals.as_ref()
                .map(FilterFor::<_, S>::dyn_from_ref),
            filters.mounts.as_ref()
                .map(FilterFor::<_, S>::dyn_from_ref),
            filters.professions.as_ref()
                .map(FilterFor::<_, S>::dyn_from_ref),
            filters.races.as_ref()
                .map(FilterFor::<_, S>::dyn_from_ref),
            //(!specializations.is_empty()).then_some(specializations).map(FilterFor::<_, S>::dyn_from_ref),
            //(!map_types.is_empty()).then_some(map_types).map(FilterFor::<_, S>::dyn_from_ref),
            filters.specializations.as_ref().map(keys::Specializations::from_attrlist)
                .map(FilterFor::<_, S>::dyn_from_ref),
            filters.map_types.as_ref().map(keys::MapTypes::from_attrlist)
                .map(FilterFor::<_, S>::dyn_from_ref),
            filters.raids.as_ref().map(keys::Raids::from_attrlist)
                .map(FilterFor::<_, S>::dyn_from_ref),
            //(!raids.is_empty()).then_some(raids).map(FilterFor::<_, S>::dyn_from_ref),
            #[cfg(todo)]
            filters.achievement_id().map(|id| AchievementConfig {
                id,
                bit: filters.achievement_bit(),
            }),
        ]).flatten()
    }
    pub fn filters_is_visible(filters: &FilterAttributes, state: &FilterState) -> FilterAllow {
        let achievement = Self::filters_achievement(filters);
        let mut filters = Self::filters_from::<FilterState>(filters)
            .chain(achievement.as_ref().map(FilterFor::<_, FilterState>::dyn_from_ref));

        match filters.all(|f| f.is_visible(state) != FILTER_HIDDEN) {
            true => FILTER_ALLOWED,
            false => FILTER_HIDDEN,
        }
    }
}
impl MarkerFilter for FilterConfig {
    type State = FilterState;

    fn is_visible(&self, state: &Self::State) -> FilterAllow {
        Self::filters_is_visible(&self.filters, state)
    }
}

#[derive(Debug, Clone, Default)]
pub struct RaidState {
    pub completed: BTreeSet<keys::Raid>,
}
impl RaidState {
    pub fn new<I>(completed: I) -> Self where
        I: IntoIterator,
        I::Item: Into<keys::Raid>,
    {
        Self {
            completed: completed.into_iter().map(Into::into).collect(),
        }
    }

    pub fn update_with(&mut self, raids: &SharedRaidState) -> bool {
        let mut dirty = false;
        self.completed.retain(|raid| {
            let keep = raids.contains(raid);
            if !keep { dirty = true; }
            keep
        });

        for raid in raids.iter() {
            if self.completed.contains(raid) {
                continue
            }
            dirty |= self.completed.insert(raid.into());
        }

        dirty
    }
}

/// TODO: consider more than just a dumb clone
#[derive(Debug, Clone, Default)]
pub struct AchievementState {
    pub status: SharedAchievementState,
    pub hash: u32,
}
impl AchievementState {
    pub fn new(status: impl Into<SharedAchievementState>) -> Self {
        Self {
            status: status.into(),
            hash: PackInfoSignature::EMPTY.hash,
        }
    }

    pub fn update_with(&mut self, new: &SharedAchievementState) -> bool {
        if Arc::ptr_eq(&self.status, new) { return false }

        let prev_hash = mem::replace(&mut self.hash, Self::hash_state(new));
        prev_hash == 0 || prev_hash != self.hash
    }

    fn hash_state(status: &ApiAchievementState) -> u32 {
        PackInfoSignature::hash_with(|h| {
            status.hash(h)
        }).hash
    }
}
#[derive(Debug, Clone)]
pub struct AchievementConfig {
    pub id: keys::AchievementId,
    pub bit: Option<keys::AchievementBit>,
}
impl AchievementConfig {
    pub fn from_attributes(attrs: &FilterAttributes) -> Option<Self> {
        attrs.achievement_id.map(|id| Self {
            id: id.into(),
            bit: attrs.achievement_bit.map(keys::AchievementBit::from),
        })
    }

    pub fn is_complete(&self, state: &AchievementState) -> bool {
        self.bit
            .and_then(|bit| state.status.is_bit_complete(self.id.0, bit.0))
            .unwrap_or_else(|| state.status.is_complete(self.id.0))
    }
}
impl MarkerFilter for AchievementConfig {
    type State = AchievementState;

    fn is_visible(&self, state: &Self::State) -> FilterAllow {
        match self.is_complete(state) {
            true => FILTER_HIDDEN,
            false => FILTER_ALLOWED,
        }
    }
}

#[derive(Debug, Clone)]
#[cfg(feature = "paths-schedule")]
pub struct ScheduleConfig {
    pub pattern: croner::Cron,
    pub duration: keys::ScheduleDuration,
}
#[cfg(feature = "paths-schedule")]
pub type ScheduleTimezone = chrono::Utc;
#[cfg(feature = "paths-schedule")]
impl ScheduleConfig {
    pub fn from_attributes(attrs: &FilterAttributes) -> Result<Option<Self>, CronError> {
        let Some(schedule) = attrs.schedule.as_ref() else {
            return Ok(None)
        };

        let schedule = Self {
            pattern: schedule.parse()?,
            duration: attrs.schedule_duration.clone().map(Into::into).unwrap_or_default(),
        };
        Ok(Some(schedule))
    }
    pub fn duration(&self) -> Duration {
        Duration::from_secs_f32(self.duration.into())
    }
    pub fn is_active(&self, now: &DateTime<ScheduleTimezone>) -> bool {
        let Some(remaining) = self.remaining_s(now) else { return false };
        remaining > 0.0
    }

    pub fn remaining_s(&self, now: &DateTime<ScheduleTimezone>) -> Option<f32> {
        let Some(elapsed) = self.elapsed(now) else { return None };
        let seconds = elapsed.num_seconds();
        if seconds < 0 {
            return None
        }

        Some(self.duration.0 - seconds as f32)
    }

    pub fn next_schedule_change(&self, now: &DateTime<ScheduleTimezone>) -> Option<DateTime<ScheduleTimezone>> {
        let start = self.pattern.find_previous_occurrence(now, true).ok()?;
        let end = TimeDelta::from_std(self.duration()).ok()
            .and_then(|duration| start.checked_add_signed(duration))?;
        if now < &end {
            return Some(end)
        }

        self.pattern.find_next_occurrence(now, false).ok()
    }

    pub fn elapsed(&self, now: &DateTime<ScheduleTimezone>) -> Option<TimeDelta> {
        let start = self.pattern.find_previous_occurrence(now, true).ok()?;
        Some(now.signed_duration_since(start))
    }
    pub fn latest_end<T>(&self, now: T) -> Result<DateTime<ScheduleTimezone>, CronError> where
        T: Into<DateTime<ScheduleTimezone>>,
    {
        let start = self.latest_start(now)?;
        Ok(start + self.duration())
    }

    pub fn latest_start<T>(&self, point: T) -> Result<DateTime<ScheduleTimezone>, CronError> where
        T: Into<DateTime<ScheduleTimezone>>,
    {
        self.pattern.find_previous_occurrence(&point.into(), true)
    }
    pub fn next_start<T>(&self, point: T) -> Result<DateTime<ScheduleTimezone>, CronError> where
        T: Into<DateTime<ScheduleTimezone>>,
    {
        self.pattern.find_next_occurrence(&point.into(), false)
    }
}
#[cfg(feature = "paths-schedule")]
impl MarkerFilter for ScheduleConfig {
    type State = ScheduleState;

    fn is_visible(&self, state: &Self::State) -> FilterAllow {
        match state.now.as_ref().map(|now| self.is_active(now)) {
            None => None,
            Some(true) => FILTER_ALLOWED,
            Some(false) => FILTER_HIDDEN,
        }
    }
}
#[derive(Debug, Clone, Default)]
#[cfg(feature = "paths-schedule")]
pub struct ScheduleState {
    pub now: Option<DateTime<ScheduleTimezone>>,
}
#[cfg(feature = "paths-schedule")]
impl ScheduleState {
    pub fn update_time(&mut self) {
        self.now = Some(ScheduleTimezone::now());
    }
}

#[cfg(feature = "paths-schedule")]
impl fmt::Display for ScheduleConfig {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} for ", self.pattern.describe())?;
        match TimeDelta::from_std(self.duration()) {
            Ok(d) => write!(f, "{d}"),
            Err(..) => write!(f, "{}S", self.duration.0),
        }
    }
}

pub trait MarkerFilter {
    type State: ?Sized;

    fn is_visible(&self, state: &Self::State) -> FilterAllow;
}

#[derive(Debug, Copy, Clone)]
pub struct GroupConfig {
    pub guid: Guid,
    pub inverted: bool,
}
impl GroupConfig {
    pub const EMPTY: Self = Self {
        guid: Guid::EMPTY,
        inverted: false,
    };
}
/// lazy hack .-.
impl MarkerFilter for GroupConfig {
    type State = FilterState;

    fn is_visible(&self, state: &Self::State) -> FilterAllow {
        let guid = MarkerId::from_uuid_ref(&self.guid.0);
        match state.hidden.is_hidden(guid, &state.map, &state.character) ^ self.inverted {
            true => FILTER_HIDDEN,
            false => FILTER_ALLOWED,
        }
    }
}

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HiddenAlways;
impl HiddenAlways {
    pub fn singleton() -> &'static Arc<Self> {
        static SINGLETON: LazyLock<Arc<HiddenAlways>> = LazyLock::new(|| Arc::new(HiddenAlways));
        &*SINGLETON
    }
}
impl MarkerFilter for HiddenAlways {
    type State = ();
    fn is_visible(&self, _state: &Self::State) -> FilterAllow {
        FILTER_HIDDEN
    }
}

/// Markers that can reappear on map change
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HiddenForMap {
    pub map: MapIndex,
    /// optionally tied to a specific instance
    pub shard: Option<NonZero<u32>>,
}
impl MarkerFilter for HiddenForMap {
    type State = MapMetadata;
    fn is_visible(&self, state: &Self::State) -> FilterAllow {
        match *self {
            Self { map, .. } if Some(map) != state.map_id =>
                FILTER_ALLOWED,
            Self { shard: Some(shard), .. } if shard.get() != state.shard_id =>
                FILTER_ALLOWED,
            _ => FILTER_HIDDEN,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HiddenForCharacter {
    pub name: Arc<[u8]>,
}
impl MarkerFilter for HiddenForCharacter {
    type State = CharacterMetadata;
    fn is_visible(&self, state: &Self::State) -> FilterAllow {
        if Arc::ptr_eq(&state.name, &self.name) {
            return FILTER_HIDDEN
        }
        match state.name == self.name {
            false => FILTER_ALLOWED,
            true => FILTER_HIDDEN,
        }
    }
}

fn any_or_empty<I: IntoIterator<Item = FilterAllow>>(filters: I) -> FilterAllow {
    let mut filters = filters.into_iter().peekable();
    if filters.peek().is_none() {
        return FILTER_ALLOWED
    }
    match filters.any(|f| f != FILTER_HIDDEN) {
        true => FILTER_ALLOWED,
        false => FILTER_HIDDEN,
    }
}
impl MarkerFilter for attr::Festival {
    type State = Festivals;
    fn is_visible(&self, state: &Self::State) -> FilterAllow {
        attr::Festivals::from(*self).is_visible(state)
    }
}
impl MarkerFilter for attr::Festivals {
    type State = Festivals;
    fn is_visible(&self, state: &Self::State) -> FilterAllow {
        match (*self).into_iter().find(|&flag| state.contains(flag)) {
            None if !self.is_empty() =>
                FILTER_HIDDEN,
            _ => FILTER_ALLOWED,
        }
    }
}
impl MarkerFilter for attr::Mount {
    type State = AvatarMetadata;
    fn is_visible(&self, state: &Self::State) -> FilterAllow {
        match state.mount.map(|mount| mount.get() == *self as u8) {
            Some(true) | None => FILTER_ALLOWED,
            Some(false) => FILTER_HIDDEN,
        }
    }
}
impl MarkerFilter for attr::Mounts {
    type State = <attr::Mount as MarkerFilter>::State;
    fn is_visible(&self, state: &Self::State) -> FilterAllow {
        if self.is_empty() { return FILTER_ALLOWED }
        match state.mount().map(|m| self.get(m)) {
            Some(true) => FILTER_ALLOWED,
            None | Some(false) => FILTER_HIDDEN,
        }
    }
}
impl MarkerFilter for keys::Mounts {
    type State = <attr::Mount as MarkerFilter>::State;
    fn is_visible(&self, state: &Self::State) -> FilterAllow {
        any_or_empty(self.0.iter_mounts().map(|mount| mount.is_visible(state)))
    }
}
impl MarkerFilter for attr::Profession {
    type State = CharacterMetadata;
    fn is_visible(&self, state: &Self::State) -> FilterAllow {
        match state.prof.map(|prof| prof as u8 == *self as u8) {
            Some(true) | None => FILTER_ALLOWED,
            Some(false) => FILTER_HIDDEN,
        }
    }
}
impl MarkerFilter for attr::Professions {
    type State = <attr::Profession as MarkerFilter>::State;
    fn is_visible(&self, state: &Self::State) -> FilterAllow {
        if self.is_empty() { return FILTER_ALLOWED }
        match state.prof.map(|p| self.get(p)) {
            Some(true) => FILTER_ALLOWED,
            None | Some(false) => FILTER_HIDDEN,
        }
    }
}
impl MarkerFilter for keys::Professions {
    type State = <attr::Profession as MarkerFilter>::State;
    fn is_visible(&self, state: &Self::State) -> FilterAllow {
        any_or_empty(self.0.iter_professions().map(|prof| prof.is_visible(state)))
    }
}
impl MarkerFilter for attr::Race {
    type State = CharacterMetadata;
    fn is_visible(&self, state: &Self::State) -> FilterAllow {
        match state.race.map(|race| race as u8 == *self as u8) {
            Some(true) | None => FILTER_ALLOWED,
            Some(false) => FILTER_HIDDEN,
        }
    }
}
impl MarkerFilter for attr::Races {
    type State = <attr::Race as MarkerFilter>::State;
    fn is_visible(&self, state: &Self::State) -> FilterAllow {
        if self.is_empty() { return FILTER_ALLOWED }
        match state.race.map(|r| self.get(r)) {
            Some(true) => FILTER_ALLOWED,
            None | Some(false) => FILTER_HIDDEN,
        }
    }
}
impl MarkerFilter for keys::Races {
    type State = <attr::Race as MarkerFilter>::State;
    fn is_visible(&self, state: &Self::State) -> FilterAllow {
        any_or_empty(self.0.iter_races().map(|race| race.is_visible(state)))
    }
}
impl MarkerFilter for attr::MapType {
    type State = MapMetadata;
    fn is_visible(&self, state: &Self::State) -> FilterAllow {
        match state.map_type == *self as i32 {
            true => FILTER_ALLOWED,
            false => FILTER_HIDDEN,
        }
    }
}
impl MarkerFilter for keys::MapTypes {
    type State = <attr::MapType as MarkerFilter>::State;
    fn is_visible(&self, state: &Self::State) -> FilterAllow {
        any_or_empty(self.iter().map(|map| map.is_visible(state)))
    }
}
impl MarkerFilter for keys::Specialization {
    type State = CharacterMetadata;
    fn is_visible(&self, state: &Self::State) -> FilterAllow {
        match state.spec.map(|spec| spec.get() == self.0 as u32) {
            Some(true) | None => FILTER_ALLOWED,
            Some(false) => FILTER_HIDDEN,
        }
    }
}
impl MarkerFilter for arcdps::Specialization {
    type State = <keys::Specialization as MarkerFilter>::State;
    fn is_visible(&self, state: &Self::State) -> FilterAllow {
        keys::Specialization((*self).into()).is_visible(state)
    }
}
impl MarkerFilter for keys::Specializations {
    type State = <keys::Specialization as MarkerFilter>::State;
    fn is_visible(&self, state: &Self::State) -> FilterAllow {
        any_or_empty(self.iter().map(|spec| spec.is_visible(state)))
    }
}
impl MarkerFilter for keys::Raid {
    type State = RaidState;
    fn is_visible(&self, state: &Self::State) -> FilterAllow {
        match state.completed.contains(self) {
            true => FILTER_HIDDEN,
            _ => FILTER_ALLOWED,
        }
    }
}
impl MarkerFilter for keys::Raids {
    type State = <keys::Raid as MarkerFilter>::State;
    fn is_visible(&self, state: &Self::State) -> FilterAllow {
        match self.iter().all(|raid| raid.is_visible(state) != FILTER_HIDDEN) {
            true => FILTER_ALLOWED,
            false => FILTER_HIDDEN,
        }
    }
}

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AvatarMetadata {
    pub mount: Option<NonZero<u8>>,
}
impl AvatarMetadata {
    /// TODO: forces use of gw2_mumble enum, bad idea!
    pub fn from_mumblelink_context(ml: &rt::MumblePtr) -> Self {
        let mount = NonZero::new(ml.read_mount_index() as u8);
        Self {
            mount,
        }
    }
    pub fn update_from_mumblelink_context(&mut self, ml: &rt::MumblePtr) {
        *self = Self::from_mumblelink_context(ml);
    }

    pub fn mount(&self) -> Option<attr::Mount> {
        let repr = self.mount.map(|m| m.get()).unwrap_or(0);
        let mount = attr::Mount::from_repr(repr);
        if mount.is_none() {
            log::info!("unrecognized mount #{repr}, please report this as a bug!");
        }
        mount
    }
}
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MapMetadata {
    pub map_id: Option<MapIndex>,
    pub shard_id: u32,
    pub map_type: i32,
}
impl MapMetadata {
    pub fn from_mumblelink_context(ml: &rt::MumblePtr) -> Self {
        let mut meta = Self::default();
        meta.update_from_mumblelink_context(ml);
        meta
    }
    pub fn update_from_mumblelink_context(&mut self, ml: &rt::MumblePtr) {
        self.map_id = NonZero::new(ml.read_map_id());
        self.shard_id = ml.read_shard_id();
        self.map_type = ml.read_map_type() as i32;
    }
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CharacterMetadata {
    pub race: Option<attr::Race>,
    pub prof: Option<attr::Profession>,
    pub spec: Option<NonZero<u32>>,
    pub name: Arc<[u8]>,
}
impl CharacterMetadata {
    pub fn from_mumblelink(id: &MumbleIdentityUpdate) -> Self {
        let mut meta = Self::default();
        meta.update_from_mumblelink(id);
        meta
    }
    pub fn update_from_mumblelink(&mut self, id: &MumbleIdentityUpdate) -> bool {
        let mut dirty = false;
        let prev_race = mem::replace(&mut self.race, (id.race as i32).try_into().ok());
        dirty |= prev_race != self.race;
        let prev_prof = mem::replace(&mut self.prof, (id.profession as i32).try_into().ok());
        dirty |= prev_prof != self.prof;
        let prev_spec = mem::replace(&mut self.spec, NonZero::new(id.specialization));
        dirty |= prev_spec != self.spec;
        let name_len = id.name.iter().position(|&c| c == 0)
            .unwrap_or(id.name.len());
        let name = unsafe { id.name.get_unchecked(..name_len) };
        if self.name.len() != name_len || name != &self.name[..] {
            self.name = name.into();
            dirty = true;
        }
        dirty
    }
}

#[derive(Debug, Clone, Default)]
pub struct FilterState {
    pub achievements: AchievementState,
    pub raids: RaidState,
    pub festival: Festivals,
    pub map: MapMetadata,
    pub character: CharacterMetadata,
    pub avatar: AvatarMetadata,
    #[cfg(feature = "paths-schedule")]
    pub schedule: ScheduleState,
    pub hidden: MarkerState,
}

pub type FilterStateFilter = Arc<dyn MarkerFilterState>;
pub trait MarkerFilterState: Send + Sync + 'static {
    fn is_filter_visible(&self, state: &FilterState) -> FilterAllow;
}
impl<T> MarkerFilterState for T where
    T: MarkerFilter + Send + Sync + 'static,
    FilterState: AsRef<<T as MarkerFilter>::State>,
{
    fn is_filter_visible(&self, state: &FilterState) -> FilterAllow {
        MarkerFilter::is_visible(self, state.as_ref())
    }
}
#[cfg(todo)]
pub type FilterStateFilter = Arc<dyn MarkerFilter<State = FilterState>>;
#[cfg(todo)]
impl<T> MarkerFilterState for T where
    T: MarkerFilter<State = FilterState>,
{
    // TODO: equality/dedupe checks?
}

#[derive(Debug, Clone, Default)]
#[repr(transparent)]
pub struct FilterFor<T: ?Sized, S: ?Sized = FilterState> {
    _state: PhantomData<fn(&S) -> FilterAllow>,
    filter: T,
}
impl<T: ?Sized, S: ?Sized> FilterFor<T, S> {
    #[inline]
    pub const fn new(filter: T) -> Self where T: Sized {
        Self {
            _state: PhantomData,
            filter,
        }
    }
    #[inline]
    pub const fn from_ref(filter: &T) -> &Self {
        unsafe { mem::transmute(filter) }
    }
    pub fn dyn_from_ref<'a>(filter: &'a T) -> &'a (dyn MarkerFilter<State = S> + 'a) where
        T: Sized + 'a,
        Self: MarkerFilter<State = S>,
    {
        Self::from_ref(filter)
    }
}
impl<T: ?Sized, S: ?Sized> MarkerFilter for FilterFor<T, S> where
    T: MarkerFilter,
    S: AsRef<<T as MarkerFilter>::State>,
{
    type State = S;
    fn is_visible(&self, state: &Self::State) -> FilterAllow {
        self.filter.is_visible(state.as_ref())
    }
}
impl AsRef<AchievementState> for FilterState {
    fn as_ref(&self) -> &AchievementState { &self.achievements }
}
impl AsRef<RaidState> for FilterState {
    fn as_ref(&self) -> &RaidState { &self.raids }
}
#[cfg(feature = "paths-schedule")]
impl AsRef<ScheduleState> for FilterState {
    fn as_ref(&self) -> &ScheduleState { &self.schedule }
}
impl AsRef<Festivals> for FilterState {
    fn as_ref(&self) -> &Festivals { &self.festival }
}
impl AsRef<MapMetadata> for FilterState {
    fn as_ref(&self) -> &MapMetadata { &self.map }
}
impl AsRef<CharacterMetadata> for FilterState {
    fn as_ref(&self) -> &CharacterMetadata { &self.character }
}
impl AsRef<AvatarMetadata> for FilterState {
    fn as_ref(&self) -> &AvatarMetadata { &self.avatar }
}
impl AsRef<()> for FilterState {
    fn as_ref(&self) -> &() { &() }
}
impl AsRef<FilterState> for FilterState {
    fn as_ref(&self) -> &FilterState { self }
}

#[derive(Debug, Clone, Default)]
pub struct MapFilters {
    pub pois: Vec<(PoiPath, FilterStateFilters)>,
    pub trails: Vec<(TrailPath, FilterStateFilters)>,
    #[cfg(feature = "paths-schedule")]
    pub schedules: Vec<(MarkerPath, Arc<ScheduleConfig>)>,
    pub achievements: Vec<(MarkerPath, Arc<AchievementConfig>)>,
    pub inversions: MarkerSet,
}

impl MapFilters {
    pub fn from_pack(info: &MapPackInfo, pack: &Pack) -> Self {
        #[cfg(feature = "paths-schedule")]
        let mut schedules = Vec::new();
        let mut achievements = Vec::new();
        let mut inversions = MarkerSet::default();
        // TODO: filter_map + filters.as_ref().map(FilterStateFilters::from_attributes) - repeat trails too
        let pois = info.pois()
            .filter_map(|path|
                pack.pois.get(path.path as usize).map(|t| (path, t))
            ).map(|(path, poi)| (path, FilterStateFilters::from_attributes(&poi.attributes.filters())))
            .filter(|(path, (f, extras))| {
                if let Some(GroupConfig { inverted: true, .. }) = extras.group {
                    inversions.insert_poi(*path);
                }
                !f.is_empty() || !extras.is_empty()
            })
            .map(|(path, (mut f, extras))| {
                if let Some(a) = extras.achievements {
                    achievements.push((MarkerPath::with_path(MarkerIndex::from(path)), a.clone()));
                    f.push(a as Arc<_>);
                }
                #[cfg(feature = "paths-schedule")]
                if let Some(s) = extras.schedule {
                    schedules.push((MarkerPath::with_path(MarkerIndex::from(path)), s.clone()));
                    f.push(s as Arc<_>);
                }
                (path, f)
            }).collect::<Vec<_>>();
        let trails = info.trails()
            .filter_map(|path|
                pack.trails.get(path.path as usize).map(|t| (path, t))
            ).map(|(path, trail)| (path, FilterStateFilters::from_attributes(&trail.attributes.filters())))
            .filter(|(path, (f, extras))| {
                if let Some(GroupConfig { inverted: true, .. }) = extras.group {
                    inversions.insert_trail(*path);
                }
                !f.is_empty() || !extras.is_empty()
            }).map(|(path, (mut f, extras))| {
                if let Some(a) = extras.achievements {
                    achievements.push((MarkerPath::with_path(MarkerIndex::from(path)), a.clone()));
                    f.push(a as Arc<_>);
                }
                #[cfg(feature = "paths-schedule")]
                if let Some(s) = extras.schedule {
                    schedules.push((MarkerPath::with_path(MarkerIndex::from(path)), s.clone()));
                    f.push(s as Arc<_>);
                }
                (path, f)
            }).collect::<Vec<_>>();
        achievements.shrink_to_fit();

        Self {
            pois,
            trails,
            #[cfg(feature = "paths-schedule")]
            schedules: {
                schedules.shrink_to_fit();
                schedules
            },
            achievements,
            inversions,
        }
    }

    pub fn group_filter_for<I>(&self, path: &I, guid: &Guid) -> Option<GroupConfig> where
        MarkerSet: TaimiSet<I>,
    {
        (!guid.is_empty()).then(move || GroupConfig {
            guid: *guid,
            inverted: self.inversions.set_contains(path),
        })
    }

    #[cfg(feature = "paths-schedule")]
    pub fn next_schedule_event(&mut self, now: &DateTime<ScheduleTimezone>) -> Option<DateTime<ScheduleTimezone>> {
        self.schedules.iter()
            .filter_map(|(_, schedule)| schedule.next_schedule_change(now))
            .min()
    }
}
#[derive(Clone, Default)]
pub struct FilterStateExtras {
    pub achievements: Option<Arc<AchievementConfig>>,
    pub group: Option<GroupConfig>,
    #[cfg(feature = "paths-schedule")]
    pub schedule: Option<Arc<ScheduleConfig>>,
}
impl FilterStateExtras {
    pub fn is_empty(&self) -> bool {
        match self {
            Self {
                achievements: None,
                group: None,
                #[cfg(feature = "paths-schedule")]
                schedule: None,
            } => true,
            _ => false,
        }
    }
}
#[derive(Clone, Default)]
pub struct FilterStateFilters {
    pub filters: Vec<FilterStateFilter>,
}

impl FilterStateFilters {
    pub fn from_attributes(filters: &FilterAttributes) -> (Self, FilterStateExtras) {
        let achievements = AchievementConfig::from_attributes(filters)
            .map(Arc::new);
        #[cfg(feature = "paths-schedule")]
        let schedule = rt::log::warn_ok(ScheduleConfig::from_attributes(filters)).flatten()
            .map(Arc::new);
        let festivals = filters.festivals.clone()
            .and_then(|f| match f.is_empty() {
                false => Some(f),
                true => None,
            })
            .map(|f| Arc::new(f) as Arc<dyn MarkerFilterState>);
        let mounts = filters.mounts.clone().map(keys::Mounts)
            .map(|f| Arc::new(f) as Arc<dyn MarkerFilterState>);
        let races = filters.races.clone().map(keys::Races)
            .map(|f| Arc::new(f) as Arc<dyn MarkerFilterState>);
        let professions = filters.professions.clone().map(keys::Professions)
            .map(|f| Arc::new(f) as Arc<dyn MarkerFilterState>);
        let specializations = filters.specializations.as_ref()
            .map(|s| s.iter().map(|&s| keys::Specialization(s as u32)).collect())
            .map(keys::Specializations)
            .map(|f| Arc::new(f) as Arc<dyn MarkerFilterState>);
        let raids = filters.raids.as_ref()
            .map(|s| s.iter().cloned().map(keys::Raid).collect())
            .map(keys::Raids)
            .map(|f| Arc::new(f) as Arc<dyn MarkerFilterState>);
        let group = match filters.invert_behavior {
            Some(true) => Some(GroupConfig {
                inverted: true,
                .. GroupConfig::EMPTY
            }),
            _ => None,
        };
        let filters = festivals.into_iter()
            .chain(mounts)
            .chain(races)
            .chain(professions)
            .chain(specializations)
            .chain(raids)
            .collect();

        let extras = FilterStateExtras {
            achievements,
            group,
            #[cfg(feature = "paths-schedule")]
            schedule,
        };
        (Self { filters }, extras)
    }
}

impl MarkerFilter for FilterStateFilters {
    type State = FilterState;

    fn is_visible(&self, state: &Self::State) -> FilterAllow {
        let filtered = FILTER_ALLOWED;
        for filter in &self.filters {
            match filter.is_filter_visible(state) {
                self::FILTER_ALLOWED => (),
                #[cfg(todo = "unnecessary")]
                f @ self::FILTER_HIDDEN => filtered = f,
                f => return f,
            }
        }
        filtered
    }
}
impl fmt::Debug for FilterStateFilters {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("FilterStateFilters")
            .field(&self.filters.len())
            .finish()
    }
}
impl ops::Deref for FilterStateFilters {
    type Target = Vec<FilterStateFilter>;
    fn deref(&self) -> &Self::Target {
        &self.filters
    }
}
impl ops::DerefMut for FilterStateFilters {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.filters
    }
}
