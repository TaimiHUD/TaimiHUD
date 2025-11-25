use {
    crate::controller::pathing::registry::RecentlyUsed,
    std::{ops, sync::Arc},
};

pub mod info;
pub mod shared;
pub mod festival;

#[derive(Debug, Clone)]
pub struct MapPackInfoStorage {
    pub used: RecentlyUsed,
    pub info: Arc<info::MapPackInfo>,
}

impl MapPackInfoStorage {
    pub const fn new(info: Arc<info::MapPackInfo>) -> Self {
        Self {
            used: RecentlyUsed::DEFAULT,
            info,
        }
    }
}

impl ops::Deref for MapPackInfoStorage {
    type Target = info::MapPackInfo;
    fn deref(&self) -> &Self::Target {
        &self.info
    }
}
