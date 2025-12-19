use {
    super::PathingShared,
    crate::controller::pathing::{
        registry::{PackBoxOf, PackMapPath, PackPath, LoadedPoiPath, LoadedPoiIndex, LoadedTrailPath, LoadedTrailIndex},
        visible::{LoadedCategory, LoadedMapPack},
        shared::MapPackInfo,
    },
    std::{cmp, ops, sync::Arc},
    taimi_hoard::{
        collections::TaimiSet,
        loc::{
            Locator,
            LocationRef, LocationMut,
        },
    },
    taimi_meta::packs::{
        id::{IdVariant, MarkerIndexVariant, MarkerId, MarkerIndex, MarkerPath, FromMarkerId1},
        MapIndex, CategoryPath,
    },
};
#[cfg(todo)]
use {
    taimi_pack::attributes::keys::Guid,
    taimi_meta::packs::{
        PoiPath,
        PackIndex,
        PoiPath,
    },
    crate::controller::pathing::{
        state::hidden::MarkerState,
        interactive::{InteractivePoi, InteractionEvent},
    },
    std::collections::{btree_map, BTreeMap, BTreeSet},
};
type MarkerState = ();

#[cfg(todo)]
#[derive(Debug, Clone, Default)]
pub struct SharedMaps {
    pub map_info: BTreeMap<PackMapPath, SharedMapPackLoaded>,
}
#[cfg(todo)]
impl SharedMaps {
    pub const fn empty() -> Self {
        Self {
            map_info: BTreeMap::new(),
        }
    }

    /// controller internal use
    pub(crate) fn update_prune_maps<C>(&mut self, keep: C) -> bool where
        C: TaimiSet<PackMapPath>,
    {
        let prev_len = self.map_info.len();
        self.map_info.retain(|path, _| keep.set_contains(path));
        self.map_info.len() != prev_len
    }
    /// controller internal use
    pub(crate) fn update_prune_maps_for<C>(&mut self, keep: C) -> bool where
        C: TaimiSet<PackPath>,
    {
        let prev_len = self.map_info.len();
        self.map_info.retain(|path, _| keep.set_contains(&path.root));
        self.map_info.len() != prev_len
    }

    /// remove outdated info from a local cache
    pub fn prune_map<P, T>(&self, maps: &mut BTreeMap<P, T>) -> bool where
        P: AsRef<PackMapPath> + Ord,
    {
        let prev_len = maps.len();
        maps.retain(|path, _| self.map_info.contains_key(path.as_ref()));
        maps.len() != prev_len
    }
    pub fn prune_map_of<P, T>(&self, maps: &mut BTreeMap<P, T>) -> bool where
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
    pub fn prune_set<P>(&self, maps: &mut BTreeSet<P>) -> bool where
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
    pub fn prepare_for_map(&mut self, map_id: Option<MapIndex>) {
        if self.map_id != map_id {
            self.clear_for(map_id);
        }
    }

    pub fn last_pack_path(&self) -> Option<PackPath> {
        self.info.rposition(Option::is_some)
    }

    pub fn cloned(&self) -> Self {
        let map_id = self.map_id;
        let (info, state) = self.map_id.and_then(|_|
            self.last_pack_path()
        ).and_then(|last_path| {
            let range = 0..=last_path.path as usize;
            // TODO: get_unchecked?
            let info = self.info.data.get(range.clone())?;
            let state = self.state.data.get(range)?;
            Some((
                PackBoxOf::new(Box::from(info)),
                PackBoxOf::new(Box::from(state)),
            ))
        }).unwrap_or_default();
        Self { map_id, info, state }
    }

    pub(crate) fn update_prune_for<C>(&mut self, keep: C) -> bool where
        C: TaimiSet<PackPath>,
    {
        let mut dirty = false;
        for (path, info) in &mut self.info {
            if keep.set_contains(&path) { continue }
            dirty |= info.is_some();
            let _ = info.take();
        }
        for (path, state) in &mut self.state {
            if keep.set_contains(&path) { continue }
            dirty |= state.is_some();
            let _ = state.take();
        }
        dirty
    }

