use std::fmt;
use std::path::Path;
use std::{collections::{BTreeMap, BTreeSet}, path::PathBuf, sync::Arc};
use crate::exports::runtime as rt;
use crate::controller::pathing::visible::InteractivePoi;
use crate::controller::{pathing::{registry::{CategoryPath, LoadedPack, PackInfo, PackLoader, PackMapPath, PackPath, UnloadedReason}, visible::{InteractionEvent, LoadedCategory, LoadedMapPack}, MapPackInfo}, Controller};
use bitvec::vec::BitVec;
use tokio::sync::broadcast;
use taimi_pack::attributes::keys::Guid;

#[derive(Debug, Clone)]
pub struct SharedMapPackInfo {
    pub shared_loader: Option<Arc<PackLoader>>,
    pub interactions: broadcast::Sender<InteractionEvent>,
    pub pack_info: BTreeMap<PackPath, Result<Arc<PackInfo>, UnloadedPack>>,
    pub pack_loaded: BTreeSet<PackPath>,
    pub map_info: BTreeMap<PackMapPath, Arc<MapPackInfo>>,
    pub map_state: BTreeMap<PackMapPath, SharedMapPackState>,
}

impl SharedMapPackInfo {
    pub const INTERACTIONS_BUFFER_LEN: usize = 48;

    pub fn map_info_with<R, F: FnOnce(PackMapPath, &Arc<MapPackInfo>) -> R>(path: PackPath, f: F) -> Option<R> {
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

    pub fn is_loaded(&self, path: &PackPath) -> bool {
        self.pack_loaded.contains(path)
    }

    #[cfg(todo = "unused")]
    pub fn unloaded_pack_info(&self) -> impl Iterator<Item = (PackPath, &UnloadedPack)> + '_ {
        self.pack_info.iter().filter_map(|(&path, info)|
            info.as_ref().err().map(|e| (path, e))
        )
    }
    pub fn pack_info(&self) -> impl Iterator<Item = (PackPath, &Arc<PackInfo>)> + '_ {
        self.pack_info.iter().filter_map(|(&path, info)|
            info.as_ref().ok().map(|e| (path, e))
        )
    }
}

impl Default for SharedMapPackInfo {
    fn default() -> Self {
        Self {
            shared_loader: Default::default(),
            interactions: broadcast::Sender::new(Self::INTERACTIONS_BUFFER_LEN),
            pack_info: Default::default(),
            pack_loaded: Default::default(),
            map_info: Default::default(),
            map_state: Default::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct UnloadedPack {
    pub path: PathBuf,
    pub reason: UnloadedReason,
}

impl fmt::Display for UnloadedPack {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let name = self.path.file_name()
            .map(Path::new)
            .unwrap_or_else(|| rt::relative_path(&self.path));
        write!(f, "{}", name.display())
    }
}

#[derive(Debug, Clone, Default)]
pub struct SharedMapPackState {
    pub categories: Arc<[LoadedCategory]>,
    pub interactive_pois: Arc<[InteractivePoi]>,
    pub interactive_pois_nearby: BitVec,
    pub poi_guids: Arc<[Guid]>,
}

impl SharedMapPackState {
    pub fn with_loaded(map_pack: &LoadedMapPack) -> Self {
        let categories = map_pack.categories.clone();
        let interactive_pois = map_pack.interactive_pois.clone();
        Self {
            categories,
            interactive_pois,
            interactive_pois_nearby: map_pack.interactive_pois_nearby.clone(),
            poi_guids: map_pack.poi_guids.clone(),
        }
    }
    pub fn update_static(&mut self, map_pack: &LoadedMapPack) {
        self.categories = map_pack.categories.clone();
        self.interactive_pois = map_pack.interactive_pois.clone();
    }

    pub fn categories<'a, 'i>(&'a self, info: &'i MapPackInfo) -> impl Iterator<Item = (CategoryPath, &'a LoadedCategory)> + 'i where
        'a: 'i,
    {
        info.categories().zip(self.categories.iter())
    }
}
