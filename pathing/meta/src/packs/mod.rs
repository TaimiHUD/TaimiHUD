use {
    taimi_hoard::loc::locator_ns,
    crate::map::MapID,
    core::num::NonZero,
};
pub use self::id::{MarkerId, MarkerPath, MarkerIndex, MarkerIndexVariant, MarkerIndexNamespace, IdVariant};

pub mod collections;
pub mod id;

locator_ns! {
    pub struct PackCategoryNs;
    impl LocatorNamespace {
        pub index CategoryIndex = u32;
        pub path CategoryPath;
        fn fmt(&self, f) {
            f.write_str("pack/category")
        }
    }

    pub struct PackPoiNs;
    impl LocatorNamespace {
        pub index PoiIndex = u32;
        pub path PoiPath;
        fn fmt(&self, f) {
            f.write_str("pack/poi")
        }
    }

    pub struct PackTrailNs;
    impl LocatorNamespace {
        pub index TrailIndex = u16;
        pub path TrailPath;
        fn fmt(&self, f) {
            f.write_str("pack/trail")
        }
    }

    pub struct PackTrailSectionNs;
    impl LocatorNamespace {
        pub index TrailSectionIndex = u16;
        pub path TrailSectionPath;
        fn fmt(&self, f) {
            f.write_str("trail/section")
        }
    }

    pub struct MapNs;
    impl LocatorNamespace {
        pub index MapIndex = NonZero<MapID>;
        pub path MapPath;
        fn fmt(&self, f) {
            f.write_str("map")
        }
    }
}

locator_ns! {
    /// TODO: move this out of crate?
    /// [MarkerId] needs to stop using it somehow
    pub struct PackRegistryNs;
    impl LocatorNamespace {
        pub index PackIndex = u16;
        pub path PackPath;
        fn fmt(&self, f) {
            f.write_str("controller/packs")
        }
    }
}
/// TODO: move this out of crate?
pub type PackMapPath<N = PackPath> = MapPath<N>;
