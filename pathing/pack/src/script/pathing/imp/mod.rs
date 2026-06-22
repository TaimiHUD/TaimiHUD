use {
    crate::{
        attributes::keys::Guid,
        category::{id, Category, CategoryId},
        pack::{Pack, PackCategoryArc, PackPoiArc, PackTrailArc},
        script::{
            format_err,
            pathing::{
                BehaviourHandle,
                CategoryHandle,
                CategoryHandleMut,
                GuidHandle,
                InstanceGuid,
                InstanceTexture,
                InstanceVec3,
                MapFilterArg,
                MapID,
                MenuDesc,
                MenuHandle,
                MenuHandleMut,
                MenuInstance,
                PackHandle,
                PackHandleFactory,
                PackHandleMut,
                PathableHandle,
                PathableHandleFactory,
                PathableHandleMut,
                PoiHandle,
                PoiHandleMut,
                ScriptApiEvent,
                ScriptApiLookup,
                ScriptApiMumble,
                ScriptApiPack,
                ScriptApiPackAssets,
                ScriptApiSpaceQuery,
                ScriptApiStorage,
                ScriptApiUser,
                ScriptApiVersion,
                ScriptApiWorld,
                TextureHandle,
                TrailHandle,
                TrailHandleMut,
            },
            user::{IntoUserHandle, ScriptUserGuid, ScriptUserStr},
            value::Vec3,
            Result,
            Unimplemented,
        },
    },
    anyhow::{anyhow, Context},
    core::{
        borrow::Borrow,
        hash::{Hash, Hasher},
        iter,
        mem,
        ops,
    },
    std::{borrow::Cow, io, sync::Arc},
};

mod r#mut;

pub use self::r#mut::*;

/// always empty
impl ScriptApiStorage for () {
    fn insert_string<K, N, V>(&self, _: K, _: Option<N>, _: V) -> Result<Option<String>>
    where
        K: ScriptUserStr,
        N: ScriptUserStr,
        V: ScriptUserStr,
    {
        Ok(None)
    }
    fn get_string<K, N>(&self, _: K, _: Option<N>) -> Result<Option<String>>
    where
        K: ScriptUserStr,
        N: ScriptUserStr,
    {
        Ok(None)
    }
    fn remove_key<K, N>(&self, _: K, _: Option<N>) -> Result<()>
    where
        K: ScriptUserStr,
        N: ScriptUserStr,
    {
        Ok(())
    }
}
impl ScriptApiPack for Unimplemented {
    fn current_pack(&self) -> Result<Self::Pack> {
        Ok(*self)
    }
    type Pack = Self;

