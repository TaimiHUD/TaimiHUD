use {
    super::PathingShared,
    crate::controller::pathing::{
        info::{self, LoadedMarkerInfo, LoadedPoiInfo, LoadedTrailInfo, MapPackInfo},
        registry::{
            LoadedCategoryIndex,
            LoadedCategoryNs,
            LoadedCategoryPath,
            LoadedMarkerPath,
            LoadedPoiIndex,
            LoadedPoiNs,
            LoadedPoiPath,
            LoadedTrailIndex,
            LoadedTrailNs,
            LoadedTrailPath,
            PackBoxOf,
            PackMapPath,
            PackPath,
            PoiMapPath,
            TrailMapPath,
        },
        shared::LocDisplay,
        space::DrawSpace,
        state::{LoadedCategory, LoadedMapPack, LoadedPoi, LoadedTrail},
    },
    glamour::Point3,
    std::{cmp, fmt, mem, ops, sync::Arc},
    taimi_hoard::{
        collections::TaimiSet,
        iters::{all_zipped, IterExt as _},
        loc::{indexed::IndexedList, LocationMut, LocationRef, Locator},
    },
    taimi_meta::packs::{
        id::{MarkerId, MarkerIndex, MarkerPath},
        CategoryIndex,
        CategoryPath,
        MapIndex,
        PoiPath,
        TrailPath,
        TrailSectionIndex,
        VisibilityFlags,
    },
    taimi_pack::attributes::{
        keys::Guid,
        FilterAttributes,
        InteractionAttributes,
        PoiAttributes,
        RenderAttributes,
        TrailAttributes,
    },
    taimi_sync::arcs::ArcPtrCmp,
};

#[cfg(feature = "paths-filter")]
use crate::controller::pathing::state::hidden::MarkerState;
#[cfg(not(feature = "paths-filter"))]
type MarkerState = ();

#[derive(Debug, Clone, Default)]
pub struct SharedGameplayMap {
    pub map_id: Option<MapIndex>,
    pub info: PackBoxOf<Option<SharedMapPackLoaded>>,
    pub state: PackBoxOf<Option<SharedMapPackState>>,
}
impl SharedGameplayMap {
    pub fn empty_for(map_id: Option<MapIndex>, pack_count: usize) -> Self {
        Self {
            map_id,
            info: PackBoxOf::new(vec![None; pack_count].into_boxed_slice()),
            state: PackBoxOf::new(vec![None; pack_count].into_boxed_slice()),
        }
    }

    pub fn clear(&mut self) {
        self.update_prune_for(false);
    }
    pub fn clear_for(&mut self, map_id: Option<MapIndex>) {
        self.clear();
        self.map_id = map_id;
    }
    pub fn prepare_for_map(&mut self, map_id: Option<MapIndex>) -> bool {
        if self.map_id != map_id {
            self.clear_for(map_id);
            true
        } else {
            false
        }
    }

    pub fn last_pack_path(&self) -> Option<PackPath> {
        self.info.rposition(Option::is_some)
    }

    pub fn cloned(&self) -> Self {
        let map_id = self.map_id;
        let (info, state) = self
            .map_id
            .and_then(|_| self.last_pack_path())
            .and_then(|last_path| {
                let range = 0..=last_path.path as usize;
                // TODO: get_unchecked?
                let info = self.info.data.get(range.clone())?;
                let state = self.state.data.get(range)?;
                Some((PackBoxOf::new(Box::from(info)), PackBoxOf::new(Box::from(state))))
            })
            .unwrap_or_default();
        Self { map_id, info, state }
    }

    pub(crate) fn update_prune_for<C>(&mut self, keep: C) -> bool
    where
        C: TaimiSet<PackPath>,
    {
        let mut dirty = false;
        for (path, info) in &mut self.info {
            if keep.set_contains(&path) {
                continue
            }
            dirty |= info.is_some();
            let _ = info.take();
        }
        for (path, state) in &mut self.state {
            if keep.set_contains(&path) {
                continue
            }
            dirty |= state.is_some();
            let _ = state.take();
        }
        dirty
    }

