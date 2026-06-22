use {
    crate::{
        attributes::{
            cell::{
                AttrKeyValue,
                GetAttrDyn,
                PackKeyId,
                PackKeySet,
                PackValueDyn,
                PackValueOf,
                PackValueSet,
            },
            keys::{AttrKey, GetAttr, Guid},
            MarkerAttributes,
        },
        category::CategoryId,
        pack::{Pack, PackCategoryArc, PackPoiArc, PackTrailArc},
        script::{format_err, Result},
    },
    core::{borrow::Borrow, fmt, ops},
    std::{
        borrow::{Cow, ToOwned},
        cell::OnceCell,
        collections::{btree_map, BTreeMap, BTreeSet},
        sync::{self, Arc, RwLock},
    },
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MarkerType {
    Category,
    Poi,
    Trail,
}
impl MarkerType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Category => "cat",
            Self::Poi => "poi",
            Self::Trail => "trail",
        }
    }
}
impl fmt::Display for MarkerType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// TODO: there's a real type for this in pathcontrol branch
pub type MarkerLoc = (MarkerType, usize);
impl Into<MarkerLoc> for &'_ PackCategoryArc {
    fn into(self) -> MarkerLoc {
        (MarkerType::Category, self.category_idx())
    }
}
impl Into<MarkerLoc> for &'_ PackPoiArc {
    fn into(self) -> MarkerLoc {
        (MarkerType::Poi, self.poi_idx())
    }
}
impl Into<MarkerLoc> for &'_ PackTrailArc {
    fn into(self) -> MarkerLoc {
        (MarkerType::Trail, self.trail_idx())
    }
}

