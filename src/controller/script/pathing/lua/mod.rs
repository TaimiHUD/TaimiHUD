use {
    crate::{
        controller::{
            pathing::{
                registry::{
                    LoadedMarkerPath,
                    LoadedPoiPath,
                    PackMapPath,
                    PackPath,
                    SharedLoaderBox as SharedLoader,
                },
                PathingEvent,
            },
            script::{
                event::{ScriptNotification, ScriptSignal},
                id::{PackScriptPath, ScriptIndex, ScriptPath},
                lua::{LuaMessage, LuaPlugBase, ScriptNotification0},
                menu::{PlugMenu, PlugMenuInstance},
                pathing::{marker_index2loc, marker_loc2index, marker_ty2ns, PackPlugStash},
                persistence::ScriptHostPersistence,
                PackPlugShared,
                PlugSharedData,
                PlugSharedRef,
            },
            Controller,
        },
        exports::runtime::{self as rt, textures::TextureKey},
    },
    anyhow::Context,
    core::{fmt, mem, ops},
    mlua::{
        IntoLua,
        Lua,
        MetaMethod,
        Result as LuaResult,
        UserData,
        UserDataMethods,
        UserDataRegistry,
        Value as LuaValue,
    },
    std::{
        borrow::Cow,
        cell::RefCell,
        collections::BTreeSet,
        path::Path,
        rc::Rc,
        sync::{Arc, OnceLock},
    },
    taimi_hoard::{
        lazyfmt,
        loc::{LocationRef, Locator},
    },
    taimi_meta::packs::{
        CategoryIndex,
        CategoryPath,
        MapIndex,
        MarkerId,
        MarkerIndex,
        MarkerPath,
        VisibilityFlags,
    },
    taimi_pack::{
        attributes::{
            cell::{
                pack_attr,
                AttrKeyValue,
                GetAttrDyn,
                GetAttrDynExt,
                PackKeyId,
                PackValueCell,
                SetAttrDyn,
            },
            keys::{self, SetAttr},
        },
        category::{
            id::{self, AsFullId, FullIdRef, IdCmpRelaxed},
            Category,
            CategoryFlag,
            CategoryId,
        },
        loader::{LoaderAssetReader, PackLoaderContext},
        pack::{Pack, PackCategoryArc, PackPoiArc, PackTrailArc},
        poi::Poi,
        script::{
            self,
            lua::{to_lua_error, RuntimeLua, ScriptApiTable},
            pathing::{
                event::NotifyScript,
                imp::{
                    MarkerOverrides,
                    MarkerOverridesAttrs,
                    MarkerType,
                    PackArc,
                    PackMarkerMut,
                    PackMarkerRef,
                    PackOverrides,
                    PackOverridesShared,
                    PackRootCategories,
                },
                CategoryHandle,
                CategoryHandleMut,
                InstanceTexture,
                MenuDesc,
                MenuInstance,
                PackHandle,
                PackHandleFactory,
                PackHandleMut,
                PathableHandle,
                PathableHandleFactory,
                PathableHandleMut,
                PoiHandle,
                PoiHandleMut,
                ScriptApiLookup,
                ScriptApiPack,
                ScriptApiPackAssets,
                ScriptApiSpaceQuery,
                ScriptApiUser,
                TextureHandle,
                TrailHandle,
                TrailHandleMut,
            },
            user::{IntoUserHandle, ScriptUserGuid, ScriptUserStr},
            value::Vec3,
        },
        trail::Trail,
    },
};

#[cfg(feature = "paths-interact")]
use crate::controller::pathing::{
    state::interactive::{InteractionEvent, InteractionEventAction},
    InteractMessage,
};

type SharedStash = Rc<RefCell<PackPlugStash>>;
#[derive(Clone)]
pub struct LuaPackDesc {
    pub(crate) plug: LuaPlugBase,
    /// TODO: stop cloning this, then Rc unneeded if markers can ref or something?
    pub(crate) stash: SharedStash,
}
impl ops::Deref for LuaPackDesc {
    type Target = LuaPlugBase;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.plug
    }
}
impl ops::DerefMut for LuaPackDesc {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.plug
    }
}
impl LuaPackDesc {
    #[inline]
    pub fn with_stash_mut<R, F>(&self, f: F) -> R
    where
        F: FnOnce(&mut PackPlugStash) -> R,
    {
        let mut stash = match self.stash.try_borrow_mut() {
            #[cfg(taimi_debug)]
            s => s.unwrap(),
            #[cfg(not(taimi_debug))]
            s => unsafe { s.unwrap_unchecked() },
        };
        f(&mut *stash)
    }
    #[inline]
    pub fn with_stash_ref<R, F>(&self, f: F) -> R
    where
        F: FnOnce(&PackPlugStash) -> R,
    {
        let stash = match self.stash.try_borrow() {
            #[cfg(taimi_debug)]
            s => s.unwrap(),
            #[cfg(not(taimi_debug))]
            s => unsafe { s.unwrap_unchecked() },
        };
        f(&*stash)
    }
    /// hmmmmmmmmm
    #[cfg(todo)]
    pub fn shared_arc(&self) -> &Arc<PackPlugShared> {
        unsafe { mem::transmute::<&Arc<dyn PlugSharedRef>, &Arc<PackPlugShared>>(&self.plug.shared) }
    }
    pub fn shared_arc(&self) -> Arc<PackPlugShared> {
        unsafe {
            Arc::from_raw(
                Arc::into_raw(self.plug.shared.clone()) as *const dyn core::any::Any
                    as *const PackPlugShared,
            )
        }
    }
    pub fn share_state(&self) -> PackMarkerState {
        PackMarkerState {
            shared: self.shared_arc(),
            stash: self.stash.clone(),
        }
    }
    pub fn shared(&self) -> &PackPlugShared {
        unsafe { <dyn PlugSharedRef>::as_pack_unchecked(&*self.plug.shared) }
    }
    pub fn notify0(&mut self, lua: &RuntimeLua, id: ScriptNotification) -> script::Result<()> {
        let co = self.running()?;
        match id {
            #[cfg(todo = "unnecessary")]
            ScriptNotification::PathingMapExit => {
                self.with_stash_mut(|s| {
                    s.prepare_map_exit();
                });
            },
            _ => (),
        }
        let yielded = co.call(ScriptNotification0(id));
        self.spun(lua, yielded)
    }
    pub fn exit(&mut self, lua: &RuntimeLua) -> anyhow::Result<()> {
        let mut signalled = false;
        if self.received == ScriptSignal::Restart {
            // give it one chance to clean up
            self.received = ScriptSignal::Resume;
        }
        while !matches!(
            self.received,
            ScriptSignal::Ended | ScriptSignal::Restart | ScriptSignal::Pending
        ) {
            if !signalled {
                let Ok(..) = self.running() else { return Ok(()) };
                let () = self
                    .notify0(lua, ScriptNotification::Exit)
                    .context("requesting exit")?;
                signalled = true;
            } else {
                self.notify0(lua, ScriptNotification::Nop)?;
            }
        }
        Ok(())
    }
    pub fn notify_with(
        &mut self,
        lua: &RuntimeLua,
        id: ScriptNotification,
        args: impl mlua::IntoLuaMulti,
    ) -> anyhow::Result<()> {
        let co = match (self.running(), id) {
            (Err(..), ScriptNotification::PathingTick) => return Ok(()),
            (Err(..), ScriptNotification::PathingMapExit) => {
                LuaMessage::Stop { context: self.shared().plug.path.path }.try_send();
                return Ok(())
            },
            (co, ScriptNotification::PathingMapExit) => {
                self.with_stash_mut(|s| {
                    s.prepare_map_exit();
                });
                co
            },
            (co, ..) => co,
        }?;
        let args = args.into_lua_multi(lua.lua())?;
        let yielded = co.call((LuaPlugBase::signal_with(id, args),));
        self.spun(lua, yielded)
    }
    pub fn spun(
        &mut self,
        _lua: &RuntimeLua,
        yielded: mlua::Result<Option<NotifyScript<mlua::MultiValue>>>,
    ) -> script::Result<()> {
        let yielded = match yielded {
            Ok(v) => v,
            Err(e) => {
                self.received = ScriptSignal::Resume;
                return Err(mlua::ErrorContext::with_context(e, |_| format!("polling {self}")).into())
            },
        };
        let (id, yielded) = match yielded {
            None => {
                log::warn!("{self} ended suddenly");
                self.received = ScriptSignal::Ended;
                return Ok(())
            },
            Some(ev) => {
                let id = ScriptSignal::from_repr(ev.id as _);
                let Some(id) = id else {
                    log::warn!("ignoring unsupported message {}", ev.id);
                    self.received = ScriptSignal::Resume;
                    return Ok(())
                };
                self.received = id;
                (id, ev)
            },
        };
        self.state().update_status(id);
        match id {
            ScriptSignal::Started => {
                let gameplay =
                    Controller::with_sender(|s| s.pathing.as_ref().map(|p| p.shared.gameplay.clone()))
                        .flatten();
                let gameplay = gameplay.as_ref().map(|gp| gp.borrow());
                if let Some((map_id, map_info)) = gameplay
                    .as_ref()
                    .and_then(|gp| gp.get_info_for(self.shared().path.pivot_from()))
                {
                    self.with_stash_mut(|s| {
                        let pois = map_info
                            .info
                            .pois()
                            .map(|p| MarkerPath::new_path(MarkerIndex::with_poi(p.path)));
                        let trails = map_info
                            .info
                            .trails()
                            .map(|p| MarkerPath::new_path(MarkerIndex::with_trail(p.path)));
                        s.pending_start.extend(pois.chain(trails));
                    });
                }
                drop(gameplay);
            },
            ScriptSignal::Pending | ScriptSignal::Resume => (),
            ScriptSignal::Ended => {
                log::info!("{self} quit");
                self.co = None;
                return Ok(())
            },
            ScriptSignal::Restart => {
                log::info!("{self} requested a restart");
                LuaMessage::Restart { context: self.shared().plug.path.path }.try_send();
                return Ok(())
            },
            id => {
                log::warn!("TODO: handle {id:?}: {yielded:?}");
            },
        }
        Ok(())
    }