    pub fn for_pack_mut(
        &mut self,
        path: PackPath,
    ) -> (&mut Option<SharedMapPackLoaded>, &mut Option<SharedMapPackState>) {
        let info = self.info.lookup_extend_with(path.path, || None);
        let state = self.state.lookup_extend_with(path.path, || None);
        (info, state)
    }
    pub fn for_mut(
        &mut self,
        path: PackMapPath,
    ) -> (&mut Option<SharedMapPackLoaded>, &mut Option<SharedMapPackState>) {
        if self.map_id != Some(path.path) {
            self.clear_for(Some(path.path));
        }
        self.for_pack_mut(path.root)
    }
    fn consistency_check(path: PackMapPath, map_some: bool, info_some: bool) {
        if map_some && !info_some {
            log::debug!(
                "INCONSISTENT map state for {path}: info({}) != state({})",
                info_some,
                map_some
            );
        }
    }
    pub fn iter_state_mut(
        &mut self,
    ) -> impl Iterator<Item = (PackMapPath, &mut SharedMapPackState, &mut SharedMapPackLoaded)> {
        let map_id = self.map_id;
        self.state
            .iter_mut()
            .zip(self.info.values_mut())
            .filter_map(move |((path, map), info)| {
                let map_id = map_id?;
                let path = path.rel(map_id);
                Self::consistency_check(path, map.is_some(), info.is_some());
                let info = info.as_mut()?;
                map.as_mut().map(|map| (path, map, info))
            })
    }
    pub fn iter_state(
        &self,
    ) -> impl Iterator<Item = (PackMapPath, &SharedMapPackState, &SharedMapPackLoaded)> {
        self.iter_loaded()
            .filter_map(|(path, info, state)| state.map(|state| (path, state, info)))
    }
    pub fn iter_loaded(
        &self,
    ) -> impl Iterator<Item = (PackMapPath, &SharedMapPackLoaded, Option<&SharedMapPackState>)> {
        self.iter()
            .filter_map(|(path, info, state)| info.map(|info| (path, info, state)))
    }
    pub fn iter(
        &self,
    ) -> impl Iterator<
        Item = (
            PackMapPath,
            Option<&SharedMapPackLoaded>,
            Option<&SharedMapPackState>,
        ),
    > {
        let map_id = self.map_id;
        self.state
            .iter()
            .zip(self.info.values())
            .filter_map(move |((path, map), info)| {
                let map_id = map_id?;
                let path = path.rel(map_id);
                Self::consistency_check(path, map.is_some(), info.is_some());
                Some((path, info.as_ref(), map.as_ref()))
            })
    }
    pub fn get_mut(&mut self, map_id: MapIndex) -> Option<&mut Self> {
        match self.map_id {
            None => None,
            Some(map) if map != map_id => None,
            Some(..) => Some(self),
        }
    }
    pub fn get_ref(&self, map_id: MapIndex) -> Option<&Self> {
        match self.map_id {
            Some(map) if map == map_id => Some(self),
            _ => None,
        }
    }
    pub fn get_state_mut(&mut self, path: PackMapPath) -> Option<&mut SharedMapPackState> {
        let Locator { root, path } = path;
        self.get_mut(path)
            .and_then(|map| map.state.lookup_mut(&root))
            .and_then(|state| state.as_mut())
    }
    pub fn get_state(&self, path: PackMapPath) -> Option<&SharedMapPackState> {
        let Locator { root, path } = path;
        self.get_ref(path)
            .and_then(|map| map.state.lookup_ref(&root))
            .and_then(|state| state.as_ref())
    }
    pub fn get_info_mut(&mut self, path: PackMapPath) -> Option<&mut SharedMapPackLoaded> {
        let Locator { root, path } = path;
        self.get_mut(path)
            .and_then(|map| map.info.lookup_mut(&root))
            .and_then(|info| info.as_mut())
    }
    pub fn get_info_for(&self, path: PackPath) -> Option<(PackMapPath, &SharedMapPackLoaded)> {
        let map_id = self.map_id?;
        let path = path.rel(map_id);
        self.info
            .lookup_ref(&path.root)
            .and_then(|info| info.as_ref())
            .map(|info| (path, info))
    }
    pub fn iter_markers(&self) -> impl Iterator<Item = SharedMarkerRef<'_>> {
        self.iter_loaded().flat_map(move |(_map_path, map_info, map)| {
            map_info
                .loaded_pois(map)
                .map(|poi| poi.into_marker())
                .chain(map_info.loaded_trails(map).map(|poi| poi.into_marker()))
        })
    }
    pub fn iter_markers_loaded(&self) -> impl Iterator<Item = LoadedMarkerRef<'_>> {
        self.iter_state().flat_map(move |(_map_path, map, map_info)| {
            map.loaded_pois(map_info)
                .lazy_map(LoadedMarkerRef::Poi)
                .chain(map.loaded_trails(map_info).lazy_map(LoadedMarkerRef::Trail))
        })
    }

    pub fn ref_loaded_marker(&self, loaded_id: MarkerId) -> Option<SharedMarkerRef<'_>> {
        let pack = loaded_id.get_marker_pack_path();
        match (self.info.lookup_ref(&pack), self.state.lookup_ref(&pack)) {
            (Some(Some(map_info)), Some(map)) =>
                SharedMarkerRef::from_loaded_id(map_info, map.as_ref(), loaded_id),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SharedMapPackLoaded {
    pub path: PackMapPath,
    pub info: Arc<MapPackInfo>,
    pub pois: IndexedList<LoadedPoiNs, LoadedPoiIndex, Arc<[LoadedPoiInfo]>>,
    pub trails: IndexedList<LoadedTrailNs, LoadedTrailIndex, Arc<[LoadedTrailInfo]>>,
    pub poi_guids: Arc<[Guid]>,
}
impl SharedMapPackLoaded {
    #[cfg(todo)]
    pub fn with_info(path: PackMapPath, info: Arc<MapPackInfo>) -> Self {
        Self {
            path,
            poi_guids: Default::default(),
            info,
            pois: Default::default(),
            trails: Default::default(),
        }
    }
    pub fn pois(&self) -> &IndexedList<LoadedPoiNs, LoadedPoiIndex, [LoadedPoiInfo]> {
        IndexedList::from_ref(&self.pois)
    }
    pub fn trails(&self) -> &IndexedList<LoadedTrailNs, LoadedTrailIndex, [LoadedTrailInfo]> {
        IndexedList::from_ref(&self.trails)
    }

    pub fn with_loaded(path: PackMapPath, info: Arc<MapPackInfo>, map_pack: &LoadedMapPack) -> Self {
        Self {
            path,
            poi_guids: map_pack.poi_guids.clone(),
            info,
            pois: map_pack.pois.iter().map(|poi| poi.info().clone()).collect(),
            trails: map_pack.trails.iter().map(|trail| trail.info().clone()).collect(),
        }
    }
    pub fn update_with_info(&mut self, info: &Arc<MapPackInfo>) -> bool {
        ArcPtrCmp::from_mut(&mut self.info).clone_from_arc(info)
    }
    pub fn update_with(&mut self, map_pack: &LoadedMapPack) -> bool {
        let mut dirty = false;
        dirty |= ArcPtrCmp::from_mut(&mut self.poi_guids).clone_from_arc(&map_pack.poi_guids);
        if !all_zipped(
            |l, r| l.info().sig() == r.sig(),
            map_pack.pois.iter(),
            self.pois.data.iter(),
        ) {
            // XXX: could try to do partial update?
            self.pois = map_pack.pois.iter().map(|poi| poi.info().clone()).collect();
            dirty = true;
        }
        if !all_zipped(
            |l, r| l.info().sig() == r.sig(),
            map_pack.trails.iter(),
            self.trails.data.iter(),
        ) {
            // XXX: could try to do partial update?
            self.trails = map_pack.trails.iter().map(|trail| trail.info().clone()).collect();
            dirty = true;
        }
        dirty
    }

    pub fn poi_guids<'a>(&'a self) -> impl Iterator<Item = (PoiPath, Option<&'a Guid>)> + 'a {
        let mut poi_guids = self.poi_guids.iter();
        self.info
            .pois()
            .zip(self.info.poi_guid_mask())
            .map(move |(path, mask)| {
                (path, match mask {
                    true => poi_guids.next(),
                    false => None,
                })
            })
    }
    pub fn poi_guid_by_index<'a>(&'a self, path: LoadedPoiPath) -> Option<&'a Guid> {
        match self
            .info
            .poi_guid_filter(self.info.loaded_pois())
            .enumerate()
            .find(|(_, (p, _))| *p >= path)
        {
            Some((i, (p, _))) if p == path => self.poi_guids.get(i),
            _ => None,
        }
    }
    pub fn poi_guid_by_path<'a>(&'a self, path: PoiPath) -> Option<&'a Guid> {
        match self
            .info
            .poi_guid_filter(self.info.loaded_pois())
            .enumerate()
            .find(|(_, (_, p))| *p >= path)
        {
            Some((i, (_, p))) if p == path => self.poi_guids.get(i),
            _ => None,
        }
    }

    pub fn pois_iter<'a>(&'a self) -> impl DoubleEndedIterator<Item = SharedPoiRef<'a>> {
        self.pois().paths().lazy_map(|loaded_path| unsafe {
            let loaded_index = loaded_path.pivot_to();
            SharedPoiRef::new_unchecked(SharedMarkerRef::from_parts_unchecked(self, None, loaded_index))
        })
    }
    pub fn trails_iter<'a>(&'a self) -> impl DoubleEndedIterator<Item = SharedTrailRef<'a>> {
        self.trails().paths().lazy_map(|loaded_path| unsafe {
            let loaded_index = loaded_path.pivot_to();
            SharedTrailRef::new_unchecked(SharedMarkerRef::from_parts_unchecked(self, None, loaded_index))
        })
    }
    pub fn loaded_pois<'a>(
        &'a self,
        map: Option<&'a SharedMapPackState>,
    ) -> impl DoubleEndedIterator<Item = SharedPoiRef<'a>> {
        let amt = map.map(|map| map.pois().len()).unwrap_or(0);
        self.pois_iter().lazy_map(move |mut shared| {
            shared.marker.map = ((shared.loaded_index().path as usize) < amt)
                .then_some(map)
                .flatten();
            shared
        })
    }
    pub fn loaded_trails<'a>(
        &'a self,
        map: Option<&'a SharedMapPackState>,
    ) -> impl DoubleEndedIterator<Item = SharedTrailRef<'a>> {
        let amt = map.map(|map| map.trails().len()).unwrap_or(0);
        self.trails_iter().lazy_map(move |mut shared| {
            shared.marker.map = ((shared.loaded_index().path as usize) < amt)
                .then_some(map)
                .flatten();
            shared
        })
    }
}
impl ops::Deref for SharedMapPackLoaded {
    type Target = MapPackInfo;
    fn deref(&self) -> &Self::Target {
        &self.info
    }
}