#[derive(Debug, Clone, Default)]
pub struct MarkerOverrides {
    pub attrs: PackValueSet,
    pub masked_to_default: PackKeySet,
}
impl MarkerOverrides {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.masked_to_default.is_empty() && self.attrs.is_empty()
    }
    pub fn get_dyn(&self, id: PackKeyId) -> Option<Option<&PackValueDyn>> {
        if self.masked_to_default.contains(&id) {
            return Some(None)
        }
        self.attrs.get(&id).map(PackValueDyn::from_cell_dyn_ref)
    }
    pub fn get<A>(&self) -> Option<Option<&PackValueOf<A>>>
    where
        A: AttrKeyValue,
    {
        self.get_dyn(A::pack_key_of())
            .map(|v| v.map(|v| unsafe { PackValueOf::from_ref_unchecked(v.inner()) }))
    }

    #[inline]
    pub fn shared_write(overrides: &MarkerOverridesShared) -> sync::RwLockWriteGuard<'_, Self> {
        overrides.write().unwrap_or_else(|e| e.into_inner())
    }
    #[inline]
    pub fn shared_read(overrides: &MarkerOverridesShared) -> sync::RwLockReadGuard<'_, Self> {
        overrides.read().unwrap_or_else(|e| e.into_inner())
    }
    #[inline]
    pub fn shared_try_read(overrides: &MarkerOverridesShared) -> Option<sync::RwLockReadGuard<'_, Self>> {
        overrides.try_read().ok()
    }

    #[inline]
    pub fn empty_ref() -> &'static Self {
        static EMPTY: MarkerOverrides = MarkerOverrides {
            attrs: PackValueSet::new(),
            masked_to_default: PackKeySet::new(),
        };
        &EMPTY
    }
}
impl Into<MarkerLoc> for &'_ PackMarkerRef {
    fn into(self) -> MarkerLoc {
        self.path
    }
}
#[derive(Debug)]
pub struct MarkerOverridesAttrs<'a, T> {
    overrides: &'a MarkerOverrides,
    attrs: &'a T,
}
impl<'a, T> MarkerOverridesAttrs<'a, T> {
    #[inline]
    pub const fn wrap_with_overrides(attrs: &'a T, overrides: &'a MarkerOverrides) -> Self {
        Self { overrides, attrs }
    }
    #[inline]
    pub fn empty(attrs: &'a T) -> Self {
        Self::wrap_with_overrides(attrs, MarkerOverrides::empty_ref())
    }
}
impl<'a, T> GetAttrDyn for MarkerOverridesAttrs<'a, T>
where
    T: GetAttrDyn,
{
    fn has_attr_dyn(&self, key: PackKeyId) -> bool {
        match self.overrides.get_dyn(key) {
            Some(v) => v.is_some(),
            None => self.attrs.has_attr_dyn(key),
        }
    }
    /// XXX: despite being able to hold anything as an override, defer to the
    /// wrapped type's idea of what is valid
    fn holds_attr_dyn(key: PackKeyId) -> bool {
        T::holds_attr_dyn(key)
    }
    fn get_attr_dyn(&self, key: PackKeyId) -> Option<Cow<'_, dyn AttrKeyValue>> {
        if self.overrides.masked_to_default.contains(&key) {
            None
        } else if let v @ Some(..) = self.overrides.attrs.get_attr_dyn(key) {
            v
        } else {
            self.attrs.get_attr_dyn(key)
        }
    }
    fn get_attr_dyn_ref(&self, key: PackKeyId) -> Option<&dyn AttrKeyValue> {
        if self.overrides.masked_to_default.contains(&key) {
            None
        } else if let v @ Some(..) = self.overrides.attrs.get_attr_dyn_ref(key) {
            v
        } else {
            self.attrs.get_attr_dyn_ref(key)
        }
    }
    fn iter_attrs_dyn(&self) -> impl Iterator<Item = Cow<'_, dyn AttrKeyValue>> + '_ {
        let o = self.overrides;
        let attrs = self.attrs.iter_attrs_dyn().filter(move |v| {
            let key = v.pack_key_id();
            !o.masked_to_default.contains(&key) && !o.attrs.contains(&key)
        });
        attrs.chain(self.overrides.attrs.iter_attrs_dyn())
    }
}
impl<'a, T, A> GetAttr<A> for MarkerOverridesAttrs<'a, T>
where
    A: AttrKey + AttrKeyValue,
    T: GetAttr<A>,
{
    fn has_attr(&self) -> bool {
        let o = (!self.overrides.is_empty())
            .then(|| self.overrides.get::<A>())
            .flatten();
        match o {
            Some(v) => v.is_some(),
            None => self.attrs.has_attr(),
        }
    }
    fn get_attr(&self) -> Option<Cow<'_, A>> {
        let o = (!self.overrides.is_empty())
            .then(|| self.overrides.get::<A>())
            .flatten();
        match o {
            Some(None) => None,
            Some(Some(v)) => Some(Cow::Borrowed(v)),
            None => self.attrs.get_attr(),
        }
    }
    fn get_attr_ref(&self) -> Option<&A> {
        let o = (!self.overrides.is_empty())
            .then(|| self.overrides.get::<A>())
            .flatten();
        match o {
            Some(None) => None,
            Some(Some(v)) => Some(v),
            None => self.attrs.get_attr_ref(),
        }
    }
}
impl<'a, T> Copy for MarkerOverridesAttrs<'a, T> {}
impl<'a, T> Clone for MarkerOverridesAttrs<'a, T> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

