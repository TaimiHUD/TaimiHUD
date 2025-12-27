use {
    std::{ops, sync::{Arc, LazyLock}},
    taimi_meta::packs::{
        CategoryIndex, CategoryPath,
    },
    taimi_pack::{
        attributes::{self, AttrString, RenderAttributes},
        trail::{Trail, TrlPath},
    },
};

pub use self::map::{MapPackInfo, MapTrailInfo};

mod map;

#[derive(Debug, Clone)]
pub struct LoadedMarkerInfo {
    pub category_path: CategoryPath,
    pub attrs: Arc<RenderAttributes>,
}
impl LoadedMarkerInfo {
    pub fn empty() -> Self {
        Self {
            category_path: CategoryPath::with_path(CategoryIndex::MAX),
            attrs: EMPTY_RENDER_ATTRS.clone(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.category_path.path == CategoryIndex::MAX
    }
}
impl Default for LoadedMarkerInfo {
    fn default() -> Self { Self::empty() }
}

#[derive(Debug, Clone, Default)]
pub struct LoadedTrailInfo {
    pub marker_info: LoadedMarkerInfo,
    pub trl: Option<TrlPath>,
}
impl LoadedTrailInfo {
    pub fn empty() -> Self {
        Self {
            marker_info: LoadedMarkerInfo::empty(),
            trl: None,
        }
    }
}
impl ops::Deref for LoadedTrailInfo {
    type Target = LoadedMarkerInfo;
    fn deref(&self) -> &Self::Target { &self.marker_info }
}
impl ops::DerefMut for LoadedTrailInfo {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.marker_info }
}

#[derive(Debug, Clone, Default)]
pub struct LoadedPoiInfo {
    pub marker_info: LoadedMarkerInfo,
}
impl LoadedPoiInfo {
    pub fn empty() -> Self {
        Self {
            marker_info: LoadedMarkerInfo::empty(),
        }
    }
}
impl ops::Deref for LoadedPoiInfo {
    type Target = LoadedMarkerInfo;
    fn deref(&self) -> &Self::Target { &self.marker_info }
}
impl ops::DerefMut for LoadedPoiInfo {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.marker_info }
}

pub(crate) static EMPTY_RENDER_ATTRS: LazyLock<Arc<RenderAttributes>> = LazyLock::new(|| {
    Arc::new(RenderAttributes {
        poi: Some(Default::default()),
        trail: Some(Default::default()),
        .. Default::default()
    })
});

#[cfg(deleteme)]
mod we_already_defined_all_this_right_guys {
#[derive(Debug, Clone)]
pub struct LoadedMarkerInfo {
    pub category: CategoryPath,
    pub attrs: Arc<RenderAttributes>,
}
impl LoadedMarkerInfo {
    pub fn empty() -> Self {
        Self {
            category: CategoryPath::with_path(CategoryIndex::MAX),
            attrs: EMPTY_RENDER_ATTRS.clone(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.category.path == CategoryIndex::MAX
    }
}
impl Default for LoadedMarkerInfo {
    fn default() -> Self { Self::empty() }
}
pub(crate) static EMPTY_RENDER_ATTRS: LazyLock<Arc<RenderAttributes>> = LazyLock::new(|| {
    Arc::new(RenderAttributes {
        poi: Some(Default::default()),
        trail: Some(Default::default()),
        .. Default::default()
    })
});

#[derive(Debug, Clone, Default)]
pub struct LoadedTrailInfo {
    pub marker_info: LoadedMarkerInfo,
}
impl LoadedTrailInfo {
    pub fn empty() -> Self {
        Self {
            marker_info: LoadedMarkerInfo::empty(),
        }
    }
}
impl ops::Deref for LoadedTrailInfo {
    type Target = LoadedMarkerInfo;
    fn deref(&self) -> &Self::Target { &self.marker_info }
}
impl ops::DerefMut for LoadedTrailInfo {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.marker_info }
}

#[derive(Debug, Clone, Default)]
pub struct LoadedPoiInfo {
    pub marker_info: LoadedMarkerInfo,
}
impl LoadedPoiInfo {
    pub fn empty() -> Self {
        Self {
            marker_info: LoadedMarkerInfo::empty(),
        }
    }
}
impl ops::Deref for LoadedPoiInfo {
    type Target = LoadedMarkerInfo;
    fn deref(&self) -> &Self::Target { &self.marker_info }
}
impl ops::DerefMut for LoadedPoiInfo {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.marker_info }
}
}
