pub mod dx11;
pub mod engine;
pub mod object;
pub mod pack;
pub mod render_list;
#[cfg(feature = "goggles")]
pub mod goggles;
#[deprecated = "crate::resources"]
pub(crate) use crate::resources;

pub type DrawSpace = taimi_meta::coords::LocalSpace;
pub type ScreenSpace = taimi_meta::coords::ScreenSpace;

taimi_meta::coords::coord_newtype! {
/// UV coords 0.0 to 1.0
pub struct TextureSpace([f32; 2]);
}

pub use self::engine::Engine;

use taimi_meta::ui::MapContext;

#[derive(Copy, Clone)]
pub enum LocalContext {
    World,
    Map(MapContext),
}

impl LocalContext {
    pub const MAP: Self = Self::GLOBAL;
    pub const GLOBAL: Self = Self::Map(MapContext::Global);
    pub const MINIMAP: Self = Self::Map(MapContext::Minimap);

    pub fn as_map(&self) -> Option<MapContext> {
        match *self {
            LocalContext::World => None,
            LocalContext::Map(map) => Some(map),
        }
    }

    pub fn is_map(&self) -> bool {
        matches!(self, LocalContext::Map(..))
    }
}

impl From<MapContext> for LocalContext {
    fn from(map: MapContext) -> Self {
        Self::Map(map)
    }
}

impl From<Option<MapContext>> for LocalContext {
    fn from(value: Option<MapContext>) -> Self {
        match value {
            None => Self::World,
            Some(map) => Self::Map(map),
        }
    }
}

impl From<LocalContext> for Option<MapContext> {
    fn from(value: LocalContext) -> Self {
        value.as_map()
    }
}