    pub fn for_pack_mut(&mut self, path: PackPath) -> (&mut Option<SharedMapPackLoaded>, &mut Option<SharedMapPackState>) {
        let info = self.info.lookup_extend_with(path.path, || None);
        let state = self.state.lookup_extend_with(path.path, || None);
        (info, state)
    }
    pub fn for_mut(&mut self, path: PackMapPath) -> (&mut Option<SharedMapPackLoaded>, &mut Option<SharedMapPackState>) {
        if self.map_id != Some(path.path) {
            self.clear_for(Some(path.path));
        }
        self.for_pack_mut(path.root)
    }
    fn consistency_check(path: PackMapPath, map_some: bool, info_some: bool) {
        if map_some && !info_some {
            log::debug!("INCONSISTENT map state for {path}: info({}) != state({})", info_some, map_some);
        }
    }
    pub fn iter_state_mut(&mut self) -> impl Iterator<Item = (PackMapPath, &mut SharedMapPackState, &mut SharedMapPackLoaded)> {
        let map_id = self.map_id;
        self.state.iter_mut().zip(self.info.values_mut())
            .filter_map(move |((path, map), info)| {
                let map_id = map_id?;
                let path = path.rel(map_id);
                Self::consistency_check(path, map.is_some(), info.is_some());
                let info = info.as_mut()?;
                map.as_mut()
                    .map(|map| (path, map, info))
            })
    }
    pub fn iter_state(&self) -> impl Iterator<Item = (PackMapPath, &SharedMapPackState, &SharedMapPackLoaded)> {
        self.iter_loaded().filter_map(|(path, info, state)| state.map(|state|
            (path, state, info)
        ))
    }
    pub fn iter_loaded(&self) -> impl Iterator<Item = (PackMapPath, &SharedMapPackLoaded, Option<&SharedMapPackState>)> {
        self.iter().filter_map(|(path, info, state)| info.map(|info|
            (path, info, state)
        ))
    }
    pub fn iter(&self) -> impl Iterator<Item = (PackMapPath, Option<&SharedMapPackLoaded>, Option<&SharedMapPackState>)> {
        let map_id = self.map_id;
        self.state.iter().zip(self.info.values())
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
            Some(map) if map != map_id =>
                None,
            Some(..) => Some(self),
        }
    }
    pub fn get_ref(&self, map_id: MapIndex) -> Option<&Self> {
        match self.map_id {
            Some(map) if map == map_id =>
                Some(self),
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
        self.info.lookup_ref(&path.root)
            .and_then(|info| info.as_ref())
            .map(|info| (path, info))
    }
    pub fn iter_markers(&self) -> impl Iterator<Item = SharedMarkerRef<'_>> {
        self.iter_state()
            .flat_map(move |(map_path, map, map_info)|
                map_info.loaded_pois().map(move |(loaded, poi)| SharedMarkerRef {
                    loaded_id: MarkerId::for_marker(MarkerPath::with_parts(map_path, MarkerIndex::with_poi(loaded.path))),
                    index: MarkerIndex::with_poi(poi.path),
                    map_info,
                    map,
                }).chain(map_info.loaded_trails().map(move |(loaded, trail)| SharedMarkerRef {
                    loaded_id: MarkerId::for_marker(MarkerPath::with_parts(map_path, MarkerIndex::with_trail(loaded.path))),
                    index: MarkerIndex::with_trail(trail.path),
                    map_info,
                    map,
                }))
            )
    }
}

#[cfg(todo)]
impl super::PathingSender {
    /// deleteme?
    #[cfg(todo)]
    pub fn map_info_with<R, F: FnOnce(PackMapPath, &SharedMapPackLoaded) -> R>(path: PackPath, f: F) -> Option<R> {
        Controller::with_sender(|s| {
            let map_id = s.gameplay.as_ref().and_then(|g| g.borrow().gameplay_map());
            map_id.and_then(|map_id| {
                let path = path.rel(map_id);
                s.pack_info.as_ref().and_then(|pack_info|
                    pack_info.borrow().map_info.get(&path)
                        .map(|info| f(path, info))
                )
            })
        }).flatten()
    }

    #[cfg(deleteme)]
    pub(crate) fn update_pack(&mut self, path: PackPath, pack: &LoadedPack) {
        if let Some(..) = pack.active {
            self.pack_loaded.insert(path.clone());
        } else {
            self.pack_loaded.remove(&path);
        }

        match self.pack_info.get(&path) {
            Some(shared_info) => {
                let shared_info = shared_info.as_ref().map(Arc::as_ptr)
                    .map_err(|r| &r.reason);
                let pack_info = pack.info.info.as_ref().map(Arc::as_ptr);
                if pack_info == shared_info {
                    return
                }
            },
            None => (),
        }

        let info = match pack.info.info.clone() {
            Ok(info) => Ok(info),
            Err(reason) => Err(UnloadedPack {
                path: pack.info.path.to_path_buf(),
                reason,
            }),
        };
        self.pack_info.insert(path, info);
    }
    #[cfg(deleteme)]
    #[deprecated]
    pub(crate) fn update_pack(&mut self, path: PackPath, pack: &LoadedPack) {
    }