    fn current_pack_assets<'a>(&'a self) -> Result<Self::PackAssets<'a>> {
        Ok(*self)
    }
    type PackAssets<'a> = Self;

    fn current_pack_store<'a>(&'a self) -> Result<Self::PackStore<'a>> {
        Ok(*self)
    }
    type PackStore<'a> = Self;

    fn current_pack_world<'a>(&'a self) -> Result<Self::PackWorld<'a>> {
        Ok(*self)
    }
    type PackWorld<'a> = Self;

    fn current_pack_space<'a>(&'a self) -> Result<Self::PackSpace<'a>> {
        Ok(*self)
    }
    type PackSpace<'a> = Self;

    fn current_pack_menu<'a>(&'a self) -> Result<Self::PackMenu<'a>> {
        Ok(*self)
    }
    type PackMenu<'a> = Self;
}
impl ScriptApiStorage for Unimplemented {}
impl ScriptApiWorld for Unimplemented {}
impl ScriptApiPackAssets for Unimplemented {
    type RequireSrc = io::Empty;
    fn open_texture<P>(&self, _path: P) -> Result<Self::Texture>
    where
        P: ScriptUserStr,
    {
        Ok(*self)
    }
    type Texture = Self;
}
impl ScriptApiVersion for Unimplemented {
    fn taimi_version(&self) -> Cow<'_, str> {
        Cow::Borrowed("0.0.1")
    }
}
impl ScriptApiLookup for Unimplemented {
    type PathablesByGuid<'a>
        = iter::Empty<<Self as PathableHandleFactory>::Pathable>
    where
        Self: 'a;
    type CategoryPois<'a>
        = iter::Empty<<Self as PathableHandleFactory>::Poi>
    where
        Self: 'a;
    type CategoryTrails<'a>
        = iter::Empty<<Self as PathableHandleFactory>::Trail>
    where
        Self: 'a;
    type CategoryPoisRec<'a>
        = iter::Empty<<Self as PathableHandleFactory>::Poi>
    where
        Self: 'a;
    type CategoryTrailsRec<'a>
        = iter::Empty<<Self as PathableHandleFactory>::Trail>
    where
        Self: 'a;
}
impl ScriptApiSpaceQuery for Unimplemented {
    type ClosestPois = iter::Empty<<Self as PathableHandleFactory>::Poi>;
}
impl ScriptApiMumble for Unimplemented {}
impl PathableHandleFactory for Unimplemented {
    type Trail = Unimplemented;
    type Poi = Unimplemented;
    /// TODO: `Box<dyn>`
    type Pathable = Unimplemented;
}
impl PackHandleFactory for Unimplemented {
    type Category = Unimplemented;
    type Behaviour = Unimplemented;
    type Guid = Guid;
}
impl CategoryHandle for Unimplemented {
    type GetCategories<'a> = iter::Empty<<Self as PackHandleFactory>::Category>;
    type GetTrails<'a> = iter::Empty<<Self as PathableHandleFactory>::Trail>;
    type GetPois<'a> = iter::Empty<<Self as PathableHandleFactory>::Poi>;
}
impl CategoryHandleMut for Unimplemented {}
impl PackHandle for Unimplemented {
    type RootCategory = Unimplemented;
    type RootCategories<'a> = iter::Empty<<Self as PackHandleFactory>::Category>;
    type GetCategories<'a> = iter::Empty<<Self as PackHandleFactory>::Category>;
    type GetCategoriesRec<'a> = iter::Empty<<Self as PackHandleFactory>::Category>;
}
impl PackHandleMut for Unimplemented {}
impl BehaviourHandle for Unimplemented {}
impl TrailHandle for Unimplemented {}
impl PoiHandle for Unimplemented {
    type Point3 = Vec3;
    type RotationEuler = Vec3;
}
impl PoiHandleMut for Unimplemented {}
impl PathableHandle for Unimplemented {
    fn pathable_tag_index(&self) -> u32 {
        u32::MAX
    }
    #[inline]
    fn pathable_tag_type(&self) -> MarkerType {
        MarkerType::Poi
    }
    fn get_behaviour_filtered(&self) -> Result<bool> {
        Ok(false)
    }
}
impl PathableHandleMut for Unimplemented {}
impl TextureHandle for Unimplemented {}
impl InstanceTexture for Unimplemented {}
impl ScriptApiUser for Unimplemented {}
impl ScriptApiEvent for Unimplemented {
    type SignalNames = Box<dyn Iterator<Item = (Cow<'static, str>, u32)>>;

    fn all_signals(&self) -> Self::SignalNames {
        let all = IntoIterator::into_iter([
            ("Started", 100u32),
            ("Ended", 101),
            ("Pending", 102),
            ("Resume", 103),
        ])
        .map(|(k, v)| (Cow::Borrowed(k), v));
        Box::new(all) as Box<_>
    }

    fn all_notifications(&self) -> Self::SignalNames {
        let all = IntoIterator::into_iter([
            ("Exit", 1u32),
            ("PathingTick", 2),
            ("MenuClick", 3),
            ("PathingMapExit", 4),
            ("GameplayKeybind", 5),
            ("PathingFocus", 34),
            ("PathingTrigger", 35),
            ("PathingTickMarker", 36),
            ("PathingLoadMarker", 37),
            ("PathingFilterMarker", 38),
        ])
        .map(|(k, v)| (Cow::Borrowed(k), v));
        Box::new(all) as Box<_>
    }
}
impl MenuDesc for Unimplemented {}
impl MenuHandle for Unimplemented {}
impl MenuHandleMut for Unimplemented {}
impl MenuInstance for Unimplemented {
    type Menu = Self;
    type RegisteredMenu = Self;
}

impl GuidHandle for Guid {}
impl InstanceGuid for Guid {
    fn from_base_64<G: ScriptUserStr>(guid: G) -> Result<Self> {
        guid.with_str(|s| s.parse()).context("expected base64 GUID")
    }
    #[inline]
    fn to_base_64(&self) -> Result<String> {
        Ok(self.to_string())
    }
}
impl IntoUserHandle for Unimplemented {
    type IntoHandle = Self;
    fn into_handle(self) -> Self::IntoHandle {
        self
    }
    fn clone_into_handle(&self) -> Self::IntoHandle {
        *self
    }
    #[cfg(feature = "script-lua")]
    fn to_lua_handle(&self, lua: &mlua::Lua) -> mlua::Result<mlua::Value> {
        mlua::IntoLua::into_lua(mlua::Nil, lua)
    }
}

/// can't impl for Arc since external type :<
#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct PackArc {
    pub pack: Arc<Pack>,
}
impl PackArc {
    #[inline(always)]
    pub fn new(pack: Arc<Pack>) -> Self {
        Self { pack }
    }
    #[inline(always)]
    pub fn from_ref(pack: &Arc<Pack>) -> &Self {
        unsafe { mem::transmute(pack) }
    }
}
impl ops::Deref for PackArc {
    type Target = Arc<Pack>;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.pack
    }
}
impl PartialEq for PackArc {
    fn eq(&self, rhs: &Self) -> bool {
        Arc::ptr_eq(&self.pack, &rhs.pack)
    }
}
impl Eq for PackArc {}
impl Hash for PackArc {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.pack).hash(state)
    }
}
impl Borrow<Arc<Pack>> for PackArc {
    #[inline(always)]
    fn borrow(&self) -> &Arc<Pack> {
        &self.pack
    }
}

