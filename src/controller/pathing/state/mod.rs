use {
    crate::controller::pathing::{
        registry::{PackInfo, PackInfoSignature},
        shared::{MapPackInfo, SharedPackInfo, SharedPackLoad, SharedPackLoaded, EMPTY_RENDER_ATTRS},
        UnloadedReason,
    },
    std::{collections::BTreeMap, iter, ops, sync::Arc},
    taimi_hoard::{
        collections::{lru::RecentlyUsed, TaimiSet},
        iters::all_zipped,
        loc::{indexed::IndexedList, LocationMut, LocationRef},
    },
    taimi_meta::packs::{
        collections::PackSet,
        MapIndex,
        MapPath,
        PackIndex,
        PackMapPath,
        PackPath,
        PackRegistryNs,
        VisibilityFlags,
    },
    taimi_pack::attributes::RenderAttributes,
    taimi_sync::arcs::ArcPtrCmp,
};

#[doc(inline)]
pub use self::{
    map::LoadedMapPack,
    poi::LoadedPoi,
    trail::{LoadedTrail, LoadedTrailGeometry, LoadedTrailSection},
    visible::VisibilityFlagsExt,
};

mod map;
mod poi;
mod trail;
pub(crate) mod visible;

/// [MapPackInfo] plus some metadata
pub struct LoadedMapInfoStorage {
    pub used: RecentlyUsed,
    pub info: Arc<MapPackInfo>,
}
impl LoadedMapInfoStorage {
    #[inline]
    pub fn new(info: impl Into<Arc<MapPackInfo>>) -> Self {
        Self {
            used: Default::default(),
            info: info.into(),
        }
    }

    pub fn set_info(&mut self, info: impl Into<Arc<MapPackInfo>>) {
        self.used.mark_used();
        self.info = info.into();
    }
}
impl ops::Deref for LoadedMapInfoStorage {
    type Target = MapPackInfo;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.info
    }
}
impl Default for LoadedMapInfoStorage {
    fn default() -> Self {
        Self::new(MapPackInfo::empty())
    }
}
/// a collection of [LoadedMapInfoStorage]
#[derive(Default)]
pub struct LoadedMapInfo {
    pub map_info: BTreeMap<PackMapPath, LoadedMapInfoStorage>,
}
impl LoadedMapInfo {
    pub fn write(&mut self, path: PackMapPath) -> &mut LoadedMapInfoStorage {
        self.map_info.entry(path).or_default()
    }

    const USED_THRESHOLD: u32 = 6;
    /// TODO: BTreemap::extract_if (for [LoadedMaps] too)
    pub fn age_tick(&mut self, map_id: Option<MapIndex>) {
        for (path, map_info) in self.map_info.iter_mut() {
            map_info.used.mark_if(map_id == Some(path.path));
        }
    }
    /// remove outdated info from the cache
    ///
    /// TODO: BTreemap::extract_if (for [LoadedMaps] too)
    pub fn prune(&mut self, packs: Option<&LoadedPacks>) {
        self.map_info.retain(|path, map_info| {
            if map_info.used.is_elderly(Self::USED_THRESHOLD) {
                return false
            }
            if let Some(false) = packs.map(|p| p.set_contains(&(path.root, map_info.info_sig))) {
                return false
            }
            #[cfg(todo = "unnecessary")]
            if let Some(packs) = packs {
                let info_sig = packs.lookup_ref(&path.root).map(|info| info.sig.get());
                match info_sig {
                    Some(Some(sig)) if sig == map_info.info_sig => (),
                    None | Some(None) | Some(Some(..)) => return false,
                }
            }
            true
        });
    }
    /// optional exception for current map
    pub fn clear(&mut self, map_id: Option<MapIndex>) {
        match map_id {
            None => self.map_info.clear(),
            Some(map_id) => self.map_info.retain(|path, _| path.path == map_id),
        }
    }
}
impl LocationRef<PackPath, MapIndex> for LoadedMapInfo {
    type LookupRef = LoadedMapInfoStorage;
    #[inline]
    fn lookup_ref(&self, loc: &'_ PackMapPath) -> Option<&Self::LookupRef> {
        self.map_info.get(loc)
    }
}
impl LocationMut<PackPath, MapIndex> for LoadedMapInfo {
    #[inline]
    fn lookup_mut(&mut self, loc: &'_ PackMapPath) -> Option<&mut Self::LookupRef> {
        self.map_info.get_mut(loc)
    }
}
impl TaimiSet<PackPath> for LoadedMapInfo {
    fn set_contains(&self, path: &PackPath) -> bool {
        self.map_info.keys().any(|p| p.root == path)
    }
}
impl TaimiSet<MapPath> for LoadedMapInfo {
    fn set_contains(&self, path: &MapPath) -> bool {
        self.map_info.keys().any(|p| p.path == path.path)
    }
}
impl TaimiSet<PackMapPath> for LoadedMapInfo {
    #[inline]
    fn set_contains(&self, path: &PackMapPath) -> bool {
        self.map_info.contains_key(path)
    }
}
impl TaimiSet<(PackMapPath, PackInfoSignature)> for LoadedMapInfo {
    fn set_contains(&self, (path, sig): &(PackMapPath, PackInfoSignature)) -> bool {
        match self.map_info.get(path) {
            Some(map_info) if map_info.info_sig == *sig => true,
            _ => false,
        }
    }
}