#[derive(Debug, Clone, Default)]
pub struct SharedMapPackState {
    pub categories: Arc<[LoadedCategory]>,
    pub pois: IndexedList<LoadedPoiNs, LoadedPoiIndex, Arc<[LoadedPoiShared]>>,
    pub trails: IndexedList<LoadedTrailNs, LoadedTrailIndex, Arc<[LoadedTrailShared]>>,
    #[cfg(feature = "paths-filter")]
    pub hidden_markers: Arc<[MarkerId]>,
}

impl SharedMapPackState {
    pub fn with_static(_path: PackMapPath, map_pack: &LoadedMapPack) -> Self {
        Self {
            categories: map_pack.categories.clone(),
            pois: map_pack.pois.iter().map(LoadedPoiShared::with_loaded).collect(),
            trails: map_pack
                .trails
                .iter()
                .map(LoadedTrailShared::with_loaded)
                .collect(),
            #[cfg(feature = "paths-filter")]
            hidden_markers: Default::default(),
        }
    }
    pub fn with_loaded(path: PackMapPath, map_pack: &LoadedMapPack, state: &MarkerState) -> Self {
        Self {
            categories: map_pack.categories.clone(),
            pois: map_pack.pois.iter().map(LoadedPoiShared::with_loaded).collect(),
            trails: map_pack
                .trails
                .iter()
                .map(LoadedTrailShared::with_loaded)
                .collect(),
            #[cfg(feature = "paths-filter")]
            hidden_markers: Self::hidden_markers_from(path, state, map_pack)
                .cloned()
                .collect(),
        }
    }

    pub fn pois(&self) -> &IndexedList<LoadedPoiNs, LoadedPoiIndex, [LoadedPoiShared]> {
        IndexedList::from_ref(&self.pois)
    }
    pub fn trails(&self) -> &IndexedList<LoadedTrailNs, LoadedTrailIndex, [LoadedTrailShared]> {
        IndexedList::from_ref(&self.trails)
    }
    pub fn categories(&self) -> &IndexedList<LoadedCategoryNs, LoadedCategoryIndex, [LoadedCategory]> {
        IndexedList::from_ref(&self.categories)
    }

    pub fn update_static(&mut self, map_pack: &LoadedMapPack) -> bool {
        if Arc::ptr_eq(&self.categories, &map_pack.categories) {
            return false
        }
        true
    }
    pub fn write_with_loaded(&mut self, map_pack: &LoadedMapPack) {
        self.trails = map_pack
            .trails
            .iter()
            .map(LoadedTrailShared::with_loaded)
            .collect();
        self.pois = map_pack.pois.iter().map(LoadedPoiShared::with_loaded).collect();
    }
    pub fn update_with_loaded(&mut self, map_pack: &LoadedMapPack) -> bool {
        let mut trails_dirty = self.trails.data.len() != map_pack.trails.len();
        if trails_dirty {
            self.trails = map_pack
                .trails
                .iter()
                .map(LoadedTrailShared::with_loaded)
                .collect();
        } else {
            let has_changes = self
                .trails
                .data
                .iter()
                .map(|t| t.sig())
                .zip(map_pack.trails.iter().map(LoadedTrailShared::sig_loaded))
                .any(|(l, r)| l != r);
            if has_changes {
                let trails_mut = Arc::make_mut(&mut self.trails.data).iter_mut();
                for (dest, trail) in trails_mut.zip(map_pack.trails.iter()) {
                    trails_dirty |= dest.update_from_loaded(trail);
                }
            }
        }
        let mut pois_dirty = self.pois.data.len() != map_pack.pois.len();
        if pois_dirty {
            self.pois = map_pack.pois.iter().map(LoadedPoiShared::with_loaded).collect();
        } else {
            let has_changes = self
                .pois
                .data
                .iter()
                .map(|t| t.sig())
                .zip(map_pack.pois.iter().map(LoadedPoiShared::sig_loaded))
                .any(|(l, r)| l != r);
            if has_changes {
                let pois_mut = Arc::make_mut(&mut self.pois.data).iter_mut();
                for (dest, poi) in pois_mut.zip(map_pack.pois.iter()) {
                    pois_dirty |= dest.update_from_loaded(poi);
                }
            }
        }
        trails_dirty | pois_dirty
    }
    /// TODO: check if changed properly...
    #[cfg(feature = "paths-filter")]
    pub fn update_with_hidden(
        &mut self,
        path: PackMapPath,
        state: &MarkerState,
        map_pack: &LoadedMapPack,
    ) -> Option<bool> {
        let mut hidden_dirty = true;
        if hidden_dirty {
            let prev_len = self.hidden_markers.len();
            self.hidden_markers = Self::hidden_markers_from(path, state, map_pack)
                .cloned()
                .collect();
            hidden_dirty = prev_len != self.hidden_markers.len();
        }
        match hidden_dirty {
            false => None,
            true => Some(true),
        }
    }