    fn lookup_pack_category_index(pack: &Arc<Pack>, q: &str) -> Option<usize> {
        pack.categories
            .all_categories
            .get_full(q)
            .map(|(idx, _, _)| idx)
            .or_else(|| {
                pack.categories
                    .all_categories
                    .keys()
                    .position(|k| IdCmpRelaxed::with_ref(k.as_id()).eq_with(q))
            })
    }
    fn lookup_pack_category<'a>(pack: &'a Arc<Pack>, q: &str) -> Option<&'a Category> {
        Self::lookup_pack_category_index(pack, q)
            .map(move |idx| unsafe { pack.categories.all_categories.get_index(idx).unwrap_unchecked() }.1)
    }
    fn try_lookup_pack_category_arc(pack: &Arc<Pack>, q: &str) -> script::Result<PackCategoryArc> {
        Self::lookup_pack_category_arc(pack, q)
            .ok_or_else(|| script::format_err!("category {q:?} not found"))
    }
    fn lookup_pack_category_arc(pack: &Arc<Pack>, q: &str) -> Option<PackCategoryArc> {
        Self::lookup_pack_category_index(pack, q)
            .map(move |idx| unsafe { PackCategoryArc::new_unchecked(pack.clone(), idx) })
    }

    pub fn path(&self) -> PackScriptPath {
        self.plug.shared().path.path.get_pack_index()
    }

    /// shadows deref btw
    pub(crate) fn wants_poll(&self) -> bool {
        self.plug.wants_poll() | self.with_stash_ref(|s| s.is_dirty())
    }
    /// shadows deref btw
    pub(crate) fn poll_idle(&mut self, lua: &RuntimeLua) -> bool {
        let pending_start = self.with_stash_mut(|s| {
            s.process_changes_to_outbound(self.path().pivot_from(), self.shared());
            s.process_outbound_pathing();
            let to_start = (!s.pending_start.is_empty()).then(|| mem::take(&mut s.pending_start));
            to_start
        });
        if let Some(pending_start) = pending_start {
            let res = self.start_map_markers(lua, None, &mut { pending_start.into_iter() });
            let _ = rt::log::warn_ok(res);
        }
        false
    }
    /// TODO: this doesn't want to use pack, it wants a way to lookup attrs
    /// by `Locator<MarkerPath, PackKeyId>`
    /// (or just `&mut dyn Iterator<Item = &mut dyn GetAttrDyn>`?)
    #[cfg(feature = "paths-lua")]
    pub(crate) fn start_map_markers(
        &mut self,
        lua: &RuntimeLua,
        map_id: Option<MapIndex>,
        markers: &mut dyn Iterator<Item = MarkerPath>,
    ) -> anyhow::Result<()> {
        use mlua::ObjectLike;

        let pack = self.shared().get_pack()?;
        let overrides = self.shared().overrides.clone();

        // TODO: lol
        let eventloop = self
            .globals
            .get::<mlua::Table>("Taimi")?
            .get::<mlua::Table>("ctx")?
            .get::<mlua::Table>("events")?;

        if let Some(map_id) = map_id {
            // event loop may want to clean up prior to receiving new set of handlers...
            let res = self.notify_with(lua, ScriptNotification::PathingMapExit, (map_id.get(),));
            let abort = res.is_err() | matches!(self.received, ScriptSignal::Ended | ScriptSignal::Restart);
            let _ = rt::log::error_ok(res);
            // typical packs will request restart on map changes so wait for them...
            if abort {
                return Ok(())
            }
        }

        let key_once = keys::ScriptOnce::pack_key_of();
        for marker_path in markers {
            let loc = marker_index2loc(marker_path.path);
            let marker = unsafe { PackMarkerRef::new_unchecked(pack.clone(), loc) };
            let overrides = PackOverrides::shared_read(&overrides);
            let attrs_o;
            let o = overrides.overrides.get(&loc).map(MarkerOverrides::shared_read);
            let attrs = match o.as_ref() {
                None => marker.get_attrs_dyn(),
                Some(o) => {
                    attrs_o = MarkerOverridesAttrs::wrap_with_overrides(&marker, o);
                    &attrs_o as &_
                },
            };
            let script_attrs = [
                (keys::ScriptFocus::pack_key_of(), ScriptNotification::PathingFocus),
                (
                    keys::ScriptFilter::pack_key_of(),
                    ScriptNotification::PathingFilterMarker,
                ),
                (
                    keys::ScriptTrigger::pack_key_of(),
                    ScriptNotification::PathingTrigger,
                ),
                (
                    keys::ScriptTick::pack_key_of(),
                    ScriptNotification::PathingTickMarker,
                ),
                (key_once, ScriptNotification::PathingLoadMarker),
            ];
            /// ew but they're all repr(transparent) to the same type so let's skip some pain...
            unsafe fn script_cell_to_str(cell: &dyn AttrKeyValue) -> &keys::Script {
                let inconspicuous_whisling = cell as &dyn core::any::Any;
                &*(inconspicuous_whisling as *const _ as *const keys::Script)
            }
            let mut has_once = false;
            let markertag = marker_path.path.repr();
            let guid = attrs.clone_attr_dyn_of::<keys::Guid>();
            let name = lazyfmt::fmt_fn(|f| {
                if let Some(guid) = &guid {
                    write!(f, "{guid}")
                } else {
                    write!(f, "{}#{}", loc.0, loc.1)
                }
            });
            for (key, id) in script_attrs {
                let Some(attr) = attrs.get_attr_dyn(key) else { continue };
                if key == key_once {
                    has_once = true;
                }
                let attr = unsafe { script_cell_to_str(&*attr) };
                let name = format!("{name}/{key}");
                let args = lua.prepare_script_attr_args(&name, attr[..].as_bytes(), self.globals.clone());
                let Some((fname, lazyargs)) = rt::log::warn_ok(args) else { continue };
                let globals = self.globals.clone();
                let callback = self.globals.get::<mlua::Function>(&fname).or_else(|_| {
                    lua.lua().create_function(move |_lua, a: mlua::MultiValue| {
                        mlua::ErrorContext::with_context(globals.get::<mlua::Function>(&fname), |_| {
                            format!("{key} handler {}() missing", fname.display())
                        })
                        .and_then(move |f| f.call::<()>(a))
                    })
                })?;
                eventloop.call_method::<()>("RegisterMarkerAttr", (id, markertag, callback, lazyargs))?;
            }
            if has_once {
                // TODO: maybe schedule for next tick instead?
                let res = self
                    .notify_with(lua, ScriptNotification::PathingLoadMarker, (markertag,))
                    .with_context(|| format!("{name}/{key_once}"));
                let _ = rt::log::warn_ok(res);
            }
        }

        Ok(())
    }
}
impl fmt::Display for LuaPackDesc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.plug, f)
    }
}
impl ScriptApiPackAssets for LuaPackDesc {
    fn require_src<S>(&self, path: S) -> script::Result<Option<Self::RequireSrc>>
    where
        S: ScriptUserStr,
    {
        let loader = self.shared().get_loader()?;
        let mut loader = loader.blocking_lock();
        path.with_str(|path| {
            let mut res = loader
                .load_asset_dyn(path)
                .with_context(|| format!("{path} not found in pack"));
            // the ".lua" extension is optional...
            let has_ext = || {
                Path::new(path)
                    .extension()
                    .map(|ext| ext.eq_ignore_ascii_case("lua"))
                    .unwrap_or(false)
            };
            match res {
                Err(..) if !has_ext() => {
                    let fallback = loader.load_asset_dyn(&format!("{path}.lua"));
                    if let Ok(fallback) = fallback {
                        res = Ok(fallback);
                    }
                },
                _ => (),
            }
            res
        })
        .map(Some)
    }
    type RequireSrc = Box<dyn LoaderAssetReader>;

    fn open_texture<P>(&self, path: P) -> script::Result<Self::Texture>
    where
        P: ScriptUserStr,
    {
        let loader = self.shared().get_loader()?;
        let exists = path.with_str(|p| {
            if let Some(key) = self.shared().load.info.lookup_key_for_subresource(p) {
                return Ok((p.to_owned(), Some(key)))
            }
            let loader = loader.blocking_lock();
            match loader.contains_asset(p) {
                Err(e) => Err(e),
                Ok(false) => Err(script::format_err!("texture {p} not found")),
                Ok(true) => Ok((p.to_owned(), None)),
            }
        });
        exists.map(|(path, key)| PackTexture::new(path, self.path(), loader, key))
    }
    type Texture = PackTexture;
    fn open_web_texture<P>(&self, url: P) -> script::Result<Self::WebTexture>
    where
        P: ScriptUserStr,
    {
        Ok(url.with_str(|url| WebTexture::new(url.to_owned(), self.path())))
    }
    type WebTexture = WebTexture;
}
impl ScriptApiPack for LuaPackDesc {
    fn current_pack(&self) -> script::Result<Self::Pack> {
        Ok(self.clone())
    }
    type Pack = Self;

    fn current_pack_assets(&self) -> script::Result<Self::PackAssets<'_>> {
        Ok(self.clone())
    }
    type PackAssets<'a> = Self;

    fn current_pack_store(&self) -> script::Result<Self::PackStore<'_>> {
        let pack = self.shared().get_pack()?;
        let root = PackRootCategories::from_ref(&pack);
        let root_ns = root
            .primary_root()
            .or(root.iter_root_categories().next())
            .context("confusing root cat")
            .map_err(to_lua_error)?;
        let id = ScriptHostPersistence::id_for_pack(&pack.name, &root_ns.full_id);
        crate::SETTINGS
            .get()
            .cloned()
            .context("settings missing")
            .map(|settings| ScriptHostPersistence::with_owner_id(id, settings))
    }
    type PackStore<'a> = ScriptHostPersistence;

    fn current_pack_menu(&self) -> script::Result<Self::PackMenu<'_>> {
        Ok(PlugMenuInstance::new(self.shared_arc(), Some(self.clone())))
    }
    type PackMenu<'a> = PlugMenuInstance<Arc<PackPlugShared>, Self>;
    #[cfg(todo = "unnecessary")]
    type PackMenu<'a> = Self;

    fn current_pack_world(&self) -> script::Result<Self::PackWorld<'_>> {
        Ok(self.clone())
    }
    type PackWorld<'a> = Self;

    fn current_pack_space(&self) -> script::Result<Self::PackSpace<'_>> {
        Ok(self.clone())
    }
    type PackSpace<'a> = Self;
}
impl ScriptApiLookup for LuaPackDesc {
    fn poi_by_guid<G>(&self, guid: G, map_filter: Option<u32>) -> script::Result<Option<Self::Poi>>
    where
        G: ScriptUserGuid,
    {
        guid.try_with_guid(|guid| {
            let pack = self.shared().get_pack()?;
            let poi = PackArc::from_ref(&pack)
                .poi_by_guid(guid, map_filter)?
                .map(|p| PackPoi::new(p, self));
            let path = {
                let overrides = PackOverrides::shared_read(&self.shared().overrides);
                if let Some(poi) = poi {
                    let path = poi.marker.marker.path();
                    if !overrides.is_masked(path) && overrides.assert_guid(path, guid) {
                        return Ok(Some(poi))
                    }
                }
                overrides.path_by_guid(Some(MarkerType::Poi), guid)
            };
            Ok(path.map(move |path| unsafe {
                PackPoi::from_marker_unchecked(
                    PackMarkerRef::new_unchecked(pack, path),
                    self.path(),
                    self.share_state(),
                )
            }))
        })
        .and_then(|res| res)
    }
    fn trail_by_guid<G>(&self, guid: G, map_filter: Option<u32>) -> script::Result<Option<Self::Trail>>
    where
        G: ScriptUserGuid,
    {
        guid.try_with_guid(|guid| {
            let pack = self.shared().get_pack()?;
            let trail = PackArc::from_ref(&pack)
                .trail_by_guid(guid, map_filter)?
                .map(|p| PackTrail::new(p, self));
            let path = {
                let overrides = PackOverrides::shared_read(&self.shared().overrides);
                if let Some(trail) = trail {
                    let path = trail.marker.marker.path();
                    if !overrides.is_masked(path) && overrides.assert_guid(path, guid) {
                        return Ok(Some(trail))
                    }
                }
                overrides.path_by_guid(Some(MarkerType::Trail), guid)
            };
            Ok(path.map(move |path| unsafe {
                PackTrail::from_marker_unchecked(
                    PackMarkerRef::new_unchecked(pack, path),
                    self.path(),
                    self.share_state(),
                )
            }))
        })
        .and_then(|res| res)
    }
    fn pathable_by_guid<G>(
        &self,
        guid: G,
        map_filter: Option<u32>,
    ) -> script::Result<Option<Self::Pathable>>
    where
        G: ScriptUserGuid,
    {
        guid.try_with_guid(|guid| {
            let pack = self.shared().get_pack()?;
            let poi = PackArc::from_ref(&pack).poi_by_guid(guid, map_filter)?;
            let trail = match &poi {
                None => Some(PackArc::from_ref(&pack).trail_by_guid(guid, map_filter)?),
                _ => None,
            };
            let path = {
                let overrides = PackOverrides::shared_read(&self.shared().overrides);
                if let Some(poi) = poi {
                    let path = (MarkerType::Poi, poi.poi_idx());
                    if !overrides.is_masked(path) && overrides.assert_guid(path, guid) {
                        return Ok(Some(unsafe {
                            let marker = PackMarkerRef::new_unchecked(pack, path);
                            PackMarker::from_marker_unchecked(marker, self.path(), self.share_state())
                        }))
                    }
                }
                let trail = match trail {
                    Some(t) => t,
                    None => PackArc::from_ref(&pack).trail_by_guid(guid, map_filter)?,
                };
                if let Some(trail) = trail {
                    let path = (MarkerType::Trail, trail.trail_idx());
                    if !overrides.is_masked(path) && overrides.assert_guid(path, guid) {
                        return Ok(Some(unsafe {
                            let marker = PackMarkerRef::new_unchecked(pack, path);
                            PackMarker::from_marker_unchecked(marker, self.path(), self.share_state())
                        }))
                    }
                }
                overrides.path_by_guid(None, guid)
            };
            let Some(path) = path else { return Ok(None) };
            Ok(Some(unsafe {
                let marker = PackMarkerRef::new_unchecked(pack, path);
                PackMarker::from_marker_unchecked(marker, self.path(), self.share_state())
            }))
        })
        .and_then(|res| res)
    }
    fn pathable_by_tag(&self, tag: u32) -> script::Result<Option<Self::Pathable>> {
        let index = MarkerIndex::from_repr(tag);
        let loc = marker_index2loc(index);

        let pack = self.shared().get_pack()?;
        Ok(match PackMarkerRef::new(&pack, loc) {
            Some(m) => Some(m),
            None if PackOverrides::shared_read(&self.shared().overrides)
                .dynamic
                .contains(&loc) =>
                Some(unsafe { PackMarkerRef::new_unchecked(pack, loc) }),
            None => None,
        }
        .map(|m| unsafe { PackMarker::from_marker_unchecked(m, self.path(), self.share_state()) }))
    }
    fn pathables_by_guid<G>(
        &self,
        guid: G,
        map_filter: Option<u32>,
    ) -> script::Result<Self::PathablesByGuid<'_>>
    where
        G: ScriptUserGuid,
    {
        guid.try_with_guid(|guid| {
            let overrides = PackOverrides::shared_read(&self.shared().overrides);
            let pack = self.shared().get_pack()?;
            let pois = pack
                .pois
                .iter()
                .enumerate()
                .filter(|(_, poi)| {
                    map_filter
                        .map(|map_id| poi.map_id as u32 == map_id)
                        .unwrap_or(true)
                })
                .filter(|(_, poi)| {
                    poi.get_attr_of::<keys::Guid>()
                        .map(|g| &*g == guid)
                        .unwrap_or(guid.is_empty())
                });
            let trails = pack
                .trails
                .iter()
                .enumerate()
                .filter(|(_, trail)| {
                    map_filter
                        .map(|map_id| trail.map_id == Some(map_id as _))
                        .unwrap_or(true)
                })
                .filter(|(_, trail)| {
                    trail
                        .get_attr_of::<keys::Guid>()
                        .map(|g| &*g == guid)
                        .unwrap_or(guid.is_empty())
                });
            let pois = pois.filter_map(|(i, _poi)| {
                let path = (MarkerType::Poi, i);
                (!overrides.is_masked(path) && overrides.assert_guid(path, guid)).then_some(path)
            });
            let trails = trails.filter_map(|(i, _trail)| {
                let path = (MarkerType::Trail, i);
                (!overrides.is_masked(path) && overrides.assert_guid(path, guid)).then_some(path)
            });
            let markers = pois.chain(trails).map(|path| (path, false));
            let dynamic = overrides.paths_by_guid(None, guid).map(|path| (path, true));
            Ok(markers
                .chain(dynamic)
                .filter_map(|(path, dynamic)| unsafe {
                    let marker = PackMarkerRef::new_unchecked(pack.clone(), path);
                    if let (Some(target), true) = (map_filter, dynamic) {
                        let map_id = overrides
                            .overrides
                            .get(&path)
                            .map(MarkerOverrides::shared_read)
                            .and_then(|o| o.get::<keys::GameMap>().flatten().map(|map| *map.get()));
                        if map_id != Some(keys::GameMap::from(target)) {
                            return None
                        }
                    }
                    Some(PackMarker::from_marker_unchecked(
                        marker,
                        self.path(),
                        self.share_state(),
                    ))
                })
                .collect::<Vec<_>>())
        })
        .and_then(|res| res.map(|pathables| Box::new(pathables.into_iter()) as Box<_>))
    }

