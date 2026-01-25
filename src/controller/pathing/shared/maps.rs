use {
    super::PathingShared,
    crate::controller::pathing::{
        info::{LoadedMarkerInfo, LoadedPoiInfo, LoadedTrailInfo, MapPackInfo},
        registry::{
            LoadedCategoryIndex,
            LoadedCategoryNs,
            LoadedCategoryPath,
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
        space::DrawSpace,
        state::{
            hidden::MarkerState,
            LoadedCategory, LoadedMapPack, LoadedPoi, LoadedTrail,
        },
    },
    glamour::Point3,
    std::{cmp, ops, sync::Arc},
    taimi_hoard::{
        collections::TaimiSet,
        iters::all_zipped,
        loc::{indexed::IndexedList, LocationMut, LocationRef, Locator},
    },
    taimi_meta::packs::{
        id::{FromMarkerId1, MarkerId, MarkerIndex, MarkerIndexVariant, MarkerPath, PackMarkerNs},
        CategoryIndex,
        CategoryPath,
        MapIndex,
        PoiIndex,
        PoiPath,
        TrailIndex,
        TrailPath,
        VisibilityFlags,
    },
    taimi_pack::attributes::{
        keys::Guid,
        PoiAttributes,
        RenderAttributes,
        TrailAttributes,
    },
    taimi_sync::arcs::ArcPtrCmp,
};

#[cfg(todo)]
#[derive(Debug, Clone, Default)]
pub struct SharedMaps {
    pub map_info: BTreeMap<PackMapPath, SharedMapPackLoaded>,
}
#[cfg(todo)]
impl SharedMaps {
    pub const fn empty() -> Self {
        Self { map_info: BTreeMap::new() }
    }

    /// controller internal use
    pub(crate) fn update_prune_maps<C>(&mut self, keep: C) -> bool
    where
        C: TaimiSet<PackMapPath>,
    {
        let prev_len = self.map_info.len();
        self.map_info.retain(|path, _| keep.set_contains(path));
        self.map_info.len() != prev_len
    }
    /// controller internal use
    pub(crate) fn update_prune_maps_for<C>(&mut self, keep: C) -> bool
    where
        C: TaimiSet<PackPath>,
    {
        let prev_len = self.map_info.len();
        self.map_info.retain(|path, _| keep.set_contains(&path.root));
        self.map_info.len() != prev_len
    }

    /// remove outdated info from a local cache
    pub fn prune_map<P, T>(&self, maps: &mut BTreeMap<P, T>) -> bool
    where
        P: AsRef<PackMapPath> + Ord,
    {
        let prev_len = maps.len();
        maps.retain(|path, _| self.map_info.contains_key(path.as_ref()));
        maps.len() != prev_len
    }
    pub fn prune_map_of<P, T>(&self, maps: &mut BTreeMap<P, T>) -> bool
    where
        P: AsRef<PackPath> + Ord,
    {
        let prev_len = maps.len();
        maps.retain(|path, _| {
            let path = path.as_ref();
            self.map_info.keys().any(|p| p.root == path)
        });
        maps.len() != prev_len
    }
    /// remove outdated info from a local cache
    pub fn prune_set<P>(&self, maps: &mut BTreeSet<P>) -> bool
    where
        P: AsRef<PackMapPath> + Ord,
    {
        let prev_len = maps.len();
        maps.retain(|path| self.map_info.contains_key(path.as_ref()));
        maps.len() != prev_len
    }
}

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
        self.iter_state().flat_map(move |(map_path, map, map_info)| {
            map_info
                .loaded_pois()
                .map(move |(loaded, poi)| SharedMarkerRef {
                    loaded_id: MarkerId::for_marker(MarkerPath::with_parts(
                        map_path,
                        MarkerIndex::with_poi(loaded.path),
                    )),
                    index: MarkerIndex::with_poi(poi.path),
                    map_info,
                    map,
                })
                .chain(
                    map_info
                        .loaded_trails()
                        .map(move |(loaded, trail)| SharedMarkerRef {
                            loaded_id: MarkerId::for_marker(MarkerPath::with_parts(
                                map_path,
                                MarkerIndex::with_trail(loaded.path),
                            )),
                            index: MarkerIndex::with_trail(trail.path),
                            map_info,
                            map,
                        }),
                )
        })
    }

    pub fn ref_loaded_marker(&self, loaded_id: MarkerId) -> Option<SharedMarkerRef<'_>> {
        let pack = loaded_id.get_marker_pack_path();
        match (self.info.lookup_ref(&pack), self.state.lookup_ref(&pack)) {
            (Some(Some(map_info)), Some(Some(map))) => Some(SharedMarkerRef {
                loaded_id,
                index: MarkerIndex::UNK,
                map_info,
                map,
            }),
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
    #[cfg(todo)]
    pub interactive_pois: Arc<[InteractivePoi]>,
    pub poi_guids: Arc<[Guid]>,
}
impl SharedMapPackLoaded {
    #[cfg(todo)]
    pub fn with_info(path: PackMapPath, info: Arc<MapPackInfo>) -> Self {
        Self {
            path,
            #[cfg(todo)]
            interactive_pois: Default::default(),
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
            #[cfg(todo)]
            interactive_pois: map_pack.interactive_pois.clone(),
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
        #[cfg(todo)]
        {
            self.interactive_pois = map_pack.interactive_pois.clone();
        }
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
        match self.info.poi_guid_filter(self.info.loaded_pois()).enumerate().find(|(_, (p, _))| *p >= path) {
            Some((i, (p, _))) if p == path =>
                self.poi_guids.get(i),
            _ => None,
        }
    }
    pub fn poi_guid_by_path<'a>(&'a self, path: PoiPath) -> Option<&'a Guid> {
        match self.info.poi_guid_filter(self.info.loaded_pois()).enumerate().find(|(_, (_, p))| *p >= path) {
            Some((i, (_, p))) if p == path =>
                self.poi_guids.get(i),
            _ => None,
        }
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
    #[cfg(todo)]
    pub interactive_pois_nearby: Arc<BitVec>,
    #[cfg(todo)]
    pub interactive_poi_pois: Arc<[LoadedPoi]>,
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
            #[cfg(todo)]
            interactive_pois_nearby: Arc::new(map_pack.interactive_pois_nearby.clone()),
            #[cfg(todo)]
            interactive_poi_pois: Self::interactive_pois_from(map_pack),
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
            #[cfg(todo)]
            interactive_pois_nearby: Arc::new(map_pack.interactive_pois_nearby.clone()),
            #[cfg(todo)]
            interactive_poi_pois: Self::interactive_pois_from(map_pack),
            hidden_markers: Self::hidden_markers_from(path, state, map_pack),
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
        #[cfg(todo)]
        interaction_pois_etc();
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
        let dirty = trails_dirty | pois_dirty;