    pub fn category_paths<'a, 'i>(
        &'a self,
        info: &'i MapPackInfo,
    ) -> impl Iterator<Item = (CategoryPath, &'a LoadedCategory)> + 'i
    where
        'a: 'i,
    {
        info.categories().zip(self.categories.iter())
    }

    #[cfg(feature = "paths-filter")]
    fn hidden_markers_from<'a: 'b, 'b>(
        map_path: PackMapPath,
        state: &'a MarkerState,
        map_pack: &'b LoadedMapPack,
    ) -> impl Iterator<Item = &'a MarkerId> + 'b {
        let pack_path = map_path.root;
        let poi_guids = &map_pack.poi_guids;
        state.hidden.keys().filter(move |id| match id {
            id if id
                .marker_path::<PackMapPath>()
                .map(|path| path.root == map_path)
                .unwrap_or(false) =>
                true,
            id if id
                .marker_path::<PackPath>()
                .map(|path| path.root == pack_path)
                .unwrap_or(false) =>
                true,
            _ => poi_guids.contains(Guid::from_uuid_ref(id)),
        })
    }
    #[cfg(feature = "paths-filter")]
    pub fn is_hidden(&self, marker_ids: &[MarkerId]) -> bool {
        match marker_ids {
            #[cfg(todo = "unnecessary")]
            marker_ids => self
                .hidden_markers
                .iter()
                .any(|hidden| marker_ids.contains(hidden)),
            marker_ids => self.any_hidden(marker_ids),
        }
    }
    #[cfg(feature = "paths-filter")]
    pub fn any_hidden<'a, I: IntoIterator<Item = &'a MarkerId>>(&self, marker_ids: I) -> bool {
        marker_ids
            .into_iter()
            .any(|mid| self.hidden_markers[..].binary_search(mid).is_ok())
    }

    pub fn loaded_pois<'a>(
        &'a self,
        info: &'a SharedMapPackLoaded,
    ) -> impl DoubleEndedIterator<Item = LoadedPoiRef<'a>> {
        info.loaded_pois(Some(self))
            .lazy_map(|shared| unsafe { shared.to_loaded_unchecked() })
    }
    pub fn loaded_trails<'a>(
        &'a self,
        info: &'a SharedMapPackLoaded,
    ) -> impl DoubleEndedIterator<Item = LoadedTrailRef<'a>> {
        info.loaded_trails(Some(self))
            .lazy_map(|shared| unsafe { shared.to_loaded_unchecked() })
    }

    pub fn is_empty(&self) -> bool {
        self.categories.is_empty()
    }
    pub fn clear(&mut self) {
        if self.is_empty() { return }
        *self = Default::default();
    }
}

#[derive(Debug, Clone, Default)]
pub struct LoadedPoiShared {
    pub visibility: VisibilityFlags,
    pub position: Point3<DrawSpace>,
    #[cfg(todo)]
    overrides: Option<Box<RenderAttributes>>,
}
impl LoadedPoiShared {
    pub fn with_loaded(lpoi: &LoadedPoi) -> Self {
        Self {
            visibility: lpoi.visibility,
            position: lpoi.position(),
        }
    }
    pub fn update_from_loaded(&mut self, lpoi: &LoadedPoi) -> bool {
        let mut dirty = self.visibility != lpoi.visibility;
        self.visibility = lpoi.visibility;
        let position = lpoi.position();
        dirty |= self.position != position;
        self.position = position;
        dirty
    }

    pub(crate) fn sig(&self) -> [u32; 3] {
        let [s0, s1, s2] = self.position.to_array();
        let mut s0 = f32::to_bits(s0);
        s0 ^= self.visibility.bits() as u32;
        [s0, f32::to_bits(s1), f32::to_bits(s2)]
    }
    pub(crate) fn sig_loaded(lpoi: &LoadedPoi) -> [u32; 3] {
        Self::with_loaded(lpoi).sig()
    }
}

#[derive(Debug, Clone, Default)]
pub struct LoadedTrailShared {
    pub visibility: VisibilityFlags,
    #[cfg(todo)]
    pub section_count: TrailSectionIndex,
}
impl LoadedTrailShared {
    pub fn with_loaded(ltrail: &LoadedTrail) -> Self {
        Self {
            visibility: ltrail.visibility,
            #[cfg(todo)]
            section_count: 0,
        }
    }
    pub fn update_from_loaded(&mut self, ltrail: &LoadedTrail) -> bool {
        let dirty = self.visibility != ltrail.visibility;
        self.visibility = ltrail.visibility;
        //self.section_count = ltrail.sections.len();
        dirty
    }

    #[cfg(todo)]
    pub fn section_len(&self) -> usize {
        self.section_count as usize
    }

    pub(crate) fn sig(&self) -> [u32; 2] {
        let mut sig = match () {
            #[cfg(todo)]
            _ => [self.section_count, 0u32],
            #[cfg(todo)]
            _ => Self::sig_sections(&self.section_info),
            _ => [0u32; 2],
        };
        sig[0] ^= self.visibility.bits() as u32;
        sig
    }
    pub(crate) fn sig_loaded(ltrail: &LoadedTrail) -> [u32; 2] {
        let mut sig = match () {
            #[cfg(todo)]
            _ => [ltrail.sections.len() as u32, 0u32],
            #[cfg(todo)]
            _ => Self::sig_sections(&ltrail.section_info),
            _ => [0u32; 2],
        };
        sig[0] ^= ltrail.visibility.bits() as u32;
        sig
    }
    #[cfg(todo)]
    pub fn sig_sections(sections: &LoadedTrailGeometryInfo) -> [u32; 2] {
        let s = sections as *const _ as usize;
        [s as u32, (s >> 32) as u32]
    }
    #[cfg(todo = "unnecessary")]
    pub fn sig_sections(sections: &LoadedTrailGeometryInfo) -> [u32; 2] {
        let (s0, s1) = self.section_info.sections_sig();
        [s0.map(|v| v as u32).unwrap_or(u32::MAX), s1]
    }
}
impl LoadedPoiInfo {
    pub(crate) fn sig(&self) -> [u32; 2] {
        let Self { marker_info } = self;
        marker_info.sig()
    }
}
impl LoadedTrailInfo {
    pub(crate) fn sig(&self) -> [u32; 2] {
        let Self { marker_info, trl } = self;
        let mut sig = marker_info.sig();
        if let [ref mut s0, ref mut s1] = sig {
            *s0 ^= self.category_path.path as u32;
            if let Some(trl) = trl {
                #[cfg(todo)]
                if let Some(parent) = &trl.parent_path {}
                let [p0, p1] = LoadedMarkerInfo::sig_ptr(Arc::as_ptr(&trl.path) as *const ());
                *s0 ^= p0;
                *s1 ^= p1;
            }
        }
        sig
    }
}