    fn pois_in_category<I>(&self, cat: I) -> script::Result<Self::CategoryPois<'_>>
    where
        I: ScriptUserStr,
    {
        let pack = self.shared().get_pack()?;
        let cat = cat.with_str(|q| Self::try_lookup_pack_category_arc(&pack, q));

        cat.map(move |cat| {
            // bleh the borrow upgrade...
            (0..pack.pois.len())
                .filter_map(move |i| {
                    let poi = unsafe { pack.pois.get_unchecked(i) };
                    if poi.category.as_id() != cat.full_id.as_id() {
                        return None
                    }
                    let _overrides = {
                        let overrides = PackOverrides::shared_read(&self.shared().overrides);
                        let path = (MarkerType::Poi, i);
                        if overrides.is_masked(path) {
                            return None
                        }
                        overrides
                            .overrides
                            .get(&path)
                            //.cloned()
                            .map(drop)
                    };
                    Some(unsafe { PackPoiArc::new_unchecked(pack.clone(), i) })
                })
                .map(|poi| PackPoi::new(poi, self))
        })
        .map(|i| Box::new(i) as Box<_>)
    }
    fn pois_under_category<I>(&self, cat: I) -> script::Result<Self::CategoryPoisRec<'_>>
    where
        I: ScriptUserStr,
    {
        let pack = self.shared().get_pack()?;
        let cat = cat.with_str(|q| Self::try_lookup_pack_category_arc(&pack, q));

        cat.map(move |cat| {
            // bleh the borrow upgrade...
            (0..pack.pois.len())
                .filter_map(move |i| {
                    let poi = unsafe { pack.pois.get_unchecked(i) };
                    if !poi.category.as_id().id_starts_with(cat.full_id.as_id()) {
                        return None
                    }
                    let _overrides = {
                        let overrides = PackOverrides::shared_read(&self.shared().overrides);
                        let path = (MarkerType::Poi, i);
                        if overrides.is_masked(path) {
                            return None
                        }
                        overrides
                            .overrides
                            .get(&path)
                            //.cloned()
                            .map(drop)
                    };
                    #[cfg(todo)]
                    if !overrides
                        .map(|o| {
                            MarkerOverrides::shared_read(&o)
                                .get::<keys::CategoryRef>()
                                .unset_or_matches()
                        })
                        .unwrap_or(true)
                    {
                        return None
                    }
                    Some(unsafe { PackPoiArc::new_unchecked(pack.clone(), i) })
                })
                .map(|poi| PackPoi::new(poi, self))
        })
        .map(|i| Box::new(i) as Box<_>)
    }
    fn trails_in_category<I>(&self, cat: I) -> script::Result<Self::CategoryTrails<'_>>
    where
        I: ScriptUserStr,
    {
        let pack = self.shared().get_pack()?;
        let cat = cat.with_str(|q| Self::try_lookup_pack_category_arc(&pack, q));

        cat.map(move |cat| {
            // bleh the borrow upgrade...
            (0..pack.trails.len())
                .filter_map(move |i| {
                    let trail = unsafe { pack.trails.get_unchecked(i) };
                    if trail.category.as_id() != cat.full_id.as_id() {
                        return None
                    }
                    let _overrides = {
                        let overrides = PackOverrides::shared_read(&self.shared().overrides);
                        let path = (MarkerType::Trail, i);
                        if overrides.is_masked(path) {
                            return None
                        }
                        overrides
                            .overrides
                            .get(&path)
                            //.cloned()
                            .map(drop)
                    };
                    Some(unsafe { PackTrailArc::new_unchecked(pack.clone(), i) })
                })
                .map(|trail| PackTrail::new(trail, self))
        })
        .map(|i| Box::new(i) as Box<_>)
    }
    fn trails_under_category<I>(&self, cat: I) -> script::Result<Self::CategoryTrailsRec<'_>>
    where
        I: ScriptUserStr,
    {
        let pack = self.shared().get_pack()?;
        let cat = cat.with_str(|q| Self::try_lookup_pack_category_arc(&pack, q));

        cat.map(move |cat| {
            // bleh the borrow upgrade...
            (0..pack.trails.len())
                .filter_map(move |i| {
                    let trail = unsafe { pack.trails.get_unchecked(i) };
                    if !trail.category.as_id().id_starts_with(cat.full_id.as_id()) {
                        return None
                    }
                    let _overrides = {
                        let overrides = PackOverrides::shared_read(&self.shared().overrides);
                        let path = (MarkerType::Trail, i);
                        if overrides.is_masked(path) {
                            return None
                        }
                        overrides
                            .overrides
                            .get(&path)
                            //.cloned()
                            .map(drop)
                    };
                    #[cfg(todo)]
                    if !overrides
                        .map(|o| {
                            MarkerOverrides::shared_read(&o)
                                .get::<keys::CategoryRef>()
                                .unset_or_matches()
                        })
                        .unwrap_or(true)
                    {
                        return None
                    }
                    Some(unsafe { PackTrailArc::new_unchecked(pack.clone(), i) })
                })
                .map(|trail| PackTrail::new(trail, self))
        })
        .map(|i| Box::new(i) as Box<_>)
    }

