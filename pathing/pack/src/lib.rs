#![doc(html_logo_url = "https://taimihud.com/logotype-holo.png")]
//! Parser and data types for GW2 TacO pathing packs.
pub mod attributes;
pub mod category;
pub mod loader;
pub mod pack;
pub mod poi;
pub mod trail;

pub use {
    self::{
        attributes::MarkerAttributes,
        category::Category,
        loader::{LoaderAssetReader, PackLoaderContext},
        pack::Pack,
        poi::Poi,
        trail::Trail,
    },
    uuid::Uuid,
};
