use crate::{
    controller::pathing::registry::{PackRegistry, LoadedPack},
    exports::runtime::locator::{Locator, LocationGet, LocationMut, LocationRef},
};

pub use taimi_meta::loc::packs::*;

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