    type CategoryPois<'a> = Box<dyn Iterator<Item = <Self as PathableHandleFactory>::Poi> + 'a>;
    type CategoryPoisRec<'a> = Box<dyn Iterator<Item = <Self as PathableHandleFactory>::Poi> + 'a>;
    type CategoryTrails<'a> = Box<dyn Iterator<Item = <Self as PathableHandleFactory>::Trail> + 'a>;
    type CategoryTrailsRec<'a> = Box<dyn Iterator<Item = <Self as PathableHandleFactory>::Trail> + 'a>;
    type PathablesByGuid<'a> = Box<dyn Iterator<Item = <Self as PathableHandleFactory>::Pathable> + 'a>;
}
impl MenuDesc for &'_ LuaPackDesc {
    #[inline]
    fn get_id(&self) -> script::Result<CategoryId> {
        MenuDesc::get_id(*self)
    }
    #[inline]
    fn get_menu_attr_dyn(&self, id: PackKeyId) -> script::Result<Option<PackValueCell>> {
        MenuDesc::get_menu_attr_dyn(*self, id)
    }
}
impl MenuDesc for LuaPackDesc {
    fn get_id(&self) -> script::Result<CategoryId> {
        let pack = PackRootCategories::new(self.shared().get_pack()?);
        CategoryHandle::get_id(&pack).map(|id| id.clone())
    }
    fn get_menu_attr_dyn(&self, id: PackKeyId) -> script::Result<Option<PackValueCell>> {
        let pack = PackRootCategories::new(self.shared().get_pack()?);
        let root = pack.primary_root();
        let v = root
            .and_then(|r| r.get_attr_dyn(id))
            .map(|v| v.into_owned().into_inner());
        pack_attr! { match =id_is(id) {
            = keys::DisplayName => Ok(v
                .or_else(|| taimi_hoard::str_opt_ref(&pack.pack.name)
                .or_else(|| root.map(|r| r.display_name())).map(keys::DisplayName::from).map(PackValueCell::new_boxed))),
            = keys::NameId => v.map(Ok).unwrap_or_else(|| self.get_id().map(|id| keys::NameId::from(id.as_str())).map(PackValueCell::new_boxed)).map(Some),
            _ => Ok(v),
        } }
    }
}
#[cfg(todo)]
impl MenuHandle for LuaPackDesc {
    fn get_check_state(&self) -> script::Result<Option<bool>> {
        if self.get_checkable().ok().flatten() == Some(false) {
            return Ok(None)
        }
        self.root_category()
            .and_then(|root| CategoryHandleMut::is_visible(&root).map(Some))
    }
}
impl MenuInstance for LuaPackDesc {
    fn gen_id(&self, parent: Option<&FullIdRef>, name: Option<&id::IdNameSeg>) -> script::Result<String> {
        PlugMenuInstance::<&PackPlugShared, _>::new(self.shared(), Some(self)).gen_id(parent, name)
    }
    fn lookup_id(&self, id: &FullIdRef) -> script::Result<Option<Self::Menu>> {
        PlugMenuInstance::<Arc<PackPlugShared>, Self>::new(self.shared_arc(), Some(self.clone()))
            .lookup_id(id)
    }
    fn remove_id(&self, id: &FullIdRef, recursive: bool) -> script::Result<()> {
        PlugMenuInstance::<&PackPlugShared, _>::new(self.shared(), Some(self)).imp_remove_id(id, recursive)
    }
    fn register_id(&self, id: CategoryId) -> script::Result<Self::RegisteredMenu> {
        PlugMenuInstance::<Arc<PackPlugShared>, Self>::new(self.shared_arc(), Some(self.clone()))
            .register_id(id)
    }
    type Menu = <PlugMenuInstance<Arc<PackPlugShared>, Self> as MenuInstance>::Menu;
    type RegisteredMenu = <PlugMenuInstance<Arc<PackPlugShared>, Self> as MenuInstance>::RegisteredMenu;
}
impl ScriptApiSpaceQuery for LuaPackDesc {
    type ClosestPois = Box<dyn Iterator<Item = <Self as PathableHandleFactory>::Poi>>;
    /// TODO: lookup in interact cache
    /// TODO: ensure marker is on the same map?
    fn get_distance_to_player(&self, marker: &Self::Poi) -> script::Result<f32> {
        let mut pos = marker.get_pos()?;
        pos.y += f32::from(marker.marker.attr_dyn_or_default::<keys::HeightOffset>());

        let ml = crate::exports::runtime::mumble_link_ptr().map_err(|e| script::format_err!("{e}"))?;
        let playerpos = Vec3::from_array(unsafe { *&raw const (*ml.as_ptr()).avatar.position });
        Ok(pos.distance(playerpos))
    }
    /// TODO: nab bvh from render (or controller w/ pathcontrol)
    fn get_closest_poi_in_category<I>(&self, _id: Option<I>) -> script::Result<Option<Self::Poi>>
    where
        I: ScriptUserStr,
    {
        script::script_unimpl!("GetClosestPoiIn")
    }
    /// TODO: nab bvh from render (or controller w/ pathcontrol)
    fn get_closest_pois_in_category<I>(
        &self,
        _limit: usize,
        _id: Option<I>,
    ) -> script::Result<Self::ClosestPois>
    where
        I: ScriptUserStr,
    {
        script::script_unimpl!("GetClosestPoisIn")
    }
}
impl PackHandle for LuaPackDesc {
    type RootCategory = PackRoot;
    fn get_category<I>(&self, id: I) -> script::Result<Option<Self::Category>>
    where
        I: ScriptUserStr,
    {
        let pack = self.shared().get_pack()?;
        let found = id
            .with_str(|id| Self::lookup_pack_category_arc(&pack, id))
            .map(|c| PackCategory::new(c, self));
        if let Some(found) = found {
            return Ok(Some(found))
        }
        if found.is_none() {
            // ew...
            let overrides = PackOverrides::shared_read(&self.shared().overrides);
            let mut found_dyn = overrides
                .overrides
                .iter()
                .filter(|&(&(ty, ..), _)| ty == MarkerType::Category)
                .filter(|&(&(_, idx), o)| {
                    let o = MarkerOverrides::shared_read(o);
                    let (parent, name) = (o.get::<keys::CategoryRef>(), o.get::<keys::NameId>());
                    let cat = pack.categories.all_categories.get_index(idx);
                    let parent = match parent {
                        Some(Some(p)) => Some(Cow::Borrowed(p.get())),
                        Some(None) => None,
                        None => cat.and_then(|(_, c)| keys::GetAttr::<keys::CategoryRef>::get_attr(c)),
                    };
                    let name = match name {
                        Some(Some(p)) => Some(Cow::Borrowed(p.get())),
                        Some(None) => None,
                        None => cat.and_then(|(_, c)| keys::GetAttr::<keys::NameId>::get_attr(c)),
                    };
                    id.with_str(|id| match (&parent, &name) {
                        #[cfg(todo)]
                        (Some(parent), Some(..)) if !IdCmpRelaxed::with_ref(id).starts_idk(parent) => false,
                        (Some(parent), Some(..))
                            if !FullIdRef::from_str(id)
                                .parent()
                                .map(|p| IdCmpRelaxed::with_ref(&parent[..]) == p)
                                .unwrap_or(parent[..].is_empty()) =>
                            false,
                        (Some(..), Some(name)) =>
                            IdCmpRelaxed::with_ref(&name[..]) == FullIdRef::from_str(id).name(),
                        (None, Some(name)) => IdCmpRelaxed::with_ref(&name[..]) == id,
                        (_, None) => {
                            log::warn!("incomplete category id for {parent:?}");
                            false
                        },
                    })
                });
            if let Some((&path, o)) = found_dyn.next() {
                let shared = self.share_state();
                unsafe {
                    return Ok(Some(PackCategory {
                        marker: PackMarker {
                            marker: PackMarkerMut::new_from_parts(
                                PackMarkerRef::new_unchecked(pack, path),
                                shared.overrides.clone(),
                                Some(o.clone()),
                            ),
                            pack_path: self.path(),
                            lpath: PackMarker::empty_path_of(self.path(), MarkerType::Category),
                            shared,
                        },
                    }))
                }
            }
        }
        Ok(found)
    }

    fn root_category(&self) -> script::Result<Self::RootCategory> {
        PackRoot::with_lua_pack(self)
    }

    /// TODO: dynamic roots too!
    ///
    /// TODO: lifetimes because this isn't impl'd on a dedicated wrapper bleh...
    fn category_roots(&self) -> script::Result<Self::RootCategories<'_>> {
        let root = PackRootCategories::new(self.shared().get_pack()?);
        let roots = root
            .root_categories()
            .map(|cat| PackCategory::new(cat, self))
            .collect::<Box<[_]>>();
        let roots = IntoIterator::into_iter(roots);
        // TODO: append dynamics here!
        Ok(Box::new(roots) as Box<_>)
    }
    type RootCategories<'a> = Box<dyn Iterator<Item = Self::Category> + 'a>;
    fn get_category_children<'a>(
        &'a self,
        parent: &'a Self::Category,
    ) -> script::Result<Self::GetCategories<'a>> {
        let children = parent
            .category()
            .map(|p| {
                let o = PackOverrides::shared_read(&self.shared().overrides);
                p.child_ids()
                    .filter_map(|id| PackCategoryArc::get_category(parent.marker.marker.pack(), id))
                    .filter(|cat| !o.is_masked((MarkerType::Category, cat.category_idx())))
                    .collect::<Box<[_]>>()
            })
            .into_iter()
            .flatten()
            .map(|cat| PackCategory::new(cat, self));
        // TODO: append dynamics here!
        Ok(Box::new(children) as Box<_>)
    }
    type GetCategories<'a> = Box<dyn Iterator<Item = Self::Category> + 'a>;
    /// TODO: bitvec set instead?
    fn get_category_descendents<'a>(
        &'a self,
        parent: &'a Self::Category,
    ) -> script::Result<Self::GetCategoriesRec<'a>> {
        let desc = parent
            .category()
            .map(|p| {
                let o = PackOverrides::shared_read(&self.shared().overrides);
                let masked = o
                    .iter_masked_indices(MarkerType::Category)
                    .collect::<BTreeSet<usize>>();
                PackArc::imp_get_category_descendents_with(parent.marker.marker.pack(), &p.full_id)
                    .filter(move |cat| !masked.contains(&cat.category_idx()))
                    .map(|cat| PackCategory::new(cat, self))
            })
            .into_iter()
            .flatten();
        // TODO: append dynamics here!
        Ok(Box::new(desc) as Box<_>)
    }
    type GetCategoriesRec<'a> = Box<dyn Iterator<Item = Self::Category> + 'a>;
}
impl PackHandleMut for LuaPackDesc {
    fn create_poi<A>(&self, attrs: A) -> script::Result<Self::Poi>
    where
        A: IntoIterator<Item = PackValueCell>,
    {
        let pack = self.shared().get_pack()?;
        let mut changes = Vec::new();
        let path = {
            let path = Controller::with_sender(|s| {
                s.pathing.as_ref().and_then(|p| {
                    p.shared
                        .packs
                        .packs
                        .borrow()
                        .lookup_ref(&self.path().pivot_from())
                        .map(|p| p.info.dynamics.paths.reserve_poi())
                })
            })
            .flatten()
            .context("poi alloc")?;
            let mut overrides = PackOverrides::shared_write(&self.shared().overrides);
            let path = match () {
                #[cfg(todo)]
                _ => overrides.allocate_dynamic(MarkerType::Poi, &pack)?,
                _ => {
                    let p = marker_index2loc(MarkerIndex::with_poi(path.path));
                    overrides.allocate_dynamic_post(p);
                    p
                },
            };
            let mut o = overrides
                .overrides
                .get(&path)
                .map(|o| MarkerOverrides::shared_write(o))
                .ok_or_else(|| script::format_err!("dynamic marker storage missing"))?;
            o.attrs
                .extend(attrs.into_iter().inspect(|a| changes.push(a.id())));
            path
        };

        let poi = unsafe {
            let marker = PackMarkerRef::new_unchecked(pack, path);
            PackPoi::from_marker_unchecked(marker, self.path(), self.share_state())
        };
        self.with_stash_mut(|stash| {
            stash.record_start(marker_loc2index(path));

            stash.queue_outbound_pathing(PathingEvent::CommitMarkerLoad {
                marker_path: poi.marker.pack_path().rel(marker_loc2index(path).path),
                map_id: None,
            });
        });
        poi.marker.notify_change(changes);
        Ok(poi)
    }
    fn create_trail<A>(&self, attrs: A) -> script::Result<Self::Trail>
    where
        A: IntoIterator<Item = PackValueCell>,
    {
        let pack = self.shared().get_pack()?;
        let mut changes = Vec::new();
        let path = {
            let path = Controller::with_sender(|s| {
                s.pathing.as_ref().and_then(|p| {
                    p.shared
                        .packs
                        .packs
                        .borrow()
                        .lookup_ref(&self.path().pivot_from())
                        .map(|p| p.info.dynamics.paths.reserve_trail())
                })
            })
            .flatten()
            .context("trail alloc")?;
            let mut overrides = PackOverrides::shared_write(&self.shared().overrides);
            let path = match () {
                #[cfg(todo)]
                _ => overrides.allocate_dynamic(MarkerType::Trail, &pack)?,
                _ => {
                    let p = marker_index2loc(MarkerIndex::with_trail(path.path));
                    overrides.allocate_dynamic_post(p);
                    p
                },
            };
            let mut o = overrides
                .overrides
                .get(&path)
                .map(|o| MarkerOverrides::shared_write(o))
                .ok_or_else(|| script::format_err!("dynamic marker storage missing"))?;
            o.attrs
                .extend(attrs.into_iter().inspect(|a| changes.push(a.id())));
            path
        };

        let trail = unsafe {
            let marker = PackMarkerRef::new_unchecked(pack, path);
            PackTrail::from_marker_unchecked(marker, self.path(), self.share_state())
        };
        trail.marker.shared.with_stash_mut(|s| {
            s.record_start(marker_loc2index(path));
            s.queue_outbound_pathing(PathingEvent::CommitMarkerLoad {
                marker_path: trail.marker.pack_path().rel(marker_loc2index(path).path),
                map_id: None,
            });
        });
        trail.marker.notify_change(changes);
        Ok(trail)
    }

