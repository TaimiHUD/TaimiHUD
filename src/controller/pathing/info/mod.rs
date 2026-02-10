use {
    crate::controller::pathing::PathingController,
    futures::future::Either,
    std::{
        ops,
        sync::{Arc, LazyLock},
    },
    taimi_meta::packs::{CategoryIndex, CategoryPath},
    taimi_pack::{
        attributes::{FilterAttributes, InteractionAttributes, MarkerAttributes, RenderAttributes},
        trail::TrlPath,
    },
};

pub use self::map::{MapPackInfo, MapTrailInfo};

mod map;

#[derive(Debug, Clone)]
pub struct LoadedMarkerInfo {
    pub category_path: CategoryPath,
    attrs: Either<Arc<RenderAttributes>, Arc<MarkerAttributes>>,
}
impl LoadedMarkerInfo {
    pub fn empty() -> Self {
        Self {
            category_path: CategoryPath::with_path(CategoryIndex::MAX),
            attrs: Either::Left(EMPTY_RENDER_ATTRS.clone()),
        }
    }
    pub(crate) fn with_marker_attrs(
        category_path: CategoryPath,
        attrs: &MarkerAttributes,
        keep_attrs: bool,
    ) -> Self {
        Self {
            category_path,
            attrs: match keep_attrs {
                true => {
                    let mut attrs = attrs.clone();
                    let _ = attrs.render.get_or_insert_with(|| EMPTY_RENDER_ATTRS.clone());
                    Either::Right(Arc::new(attrs))
                },
                _ => {
                    let render = attrs.render.as_ref().unwrap_or(&*EMPTY_RENDER_ATTRS);
                    Either::Left(render.clone())
                },
            },
        }
    }

    pub fn is_empty(&self) -> bool {
        self.category_path.path == CategoryIndex::MAX
    }

    pub fn get_marker_attrs(&self) -> Option<&Arc<MarkerAttributes>> {
        match &self.attrs {
            Either::Right(a) => Some(a),
            Either::Left(..) => None,
        }
    }
    pub fn attrs(&self) -> &Arc<RenderAttributes> {
        match &self.attrs {
            Either::Right(a) => match &a.render {
                Some(a) => a,
                None => &EMPTY_RENDER_ATTRS,
            },
            Either::Left(a) => a,
        }
    }
    /// may involve `Arc::make_mut`
    pub(crate) fn attrs_mut(&mut self) -> &mut Arc<RenderAttributes> {
        match &mut self.attrs {
            Either::Left(a) => a,
            Either::Right(a) => {
                let render = Arc::make_mut(a).render.as_mut();
                match render {
                    #[cfg(debug_assertions)]
                    r => r.expect("constructor"),
                    #[cfg(not(debug_assertions))]
                    r => unsafe { r.unwrap_unchecked() },
                }
            },
        }
    }
    pub fn get_filter_attrs(&self) -> Option<&Box<FilterAttributes>> {
        match &self.attrs {
            Either::Right(a) => a.filters.as_ref(),
            Either::Left(..) => None,
        }
    }
    pub fn filter_attrs(&self) -> &FilterAttributes {
        self.get_filter_attrs()
            .map(|f| &**f)
            .unwrap_or_else(|| &EMPTY_FILTER_ATTRS)
    }
    #[cfg(feature = "paths-interact")]
    pub fn get_interaction_attrs(&self) -> Option<&Arc<InteractionAttributes>> {
        match &self.attrs {
            Either::Right(a) => a.interaction.as_ref(),
            Either::Left(..) => None,
        }
    }
    #[cfg(feature = "paths-interact")]
    pub fn interaction_attrs(&self) -> &InteractionAttributes {
        self.get_interaction_attrs()
            .map(|f| &**f)
            .unwrap_or_else(|| &**EMPTY_INTERACTION_ATTRS)
    }