#[derive(Debug, Clone, Default)]
pub struct MarkerStateOverride {
    pub masked: bool,
}
#[derive(Debug, Clone, Default)]
pub struct PackOverrides {
    pub overrides: BTreeMap<MarkerLoc, MarkerOverridesShared>,
    pub masked: BTreeMap<MarkerLoc, MarkerStateOverride>,
    pub dynamic: BTreeSet<MarkerLoc>,
    pub cat_overrides: BTreeMap<CategoryId, usize>,
}
impl PackOverrides {
    pub fn allocate_dynamic(&mut self, kind: MarkerType, pack: &'_ Pack) -> Result<MarkerLoc> {
        Self::allocate_dynamic_inner(&mut self.dynamic, &mut self.overrides, kind, pack)
    }
    fn allocate_dynamic_inner(
        dynamic: &mut BTreeSet<MarkerLoc>,
        overrides: &mut BTreeMap<MarkerLoc, MarkerOverridesShared>,
        kind: MarkerType,
        pack: &'_ Pack,
    ) -> Result<MarkerLoc> {
        let min = match kind {
            MarkerType::Category => pack.categories.all_categories.len(),
            MarkerType::Poi => pack.pois.len(),
            MarkerType::Trail => pack.trails.len(),
        };
        let mut path = (kind, min);
        while dynamic.contains(&path) {
            path.1 = match path.1.checked_add(1) {
                Some(n) => n,
                None => return Err(format_err!("ran out of dynamic IDs")),
            };
        }
        Self::allocate_dynamic_post_inner(dynamic, overrides, path);
        Ok(path)
    }
    pub fn allocate_dynamic_post(&mut self, loc: MarkerLoc) {
        Self::allocate_dynamic_post_inner(&mut self.dynamic, &mut self.overrides, loc)
    }
    fn allocate_dynamic_post_inner(
        dynamic: &mut BTreeSet<MarkerLoc>,
        overrides: &mut BTreeMap<MarkerLoc, MarkerOverridesShared>,
        loc: MarkerLoc,
    ) {
        dynamic.insert(loc);
        // ensure an entry for containing attrs is available, since this won't be able to fallback to the pack
        let _ = overrides.entry(loc).or_default();
    }
    /// NOTE: does not store id in override attrs, must be done immediately after! TODO?
    pub fn allocate_dynamic_cat(
        &mut self,
        id: CategoryId,
        pack: &'_ Pack,
        loc: Option<MarkerLoc>,
    ) -> Result<MarkerLoc> {
        let id_entry = self.cat_overrides.entry(id);
        if let btree_map::Entry::Occupied(e) = id_entry {
            let id = e.key();
            return Err(format_err!("duplicate dynamic category {id}"))
        }
        #[cfg(todo)]
        match pack.categories.all_categories.get_index_of(&id) {
            Some(cat_idx) =>
                if self
                    .overrides
                    .get((MarkerType::Category, cat_idx))
                    .map(|o| {
                        MarkerOverrides::shared_read(o)
                            .get::<keys::CategoryRef>()
                            .is_none()
                            && MarkerOverrides::shared_read(o).get::<keys::NameId>().is_none()
                    })
                    .unwrap_or(true)
                {
                    return Err(format_err!("pack category {id} shadowed by dynamic"))
                },
            _ => (),
        }
        let res = match loc {
            None => Self::allocate_dynamic_inner(
                &mut self.dynamic,
                &mut self.overrides,
                MarkerType::Category,
                pack,
            ),
            Some(loc) => {
                Self::allocate_dynamic_post_inner(&mut self.dynamic, &mut self.overrides, loc);
                Ok(loc)
            },
        };
        if let &Ok((_, cat_idx)) = &res {
            let _ = id_entry.insert_entry(cat_idx);
        }
        res
    }
    pub fn remove_dynamic(&mut self, path: MarkerLoc) {
        self.dynamic.remove(&path);
        self.clear_marker_overrides(path);
    }
    pub fn clear_marker_overrides(&mut self, path: MarkerLoc) {
        if let Some(o) = self.overrides.remove(&path) {
            match Arc::try_unwrap(o) {
                Err(o) => {
                    // TODO: this may just confuse other refs if they actually try to use it..?
                    // also be more assertive here, if this is meaningful then acquire the write lock!
                    if let Ok(mut o) = o.try_write() {
                        o.attrs = Default::default();
                        #[cfg(todo = "unnecessary")]
                        {
                            o.masked_to_default = Default::default();
                        }
                    }
                },
                // no other refs remain, so nothing to cleanup...
                Ok(o) => drop(o),
            }
        }
        // maybe unnecessary?
        self.masked.remove(&path);
    }
    pub fn iter_dynamic_all<'a>(&'a self) -> impl Iterator<Item = (MarkerLoc, &'a MarkerOverridesShared)> {
        let masked = &self.masked;
        let attrs = &self.overrides;
        self.dynamic
            .iter()
            .copied()
            .filter(|loc| !masked.get(loc).map(|m| m.masked).unwrap_or(false))
            .filter_map(|loc| attrs.get(&loc).map(|o| (loc, o)))
    }
    pub fn mask_marker(&mut self, path: MarkerLoc) {
        let masked = self.masked.entry(path).or_default();
        masked.masked = true;
    }
    pub fn unmask_marker(&mut self, path: MarkerLoc) {
        if let Some(masked) = self.masked.get_mut(&path) {
            masked.masked = false;
        }
    }
    pub fn is_masked(&self, path: MarkerLoc) -> bool {
        self.masked.get(&path).map(|m| m.masked).unwrap_or(false)
    }
    pub fn iter_masked_indices(&self, ty: MarkerType) -> impl Iterator<Item = usize> + Clone + '_ {
        self.masked
            .iter()
            .filter_map(move |(&(t, i), mask)| (t == ty && mask.masked).then_some(i))
    }
    pub fn assert_guid(&self, path: MarkerLoc, query: &Guid) -> bool {
        let query = query.or_empty();
        let Some(overrides) = self.overrides.get(&path) else { return true };
        let overrides = MarkerOverrides::shared_read(overrides);
        if overrides.masked_to_default.contains(&Guid::pack_key_of()) {
            query.is_none()
        } else {
            overrides
                .get::<Guid>()
                .map(|o| o.and_then(|g| g.or_empty()) == query)
                .unwrap_or(true)
        }
    }
    pub fn path_by_guid(&self, kind: Option<MarkerType>, guid: &Guid) -> Option<MarkerLoc> {
        self.paths_by_guid(kind, guid).next()
    }
    /// TODO: keep key index overrides in a lookup table somewhere...
    pub fn paths_by_guid<'a>(
        &'a self,
        kind: Option<MarkerType>,
        guid: &'a Guid,
    ) -> impl Iterator<Item = MarkerLoc> + 'a {
        let guid = guid.or_empty();
        self.overrides
            .iter()
            .filter(move |&(&p @ (ty, ..), _)| kind.map(|k| ty == k).unwrap_or(true) && !self.is_masked(p))
            .filter(move |(_, o)| {
                let o = MarkerOverrides::shared_read(o);
                let guid_override = o.get::<Guid>().map(|o| o.and_then(|g| g.get().or_empty()));
                guid_override.map(|g| g == guid).unwrap_or(false)
            })
            .map(|(&k, _)| k)
    }

    #[inline]
    pub fn shared_write(overrides: &PackOverridesShared) -> sync::RwLockWriteGuard<'_, Self> {
        overrides.write().unwrap_or_else(|e| e.into_inner())
    }
    #[inline]
    pub fn shared_read(overrides: &PackOverridesShared) -> sync::RwLockReadGuard<'_, Self> {
        overrides.read().unwrap_or_else(|e| e.into_inner())
    }
    #[inline]
    pub fn shared_try_read(overrides: &PackOverridesShared) -> Option<sync::RwLockReadGuard<'_, Self>> {
        overrides.try_read().ok()
    }
}
pub type PackOverridesShared = Arc<RwLock<PackOverrides>>;
pub type MarkerOverridesShared = Arc<RwLock<MarkerOverrides>>;