impl PackHandleFactory for PackArc {
    type Category = PackCategoryArc;
    type Behaviour = Unimplemented;
    type Guid = Guid;
}
impl PathableHandleFactory for PackArc {
    type Trail = PackTrailArc;
    type Poi = PackPoiArc;
    /// TODO: `Box<dyn>`
    type Pathable = Unimplemented;
}
impl PackArc {
    pub fn category_descends_from<C, I>(cat: C, parent_id: I) -> bool
    where
        C: id::AsFullId,
        I: AsRef<id::FullIdRef>,
    {
        #[cfg(todo = "unnecessary")]
        let parent_id = IdCmpRelaxed::with_ref(parent_id.as_ref());
        cat.id_starts_with(parent_id)
    }
    pub fn imp_get_category_descendents_with<'a, C>(
        pack: &'a Arc<Pack>,
        parent_id: C,
    ) -> impl Iterator<Item = PackCategoryArc> + 'a
    where
        C: AsRef<id::FullIdRef> + 'a,
    {
        let parent_id_len = id::AsFullId::id_len(parent_id.as_ref());
        pack.categories
            .all_categories
            .iter()
            .enumerate()
            .filter(move |(_, (_, cat))| {
                cat.full_id.len() > parent_id_len
                    && Self::category_descends_from(&cat.full_id, parent_id.as_ref())
            })
            .map(move |(idx, ..)| unsafe { PackCategoryArc::new_unchecked(pack.clone(), idx) })
    }
    pub fn imp_get_category_descendents(
        parent: &PackCategoryArc,
    ) -> impl Iterator<Item = PackCategoryArc> + '_ {
        Self::imp_get_category_descendents_with(parent.pack(), &parent.full_id)
    }
}