    fn create_category<N, A>(&self, id: N, attrs: A) -> script::Result<Self::Category>
    where
        N: ScriptUserStr,
        A: IntoIterator<Item = PackValueCell>,
    {
        let pack = self.shared().get_pack()?;
        let mut changes = Vec::new();
        let path = {
            let mut overrides = PackOverrides::shared_write(&self.shared().overrides);
            let id = id.with_str(|ids| {
                let id = ids.as_ref();
                let parent = FullIdRef::parent(id);
                let parent_ref = parent.and_then(|p| {
                    Self::lookup_pack_category(&pack, p.as_str())
                        .map(|c| (keys::CategoryRef::from(&c.full_id), p.id_len()))
                });
                let grandparent = parent.and_then(|p| p.parent());
                let parent_ref = parent_ref.or_else(|| {
                    grandparent.and_then(|gp| {
                        let mut ancestor = Some((gp, None::<&id::IdNameSeg>));
                        while let Some((p, None)) = ancestor {
                            match Self::lookup_pack_category(&pack, p.as_str()) {
                                Some(c) => {
                                    // canonicalize
                                    ancestor = Some((p, Some(c.full_id.as_ref())));
                                },
                                None => ancestor = p.parent().map(|p| (p, None)),
                            }
                        }
                        ancestor.and_then(|(p, ancestor)| {
                            ancestor.map(|a| {
                                log::warn!("TODO: create interim segments, or mark as orphan: {a}/{id}");
                                (keys::CategoryRef::from(a), p.id_len())
                            })
                        })
                    })
                });
                let id_name = match &parent_ref {
                    None => keys::NameId::from(id),
                    // TODO: likely to cause problems, so cfg out for now...
                    // (unnecessary if interim segments are created for it)
                    //#[cfg(todo)]
                    Some((_, amt)) if *amt > 0 => keys::NameId::from(
                        id.as_str()
                            .get((amt + id::SEP_STR.len())..)
                            .unwrap_or(id.name().as_str()),
                    ),
                    Some(..) => keys::NameId::from(id.name()),
                };
                CategoryId::try_with_full_id(ids).map(|id| (id, parent_ref.map(|(p, ..)| p), id_name))
            });
            let (id, parent_id, name_id) = id.ok_or_else(|| script::format_err!("invalid category id"))?;
            let path = Controller::with_sender(|s| {
                s.pathing.as_ref().and_then(|p| {
                    p.shared
                        .packs
                        .packs
                        .borrow()
                        .lookup_ref(&self.path().pivot_from())
                        .map(|p| p.info.dynamics.paths.reserve_category())
                })
            })
            .flatten()
            .context("cat alloc")?;
            let path = overrides.allocate_dynamic_cat(
                id,
                &pack,
                Some(marker_index2loc(MarkerIndex::with_category(path.path))),
            )?;
            let mut o = overrides
                .overrides
                .get(&path)
                .map(|o| MarkerOverrides::shared_write(o))
                .ok_or_else(|| script::format_err!("dynamic marker storage missing"))?;
            o.attrs
                .extend(attrs.into_iter().inspect(|a| changes.push(a.id())));
            if let Some(p) = parent_id {
                #[cfg(todo)]
                {
                    changes.push(PackKeyId::of::<keys::CategoryRef>());
                }
                o.attrs.set_attr(p);
            }
            #[cfg(todo)]
            {
                changes.push(PackKeyId::of::<keys::NameId>());
            }
            o.attrs.set_attr(name_id);
            path
        };

        let cat = unsafe {
            let marker = PackMarkerRef::new_unchecked(pack, path);
            PackCategory::from_marker_unchecked(marker, self.path(), self.share_state())
        };
        cat.marker.shared.with_stash_mut(|s| {
            s.record_start(marker_loc2index(path));
            s.queue_outbound_pathing(PathingEvent::CommitMarkerLoad {
                marker_path: cat.marker.pack_path().rel(marker_loc2index(path).path),
                map_id: None,
            })
        });
        cat.marker.notify_change(changes);
        Ok(cat)
    }

    fn remove_poi(&self, poi: &Self::Poi) -> script::Result<()> {
        let is_dynamic = poi.poi().is_some();
        let mut o = PackOverrides::shared_write(&self.shared().overrides);
        let path = poi.marker.marker.path();
        if is_dynamic {
            o.remove_dynamic(path);
        } else {
            o.mask_marker(path);
        }
        self.with_stash_mut(|s| {
            s.queue_outbound_pathing(PathingEvent::MaskMarker {
                marker: MarkerId::for_marker(poi.marker.pack_marker_index()),
            })
        });
        Ok(())
    }
    fn remove_trail(&self, trail: &Self::Trail) -> script::Result<()> {
        let is_dynamic = trail.trail().is_some();
        let mut o = PackOverrides::shared_write(&self.shared().overrides);
        let path = trail.marker.marker.path();
        if is_dynamic {
            o.remove_dynamic(path);
        } else {
            o.mask_marker(path);
        }
        self.with_stash_mut(|s| {
            s.queue_outbound_pathing(PathingEvent::MaskMarker {
                marker: MarkerId::for_marker(trail.marker.pack_marker_index()),
            })
        });
        Ok(())
    }
    fn remove_category(&self, cat: &Self::Category) -> script::Result<()> {
        let is_dynamic = cat.category().is_some();
        let mut o = PackOverrides::shared_write(&self.shared().overrides);
        let path = cat.marker.marker.path();
        if is_dynamic {
            o.remove_dynamic(path);
        } else {
            o.mask_marker(path);
        }

        self.with_stash_mut(|s| {
            s.queue_outbound_pathing(PathingEvent::MaskMarker {
                marker: MarkerId::for_marker(cat.marker.pack_marker_index()),
            })
        });

        Ok(())
    }
}
#[derive(Clone)]
pub struct PackTexture {
    loader: SharedLoader,
    pack_path: PackScriptPath,
    path: String,
    key: Option<TextureKey>,
    size: OnceLock<[u32; 2]>,
}
impl PackTexture {
    pub fn new(
        path: String,
        pack_path: PackScriptPath,
        loader: SharedLoader,
        key: Option<TextureKey>,
    ) -> Self {
        Self {
            size: Default::default(),
            path,
            key,
            pack_path,
            loader,
        }
    }

    pub fn has_size(&self) -> bool {
        self.size.get().is_some()
    }
    const SIZE_UNAVAIL: [u32; 2] = [0, 0];
    pub fn lookup_loaded_size(&self) {
        let (Some(key), None) = (&self.key, self.size.get()) else { return };
        let size = crate::TEXTURES.lookup_with(key, |slot| match slot {
            rt::textures::TextureSlot::Unavailable => Some(Self::SIZE_UNAVAIL),
            slot => slot.im_size().map(|s| s.as_::<u32>().to_array()),
        });
        if let Some(Some(size)) = size {
            let _ = self.size.get_or_init(move || size);
        }
    }
}
impl fmt::Debug for PackTexture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("PackTexture")
            .field(&self.path)
            .field(&self.pack_path)
            .finish()
    }
}
impl InstanceTexture for PackTexture {
    fn get_size(&self) -> script::Result<[u32; 2]> {
        self.lookup_loaded_size();
        let Some(size) = self.size.get() else {
            log::debug!("texture {} not loaded", self.path);
            return Ok(Self::SIZE_UNAVAIL)
        };
        Ok(*size)
    }
}
impl TextureHandle for PackTexture {}
#[derive(Clone)]
pub struct WebTexture {
    pack_path: PackScriptPath,
    url: String,
    size: OnceLock<[u32; 2]>,
}
impl WebTexture {
    pub fn new(url: String, pack_path: PackScriptPath) -> Self {
        Self {
            url,
            pack_path,
            size: Default::default(),
        }
    }
}
impl fmt::Debug for WebTexture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("WebTexture")
            .field(&self.url)
            .field(&self.pack_path)
            .finish()
    }
}
impl InstanceTexture for WebTexture {
    fn get_size(&self) -> script::Result<[u32; 2]> {
        log::debug!("TODO: WebTexture:GetSize");
        Ok([0, 0])
    }
}
impl TextureHandle for WebTexture {}
#[derive(Debug, Clone)]
pub struct PackRoot {
    root: PackRootCategories,
    shared: PackMarkerState,
}
impl PackRoot {
    pub fn with_lua_pack(desc: &LuaPackDesc) -> script::Result<Self> {
        let shared = desc.share_state();
        Ok(Self {
            root: PackRootCategories::new(shared.get_pack()?),
            shared,
        })
    }
}
impl CategoryHandle for PackRoot {
    type GetCategories<'a> = Box<dyn Iterator<Item = <Self as PackHandleFactory>::Category> + 'a>;
    type GetPois<'a> = core::iter::Empty<Self::Poi>;
    type GetTrails<'a> = core::iter::Empty<Self::Trail>;

    fn get_id(&self) -> script::Result<CategoryId> {
        self.root.get_id()
    }
    fn is_root(&self) -> script::Result<bool> {
        self.root.is_root()
    }
    fn is_dynamic(&self) -> script::Result<bool> {
        Ok(false)
    }

    fn get_category_attr_dyn(&self, id: PackKeyId) -> script::Result<Option<PackValueCell>> {
        self.root.get_category_attr_dyn(id)
    }
}
impl CategoryHandleMut for PackRoot {
    fn is_visible(&self) -> script::Result<bool> {
        Ok(self.root.iter_root_categories().any(|c| {
            let idx = self
                .root
                .pack
                .categories
                .all_categories
                .get_index_of(c.full_id.as_id());
            idx.and_then(|idx| {
                PackCategory::configured_cat_state(
                    self.shared.path.pivot_from().rel(idx as CategoryIndex),
                    None,
                )
                .ok()
            }) == Some(true)
        }))
    }
    fn show(&self) -> script::Result<()> {
        let mut res = None;
        for cat in self.root.iter_root_categories() {
            res = Some(Ok(()));
            PathingEvent::CategoryEnableById(
                self.shared.path.pivot_from(),
                cat.full_id.to_id_box().into_owned(),
                Some(true),
            )
            .try_send();
        }
        res.unwrap_or_else(|| Err(script::format_err!("missing root")))
    }
    fn hide(&self) -> script::Result<()> {
        let mut res = None;
        for cat in self.root.iter_root_categories() {
            res = Some(Ok(()));
            PathingEvent::CategoryEnableById(
                self.shared.path.pivot_from(),
                cat.full_id.to_id_box().into_owned(),
                Some(true),
            )
            .try_send();
        }
        res.unwrap_or_else(|| Err(script::format_err!("missing root")))
    }
}
#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct PackCategory {
    marker: PackMarker,
}
impl PackCategory {
    pub unsafe fn from_ref_unchecked(marker: &PackMarker) -> &Self {
        mem::transmute(marker)
    }
    pub unsafe fn from_marker_unchecked(
        marker: PackMarkerRef,
        pack_path: PackScriptPath,
        shared: PackMarkerState,
    ) -> Self {
        Self {
            marker: PackMarker::from_marker_unchecked(marker, pack_path, shared),
        }
    }
    pub fn new(cat: PackCategoryArc, desc: &LuaPackDesc) -> Self {
        unsafe { Self::from_marker_unchecked(cat.into(), desc.path(), desc.share_state()) }
    }
    pub fn category(&self) -> Option<&Category> {
        self.marker
            .marker
            .pack()
            .categories
            .all_categories
            .get_index(self.marker.marker_index().path.index() as usize)
            .map(|(_, c)| c)
    }
    /// TODO: is this effective or configured state?
    fn configured_cat_state(
        path: CategoryPath<PackPath>,
        defaulttoggle: Option<bool>,
    ) -> script::Result<bool> {
        let cat_path: CategoryPath = path.unscope();
        let pathing = Controller::with_sender(|s| s.pathing.as_ref().map(|p| p.shared.clone()))
            .flatten()
            .context("controller offline")?;
        let (defaulttoggle, config) = {
            let packs = pathing.packs.packs.borrow();
            let pack = packs.lookup_ref(&path.root).context("pack unloaded")?;
            let defaulttoggle = defaulttoggle
                .or_else(|| {
                    pack.info
                        .category_info()
                        .map(|(c, _)| !c.disabled.contains(cat_path))
                })
                .unwrap_or(true);
            (defaulttoggle, pack.config.clone())
        };
        let vis_dev = {
            let config = config.borrow();
            config.config.visibility_deviation_for(path.unscope())
        };
        let config_vis = defaulttoggle ^ vis_dev.contains(VisibilityFlags::TOGGLE);
        Ok(config_vis)
    }
    fn set_cat_state(path: CategoryPath<PackPath>, state: Option<bool>) {
        PathingEvent::CategoryEnableSet(path.root, path.unscope(), state).try_send();
    }
}
impl CategoryHandle for PackCategory {
    type GetCategories<'a> = Box<dyn Iterator<Item = <Self as PackHandleFactory>::Category> + 'a>;
    type GetPois<'a> = core::iter::Empty<Self::Poi>;
    type GetTrails<'a> = core::iter::Empty<Self::Trail>;

