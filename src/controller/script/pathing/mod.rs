#[cfg(feature = "paths-lua")]
use {
    crate::{
        controller::script::{
            event::ScriptNotification,
            lua::LuaMessage,
            ScriptController,
        },
        exports::runtime as rt,
    },
    taimi_meta::{packs::MapIndex, map::MapID},
    taimi_pack::{category::id::CategoryId, script::lua::RuntimeLua},
};
use {
    crate::controller::{
        pathing::{
            shared::SharedPackLoad,
            registry::{LoadedMarkerPath, PackPath, SharedLoaderBox},
            PathingEvent,
        },
        script::{id::{PackScriptPath, ScriptIndex}, PlugSharedData, ScriptMessage},
        Controller,
    },
    anyhow::Context,
    core::{fmt, mem},
    std::{
        collections::{BTreeMap, BTreeSet},
        sync::Arc,
    },
    taimi_pack::{
        attributes::{keys, cell::{PackKeyId, GetAttrDyn, SetAttrDyn, PackValueCell, PackValueOf, PackKeySet, pack_attr}, RenderAttributes},
        pack::Pack,
        script::{
            self,
            pathing::imp::{MarkerLoc, MarkerType, PackOverridesShared, PackOverrides, MarkerOverrides},
        },
    },
    taimi_meta::packs::{MarkerIndex, MarkerId, MarkerPath, CategoryIndex, VisibilityFlags},
    taimi_hoard::loc::LocationRef,
};

#[cfg(feature = "paths-lua")]
mod lua;
#[cfg(feature = "paths-lua")]
pub use self::lua::LuaPackDesc;

#[cfg(not(feature = "paths-lua"))]
pub type LuaPackDesc = ();

#[cfg(feature = "paths-lua")]
pub const PACK_ENTRYPOINT: &'static str = RuntimeLua::PACK_ENTRYPOINT;

pub struct PackPlugShared {
    pub plug: PlugSharedData,
    pub path: PackScriptPath,
    pub load: SharedPackLoad,
    pub overrides: PackOverridesShared,
}
impl PackPlugShared {
    #[inline]
    pub fn new(path: PackPath) -> anyhow::Result<Self> {
        Controller::with_sender(|s| s.pathing.as_ref().and_then(|p|
            p.shared.packs.packs.borrow().lookup_ref(&path).cloned()
        )).flatten()
            .map(Self::with_shared_pack)
            .context("pack not found")
    }
    #[inline]
    pub fn with_shared_pack(load: SharedPackLoad) -> Self {
        let name =
            load.info.info.as_ref().and_then(|i| i.primary_root()).and_then(|r| r.display_name.as_ref().map(|n| &n[..]))
            .or_else(|| load.info.path_name().to_str())
            .unwrap_or("pack");
        let path: PackScriptPath = load.info.index.pivot_from();
        Self {
            plug: PlugSharedData::with_name(path.pivot_from(), name),
            path,
            load,
            overrides: Default::default(),
        }
    }
    pub fn get_pack(&self) -> script::Result<Arc<Pack>> {
        self.load.loaded.borrow().pack.clone()
            .context("pack unloaded")
    }
    pub fn get_loader(&self) -> script::Result<SharedLoaderBox> {
        self.load.loaded.borrow().loader.clone()
            .context("pack unloaded")
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
            .field(&format_args!("{}", self.load.info))
            .field(&self.plug)
            .finish()
    }
}