impl PackHandle for PackArc {
    fn get_category<I>(&self, id: I) -> Result<Option<Self::Category>>
    where
        I: ScriptUserStr,
    {
        Ok(id.with_str(|id| {
            #[cfg(todo)]
            let id = IdCmpRelaxed::with_ref(FullIdRef::from_str(id));
            PackCategoryArc::get_category(self, id)
        }))
    }
    type RootCategory = PackRootCategories;
    fn root_category(&self) -> Result<Self::RootCategory> {
        Ok(PackRootCategories::new(self.pack.clone()))
    }
    fn category_roots(&self) -> Result<Self::RootCategories<'_>> {
        let it = PackRootCategories::from_ref(&self.pack).root_categories();
        Ok(Box::new(it) as Box<_>)
    }
    type RootCategories<'a> = Box<dyn Iterator<Item = <Self as PackHandleFactory>::Category> + 'a>;
    fn get_category_children<'a>(&'a self, parent: &'a Self::Category) -> Result<Self::GetCategories<'a>> {
        let it = parent.children_cloned();
        Ok(Box::new(it) as Box<_>)
    }
    type GetCategories<'a> = Box<dyn Iterator<Item = <Self as PackHandleFactory>::Category> + 'a>;
    fn get_category_descendents<'a>(
        &'a self,
        parent: &'a Self::Category,
    ) -> Result<Self::GetCategories<'a>> {
        let it = Self::imp_get_category_descendents(parent);
        Ok(Box::new(it) as Box<_>)
    }
    type GetCategoriesRec<'a> = Box<dyn Iterator<Item = <Self as PackHandleFactory>::Category> + 'a>;
}
impl ScriptApiWorld for PackArc {}
impl ScriptApiLookup for PackArc {
    fn poi_by_guid<G>(&self, guid: G, map_filter: MapFilterArg) -> Result<Option<Self::Poi>>
    where
        G: ScriptUserGuid,
    {
        guid.try_with_guid(|guid| {
            PackPoiArc::find_poi(self, |poi, _| {
                if let Some(target) = map_filter {
                    if poi.map_id != target as _ {
                        return false
                    }
                }
                &poi.guid == guid
            })
        })
    }
    fn trail_by_guid<G>(&self, guid: G, map_filter: MapFilterArg) -> Result<Option<Self::Trail>>
    where
        G: ScriptUserGuid,
    {
        guid.try_with_guid(|guid| {
            PackTrailArc::find_trail(self, |trail, _| {
                if let Some(target) = map_filter {
                    if trail.map_id != Some(target as _) {
                        return false
                    }
                }
                &trail.guid == guid
            })
        })
    }
    type PathablesByGuid<'a>
        = iter::Empty<<Self as PathableHandleFactory>::Pathable>
    where
        Self: 'a;
    type CategoryPois<'a>
        = iter::Empty<<Self as PathableHandleFactory>::Poi>
    where
        Self: 'a;
    type CategoryTrails<'a>
        = iter::Empty<<Self as PathableHandleFactory>::Trail>
    where
        Self: 'a;
    type CategoryPoisRec<'a>
        = iter::Empty<<Self as PathableHandleFactory>::Poi>
    where
        Self: 'a;
    type CategoryTrailsRec<'a>
        = iter::Empty<<Self as PathableHandleFactory>::Trail>
    where
        Self: 'a;
}
impl ScriptApiSpaceQuery for PackArc {
    type ClosestPois = iter::Empty<<Self as PathableHandleFactory>::Poi>;
}
impl PackHandleMut for PackArc {}
impl ScriptApiPack for PackArc {
    fn current_pack(&self) -> Result<Self::Pack> {
        Ok(self.clone())
    }
    type Pack = Self;

