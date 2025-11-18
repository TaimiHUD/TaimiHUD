use std::{collections::{BTreeMap, BTreeSet}, fmt, num::NonZero, ops, sync::Arc, time::{Duration, SystemTime}};
use crate::settings::{pathing::PathingAchievementSave, state::SaveState};
use crate::exports::runtime::locator::LocationGet;
use crate::render::machine::MumbleIdentityUpdate;
use chrono::{DateTime, TimeDelta};
use croner::errors::CronError;
use crate::exports::runtime::{self as rt, Locator};
use taimi_pack::{attributes::{self as attr, keys::{self, Guid}}, MarkerAttributes};
use super::{festivals::Festivals, registry::{ActivePack, MapIndex, PackPoiNs, PackTrailNs, PoiIndex, PoiPath, TrailIndex, TrailPath}, FestivalState, MapPackInfo};

pub const FILTER_HIDDEN: Option<bool> = Some(false);
pub const FILTER_ALLOWED: Option<bool> = None;
#[cfg(todo = "unused")]
pub const FILTER_VISIBLE_OVERRIDE: Option<bool> = Some(true);
pub type FilterAllow = Option<bool>;
pub type PackMarkerNs = super::registry::PackRegistryNs;
pub type MarkerPath<N = PackMarkerNs> = Locator<N, MarkerIndex>;
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MarkerIndex {
    Poi(PoiIndex),
    Trail(TrailIndex),
}
impl From<PoiIndex> for MarkerIndex {
    fn from(i: PoiIndex) -> Self {
        Self::Poi(i)
    }
}
impl From<TrailIndex> for MarkerIndex {
    fn from(i: TrailIndex) -> Self {
        Self::Trail(i)
    }
}

#[derive(Debug, Clone, Default)]
pub struct MarkerSet {
    pub pois: BTreeSet<PoiPath>,
    pub trails: BTreeSet<TrailPath>,
}
impl MarkerSet {
    pub fn contains<N, I>(&self, marker: Locator<N, I>) -> bool where
        I: Into<MarkerIndex>,
    {
        match marker.path.into() {
            MarkerIndex::Poi(poi) => self.pois.contains(&Locator::with_path(poi)),
            MarkerIndex::Trail(trail) => self.trails.contains(&Locator::with_path(trail)),
        }
    }
    pub fn insert<N, I>(&mut self, marker: Locator<N, I>) -> bool where
        I: Into<MarkerIndex>,
    {
        match marker.path.into() {
            MarkerIndex::Poi(poi) => self.pois.insert(Locator::with_path(poi)),
            MarkerIndex::Trail(trail) => self.trails.insert(Locator::with_path(trail)),
        }
    }
}

/// TODO: lookup by guid will probably not be common until later..?
#[cfg(deleteme)]
#[derive(Debug, Clone, Default)]
pub struct MarkerGroups {
    pub groups: BTreeMap<Guid, MarkerSet>,
    pub inversions: MarkerSet,
}
#[cfg(deleteme)]
impl LocationGet<PackPoiNs, PoiIndex> for MarkerGroups {
    type LookupGet = Guid;
    fn lookup_get(&self, loc: &Locator<PackPoiNs, PoiIndex>) -> Option<Self::LookupGet> {
        self.groups.iter().find(|(_guid, markers)| markers.pois.contains(loc))
            .map(|(guid, ..)| guid.clone())
    }
}
#[cfg(deleteme)]
impl LocationGet<PackTrailNs, TrailIndex> for MarkerGroups {
    type LookupGet = Guid;
    fn lookup_get(&self, loc: &Locator<PackTrailNs, TrailIndex>) -> Option<Self::LookupGet> {
        self.groups.iter().find(|(_guid, markers)| markers.trails.contains(loc))
            .map(|(guid, ..)| guid.clone())
    }
}

/// TODO: probably just replacing this with a collection of [keys]
#[derive(Debug, Clone, Default)]
pub struct FilterConfig {
    pub achievement: Option<AchievementConfig>,
    pub schedule: Option<ScheduleConfig>,
    #[cfg(todo)]
    pub profession: Professions,
    #[cfg(todo)]
    pub specialization: Specialization,
    #[cfg(todo)]
    pub race: Race,
    #[cfg(todo)]
    pub mount: Mounts,
    #[cfg(todo)]
    pub festival: Festivals,
    #[cfg(todo)]
    pub raid: Raids,
    #[cfg(todo)]
    /// kinda is one?
    pub visibility: Option<VisibilityFlags>,
    /// this seems mostly pointless until scripting works
    #[cfg(todo)]
    pub map_type: keys::MapTypes,
}
impl FilterConfig {
    pub const EMPTY: Self = Self {
        achievement: None,
        schedule: None,
    };