#[derive(Clone)]
pub struct SharedMarkerRef<'a> {
    loaded_index: LoadedMarkerPath,
    map_info: &'a SharedMapPackLoaded,
    map: Option<&'a SharedMapPackState>,
}
impl<'a> SharedMarkerRef<'a> {
    /// TODO
    pub fn from_path(
        map_info: &'a SharedMapPackLoaded,
        map: Option<&'a SharedMapPackState>,
        path: MarkerPath<PackPath>,
    ) -> Option<Self> {
        let loaded_path: Option<MarkerPath> = match path {
            path if path.root != map_info.path.root => None,
            path => Some(path.unscope()),
        }
        .and_then(|path| map_info.marker_index(path));
        loaded_path.and_then(|loaded_path| match map {
            #[cfg(todo)]
            None => Some(unsafe { Self::from_parts_unchecked(map_info, map, loaded_path) }),
            map => Self::from_parts(map_info, map, loaded_path),
        })
    }
    /// only check map
    ///
    /// TODO
    pub unsafe fn from_loaded_path_unchecked(
        map_info: &'a SharedMapPackLoaded,
        map: Option<&'a SharedMapPackState>,
        loaded_path: LoadedMarkerPath,
    ) -> Option<Self> {
        Self::from_parts(map_info, map, loaded_path)
    }
    pub fn from_loaded_id(
        map_info: &'a SharedMapPackLoaded,
        map: Option<&'a SharedMapPackState>,
        loaded_id: MarkerId,
    ) -> Option<Self> {
        loaded_id
            .marker_path()
            .and_then(|loaded_path| Self::from_loaded_path(map_info, map, loaded_path))
    }
    pub fn from_loaded_path(
        map_info: &'a SharedMapPackLoaded,
        map: Option<&'a SharedMapPackState>,
        loaded_path: LoadedMarkerPath<PackMapPath>,
    ) -> Option<Self> {
        if map_info.path != loaded_path.root {
            return None
        }
        Self::from_parts(map_info, map, loaded_path.unscope())
    }
    pub fn from_parts(
        map_info: &'a SharedMapPackLoaded,
        map: Option<&'a SharedMapPackState>,
        loaded_index: LoadedMarkerPath,
    ) -> Option<Self> {
        let mut trail_section = TrailSectionIndex::MAX;
        let (idx, len_info, len_map) = match loaded_index.path.namespace() {
            MarkerIndex::NS_CAT => (
                loaded_index.path.index_category_unchecked() as usize,
                map_info.categories().count(),
                map.map(|map| map.categories().len()),
            ),
            MarkerIndex::NS_POI => (
                loaded_index.path.index_poi_unchecked() as usize,
                map_info.pois().len(),
                map.map(|map| map.pois().len()),
            ),
            MarkerIndex::NS_TRAIL => {
                let (i, seci) = loaded_index.path.index_trail_section_unchecked();
                trail_section = seci;
                (
                    i as usize,
                    map_info.trails().len(),
                    map.map(|map| map.trails().len()),
                )
            },
            _ => return None,
        };
        if idx >= len_info {
            return None
        }
        let map = match len_map {
            Some(len_map) if idx >= len_map => {
                log::debug!("DELETEME: {loaded_index} map state expected");
                None
            },
            _ => map,
        };
        if trail_section != TrailSectionIndex::MAX {
            // TODO bleh
        }
        Some(unsafe { Self::from_parts_unchecked(map_info, map, loaded_index) })
    }
    #[inline]
    pub const unsafe fn from_parts_unchecked(
        map_info: &'a SharedMapPackLoaded,
        map: Option<&'a SharedMapPackState>,
        loaded_index: LoadedMarkerPath,
    ) -> Self {
        Self { map_info, map, loaded_index }
    }

