use {
    self::{
        pathing::{PathingFilterFlags, PathingSearchFlags},
        interact::{InteractSortFlags, InteractFilterFlags},
    },
    serde::{Deserialize, Serialize},
    taimi_sync::watched,
};

pub mod pathing;
pub mod interact;

pub type UiState = Render2DState;

/// TODO: move to crate::settings::v2 or something, then alias latest here?
/// (also finish reviving src/settings/attempt)
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Render2DState {
    #[cfg(todo)]
    #[serde(default, skip_serializing_if = "PrimaryWindowState::is_empty")]
    pub primary_window: watch::Sender<PrimaryWindowState>,
    #[cfg(todo)]
    #[serde(default, skip_serializing_if = "TimersWindowState::is_empty")]
    pub timers_window: watch::Sender<TimersWindowState>,
    /// TODO: switch to [watched::Watcher]
    #[serde(
        default,
        skip_serializing_if = "Render2DState::is_empty_pathing",
        with = "watched::serde_imp::Sender"
    )]
    pub pathing_window: watched::Tx<PathingWindowState>,
}

#[cfg(todo)]
#[derive(Deserialize, Serialize, Default, Debug)]
pub struct PrimaryWindowState {
    #[serde(default, skip_serializing_if = "is_false")]
    pub open: bool,
}

#[cfg(todo)]
#[derive(Deserialize, Serialize, Default, Debug)]
pub struct TimersWindowState {
    #[serde(default, skip_serializing_if = "Render2DState::is_default_open")]
    pub open: bool,
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct PathingWindowState {
    #[cfg(todo)]
    #[serde(default, skip_serializing_if = "Render2DState::is_default_open")]
    pub open: bool,
    #[serde(default, skip_serializing_if = "PathingSearchState::is_empty")]
    pub search: PathingSearchState,
    #[serde(default, skip_serializing_if = "PathingFilterState::is_empty")]
    pub filter: PathingFilterState,
    #[serde(default, skip_serializing_if = "InteractPoiState::is_empty")]
    pub interact_pois: InteractPoiState,
}
impl PathingWindowState {
    pub fn is_empty(&self) -> bool {
        let Self { search, filter, interact_pois } = self;
        search.is_empty() & filter.is_empty() & interact_pois.is_empty()
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct PathingSearchState {
    #[serde(default, skip_serializing_if = "Render2DState::is_default_open")]
    pub open: bool,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub query: String,
    #[serde(default, skip_serializing_if = "PathingSearchFlags::is_empty")]
    pub flags: PathingSearchFlags,
}
impl PathingSearchState {
    pub fn query(&self) -> Option<&str> {
        match &self.query[..] {
            s if s.is_empty() => None,
            s => Some(s),
        }
    }

    pub fn is_empty(&self) -> bool {
        let Self {
            open: Render2DState::DEFAULT_OPEN,
            flags: PathingSearchFlags::DEFAULT,
            query,
        } = self
        else {
            return false
        };
        query.is_empty()
    }
}
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct PathingFilterState {
    #[serde(default, skip_serializing_if = "PathingFilterFlags::is_empty")]
    pub flags: PathingFilterFlags,
}
impl PathingFilterState {
    pub fn is_empty(&self) -> bool {
        let Self { flags: PathingFilterFlags::DEFAULT } = self else { return false };
        true
    }
}

/// TODO: sort order too (but maybe let imgui persist it for us and serde(skip))?
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct InteractPoiState {
    #[serde(default = "InteractFilterFlags::settings_default", skip_serializing_if = "InteractFilterFlags::is_settings_default")]
    pub flags: InteractFilterFlags,
    #[serde(default = "InteractSortFlags::settings_default", skip_serializing_if = "InteractSortFlags::is_settings_default")]
    pub sort: InteractSortFlags,
    #[serde(default = "InteractSortFlags::settings_default_descending", skip_serializing_if = "InteractSortFlags::is_settings_default_descending")]
    pub sort_desc: InteractSortFlags,
}
impl InteractPoiState {
    pub fn is_empty(&self) -> bool {
        let Self { flags: InteractFilterFlags::DEFAULT_UI, sort: InteractSortFlags::DEFAULT_UI, sort_desc: InteractSortFlags::DEFAULT_UI_DESC } = self else { return false };
        true
    }
}
impl Default for InteractPoiState {
    fn default() -> Self {
        Self {
            flags: InteractFilterFlags::settings_default(),
            sort: InteractSortFlags::settings_default(),
            sort_desc: InteractSortFlags::settings_default_descending(),
        }
    }
}

impl Render2DState {
    pub const DEFAULT_OPEN: bool = false;

    pub fn is_empty(&self) -> bool {
        let Self { pathing_window } = self;
        #[cfg(todo)]
        let primary_window_empty = Self::is_empty_primary(primary_window);
        #[cfg(todo)]
        let timers_window_empty = Self::is_empty_timers(timers_window);
        #[cfg(todo)]
        let markers_window_empty = Self::is_empty_markers(markers_window);
        let (primary_window_empty, timers_window_empty, markers_window_empty) = (true, true, true);
        let pathing_window_empty = Self::is_empty_pathing(pathing_window);
        primary_window_empty && timers_window_empty && markers_window_empty && pathing_window_empty
    }

    fn is_default_open(v: &bool) -> bool {
        !*v
    }
    fn is_empty_pathing(pathing: &watched::Tx<PathingWindowState>) -> bool {
        pathing.borrow().is_empty()
    }
    #[cfg(todo)]
    fn is_empty_pathing(pathing: &Watcher<PathingWindowState>) -> bool {
        pathing.try_read().map(|w| w.is_empty()).unwrap_or(false)
    }

    pub fn is_dirty(&self) -> bool {
        #[cfg(todo)]
        if self.pathing_window.watch.has_changed() {
            return true
        }
        false
    }
    pub fn mark_clean(&self) {
        //self.pathing_window.watch.mark_unchanged);
    }
}