    fn current_pack_world<'a>(&'a self) -> Result<Self::PackWorld<'a>> {
        Ok(self.clone())
    }
    type PackWorld<'a> = Self;

    fn current_pack_space<'a>(&'a self) -> Result<Self::PackSpace<'a>> {
        Ok(self.clone())
    }
    type PackSpace<'a> = Self;

    type PackAssets<'a> = Unimplemented;
    type PackMenu<'a> = Unimplemented;
    type PackStore<'a> = Unimplemented;
}
impl IntoUserHandle for PackArc {
    type IntoHandle = PackArc;
    #[inline]
    fn into_handle(self) -> Self::IntoHandle {
        self
    }
    #[inline]
    fn clone_into_handle(&self) -> Self::IntoHandle {
        self.clone().into_handle()
    }
    #[cfg(feature = "script-lua")]
    fn to_lua_handle(&self, lua: &mlua::Lua) -> mlua::Result<mlua::Value> {
        mlua::IntoLua::into_lua(self.clone_into_handle(), lua)
    }
}

#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct PackRootCategories {
    pub pack: Arc<Pack>,
}
impl PackRootCategories {
    #[inline(always)]
    pub fn new(pack: Arc<Pack>) -> Self {
        Self { pack }
    }
    #[inline(always)]
    pub fn from_ref(pack: &Arc<Pack>) -> &Self {
        unsafe { mem::transmute(pack) }
    }
    pub fn root_categories(&self) -> impl Iterator<Item = PackCategoryArc> + '_ {
        self.pack
            .categories
            .root_categories
            .iter()
            .filter_map(move |cat| PackCategoryArc::get_category(&self.pack, cat))
    }
    #[inline]
    pub fn iter_root_categories(&self) -> impl Iterator<Item = &Category> + '_ {
        self.pack.categories.root_categories()
    }
    pub fn primary_root(&self) -> Option<&Category> {
        self.iter_root_categories()
            .max_by_key(|c| {
                (
                    !c.is_hidden(),
                    !c.is_separator(),
                    c.display_name.is_some(),
                    c.default_toggle(),
                )
            })
            .or(self.iter_root_categories().next())
    }
}
impl PartialEq for PackRootCategories {
    fn eq(&self, rhs: &Self) -> bool {
        Arc::ptr_eq(&self.pack, &rhs.pack)
    }
}
impl Eq for PackRootCategories {}
impl Hash for PackRootCategories {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.pack).hash(state)
    }
}
impl CategoryHandle for PackRootCategories {
    type GetCategories<'a> = Box<dyn Iterator<Item = <Self as PackHandleFactory>::Category> + 'a>;
    fn get_children(&self) -> Result<Self::GetCategories<'_>> {
        Ok(Box::new(self.root_categories()) as Box<_>)
    }
    #[cfg(todo)]
    fn get_pois(&self, recursive: bool) -> Result<Self::GetPois<'_>> {}
    #[cfg(todo)]
    fn get_trails(&self, recursive: bool) -> Result<Self::GetTrails<'_>> {}
    type GetTrails<'a> = Box<dyn Iterator<Item = <Self as PathableHandleFactory>::Trail> + 'a>;
    type GetPois<'a> = Box<dyn Iterator<Item = <Self as PathableHandleFactory>::Poi> + 'a>;

    fn is_root(&self) -> Result<bool> {
        Ok(true)
    }
    fn is_hidden(&self) -> Result<bool> {
        Ok(self.iter_root_categories().all(|c| c.is_hidden()))
    }
    fn is_dynamic(&self) -> Result<bool> {
        Ok(false)
    }
    fn is_separator(&self) -> Result<bool> {
        Ok(self.iter_root_categories().all(|c| c.is_separator()))
    }
    fn get_id_name(&self) -> Result<String> {
        self.primary_root()
            .map(|c| Ok(c.id().into()))
            .unwrap_or_else(|| self.get_id().map(|id| id.name().into()))
    }
    fn get_id(&self) -> Result<CategoryId> {
        self.primary_root()
            .map(|c| c.full_id.clone())
            .ok_or_else(|| anyhow!("TODO: longest common prefix of roots"))
    }
    fn get_display_name(&self) -> Result<String> {
        let name = self
            .primary_root()
            .and_then(|c| c.display_name.as_ref().map(|n| n[..].into()))
            .unwrap_or_else(|| self.pack.name.clone());
        Ok(name)
    }
    fn is_default_toggle(&self) -> Result<bool> {
        Ok(self.iter_root_categories().any(|c| c.default_toggle()))
    }
    fn get_parent(&self) -> Result<Option<Self::Category>> {
        Ok(None)
    }
}
impl CategoryHandleMut for PackRootCategories {}
impl PackHandleFactory for PackRootCategories {
    type Category = <PackArc as PackHandleFactory>::Category;
    type Behaviour = <PackArc as PackHandleFactory>::Behaviour;
    type Guid = <PackArc as PackHandleFactory>::Guid;
}
impl PathableHandleFactory for PackRootCategories {
    type Trail = <PackArc as PathableHandleFactory>::Trail;
    type Poi = <PackArc as PathableHandleFactory>::Poi;
    type Pathable = <PackArc as PathableHandleFactory>::Pathable;
}
impl IntoUserHandle for PackRootCategories {
    type IntoHandle = Self;
    fn into_handle(self) -> Self::IntoHandle {
        self
    }
    #[inline]
    fn clone_into_handle(&self) -> Self::IntoHandle {
        self.clone()
    }
    #[cfg(feature = "script-lua")]
    fn to_lua_handle(&self, lua: &mlua::Lua) -> mlua::Result<mlua::Value> {
        let h = crate::script::lua::RuntimeLua::new_api_root_category(self.clone_into_handle());
        mlua::IntoLua::into_lua(h, lua)
    }
}

