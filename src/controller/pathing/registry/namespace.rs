use {
    crate::controller::pathing::registry::{PackRegistry, LoadedPack},
    taimi_hoard::loc::{
        locator_ns,
        indexed::IndexedList,
        Locator, LocationMut, LocationRef,
        NamespacePivotTo, NamespacePivotFrom,
    },
    taimi_meta::packs::{self, id::{MarkerIndex, MarkerPath, PackMarkerNs}, TrailSectionPath},
};
pub use taimi_meta::packs::{PackRegistryNs, PackPath, PackIndex, PackMapPath};

pub type PackListWith<T> = IndexedList<PackRegistryNs, PackIndex, T>;
pub type PackVecOf<T> = PackListWith<Vec<T>>;
pub type PackBoxOf<T> = PackListWith<Box<[T]>>;
pub type PackArcOf<T> = PackListWith<std::sync::Arc<[T]>>;

impl LocationRef<PackRegistryNs, PackIndex> for PackRegistry {
    type LookupRef = LoadedPack;

    fn lookup_ref<'a>(&'a self, loc: &Locator<PackRegistryNs, PackIndex>) -> Option<&'a Self::LookupRef> {
        self.packs.get(loc.path as usize)
    }
}
impl LocationMut<PackRegistryNs, PackIndex> for PackRegistry {
    fn lookup_mut<'a>(&'a mut self, loc: &Locator<PackRegistryNs, PackIndex>) -> Option<&'a mut Self::LookupRef> {
        self.packs.get_mut(loc.path as usize)
    }
}

locator_ns! {
    pub struct LoadedCategoryNs;
    impl LocatorNamespace {
        pub index LoadedCategoryIndex = packs::CategoryIndex;
        pub path LoadedCategoryPath;
        fn fmt(&self, f) {
            f.write_str("controller/cats")
        }
    }

    pub struct LoadedPoiNs;
    impl LocatorNamespace {
        pub index LoadedPoiIndex = packs::PoiIndex;
        pub path LoadedPoiPath;
        fn fmt(&self, f) {
            f.write_str("controller/pois")
        }
    }

    pub struct LoadedTrailNs;
    impl LocatorNamespace {
        pub index LoadedTrailIndex = packs::TrailIndex;
        pub path LoadedTrailPath;
        fn fmt(&self, f) {
            f.write_str("controller/trails")
        }
    }
    pub struct LoadedTrailSectionNs;
    impl LocatorNamespace {
        pub index LoadedTrailSectionIndex = Locator<LoadedTrailPath, TrailSectionPath>;
        pub path LoadedTrailSectionPath;
        fn fmt(&self, f) {
            f.write_str("section\\")
        }
    }
}
pub type CategoryMapPath<N = PackMapPath> = LoadedCategoryPath<N>;
pub type PoiMapPath<N = PackMapPath> = LoadedPoiPath<N>;
pub type TrailMapPath<N = PackMapPath> = LoadedTrailPath<N>;
pub type TrailSectionMapPath<N = PackMapPath> = Locator<N, LoadedTrailSectionIndex>;

impl LoadedCategoryNs {
    #[inline]
    pub fn to_marker_index(index: LoadedCategoryIndex) -> MarkerIndex {
        MarkerIndex::with_category(index as _)
    }
    #[inline]
    pub fn to_marker_path<N>(lpath: CategoryMapPath<N>) -> MarkerPath<N> {
        lpath.map_path(Self::to_marker_index)
    }
}
impl NamespacePivotFrom<PackMapPath, LoadedCategoryIndex> for LoadedCategoryNs {
    type NsPivotFromPath = LoadedCategoryIndex;
    #[inline]
    fn loc_pivot_from(path: CategoryMapPath) -> LoadedCategoryPath {
        path.unscope()
    }
}

