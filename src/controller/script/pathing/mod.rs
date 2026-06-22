#[cfg(feature = "paths-lua")]
use {
    crate::controller::script::{
        event::ScriptNotification,
        lua::{LuaExecContext, LuaMessage},
    },
    taimi_meta::map::MapID,
    taimi_pack::{category::id::CategoryId, script::lua::RuntimeLua},
};
use {
    crate::controller::{
        pathing::registry::SharedLoaderBox as SharedLoader,
        script::{PlugSharedData, ScriptMessage},
    },
    anyhow::Context,
    core::fmt,
    std::{
        collections::BTreeMap,
        sync::{Arc, Mutex, Weak},
    },
    taimi_pack::{
        loader::PackLoaderContext,
        pack::Pack,
        script::{
            self,
            pathing::imp::{MarkerLoc, PackOverridesShared},
        },
    },
};

#[cfg(feature = "paths-lua")]
mod lua;
#[cfg(feature = "paths-lua")]
pub use self::lua::LuaPackDesc;

#[cfg(not(feature = "paths-lua"))]
pub type LuaPackDesc = ();

#[cfg(feature = "paths-lua")]
pub const PACK_ENTRYPOINT: &'static str = RuntimeLua::PACK_ENTRYPOINT;

pub type WeakLoader = Weak<tokio::sync::Mutex<Box<dyn PackLoaderContext + Send + 'static>>>;
pub struct PackPlugShared {
    pub plug: PlugSharedData,
    pub path: PackLoc,
    pub pack: Weak<Pack>,
    pub loader: WeakLoader,
    pub overrides: PackOverridesShared,
    /// fresh (dynamic) markers that may need script attr events registered
    ///
    /// TODO: this should remain private on pack desc local state, like a RefCell at most
    pub(super) pending_start: Mutex<Vec<MarkerLoc>>,
    pub(super) active_markers: Mutex<BTreeMap<u32, PoiStatus>>,
}
impl PackPlugShared {
    #[inline]
    pub fn new(path: PackLoc, pack: &Arc<Pack>, loader: WeakLoader) -> Self {
        Self {
            path,
            plug: PlugSharedData::with_name(&pack.name[..]),
            pack: Arc::downgrade(pack),
            loader,
            overrides: Default::default(),
            pending_start: Default::default(),
            active_markers: Default::default(),
        }
    }
}
impl PackPlugShared {
    pub fn get_pack(&self) -> script::Result<Arc<Pack>> {
        self.pack.upgrade().context("pack unloaded")
    }
    pub fn get_loader(&self) -> script::Result<SharedLoader> {
        self.loader.upgrade().context("pack unloaded")
    }
}
impl AsRef<PlugSharedData> for PackPlugShared {
    #[inline]
    fn as_ref(&self) -> &PlugSharedData {
        &self.plug
    }
}
impl AsRef<PlugSharedData> for Arc<PackPlugShared> {
    #[inline]
    fn as_ref(&self) -> &PlugSharedData {
        &self.plug
    }
}
impl fmt::Debug for PackPlugShared {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("PackPlug")
            .field(&format_args!("{}", self.path))
            .field(&self.plug)
            .finish()
    }
}

#[derive(Copy, Clone, Default)]
pub(super) struct PoiStatus {
    pub focused: bool,
    #[cfg(todo = "unnecessary")]
    pub filtered: bool,
}

/// TODO: replace with pathcontrol locator type
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackLoc {
    /// invalidated when packs are unloaded/cleared
    /// (index is stable otherwise)
    pub generation: usize,
    /// reference inside PackCollection in engine
    pub index: usize,
}
impl PackLoc {
    #[inline]
    pub const fn new(generation: usize, index: usize) -> Self {
        Self { generation, index }
    }
}
impl fmt::Display for PackLoc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "pack#{}", self.index)?;
        if self.generation > 1 {
            write!(f, "(gen{})", self.generation)?;
        }
        Ok(())
    }
}

/// TODO: remove reliance on lua here
#[cfg(feature = "paths-lua")]
impl ScriptMessage {
    pub fn menu_clicked_pack(id: CategoryId, generation: usize, index: usize) -> Self {
        let loc = PackLoc::new(generation, index);
        Self::menu_clicked_with(id, LuaExecContext::Pack(loc))
    }
    pub fn marker_event(
        id: ScriptNotification,
        marker: MarkerLoc,
        generation: usize,
        index: usize,
    ) -> Self {
        let loc = PackLoc::new(generation, index);
        let args = vec![Box::new(Some(LuaPackDesc::pathable_tag_for(marker)))
            as Box<dyn taimi_pack::script::lua::IntoLuaMut + Send>];
        LuaMessage::NotifyScriptWith {
            id,
            context: LuaExecContext::Pack(loc),
            args,
        }
        .into()
    }
    pub fn marker_event_bool(
        id: ScriptNotification,
        arg: bool,
        marker: MarkerLoc,
        generation: usize,
        index: usize,
    ) -> Self {
        let loc = PackLoc::new(generation, index);
        let args = vec![
            Box::new(Some(LuaPackDesc::pathable_tag_for(marker)))
                as Box<dyn taimi_pack::script::lua::IntoLuaMut + Send>,
            Box::new(Some(arg)) as Box<_>,
        ];
        LuaMessage::NotifyScriptWith {
            id,
            context: LuaExecContext::Pack(loc),
            args,
        }
        .into()
    }
    pub fn map_prepared_pack<I>(generation: usize, index: usize, map_id: MapID, active_markers: I) -> Self
    where
        I: IntoIterator<Item = MarkerLoc>,
        I::IntoIter: Send + 'static,
    {
        let target = PackLoc::new(generation, index);
        let active_markers = Box::new(active_markers.into_iter()) as Box<_>;
        LuaMessage::NotifyMapEnter {
            target,
            map_id,
            active_markers,
            append: false,
        }
        .into()
    }
}
