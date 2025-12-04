use {
    self::pathing::{PathingFilterFlags, PathingSearchFlags},
    crate::exports::runtime::watched,
    serde::{Deserialize, Serialize},
};

pub mod pathing;

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
}
impl PathingWindowState {
    pub fn is_empty(&self) -> bool {
        let Self { search, filter } = self;
        search.is_empty() && filter.is_empty()
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