        #[cfg(todo)]
        {
            let nearby_dirty = self.interactive_pois_nearby[..] != map_pack.interactive_pois_nearby[..];
            if nearby_dirty {
                self.interactive_pois_nearby = Arc::new(map_pack.interactive_pois_nearby.clone());
            }
            // TODO: check if changed?
            let interactive_dirty = true;
            if interactive_dirty {
                self.interactive_poi_pois = Self::interactive_pois_from(map_pack);
            }
            let dirty = dirty | nearby_dirty | interactive_dirty;
        }
        dirty
    }
    pub fn update_with_hidden(
        &mut self,
        path: PackMapPath,
        state: &MarkerState,
        map_pack: &LoadedMapPack,
    ) -> bool {
        // TODO: check if changed?
        let hidden_dirty = true;
        if hidden_dirty {
            self.hidden_markers = Self::hidden_markers_from(path, state, map_pack);
        }
        hidden_dirty
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

    #[cfg(todo)]
    pub(crate) fn interactive_pois_from(map_pack: &LoadedMapPack) -> Arc<[LoadedPoi]> {
        map_pack
            .interactive_pois
            .iter()
            .map(|ipoi| {
                map_pack
                    .pois
                    .get(ipoi.loaded_index().path as usize)
                    .cloned()
                    .unwrap_or(LoadedPoi::INVALID)
            })
            .collect()
    }
    fn hidden_markers_from(
        map_path: PackMapPath,
        state: &MarkerState,
        map_pack: &LoadedMapPack,
    ) -> Arc<[MarkerId]> {
        let pack_path = map_path.root;
        state
            .hidden
            .keys()
            .filter(|id| match id {
                id if id
                    .marker_path::<PackPath>()
                    .map(|path| path.root == pack_path)
                    .unwrap_or(false) =>
                    true,
                id if id
                    .marker_path::<PackMapPath>()
                    .map(|path| path.root == map_path)
                    .unwrap_or(false) =>
                    true,
                _ => map_pack.poi_guids.contains(Guid::from_uuid_ref(id)),
            })
            .cloned()
            .collect()
    }

    pub fn loaded_pois<'a>(
        &'a self,
        info: &'a SharedMapPackLoaded,
    ) -> impl Iterator<Item = LoadedPoiRef<'a>> {
        let count = info.info.poi_count().min(self.pois.data.len());
        info.info.loaded_pois().take(count).map(move |(path, p)| unsafe {
            LoadedPoiRef::new_unchecked(SharedPoiRef::new_unchecked(SharedMarkerRef {
                loaded_id: MarkerId::for_marker(Locator::with_parts(
                    info.path,
                    path.pivot_to::<PackMarkerNs>().path,
                )),
                index: p.into(),
                map_info: info,
                map: self,
            }))
        })
    }
    pub fn loaded_trails<'a>(
        &'a self,
        info: &'a SharedMapPackLoaded,
    ) -> impl Iterator<Item = LoadedTrailRef<'a>> {
        let count = info.info.trail_count().min(self.trails.data.len());
        info.info
            .loaded_trails()
            .take(count)
            .map(move |(path, p)| unsafe {
                LoadedTrailRef::new_unchecked(SharedTrailRef::new_unchecked(SharedMarkerRef {
                    loaded_id: MarkerId::for_marker(Locator::with_parts(
                        info.path,
                        path.pivot_to::<PackMarkerNs>().path,
                    )),
                    index: p.into(),
                    map_info: info,
                    map: self,
                }))
            })
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
    pub loaded_id: MarkerId,
    pub index: MarkerIndex,
    pub map_info: &'a SharedMapPackLoaded,
    pub map: &'a SharedMapPackState,
}
impl<'a> SharedMarkerRef<'a> {
    pub fn pack_path(&self) -> PackPath {
        self.loaded_id.get_marker_pack_path()
    }
    pub fn map_path(&self) -> PackMapPath {
        self.loaded_id.get_marker_pack_map_path()
    }
    pub fn loaded_path(&self) -> MarkerPath<PackMapPath> {
        let root = self.map_path();
        let path = match self.loaded_id.marker_index() {
            #[cfg(debug_assertions)]
            _ if self.loaded_id.ns01().1 != <PackMapPath as FromMarkerId1>::NS1 => None,
            path => path,
        };
        let path = path.unwrap_or_else(|| {
            log::error!("SharedMarkerRef invalid id: {}", self.loaded_id);
            MarkerIndex::UNK
        });
        MarkerPath::with_parts(root, path)
    }
    /// as opposed to [self.loaded_path]
    pub fn path(&self) -> MarkerPath<PackPath> {
        match self.index {
            MarkerIndex::UNK => {
                let path = self.loaded_path();
                match self.map_info.info.path_from_loaded(self.loaded_path()) {
                    Some(p) => p,
                    None => {
                        log::error!("SharedMarkerRef invalid id: {}", self.loaded_id);
                        MarkerPath::with_parts(path.root.root, MarkerIndex::UNK)
                    },
                }
            },
            i => MarkerPath::with_parts(self.pack_path(), i),
        }
    }
    /// as opposed to [self.loaded_id]
    pub fn marker_id(&self) -> MarkerId {
        MarkerId::for_marker(self.path())
    }