#[derive(Debug, Clone)]
pub struct PackMarkerRef {
    pack: Arc<Pack>,
    path: MarkerLoc,
}
impl PackMarkerRef {
    pub fn new(pack: &Arc<Pack>, path: MarkerLoc) -> Option<Self> {
        let valid = match path {
            (MarkerType::Category, idx) => idx < pack.categories.all_categories.len(),
            (MarkerType::Poi, idx) => idx < pack.pois.len(),
            (MarkerType::Trail, idx) => idx < pack.trails.len(),
        };
        valid.then(|| unsafe { Self::new_unchecked(pack.clone(), path) })
    }
    #[inline]
    pub unsafe fn new_unchecked(pack: Arc<Pack>, path: MarkerLoc) -> Self {
        Self { pack, path }
    }

    #[inline]
    pub fn marker_kind(&self) -> MarkerType {
        self.path.0
    }
    #[inline]
    pub fn marker_index(&self) -> usize {
        self.path.1
    }

    #[inline]
    pub fn path(&self) -> &MarkerLoc {
        &self.path
    }
    #[inline]
    pub unsafe fn path_mut(&mut self) -> &mut MarkerLoc {
        &mut self.path
    }

    #[inline]
    pub fn pack(&self) -> &Arc<Pack> {
        &self.pack
    }
    #[inline]
    pub unsafe fn pack_mut(&mut self) -> &mut Arc<Pack> {
        &mut self.pack
    }