/// private implementation details
#[derive(Default)]
pub(super) struct PackPlugStash {
    /// fresh markers that may need script attr events registered
    pub pending_start: BTreeSet<MarkerPath>,
    pub pending_changes: BTreeMap<MarkerPath, PackKeySet>,
    pub outbound_pathing: PathingEvent,
    pub changes_dirty: bool,
}
impl PackPlugStash {
    /// TODO: event should be unnecessary if signal any other way...
    pub fn record_start(&mut self, marker_path: MarkerPath) {
        self.pending_start.insert(marker_path);
    }
    pub fn record_changes(&mut self, marker_path: MarkerPath, keys: impl IntoIterator<Item = PackKeyId>) {
        let dirty = &mut self.changes_dirty;
        #[cfg(todo = "unnecessary")]
        let keys = keys.into_iter().inspect(|_| *dirty |= true);
        *dirty = true;
        self.pending_changes.entry(marker_path).or_default().extend(keys);
    }
    pub fn collect_changes_from(pending: &PackKeySet, shared: &PackPlugShared, marker_path: MarkerPath) -> Option<Vec<PackValueCell>> {
        let o = PackOverrides::shared_read(&shared.overrides)
            .overrides.get(&marker_index2loc(marker_path.path)).cloned();
        o.as_ref().map(MarkerOverrides::shared_read).map(|o|
            pending.iter().filter_map(|&key| o.get_dyn(key).and_then(|v|
                match v.map(|v| v.clone_dyn()) {
                    None => Some(PackValueCell::new_empty(key)),
                    Some(None) => {
                        log::warn!("lost {key} to clone");
                        None
                    },
                    Some(Some(v)) => Some(PackValueCell::from_box(v)),
                }
            )).collect::<Vec<_>>()
        )
    }
    pub fn drain_changes_for(&mut self, shared: &PackPlugShared, marker_path: MarkerPath) -> Vec<PackValueCell> {
        let pending = self.pending_changes.get_mut(&marker_path).and_then(|c|
            (!c.is_empty()).then_some(c)
        );
        let Some(pending) = pending else {
            return Vec::new()
        };
        let changes = Self::collect_changes_from(&*pending, shared, marker_path);
        if changes.is_some() {
            //pending.clear();
            self.pending_changes.remove(&marker_path);
        }
        #[cfg(deleteme)]
        if let (MarkerIndex::NS_TRAIL, Some(changes)) = (marker_path.path.namespace(), &changes) {
            for c in changes {
                log::debug!("changing a trail key {}", c.id());
            }
        }
        changes.unwrap_or_default()
    }
    #[inline]
    #[cfg(todo = "unused")]
    pub fn drain_all_changes<'a>(&'a mut self, shared: &'a PackPlugShared) -> impl Iterator<Item = (MarkerPath, Vec<PackValueCell>)> + 'a {
        Self::drain_all_changes_imp(&mut self.changes_dirty, &mut self.pending_changes, shared)
    }
    pub(super) fn drain_all_changes_imp<'a>(dirty: &'_ mut bool, pending: &'a mut BTreeMap<MarkerPath, PackKeySet>, shared: &'a PackPlugShared) -> impl Iterator<Item = (MarkerPath, Vec<PackValueCell>)> + 'a {
        *dirty = false;
        pending.iter_mut().filter_map(move |(&path, pending)| {
            let changes = Self::collect_changes_from(&*pending, shared, path);
            if changes.is_some() {
                pending.clear();
            }
        #[cfg(deleteme)]
            if let (MarkerIndex::NS_TRAIL, Some(changes)) = (path.path.namespace(), &changes) {
                for c in changes {
                    log::debug!("changing a trail key {}", c.id());
                }
            } else if MarkerIndex::NS_TRAIL == path.path.namespace() {
                log::debug!("trail#{} changes empty?", path.path.trail_index_unchecked());
            }
            changes.map(|c| (path, c))
        })
    }
    pub fn prune_changes(&mut self) {
        self.pending_changes.retain(|_, c|
            !c.is_empty()
        );
        self.changes_dirty = !self.pending_changes.is_empty();
    }
    pub fn prepare_map_exit(&mut self) {
        #[cfg(deleteme)]
        log::debug!("AAAAAKJNASD preparing map exit!");
        self.changes_dirty = false;
        self.pending_changes.clear();
        self.pending_start = Default::default();
        self.outbound_pathing = Default::default();
    }
    pub fn is_dirty(&self) -> bool { self.changes_dirty | !self.pending_start.is_empty() }

    #[inline]
    pub fn queue_outbound_pathing(&mut self, event: impl Into<PathingEvent>) {
        self.outbound_pathing.push(event.into())
    }
    #[inline]
    pub fn process_changes_to_outbound(&mut self, pack_path: PackPath, shared: &PackPlugShared) {
        if !self.changes_dirty { return }
        let changes = PackPlugStash::drain_all_changes_imp(&mut self.changes_dirty, &mut self.pending_changes, shared)
            .map(|(marker_path, changes)| PathingEvent::CommitMarkerAttrs {
                marker: MarkerId::for_marker(pack_path.rel(marker_path)),
                write_attrs: Box::new(changes.into_iter()) as Box<_>,
            });
        self.outbound_pathing.extend(changes);
        self.prune_changes();
    }
    /// [try_send](crate::controller::pathing::PathingController::try_send) but queue
    #[inline]
    pub fn process_outbound_pathing(&mut self) {
        use tokio::sync::mpsc::error::TrySendError;
        let outgoing = mem::replace(&mut self.outbound_pathing, PathingEvent::Nop);
        if let PathingEvent::Nop = outgoing {
            return
        }
        let leftover = Controller::with_sender(|s| match s.pathing.as_ref() {
            Some(p) => match p.command.try_send(outgoing) {
                | Ok(())
                | Err(TrySendError::Closed(..))
                => PathingEvent::Nop,
                Err(TrySendError::Full(e)) => {
                    #[cfg(taimi_debug)]
                    log::debug!("SPACE QUEUE FULL");
                    e
                },
            },
            None => PathingEvent::Nop,
        });
        match leftover {
            None | Some(PathingEvent::Nop) => (),
            Some(e) => self.outbound_pathing = e,
        }
    }
}