    #[inline]
    pub fn loaded_id(&self) -> MarkerId {
        MarkerId::for_marker(self.loaded_path())
    }
    #[inline(always)]
    pub fn loaded_index(&self) -> LoadedMarkerPath {
        self.loaded_index
    }
    #[inline]
    pub fn map_info(&self) -> &'a SharedMapPackLoaded {
        self.map_info
    }
    #[inline]
    pub fn map(&self) -> Option<&'a SharedMapPackState> {
        self.map
    }
    #[inline]
    pub unsafe fn map_unchecked(&self) -> &'a SharedMapPackState {
        self.map.unwrap_unchecked()
    }
    #[inline]
    pub fn pack_path(&self) -> PackPath {
        self.map_path().root
    }
    #[inline]
    pub fn map_path(&self) -> PackMapPath {
        self.map_info.path
    }
    #[inline]
    pub fn loaded_path(&self) -> LoadedMarkerPath<PackMapPath> {
        self.loaded_index().pivot(self.map_path())
    }
    /// as opposed to [Self::loaded_index]
    #[inline]
    pub fn marker_index(&self) -> MarkerPath {
        unsafe { self.map_info.info.marker_path_unchecked(self.loaded_index()) }
    }
    /// as opposed to [Self::loaded_path]
    pub fn path(&self) -> MarkerPath<PackPath> {
        self.marker_index().pivot(self.pack_path())
    }
    /// as opposed to [self.loaded_id]
    pub fn marker_id(&self) -> MarkerId {
        MarkerId::for_marker(self.path())
    }

    pub fn loaded_category_path(&self) -> Option<LoadedCategoryPath> {
        (self.loaded_index().path.namespace() == MarkerIndex::NS_CAT)
            .then_some(self.loaded_category_path_unchecked())
    }
    #[inline(always)]
    fn loaded_category_path_unchecked(&self) -> LoadedCategoryPath {
        LoadedPoiPath::with_path(self.loaded_index().path.index_category_unchecked())
    }
    pub fn loaded_poi_path(&self) -> Option<LoadedPoiPath> {
        (self.loaded_index().path.namespace() == MarkerIndex::NS_POI)
            .then_some(self.loaded_poi_path_unchecked())
    }
    #[inline(always)]
    fn loaded_poi_path_unchecked(&self) -> LoadedPoiPath {
        LoadedPoiPath::with_path(self.loaded_index().path.index_poi_unchecked())
    }
    pub fn category_path(&self) -> Option<CategoryPath> {
        let lpath = self.loaded_category_path()?;
        Some(unsafe { self.map_info.info.category_path_unchecked(lpath) })
    }
    pub fn poi_path(&self) -> Option<PoiPath> {
        let lpath = self.loaded_poi_path()?;
        Some(unsafe { self.map_info.info.poi_path_unchecked(lpath) })
    }
    pub fn loaded_trail_path(&self) -> Option<LoadedTrailPath> {
        (self.loaded_index().path.namespace() == MarkerIndex::NS_TRAIL)
            .then_some(self.loaded_trail_path_unchecked())
    }
    #[inline(always)]
    fn loaded_trail_path_unchecked(&self) -> LoadedTrailPath {
        LoadedTrailPath::with_path(self.loaded_index().path.trail_index_unchecked())
    }
    pub fn trail_path(&self) -> Option<TrailPath> {
        let lpath = self.loaded_trail_path()?;
        Some(unsafe { self.map_info.info.trail_path_unchecked(lpath) })
    }
    pub fn loaded_category(&self) -> Option<&'a LoadedCategory> {
        let lpath = self.loaded_category_path()?;
        self.map()
            .map(|map| unsafe { map.categories().index_unchecked(lpath) })
    }
    pub fn loaded_poi_info(&self) -> Option<&'a LoadedPoiInfo> {
        let lpath = self.loaded_poi_path()?;
        Some(unsafe { self.map_info.pois().index_unchecked(lpath) })
    }
    pub fn loaded_trail_info(&self) -> Option<&'a LoadedTrailInfo> {
        let lpath = self.loaded_trail_path()?;
        Some(unsafe { self.map_info.trails().index_unchecked(lpath) })
    }
    pub fn loaded_poi(&self) -> Option<&'a LoadedPoiShared> {
        let lpath = self.loaded_poi_path()?;
        self.map().map(|map| unsafe { map.pois().index_unchecked(lpath) })
    }
    pub fn loaded_trail(&self) -> Option<&'a LoadedTrailShared> {
        let lpath = self.loaded_trail_path()?;
        self.map()
            .map(move |map| unsafe { map.trails().index_unchecked(lpath) })
    }

    /// TODO: categories please!
    pub fn marker_info(&self) -> Option<&'a LoadedMarkerInfo> {
        if let Some(poi) = self.loaded_poi_info() {
            Some(poi.marker_info())
        } else if let Some(trail) = self.loaded_trail_info() {
            Some(trail.marker_info())
        } else {
            None
        }
    }
    pub fn render_attrs(&self) -> &'a Arc<RenderAttributes> {
        self.marker_info()
            .map(|info| info.attrs())
            .unwrap_or(&info::EMPTY_RENDER_ATTRS)
    }
    #[cfg(feature = "paths-interact")]
    pub fn interaction_attrs(&self) -> &'a Arc<InteractionAttributes> {
        self.marker_info()
            .and_then(|info| info.get_interaction_attrs())
            .unwrap_or(&info::EMPTY_INTERACTION_ATTRS)
    }
    #[inline]
    pub fn filter_attrs(&self) -> &'a FilterAttributes {
        self.marker_info()
            .and_then(|info| info.get_filter_attrs())
            .map(|a| &**a)
            .unwrap_or(&info::EMPTY_FILTER_ATTRS)
    }

    #[inline]
    pub fn to_loaded(self) -> Option<LoadedMarkerRef<'a>> {
        LoadedMarkerRef::try_new(self)
    }
    #[inline]
    pub fn to_poi(self) -> Option<SharedPoiRef<'a>> {
        SharedPoiRef::try_new(self)
    }
    #[inline]
    pub fn to_loaded_poi(self) -> Option<LoadedPoiRef<'a>> {
        self.to_poi().and_then(LoadedPoiRef::try_new)
    }
    #[inline]
    pub fn to_trail(self) -> Option<SharedTrailRef<'a>> {
        SharedTrailRef::try_new(self)
    }
    #[inline]
    pub fn to_loaded_trail(self) -> Option<LoadedTrailRef<'a>> {
        self.to_trail().and_then(LoadedTrailRef::try_new)
    }
}
impl<'a> fmt::Debug for SharedMarkerRef<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let path = self.loaded_path();
        fmt::Debug::fmt(LocDisplay::from_ref(&path), f)
    }
}
#[derive(Debug, Clone)]
pub enum LoadedMarkerRef<'a> {
    Poi(LoadedPoiRef<'a>),
    Trail(LoadedTrailRef<'a>),
    #[cfg(todo)]
    Category(),
}
impl<'a> LoadedMarkerRef<'a> {
    pub fn try_new(marker: SharedMarkerRef<'a>) -> Option<Self> {
        match marker.loaded_index().path.namespace() {
            MarkerIndex::NS_POI => marker.to_loaded_poi().map(Self::Poi),
            MarkerIndex::NS_TRAIL => marker.to_loaded_trail().map(Self::Trail),
            #[cfg(todo)]
            MarkerIndex::NS_CAT => marker.to_loaded_trail().map(Self::Trail),
            _ => None,
        }
    }
    pub unsafe fn new_unchecked(marker: SharedMarkerRef<'a>) -> Option<Self> {
        match marker.loaded_index().path.namespace() {
            MarkerIndex::NS_POI => Some(Self::Poi(
                SharedPoiRef::new_unchecked(marker).to_loaded_unchecked(),
            )),
            MarkerIndex::NS_TRAIL => Some(Self::Trail(
                SharedTrailRef::new_unchecked(marker).to_loaded_unchecked(),
            )),
            #[cfg(todo)]
            MarkerIndex::NS_CAT => (),
            _ => None,
        }
    }
}
#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct SharedPoiRef<'a> {
    marker: SharedMarkerRef<'a>,
}
pub type SharedPoiInfo = CategoryIndex;
impl<'a> SharedPoiRef<'a> {
    pub fn try_new(marker: SharedMarkerRef<'a>) -> Option<Self> {
        #[cfg(todo = "unnecessary")]
        let _ = marker.poi_info()?;
        #[cfg(todo = "unnecessary")]
        let _ = marker.render_attrs().poi.as_ref()?;
        match marker.loaded_poi_path() {
            Some(..) => Some(unsafe { Self::new_unchecked(marker) }),
            None => None,
        }
    }
    #[inline]
    pub unsafe fn new_unchecked(marker: SharedMarkerRef<'a>) -> Self {
        Self { marker }
    }
    #[inline(always)]
    pub fn map_info(&self) -> &'a SharedMapPackLoaded {
        self.marker.map_info()
    }
    #[inline(always)]
    pub fn map(&self) -> Option<&'a SharedMapPackState> {
        self.marker.map()
    }

    #[inline]
    pub fn loaded_path(&self) -> PoiMapPath {
        self.loaded_index().pivot(self.marker.map_path())
    }
    #[inline]
    pub fn loaded_index(&self) -> LoadedPoiPath {
        self.marker.loaded_poi_path_unchecked()
    }
    #[inline]
    pub fn poi_path(&self) -> PoiPath {
        unsafe { self.map_info().info.poi_path_unchecked(self.loaded_index()) }
    }
    #[cfg(todo)]
    pub fn poi_info(&self) -> &SharedPoiInfo {
        let path = self.loaded_path().path;
        unsafe { self.marker.map_info.pois.get_unchecked(path as usize) }
    }
    #[inline]
    pub fn category_path(&self) -> CategoryPath {
        self.lpoi_info().category_path
    }
    #[inline]
    pub fn lpoi_info(&self) -> &'a LoadedPoiInfo {
        unsafe { self.map_info().pois().index_unchecked(self.loaded_index()) }
    }
    pub fn lpoi(&self) -> Option<&'a LoadedPoiShared> {
        self.map().map(|_map| unsafe {
            self.as_loaded_unchecked().lpoi()
            //map.pois().index_unchecked(self.loaded_index())
        })
    }
    #[inline]
    pub fn render_attrs(&self) -> &'a Arc<RenderAttributes> {
        self.lpoi_info().attrs()
    }
    #[inline]
    pub fn poi_attrs(&self) -> &'a Box<PoiAttributes> {
        unsafe { self.render_attrs().poi.as_ref().unwrap_unchecked() }
    }
    #[inline(always)]
    pub fn as_marker(&self) -> &SharedMarkerRef<'a> {
        &self.marker
    }
    #[inline]
    pub fn into_marker(self) -> SharedMarkerRef<'a> {
        self.marker
    }
    #[inline]
    pub fn to_loaded(self) -> Option<LoadedPoiRef<'a>> {
        LoadedPoiRef::try_new(self)
    }
    #[inline]
    pub fn as_loaded(&self) -> Option<&LoadedPoiRef<'a>> {
        LoadedPoiRef::try_new_ref(self)
    }
    #[inline(always)]
    pub unsafe fn to_loaded_unchecked(self) -> LoadedPoiRef<'a> {
        LoadedPoiRef::new_unchecked(self)
    }
    #[inline(always)]
    unsafe fn as_loaded_unchecked(&self) -> &LoadedPoiRef<'a> {
        LoadedPoiRef::new_ref_unchecked(self)
    }
}
#[cfg(todo)]
impl ops::Deref for SharedPoiRef<'a> {}
#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct LoadedPoiRef<'a> {
    marker: SharedPoiRef<'a>,
}
impl<'a> LoadedPoiRef<'a> {
    pub fn try_new(marker: SharedPoiRef<'a>) -> Option<Self> {
        let _ = marker.lpoi()?;

        Some(unsafe { Self::new_unchecked(marker) })
    }
    pub fn try_new_ref<'b>(marker: &'b SharedPoiRef<'a>) -> Option<&'b Self> {
        let _ = marker.lpoi()?;

        Some(unsafe { Self::new_ref_unchecked(marker) })
    }
    #[inline(always)]
    pub unsafe fn new_unchecked(marker: SharedPoiRef<'a>) -> Self {
        Self { marker }
    }
    #[inline(always)]
    pub unsafe fn new_ref_unchecked<'b>(marker: &'b SharedPoiRef<'a>) -> &'b Self {
        mem::transmute(marker)
    }
    #[inline(always)]
    pub fn map(&self) -> &'a SharedMapPackState {
        unsafe { self.marker.marker.map_unchecked() }
    }
    #[inline]
    pub fn lpoi(&self) -> &'a LoadedPoiShared {
        unsafe { self.map().pois().index_unchecked(self.loaded_index()) }
    }

    pub fn guid(&self) -> Option<&'a Guid> {
        self.marker
            .marker
            .map_info
            .poi_guid_by_index(self.marker.loaded_index())
    }
    #[cfg(feature = "paths-filter")]
    pub fn is_hidden(&self) -> bool {
        let guid = self.guid();
        let has_guid = guid.is_some();
        let mids = [
            MarkerId::with_uuid(guid.copied().unwrap_or_default().0),
            self.marker.marker.loaded_id(),
            self.marker.marker.marker_id(),
        ];
        let mids = match (has_guid, &mids[..]) {
            (false, &[_, ref rest @ ..]) => rest,
            (_, mids) => mids,
        };
        self.map().is_hidden(mids)
    }
}
impl<'a> ops::Deref for LoadedPoiRef<'a> {
    type Target = SharedPoiRef<'a>;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.marker
    }
}