    pub unsafe fn get_attrs_unchecked(&self) -> &MarkerAttributes {
        match self.path {
            (MarkerType::Category, idx) =>
                &self
                    .pack
                    .categories
                    .all_categories
                    .get_index(idx)
                    .unwrap_unchecked()
                    .1
                    .marker_attributes,
            (MarkerType::Poi, idx) => &self.pack.pois.get_unchecked(idx).attributes,
            (MarkerType::Trail, idx) => &self.pack.trails.get_unchecked(idx).attributes,
        }
    }
    pub fn get_attrs(&self) -> Option<&MarkerAttributes> {
        match self.path {
            (MarkerType::Category, idx) => self
                .pack
                .categories
                .all_categories
                .get_index(idx)
                .map(|(_, c)| &c.marker_attributes),
            (MarkerType::Poi, idx) => self.pack.pois.get(idx).map(|p| &p.attributes),
            (MarkerType::Trail, idx) => self.pack.trails.get(idx).map(|t| &t.attributes),
        }
    }
    pub fn get_attrs_dyn(&self) -> &dyn GetAttrDyn {
        match self.path {
            (MarkerType::Category, idx) => self
                .pack
                .categories
                .all_categories
                .get_index(idx)
                .map(|(_, c)| c as &dyn GetAttrDyn),
            (MarkerType::Poi, idx) => self.pack.pois.get(idx).map(|c| c as &dyn GetAttrDyn),
            (MarkerType::Trail, idx) => self.pack.trails.get(idx).map(|c| c as &dyn GetAttrDyn),
        }
        .unwrap_or(<dyn GetAttrDyn>::EMPTY)
    }
}
impl GetAttrDyn for PackMarkerRef {
    #[inline]
    fn has_attr_dyn(&self, key: PackKeyId) -> bool {
        self.get_attrs_dyn().has_attr_dyn(key)
    }
    fn get_attr_dyn(&self, key: PackKeyId) -> Option<Cow<'_, dyn AttrKeyValue>> {
        self.get_attrs_dyn().get_attr_dyn(key)
    }
    fn holds_attr_dyn(key: PackKeyId) -> bool
    where
        Self: Sized,
    {
        MarkerAttributes::holds_attr_dyn(key)
        // TODO: || Poi::holds_attr_dyn(key)
    }
    fn clone_attr_dyn(&self, key: PackKeyId) -> Option<PackValueDyn> {
        self.get_attrs_dyn().clone_attr_dyn(key)
    }
    fn get_attr_dyn_ref(&self, key: PackKeyId) -> Option<&dyn AttrKeyValue> {
        self.get_attrs_dyn().get_attr_dyn_ref(key)
    }
    fn iter_attrs_dyn(&self) -> impl Iterator<Item = Cow<'_, dyn AttrKeyValue>> + '_ {
        match self.path {
            (MarkerType::Category, idx) =>
                self.pack.categories.all_categories.get_index(idx).map(|(_, c)| {
                    Box::new(c.iter_attrs_dyn()) as Box<dyn Iterator<Item = Cow<dyn AttrKeyValue>>>
                }),
            (MarkerType::Poi, idx) => self
                .pack
                .pois
                .get(idx)
                .map(|c| Box::new(c.iter_attrs_dyn()) as Box<_>),
            (MarkerType::Trail, idx) => self
                .pack
                .trails
                .get(idx)
                .map(|c| Box::new(c.iter_attrs_dyn()) as Box<_>),
        }
        .unwrap_or_else(|| Box::new(().iter_attrs_dyn()) as Box<_>)
    }
}
impl AsRef<Pack> for PackMarkerRef {
    #[inline(always)]
    fn as_ref(&self) -> &Pack {
        &self.pack
    }
}
impl Borrow<Pack> for PackMarkerRef {
    #[inline(always)]
    fn borrow(&self) -> &Pack {
        &self.pack
    }
}
impl AsRef<Arc<Pack>> for PackMarkerRef {
    #[inline(always)]
    fn as_ref(&self) -> &Arc<Pack> {
        &self.pack
    }
}
impl Borrow<Arc<Pack>> for PackMarkerRef {
    #[inline(always)]
    fn borrow(&self) -> &Arc<Pack> {
        &self.pack
    }
}
impl<A> GetAttr<A> for PackMarkerRef
where
    A: ?Sized + AttrKey,
    MarkerAttributes: GetAttr<A>,
{
    #[inline]
    fn has_attr(&self) -> bool {
        self.get_attrs().map(|m| m.has_attr()).unwrap_or(false)
    }
    #[inline]
    fn get_attr(&self) -> Option<Cow<'_, A>> {
        self.get_attrs().and_then(|m| m.get_attr())
    }
    #[inline]
    fn get_attr_ref(&self) -> Option<&A> {
        self.get_attrs().and_then(|m| m.get_attr_ref())
    }
}
impl From<PackCategoryArc> for PackMarkerRef {
    fn from(marker: PackCategoryArc) -> Self {
        let path: MarkerLoc = (&marker).into();
        unsafe { Self::new_unchecked(marker.into_pack(), path) }
    }
}
impl From<PackPoiArc> for PackMarkerRef {
    fn from(marker: PackPoiArc) -> Self {
        let path: MarkerLoc = (&marker).into();
        unsafe { Self::new_unchecked(marker.into_pack(), path) }
    }
}
impl From<PackTrailArc> for PackMarkerRef {
    fn from(marker: PackTrailArc) -> Self {
        let path: MarkerLoc = (&marker).into();
        unsafe { Self::new_unchecked(marker.into_pack(), path) }
    }
}
#[derive(Debug, Clone)]
pub struct PackMarkerMut<M = PackMarkerRef> {
    marker: M,
    pub overrides_shared: PackOverridesShared,
    pub overrides: OnceCell<MarkerOverridesShared>,
}
impl<M> PackMarkerMut<M> {
    pub fn new(marker: M, overrides_shared: PackOverridesShared) -> Self {
        Self {
            marker,
            overrides_shared,
            overrides: OnceCell::new(),
        }
    }
    pub unsafe fn new_from_parts(
        marker: M,
        overrides_shared: PackOverridesShared,
        overrides: Option<MarkerOverridesShared>,
    ) -> Self {
        Self {
            marker,
            overrides_shared,
            overrides: overrides.map(OnceCell::from).unwrap_or_default(),
        }
    }