impl CategoryHandle for PackCategoryArc {
    type GetCategories<'a> = Box<dyn Iterator<Item = <Self as PackHandleFactory>::Category> + 'a>;
    fn get_children(&self) -> Result<Self::GetCategories<'_>> {
        Ok(Box::new(self.children_cloned()) as Box<_>)
    }

    type GetTrails<'a> = Box<dyn Iterator<Item = <Self as PathableHandleFactory>::Trail> + 'a>;
    type GetPois<'a> = Box<dyn Iterator<Item = <Self as PathableHandleFactory>::Poi> + 'a>;

    fn get_id(&self) -> Result<CategoryId> {
        Ok(self.full_id.clone())
    }
}
impl CategoryHandleMut for PackCategoryArc {
    fn hide(&self) -> Result<()> {
        if let Ok(id) = self.get_id() {
            log::warn!("sure I'll hide `{id}` whatever you say boss");
        }
        Ok(())
    }
}
impl PackHandleFactory for PackCategoryArc {
    type Category = Self;
    type Behaviour = <PackArc as PackHandleFactory>::Behaviour;
    type Guid = <PackArc as PackHandleFactory>::Guid;
}
impl PathableHandleFactory for PackCategoryArc {
    type Trail = <PackArc as PathableHandleFactory>::Trail;
    type Poi = <PackArc as PathableHandleFactory>::Poi;
    type Pathable = <PackArc as PathableHandleFactory>::Pathable;
}
impl IntoUserHandle for PackCategoryArc {
    type IntoHandle = Self;
    fn into_handle(self) -> Self::IntoHandle {
        self
    }
    #[inline]
    fn clone_into_handle(&self) -> Self::IntoHandle {
        self.clone()
    }
    #[cfg(feature = "script-lua")]
    fn to_lua_handle(&self, lua: &mlua::Lua) -> mlua::Result<mlua::Value> {
        mlua::IntoLua::into_lua(self.clone_into_handle(), lua)
    }
}