impl LoadedPoiNs {
    #[inline]
    pub fn to_marker_index(index: LoadedPoiIndex) -> MarkerIndex {
        MarkerIndex::with_poi(index as _)
    }
    #[inline]
    pub fn to_marker_path<N>(lpath: PoiMapPath<N>) -> MarkerPath<N> {
        lpath.map_path(Self::to_marker_index)
    }
}
impl NamespacePivotFrom<PackMapPath, LoadedPoiIndex> for LoadedPoiNs {
    type NsPivotFromPath = LoadedPoiIndex;
    #[inline]
    fn loc_pivot_from(path: PoiMapPath) -> LoadedPoiPath {
        path.unscope()
    }
}

impl LoadedTrailNs {
    #[inline]
    pub fn to_marker_index(index: LoadedTrailIndex) -> MarkerIndex {
        MarkerIndex::with_trail(index as _)
    }
    #[inline]
    pub fn to_marker_path<N>(lpath: TrailMapPath<N>) -> MarkerPath<N> {
        lpath.map_path(Self::to_marker_index)
    }
}
impl NamespacePivotFrom<PackMapPath, LoadedTrailIndex> for LoadedTrailNs {
    type NsPivotFromPath = LoadedTrailIndex;
    #[inline]
    fn loc_pivot_from(path: TrailMapPath) -> LoadedTrailPath {
        path.unscope()
    }
}
impl LoadedTrailSectionNs {
    #[inline]
    pub fn to_marker_index(index: LoadedTrailSectionIndex) -> MarkerIndex {
        let Locator { root, path: section } = index;
        MarkerIndex::with_trail_section(root.path, section.path)
    }
    #[inline]
    pub fn to_marker_path<N>(lpath: TrailSectionMapPath<N>) -> MarkerPath<N> {
        lpath.map_path(Self::to_marker_index)
    }
}
impl NamespacePivotFrom<PackMapPath, LoadedTrailSectionIndex> for LoadedTrailSectionNs {
    type NsPivotFromPath = LoadedTrailSectionIndex;
    #[inline]
    fn loc_pivot_from(path: TrailSectionMapPath) -> LoadedTrailSectionPath {
        path.unscope()
    }
}

/// TODO: differentiate loaded from not x.x
pub type LoadedMarkerNs = PackMarkerNs;
pub type LoadedMarkerPath<N = LoadedMarkerNs> = MarkerPath<N>;
impl NamespacePivotTo<LoadedMarkerNs, LoadedCategoryIndex> for LoadedCategoryNs {
    type NsPivotToPath = MarkerIndex;
    #[inline]
    fn loc_pivot_to(path: LoadedCategoryPath) -> LoadedMarkerPath {
        LoadedMarkerPath::with_path(LoadedCategoryNs::to_marker_index(path.path))
    }
}
impl NamespacePivotTo<LoadedMarkerNs, LoadedPoiIndex> for LoadedPoiNs {
    type NsPivotToPath = MarkerIndex;
    #[inline]
    fn loc_pivot_to(path: LoadedPoiPath) -> LoadedMarkerPath {
        LoadedMarkerPath::with_path(LoadedPoiNs::to_marker_index(path.path))
    }
}
impl NamespacePivotTo<LoadedMarkerNs, LoadedTrailIndex> for LoadedTrailNs {
    type NsPivotToPath = MarkerIndex;
    #[inline]
    fn loc_pivot_to(path: LoadedTrailPath) -> LoadedMarkerPath {
        LoadedMarkerPath::with_path(LoadedTrailNs::to_marker_index(path.path))
    }
}
impl NamespacePivotTo<LoadedMarkerNs, LoadedTrailSectionIndex> for LoadedTrailSectionNs {
    type NsPivotToPath = MarkerIndex;
    #[inline]
    fn loc_pivot_to(path: LoadedTrailSectionPath) -> LoadedMarkerPath {
        LoadedMarkerPath::with_path(LoadedTrailSectionNs::to_marker_index(path.path))
    }
}