    #[inline]
    pub fn marker(&self) -> &M {
        &self.marker
    }
    #[inline]
    pub unsafe fn marker_mut(&mut self) -> &mut M {
        &mut self.marker
    }
}
impl<M> PackMarkerMut<M>
where
    for<'a> &'a M: Into<MarkerLoc>,
{
    #[inline]
    pub fn path(&self) -> MarkerLoc {
        (&self.marker).into()
    }

    pub fn overrides_ref(&self) -> Option<&MarkerOverridesShared> {
        if let Some(o) = self.overrides.get() {
            return Some(o)
        }

        // TODO: skip in most cases? use a generation counter or watched ref or something? idk
        let o = PackOverrides::shared_try_read(&self.overrides_shared)
            .and_then(|o| o.overrides.get(&self.path()).cloned());
        o.map(|o| self.overrides.get_or_init(move || o))
    }
    pub fn overrides_mut(&self) -> &MarkerOverridesShared {
        self.overrides.get_or_init(|| {
            let mut overrides = PackOverrides::shared_write(&self.overrides_shared);
            overrides.overrides.entry(self.path()).or_default().clone()
        })
    }
    pub fn overrides_read(&self) -> Option<sync::RwLockReadGuard<'_, MarkerOverrides>> {
        self.overrides_ref().map(|o| MarkerOverrides::shared_read(o))
    }
    pub fn overrides_write(&self) -> sync::RwLockWriteGuard<'_, MarkerOverrides> {
        MarkerOverrides::shared_write(self.overrides_mut())
    }

    pub fn lookup_override<A>(&self) -> Option<Option<A::Owned>>
    where
        A: ?Sized + AttrKeyValue + AttrKey + ToOwned,
    {
        if let Some(o) = self.overrides_read() {
            match o.get::<A>() {
                Some(Some(v)) => Some(Some(v.get().to_owned())),
                Some(None) => Some(None),
                None => None,
            }
        } else {
            None
        }
    }
    pub fn lookup_override_dyn(&self, key: PackKeyId) -> Option<Option<PackValueDyn>> {
        if let Some(o) = self.overrides_read() {
            match o.get_dyn(key) {
                Some(Some(v)) => Some(Some(v.get_dyn().to_owned())),
                Some(None) => Some(None),
                None => None,
            }
        } else {
            None
        }
    }
    pub fn lookup_attr<A>(&self) -> Option<Cow<'_, A>>
    where
        A: ?Sized + AttrKeyValue + AttrKey + ToOwned,
        M: GetAttr<A>, //MarkerAttributes: GetAttr<A>,
    {
        if let Some(o) = self.lookup_override::<A>() {
            return o.map(Cow::Owned)
        }
        self.get_pack_attr::<A>()
    }
    pub fn lookup_attr_dyn(&self, key: PackKeyId) -> Option<Cow<'_, dyn AttrKeyValue>>
    where
        M: GetAttrDyn,
    {
        if let Some(o) = self.lookup_override_dyn(key) {
            return o.map(Cow::Owned)
        }
        self.marker.get_attr_dyn(key)
    }
}
impl<M> PackMarkerMut<M> {
    pub fn get_pack_attr<A>(&self) -> Option<Cow<'_, A>>
    where
        A: ?Sized + AttrKey + ToOwned,
        M: GetAttr<A>, //MarkerAttributes: GetAttr<A>,
    {
        self.marker.get_attr()
    }
}
impl<M> ops::Deref for PackMarkerMut<M> {
    type Target = M;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.marker
    }
}
impl<M> AsRef<Pack> for PackMarkerMut<M>
where
    M: AsRef<Pack>,
{
    #[inline(always)]
    fn as_ref(&self) -> &Pack {
        self.marker.as_ref()
    }
}
impl<M> Borrow<Pack> for PackMarkerMut<M>
where
    M: AsRef<Pack>,
{
    #[inline(always)]
    fn borrow(&self) -> &Pack {
        self.marker.as_ref()
    }
}
impl<M> Borrow<Arc<Pack>> for PackMarkerMut<M>
where
    M: AsRef<Arc<Pack>>,
{
    #[inline(always)]
    fn borrow(&self) -> &Arc<Pack> {
        self.marker.as_ref()
    }
}