    pub fn from_attributes(attrs: &MarkerAttributes) -> Self {
        let achievement = AchievementConfig::from_attributes(attrs);
        let schedule = rt::log::warn_ok(ScheduleConfig::from_attributes(attrs)).flatten();
        Self {
            achievement,
            schedule,
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Self {
                achievement: None,
                schedule: None,
            } => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct AchievementState {
    pub status: Arc<PathingAchievementSave>,
}
impl AchievementState {
    pub fn update_from_save(&mut self) {
        let acc = crate::ACCOUNT_NAME_CELL.get().map(|n| &n[..]);
        SaveState::read_with(|s| if let Some(p) = &s.pathing_state {
            let a = p.per_account.get(acc.unwrap_or(""))
                .or_else(|| if p.per_account.len() <= 1 {
                    p.per_account.values().next()
                } else { None });

            if let Some(a) = a {
                if !Arc::ptr_eq(&self.status, &a.achievements) {
                    self.status = a.achievements.clone();
                }
            }
        });
    }
}
#[derive(Debug, Clone)]
pub struct AchievementConfig {
    pub id: keys::AchievementId,
    pub bit: Option<keys::AchievementBit>,
}
impl AchievementConfig {
    pub fn from_attributes(attrs: &MarkerAttributes) -> Option<Self> {
        attrs.achievement_id.map(|id| Self {
            id: id.into(),
            bit: attrs.achievement_bit.map(keys::AchievementBit::from),
        })
    }

    pub fn is_complete(&self, state: &AchievementState) -> bool {
        if state.status.completed.contains(&self.id.0) {
            return true
        }
        if let Some(bit) = self.bit {
            if let Some(progress) = state.status.progress.get(&self.id.0) {
                return progress.bit_complete(bit.0)
            }
        }
        false
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
pub struct ScheduleConfig {
    pub pattern: croner::Cron,
    pub duration: keys::ScheduleDuration,
}
pub type ScheduleTimezone = chrono::Utc;
impl ScheduleConfig {
    pub fn from_attributes(attrs: &MarkerAttributes) -> Result<Option<Self>, CronError> {
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
pub struct ScheduleState {
    pub now: Option<DateTime<ScheduleTimezone>>,
}
impl ScheduleState {
    pub fn update_time(&mut self) {
        self.now = Some(ScheduleTimezone::now());
    }
}

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

#[cfg(deleteme)]
pub type MarkerGuid<N = PackMarkerNs> = (Locator<MarkerPath<N>, MarkerIndex>, Guid);
#[cfg(deleteme)]
impl MarkerFilter for MarkerGuid {
    type State = MarkerGroups;

    fn is_visible(&self, state: &Self::State) -> FilterAllow {
        let &(index, ref guid) = self;
        let inverted = state.inversions.contains(index);
        let _markers = match state.groups.get(guid) {
            Some(group) => group,
            None => return match inverted {
                true => FILTER_HIDDEN,
                false => FILTER_ALLOWED,
            },
        };
        #[cfg(debug_assertions)]
        debug_assert!(_markers.contains(index));
        match inverted {
            true => FILTER_ALLOWED,
            false => FILTER_HIDDEN,
        }
    }
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
    type State = BTreeMap<Guid, u64>;

    fn is_visible(&self, state: &Self::State) -> FilterAllow {
        match state.contains_key(&self.guid) ^ self.inverted {
            true => FILTER_HIDDEN,
            false => FILTER_ALLOWED,
        }
    }
}

impl MarkerFilter for attr::Festival {
    type State = FestivalState;
    fn is_visible(&self, state: &Self::State) -> FilterAllow {
        Festivals::from(*self).is_visible(state)
    }
}
impl MarkerFilter for Festivals {
    type State = FestivalState;
    fn is_visible(&self, state: &Self::State) -> FilterAllow {
        let state = state.get();
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
impl MarkerFilter for keys::Mounts {
    type State = <attr::Mount as MarkerFilter>::State;
    fn is_visible(&self, state: &Self::State) -> FilterAllow {
        match self.iter().all(|mount| mount.is_visible(state) != FILTER_HIDDEN) {
            true => FILTER_ALLOWED,
            false => FILTER_HIDDEN,
        }
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
impl MarkerFilter for keys::Professions {
    type State = <attr::Profession as MarkerFilter>::State;
    fn is_visible(&self, state: &Self::State) -> FilterAllow {
        match self.iter().all(|prof| prof.is_visible(state) != FILTER_HIDDEN) {
            true => FILTER_ALLOWED,
            false => FILTER_HIDDEN,
        }
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
impl MarkerFilter for keys::Races {
    type State = <attr::Race as MarkerFilter>::State;
    fn is_visible(&self, state: &Self::State) -> FilterAllow {
        match self.iter().all(|race| race.is_visible(state) != FILTER_HIDDEN) {
            true => FILTER_ALLOWED,
            false => FILTER_HIDDEN,
        }
    }
}
impl MarkerFilter for attr::MapType {
    type State = MapMetadata;
    fn is_visible(&self, state: &Self::State) -> FilterAllow {
        log::debug!("TODO: MarkerFilter for MapType"); FILTER_ALLOWED
    }
}
impl MarkerFilter for keys::MapTypes {
    type State = <attr::MapType as MarkerFilter>::State;
    fn is_visible(&self, state: &Self::State) -> FilterAllow {
        match self.iter().all(|map| map.is_visible(state) != FILTER_HIDDEN) {
            true => FILTER_ALLOWED,
            false => FILTER_HIDDEN,
        }
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
        match self.iter().all(|spec| spec.is_visible(state) != FILTER_HIDDEN) {
            true => FILTER_ALLOWED,
            false => FILTER_HIDDEN,
        }
    }
}
impl MarkerFilter for keys::Raid {
    type State = MapMetadata;
    fn is_visible(&self, state: &Self::State) -> FilterAllow {
        log::debug!("TODO: MarkerFilter for Raid"); FILTER_ALLOWED
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
}
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MapMetadata {
    pub raid: i32,
    pub map_type: i32,
}
impl MapMetadata {
    pub fn from_mumblelink_context(ml: &rt::MumblePtr) -> Self {
        let map_type = ml.read_map_type() as i32;
        Self {
            map_type,
            raid: Default::default(),
        }
    }
    pub fn update_from_mumblelink_context(&mut self, ml: &rt::MumblePtr) {
        let Self { map_type, raid: _ } = Self::from_mumblelink_context(ml);
        self.map_type = map_type;
    }
}
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
pub struct CharacterMetadata {
    pub race: Option<attr::Race>,
    pub prof: Option<attr::Profession>,
    pub spec: Option<NonZero<u32>>,
}
impl CharacterMetadata {
    pub fn from_mumblelink(id: &MumbleIdentityUpdate) -> Self {
        let race = (id.race as i32).try_into().ok();
        let prof = (id.profession as i32).try_into().ok();
        let spec = NonZero::new(id.specialization);
        Self {
            race,
            prof,
            spec,
        }
    }
    pub fn update_from_mumblelink(&mut self, id: &MumbleIdentityUpdate) {
        *self = Self::from_mumblelink(id);
    }
}

#[derive(Debug, Clone, Default)]
pub struct FilterState {
    #[cfg(deleteme)]
    pub groups: MarkerGroups,
    pub achievements: AchievementState,
    pub festival: FestivalState,
    pub map: MapMetadata,
    pub character: CharacterMetadata,
    pub avatar: AvatarMetadata,
    pub schedule: ScheduleState,
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

#[cfg(todo)]
#[derive(Debug, Clone, Default)]
pub struct FilterFor<T>(pub T);
#[cfg(todo)]
impl<T> MarkerFilter for FilterFor<T> where
    T: MarkerFilter,
    FilterState: AsRef<<T as MarkerFilter>::State>,
{
    type State = FilterState;
    fn is_visible(&self, state: &Self::State) -> FilterAllow {
        self.0.is_visible(state.as_ref())
    }
}
#[cfg(deleteme)]
impl AsRef<MarkerGroups> for FilterState {
    fn as_ref(&self) -> &MarkerGroups { &self.groups }
}
impl AsRef<AchievementState> for FilterState {
    fn as_ref(&self) -> &AchievementState { &self.achievements }
}
impl AsRef<ScheduleState> for FilterState {
    fn as_ref(&self) -> &ScheduleState { &self.schedule }
}
impl AsRef<FestivalState> for FilterState {
    fn as_ref(&self) -> &FestivalState { &self.festival }
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

#[derive(Debug, Clone, Default)]
pub struct MapFilters {
    pub pois: Vec<(PoiPath, FilterStateFilters)>,
    pub trails: Vec<(TrailPath, FilterStateFilters)>,
    pub schedules: Vec<(MarkerPath, Arc<ScheduleConfig>)>,
    pub achievements: Vec<(MarkerPath, Arc<AchievementConfig>)>,
    pub inversions: MarkerSet,
}

impl MapFilters {
    pub fn from_pack(info: &MapPackInfo, active: &ActivePack) -> Self {
        let mut schedules = Vec::new();
        let mut achievements = Vec::new();
        let mut inversions = MarkerSet::default();
        let pois = info.pois()
            .filter_map(|path|
                active.pack.pois.get(path.path as usize).map(|t| (path, t))
            ).map(|(path, poi)| (path, FilterStateFilters::from_attributes(&poi.attributes)))
            .filter(|(path, (f, (a, s, g)))| {
                if let Some(GroupConfig { inverted: true, .. }) = g {
                    inversions.insert(*path);
                }
                !f.is_empty() || a.is_some() || s.is_some()
            })
            .map(|(path, (mut f, (a, s, _g)))| {
                if let Some(a) = a {
                    achievements.push((MarkerPath::with_path(MarkerIndex::from(path.path)), a.clone()));
                    f.push(a as Arc<_>);
                }
                if let Some(s) = s {
                    schedules.push((MarkerPath::with_path(MarkerIndex::from(path.path)), s.clone()));
                    f.push(s as Arc<_>);
                }
                (path, f)
            }).collect::<Vec<_>>();
        let trails = info.trails()
            .filter_map(|path|
                active.pack.trails.get(path.path as usize).map(|t| (path, t))
            ).map(|(path, trail)| (path, FilterStateFilters::from_attributes(&trail.attributes)))
            .filter(|(path, (f, (a, s, g)))| {
                if let Some(GroupConfig { inverted: true, .. }) = g {
                    inversions.insert(*path);
                }
                !f.is_empty() || a.is_some() || s.is_some()
            }).map(|(path, (mut f, (a, s, _g)))| {
                if let Some(a) = a {
                    achievements.push((MarkerPath::with_path(MarkerIndex::from(path.path)), a.clone()));
                    f.push(a as Arc<_>);
                }
                if let Some(s) = s {
                    schedules.push((MarkerPath::with_path(MarkerIndex::from(path.path)), s.clone()));
                    f.push(s as Arc<_>);
                }
                (path, f)
            }).collect::<Vec<_>>();
        schedules.shrink_to_fit();
        achievements.shrink_to_fit();

        Self {
            pois,
            trails,
            schedules,
            achievements,
            inversions,
        }
    }

    pub fn group_filter_for<N, I>(&self, path: Locator<N, I>, guid: &Guid) -> Option<GroupConfig> where
        I: Into<MarkerIndex>,
    {
        (!guid.is_empty()).then(move || GroupConfig {
            guid: *guid,
            inverted: self.inversions.contains(path),
        })
    }

    pub fn next_schedule_event(&mut self, now: &DateTime<ScheduleTimezone>) -> Option<DateTime<ScheduleTimezone>> {
        self.schedules.iter()
            .filter_map(|(_, schedule)| schedule.next_schedule_change(now))
            .min()
    }
}

#[derive(Clone, Default)]
pub struct FilterStateFilters {
    pub filters: Vec<FilterStateFilter>,
}

impl FilterStateFilters {
    pub fn from_attributes(attrs: &MarkerAttributes) -> (Self, (Option<Arc<AchievementConfig>>, Option<Arc<ScheduleConfig>>, Option<GroupConfig>)) {
        let achievements = AchievementConfig::from_attributes(attrs)
            .map(Arc::new);
        let schedule = rt::log::warn_ok(ScheduleConfig::from_attributes(attrs)).flatten()
            .map(Arc::new);
        let festivals = attrs.festivals.as_ref().map(|f| f.iter().copied().collect::<Festivals>())
            .and_then(|f| match f.is_empty() {
                false => Some(f),
                true => None,
            })
            .map(|f| Arc::new(f) as Arc<dyn MarkerFilterState>);
        let mounts = attrs.mounts.clone().map(keys::List::from_iter).map(keys::Mounts::from)
            .map(|f| Arc::new(f) as Arc<dyn MarkerFilterState>);
        let races = attrs.races.clone().map(keys::List::from_iter).map(keys::Races::from)
            .map(|f| Arc::new(f) as Arc<dyn MarkerFilterState>);
        let professions = attrs.professions.clone().map(keys::List::from_iter).map(keys::Professions::from)
            .map(|f| Arc::new(f) as Arc<dyn MarkerFilterState>);
        let specializations = attrs.specializations.as_ref()
            .map(|s| s.iter().map(|&s| keys::Specialization(s as u32)).collect())
            .map(keys::Specializations)
            .map(|f| Arc::new(f) as Arc<dyn MarkerFilterState>);
        let raids = attrs.raids.as_ref()
            .map(|s| s.iter().cloned().map(keys::Raid).collect())
            .map(keys::Raids)
            .map(|f| Arc::new(f) as Arc<dyn MarkerFilterState>);
        let filters = festivals.into_iter()
            .chain(mounts)
            .chain(races)
            .chain(professions)
            .chain(specializations)
            .chain(raids)
            .collect();
        let group = match attrs.invert_behavior {
            Some(true) => Some(GroupConfig {
                inverted: true,
                .. GroupConfig::EMPTY
            }),
            _ => None,
        };

        (Self { filters }, (achievements, schedule, group))
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

pub enum BehaviourReset {
    LeaveRange,
    MapChange {
        id: MapIndex,
    },
    MapInstance {
        id: MapIndex,
        shard: u32,
    },
    DailyCharacter {
        character: String,
    },
}
