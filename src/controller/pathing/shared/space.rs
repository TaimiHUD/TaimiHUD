use {
    crate::{
        controller::pathing::{
            registry::{LoadedMarkerPath, LoadedTrailPath},
            shared::{
                LoadedTrailGeometry,
                LoadedTrailSection,
                SharedPackInfo,
                SharedResourceRequests,
                SharedResourceRequestsTx,
            },
            space::SpacePackCollection,
        },
        exports::runtime::textures::{TextureKey, TextureSlot},
        TEXTURES,
    },
    std::{fmt, sync::Arc},
    taimi_meta::packs::{
        id::{MarkerId, MarkerIndex},
        PackMapPath,
    },
    taimi_pack::attributes::AttrString,
    taimi_sync::watched::watch,
};

#[derive(Clone)]
pub struct SpacePackShared {
    pub collection: watch::Sender<Arc<SpacePackCollection>>,
    pub trail_geometry: TrailGeometryRequestsTx,
    pub texture_loads: TextureLoadRequestsTx,
}
impl SpacePackShared {
    pub fn new() -> Self {
        Self {
            collection: Default::default(),
            trail_geometry: TrailGeometryRequests::new_sender(),
            texture_loads: TextureLoadRequests::new_sender(),
        }
    }
    pub fn trail_geometry_id(path: &LoadedTrailPath<PackMapPath>) -> MarkerId {
        let path = path.map_path(|path| MarkerIndex::with_trail_section(path, 0));
        MarkerId::for_marker(path)
    }
    fn setup_texture(
        key: &mut Option<TextureKey>,
        slot: &mut Option<TextureSlot>,
        pack_info: &SharedPackInfo,
        texture: Option<&AttrString>,
    ) {
        let key = match key {
            Some(key) => return Self::get_texture(&*key, slot),
            None => match texture {
                Some(texture) => key.insert(pack_info.key_for_subresource(texture)),
                None => return,
            },
        };
        *slot = TEXTURES.reserve_key_mut(key);
    }
    pub fn get_texture(key: &TextureKey, slot: &mut Option<TextureSlot>) {
        if Self::slot_loaded(slot.as_ref()) {
            return
        }
        *slot = TEXTURES.lookup_slot(key);
    }
    pub fn slot_loaded(slot: Option<&TextureSlot>) -> bool {
        slot.map(|slot| !slot.is_loading() && !slot.can_load())
            .unwrap_or(false)
    }
}
impl Default for SpacePackShared {
    fn default() -> Self {
        Self::new()
    }
}
impl fmt::Debug for SpacePackShared {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("SpacePackShared").finish()
    }
}
impl SharedPackInfo {
    #[inline]
    pub fn setup_texture(
        &self,
        key: &mut Option<TextureKey>,
        slot: &mut Option<TextureSlot>,
        texture: Option<&AttrString>,
    ) {
        SpacePackShared::setup_texture(key, slot, self, texture)
    }
}
pub enum LoadResult<T> {
    Loaded(T),
    /// produce fallback or disable entity etc
    Failed,
    /// drop cache and request again if still needed
    Invalidate,
}
impl<T> LoadResult<T> {
    #[doc(alias = "ok")]
    pub fn get(self) -> Option<T> {
        match self {
            Self::Loaded(v) => Some(v),
            _ => None,
        }
    }
}
pub type TrailGeometryRequests =
    SharedResourceRequests<LoadedTrailPath<PackMapPath>, LoadResult<LoadedTrailGeometry>>;
pub type TrailGeometryRequestsTx =
    SharedResourceRequestsTx<LoadedTrailPath<PackMapPath>, LoadResult<LoadedTrailGeometry>>;
pub type TextureLoadRequests =
    SharedResourceRequests<LoadedMarkerPath<PackMapPath>, LoadResult<TextureKey>>;
pub type TextureLoadRequestsTx =
    SharedResourceRequestsTx<LoadedMarkerPath<PackMapPath>, LoadResult<TextureKey>>;
pub type TrailGeometrySections = Arc<[LoadedTrailSection]>;