    pub fn loaded_category_path(&self) -> Option<LoadedCategoryPath> {
        match self.loaded_path().path.variant() {
            MarkerIndexVariant::Category(i) => Some(LoadedCategoryPath::with_path(i)),
            _ => None,
        }
    }
    pub fn loaded_poi_path(&self) -> Option<LoadedPoiPath> {
        match self.loaded_path().path.variant() {
            MarkerIndexVariant::Poi(i) => Some(LoadedPoiPath::with_path(i)),
            _ => None,
        }
    }
    pub fn category_path(&self) -> Option<CategoryPath> {
        let lpath = self.loaded_category_path()?;
        self.map_info.info.category_path(lpath)
    }
    pub fn poi_path(&self) -> Option<PoiPath> {
        let lpath = self.loaded_poi_path()?;
        self.map_info.info.poi_path(lpath)
    }
    pub fn loaded_trail_path(&self) -> Option<LoadedTrailPath> {
        match self.loaded_path().path.variant() {
            MarkerIndexVariant::Trail(i) | MarkerIndexVariant::TrailSection(i, _) =>
                Some(LoadedTrailPath::with_path(i)),
            _ => None,
        }
    }
    pub fn trail_path(&self) -> Option<TrailPath> {
        let lpath = self.loaded_trail_path()?;
        self.map_info.info.trail_path(lpath)
    }
    /// TODO
    pub fn poi_info(&self) -> Option<&()> {
        self.poi_path().map(|_| &())
    }
    /// TODO
    pub fn trail_info(&self) -> Option<&()> {
        self.trail_path().map(|_| &())
    }
    pub fn loaded_category(&self) -> Option<&LoadedCategory> {
        let lpath = self.loaded_category_path()?;
        self.map.categories.get(lpath.path as usize)
    }
    pub fn loaded_poi_info(&self) -> Option<&LoadedPoiInfo> {
        let lpath = self.loaded_poi_path()?;
        self.map_info.pois().lookup_ref(&lpath)
    }
    pub fn loaded_trail_info(&self) -> Option<&LoadedTrailInfo> {
        let lpath = self.loaded_trail_path()?;
        self.map_info.trails().lookup_ref(&lpath)
    }
    pub fn loaded_poi(&self) -> Option<&LoadedPoiShared> {
        let lpath = self.loaded_poi_path()?;
        self.map.pois().lookup_ref(&lpath)
    }
    pub fn loaded_trail(&self) -> Option<&LoadedTrailShared> {
        let lpath = self.loaded_trail_path()?;
        self.map.trails().lookup_ref(&lpath)
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
pub enum LoadedMarkerRef<'a> {
    Poi(LoadedPoiRef<'a>),
    Trail(LoadedTrailRef<'a>),
    #[cfg(todo)]
    Category(),
}
impl<'a> LoadedMarkerRef<'a> {
    pub fn try_new(marker: SharedMarkerRef<'a>) -> Option<Self> {
        match marker.loaded_id.get_marker_index().namespace() {
            MarkerIndex::NS_POI => marker.to_loaded_poi().map(Self::Poi),
            MarkerIndex::NS_TRAIL => marker.to_loaded_trail().map(Self::Trail),
            #[cfg(todo)]
            MarkerIndex::NS_CAT => marker.to_loaded_trail().map(Self::Trail),
            _ => None,
        }
    }
    pub unsafe fn new_unchecked(marker: SharedMarkerRef<'a>) -> Option<Self> {
        match marker.loaded_id.get_marker_index().namespace() {
            MarkerIndex::NS_POI => Some(Self::Poi(LoadedPoiRef::new_unchecked(
                SharedPoiRef::new_unchecked(marker),
            ))),
            MarkerIndex::NS_TRAIL => Some(Self::Trail(LoadedTrailRef::new_unchecked(
                SharedTrailRef::new_unchecked(marker),
            ))),
            #[cfg(todo)]
            MarkerIndex::NS_CAT => (),
            _ => None,
        }
    }
}
#[derive(Clone)]
#[repr(transparent)]
pub struct SharedPoiRef<'a> {
    marker: SharedMarkerRef<'a>,
}
pub type SharedPoiInfo = CategoryIndex;
impl<'a> SharedPoiRef<'a> {
    pub fn try_new(marker: SharedMarkerRef<'a>) -> Option<Self> {
        let _ = marker.poi_info()?;

        Some(unsafe { Self::new_unchecked(marker) })
    }
    pub unsafe fn new_unchecked(marker: SharedMarkerRef<'a>) -> Self {
        Self { marker }
    }

    pub fn loaded_map_path(&self) -> PoiMapPath {
        self.loaded_path().pivot(self.marker.map_path())
    }
    pub fn loaded_path(&self) -> LoadedPoiPath {
        let index = self.marker.loaded_id.get_marker_index();
        LoadedPoiPath::with_path(index.index_poi_unchecked() as LoadedPoiIndex)
    }
    pub fn poi_path(&self) -> PoiPath {
        let loaded = self.loaded_path();
        match self.marker.map_info.poi_path(loaded) {
            #[cfg(todo = "unnecessary")]
            path => unsafe { path.unwrap_unchecked() },
            path => path.unwrap_or(PoiPath::with_path(PoiIndex::MAX)),
        }
    }
    #[cfg(todo)]
    pub fn poi_info(&self) -> &SharedPoiInfo {
        let path = self.loaded_path().path;
        unsafe { self.marker.map_info.pois.get_unchecked(path as usize) }
    }
    pub fn category_path(&self) -> CategoryPath {
        self.lpoi_info()
            .map(|info| info.category_path)
            .unwrap_or(CategoryPath::with_path(CategoryIndex::MAX))
    }
    pub fn lpoi_info(&self) -> Option<&LoadedPoiInfo> {
        let path = self.loaded_path().path;
        self.marker.map_info.pois.get(path as usize)
    }
    pub fn lpoi(&self) -> Option<&LoadedPoiShared> {
        let path = self.loaded_path().path;
        self.marker.map.pois.get(path as usize)
    }
    pub fn render_attrs(&self) -> Option<&Arc<RenderAttributes>> {
        self.lpoi_info().map(|info| info.attrs())
    }
    pub fn poi_attrs(&self) -> Option<&Box<PoiAttributes>> {
        self.render_attrs().and_then(|render| render.poi.as_ref())
    }
    #[inline]
    pub fn to_loaded(self) -> Option<LoadedPoiRef<'a>> {
        LoadedPoiRef::try_new(self)
    }
}
#[cfg(todo)]
impl ops::Deref for SharedPoiRef<'a> {}
#[derive(Clone)]
#[repr(transparent)]
pub struct LoadedPoiRef<'a> {
    marker: SharedPoiRef<'a>,
}
impl<'a> LoadedPoiRef<'a> {
    pub fn try_new(marker: SharedPoiRef<'a>) -> Option<Self> {
        #[cfg(todo = "unnecessary")]
        let _ = marker.lpoi_info()?;
        let _ = marker.poi_attrs()?;
        let _ = marker.lpoi()?;

        Some(unsafe { Self::new_unchecked(marker) })
    }
    pub unsafe fn new_unchecked(marker: SharedPoiRef<'a>) -> Self {
        Self { marker }
    }
    #[inline]
    pub fn lpoi_info(&self) -> &LoadedPoiInfo {
        let path = self.marker.loaded_path().path;
        unsafe { self.marker.marker.map_info.pois.get_unchecked(path as usize) }
    }
    #[inline]
    pub fn lpoi(&self) -> &LoadedPoiShared {
        let path = self.marker.loaded_path().path;
        unsafe { self.marker.marker.map.pois.get_unchecked(path as usize) }
    }
    #[inline]
    pub fn render_attrs(&self) -> &Arc<RenderAttributes> {
        self.lpoi_info().attrs()
    }
    #[inline]
    pub fn poi_attrs(&self) -> &Box<PoiAttributes> {
        unsafe { self.render_attrs().poi.as_ref().unwrap_unchecked() }
    }
}
impl<'a> ops::Deref for LoadedPoiRef<'a> {
    type Target = SharedPoiRef<'a>;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.marker
    }
}

