use {
    crate::controller::pathing::registry::{PackRegistry, LoadedPack},
    taimi_hoard::loc::{
        indexed::IndexedList,
        Locator, LocationMut, LocationRef,
    },
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