    fn get_id(&self) -> script::Result<CategoryId> {
        let o = self.marker.marker.overrides_read();
        let (parent, name) = (
            o.as_ref().and_then(|o| o.get::<keys::CategoryRef>()),
            o.as_ref().and_then(|o| o.get::<keys::NameId>()),
        );
        let parent = match parent {
            Some(Some(i)) if i[..].is_empty() => Some(None),
            i => i,
        };
        let name = match name {
            Some(Some(i)) if i[..].is_empty() => Some(None),
            i => i,
        };
        match (self.category(), parent, name) {
            | (_, Some(None), Some(Some(name))) | (None, None, Some(Some(name))) =>
                CategoryId::try_with_full_id(&name[..]),
            (_, Some(Some(parent)), Some(Some(name))) =>
                CategoryId::try_with_full_id(format!("{parent}{}{name}", id::SEP_STR)),
            (None, ..) => return Err(script::format_err!("incomplete dynamic category")),
            (Some(..), _, Some(None)) => return Err(script::format_err!("incomplete category override")),
            (Some(cat), None | Some(None), _) => Some(cat.full_id.clone()),
            (Some(cat), Some(Some(parent)), None) =>
                CategoryId::try_with_full_id(format!("{parent}{}{}", id::SEP_STR, cat.id())),
        }
        .ok_or_else(|| script::format_err!("{}", CategoryId::<id::IdNameBox>::WITH_FULL_ID_ERR))
    }
    /// XXX: overriding parent ref of an existing cat to None
    /// to elevate it to root could be made valid but seems insane...
    fn is_root(&self) -> script::Result<bool> {
        let root = self
            .category()
            .map(|c| c.flags.is(CategoryFlag::Root))
            .or_else(|| {
                self.marker.marker.overrides_read().map(|o| {
                    let parent = o.get::<keys::CategoryRef>();
                    matches!(parent, None | Some(None))
                })
            })
            .unwrap_or(false);
        Ok(root)
    }
    fn is_dynamic(&self) -> script::Result<bool> {
        Ok(self.category().is_none())
    }

    fn get_category_attr_dyn(&self, id: PackKeyId) -> script::Result<Option<PackValueCell>> {
        #[cfg(todo)]
        if let Some(v) = self.marker.marker.lookup_override_dyn(id) {
            return Ok(v.map(|v| v.into_inner()))
        }
        Ok(self
            .marker
            .marker
            .lookup_attr_dyn(id)
            .map(|v| v.into_owned().into_inner()))
    }
}
impl CategoryHandleMut for PackCategory {
    fn set_category_attr_dyn(&self, value: PackValueCell) -> script::Result<()> {
        let key = value.id();
        self.marker.marker.overrides_write().attrs.set_attr_dyn(value);
        self.marker.notify_change([key]);
        Ok(())
    }
    fn is_visible(&self) -> script::Result<bool> {
        let defaulttoggle = self
            .marker
            .marker
            .lookup_override::<keys::DefaultToggle>()
            .map(|v| bool::from(v.unwrap_or_default()));
        Self::configured_cat_state(
            self.marker
                .pack_path()
                .rel(self.marker.marker_index().path.index_category_unchecked()),
            defaulttoggle,
        )
    }
    fn hide(&self) -> script::Result<()> {
        Self::set_cat_state(
            self.marker
                .pack_path()
                .rel(self.marker.marker_index().path.index_category_unchecked()),
            Some(false),
        );
        Ok(())
    }
    fn show(&self) -> script::Result<()> {
        Self::set_cat_state(
            self.marker
                .pack_path()
                .rel(self.marker.marker_index().path.index_category_unchecked()),
            Some(true),
        );
        Ok(())
    }
}
#[derive(Clone)]
pub struct PackMarkerState {
    shared: Arc<PackPlugShared>,
    stash: SharedStash,
}
impl PackMarkerState {
    #[inline]
    pub fn with_stash_mut<R, F>(&self, f: F) -> R
    where
        F: FnOnce(&mut PackPlugStash) -> R,
    {
        let mut stash = match self.stash.try_borrow_mut() {
            #[cfg(taimi_debug)]
            s => s.unwrap(),
            #[cfg(not(taimi_debug))]
            s => unsafe { s.unwrap_unchecked() },
        };
        f(&mut *stash)
    }
    #[inline]
    pub fn with_stash_ref<R, F>(&self, f: F) -> R
    where
        F: FnOnce(&PackPlugStash) -> R,
    {
        let stash = match self.stash.try_borrow() {
            #[cfg(taimi_debug)]
            s => s.unwrap(),
            #[cfg(not(taimi_debug))]
            s => unsafe { s.unwrap_unchecked() },
        };
        f(&*stash)
    }
}
impl fmt::Debug for PackMarkerState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("PackMarkerState").field(&self.shared).finish()
    }
}
impl ops::Deref for PackMarkerState {
    type Target = PackPlugShared;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.shared
    }
}
#[derive(Debug, Clone)]
pub struct PackMarker {
    marker: PackMarkerMut,
    shared: PackMarkerState,
    pack_path: PackScriptPath,
    lpath: Locator<PackMapPath, LoadedMarkerPath>,
}
impl PackMarker {
    #[inline]
    pub unsafe fn from_marker_unchecked(
        marker: PackMarkerRef,
        pack_path: PackScriptPath,
        shared: PackMarkerState,
    ) -> Self {
        Self {
            lpath: Self::empty_path_of(pack_path, marker.marker_kind()),
            marker: PackMarkerMut::new(marker, shared.overrides.clone()),
            shared,
            pack_path,
        }
    }
    #[inline]
    pub fn pack_path(&self) -> PackPath {
        self.lpath.root.root
    }
    #[inline]
    pub fn map_path(&self) -> Option<PackMapPath> {
        (self.lpath.root.path != MapIndex::MAX).then_some(self.lpath.root)
    }
    #[inline]
    pub fn is_loaded(&self) -> bool {
        match () {
            #[cfg(todo)]
            _ => self.lpath.root.path != MapIndex::MAX,
            _ => self.lpath.path.path.index() != MarkerIndex::INDEX_INVALID,
        }
    }
    pub fn lindex(&self) -> Option<LoadedMarkerPath> {
        self.is_loaded().then_some(self.lpath.path)
    }
    pub fn lpath(&self) -> Option<Locator<PackMapPath, LoadedMarkerPath>> {
        self.is_loaded().then_some(self.lpath)
    }
    pub fn marker_index(&self) -> MarkerPath {
        marker_loc2index(self.marker.path())
    }
    pub fn pack_marker_index(&self) -> MarkerPath<PackPath> {
        self.marker_index().pivot(self.pack_path())
    }
    fn empty_path_of(pack_path: PackScriptPath, ty: MarkerType) -> Locator<PackMapPath, LoadedMarkerPath> {
        let ns = marker_ty2ns(ty);
        Locator::with_parts(
            PackMapPath::with_parts(pack_path.pivot_from(), MapIndex::MAX),
            Locator::new_path(MarkerIndex::new_invalid(ns)),
        )
    }
    #[inline]
    pub fn as_poi(&self) -> Option<&PackPoi> {
        match self.marker.marker_kind() {
            MarkerType::Poi => Some(unsafe { PackPoi::from_ref_unchecked(self) }),
            _ => None,
        }
    }
    #[inline]
    pub fn as_trail(&self) -> Option<&PackTrail> {
        match self.marker.marker_kind() {
            MarkerType::Trail => Some(unsafe { PackTrail::from_ref_unchecked(self) }),
            _ => None,
        }
    }
    #[inline]
    pub fn as_category(&self) -> Option<&PackCategory> {
        match self.marker.marker_kind() {
            MarkerType::Category => Some(unsafe { PackCategory::from_ref_unchecked(self) }),
            _ => None,
        }
    }

    #[inline]
    pub fn notify_change(&self, keys: impl IntoIterator<Item = PackKeyId>) {
        self.shared
            .with_stash_mut(|s| s.record_changes(self.marker_index(), keys));
    }
}
impl GetAttrDyn for PackMarker {
    /// TODO
    fn has_attr_dyn(&self, key: PackKeyId) -> bool {
        self.marker.get_attrs_dyn().has_attr_dyn(key) || self.marker.lookup_override_dyn(key).is_some()
    }
    /// TODO
    fn get_attr_dyn_ref(&self, key: PackKeyId) -> Option<&dyn AttrKeyValue> {
        self.marker.lookup_attr_dyn(key).and_then(|v| match v {
            Cow::Borrowed(v) => Some(v),
            _ => None,
        })
    }
    fn get_attr_dyn(&self, key: PackKeyId) -> Option<Cow<'_, dyn AttrKeyValue>> {
        self.marker.lookup_attr_dyn(key)
    }
    #[inline]
    fn holds_attr_dyn(_: PackKeyId) -> bool {
        true
    }
    #[cfg(todo)]
    fn iter_attrs_dyn(&self) -> impl Iterator<Item = Cow<'_, dyn AttrKeyValue>> + '_ {}
}
impl PathableHandle for PackMarker {
    #[inline]
    fn pathable_tag_index(&self) -> u32 {
        self.marker_index().path.repr()
    }
    #[inline]
    fn pathable_tag_type(&self) -> MarkerType {
        self.marker.marker_kind()
    }

    fn get_marker_attr_dyn(&self, id: PackKeyId) -> script::Result<Option<PackValueCell>> {
        if let Some(m) = self.as_poi() {
            m.get_marker_attr_dyn(id)
        } else if let Some(m) = self.as_trail() {
            m.get_marker_attr_dyn(id)
        } else if let Some(m) = self.as_category() {
            m.get_category_attr_dyn(id)
        } else {
            Err(script::format_err!(
                "unknown pathable {}#{}",
                self.marker.marker_kind(),
                self.marker.marker_index()
            ))
        }
    }

    #[cfg(feature = "paths-filter")]
    fn get_behaviour_filtered(&self) -> script::Result<bool> {
        let pack_path = self.pack_path();
        let marker_path = self.marker_index();
        let marker_id = MarkerId::for_marker(pack_path.rel(marker_path));
        let hidden = crate::controller::Controller::with_sender(|s| {
            let gameplay = s.pathing.as_ref().map(|s| s.shared.gameplay.borrow());
            let map = gameplay.as_ref().and_then(|gp| {
                gp.map_id.and_then(|map_id| {
                    gp.for_ref(pack_path.rel(map_id))
                        .and_then(|(info, map)| map.map(|m| (map_id, info, m)))
                })
            });
            map.map(|(map_id, info, map)| {
                let map_path = pack_path.rel(map_id);
                let lpath = info.marker_index(marker_path);
                let mut mids = [marker_id, MarkerId::EMPTY, MarkerId::EMPTY];
                let mut amt = 1;
                let mut write = mids.iter_mut().skip(1).inspect(|_| amt += 1);
                if let Some(lpath) = lpath {
                    if let Some(dest) = write.next() {
                        *dest = MarkerId::for_marker(map_path.rel(lpath));
                    }
                }
                let guid = self.marker.lookup_override::<keys::Guid>().unwrap_or_else(|| {
                    lpath.and_then(|lpath| match lpath.path.namespace() {
                        MarkerIndex::NS_POI => info
                            .poi_guid_by_index(LoadedPoiPath::new_path(lpath.path.index_poi_unchecked()))
                            .cloned(),
                        MarkerIndex::NS_TRAIL => self.shared.get_pack().ok().and_then(|pack| {
                            pack.trails
                                .get(self.marker.marker_index())
                                .and_then(|t| keys::Guid::from_ref(&t.guid).or_empty().cloned())
                        }),
                        _ => None,
                    })
                });
                if let Some(guid) = guid {
                    if let Some(dest) = write.next() {
                        *dest = MarkerId::with_uuid(guid.into());
                    }
                }
                map.is_hidden(unsafe { mids.get_unchecked(..amt) })
            })
        })
        .flatten()
        .unwrap_or(false);
        Ok(hidden)
    }
    #[cfg(not(feature = "paths-filter"))]
    fn get_behaviour_filtered(&self) -> script::Result<bool> {
        Ok(false)
    }