#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct SharedTrailRef<'a> {
    marker: SharedMarkerRef<'a>,
}
pub type SharedTrailInfo = CategoryIndex;
impl<'a> SharedTrailRef<'a> {
    pub fn try_new(marker: SharedMarkerRef<'a>) -> Option<Self> {
        #[cfg(todo = "unnecessary")]
        let _ = marker.trail_info()?;
        #[cfg(todo = "unnecessary")]
        let _ = marker.render_attrs().trail.as_ref()?;
        match marker.loaded_trail_path() {
            Some(..) => Some(unsafe { Self::new_unchecked(marker) }),
            None => None,
        }
    }
    #[inline]
    pub unsafe fn new_unchecked(marker: SharedMarkerRef<'a>) -> Self {
        Self { marker }
    }
    #[inline(always)]
    pub fn map_info(&self) -> &'a SharedMapPackLoaded {
        self.marker.map_info()
    }
    #[inline(always)]
    pub fn map(&self) -> Option<&'a SharedMapPackState> {
        self.marker.map()
    }

    #[inline]
    pub fn loaded_path(&self) -> TrailMapPath {
        self.loaded_index().pivot(self.marker.map_path())
    }
    #[inline]
    pub fn loaded_index(&self) -> LoadedTrailPath {
        self.marker.loaded_trail_path_unchecked()
    }
    #[inline]
    pub fn trail_path(&self) -> TrailPath {
        unsafe { self.map_info().info.trail_path_unchecked(self.loaded_index()) }
    }
    #[cfg(todo)]
    pub fn trail_info(&self) -> &SharedTrailInfo {
        let path = self.loaded_path().path;
        unsafe { self.marker.map_info.trails.get_unchecked(path as usize) }
    }
    pub fn category_path(&self) -> CategoryPath {
        self.ltrail_info().category_path
    }
    #[inline]
    pub fn ltrail_info(&self) -> &'a LoadedTrailInfo {
        unsafe { self.map_info().trails().index_unchecked(self.loaded_index()) }
    }
    pub fn ltrail(&self) -> Option<&'a LoadedTrailShared> {
        self.map().map(|_map| unsafe {
            //map.trails().index_unchecked(self.loaded_index())
            self.as_loaded_unchecked().ltrail()
        })
    }
    #[inline]
    pub fn render_attrs(&self) -> &'a Arc<RenderAttributes> {
        self.ltrail_info().attrs()
    }
    #[inline]
    pub fn trail_attrs(&self) -> &'a Box<TrailAttributes> {
        unsafe { self.render_attrs().trail.as_ref().unwrap_unchecked() }
    }
    #[inline(always)]
    pub fn as_marker(&self) -> &SharedMarkerRef<'a> {
        &self.marker
    }
    #[inline]
    pub fn into_marker(self) -> SharedMarkerRef<'a> {
        self.marker
    }
    #[inline]
    pub fn to_loaded(self) -> Option<LoadedTrailRef<'a>> {
        LoadedTrailRef::try_new(self)
    }
    #[inline]
    pub fn as_loaded(&self) -> Option<&LoadedTrailRef<'a>> {
        LoadedTrailRef::try_new_ref(self)
    }
    #[inline(always)]
    pub unsafe fn to_loaded_unchecked(self) -> LoadedTrailRef<'a> {
        LoadedTrailRef::new_unchecked(self)
    }
    #[inline(always)]
    unsafe fn as_loaded_unchecked(&self) -> &LoadedTrailRef<'a> {
        LoadedTrailRef::new_ref_unchecked(self)
    }
}
#[cfg(todo)]
impl ops::Deref for SharedTrailRef<'a> {}
#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct LoadedTrailRef<'a> {
    marker: SharedTrailRef<'a>,
}
impl<'a> LoadedTrailRef<'a> {
    pub fn try_new(marker: SharedTrailRef<'a>) -> Option<Self> {
        let _ = marker.ltrail()?;

        Some(unsafe { Self::new_unchecked(marker) })
    }
    pub fn try_new_ref<'b>(marker: &'b SharedTrailRef<'a>) -> Option<&'b Self> {
        let _ = marker.ltrail()?;

        Some(unsafe { Self::new_ref_unchecked(marker) })
    }
    #[inline(always)]
    pub unsafe fn new_unchecked(marker: SharedTrailRef<'a>) -> Self {
        Self { marker }
    }
    #[inline(always)]
    pub unsafe fn new_ref_unchecked<'b>(marker: &'b SharedTrailRef<'a>) -> &'b Self {
        mem::transmute(marker)
    }
    #[inline(always)]
    pub fn map(&self) -> &'a SharedMapPackState {
        unsafe { self.marker.marker.map_unchecked() }
    }

    #[inline]
    pub fn ltrail(&self) -> &'a LoadedTrailShared {
        unsafe { self.map().trails().index_unchecked(self.loaded_index()) }
    }
    #[inline]
    pub fn trail_attrs(&self) -> &'a Box<TrailAttributes> {
        unsafe { self.render_attrs().trail.as_ref().unwrap_unchecked() }
    }
}
impl<'a> ops::Deref for LoadedTrailRef<'a> {
    type Target = SharedTrailRef<'a>;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.marker
    }
}

