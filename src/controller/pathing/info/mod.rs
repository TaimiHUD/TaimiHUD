use {
    crate::controller::pathing::{space::DrawSpace, PathingController},
    glamour::{Point3, Box3, Size3, FloatUnit},
    num_traits::AsPrimitive,
    futures::future::Either,
    std::{
        borrow::Cow,
        ops,
        sync::{Arc, LazyLock},
    },
    taimi_meta::packs::{CategoryIndex, CategoryPath},
    taimi_pack::{
        attributes::{
            cell::{AttrKeyValue, GetAttrDyn, GetAttrDynExt, SetAttrDyn, PackKeyId, PackValueCell, PackValueDyn, PackValueOf},
            keys::{self, AttrKey, GetAttr, SetAttr},
            FilterAttributes, InteractionAttributes, MarkerAttributes, RenderAttributes, PoiAttributes, TrailAttributes,
        },
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
    pub(crate) fn marker_attrs_mut(&mut self) -> &mut Arc<MarkerAttributes> {
        let render_attrs = if let Either::Left(a) = &self.attrs {
            Some(a.clone())
        } else { None };
        if let Some(render_attrs) = render_attrs {
            let mut attrs = MarkerAttributes::default();
            attrs.render = Some(render_attrs);
            self.attrs = Either::Right(Arc::new(attrs));
        }
        match &mut self.attrs {
            Either::Right(a) => a,
            #[cfg(taimi_debug)]
            Either::Left(..) => unreachable!(),
            #[cfg(not(taimi_debug))]
            Either::Left(..) => unsafe { core::hint::unreachable_unchecked() },
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
impl<A> GetAttr<A> for LoadedMarkerInfo where
    A: AttrKey + AttrKeyValue,
    MarkerAttributes: GetAttr<A>,
{
    fn has_attr(&self) -> bool {
        match &self.attrs {
            Either::Right(a) => GetAttr::<A>::has_attr(&**a),
            Either::Left(r) => r.has_attr_dyn_of::<A>(),
        }
    }
    fn get_attr(&self) -> Option<Cow<'_, A>> where
        A: ToOwned,
    {
        match &self.attrs {
            Either::Right(a) => GetAttr::<A>::get_attr(&**a),
            Either::Left(r) => r.get_attr_of_dyn::<A>(),
        }
    }
    fn get_attr_ref(&self) -> Option<&A> {
        match &self.attrs {
            Either::Right(a) => GetAttr::<A>::get_attr_ref(&**a),
            Either::Left(r) => r.get_attr_dyn_ref_of::<A>(),
        }
    }
}
impl<A> SetAttr<A> for LoadedMarkerInfo where
    A: AttrKey + AttrKeyValue,
    MarkerAttributes: GetAttr<A> + SetAttr<A>,
{
    fn unset_attr(&mut self) {
        match &mut self.attrs {
            Either::Right(a) => if GetAttr::<A>::has_attr(&**a) {
                SetAttr::<A>::unset_attr(&mut *Arc::make_mut(a))
            },
            Either::Left(r) => if RenderAttributes::holds_attr_dyn_of::<A>() {
                Arc::make_mut(r).unset_attr_dyn_of::<A>()
            },
        }
    }
    fn set_attr(&mut self, v: A) {
        let needs_upgrade = match &self.attrs {
            Either::Right(..) => None,
            Either::Left(r) if RenderAttributes::holds_attr_dyn_of::<A>() =>
                None,
            Either::Left(r) =>
                Some(r.clone()),
        };
        if let Some(r) = needs_upgrade {
            let mut attrs = MarkerAttributes::default();
            attrs.render = Some(r);
            self.attrs = Either::Right(Arc::new(attrs));
        }
        match &mut self.attrs {
            Either::Right(a) => SetAttr::<A>::set_attr(&mut *Arc::make_mut(a), v),
            Either::Left(r) => {
                Arc::make_mut(r).set_attr_dyn_of(v);
            },
        }
    }
}
impl GetAttrDyn for LoadedMarkerInfo {
    fn has_attr_dyn(&self, key: PackKeyId) -> bool {
        match &self.attrs {
            Either::Right(a) => a.has_attr_dyn(key),
            Either::Left(r) => r.has_attr_dyn(key),
        }
    }
    fn get_attr_dyn(&self, key: PackKeyId) -> Option<Cow<'_, dyn AttrKeyValue>> {
        match &self.attrs {
            Either::Right(a) => a.get_attr_dyn(key),
            Either::Left(r) => r.get_attr_dyn(key),
        }
    }
    fn holds_attr_dyn(key: PackKeyId) -> bool {
        MarkerAttributes::holds_attr_dyn(key)
    }
    fn clone_attr_dyn(&self, key: PackKeyId) -> Option<PackValueDyn> {
        match &self.attrs {
            Either::Right(a) => a.clone_attr_dyn(key),
            Either::Left(r) => r.clone_attr_dyn(key),
        }
    }
    fn iter_attrs_dyn(&self) -> impl Iterator<Item = Cow<'_, dyn AttrKeyValue>> {
        let (m, r) = match &self.attrs {
            Either::Right(a) => (Some(a.iter_attrs_dyn()), None),
            Either::Left(r) => (None, Some(r.iter_attrs_dyn())),
        };
        m.into_iter().flatten().chain(
            r.into_iter().flatten()
        )
    }
}
impl SetAttrDyn for LoadedMarkerInfo {
    fn set_attr_dyn(&mut self, value: PackValueCell) -> bool {
        let unset = !value.is_valid();
        let needs_upgrade = match &self.attrs {
            Either::Right(..) => None,
            Either::Left(r) if unset || RenderAttributes::holds_attr_dyn(value.id()) =>
                None,
            Either::Left(r) =>
                Some(r.clone()),
        };
        if let Some(r) = needs_upgrade {
            let mut attrs = MarkerAttributes::default();
            attrs.render = Some(r);
            self.attrs = Either::Right(Arc::new(attrs));
        }
        if unset {
            let attrs = match &self.attrs {
                Either::Right(a) => &**a as &dyn GetAttrDyn,
                Either::Left(r) => &**r as &dyn GetAttrDyn,
            };
            if !attrs.has_attr_dyn(value.id()) { return true }
        }
        let attrs = match &mut self.attrs {
            Either::Right(a) => &mut *Arc::make_mut(a) as &mut dyn SetAttrDyn,
            Either::Left(r) => &mut *Arc::make_mut(r) as &mut dyn SetAttrDyn,
        };
        attrs.set_attr_dyn(value)
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
    #[inline(always)]
    pub fn trail_attrs(&self) -> &Box<TrailAttributes> {
        unsafe {
            self.marker_info.attrs().trail.as_ref().unwrap_unchecked()
        }
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
impl GetAttrDyn for LoadedTrailInfo {
    fn has_attr_dyn(&self, key: PackKeyId) -> bool {
        key == keys::TrailDataFile::pack_key_of() ||
        self.marker_info.has_attr_dyn(key)
    }
    fn get_attr_dyn(&self, key: PackKeyId) -> Option<Cow<'_, dyn AttrKeyValue>> {
        if key == keys::TrailDataFile::pack_key_of() {
            self.trl.as_ref().map(|v| {
                let v = keys::TrailDataFile::from_ref(&v.path);
                Cow::Borrowed(v as &_)
            })
        } else {
            self.marker_info.get_attr_dyn(key)
        }
    }
    fn holds_attr_dyn(key: PackKeyId) -> bool {
        key == keys::TrailDataFile::pack_key_of() || LoadedMarkerInfo::holds_attr_dyn(key)
    }
    fn clone_attr_dyn(&self, key: PackKeyId) -> Option<PackValueDyn> {
        if key == keys::TrailDataFile::pack_key_of() {
            self.trl.as_ref().map(|v| PackValueDyn::new_boxed_dyn(
                    keys::TrailDataFile::from(v.path.clone())
            ))
        } else {
            self.marker_info.clone_attr_dyn(key)
        }
    }
    fn iter_attrs_dyn(&self) -> impl Iterator<Item = Cow<'_, dyn AttrKeyValue>> {
        self.trl.as_ref().map(|v| Cow::Borrowed(
            keys::TrailDataFile::from_ref(&v.path) as &_
        )).into_iter().chain(
            self.marker_info.iter_attrs_dyn()
        )
    }
}
impl SetAttrDyn for LoadedTrailInfo {
    fn set_attr_dyn(&mut self, value: PackValueCell) -> bool {
        if value.id() == keys::TrailDataFile::pack_key_of() {
            let v = value.is_valid().then(|| unsafe {
                PackValueOf::<keys::TrailDataFile>::new_unchecked(value).to_value()
            }).flatten();
            match (&mut self.trl, v) {
                (Some(trl), Some(v)) =>
                    trl.path = v.into(),
                (trl, v) =>
                    *trl = v.map(|v| TrlPath::new(v.into())),
            }
            true
        } else {
            self.marker_info.set_attr_dyn(value)
        }
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
    #[inline(always)]
    pub fn poi_attrs(&self) -> &Box<PoiAttributes> {
        unsafe {
            self.marker_info.attrs().poi.as_ref().unwrap_unchecked()
        }
    }
    /// TODO: diagonal only relevant if rotation isn't axis-aligned,
    /// also billboards will always be aligned to the near/far clip planes btw...
    pub fn bounds_at<U>(&self, origin: Point3<U>) -> Box3<U> where
        U: FloatUnit,
        f32: AsPrimitive<U::Scalar>,
    {
        if num_traits::Float::is_infinite(origin.x) {
            // used as a marker for removed markers
            const IRR: Box3<f32> = taimi_meta::spatial::irrelevant_box3();
            return Box3::new(
                IRR.min.as_(),
                IRR.max.as_(),
            )
        }
        const HALF_SQUARE_DIAG: f32 = core::f32::consts::SQRT_2 / 2.0;
        let max_diagonal_mid = match self.poi_attrs().clone_attr_of::<keys::IconSize>() {
            #[cfg(todo = "unnecessary")]
            Some(edge_len) if !edge_len.is_default() =>
                (edge_len.powi(2) * (2.0f32 / 4.0f32)).sqrt(),
            Some(edge_len) => f32::from(edge_len) * HALF_SQUARE_DIAG,
            _ => {
                const DEFAULT_DIAG: f32 = keys::IconSize::DEFAULT.0 * HALF_SQUARE_DIAG;
                DEFAULT_DIAG
            },
        };
        let size = Size3::splat(max_diagonal_mid.as_()).to_vector();
        Box3::new(origin - size, origin + size)
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
impl GetAttrDyn for LoadedPoiInfo {
    fn has_attr_dyn(&self, key: PackKeyId) -> bool {
        self.marker_info.has_attr_dyn(key)
    }
    fn get_attr_dyn(&self, key: PackKeyId) -> Option<Cow<'_, dyn AttrKeyValue>> {
        self.marker_info.get_attr_dyn(key)
    }
    fn holds_attr_dyn(key: PackKeyId) -> bool {
        LoadedMarkerInfo::holds_attr_dyn(key)
    }
    fn clone_attr_dyn(&self, key: PackKeyId) -> Option<PackValueDyn> {
        self.marker_info.clone_attr_dyn(key)
    }
    fn iter_attrs_dyn(&self) -> impl Iterator<Item = Cow<'_, dyn AttrKeyValue>> {
        self.marker_info.iter_attrs_dyn()
    }
}
impl SetAttrDyn for LoadedPoiInfo {
    fn set_attr_dyn(&mut self, value: PackValueCell) -> bool {
        self.marker_info.set_attr_dyn(value)
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