    #[cfg(deleteme)]
    pub fn is_loaded(&self, path: &PackPath) -> bool {
        self.pack_loaded.contains(path)
    }
    #[cfg(deleteme)]
    #[deprecated]
    pub fn is_loaded(&self, path: &PackPath) -> bool {
        let Some(loader) = &self.shared_loader else { return false };
        SharedPacks::pack_at(&loader.shared.data.borrow(), *path)
            .map(|data| !weak_is_null(data))
            .unwrap_or(false)
    }

    #[cfg(todo = "unused")]
    pub fn unloaded_pack_info(&self) -> impl Iterator<Item = (PackPath, &UnloadedPack)> + '_ {
        self.pack_info.iter().filter_map(|(&path, info)|
            info.as_ref().err().map(|e| (path, e))
        )
    }
    #[cfg(deleteme)]
    pub fn pack_info(&self) -> impl Iterator<Item = (PackPath, &Arc<PackInfo>)> + '_ {
        self.pack_info.iter().filter_map(|(&path, info)|
            info.as_ref().ok().map(|e| (path, e))
        )
    }
    #[cfg(deleteme)]
    #[deprecated]
    pub fn pack_info(&self) -> impl Iterator<Item = (PackPath, Arc<PackInfo>)> + '_ {
        let mut info = self.shared_loader.as_ref()
            .map(|loader| loader.shared.info.borrow());
        let mut i = 0usize;
        iter::from_fn(move || {
            if info.as_ref().map(|info| info.len() > i).unwrap_or(true) {
                let _ = info.take();
                return None
            }
            let pack_info = info.as_ref().and_then(|info|
                info.get(i)
            ).and_then(|info|
                info.info.as_ref().ok()
            );
            i += 1;
            Some(pack_info.map(|info|
                (PackPath::with_path(i as PackIndex), info.clone())
            ))
        }).filter_map(|i| i)
    }
}

#[derive(Debug, Clone)]
pub struct SharedMapPackLoaded {
    pub info: Arc<MapPackInfo>,
    #[cfg(todo)]
    pub interactive_pois: Arc<[InteractivePoi]>,
    #[cfg(todo)]
    pub poi_guids: Arc<[Guid]>,
}
impl SharedMapPackLoaded {
    pub fn with_info(info: Arc<MapPackInfo>) -> Self {
        Self {
            #[cfg(todo)]
            interactive_pois: Default::default(),
            #[cfg(todo)]
            poi_guids: Default::default(),
            info,
        }
    }

    pub fn with_loaded(info: Arc<MapPackInfo>, map_pack: &LoadedMapPack) -> Self {
        Self {
            #[cfg(todo)]
            interactive_pois: map_pack.interactive_pois.clone(),
            #[cfg(todo)]
            poi_guids: map_pack.poi_guids.clone(),
            info,
        }
    }
    #[cfg(todo)]
    pub fn update_with(&mut self, map_pack: &LoadedMapPack) {
        self.interactive_pois = map_pack.interactive_pois.clone();
        self.poi_guids = map_pack.poi_guids.clone();
    }