impl PathingShared {
    /// TODO: consider how/when this isn't dirty?
    pub(crate) fn update_map(
        &self,
        path: PackMapPath,
        map_info: &Arc<MapPackInfo>,
        map: &LoadedMapPack,
        notify: bool,
    ) -> bool {
        let state = SharedMapPackState::with_static(path, map);
        let mut dirty = false;
        #[cfg(todo)]
        {
            self.maps.send_if_modified(|shared_maps| {
                match shared_maps.map_info.entry(path) {
                    btree_map::Entry::Vacant(e) => {
                        // XXX: dumb clone because we might need it later, should this be an arc?
                        e.insert(info.clone());
                    },
                    btree_map::Entry::Occupied(e) => e.into_mut().clone_from(&info),
                }
                dirty |= true;
                dirty && notify
            });
        }
        self.gameplay.send_if_modified(move |shared_map| {
            dirty |= shared_map.prepare_for_map(Some(path.path));
            let (shared_info, shared) = shared_map.for_mut(path);
            if let Some(ref mut shared) = shared {
                shared.clone_from(&state);
            } else {
                *shared = Some(state);
            }
            dirty |= true;
            match shared_info {
                #[cfg(todo = "unnecessary")]
                Some(ref mut shared_info) => shared_info.clone_from(&info),
                Some(ref mut shared_info) => {
                    dirty |= shared_info.update_with_info(map_info);
                    dirty |= shared_info.update_with(map);
                },
                shared_info => {
                    *shared_info = Some(SharedMapPackLoaded::with_loaded(path, map_info.clone(), map));
                    dirty = true;
                },
            }
            dirty && notify
        });
        dirty
    }
    pub fn update_map_info(&self, path: PackMapPath, info: &Arc<MapPackInfo>, notify: bool) -> bool {
        let mut dirty = false;
        self.gameplay.send_if_modified(move |shared_map| {
            let Some(shared_map) = shared_map.get_mut(path.path) else { return false };
            let Some(Some(shared_info)) = shared_map.info.lookup_mut(&path.root) else {
                return false
            };
            dirty |= shared_info.update_with_info(info);
            dirty && notify
        });
        dirty
    }

    pub fn update_map_notify(&self, map_id: MapIndex) -> bool {
        #[cfg(todo)]
        {
            self.maps.send_if_modified(|_shared_maps| true);
        }
        self.update_gameplay_notify(map_id)
    }
    pub fn update_gameplay_notify(&self, map_id: MapIndex) -> bool {
        self.gameplay
            .send_if_modified(|shared_map| shared_map.map_id == Some(map_id))
    }
    pub fn update_map_id(&self, map_id: Option<MapIndex>, notify: bool) -> bool {
        let mut dirty = false;
        self.gameplay.send_if_modified(|shared_map| {
            dirty = shared_map.prepare_for_map(map_id);
            dirty && notify
        });
        dirty
    }

    pub fn clear_maps_for_packs<P>(&self, packs: P, notify: bool) -> bool
    where
        P: TaimiSet<PackPath>,
    {
        let keep = cmp::Reverse(&packs);
        let dirty = match () {
            #[cfg(todo)]
            () => {
                let mut dirty = false;
                self.maps.send_if_modified(|shared_maps| {
                    dirty = shared_maps.update_prune_maps_for(keep);
                    dirty & notify
                });
                dirty
            },
            _ => false,
        };
        let mut dirty_maps = false;
        self.gameplay.send_if_modified(|shared_map| {
            dirty_maps = shared_map.update_prune_for(keep);
            dirty_maps & notify
        });
        dirty | dirty_maps
    }
}