/// a collection of [LoadedMapPack]
#[derive(Default)]
pub struct LoadedMaps {
    pub maps: BTreeMap<PackMapPath, LoadedMapPack>,
}
impl LoadedMaps {
    pub fn write(&mut self, path: PackMapPath) -> &mut LoadedMapPack {
        self.maps
            .entry(path)
            .or_insert_with(|| LoadedMapPack::empty(path.path))
    }
    pub fn write_with_info<'a, 'i>(
        &'a mut self,
        map_info: &'i mut LoadedMapInfo,
        path: PackMapPath,
    ) -> (&'a mut LoadedMapPack, &'i mut LoadedMapInfoStorage) {
        let map = self.write(path);
        let info = map_info.write(path);
        (map, info)
    }

    /// TODO: 4 may be more reasonable
    const USED_THRESHOLD: u32 = 5;
    pub fn age_tick(&mut self, map_id: Option<MapIndex>) {
        for (path, map) in self.maps.iter_mut() {
            map.used.mark_if(map_id == Some(path.path));
        }
    }
    /// remove outdated info from the cache
    pub fn prune(&mut self, map_info: Option<&LoadedMapInfo>) {
        self.maps.retain(|path, map| {
            if map.used.is_elderly(Self::USED_THRESHOLD) {
                return false
            }
            if let Some(false) = map_info.map(|i| i.set_contains(&(*path, map.info_sig))) {
                return false
            }
            #[cfg(todo = "unnecessary")]
            if let Some(map_info) = map_info {
                let info_sig = map_info.lookup_ref(path).map(|info| info.info_sig.get());
                match info_sig {
                    Some(Some(sig)) if sig == map.info_sig => (),
                    None | Some(None) | Some(Some(..)) => return false,
                }
            }
            true
        });
    }
    pub fn clear(&mut self) {
        self.maps.clear();
    }

