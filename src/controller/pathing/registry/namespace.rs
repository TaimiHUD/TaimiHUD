use crate::{
    controller::pathing::registry::{PackRegistry, LoadedPack},
    exports::runtime::locator::{Locator, LocationGet, LocationMut, LocationRef},
};
use taimi_meta::map::MapID;
use std::num::NonZero;
use std::fmt;

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackRegistryNs;
pub type PackIndex = u16;
pub type PackPath<N = PackRegistryNs> = Locator<N, PackIndex>;
impl fmt::Display for PackRegistryNs {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("controller/packs")
    }
}
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
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackCategoryNs;
pub type CategoryIndex = u32;
pub type CategoryPath<N = PackCategoryNs> = Locator<N, CategoryIndex>;
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackPoiNs;
pub type PoiIndex = u32;
pub type PoiPath<N = PackPoiNs> = Locator<N, PoiIndex>;
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackTrailNs;
pub type TrailIndex = u16;
pub type TrailPath<N = PackTrailNs> = Locator<N, TrailIndex>;
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackTrailSectionNs;
pub type TrailSectionIndex = u16;
pub type TrailSectionPath<N = PackTrailSectionNs> = Locator<N, TrailSectionIndex>;
pub type MapIndex = NonZero<MapID>;
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MapNs;
pub type MapPath<N = MapNs> = Locator<N, MapIndex>;
pub type PackMapPath<N = PackPath> = MapPath<N>;
impl fmt::Display for PackCategoryNs {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("pack/category")
    }
}
impl fmt::Display for PackPoiNs {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("pack/poi")
    }
}
impl fmt::Display for PackTrailNs {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("pack/trail")
    }
}
impl fmt::Display for MapNs {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("map")
    }
}