    #[cfg(feature = "paths-interact")]
    fn get_focused(&self) -> script::Result<bool> {
        let marker_path = self.marker_index();
        let poi_path = match marker_path.path.namespace() {
            MarkerIndex::NS_POI => Locator::new_path(marker_path.path.index_poi_unchecked()),
            _ => return Ok(false),
        };
        let poi_path = self.pack_path().rel(poi_path);
        let focused = crate::controller::Controller::with_sender(|s| {
            s.pathing
                .as_ref()
                .map(|s| s.shared.interact.nearby.borrow().pois.contains_key(&poi_path))
        })
        .flatten()
        .unwrap_or(false);
        Ok(focused)
    }
    #[cfg(not(feature = "paths-interact"))]
    fn get_focused(&self) -> script::Result<bool> {
        log::debug!("interaction focus disabled");
        Ok(false)
    }
}
impl PathableHandleMut for PackMarker {
    fn set_marker_attr_dyn(&self, v: PackValueCell) -> script::Result<()> {
        if let Some(m) = self.as_poi() {
            m.set_marker_attr_dyn(v)
        } else if let Some(m) = self.as_trail() {
            m.set_marker_attr_dyn(v)
        } else if let Some(m) = self.as_category() {
            m.set_category_attr_dyn(v)
        } else {
            Err(script::format_err!(
                "unknown pathable {}#{}",
                self.marker.marker_kind(),
                self.marker.marker_index()
            ))
        }
    }
}
#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct PackPoi {
    marker: PackMarker,
}
impl PackPoi {
    pub unsafe fn from_ref_unchecked(marker: &PackMarker) -> &Self {
        mem::transmute(marker)
    }
    pub unsafe fn from_marker_unchecked(
        marker: PackMarkerRef,
        pack_path: PackScriptPath,
        shared: PackMarkerState,
    ) -> Self {
        Self {
            marker: PackMarker::from_marker_unchecked(marker, pack_path, shared),
        }
    }
    pub fn new(poi: PackPoiArc, desc: &LuaPackDesc) -> Self {
        unsafe { Self::from_marker_unchecked(poi.into(), desc.path(), desc.share_state()) }
    }
    pub fn poi(&self) -> Option<&Poi> {
        self.marker
            .marker
            .pack()
            .pois
            .get(self.marker.marker_index().path.index() as usize)
    }
}
impl PathableHandle for PackPoi {
    #[inline]
    fn pathable_tag_index(&self) -> u32 {
        self.marker.pathable_tag_index()
    }
    #[inline]
    fn pathable_tag_type(&self) -> MarkerType {
        MarkerType::Poi
    }

    fn get_marker_attr_dyn(&self, id: PackKeyId) -> script::Result<Option<PackValueCell>> {
        Ok(self
            .marker
            .marker
            .lookup_attr_dyn(id)
            .map(|v| v.into_owned().into_inner()))
    }

    #[inline]
    fn get_behaviour_filtered(&self) -> script::Result<bool> {
        self.marker.get_behaviour_filtered()
    }
    #[inline]
    fn get_focused(&self) -> script::Result<bool> {
        self.marker.get_focused()
    }
}
impl PathableHandleMut for PackPoi {
    fn set_marker_attr_dyn(&self, v: PackValueCell) -> script::Result<()> {
        let key = v.id();
        self.marker.marker.overrides_write().attrs.set_attr_dyn(v);
        self.marker.notify_change([key]);
        Ok(())
    }
    #[cfg(feature = "paths-interact")]
    fn focus(&self) -> script::Result<()> {
        let marker_path = self.marker.marker_index();
        let poi_path = match marker_path.path.namespace() {
            MarkerIndex::NS_POI => Locator::new_path(marker_path.path.index_poi_unchecked()),
            _ => return Err(script::format_err!("don't know how to focus {marker_path}")),
        };
        let pack_path = self.marker.pack_path();
        let lpath = crate::controller::Controller::with_sender(|s| {
            let gp = s
                .pathing
                .as_ref()
                .map(|s| s.shared.gameplay.borrow())
                .and_then(|gp| gp.map_id.map(|map_id| (map_id, gp)));

            gp.and_then(|(map_id, gp)| {
                let map_path = pack_path.rel(map_id);
                gp.get_info(map_path)
                    .and_then(|info| info.poi_index(poi_path))
                    .map(|lpath| map_path.rel(lpath))
            })
        })
        .flatten()
        .context("marker unloaded")?;
        self.marker.shared.with_stash_mut(|s| {
            s.queue_outbound_pathing(InteractMessage::Event(InteractionEvent::Nearby {
                path: poi_path,
                loaded_path: lpath.map_path(|p| p.path),
            }))
        });
        Ok(())
    }
    #[cfg(feature = "paths-interact")]
    fn unfocus(&self) -> script::Result<()> {
        let marker_path = marker_loc2index(self.marker.marker.path());
        let poi_path = match marker_path.path.namespace() {
            MarkerIndex::NS_POI => Locator::new_path(marker_path.path.index_poi_unchecked()),
            _ => return Ok(()),
        };
        let pack_path = self.marker.pack_path();
        let lpath = crate::controller::Controller::with_sender(|s| {
            let gp = s
                .pathing
                .as_ref()
                .map(|s| s.shared.gameplay.borrow())
                .and_then(|gp| gp.map_id.map(|map_id| (map_id, gp)));

            gp.and_then(|(map_id, gp)| {
                let map_path = pack_path.rel(map_id);
                gp.get_info(map_path)
                    .and_then(|info| info.poi_index(poi_path))
                    .map(|lpath| map_path.rel(lpath))
            })
        })
        .flatten()
        .context("marker unloaded")?;
        self.marker.shared.with_stash_mut(|s| {
            s.queue_outbound_pathing(InteractMessage::Event(InteractionEvent::Gone {
                path: poi_path,
                loaded_path: lpath.map_path(|p| p.path),
            }))
        });
        Ok(())
    }
    #[cfg(feature = "paths-interact")]
    fn interact(&self, auto: bool) -> script::Result<()> {
        let marker_path = marker_loc2index(self.marker.marker.path());
        let poi_path = match marker_path.path.namespace() {
            MarkerIndex::NS_POI => Locator::new_path(marker_path.path.index_poi_unchecked()),
            _ => return Ok(()),
        };
        let pack_path = self.marker.pack_path();
        let lpath = crate::controller::Controller::with_sender(|s| {
            let gp = s
                .pathing
                .as_ref()
                .map(|s| s.shared.gameplay.borrow())
                .and_then(|gp| gp.map_id.map(|map_id| (map_id, gp)));

            gp.and_then(|(map_id, gp)| {
                let map_path = pack_path.rel(map_id);
                gp.get_info(map_path)
                    .and_then(|info| info.poi_index(poi_path))
                    .map(|lpath| map_path.rel(lpath))
            })
        })
        .flatten()
        .context("marker unloaded")?;
        let action = match auto {
            true => InteractionEventAction::AutoTrigger,
            false => InteractionEventAction::Interact,
        };
        self.marker.shared.with_stash_mut(|s| {
            s.queue_outbound_pathing(InteractMessage::Event(InteractionEvent::Interact {
                path: poi_path,
                loaded_path: lpath.map_path(|p| p.path),
                action,
            }))
        });
        Ok(())
    }
}
impl PoiHandle for PackPoi {
    type Point3 = Vec3;
    type RotationEuler = Vec3;
    /// TODO: if neither marker nor overrides exist, error?
    fn get_pos(&self) -> script::Result<Self::Point3> {
        let mut pack_pos = self.poi().map(|p| p.position).unwrap_or_default();
        if let Some(o) = self.marker.marker.overrides_read() {
            if let Some(x) = o.get::<keys::PositionX>() {
                pack_pos.x = x.map(|x| f32::from(*x.get())).unwrap_or_default();
            }
            if let Some(y) = o.get::<keys::PositionY>() {
                pack_pos.y = y.map(|y| f32::from(*y.get())).unwrap_or_default();
            }
            if let Some(z) = o.get::<keys::PositionZ>() {
                pack_pos.z = z.map(|z| f32::from(*z.get())).unwrap_or_default();
            }
        }
        Ok(pack_pos.into())
    }
    fn get_rot_euler(&self) -> script::Result<Self::RotationEuler> {
        let mut pack_rot = self
            .poi()
            .and_then(|p| p.attributes.get_poi().and_then(|p| p.rotate));
        if let Some(o) = self.marker.marker.overrides_read() {
            if let Some(x) = o.get::<keys::RotateX>() {
                pack_rot.get_or_insert_default().x = x.map(|x| f32::from(*x.get())).unwrap_or_default();
            }
            if let Some(y) = o.get::<keys::RotateY>() {
                pack_rot.get_or_insert_default().y = y.map(|y| f32::from(*y.get())).unwrap_or_default();
            }
            if let Some(z) = o.get::<keys::RotateZ>() {
                pack_rot.get_or_insert_default().z = z.map(|z| f32::from(*z.get())).unwrap_or_default();
            }
        }
        Ok(pack_rot.unwrap_or_default().into())
    }
}
impl PoiHandleMut for PackPoi {
    fn set_pos<P>(&self, pos: P) -> script::Result<()>
    where
        P: script::pathing::InstanceVec3,
    {
        let [x, y, z] = pos.get3();
        let mut res = self.set_marker_attr_dyn(PackValueCell::new_boxed(keys::PositionX::from(x)));
        if let Err(e) = self.set_marker_attr_dyn(PackValueCell::new_boxed(keys::PositionY::from(y))) {
            if res.is_ok() {
                res = Err(e)
            }
        }
        if let Err(e) = self.set_marker_attr_dyn(PackValueCell::new_boxed(keys::PositionZ::from(z))) {
            if res.is_ok() {
                res = Err(e)
            }
        }
        res
    }
    fn set_rot_euler<P>(&self, rot: P) -> script::Result<()>
    where
        P: script::pathing::InstanceVec3,
    {
        let [x, y, z] = rot.get3();
        let mut res = self.set_marker_attr_dyn(PackValueCell::new_boxed(keys::RotateX::from(x)));
        if let Err(e) = self.set_marker_attr_dyn(PackValueCell::new_boxed(keys::RotateY::from(y))) {
            if res.is_ok() {
                res = Err(e)
            }
        }
        if let Err(e) = self.set_marker_attr_dyn(PackValueCell::new_boxed(keys::RotateZ::from(z))) {
            if res.is_ok() {
                res = Err(e)
            }
        }
        res
    }
}
#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct PackTrail {
    marker: PackMarker,
}
impl PackTrail {
    pub unsafe fn from_ref_unchecked(marker: &PackMarker) -> &Self {
        mem::transmute(marker)
    }
    pub unsafe fn from_marker_unchecked(
        marker: PackMarkerRef,
        pack_path: PackScriptPath,
        shared: PackMarkerState,
    ) -> Self {
        Self {
            marker: PackMarker::from_marker_unchecked(marker, pack_path, shared),
        }
    }
    pub fn new(trail: PackTrailArc, desc: &LuaPackDesc) -> Self {
        unsafe { Self::from_marker_unchecked(trail.into(), desc.path(), desc.share_state()) }
    }
    pub fn trail(&self) -> Option<&Trail> {
        self.marker
            .marker
            .pack()
            .trails
            .get(self.marker.marker_index().path.index() as usize)
    }
}
impl PathableHandle for PackTrail {
    #[inline]
    fn pathable_tag_index(&self) -> u32 {
        self.marker.pathable_tag_index()
    }
    #[inline]
    fn pathable_tag_type(&self) -> MarkerType {
        MarkerType::Trail
    }

    fn get_marker_attr_dyn(&self, id: PackKeyId) -> script::Result<Option<PackValueCell>> {
        Ok(self
            .marker
            .marker
            .lookup_attr_dyn(id)
            .map(|v| v.into_owned().into_inner()))
    }

