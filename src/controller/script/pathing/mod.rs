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
        pathing::{
            shared::SharedPackLoad,
            registry::{LoadedMarkerPath, PackMapPath, PackPath, SharedLoaderBox},
        },
        script::{PlugSharedData, ScriptMessage},
        Controller,
    },
    anyhow::Context,
    core::fmt,
    std::{
        collections::{BTreeMap, BTreeSet},
        sync::{Arc, Mutex},
    },
    taimi_pack::{
        attributes::{keys, cell::{PackKeyId, GetAttrDyn, SetAttrDyn, PackValueCell, PackValueOf, pack_attr}, RenderAttributes},
        pack::Pack,
        script::{
            self,
            pathing::imp::{MarkerLoc, MarkerType, PackOverridesShared, PackOverrides, MarkerOverrides},
        },
    },
    taimi_meta::packs::{MarkerIndex, MarkerPath, CategoryIndex, VisibilityFlags},
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

#[cfg(deleteme)]
pub type WeakLoader = Weak<tokio::sync::Mutex<Box<dyn PackLoaderContext + Send + 'static>>>;
pub struct PackPlugShared {
    pub plug: PlugSharedData,
    pub path: PackPath,
    #[cfg(deleteme)]
    pub pack: Weak<Pack>,
    pub load: SharedPackLoad,
    #[cfg(deleteme)]
    pub loader: WeakLoader,
    pub overrides: PackOverridesShared,
    /// fresh (dynamic) markers that may need script attr events registered
    ///
    /// TODO: this should remain private on pack desc local state, like a RefCell at most
    pub(super) pending_start: Mutex<Vec<MarkerPath>>,
    pub(super) active_markers: Mutex<BTreeMap<MarkerPath, PoiStatus>>,
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
        Self {
            path: load.info.index,
            plug: PlugSharedData::with_name(name),
            load,
            overrides: Default::default(),
            pending_start: Default::default(),
            active_markers: Default::default(),
        }
    }
}
impl PackPlugShared {
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

#[derive(Clone)]
pub(super) struct PoiStatus {
    pub focused: bool,
    pub dynamic: bool,
    #[cfg(todo = "unnecessary")]
    pub filtered: bool,
    pub lpath: LoadedMarkerPath,
    pub pending_changes: BTreeSet<PackKeyId>,
}
impl PoiStatus {
    pub fn flush_changes_to(&mut self, shared: &PackPlugShared, marker_path: MarkerPath) {
        if self.pending_changes.is_empty() { return }
        let gp = Controller::with_sender(|s| s.pathing.as_ref().map(|s| s.shared.gameplay.clone())).flatten();
        let Some(gp) = gp else { return };
        let changes = {
            let o = PackOverrides::shared_read(&shared.overrides)
                .overrides.get(&marker_index2loc(marker_path.path)).cloned();
            o.as_ref().map(MarkerOverrides::shared_read).map(|o|
                self.pending_changes.iter().filter_map(|&key| o.get_dyn(key).map(|v| (key, v.and_then(|v| v.clone_dyn()).map(PackValueCell::from_box)))).collect::<Vec<_>>()
            )
        };
        let Some(changes) = changes else { return };
        self.pending_changes.clear();
        gp.send_if_modified(|gp| {
            #[cfg(todo)]
            if gp.map_id != Some(map_id) { return }
            let (Some(map_info), map) = gp.for_pack_mut(shared.path) else {
                return false
            };
            let lpath = if self.lpath.path == MarkerIndex::UNK {
                if self.dynamic {
                    MarkerPath::new_path(match marker_path.path.namespace() {
                        MarkerIndex::NS_POI => MarkerIndex::with_poi(map_info.pois[..].len() as _),
                        MarkerIndex::NS_TRAIL => MarkerIndex::with_trail(map_info.trails[..].len() as _),
                        MarkerIndex::NS_CAT => MarkerIndex::with_category(map_info.categories[..].len() as _),
                        _ => return false,
                    })
                } else {
                    let Some(idx) = map_info.marker_index(marker_path) else { return false };
                    self.lpath = idx;
                    self.lpath
                }
            } else {
                self.lpath
            };
            let idx = lpath.path.index() as usize;
            let (mut pois, mut pois_state, mut poi) = (Vec::<crate::controller::pathing::info::LoadedPoiInfo>::new(), None, None);
            let (mut trails, mut trails_state, mut trail) = (Vec::<crate::controller::pathing::info::LoadedTrailInfo>::new(), None, None);
            let (mut cats, mut cats_state, mut cat) = (Vec::<u32>::new(), None, None);
            match lpath.path.namespace() {
                MarkerIndex::NS_POI => {
                    pois_state = map.as_mut().and_then(|map| if map.pois[..].len() <= idx {
                        let mut pois = map.pois[..].to_owned();
                        let init = crate::controller::pathing::shared::LoadedPoiShared {
                            visibility: VisibilityFlags::all(),
                            position: Default::default(),
                        };
                        pois.resize(idx + 1, init);
                        Some(pois)
                    } else {
                        None
                    });
                    pois = map_info.pois[..].to_owned();
                    if pois.len() <= idx {
                        pois.resize_with(idx + 1, Default::default);
                    }
                    let poi = poi.insert(unsafe {
                        pois.get_unchecked_mut(idx)
                    });
                    if poi.category_path.path == CategoryIndex::MAX {
                        if let Some((cat_info, ..)) = shared.load.info.category_info() {
                            if let Some(root) = cat_info.root_paths().next() {
                                poi.marker_info.category_path = root;
                            }
                        }
                    }
                },
                MarkerIndex::NS_TRAIL => {
                    trails_state = map.as_mut().and_then(|map| if map.trails[..].len() <= idx {
                        let mut trails = map.trails[..].to_owned();
                        let init = crate::controller::pathing::shared::LoadedTrailShared {
                            visibility: VisibilityFlags::all(),
                        };
                        trails.resize(idx + 1, init);
                        Some(trails)
                    } else {
                        None
                    });
                    trails = map_info.trails[..].to_owned();
                    if trails.len() <= idx {
                        trails.resize_with(idx + 1, Default::default);
                    }
                    let trail = trail.insert(unsafe {
                        trails.get_unchecked_mut(idx)
                    });
                    if trail.category_path.path == CategoryIndex::MAX {
                        if let Some((cat_info, ..)) = shared.load.info.category_info() {
                            if let Some(root) = cat_info.root_paths().next() {
                                trail.marker_info.category_path = root;
                            }
                        }
                    }
                },
                MarkerIndex::NS_CAT => {
                    cats_state = map.as_mut().and_then(|map| if map.categories[..].len() <= idx {
                        let mut cats = map.categories[..].to_owned();
                        cats.resize_with(idx + 1, Default::default);
                        Some(cats)
                    } else {
                        None
                    });
                    cats = map_info.categories[..].to_owned();
                    let resizing = cats.len() <= idx;
                    if resizing {
                        cats.resize_with(idx + 1, Default::default);
                    }
                    let cat = cat.insert(unsafe {
                        cats.get_unchecked_mut(idx)
                    });
                    if resizing {
                        **cat = marker_path.path.index();
                    }
                },
                _ => return false,
            }
            let mut modified = false;
            for (key, value) in changes {
                let applied = pack_attr! { match =id_is(key) {
                    = keys::CategoryRef => {
                        // TODO
                        Some(false)
                    },
                    = keys::DefaultToggle => {
                        let vis = match lpath.path.namespace() {
                            MarkerIndex::NS_POI => {
                                if pois_state.is_none() {
                                    pois_state = map.as_mut().map(|map| map.pois[..].to_owned());
                                }
                                let poi = pois_state.as_mut().map(|pois| unsafe {
                                    pois.get_unchecked_mut(idx)
                                });
                                poi.map(|poi| &mut poi.visibility)
                            },
                            MarkerIndex::NS_TRAIL => {
                                if trails_state.is_none() {
                                    trails_state = map.as_mut().map(|map| map.trails[..].to_owned());
                                }
                                let trail = trails_state.as_mut().map(|trails| unsafe {
                                    trails.get_unchecked_mut(idx)
                                });
                                trail.map(|trail| &mut trail.visibility)
                            },
                            MarkerIndex::NS_CAT => {
                                if cats_state.is_none() {
                                    cats_state = map.as_mut().map(|map| map.categories[..].to_owned());
                                }
                                let cat = cats_state.as_mut().map(|cats| unsafe {
                                    cats.get_unchecked_mut(idx)
                                });
                                cat.map(|cat| &mut cat.visibility)
                            },
                            _ => None,
                        };
                        let on = value.as_ref().and_then(|v| PackValueOf::<keys::DefaultToggle>::from_cell_ref(v))
                            .map(|v| *v.get())
                            .unwrap_or_default();
                        Some(if let Some(vis) = vis {
                            vis.set(VisibilityFlags::TOGGLES, on.into());
                            true
                        } else { false })
                    },
                    = keys::PositionX => Some({
                        match lpath.path.namespace() {
                            MarkerIndex::NS_POI => {
                                if pois_state.is_none() {
                                    pois_state = map.as_mut().map(|map| map.pois[..].to_owned());
                                }
                                let poi = pois_state.as_mut().map(|pois| unsafe {
                                    pois.get_unchecked_mut(idx)
                                });
                                if let Some(poi) = poi {
                                    poi.position.x = value.as_ref().and_then(|v| PackValueOf::<keys::PositionX>::from_cell_ref(v))
                                        .map(|v| f32::from(*v.get()))
                                        .unwrap_or_default();
                                    true
                                } else {
                                    false
                                }
                            },
                            _ => false,
                        }
                    }),
                    = keys::PositionY => Some({
                        match lpath.path.namespace() {
                            MarkerIndex::NS_POI => {
                                if pois_state.is_none() {
                                    pois_state = map.as_mut().map(|map| map.pois[..].to_owned());
                                }
                                let poi = pois_state.as_mut().map(|pois| unsafe {
                                    pois.get_unchecked_mut(idx)
                                });
                                if let Some(poi) = poi {
                                    poi.position.y = value.as_ref().and_then(|v| PackValueOf::<keys::PositionY>::from_cell_ref(v))
                                        .map(|v| f32::from(*v.get()))
                                        .unwrap_or_default();
                                    true
                                } else {
                                    false
                                }
                            },
                            _ => false,
                        }
                    }),
                    = keys::PositionZ => Some({
                        match lpath.path.namespace() {
                            MarkerIndex::NS_POI => {
                                if pois_state.is_none() {
                                    pois_state = map.as_mut().map(|map| map.pois[..].to_owned());
                                }
                                let poi = pois_state.as_mut().map(|pois| unsafe {
                                    pois.get_unchecked_mut(idx)
                                });
                                if let Some(poi) = poi {
                                    poi.position.z = value.as_ref().and_then(|v| PackValueOf::<keys::PositionZ>::from_cell_ref(v))
                                        .map(|v| f32::from(*v.get()))
                                        .unwrap_or_default();
                                    true
                                } else {
                                    false
                                }
                            },
                            _ => false,
                        }
                    }),
                    _ => None,
                } };
                if let Some(applied) = applied {
                    modified |= applied;
                    continue
                }
                let (mut marker_attrs, mut attrs) = (None, None);
                let info = match lpath.path.namespace() {
                    MarkerIndex::NS_POI => poi.as_mut().map(|poi| match RenderAttributes::holds_attr_dyn(key) {
                        true => {
                            let attrs = attrs.insert(poi.marker_info.attrs_mut());
                            Arc::make_mut(attrs) as &mut dyn SetAttrDyn
                        },
                        false => {
                            let marker_attrs = marker_attrs.insert(poi.marker_info.marker_attrs_mut());
                            Arc::make_mut(marker_attrs) as &mut dyn SetAttrDyn
                        },
                    }),
                    MarkerIndex::NS_TRAIL => trail.as_mut().map(|trail| match RenderAttributes::holds_attr_dyn(key) {
                        true => {
                            let attrs = attrs.insert(trail.marker_info.attrs_mut());
                            Arc::make_mut(attrs) as &mut dyn SetAttrDyn
                        },
                        false => {
                            let marker_attrs = marker_attrs.insert(trail.marker_info.marker_attrs_mut());
                            Arc::make_mut(marker_attrs) as &mut dyn SetAttrDyn
                        },
                    }),
                    MarkerIndex::NS_CAT => {
                        // TODO?
                        None
                    },
                    _ => None,
                };
                let Some(info) = info else { continue };
                modified |= info.set_attr_dyn(value.unwrap_or_else(|| PackValueCell::new_empty(key)));
            }
            if modified {
                self.lpath = lpath;
                if !pois.is_empty() {
                    map_info.pois.data = Arc::from(&pois[..]);
                }
                if let (Some(pois), Some(map)) = (pois_state, map.as_mut()) {
                    map.pois.data = Arc::from(&pois[..]);
                }
                if !trails.is_empty() {
                    map_info.trails.data = Arc::from(&trails[..]);
                }
                if let (Some(trails), Some(map)) = (trails_state, map.as_mut()) {
                    map.trails.data = Arc::from(&trails[..]);
                }
                if !cats.is_empty() {
                    Arc::make_mut(&mut map_info.info).categories = cats.into_boxed_slice();
                }
                if let (Some(cats), Some(map)) = (cats_state, map.as_mut()) {
                    map.categories = Arc::from(&cats[..]);
                }
            }
            modified
        });
    }
}
impl Default for PoiStatus {
    fn default() -> Self {
        Self {
            focused: false,
            dynamic: false,
            lpath: LoadedMarkerPath::new_path(MarkerIndex::UNK),
            pending_changes: Default::default(),
        }
    }
}

/// TODO: deleteme, also does not check for validity
pub fn marker_index2loc(path: MarkerIndex) -> MarkerLoc {
    let idx = path.index();
    match path.namespace() {
        MarkerIndex::NS_CAT => (MarkerType::Category, idx as _),
        MarkerIndex::NS_TRAIL => (MarkerType::Trail, idx as _),
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

#[deprecated]
pub(crate) use crate::controller::pathing::registry::PackPath as PackLoc;

/// TODO: remove reliance on lua here
#[cfg(feature = "paths-lua")]
impl ScriptMessage {
    pub fn menu_clicked_pack(id: CategoryId, target: PackPath) -> Self {
        Self::menu_clicked_with(id, LuaExecContext::Pack(target))
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
            context: LuaExecContext::Pack(target),
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
            context: LuaExecContext::Pack(target),
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
            target,
            map_id,
            active_markers,
            append: false,
        }
        .into()
    }
}