    pub fn lookup_with_info<'a, 'i>(
        &'a self,
        map_info: &'i LoadedMapInfo,
        path: &'_ PackMapPath,
    ) -> Option<(&'a LoadedMapPack, &'i Arc<MapPackInfo>)> {
        let map = self.lookup_ref(path)?;
        map_info
            .lookup_ref(path)
            .map(move |map_info| (map, &map_info.info))
    }
    pub fn lookup_mut_with_info<'a, 'i>(
        &'a mut self,
        map_info: &'i LoadedMapInfo,
        path: &'_ PackMapPath,
    ) -> Option<(&'a mut LoadedMapPack, &'i Arc<MapPackInfo>)> {
        let map = self.lookup_mut(path)?;
        map_info
            .lookup_ref(path)
            .map(move |map_info| (map, &map_info.info))
    }
    pub fn lookup_mut_with_info_mut<'a, 'i>(
        &'a mut self,
        map_info: &'i mut LoadedMapInfo,
        path: &'_ PackMapPath,
    ) -> Option<(&'a mut LoadedMapPack, &'i mut LoadedMapInfoStorage)> {
        let map = self.lookup_mut(path)?;
        map_info
            .lookup_mut(path)
            .map(move |map_info| (map, map_info))
    }
    pub fn iter_pack<'a>(
        &'a self,
        pack_path: PackPath,
    ) -> impl Iterator<Item = (PackMapPath, &'a LoadedMapPack)> {
        self.maps.iter().filter_map(move |(path, map)| match pack_path {
            p if path.root != p => None,
            _ => Some((*path, map)),
        })
    }
    pub fn iter_pack_mut<'a>(
        &'a mut self,
        pack_path: PackPath,
    ) -> impl Iterator<Item = (PackMapPath, &'a mut LoadedMapPack)> {
        self.maps
            .iter_mut()
            .filter_map(move |(path, map)| match pack_path {
                p if path.root != p => None,
                _ => Some((*path, map)),
            })
    }
    pub fn iter_pack_with_info<'a, 'i>(
        &'a self,
        map_info: &'i LoadedMapInfo,
        pack_path: PackPath,
    ) -> impl Iterator<Item = (PackMapPath, &'a LoadedMapPack, &'i Arc<MapPackInfo>)> {
        self.iter_pack(pack_path).filter_map(|(path, map)| {
            map_info
                .lookup_ref(&path)
                .map(move |map_info| (path, map, &map_info.info))
        })
    }
    pub fn iter_pack_mut_with_info<'a, 'i>(
        &'a mut self,
        map_info: &'i LoadedMapInfo,
        pack_path: PackPath,
    ) -> impl Iterator<Item = (PackMapPath, &'a mut LoadedMapPack, &'i Arc<MapPackInfo>)> {
        self.iter_pack_mut(pack_path).filter_map(|(path, map)| {
            map_info
                .lookup_ref(&path)
                .map(move |map_info| (path, map, &map_info.info))
        })
    }

    pub fn iter<'a>(
        &'a self,
        map_id: Option<MapIndex>,
    ) -> impl Iterator<Item = (PackMapPath, &'a LoadedMapPack)> {
        self.maps.iter().filter_map(move |(path, map)| match map_id {
            Some(map_id) if map_id != path.path => None,
            _ => Some((*path, map)),
        })
    }
    pub fn iter_mut<'a>(
        &'a mut self,
        map_id: Option<MapIndex>,
    ) -> impl Iterator<Item = (PackMapPath, &'a mut LoadedMapPack)> {
        self.maps.iter_mut().filter_map(move |(path, map)| match map_id {
            Some(map_id) if map_id != path.path => None,
            _ => Some((*path, map)),
        })
    }
    pub fn iter_with_info<'a, 'i>(
        &'a self,
        map_info: &'i LoadedMapInfo,
        map_id: Option<MapIndex>,
    ) -> impl Iterator<Item = (PackMapPath, &'a LoadedMapPack, &'i Arc<MapPackInfo>)> {
        self.iter(map_id).filter_map(|(path, map)| {
            map_info
                .lookup_ref(&path)
                .map(move |map_info| (path, map, &map_info.info))
        })
    }
    pub fn iter_mut_with_info<'a, 'i>(
        &'a mut self,
        map_info: &'i LoadedMapInfo,
        map_id: Option<MapIndex>,
    ) -> impl Iterator<Item = (PackMapPath, &'a mut LoadedMapPack, &'i Arc<MapPackInfo>)> {
        self.iter_mut(map_id).filter_map(|(path, map)| {
            map_info
                .lookup_ref(&path)
                .map(move |map_info| (path, map, &map_info.info))
        })
    }
}
impl LocationRef<PackPath, MapIndex> for LoadedMaps {
    type LookupRef = LoadedMapPack;
    #[inline]
    fn lookup_ref(&self, loc: &'_ PackMapPath) -> Option<&Self::LookupRef> {
        self.maps.get(loc)
    }
}
impl LocationMut<PackPath, MapIndex> for LoadedMaps {
    #[inline]
    fn lookup_mut(&mut self, loc: &'_ PackMapPath) -> Option<&mut Self::LookupRef> {
        self.maps.get_mut(loc)
    }
}
impl TaimiSet<PackPath> for LoadedMaps {
    fn set_contains(&self, path: &PackPath) -> bool {
        self.maps.keys().any(|p| p.root == path)
    }
}
impl TaimiSet<PackMapPath> for LoadedMaps {
    #[inline]
    fn set_contains(&self, path: &PackMapPath) -> bool {
        self.maps.contains_key(path)
    }
}

