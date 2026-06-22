use {
    crate::{
        controller::script::{
            event::{ScriptNotification, ScriptSignal},
            lua::{LuaPlugBase, ScriptNotification0},
            menu::{PlugMenu, PlugMenuInstance},
            pathing::PackLoc,
            persistence::ScriptHostPersistence,
            PackPlugShared,
            PlugSharedData,
            PlugSharedRef,
        },
        controller::pathing::registry::SharedLoaderBox as SharedLoader,
        space::engine::SpaceEvent,
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
        collections::BTreeSet,
        path::Path,
        sync::{Arc, OnceLock},
    },
    taimi_pack::{
        attributes::{
            cell::{pack_attr, GetAttrDyn, PackKeyId, PackValueCell, SetAttrDyn},
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
                    MarkerLoc,
                    MarkerOverrides,
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

#[derive(Clone)]
pub struct LuaPackDesc {
    pub(crate) path: PackLoc,
    pub(crate) plug: LuaPlugBase,
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
    pub fn shared(&self) -> &PackPlugShared {
        unsafe { <dyn PlugSharedRef>::as_pack_unchecked(&*self.plug.shared) }
    }
    pub fn notify0(&mut self, lua: &RuntimeLua, id: ScriptNotification) -> script::Result<()> {
        let co = self.running()?;
        let yielded = co.call(ScriptNotification0(id));
        self.spun(lua, yielded)
    }
    pub fn exit(&mut self, lua: &RuntimeLua) -> anyhow::Result<()> {
        let mut signalled = false;
        while !matches!(self.received, ScriptSignal::Ended | ScriptSignal::Pending) {
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
        let co = self.running()?;
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
        match id {
            ScriptSignal::Started | ScriptSignal::Pending | ScriptSignal::Resume => (),
            ScriptSignal::Ended => {
                log::info!("{self} quit");
                self.co = None;
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
                    .enumerate()
                    .find(|&(_, k)| IdCmpRelaxed::with_ref(k.as_id()).eq_with(q))
                    .map(|(i, _)| i)
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

    pub fn pathable_tag_for((ty, idx): MarkerLoc) -> u32 {
        let tag = (ty as u8 as u32) << 28;
        tag | idx as u32
    }
    const TAG_POI: u32 = MarkerType::Poi as u8 as u32;
    const TAG_TRAIL: u32 = MarkerType::Trail as u8 as u32;
    const TAG_CAT: u32 = MarkerType::Category as u8 as u32;
    pub fn pathable_tag_from(tag: u32) -> Option<MarkerLoc> {
        let idx = tag & 0x0fffffff;
        Some(match tag >> 28 {
            Self::TAG_POI => (MarkerType::Poi, idx as usize),
            Self::TAG_TRAIL => (MarkerType::Trail, idx as usize),
            Self::TAG_CAT => (MarkerType::Category, idx as usize),
            _ => return None,
        })
    }

    pub fn path(&self) -> PackLoc {
        self.path
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
        let exists = {
            let loader = loader.blocking_lock();
            path.with_str(|p| match loader.contains_asset(p) {
                Err(e) => Err(e),
                Ok(false) => Err(script::format_err!("texture {p} not found")),
                Ok(true) => Ok(p.to_owned()),
            })
        };
        exists.map(|path| PackTexture::new(path, self.path(), loader))
    }
    type Texture = PackTexture;
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
    fn poi_by_guid<G>(&self, guid: G) -> script::Result<Option<Self::Poi>>
    where
        G: ScriptUserGuid,
    {
        guid.try_with_guid(|guid| {
            let pack = self.shared().get_pack()?;
            let poi = PackArc::from_ref(&pack)
                .poi_by_guid(guid)?
                .map(|p| PackPoi::new(p, self));
            let path = {
                let overrides = PackOverrides::shared_read(&self.shared().overrides);
                if let Some(poi) = poi {
                    let path = poi.marker.path();
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
                    self.shared().overrides.clone(),
                )
            }))
        })
        .and_then(|res| res)
    }
    fn trail_by_guid<G>(&self, guid: G) -> script::Result<Option<Self::Trail>>
    where
        G: ScriptUserGuid,
    {
        guid.try_with_guid(|guid| {
            let pack = self.shared().get_pack()?;
            let trail = PackArc::from_ref(&pack)
                .trail_by_guid(guid)?
                .map(|p| PackTrail::new(p, self));
            let path = {
                let overrides = PackOverrides::shared_read(&self.shared().overrides);
                if let Some(trail) = trail {
                    let path = trail.marker.path();
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
                    self.shared().overrides.clone(),
                )
            }))
        })
        .and_then(|res| res)
    }
    fn pathable_by_guid<G>(&self, guid: G) -> script::Result<Option<Self::Pathable>>
    where
        G: ScriptUserGuid,
    {
        guid.try_with_guid(|guid| {
            let pack = self.shared().get_pack()?;
            let poi = PackArc::from_ref(&pack).poi_by_guid(guid)?;
            let trail = match &poi {
                None => Some(PackArc::from_ref(&pack).trail_by_guid(guid)?),
                _ => None,
            };
            let path = {
                let overrides = PackOverrides::shared_read(&self.shared().overrides);
                if let Some(poi) = poi {
                    let path = (MarkerType::Poi, poi.poi_idx());
                    if !overrides.is_masked(path) && overrides.assert_guid(path, guid) {
                        return Ok(Some(unsafe {
                            let marker = PackMarkerRef::new_unchecked(pack, path);
                            PackMarker::from_marker_unchecked(
                                marker,
                                self.path(),
                                self.shared().overrides.clone(),
                            )
                        }))
                    }
                }
                let trail = match trail {
                    Some(t) => t,
                    None => PackArc::from_ref(&pack).trail_by_guid(guid)?,
                };
                if let Some(trail) = trail {
                    let path = (MarkerType::Trail, trail.trail_idx());
                    if !overrides.is_masked(path) && overrides.assert_guid(path, guid) {
                        return Ok(Some(unsafe {
                            let marker = PackMarkerRef::new_unchecked(pack, path);
                            PackMarker::from_marker_unchecked(
                                marker,
                                self.path(),
                                self.shared().overrides.clone(),
                            )
                        }))
                    }
                }
                overrides.path_by_guid(None, guid)
            };
            let Some(path) = path else { return Ok(None) };
            Ok(Some(unsafe {
                let marker = PackMarkerRef::new_unchecked(pack, path);
                PackMarker::from_marker_unchecked(marker, self.path(), self.shared().overrides.clone())
            }))
        })
        .and_then(|res| res)
    }
    fn pathable_by_tag(&self, tag: u32) -> script::Result<Option<Self::Pathable>> {
        let ctx = || script::format_err!("unrecognized tag {tag}");
        let loc = LuaPackDesc::pathable_tag_from(tag).ok_or_else(ctx)?;

        let pack = self.shared().get_pack()?;
        Ok(match PackMarkerRef::new(&pack, loc) {
            Some(m) => Some(m),
            None if PackOverrides::shared_read(&self.shared().overrides)
                .dynamic
                .contains(&loc) =>
                Some(unsafe { PackMarkerRef::new_unchecked(pack, loc) }),
            None => None,
        }
        .map(|m| unsafe {
            PackMarker::from_marker_unchecked(m, self.path(), self.shared().overrides.clone())
        }))
    }
    fn pathables_by_guid<G>(&self, guid: G) -> script::Result<Self::PathablesByGuid<'_>>
    where
        G: ScriptUserGuid,
    {
        script::script_unimpl!("PathablesByGuid")
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
        let pos = marker.get_pos()?;
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
                        (Some(parent), Some(..)) if !IdCmpRelaxed::with_ref(id).starts_with_idk(parent) => false,
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
                // TODO: clone o since we have it anyway?
                unsafe {
                    return Ok(Some(PackCategory {
                        marker: PackMarker {
                            marker: PackMarkerMut::new_from_parts(
                                PackMarkerRef::new_unchecked(pack, path),
                                self.shared().overrides.clone(),
                                Some(o.clone()),
                            ),
                            pack_path: self.path(),
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
                    .filter_map(|id| PackCategoryArc::get_category(parent.marker.pack(), id))
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
                PackArc::imp_get_category_descendents_with(parent.marker.pack(), &p.full_id)
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
        let path = {
            let mut overrides = PackOverrides::shared_write(&self.shared().overrides);
            let path = overrides.allocate_dynamic(MarkerType::Poi, &pack)?;
            let mut o = overrides
                .overrides
                .get(&path)
                .map(|o| MarkerOverrides::shared_write(o))
                .ok_or_else(|| script::format_err!("dynamic marker storage missing"))?;
            o.attrs.extend(attrs);
            path
        };

        #[cfg(deleteme)]
        SpaceEvent::ScriptCreate {
            generation: self.path.generation,
            pack_idx: self.path.index,
            marker_path: path,
        }
        .try_send();
        if let Ok(mut pending) = self.shared().pending_start.lock() {
            if pending.is_empty() {
                super::LuaMessage::InternalMarkersStarted.try_send();
            }
            pending.push(path);
        }

        Ok(unsafe {
            let marker = PackMarkerRef::new_unchecked(pack, path);
            PackPoi::from_marker_unchecked(marker, self.path(), self.shared().overrides.clone())
        })
    }
    fn create_trail<A>(&self, attrs: A) -> script::Result<Self::Trail>
    where
        A: IntoIterator<Item = PackValueCell>,
    {
        let pack = self.shared().get_pack()?;
        let path = {
            let mut overrides = PackOverrides::shared_write(&self.shared().overrides);
            let path = overrides.allocate_dynamic(MarkerType::Trail, &pack)?;
            let mut o = overrides
                .overrides
                .get(&path)
                .map(|o| MarkerOverrides::shared_write(o))
                .ok_or_else(|| script::format_err!("dynamic marker storage missing"))?;
            o.attrs.extend(attrs);
            path
        };

        #[cfg(deleteme)]
        SpaceEvent::ScriptCreate {
            generation: self.path.generation,
            pack_idx: self.path.index,
            marker_path: path,
        }
        .try_send();

        Ok(unsafe {
            let marker = PackMarkerRef::new_unchecked(pack, path);
            PackTrail::from_marker_unchecked(marker, self.path(), self.shared().overrides.clone())
        })
    }

    fn create_category<N, A>(&self, id: N, attrs: A) -> script::Result<Self::Category>
    where
        N: ScriptUserStr,
        A: IntoIterator<Item = PackValueCell>,
    {
        let pack = self.shared().get_pack()?;
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
            let path = overrides.allocate_dynamic_cat(id, &pack)?;
            let mut o = overrides
                .overrides
                .get(&path)
                .map(|o| MarkerOverrides::shared_write(o))
                .ok_or_else(|| script::format_err!("dynamic marker storage missing"))?;
            o.attrs.extend(attrs);
            if let Some(p) = parent_id {
                o.attrs.set_attr(p);
            }
            o.attrs.set_attr(name_id);
            path
        };

        #[cfg(deleteme)]
        SpaceEvent::ScriptCreate {
            generation: self.path.generation,
            pack_idx: self.path.index,
            marker_path: path,
        }
        .try_send();

        Ok(unsafe {
            let marker = PackMarkerRef::new_unchecked(pack, path);
            PackCategory::from_marker_unchecked(marker, self.path(), self.shared().overrides.clone())
        })
    }

    fn remove_poi(&self, poi: &Self::Poi) -> script::Result<()> {
        let is_dynamic = poi.poi().is_some();
        let mut o = PackOverrides::shared_write(&self.shared().overrides);
        let path = poi.marker.path();
        if is_dynamic {
            o.remove_dynamic(path);
        } else {
            o.mask_marker(path);
        }
        #[cfg(deleteme)]
        SpaceEvent::ScriptMask {
            generation: self.path.generation,
            pack_idx: self.path.index,
            marker_path: path,
        }
        .try_send();
        Ok(())
    }
    fn remove_trail(&self, trail: &Self::Trail) -> script::Result<()> {
        let is_dynamic = trail.trail().is_some();
        let mut o = PackOverrides::shared_write(&self.shared().overrides);
        let path = trail.marker.path();
        if is_dynamic {
            o.remove_dynamic(path);
        } else {
            o.mask_marker(path);
        }
        #[cfg(deleteme)]
        SpaceEvent::ScriptMask {
            generation: self.path.generation,
            pack_idx: self.path.index,
            marker_path: path,
        }
        .try_send();
        Ok(())
    }
    fn remove_category(&self, cat: &Self::Category) -> script::Result<()> {
        let is_dynamic = cat.category().is_some();
        let mut o = PackOverrides::shared_write(&self.shared().overrides);
        let path = cat.marker.path();
        if is_dynamic {
            o.remove_dynamic(path);
        } else {
            o.mask_marker(path);
        }

        #[cfg(deleteme)]
        SpaceEvent::ScriptMask {
            generation: self.path.generation,
            pack_idx: self.path.index,
            marker_path: path,
        }
        .try_send();

        Ok(())
    }
}
#[derive(Clone)]
pub struct PackTexture {
    loader: SharedLoader,
    pack_path: PackLoc,
    path: String,
    size: OnceLock<[u32; 2]>,
}
impl PackTexture {
    pub fn new(path: String, pack_path: PackLoc, loader: SharedLoader) -> Self {
        Self {
            path,
            pack_path,
            loader,
            size: Default::default(),
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
        log::debug!("TODO: Texture:GetSize");
        Ok([0, 0])
    }
}
impl TextureHandle for PackTexture {}
#[derive(Debug, Clone)]
pub struct PackRoot {
    root: PackRootCategories,
    overrides: PackOverridesShared,
}
impl PackRoot {
    pub fn with_lua_pack(desc: &LuaPackDesc) -> script::Result<Self> {
        Ok(Self {
            root: PackRootCategories::new(desc.shared().get_pack()?),
            overrides: desc.shared().overrides.clone(),
        })
    }
    /// XXX: case-sensitive, ensure callers canonicalize!
    pub fn category_state(pack: &Pack, id: &FullIdRef) -> script::Result<bool> {
        let state = crate::SETTINGS.get().map(|s| {
            let s = s.blocking_read();
            !s.disabled_paths.contains(id.as_str())
        });
        #[cfg(todo = "unnecessary")]
        let state = state.unwrap_or(|| {
            LuaPackDesc::lookup_pack_category(&self.root.pack, id).map(|cat| cat.default_toggle())
        });
        Ok(state.unwrap_or(true))
    }
    /// XXX: case-sensitive, ensure callers canonicalize!
    pub fn category_state_set(id: &FullIdRef, show_hide: Option<bool>) -> script::Result<bool> {
        let mut new_state = show_hide;
        let changed = crate::SETTINGS.get().map(|s| {
            let mut s = s.blocking_write();
            let current_state = !s.disabled_paths.contains(id.as_str());
            let state = match show_hide {
                Some(state) if current_state == state => None,
                state => Some(*new_state.insert(state.unwrap_or(!current_state))),
            };
            if let Some(state) = state {
                let disabled_paths = s.disabled_paths_mut();
                if state {
                    disabled_paths.remove(id.as_str());
                } else {
                    disabled_paths.insert(id.into());
                }
            }
        });
        #[cfg(deleteme)]
        if let Some(()) = changed {
            crate::controller::pathing::PathingEvent::RequestDisabledPaths.try_send();
        }
        // this really shouldn't fail...
        let new_state = new_state.unwrap_or(true);
        Ok(new_state)
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
        Ok(self
            .root
            .iter_root_categories()
            .any(|c| Self::category_state(&self.root.pack, &c.full_id).ok() == Some(true)))
    }
    fn show(&self) -> script::Result<()> {
        let mut res = Ok(());
        for cat in self.root.iter_root_categories() {
            if let Err(e) = Self::category_state_set(&cat.full_id, Some(true)) {
                if res.is_ok() {
                    res = Err(e)
                }
            }
        }
        res
    }
    fn hide(&self) -> script::Result<()> {
        let mut res = Ok(());
        for cat in self.root.iter_root_categories() {
            if let Err(e) = Self::category_state_set(&cat.full_id, Some(false)) {
                if res.is_ok() {
                    res = Err(e)
                }
            }
        }
        res
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
        pack_path: PackLoc,
        overrides: PackOverridesShared,
    ) -> Self {
        Self {
            marker: PackMarker::from_marker_unchecked(marker, pack_path, overrides),
        }
    }
    pub fn new(cat: PackCategoryArc, desc: &LuaPackDesc) -> Self {
        unsafe { Self::from_marker_unchecked(cat.into(), desc.path(), desc.shared().overrides.clone()) }
    }
    pub fn category(&self) -> Option<&Category> {
        self.marker
            .pack()
            .categories
            .all_categories
            .get_index(self.marker.marker_index())
            .map(|(_, c)| c)
    }
}
impl CategoryHandle for PackCategory {
    type GetCategories<'a> = Box<dyn Iterator<Item = <Self as PackHandleFactory>::Category> + 'a>;
    type GetPois<'a> = core::iter::Empty<Self::Poi>;
    type GetTrails<'a> = core::iter::Empty<Self::Trail>;

    fn get_id(&self) -> script::Result<CategoryId> {
        let o = self.marker.overrides_read();
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
                self.marker.overrides_read().map(|o| {
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
        Ok(self
            .marker
            .lookup_attr_dyn(id)
            .map(|v| v.into_owned().into_inner()))
    }
}
impl CategoryHandleMut for PackCategory {
    fn set_category_attr_dyn(&self, value: PackValueCell) -> script::Result<()> {
        let key = value.id();
        self.marker.overrides_write().attrs.set_attr_dyn(value);
        #[cfg(deleteme)]
        SpaceEvent::ScriptOverrideUpdate {
            generation: self.marker.pack_path.generation,
            pack_idx: self.marker.pack_path.index,
            marker_path: self.marker.path(),
            changed: (Some(key), Default::default()),
        }
        .try_send();
        Ok(())
    }
    fn is_visible(&self) -> script::Result<bool> {
        let id = self.get_id()?;
        Ok(PackRoot::category_state(&self.marker.pack(), &id).ok() == Some(true))
    }
    fn show(&self) -> script::Result<()> {
        let id = self.get_id()?;
        PackRoot::category_state_set(&id, Some(true)).map(drop)
    }
    fn hide(&self) -> script::Result<()> {
        let id = self.get_id()?;
        PackRoot::category_state_set(&id, Some(false)).map(drop)
    }
}
#[derive(Debug, Clone)]
pub struct PackMarker {
    marker: PackMarkerMut,
    pack_path: PackLoc,
}
impl PackMarker {
    #[inline]
    pub unsafe fn from_marker_unchecked(
        marker: PackMarkerRef,
        pack_path: PackLoc,
        overrides: PackOverridesShared,
    ) -> Self {
        Self {
            marker: PackMarkerMut::new(marker, overrides),
            pack_path,
        }
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
}
impl ops::Deref for PackMarker {
    type Target = PackMarkerMut;
    fn deref(&self) -> &Self::Target {
        &self.marker
    }
}
impl PathableHandle for PackMarker {
    #[inline]
    fn pathable_tag_index(&self) -> u32 {
        LuaPackDesc::pathable_tag_for(self.marker.path())
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

    /// TODO
    fn get_behaviour_filtered(&self) -> script::Result<bool> {
        Ok(false)
    }

    fn get_focused(&self) -> script::Result<bool> {
        let shared = crate::controller::Controller::with_sender(|s| {
            s.scripting
                .as_ref()
                .and_then(|s| s.plugs_shared.borrow().packs.get(&self.pack_path).cloned())
        })
        .flatten()
        .context("missing pack.shared")?;
        let Ok(markers) = shared.active_markers.lock() else { return Ok(false) };
        let key = LuaPackDesc::pathable_tag_for(self.path());
        Ok(markers.get(&key).map(|s| s.focused).unwrap_or(false))
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
        pack_path: PackLoc,
        overrides: PackOverridesShared,
    ) -> Self {
        Self {
            marker: PackMarker::from_marker_unchecked(marker, pack_path, overrides),
        }
    }
    pub fn new(poi: PackPoiArc, desc: &LuaPackDesc) -> Self {
        unsafe { Self::from_marker_unchecked(poi.into(), desc.path(), desc.shared().overrides.clone()) }
    }
    pub fn poi(&self) -> Option<&Poi> {
        self.marker.pack().pois.get(self.marker.marker_index())
    }
}
impl PathableHandle for PackPoi {
    #[inline]
    fn pathable_tag_index(&self) -> u32 {
        LuaPackDesc::pathable_tag_for(self.marker.path())
    }
    #[inline]
    fn pathable_tag_type(&self) -> MarkerType {
        MarkerType::Poi
    }

    fn get_marker_attr_dyn(&self, id: PackKeyId) -> script::Result<Option<PackValueCell>> {
        Ok(self
            .marker
            .lookup_attr_dyn(id)
            .map(|v| v.into_owned().into_inner()))
    }

    fn get_behaviour_filtered(&self) -> script::Result<bool> {
        Ok(false)
    }

    fn get_focused(&self) -> script::Result<bool> {
        let shared = crate::controller::Controller::with_sender(|s| {
            s.scripting
                .as_ref()
                .and_then(|s| s.plugs_shared.borrow().packs.get(&self.marker.pack_path).cloned())
        })
        .flatten()
        .context("missing pack.shared")?;
        let Ok(markers) = shared.active_markers.lock() else { return Ok(false) };
        let key = LuaPackDesc::pathable_tag_for(self.marker.path());
        Ok(markers.get(&key).map(|s| s.focused).unwrap_or(false))
    }
}
impl PathableHandleMut for PackPoi {
    fn set_marker_attr_dyn(&self, v: PackValueCell) -> script::Result<()> {
        let key = v.id();
        self.marker.overrides_write().attrs.set_attr_dyn(v);
        pack_attr! { match =id_is(key) {
            = keys::Info => if self.get_focused().unwrap_or(false) {
                if let Some(info) = self.marker.lookup_attr::<keys::Info>() {
                    if !info[..].is_empty() {
                        crate::controller::script::ui::ScriptHostUiX::new().info_notify(&info.0[..], None);
                    }
                }
            },
        } }
        #[cfg(deleteme)]
        SpaceEvent::ScriptOverrideUpdate {
            generation: self.marker.pack_path.generation,
            pack_idx: self.marker.pack_path.index,
            marker_path: self.marker.path(),
            changed: (Some(key), Default::default()),
        }
        .try_send();
        Ok(())
    }
    fn focus(&self) -> script::Result<()> {
        let shared = crate::controller::Controller::with_sender(|s| {
            s.scripting
                .as_ref()
                .and_then(|s| s.plugs_shared.borrow().packs.get(&self.marker.pack_path).cloned())
        })
        .flatten()
        .context("missing pack.shared")?;
        let Ok(mut markers) = shared.active_markers.lock() else { return Ok(()) };
        let status = markers
            .get_mut(&LuaPackDesc::pathable_tag_for(self.marker.path()))
            .context("marker not present")?;
        if !status.focused {
            if let Some(info) = self.marker.lookup_attr::<keys::Info>() {
                // TODO: if InfoRange < TriggerRange { confirm_still_inside_idk }
                crate::controller::script::ui::ScriptHostUiX::new().info_notify(&info.0[..], None);
            }
        }
        status.focused = true;
        Ok(())
    }
    fn unfocus(&self) -> script::Result<()> {
        let shared = crate::controller::Controller::with_sender(|s| {
            s.scripting
                .as_ref()
                .and_then(|s| s.plugs_shared.borrow().packs.get(&self.marker.pack_path).cloned())
        })
        .flatten()
        .context("missing pack.shared")?;
        let Ok(mut markers) = shared.active_markers.lock() else { return Ok(()) };
        let status = markers
            .get_mut(&LuaPackDesc::pathable_tag_for(self.marker.path()))
            .context("marker not present")?;
        status.focused = false;
        Ok(())
    }
}
impl PoiHandle for PackPoi {
    type Point3 = Vec3;
    type RotationEuler = Vec3;
    /// TODO: if neither marker nor overrides exist, error?
    fn get_pos(&self) -> script::Result<Self::Point3> {
        let mut pack_pos = self.poi().map(|p| p.position).unwrap_or_default();
        if let Some(o) = self.marker.overrides_read() {
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
        if let Some(o) = self.marker.overrides_read() {
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
        pack_path: PackLoc,
        overrides: PackOverridesShared,
    ) -> Self {
        Self {
            marker: PackMarker::from_marker_unchecked(marker, pack_path, overrides),
        }
    }
    pub fn new(trail: PackTrailArc, desc: &LuaPackDesc) -> Self {
        unsafe { Self::from_marker_unchecked(trail.into(), desc.path(), desc.shared().overrides.clone()) }
    }
    pub fn trail(&self) -> Option<&Trail> {
        self.marker.pack().trails.get(self.marker.marker_index())
    }
}
impl PathableHandle for PackTrail {
    #[inline]
    fn pathable_tag_index(&self) -> u32 {
        LuaPackDesc::pathable_tag_for(self.marker.path())
    }
    #[inline]
    fn pathable_tag_type(&self) -> MarkerType {
        MarkerType::Trail
    }

    fn get_marker_attr_dyn(&self, id: PackKeyId) -> script::Result<Option<PackValueCell>> {
        Ok(self
            .marker
            .lookup_attr_dyn(id)
            .map(|v| v.into_owned().into_inner()))
    }

    fn get_behaviour_filtered(&self) -> script::Result<bool> {
        Ok(false)
    }
}
impl PathableHandleMut for PackTrail {
    fn set_marker_attr_dyn(&self, v: PackValueCell) -> script::Result<()> {
        let key = v.id();
        self.marker.overrides_write().attrs.set_attr_dyn(v);
        #[cfg(deleteme)]
        SpaceEvent::ScriptOverrideUpdate {
            generation: self.marker.pack_path.generation,
            pack_idx: self.marker.pack_path.index,
            marker_path: self.marker.path(),
            changed: (Some(key), Default::default()),
        }
        .try_send();
        Ok(())
    }
}
impl TrailHandle for PackTrail {}
impl TrailHandleMut for PackTrail {
    fn set_points<P>(&self, points: P) -> script::Result<()>
    where
        //P: ScriptUserIterable,
        P: IntoIterator<Item = Vec3>,
    {
        log::warn!("TODO: Trail::SetPoints");
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
                "unknown pathable {}#{}",
                self.marker.marker_kind(),
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