    #[cfg(todo)]
    pub fn poi_guids<'a>(&'a self) -> impl Iterator<Item = (PoiPath, Option<&'a Guid>)> + 'a {
        let mut poi_guids = self.poi_guids.iter();
        self.info.pois()
            .zip(self.info.poi_guid_mask())
            .map(move |(path, mask)| {
                (
                    path,
                    match mask {
                        true => poi_guids.next(),
                        false => None,
                    },
                )
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
    #[cfg(todo)]
    pub interactive_pois_nearby: Arc<BitVec>,
    #[cfg(todo)]
    pub interactive_poi_pois: Arc<[LoadedPoi]>,
    #[cfg(todo)]
    pub hidden_markers: Arc<[MarkerId]>,
}

impl SharedMapPackState {
    pub fn with_static(_path: PackMapPath, map_pack: &LoadedMapPack) -> Self {
        Self {
            categories: map_pack.categories.clone(),
            #[cfg(todo)]
            interactive_pois_nearby: Arc::new(map_pack.interactive_pois_nearby.clone()),
            #[cfg(todo)]
            interactive_poi_pois: Self::interactive_pois_from(map_pack),
            #[cfg(todo)]
            hidden_markers: Default::default(),
        }
    }
    pub fn with_loaded(path: PackMapPath, map_pack: &LoadedMapPack, state: &MarkerState) -> Self {
        Self {
            categories: map_pack.categories.clone(),
            #[cfg(todo)]
            interactive_pois_nearby: Arc::new(map_pack.interactive_pois_nearby.clone()),
            #[cfg(todo)]
            interactive_poi_pois: Self::interactive_pois_from(map_pack),
            #[cfg(todo)]
            hidden_markers: Self::hidden_markers_from(path, state, map_pack),
        }
    }
    pub fn update_static(&mut self, map_pack: &LoadedMapPack) -> bool {
        if Arc::ptr_eq(&self.categories, &map_pack.categories) {
            return false
        }
        self.categories = map_pack.categories.clone();
        true
    }
    #[cfg(todo)]
    pub fn update_with_loaded(&mut self, map_pack: &LoadedMapPack) -> bool {
        let nearby_dirty = self.interactive_pois_nearby[..] != map_pack.interactive_pois_nearby[..];
        if nearby_dirty {
            self.interactive_pois_nearby = Arc::new(map_pack.interactive_pois_nearby.clone());
        }
        // TODO: check if changed?
        let interactive_dirty = true;
        if interactive_dirty {
            self.interactive_poi_pois = Self::interactive_pois_from(map_pack);
        }
        nearby_dirty | interactive_dirty
    }
    #[cfg(todo)]
    pub fn update_with_hidden(&mut self, path: PackMapPath, state: &MarkerState, map_pack: &LoadedMapPack) -> bool {
        // TODO: check if changed?
        let hidden_dirty = true;
        if hidden_dirty {
            self.hidden_markers = Self::hidden_markers_from(path, state, map_pack);
        }
        hidden_dirty
    }

    pub fn categories<'a, 'i>(&'a self, info: &'i MapPackInfo) -> impl Iterator<Item = (CategoryPath, &'a LoadedCategory)> + 'i where
        'a: 'i,
    {
        info.categories().zip(self.categories.iter())
    }

    #[cfg(todo)]
    pub(crate) fn interactive_pois_from(map_pack: &LoadedMapPack) -> Arc<[LoadedPoi]> {
        map_pack.interactive_pois.iter()
            .map(|ipoi| map_pack.pois.get(ipoi.loaded_index().path as usize)
                .cloned()
                .unwrap_or(LoadedPoi::INVALID)
            ).collect()
    }
    #[cfg(todo)]
    fn hidden_markers_from(map_path: PackMapPath, state: &MarkerState, map_pack: &LoadedMapPack) -> Arc<[MarkerId]> {
        let pack_path = map_path.root;
        state.hidden.keys()
            .filter(|id| match id {
                id if id.marker_path::<PackPath>().map(|path| path.root == pack_path).unwrap_or(false) =>
                    true,
                id if id.marker_path::<PackMapPath>().map(|path| path.root == map_path).unwrap_or(false) =>
                    true,
                _ => map_pack.poi_guids.contains(Guid::from_uuid_ref(id.as_ref())),
            })
            .cloned()
            .collect()
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
        let (i1, i2) = self.loaded_id.index12();
        FromMarkerId1::from_index12(i1, i2)
    }
    pub fn map_path(&self) -> PackMapPath {
        let (i1, i2) = self.loaded_id.index12();
        FromMarkerId1::from_index12(i1, i2)
    }
    pub fn loaded_path(&self) -> MarkerPath<PackMapPath> {
        match self.loaded_id.variant() {
            IdVariant::MarkerLoaded(path) => path,
            _ => {
                log::error!("SharedMarkerRef invalid id: {}", self.loaded_id);
                MarkerPath::with_parts(self.map_path(), MarkerIndex::UNK)
            },
        }
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
}

impl PathingShared {
    /// TODO: consider how/when this isn't dirty?
    pub fn update_map(&self, path: PackMapPath, map_info: &Arc<MapPackInfo>, map: &LoadedMapPack, notify: bool) -> bool {
        let info = SharedMapPackLoaded::with_loaded(map_info.clone(), map);
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
                    btree_map::Entry::Occupied(e) =>
                        e.into_mut().clone_from(&info),
                }
                dirty |= true;
                dirty && notify
            });
        }
        self.gameplay.send_if_modified(move |shared_map| {
            shared_map.prepare_for_map(Some(path.path));
            let (shared_info, shared) = shared_map.for_mut(path);
            if let Some(ref mut shared) = shared {
                shared.clone_from(&state);
            } else {
                shared.insert(state);
            }
            if let Some(ref mut shared_info) = shared_info {
                shared_info.clone_from(&info);
            } else {
                shared_info.insert(info);
            }
            dirty |= true;
            dirty && notify
        });
        dirty
    }

    pub fn update_map_notify(&self, map_id: MapIndex) -> bool {
        #[cfg(todo)] {
            self.maps.send_if_modified(|_shared_maps| true);
        }
        self.update_gameplay_notify(map_id)
    }
    pub fn update_gameplay_notify(&self, map_id: MapIndex) -> bool {
        self.gameplay.send_if_modified(|shared_map| {
            shared_map.map_id == Some(map_id)
        })
    }

    pub fn clear_maps_for_packs<P>(&self, packs: P, notify: bool) -> bool where
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