/// [SharedPackInfo] plus some state tracking metadata
#[derive(Debug, Clone)]
pub struct LoadedPackInfo {
    pub used: RecentlyUsed,
    pub sig: PackInfoSignature,
    pub info: Arc<SharedPackInfo>,
    pub unloaded: Option<UnloadedReason>,
}
impl LoadedPackInfo {
    pub fn empty() -> Self {
        Self {
            used: RecentlyUsed::DEFAULT,
            sig: PackInfoSignature::EMPTY,
            info: Arc::new(SharedPackInfo::empty(None)),
            unloaded: Some(UnloadedReason::Gravestone),
        }
    }

    pub fn is_loaded(&self) -> bool {
        self.unloaded.is_none()
    }
    pub fn can_reload(&self) -> bool {
        self.unloaded.as_ref().map(|r| r.can_reload()).unwrap_or(false)
    }

    pub fn update_with(&mut self, info: &SharedPackLoad) -> bool {
        let mut dirty = self.update_with_info(&info.info);
        dirty |= self.update_with_loaded(&info.loaded.borrow());
        dirty
    }
    pub fn update_with_info(&mut self, info: &Arc<SharedPackInfo>) -> bool {
        let dirty = ArcPtrCmp::from_mut(&mut self.info).clone_from_arc(info);
        if dirty {
            self.sig = info.sig;
        }
        dirty
    }
    pub fn update_with_loaded(&mut self, loaded: &SharedPackLoaded) -> bool {
        let mut dirty = false;
        if self.unloaded != loaded.unloaded {
            self.unloaded = loaded.unloaded.clone();
            dirty = true;
        }
        dirty
    }
}
impl Default for LoadedPackInfo {
    fn default() -> Self {
        Self::empty()
    }
}
impl TaimiSet<MapPath> for LoadedPackInfo {
    #[inline]
    fn set_contains(&self, path: &MapPath) -> bool {
        self.info.has_map(path.path)
    }
}
/// a collection of [LoadedPackInfo]
#[derive(Default)]
pub struct LoadedPacks {
    pub packs: IndexedList<PackRegistryNs, PackIndex, Vec<LoadedPackInfo>>,
}
impl LoadedPacks {
    pub fn write(&mut self, path: PackPath) -> &mut LoadedPackInfo {
        self.packs.lookup_extend_with(path.path, LoadedPackInfo::default)
    }

    pub fn lookup_info(&self, path: PackPath) -> Option<(&Arc<PackInfo>, &LoadedPackInfo)> {
        self.lookup_ref(&path)
            .and_then(|info| info.info.info.as_ref().map(|i| (i, info)))
    }

    pub fn need_load(&self) -> impl Iterator<Item = (PackPath, &LoadedPackInfo)> {
        self.packs
            .iter()
            .filter(|(_p, pack)| matches!(pack.unloaded, Some(UnloadedReason::Pending)))
    }
    pub fn on_map(&self, map_id: MapIndex) -> impl Iterator<Item = (PackPath, &LoadedPackInfo)> {
        let map_path: MapPath = MapPath::with_path(map_id);
        self.packs
            .iter()
            .filter(move |(_p, pack)| pack.set_contains(&map_path))
    }