    pub(super) fn sig_ptr(ptr: *const ()) -> [u32; 2] {
        let ptr = ptr as usize;
        let p1 = ptr as u64 >> 32;
        [ptr as u32, p1 as u32]
    }
    fn sig_attrs(attrs: &Either<Arc<RenderAttributes>, Arc<MarkerAttributes>>) -> [u32; 2] {
        Self::sig_ptr(match attrs {
            Either::Left(a) => Arc::as_ptr(a) as *const (),
            Either::Right(a) => Arc::as_ptr(a) as *const (),
        })
    }
    pub(crate) fn sig(&self) -> [u32; 2] {
        let mut sig = Self::sig_attrs(&self.attrs);
        if let [ref mut s0, ..] = sig {
            *s0 ^= self.category_path.path as u32;
        }
        sig
    }
}
impl Default for LoadedMarkerInfo {
    fn default() -> Self {
        Self::empty()
    }
}

#[derive(Debug, Clone, Default)]
pub struct LoadedTrailInfo {
    pub(crate) marker_info: LoadedMarkerInfo,
    pub trl: Option<TrlPath>,
}
impl LoadedTrailInfo {
    pub fn empty() -> Self {
        Self {
            marker_info: LoadedMarkerInfo::empty(),
            trl: None,
        }
    }
    pub(crate) fn with_marker_attrs(category_path: CategoryPath, attrs: &MarkerAttributes) -> Self {
        let wants_attrs = PathingController::trail_wants_attrs(attrs);
        let mut marker_info = LoadedMarkerInfo::with_marker_attrs(category_path, attrs, wants_attrs);
        if marker_info.attrs().trail.is_none() {
            log::debug!("trail had incomplete render attrs?");
            let _ = Arc::make_mut(marker_info.attrs_mut())
                .trail
                .get_or_insert_default();
        }
        Self { marker_info, trl: None }
    }
    #[inline(always)]
    pub fn marker_info(&self) -> &LoadedMarkerInfo {
        &self.marker_info
    }
}
impl ops::Deref for LoadedTrailInfo {
    type Target = LoadedMarkerInfo;
    fn deref(&self) -> &Self::Target {
        &self.marker_info
    }
}
/// unsafe if it allows you to unset trail render attrs...
#[cfg(todo)]
impl ops::DerefMut for LoadedTrailInfo {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.marker_info
    }
}

#[derive(Debug, Clone, Default)]
pub struct LoadedPoiInfo {
    pub(crate) marker_info: LoadedMarkerInfo,
}
impl LoadedPoiInfo {
    pub fn empty() -> Self {
        Self { marker_info: LoadedMarkerInfo::empty() }
    }
    pub(crate) fn with_marker_attrs(category_path: CategoryPath, attrs: &MarkerAttributes) -> Self {
        let wants_attrs = PathingController::poi_wants_attrs(attrs);
        let mut marker_info = LoadedMarkerInfo::with_marker_attrs(category_path, attrs, wants_attrs);
        if marker_info.attrs().poi.is_none() {
            log::debug!("poi had incomplete render attrs?");
            let _ = Arc::make_mut(marker_info.attrs_mut()).poi.get_or_insert_default();
        }
        Self { marker_info }
    }
    #[inline(always)]
    pub fn marker_info(&self) -> &LoadedMarkerInfo {
        &self.marker_info
    }
}
impl ops::Deref for LoadedPoiInfo {
    type Target = LoadedMarkerInfo;
    fn deref(&self) -> &Self::Target {
        &self.marker_info
    }
}
/// unsafe if it allows you to unset trail render attrs...
#[cfg(todo)]
impl ops::DerefMut for LoadedPoiInfo {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.marker_info
    }
}

pub(crate) static EMPTY_RENDER_ATTRS: LazyLock<Arc<RenderAttributes>> = LazyLock::new(|| {
    Arc::new(RenderAttributes {
        poi: Some(Default::default()),
        trail: Some(Default::default()),
        ..Default::default()
    })
});
pub(crate) static EMPTY_FILTER_ATTRS: LazyLock<FilterAttributes> =
    LazyLock::new(|| FilterAttributes::default());
#[cfg(feature = "paths-interact")]
pub(crate) static EMPTY_INTERACTION_ATTRS: LazyLock<Arc<InteractionAttributes>> =
    LazyLock::new(|| Arc::new(InteractionAttributes::default()));