/// TODO: deleteme, also does not check for validity
pub fn marker_index2loc(path: MarkerIndex) -> MarkerLoc {
    let idx = path.index();
    match path.namespace() {
        MarkerIndex::NS_CAT => (MarkerType::Category, idx as _),
        MarkerIndex::NS_TRAIL => (MarkerType::Trail, path.trail_index_unchecked() as _),
        _ => (MarkerType::Poi, idx as _),
    }
}
/// TODO: deleteme
pub fn marker_loc2index((ty, index): MarkerLoc) -> MarkerPath {
    MarkerPath::new_path(MarkerIndex::new(
        marker_ty2ns(ty),
        index as u32,
    ))
}
/// TODO: deleteme
pub fn marker_ty2ns(ty: MarkerType) -> u32 {
    match ty {
        MarkerType::Poi => MarkerIndex::NS_POI,
        MarkerType::Trail => MarkerIndex::NS_TRAIL,
        MarkerType::Category => MarkerIndex::NS_CAT,
    }
}

/// TODO: remove reliance on lua here
#[cfg(feature = "paths-lua")]
impl ScriptMessage {
    pub fn menu_clicked_pack(id: CategoryId, target: PackPath) -> Self {
        Self::menu_clicked_with(id, target.pivot_from())
    }
    pub fn marker_event(
        id: ScriptNotification,
        marker: MarkerPath,
        target: PackPath,
    ) -> Self {
        let args = vec![Box::new(Some(marker.path.repr()))
            as Box<dyn taimi_pack::script::lua::IntoLuaMut + Send>];
        LuaMessage::NotifyScriptWith {
            id,
            context: ScriptIndex::for_pack(target),
            args,
        }
        .into()
    }
    pub fn marker_event_bool(
        id: ScriptNotification,
        arg: bool,
        marker: MarkerPath,
        target: PackPath,
    ) -> Self {
        let args = vec![
            Box::new(Some(marker.path.repr()))
                as Box<dyn taimi_pack::script::lua::IntoLuaMut + Send>,
            Box::new(Some(arg)) as Box<_>,
        ];
        LuaMessage::NotifyScriptWith {
            id,
            context: ScriptIndex::for_pack(target),
            args,
        }
        .into()
    }
    pub fn map_prepared_pack<I>(target: PackPath, map_id: MapID, active_markers: I) -> Self
    where
        I: IntoIterator<Item = MarkerPath>,
        I::IntoIter: Send + 'static,
    {
        let active_markers = Box::new(active_markers.into_iter()) as Box<_>;
        LuaMessage::NotifyMapEnter {
            target: target.pivot_from(),
            map_id,
            active_markers,
            append: false,
        }
        .into()
    }
    pub fn map_left_pack(target: PackPath, left: Option<MapIndex>) -> Self {
        let map_id = left.map(|l| l.get()).unwrap_or(0);
        let args = vec![
            Box::new(Some(map_id))
                as Box<dyn taimi_pack::script::lua::IntoLuaMut + Send>,
        ];
        LuaMessage::NotifyScriptWith {
            id: ScriptNotification::PathingMapExit,
            context: ScriptIndex::for_pack(target),
            args,
        }.into()
    }
}
#[cfg(feature = "paths-lua")]
impl ScriptController {
    pub(super) async fn do_refresh_packs() {
        let shared = Controller::with_sender(|s|
            s.pathing.as_ref().map(|p| p.shared.clone())
            .and_then(|p| s.scripting.as_ref().map(|s| (p, s.plugs_shared.clone())))
        ).flatten();
        let Some((pathing, script)) = shared else {
            // pathing.context("pathing offline")?
            return
        };
        let packs = pathing.packs.packs.borrow().iter()
            .filter_map(|(path, pack)|
                pack.loaded.borrow().loader.clone().map(|l| (path, l))
            ).collect::<Vec<_>>();
        let packs = packs.into_iter().map(|(path, l)| async move {
            let has = l.lock().await.contains_asset(PACK_ENTRYPOINT)
                .context("detecting pack.lua");
            rt::log::warn_ok(has).unwrap_or(false)
                .then_some(path)
        });
        // TODO: async collect etc
        let mut avail = std::collections::BTreeSet::new();
        for pack in packs {
            if let Some(pack) = pack.await {
                avail.insert(pack);
            }
        }
        script.send_if_modified(|plugs| {
            let prev_empty = plugs.available_packs.is_empty();
            plugs.available_packs = avail;
            plugs.available_packs.is_empty() != prev_empty
        });
    }
}