    const USED_THRESHOLD: u32 = 2;
    pub fn age_tick(&mut self, map_info: Option<&LoadedMapInfo>) {
        for (path, pack) in self.packs.iter_mut() {
            let used = match map_info {
                Some(map_info) if map_info.set_contains(&path) => true,
                _ => false,
            };
            pack.used.mark_if(used);
        }
    }
    #[inline]
    fn mark_at<F: FnOnce(&mut RecentlyUsed)>(&mut self, path: PackPath, f: F) {
        if let Some(pack) = self.lookup_mut(&path) {
            f(&mut pack.used)
        }
    }
    #[inline]
    pub fn mark_used(&mut self, path: PackPath) {
        self.mark_at(path, |used| used.mark_used())
    }
    #[inline]
    pub fn mark_for_death(&mut self, path: PackPath) {
        self.mark_at(path, |used| used.mark_for_death())
    }
    #[inline]
    pub fn len(&self) -> usize {
        self.packs.len()
    }
    #[inline]
    pub fn sigs_match<S>(&self, sigs: S) -> bool
    where
        S: IntoIterator<Item = PackInfoSignature>,
    {
        self.sigs_match_dyn(&mut sigs.into_iter())
    }
    pub fn sigs_match_dyn(&self, sigs: &mut dyn Iterator<Item = PackInfoSignature>) -> bool {
        all_zipped(|l, r| l == r.info.sig, sigs, self.packs.values())
    }
    #[inline]
    pub fn sigs_dirty<S>(&self, sigs: S) -> PackSet
    where
        S: IntoIterator<Item = PackInfoSignature>,
    {
        self.sigs_dirty_dyn(&mut sigs.into_iter()).collect()
    }
    pub fn sigs_dirty_dyn<'a, 's>(
        &'a self,
        sigs: &'s mut dyn Iterator<Item = PackInfoSignature>,
    ) -> impl Iterator<Item = PackPath> + 'a + 's
    where
        's: 'a,
        'a: 's,
    {
        let mut packs = self.packs.iter();
        let mut sigs = sigs.fuse();
        iter::from_fn(move || {
            while let Some(sig) = sigs.next() {
                match packs.next() {
                    Some((_, pack)) if pack.info.sig == sig => (),
                    Some((path, _)) => return Some(path),
                    None => {
                        // strange...
                        return None
                    },
                }
            }
            packs.next().map(|(path, _)| path)
        })
    }
    pub fn clear(&mut self) {
        self.packs.clear();
    }
}
impl LocationRef<PackRegistryNs, PackIndex> for LoadedPacks {
    type LookupRef = LoadedPackInfo;
    #[inline]
    fn lookup_ref(&self, loc: &'_ PackPath) -> Option<&Self::LookupRef> {
        self.packs.lookup_ref(loc)
    }
}
impl LocationMut<PackRegistryNs, PackIndex> for LoadedPacks {
    #[inline]
    fn lookup_mut(&mut self, loc: &'_ PackPath) -> Option<&mut Self::LookupRef> {
        self.packs.lookup_mut(loc)
    }
}
impl TaimiSet<(PackPath, PackInfoSignature)> for LoadedPacks {
    fn set_contains(&self, (path, sig): &(PackPath, PackInfoSignature)) -> bool {
        match self.packs.lookup_ref(path) {
            Some(pack) if pack.sig == *sig => true,
            _ => false,
        }
    }
}
impl TaimiSet<PackMapPath> for LoadedPacks {
    fn set_contains(&self, path: &PackMapPath) -> bool {
        let map_path: MapPath = path.unscope();
        self.lookup_ref(&path.root)
            .map(|p| p.set_contains(&map_path))
            .unwrap_or(false)
    }
}
impl TaimiSet<MapPath> for LoadedPacks {
    fn set_contains(&self, path: &MapPath) -> bool {
        self.on_map(path.path).next().is_some()
    }
}

/// a component of a [LoadedMapPack]
#[derive(Debug, Clone, Default)]
pub struct LoadedCategory {
    pub visibility: VisibilityFlags,
}

impl LoadedCategory {
    pub const INVALID: Self = Self { visibility: VisibilityFlags::empty() };
}

fn get_overrides_mut<'a>(
    overrides: &'a mut Option<Box<RenderAttributes>>,
) -> &'a mut Box<RenderAttributes> {
    overrides.get_or_insert_with(|| Box::new((**EMPTY_RENDER_ATTRS).clone()))
}
