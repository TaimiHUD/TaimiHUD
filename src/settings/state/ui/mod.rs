use {
    self::{
        interact::{InteractFilterFlags, InteractSortFlags},
        pathing::{PathingFilterFlags, PathingSearchFlags},
        window::WindowState,
    },
    serde::{Deserialize, Serialize},
    taimi_hoard::is_false_ref,
    taimi_sync::watched::Watcher,
};

pub use self::{
    coords::UiVec2,
    window::{AnchorPosition, WindowOpen},
};

pub mod coords;
pub mod interact;
pub mod pathing;
pub mod window;

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
    #[serde(default = "Watcher::new_default")]
    pub message_window: Watcher<MessageWindowState>,
    #[serde(default = "Watcher::new_default")]
    pub pathing_window: Watcher<PathingWindowState>,
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
    #[serde(default, skip_serializing_if = "WindowState::is_empty")]
    pub window: WindowState,
    #[serde(default, skip_serializing_if = "PathingWindowTab::is_empty")]
    pub tab: PathingWindowTab,
    #[serde(default, skip_serializing_if = "PathingSearchState::is_empty")]
    pub search: PathingSearchState,
    #[serde(default, skip_serializing_if = "PathingFilterState::is_empty")]
    pub filter: PathingFilterState,
    #[serde(default, skip_serializing_if = "InteractPoiState::is_empty")]
    pub interact_pois: InteractPoiState,
}
impl PathingWindowState {
    pub const MIN_SIZE: UiVec2 = UiVec2::new(192.0, 96.0);
    pub const DEFAULT_SIZE: UiVec2 = UiVec2::new(300.0, 200.0);
    pub fn window_size(&self) -> &UiVec2 {
        self.window.size.get().unwrap_or(&Self::DEFAULT_SIZE)
    }
    pub fn set_window_size(&mut self, size: UiVec2) {
        self.window.size = match size {
            size if size == Self::DEFAULT_SIZE => None,
            size => Some(size),
        }
        .unwrap_or_default()
    }
}
#[derive(Deserialize, Serialize, Debug, Copy, Clone, Default, Eq, PartialOrd, Ord, Hash)]
pub struct PathingWindowTab {
    #[serde(default, skip_serializing_if = "is_false_ref")]
    pub packs: bool,
    #[serde(default, skip_serializing_if = "is_false_ref")]
    pub pois: bool,
    #[cfg(feature = "paths-edit")]
    #[serde(skip)]
    pub edit: bool,
}
impl PathingWindowTab {
    pub const INDEX_PACKS: usize = 0;
    pub const INDEX_POIS: usize = 1;
    #[cfg(feature = "paths-edit")]
    pub const INDEX_EDIT: usize = 2;

    pub const fn index(&self) -> usize {
        match self {
            Self { packs: true, .. } => 0,
            Self { pois: true, .. } => 1,
            Self { edit: true, .. } => 2,
            _ => 0,
        }
    }
    pub fn selected(&self, index: usize) -> bool {
        self.index() == index
    }
    pub fn focus(&mut self, index: usize) {
        match index {
            #[cfg(feature = "paths-edit")]
            Self::INDEX_EDIT => *self = Self { edit: true, ..Default::default() },
            Self::INDEX_POIS => *self = Self { pois: true, ..Default::default() },
            _ => *self = Self { packs: true, ..Default::default() },
        }
    }
    pub const fn selected_packs(&self) -> bool {
        self.index() == Self::INDEX_PACKS
    }
    pub const fn selected_pois(&self) -> bool {
        match self {
            #[cfg(todo = "unnecessary")]
            tab => tab.index() == Self::INDEX_POIS,
            tab => tab.pois,
        }
    }
    #[cfg(feature = "paths-edit")]
    pub const fn selected_edit(&self) -> bool {
        self.edit
    }
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.index() == 0
    }
    pub fn focus_packs(&mut self) {
        *self = Self { packs: true, ..Default::default() };
    }
    pub fn focus_pois(&mut self) {
        *self = Self { pois: true, ..Default::default() };
    }
    #[cfg(feature = "paths-edit")]
    pub fn focus_edit(&mut self) {
        *self = Self { edit: true, ..Default::default() };
    }
}
impl PartialEq for PathingWindowTab {
    fn eq(&self, rhs: &Self) -> bool {
        self.index() == rhs.index()
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
    #[serde(
        default = "InteractFilterFlags::settings_default",
        skip_serializing_if = "InteractFilterFlags::is_settings_default"
    )]
    pub flags: InteractFilterFlags,
    #[serde(
        default = "InteractSortFlags::settings_default",
        skip_serializing_if = "InteractSortFlags::is_settings_default"
    )]
    pub sort: InteractSortFlags,
    #[serde(
        default = "InteractSortFlags::settings_default_descending",
        skip_serializing_if = "InteractSortFlags::is_settings_default_descending"
    )]
    pub sort_desc: InteractSortFlags,
}
impl InteractPoiState {
    pub fn is_empty(&self) -> bool {
        let Self {
            flags: InteractFilterFlags::DEFAULT_UI,
            sort: InteractSortFlags::DEFAULT_UI,
            sort_desc: InteractSortFlags::DEFAULT_UI_DESC,
        } = self
        else {
            return false
        };
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

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct MessageWindowState {
    #[serde(default, skip_serializing_if = "WindowState::is_empty")]
    pub window: WindowState,
}
impl MessageWindowState {
    pub const MIN_SIZE: UiVec2 = UiVec2::new(64.0, 96.0);
    pub const DEFAULT_SIZE: UiVec2 = UiVec2::new(288.0, 384.0);
    pub fn window_size(&self) -> &UiVec2 {
        self.window.size.get().unwrap_or(&Self::DEFAULT_SIZE)
    }
    pub fn set_window_size(&mut self, size: UiVec2) {
        self.window.size = match size {
            size if size == Self::DEFAULT_SIZE => None,
            size => Some(size),
        }
        .unwrap_or_default()
    }
}

impl Render2DState {
    pub const DEFAULT_OPEN: bool = false;

    fn is_default_open(v: &bool) -> bool {
        !*v
    }

    pub fn is_dirty(&self) -> bool {
        if self.pathing_window.has_changed() {
            return true
        }
        false
    }
    pub fn mark_clean(&mut self) {
        self.pathing_window.mark_unchanged();
    }
}