#[derive(Clone)]
#[repr(transparent)]
pub struct SharedTrailRef<'a> {
    marker: SharedMarkerRef<'a>,
}
pub type SharedTrailInfo = CategoryIndex;
impl<'a> SharedTrailRef<'a> {
    pub fn try_new(marker: SharedMarkerRef<'a>) -> Option<Self> {
        let _ = marker.trail_info()?;

        Some(unsafe { Self::new_unchecked(marker) })
    }
    pub unsafe fn new_unchecked(marker: SharedMarkerRef<'a>) -> Self {
        Self { marker }
    }

    pub fn loaded_map_path(&self) -> TrailMapPath {
        self.loaded_path().pivot(self.marker.map_path())
    }
    pub fn loaded_path(&self) -> LoadedTrailPath {
        let index = self.marker.loaded_id.get_marker_index();
        LoadedTrailPath::with_path(index.trail_index_unchecked())
    }
    pub fn trail_path(&self) -> TrailPath {
        let loaded = self.loaded_path();
        match self.marker.map_info.trail_path(loaded) {
            #[cfg(todo = "unnecessary")]
            path => unsafe { path.unwrap_unchecked() },
            path => path.unwrap_or(TrailPath::with_path(TrailIndex::MAX)),
        }
    }
    #[cfg(todo)]
    pub fn trail_info(&self) -> &SharedTrailInfo {
        let path = self.loaded_path().path;
        unsafe { self.marker.map_info.trails.get_unchecked(path as usize) }
    }
    pub fn category_path(&self) -> CategoryPath {
        self.ltrail_info()
            .map(|info| info.category_path)
            .unwrap_or(CategoryPath::with_path(CategoryIndex::MAX))
    }
    pub fn ltrail_info(&self) -> Option<&LoadedTrailInfo> {
        let path = self.loaded_path().path;
        self.marker.map_info.trails.get(path as usize)
    }
    pub fn ltrail(&self) -> Option<&LoadedTrailShared> {
        let path = self.loaded_path().path;
        self.marker.map.trails.get(path as usize)
    }
    pub fn render_attrs(&self) -> Option<&Arc<RenderAttributes>> {
        self.ltrail_info().map(|info| info.attrs())
    }
    pub fn trail_attrs(&self) -> Option<&Box<TrailAttributes>> {
        self.render_attrs().and_then(|render| render.trail.as_ref())
    }
    #[inline]
    pub fn to_loaded(self) -> Option<LoadedTrailRef<'a>> {
        LoadedTrailRef::try_new(self)
    }
}
#[cfg(todo)]
impl ops::Deref for SharedTrailRef<'a> {}
#[derive(Clone)]
#[repr(transparent)]
pub struct LoadedTrailRef<'a> {
    marker: SharedTrailRef<'a>,
}
impl<'a> LoadedTrailRef<'a> {
    pub fn try_new(marker: SharedTrailRef<'a>) -> Option<Self> {
        #[cfg(todo = "unnecessary")]
        let _ = marker.ltrail_info()?;
        let _ = marker.trail_attrs()?;
        let _ = marker.ltrail()?;

        Some(unsafe { Self::new_unchecked(marker) })
    }
    pub unsafe fn new_unchecked(marker: SharedTrailRef<'a>) -> Self {
        Self { marker }
    }
    #[inline]
    pub fn ltrail_info(&self) -> &LoadedTrailInfo {
        let path = self.marker.loaded_path().path;
        unsafe { self.marker.marker.map_info.trails.get_unchecked(path as usize) }
    }
    #[inline]
    pub fn ltrail(&self) -> &LoadedTrailShared {
        let path = self.marker.loaded_path().path;
        unsafe { self.marker.marker.map.trails.get_unchecked(path as usize) }
    }
    #[inline]
    pub fn render_attrs(&self) -> &Arc<RenderAttributes> {
        self.ltrail_info().attrs()
    }
    #[inline]
    pub fn trail_attrs(&self) -> &Box<TrailAttributes> {
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