    fn get_behaviour_filtered(&self) -> script::Result<bool> {
        self.marker.get_behaviour_filtered()
    }
}
impl PathableHandleMut for PackTrail {
    fn set_marker_attr_dyn(&self, v: PackValueCell) -> script::Result<()> {
        let key = v.id();
        self.marker.marker.overrides_write().attrs.set_attr_dyn(v);
        self.marker.notify_change([key]);
        Ok(())
    }
    fn focus(&self) -> script::Result<()> {
        self.unfocus()
    }
    fn unfocus(&self) -> script::Result<()> {
        script::bail!("how2focus trail?")
    }
    fn interact(&self, _: bool) -> script::Result<()> {
        script::bail!("how2interact trail?")
    }
}
impl TrailHandle for PackTrail {}
impl TrailHandleMut for PackTrail {
    #[cfg(feature = "space")]
    fn set_points<P>(&self, points: P) -> script::Result<()>
    where
        //P: ScriptUserIterable,
        P: IntoIterator<Item = Vec3>,
    {
        use {
            crate::controller::{
                pathing::{
                    registry::PackLoader,
                    shared::{LoadReport, LoadedTrailSection},
                    state::LoadedTrail,
                    PathingEvent,
                },
                Controller,
            },
            glamour::Point3,
            taimi_hoard::vec::vec32_eq,
            taimi_pack::trail::{TrailData, TrailSection},
        };
        let path = match self.marker.marker_index() {
            p if p.path.namespace() == MarkerIndex::NS_TRAIL =>
                Locator::new_path(p.path.trail_index_unchecked()),
            p => return Err(script::format_err!("{p} not trail")),
        };
        let pack_path = self.marker.pack_path();
        let lpath = Controller::with_sender(|s| {
            s.pathing.as_ref().and_then(|p| {
                p.shared
                    .gameplay
                    .borrow()
                    .get_info_for(pack_path)
                    .and_then(|(map_path, map_info)| map_info.trail_index(path).map(|lp| map_path.rel(lp)))
            })
        })
        .flatten()
        .context("trail unloaded")?;
        let mut sections = Vec::new();
        let mut section = Vec::<Point3<f32>>::new();
        for point in points.into_iter().map(Point3::<f32>::from) {
            if vec32_eq(point, Point3::ZERO) {
                // section delimiter
                sections.push(mem::take(&mut section));
                continue
            }
            section.push(point.into());
        }
        sections.retain(|s| !s.is_empty());
        let trl = TrailData {
            header: Default::default(),
            sections: sections
                .into_iter()
                .filter_map(|s| match s.is_empty() {
                    true => None,
                    false => Some(TrailSection::with_points(s)),
                })
                .collect(),
        };
        #[cfg(todo)]
        let attrs = self;
        let attrs = &self.marker as &dyn GetAttrDyn;
        /*let scale = match self {
            #[cfg(todo)]
            s => s.get_marker_attr_dyn(keys::TrailScale::pack_key_of()),
            s => s.marker.marker.lookup_attr::<keys::TrailScale>().map(|s|
                s.into_owned()
            ).unwrap_or_default().into(),
        };
        let is_wall = match self {
            #[cfg(todo)]
            s => s.get_marker_attr_dyn(keys::IsWall::pack_key_of()),
            s => s.marker.marker.lookup_attr::<keys::IsWall>().map(|s|
                s.into_owned()
            ).unwrap_or_default().into(),
        };*/
        let params = crate::SETTINGS
            .get()
            .and_then(|s| {
                s.blocking_read()
                    .pathing
                    .as_ref()
                    .map(|p| PackLoader::trail_params_for(p))
            })
            .unwrap_or_default();
        let y_off = 0.0;
        let geo = LoadedTrail::load_with_data(&trl, &params, attrs, y_off);
        let res = LoadReport::TrailGeometry {
            section_info: Some(
                trl.sections
                    .iter()
                    .map(LoadedTrailSection::with_section)
                    .collect(),
            ),
            path: lpath,
            geometry: Ok(geo),
        };
        self.marker
            .shared
            .with_stash_mut(|s| s.queue_outbound_pathing(PathingEvent::ReportResourceLoaded(res)));

        Ok(())
    }
}
impl PackHandleFactory for LuaPackDesc {
    type Guid = keys::Guid;
    type Category = PackCategory;
    type Behaviour = script::Unimplemented;
}
impl PathableHandleFactory for LuaPackDesc {
    type Poi = PackPoi;
    type Trail = PackTrail;
    type Pathable = PackMarker;
}
impl PackHandleFactory for PackRoot {
    type Guid = <LuaPackDesc as PackHandleFactory>::Guid;
    type Category = <LuaPackDesc as PackHandleFactory>::Category;
    type Behaviour = <LuaPackDesc as PackHandleFactory>::Behaviour;
}
impl PathableHandleFactory for PackRoot {
    type Poi = <LuaPackDesc as PathableHandleFactory>::Poi;
    type Trail = <LuaPackDesc as PathableHandleFactory>::Trail;
    type Pathable = <LuaPackDesc as PathableHandleFactory>::Pathable;
}
impl PackHandleFactory for PackCategory {
    type Guid = <LuaPackDesc as PackHandleFactory>::Guid;
    type Category = <LuaPackDesc as PackHandleFactory>::Category;
    type Behaviour = <LuaPackDesc as PackHandleFactory>::Behaviour;
}
impl PathableHandleFactory for PackCategory {
    type Poi = <LuaPackDesc as PathableHandleFactory>::Poi;
    type Trail = <LuaPackDesc as PathableHandleFactory>::Trail;
    type Pathable = <LuaPackDesc as PathableHandleFactory>::Pathable;
}
impl PackHandleFactory for PackMarker {
    type Guid = <LuaPackDesc as PackHandleFactory>::Guid;
    type Category = <LuaPackDesc as PackHandleFactory>::Category;
    type Behaviour = <LuaPackDesc as PackHandleFactory>::Behaviour;
}
impl PathableHandleFactory for PackMarker {
    type Poi = <LuaPackDesc as PathableHandleFactory>::Poi;
    type Trail = <LuaPackDesc as PathableHandleFactory>::Trail;
    type Pathable = <LuaPackDesc as PathableHandleFactory>::Pathable;
}
impl PackHandleFactory for PackPoi {
    type Guid = <LuaPackDesc as PackHandleFactory>::Guid;
    type Category = <LuaPackDesc as PackHandleFactory>::Category;
    type Behaviour = <LuaPackDesc as PackHandleFactory>::Behaviour;
}
impl PathableHandleFactory for PackPoi {
    type Poi = <LuaPackDesc as PathableHandleFactory>::Poi;
    type Trail = <LuaPackDesc as PathableHandleFactory>::Trail;
    type Pathable = <LuaPackDesc as PathableHandleFactory>::Pathable;
}
impl PackHandleFactory for PackTrail {
    type Guid = <LuaPackDesc as PackHandleFactory>::Guid;
    type Category = <LuaPackDesc as PackHandleFactory>::Category;
    type Behaviour = <LuaPackDesc as PackHandleFactory>::Behaviour;
}
impl PathableHandleFactory for PackTrail {
    type Poi = <LuaPackDesc as PathableHandleFactory>::Poi;
    type Trail = <LuaPackDesc as PathableHandleFactory>::Trail;
    type Pathable = <LuaPackDesc as PathableHandleFactory>::Pathable;
}
impl IntoUserHandle for LuaPackDesc {
    type IntoHandle = Self;
    fn into_handle(self) -> Self::IntoHandle {
        self
    }
    fn clone_into_handle(&self) -> Self::IntoHandle {
        self.clone()
    }
    fn to_lua_handle(&self, lua: &Lua) -> LuaResult<LuaValue> {
        RuntimeLua::new_api_pack(self.clone_into_handle()).into_lua(lua)
    }
}
impl IntoUserHandle for PackTexture {
    type IntoHandle = Self;
    fn into_handle(self) -> Self::IntoHandle {
        self
    }
    fn clone_into_handle(&self) -> Self::IntoHandle {
        self.clone()
    }
    fn to_lua_handle(&self, lua: &Lua) -> LuaResult<LuaValue> {
        self.clone_into_handle().into_lua(lua)
    }
}
impl UserData for PackTexture {
    fn register(reg: &mut UserDataRegistry<Self>) {
        ScriptApiTable::<_, Self>::register_texture(reg);
        reg.add_meta_method(MetaMethod::ToString.name(), |lua, this, ()| {
            mlua::IntoLua::into_lua(&this.path[..], lua)
        });
    }
}
impl IntoUserHandle for WebTexture {
    type IntoHandle = Self;
    fn into_handle(self) -> Self::IntoHandle {
        self
    }
    fn clone_into_handle(&self) -> Self::IntoHandle {
        self.clone()
    }
    fn to_lua_handle(&self, lua: &Lua) -> LuaResult<LuaValue> {
        self.clone_into_handle().into_lua(lua)
    }
}
impl UserData for WebTexture {
    fn register(reg: &mut UserDataRegistry<Self>) {
        ScriptApiTable::<_, Self>::register_texture(reg);
        reg.add_meta_method(MetaMethod::ToString.name(), |lua, this, ()| {
            mlua::IntoLua::into_lua(&this.url[..], lua)
        });
    }
}
impl IntoUserHandle for PackRoot {
    type IntoHandle = Self;
    fn into_handle(self) -> Self::IntoHandle {
        self
    }
    fn clone_into_handle(&self) -> Self::IntoHandle {
        self.clone()
    }
    fn to_lua_handle(&self, lua: &Lua) -> LuaResult<LuaValue> {
        RuntimeLua::new_api_root_category(self.clone_into_handle()).into_lua(lua)
    }
}
impl IntoUserHandle for ScriptHostPersistence {
    type IntoHandle = Self;
    fn into_handle(self) -> Self::IntoHandle {
        self
    }
    fn clone_into_handle(&self) -> Self::IntoHandle {
        self.clone()
    }
    fn to_lua_handle(&self, lua: &Lua) -> LuaResult<LuaValue> {
        RuntimeLua::new_api_persist_store(self.clone_into_handle()).into_lua(lua)
    }
}
impl IntoUserHandle for PackMarker {
    type IntoHandle = Self;
    fn into_handle(self) -> Self::IntoHandle {
        self
    }
    fn clone_into_handle(&self) -> Self::IntoHandle {
        self.clone()
    }
    fn to_lua_handle(&self, lua: &Lua) -> LuaResult<LuaValue> {
        if let Some(m) = self.as_poi() {
            m.to_lua_handle(lua)
        } else if let Some(m) = self.as_trail() {
            m.to_lua_handle(lua)
        } else if let Some(m) = self.as_category() {
            m.to_lua_handle(lua)
        } else {
            Err(to_lua_error(script::format_err!(
                "unknown pathable {}",
                self.marker.marker_index()
            )))
        }
    }
}
impl IntoUserHandle for PackCategory {
    type IntoHandle = Self;
    fn into_handle(self) -> Self::IntoHandle {
        self
    }
    fn clone_into_handle(&self) -> Self::IntoHandle {
        self.clone()
    }
    fn to_lua_handle(&self, lua: &Lua) -> LuaResult<LuaValue> {
        RuntimeLua::new_instance_category_mut(self.clone_into_handle()).into_lua(lua)
    }
}
impl IntoUserHandle for PackPoi {
    type IntoHandle = Self;
    fn into_handle(self) -> Self::IntoHandle {
        self
    }
    fn clone_into_handle(&self) -> Self::IntoHandle {
        self.clone()
    }
    fn to_lua_handle(&self, lua: &Lua) -> LuaResult<LuaValue> {
        RuntimeLua::new_instance_poi_mut(self.clone_into_handle()).into_lua(lua)
    }
}
impl IntoUserHandle for PackTrail {
    type IntoHandle = Self;
    fn into_handle(self) -> Self::IntoHandle {
        self
    }
    fn clone_into_handle(&self) -> Self::IntoHandle {
        self.clone()
    }
    fn to_lua_handle(&self, lua: &Lua) -> LuaResult<LuaValue> {
        RuntimeLua::new_instance_trail_mut(self.clone_into_handle()).into_lua(lua)
    }
}
impl<S> IntoUserHandle for PlugMenu<S>
where
    S: AsRef<PlugSharedData> + Clone + 'static,
{
    type IntoHandle = Self;
    fn into_handle(self) -> Self::IntoHandle {
        self
    }
    fn clone_into_handle(&self) -> Self::IntoHandle {
        self.clone()
    }
    fn to_lua_handle(&self, lua: &Lua) -> LuaResult<LuaValue> {
        RuntimeLua::new_instance_menu_mut(self.clone_into_handle()).into_lua(lua)
    }
}
impl fmt::Debug for LuaPackDesc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("PackInfo")
            .field(self.shared())
            //.field(&self.plug)
            .finish()
    }
}
