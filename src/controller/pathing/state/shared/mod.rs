use std::{fmt, iter, ops};
use std::path::Path;
use std::{collections::{BTreeMap, BTreeSet}, path::PathBuf, sync::{Arc, Weak}};
use crate::controller::pathing::{
    state::hidden::MarkerState,
    registry::{MarkerId, PoiPath},
};
use crate::exports::runtime as rt;
use crate::controller::pathing::visible::{InteractivePoi, LoadedPoi};
use crate::controller::{pathing::{registry::{CategoryPath, LoadedPack, PackInfo, PackLoader, PackMapPath, PackPath, UnloadedReason}, visible::{InteractionEvent, LoadedCategory, LoadedMapPack}, MapPackInfo}, Controller};
use bitvec::vec::BitVec;
use taimi_meta::loc::packs::PackIndex;
use taimi_sync::arcs::weak_is_null;
use tokio::sync::broadcast;
use taimi_pack::attributes::keys::Guid;
pub use self::loader::{
    SharedPacks, SharedLoaderPackData, SharedLoaderPackInfo, SharedLoaderPackConfig,
};

mod loader;

/// TODO: rename to SharedPacks or something?
#[derive(Debug, Clone)]
pub struct SharedMapPackInfo {
    pub shared_loader: Option<Arc<PackLoader>>,
    pub interactions: broadcast::Sender<InteractionEvent>,
    #[cfg(deleteme)]
    pub pack_info: BTreeMap<PackPath, Result<Arc<PackInfo>, UnloadedPack>>,
    #[cfg(deleteme)]
    pub pack_loaded: BTreeSet<PackPath>,
    pub map_info: BTreeMap<PackMapPath, SharedMapPackLoaded>,
    pub map_state: BTreeMap<PackMapPath, SharedMapPackState>,
}

impl SharedMapPackInfo {
    pub const INTERACTIONS_BUFFER_LEN: usize = 48;

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
    #[deprecated]
    pub(crate) fn update_pack(&mut self, path: PackPath, pack: &LoadedPack) {
    }

    #[cfg(deleteme)]
    pub fn is_loaded(&self, path: &PackPath) -> bool {
        self.pack_loaded.contains(path)
    }
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

impl Default for SharedMapPackInfo {
    fn default() -> Self {
        Self {
            shared_loader: Default::default(),
            interactions: broadcast::Sender::new(Self::INTERACTIONS_BUFFER_LEN),
            #[cfg(deleteme)]
            pack_info: Default::default(),
            #[cfg(deleteme)]
            pack_loaded: Default::default(),
            map_info: Default::default(),
            map_state: Default::default(),
        }
    }
}

#[cfg(deleteme)]
#[derive(Debug, Clone)]
pub struct UnloadedPack {
    pub path: PathBuf,
    pub reason: UnloadedReason,
}

#[cfg(deleteme)]
impl fmt::Display for UnloadedPack {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let name = self.path.file_name()
            .map(Path::new)
            .unwrap_or_else(|| rt::relative_path(&self.path));
        write!(f, "{}", name.display())
    }
}

#[derive(Debug, Clone)]
pub struct SharedMapPackLoaded {
    pub info: Arc<MapPackInfo>,
    pub interactive_pois: Arc<[InteractivePoi]>,
    pub poi_guids: Arc<[Guid]>,
}
impl SharedMapPackLoaded {
    pub fn with_info(info: Arc<MapPackInfo>) -> Self {
        Self {
            interactive_pois: Default::default(),
            poi_guids: Default::default(),
            info,
        }
    }

    pub fn with_loaded(info: Arc<MapPackInfo>, map_pack: &LoadedMapPack) -> Self {
        Self {
            interactive_pois: map_pack.interactive_pois.clone(),
            poi_guids: map_pack.poi_guids.clone(),
            info,
        }
    }
    pub fn update_with(&mut self, map_pack: &LoadedMapPack) {
        self.interactive_pois = map_pack.interactive_pois.clone();
        self.poi_guids = map_pack.poi_guids.clone();
    }

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
    pub interactive_pois_nearby: Arc<BitVec>,
    pub interactive_poi_pois: Arc<[LoadedPoi]>,
    pub hidden_markers: Arc<[MarkerId]>,
}

impl SharedMapPackState {
    pub fn with_static(path: PackMapPath, map_pack: &LoadedMapPack) -> Self {
        Self {
            categories: map_pack.categories.clone(),
            interactive_pois_nearby: Arc::new(map_pack.interactive_pois_nearby.clone()),
            interactive_poi_pois: Self::interactive_pois_from(map_pack),
            hidden_markers: Default::default(),
        }
    }
    pub fn with_loaded(path: PackMapPath, map_pack: &LoadedMapPack, state: &MarkerState) -> Self {
        Self {
            categories: map_pack.categories.clone(),
            interactive_pois_nearby: Arc::new(map_pack.interactive_pois_nearby.clone()),
            interactive_poi_pois: Self::interactive_pois_from(map_pack),
            hidden_markers: Self::hidden_markers_from(path, state, map_pack),
        }
    }
    pub fn update_static(&mut self, map_pack: &LoadedMapPack) {
        self.categories = map_pack.categories.clone();
    }
    pub fn update_with_loaded(&mut self, map_pack: &LoadedMapPack) {
        self.interactive_pois_nearby = Arc::new(map_pack.interactive_pois_nearby.clone());
        self.interactive_poi_pois = Self::interactive_pois_from(map_pack);
    }
    pub fn update_with_hidden(&mut self, path: PackMapPath, state: &MarkerState, map_pack: &LoadedMapPack) -> bool {
        self.hidden_markers = Self::hidden_markers_from(path, state, map_pack);
        // TODO: check if changed?
        true
    }

    pub fn categories<'a, 'i>(&'a self, info: &'i MapPackInfo) -> impl Iterator<Item = (CategoryPath, &'a LoadedCategory)> + 'i where
        'a: 'i,
    {
        info.categories().zip(self.categories.iter())
    }

    pub(crate) fn interactive_pois_from(map_pack: &LoadedMapPack) -> Arc<[LoadedPoi]> {
        map_pack.interactive_pois.iter()
            .map(|ipoi| map_pack.pois.get(ipoi.loaded_index().path as usize)
                .cloned()
                .unwrap_or(LoadedPoi::INVALID)
            ).collect()
    }
    fn hidden_markers_from(map_path: PackMapPath, state: &MarkerState, map_pack: &LoadedMapPack) -> Arc<[MarkerId]> {
        let pack_path = map_path.root;
        state.hidden.keys()
            .filter(|id| match id {
                id if id.marker_path::<PackPath>().map(|path| path.root == pack_path).unwrap_or(false) =>
                    true,
                id if id.marker_path::<PackMapPath>().map(|path| path.root == map_path).unwrap_or(false) =>
                    true,
                _ => map_pack.poi_guids.contains(id.as_ref()),
            })
            .cloned()
            .collect()
    }
}
