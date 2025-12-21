use std::collections::BTreeMap;
use taimi_hoard::loc::{LocationMut, LocationRef};
use taimi_hoard::collections::lru::RecentlyUsed;
use taimi_meta::packs::{MapIndex, PackMapPath, PackPath};
use visible::LoadedMapPack;

use {
    crate::controller::pathing::shared::MapPackInfo,
};
pub use self::visible::{VisibilityFlags, VisibilityFlagSet};
pub mod visible;

#[derive(Default)]
pub struct LoadedMapInfo {
    pub map_info: BTreeMap<PackMapPath, (RecentlyUsed, MapPackInfo)>,
}
impl LoadedMapInfo {
    pub fn write(&mut self, path: PackMapPath) -> &mut MapPackInfo {
        &mut self.map_info.entry(path).or_insert_with(|| (Default::default(), MapPackInfo::empty())).1
    }

    const USED_THRESHOLD: u32 = 6;
    pub fn cleanup(&mut self, map_id: Option<MapIndex>) {
        log::debug!("TODO: LoadedMapInfo::cleanup");
        for (path, (used, _map)) in self.map_info.iter_mut() {
            if map_id == Some(path.path) {
                used.mark_used();
            } else {
                used.mark_unused();
            }
        }
        self.map_info.retain(|_, (used, _)| !used.is_elderly(Self::USED_THRESHOLD));
    }
    pub fn clear(&mut self) {
        self.map_info.clear();
    }
}
impl LocationRef<PackPath, MapIndex> for LoadedMapInfo {
    type LookupRef = MapPackInfo;
    fn lookup_ref(&self, loc: &'_ PackMapPath) -> Option<&Self::LookupRef> {
        self.map_info.get(loc)
            .map(|(_, i)| i)
    }
}
impl LocationMut<PackPath, MapIndex> for LoadedMapInfo {
    fn lookup_mut(&mut self, loc: &'_ PackMapPath) -> Option<&mut Self::LookupRef> {
        self.map_info.get_mut(loc)
            .map(|(_, i)| i)
    }
}

#[derive(Default)]
pub struct LoadedMaps {
    pub maps: BTreeMap<PackMapPath, LoadedMapPack>,
}
impl LoadedMaps {
    pub fn write(&mut self, path: PackMapPath) -> &mut LoadedMapPack {
        self.maps.entry(path).or_insert_with(|| LoadedMapPack::empty(path.path))
    }

    /// TODO: 4 may be more reasonable
    const USED_THRESHOLD: u32 = 5;
    pub fn cleanup(&mut self, map_id: Option<MapIndex>) {
        log::debug!("TODO: LoadedMaps::cleanup");
        for map in self.maps.values_mut() {
            if map_id == Some(map.map_id) {
                map.used.mark_used();
            } else {
                map.used.mark_unused();
            }
        }
        self.maps.retain(|_, map| !map.used.is_elderly(Self::USED_THRESHOLD));
    }
    pub fn clear(&mut self) {
        self.maps.clear();
    }
}
impl LocationRef<PackPath, MapIndex> for LoadedMaps {
    type LookupRef = LoadedMapPack;
    fn lookup_ref(&self, loc: &'_ PackMapPath) -> Option<&Self::LookupRef> {
        self.maps.get(loc)
    }
}
impl LocationMut<PackPath, MapIndex> for LoadedMaps {
    fn lookup_mut(&mut self, loc: &'_ PackMapPath) -> Option<&mut Self::LookupRef> {
        self.maps.get_mut(loc)
    }
}