impl PackHandleFactory for PackTrailArc {
    type Category = <PackArc as PackHandleFactory>::Category;
    type Behaviour = <PackArc as PackHandleFactory>::Behaviour;
    type Guid = <PackArc as PackHandleFactory>::Guid;
}
impl PathableHandle for PackTrailArc {
    /// TODO: pathcontrol has MarkerIndex encoding for this
    #[inline]
    fn pathable_tag_index(&self) -> u32 {
        0x10000000 | (self.trail_idx() as u32)
    }
    #[inline]
    fn pathable_tag_type(&self) -> MarkerType {
        MarkerType::Trail
    }
    fn get_map_id(&self) -> Result<MapID> {
        self.trail_ref()
            .map_id
            .map(|id| id as _)
            .ok_or_else(|| format_err!("trail missing mapid"))
    }
    fn get_behaviour_filtered(&self) -> Result<bool> {
        Ok(false)
    }
}
impl PathableHandleMut for PackTrailArc {}
impl TrailHandle for PackTrailArc {}
impl TrailHandleMut for PackTrailArc {
    fn set_pack_texture<P>(&self, path: P) -> Result<()>
    where
        P: ScriptUserStr,
    {
        path.with_str(|p| {
            log::info!("set texture to {p:?}");
        });
        Ok(())
    }
}
impl IntoUserHandle for PackTrailArc {
    type IntoHandle = Self;
    fn into_handle(self) -> Self::IntoHandle {
        self
    }
    #[inline]
    fn clone_into_handle(&self) -> Self::IntoHandle {
        self.clone()
    }
    #[cfg(feature = "script-lua")]
    fn to_lua_handle(&self, lua: &mlua::Lua) -> mlua::Result<mlua::Value> {
        mlua::IntoLua::into_lua(self.clone_into_handle(), lua)
    }
}

impl PackHandleFactory for PackPoiArc {
    type Category = <PackArc as PackHandleFactory>::Category;
    type Behaviour = <PackArc as PackHandleFactory>::Behaviour;
    type Guid = <PackArc as PackHandleFactory>::Guid;
}
impl PathableHandle for PackPoiArc {
    #[inline]
    fn pathable_tag_index(&self) -> u32 {
        self.poi_idx() as u32
    }
    #[inline]
    fn pathable_tag_type(&self) -> MarkerType {
        MarkerType::Poi
    }

    fn get_map_id(&self) -> Result<MapID> {
        Ok(self.poi_ref().map_id as _)
    }
    fn get_behaviour_filtered(&self) -> Result<bool> {
        Ok(false)
    }
    #[cfg(todo)]
    fn get_marker_attr_dyn(&self, id: PackKeyId) -> Result<Option<PackValueCell>> {}
}
impl PathableHandleMut for PackPoiArc {}
impl PoiHandle for PackPoiArc {
    type Point3 = Vec3;
    type RotationEuler = Vec3;

    #[inline]
    fn get_pos(&self) -> Result<Self::Point3> {
        Ok(self.poi_ref().position.into())
    }
    fn get_rot_euler(&self) -> Result<Self::Point3> {
        Ok(self
            .poi_ref()
            .attributes
            .get_poi()
            .and_then(|poi| poi.rotate)
            .unwrap_or_default()
            .into())
    }
}
impl PoiHandleMut for PackPoiArc {
    fn set_pack_texture<P>(&self, path: P) -> Result<()>
    where
        P: ScriptUserStr,
    {
        path.with_str(|p| {
            log::info!("set texture to {p:?}");
        });
        Ok(())
    }

    fn set_pos<P>(&self, _: P) -> Result<()>
    where
        P: InstanceVec3,
    {
        Ok(())
    }
    fn set_rot_euler<R>(&self, _: R) -> Result<()>
    where
        R: InstanceVec3,
    {
        Ok(())
    }
}
impl IntoUserHandle for PackPoiArc {
    type IntoHandle = Self;
    fn into_handle(self) -> Self::IntoHandle {
        self
    }
    #[inline]
    fn clone_into_handle(&self) -> Self::IntoHandle {
        self.clone()
    }
    #[cfg(feature = "script-lua")]
    fn to_lua_handle(&self, lua: &mlua::Lua) -> mlua::Result<mlua::Value> {
        mlua::IntoLua::into_lua(self.clone_into_handle(), lua)
    }
}
