use {
    crate::{
        attributes::cell::{PackKeyId, PackValueCell},
        category::CategoryId,
        script::{
            script_unimpl,
            user::{IntoUserHandle, ScriptSourceTag, ScriptUserGuid, ScriptUserStr},
            value::Vec3,
            Result,
        },
    },
    core::time::Duration,
    std::io,
};

mod debug;
pub mod event;
pub mod imp;
mod menu;
mod mumble;
mod semver;
mod vector;

pub use self::{
    debug::{ScriptApiDebugLog, ScriptApiDebugWatch},
    event::ScriptApiEvent,
    imp::MarkerType,
    menu::{MenuDesc, MenuHandle, MenuHandleMut, MenuInstance},
    mumble::ScriptApiMumble,
    semver::{ScriptApiVersion, ScriptApiVersionString},
    vector::{InstanceColour, InstanceVec3},
};

pub type MapFilterArg = Option<MapID>;
pub type MapID = u32;
#[cfg(todo)]
pub type MapIndex = core::num::NonZero<u32>;

/// TODO: consider linking this to a pack handle? but packs can share GUIDs to reuse hidden state...
pub trait InstanceGuid {
    fn from_base_64<G: ScriptUserStr>(guid: G) -> Result<Self>
    where
        Self: Sized;
    fn to_base_64(&self) -> Result<String>;
}
pub trait TextureHandle: InstanceTexture {}
pub trait InstanceTexture {
    fn get_size(&self) -> Result<[u32; 2]> {
        script_unimpl!()
    }
}
pub trait GameTime {
    type TimeSpan: TimeSpan;
    type TickSpan: TickSpan;
    fn elapsed_game_time(&self) -> Self::TimeSpan;
    fn elapsed_game_ticks(&self) -> Self::TickSpan;
    fn total_game_time(&self) -> Self::TimeSpan;
    fn total_game_ticks(&self) -> Self::TickSpan;
}
pub trait TickSpan {
    /// number of ticks spanned?
    fn ticks(&self) -> u32;
}
impl TickSpan for u32 {
    #[inline]
    fn ticks(&self) -> u32 {
        *self
    }
}
pub trait TimeSpan {
    #[inline]
    fn total_d_float(&self) -> f32 {
        self.total_s() as f32 / 86400.0
    }
    #[inline]
    fn total_h_float(&self) -> f32 {
        self.total_s() as f32 / 3600.0
    }
    #[inline]
    fn total_m_float(&self) -> f32 {
        self.total_ms() as f32 / 60000.0
    }
    #[inline]
    fn total_s_float(&self) -> f32 {
        self.total_ms() as f32 / 1000.0
    }
    #[inline]
    fn total_ms_float(&self) -> f32 {
        self.total_ms() as f32
    }

    #[inline]
    fn total_d(&self) -> u32 {
        (self.total_h() / 24) as _
    }
    #[inline]
    fn total_h(&self) -> u32 {
        (self.total_m() / 60) as _
    }
    #[inline]
    fn total_m(&self) -> u32 {
        (self.total_s() / 60) as _
    }
    fn total_s(&self) -> u64;
    fn total_ms(&self) -> u64;

    #[inline]
    fn part_d(&self) -> u32 {
        self.total_d() as _
    }
    #[inline]
    fn part_h(&self) -> u32 {
        (self.total_h() % 24) as _
    }
    #[inline]
    fn part_m(&self) -> u32 {
        (self.total_m() % 60) as _
    }
    #[inline]
    fn part_s(&self) -> u32 {
        (self.total_s() % 60) as _
    }
    #[inline]
    fn part_ms(&self) -> u32 {
        (self.total_ms() % 1000) as _
    }
}
impl TimeSpan for Duration {
    #[inline]
    fn total_s(&self) -> u64 {
        self.as_secs() as _
    }
    #[inline]
    fn total_ms(&self) -> u64 {
        self.as_millis() as _
    }
    #[inline]
    fn total_s_float(&self) -> f32 {
        self.as_secs_f32()
    }
    #[inline]
    fn total_ms_float(&self) -> f32 {
        (self.as_micros() as f64 / 1000.0) as f32
    }
}
pub trait ScriptWebTexture {}
pub trait ScriptPackTexture {}
pub trait PackHandleFactory {
    type Category: CategoryHandle + IntoUserHandle;
    /// TODO: move to PathableHandle
    type Behaviour: BehaviourHandle + IntoUserHandle;
    type Guid: GuidHandle + IntoUserHandle;
}
pub trait PathableHandleFactory: PackHandleFactory {
    type Trail: TrailHandle + IntoUserHandle;
    type Poi: PoiHandle + IntoUserHandle;
    type Pathable: PathableHandle + IntoUserHandle;
}
#[allow(unused_variables)]
pub trait ScriptApiLookup: PathableHandleFactory {
    fn trail_by_guid<G>(&self, guid: G, map_filter: MapFilterArg) -> Result<Option<Self::Trail>>
    where
        G: ScriptUserGuid,
    {
        script_unimpl!()
    }
    fn poi_by_guid<G>(&self, guid: G, map_filter: MapFilterArg) -> Result<Option<Self::Poi>>
    where
        G: ScriptUserGuid,
    {
        script_unimpl!()
    }
    fn pathable_by_tag(&self, tag: u32) -> Result<Option<Self::Pathable>> {
        script_unimpl!()
    }
    fn pathable_by_guid<G>(&self, guid: G, map_filter: MapFilterArg) -> Result<Option<Self::Pathable>>
    where
        G: ScriptUserGuid,
    {
        script_unimpl!()
    }
    #[cfg(todo)]
    fn pathable_by_guid<G>(&self, guid: G, map_filter: MapFilterArg) -> Result<Option<Self::Pathable>>
    where
        G: ScriptUserGuid,
    {
        if let Some(poi) = self.poi_by_guid(guid)? {
            Ok(Some(poi.into()))
        } else {
            self.trail_by_guid(guid).map(|opt| opt.map(|t| t.into()))
        }
    }
    type PathablesByGuid<'a>: Iterator<Item = Self::Pathable>
    where
        Self: 'a;
    fn pathables_by_guid<G>(&self, guid: G, map_filter: MapFilterArg) -> Result<Self::PathablesByGuid<'_>>
    where
        G: ScriptUserGuid,
    {
        script_unimpl!()
    }

    fn pois_in_category<I>(&self, id: I) -> Result<Self::CategoryPois<'_>>
    where
        I: ScriptUserStr,
    {
        script_unimpl!()
    }
    type CategoryPois<'a>: Iterator<Item = Self::Poi>
    where
        Self: 'a;
    fn pois_under_category<I>(&self, id: I) -> Result<Self::CategoryPoisRec<'_>>
    where
        I: ScriptUserStr,
    {
        script_unimpl!()
    }
    type CategoryPoisRec<'a>: Iterator<Item = Self::Poi>
    where
        Self: 'a;
    fn trails_in_category<I>(&self, id: I) -> Result<Self::CategoryTrails<'_>>
    where
        I: ScriptUserStr,
    {
        script_unimpl!()
    }
    type CategoryTrails<'a>: Iterator<Item = Self::Trail>
    where
        Self: 'a;
    fn trails_under_category<I>(&self, id: I) -> Result<Self::CategoryTrailsRec<'_>>
    where
        I: ScriptUserStr,
    {
        script_unimpl!()
    }
    type CategoryTrailsRec<'a>: Iterator<Item = Self::Trail>
    where
        Self: 'a;
}
#[allow(unused_variables)]
pub trait ScriptApiSpaceQuery: PathableHandleFactory {
    fn get_closest_poi_in_category<I>(&self, id: Option<I>) -> Result<Option<Self::Poi>>
    where
        I: ScriptUserStr,
    {
        script_unimpl!()
    }
    type ClosestPois: Iterator<Item = Self::Poi>;
    fn get_closest_pois_in_category<I>(&self, limit: usize, id: Option<I>) -> Result<Self::ClosestPois>
    where
        I: ScriptUserStr,
    {
        script_unimpl!()
    }

    #[cfg(todo = "unused")]
    fn get_closest_poi(&self) -> Result<Option<Self::Poi>> {
        self.get_closest_poi_in_category(None::<&str>)
    }
    #[cfg(todo = "unused")]
    fn get_closest_pois(&self, limit: usize) -> Result<Self::ClosestPois> {
        self.get_closest_pois_in_category(limit, None::<&str>)
    }

    fn get_distance_to_player(&self, marker: &Self::Poi) -> Result<f32> {
        script_unimpl!()
    }
}
pub trait ScriptApiWorld: ScriptApiLookup + ScriptApiSpaceQuery {}
#[allow(unused_variables)]
pub trait ScriptApiUser {
    fn set_clipboard<S: ScriptUserStr, M: ScriptUserStr>(
        &self,
        value: S,
        message: Option<M>,
    ) -> Result<()> {
        script_unimpl!()
    }
    fn info_show<S: ScriptUserStr>(&self, message: S) -> Result<String> {
        script_unimpl!()
    }
    fn info_hide<S: ScriptUserStr>(&self, token: S) -> Result<()> {
        Ok(())
    }
    fn info_notify<S: ScriptUserStr>(&self, message: S, duration: Option<Duration>) -> Result<()> {
        script_unimpl!()
    }
}

#[allow(unused_variables)]
pub trait ScriptApiPack {
    type Pack: PackHandle + IntoUserHandle;
    fn current_pack(&self) -> Result<Self::Pack> {
        script_unimpl!()
    }
    fn current_pack_assets<'a>(&'a self) -> Result<Self::PackAssets<'a>> {
        script_unimpl!()
    }
    type PackAssets<'a>: ScriptApiPackAssets
    where
        Self: 'a;

    fn current_pack_store<'a>(&'a self) -> Result<Self::PackStore<'a>> {
        script_unimpl!()
    }
    type PackStore<'a>: ScriptApiStorage
    where
        Self: 'a;

    fn current_pack_world<'a>(&'a self) -> Result<Self::PackWorld<'a>> {
        script_unimpl!()
    }
    type PackWorld<'a>: ScriptApiLookup
    where
        Self: 'a;

    fn current_pack_space<'a>(&'a self) -> Result<Self::PackSpace<'a>> {
        script_unimpl!()
    }
    type PackSpace<'a>: ScriptApiSpaceQuery
    where
        Self: 'a;

    fn current_pack_menu<'a>(&'a self) -> Result<Self::PackMenu<'a>> {
        script_unimpl!()
    }
    type PackMenu<'a>: MenuDesc
    where
        Self: 'a;

    #[cfg(todo = "unnecessary")]
    fn current_pack_root<'a>(&'a self) -> Result<Self::PackRoot<'a>> {
        self.current_pack()?.root_category(id)
    }

    #[cfg(todo = "unnecessary")]
    fn current_pack_category<I>(&self, id: I) -> Result<<Self::Pack as PackHandleFactory>::Category>
    where
        I: ScriptUserStr,
    {
        self.current_pack()?.get_category(id)
    }
}
#[allow(unused_variables)]
pub trait ScriptApiPackAssets {
    type RequireSrc: io::Read;
    fn require_src<S: ScriptUserStr>(&self, path: S) -> Result<Option<Self::RequireSrc>> {
        script_unimpl!()
    }

    fn open_texture<P>(&self, path: P) -> Result<Self::Texture>
    where
        P: ScriptUserStr,
    {
        script_unimpl!()
    }
    type Texture: TextureHandle + IntoUserHandle;
}
#[cfg(todo)]
impl_upcast! {
    impl UpcastHandle for TrailHandle {}
    impl UpcastHandle for PoiHandle {}
    impl UpcastHandle for PathableHandle {}
}
pub trait GetAttrKey<K> {
    type AttrValue: ScriptSourceTag;
}
pub trait GuidHandle: InstanceGuid {}
pub trait PoiHandle: PathableHandle {
    type Point3: InstanceVec3;
    fn get_pos(&self) -> Result<Self::Point3> {
        script_unimpl!()
    }
    type RotationEuler: InstanceVec3;
    fn get_rot_euler(&self) -> Result<Self::RotationEuler> {
        script_unimpl!()
    }
}
#[allow(unused_variables)]
pub trait PoiHandleMut: PoiHandle + PathableHandleMut {
    fn set_pos<P>(&self, pos: P) -> Result<()>
    where
        P: InstanceVec3,
    {
        script_unimpl!()
    }
    fn set_pos_x(&self, x: f32) -> Result<()> {
        let [_, y, z] = self.get_pos()?.get3();
        self.set_pos::<Vec3>([x, y, z].into())
    }
    fn set_pos_y(&self, y: f32) -> Result<()> {
        let [x, _, z] = self.get_pos()?.get3();
        self.set_pos::<Vec3>([x, y, z].into())
    }
    fn set_pos_z(&self, z: f32) -> Result<()> {
        let [x, y, _] = self.get_pos()?.get3();
        self.set_pos::<Vec3>([x, y, z].into())
    }

    fn set_rot_euler<P>(&self, rot: P) -> Result<()>
    where
        P: InstanceVec3,
    {
        script_unimpl!()
    }
    fn set_rot_x(&self, x: f32) -> Result<()> {
        let [_, y, z] = self.get_pos()?.get3();
        self.set_pos::<Vec3>([x, y, z].into())
    }
    fn set_rot_y(&self, y: f32) -> Result<()> {
        let [x, _, z] = self.get_pos()?.get3();
        self.set_pos::<Vec3>([x, y, z].into())
    }
    fn set_rot_z(&self, z: f32) -> Result<()> {
        let [x, y, _] = self.get_pos()?.get3();
        self.set_pos::<Vec3>([x, y, z].into())
    }
    fn set_pack_texture<P>(&self, path: P) -> Result<()>
    where
        P: ScriptUserStr,
    {
        script_unimpl!()
    }
    fn set_web_texture(&self, id: u64) -> Result<()> {
        script_unimpl!()
    }
}
pub trait TrailHandle: PathableHandle {}
#[allow(unused_variables)]
pub trait TrailHandleMut: TrailHandle + PathableHandleMut {
    fn set_pack_texture<P>(&self, path: P) -> Result<()>
    where
        P: ScriptUserStr,
    {
        script_unimpl!()
    }
    fn set_web_texture(&self, id: u64) -> Result<()> {
        script_unimpl!()
    }
    fn set_points<P>(&self, points: P) -> Result<()>
    where
        //P: ScriptUserIterable,
        P: IntoIterator<Item = Vec3>,
    {
        script_unimpl!()
    }
}
/// TODO: `where Self: PathableHandleFactory<Category = Self>` ?
#[allow(unused_variables)]
pub trait CategoryHandle: PathableHandleFactory {
    fn get_children(&self) -> Result<Self::GetCategories<'_>> {
        script_unimpl!()
    }
    type GetCategories<'a>: Iterator<Item = Self::Category> + 'a
    where
        Self: 'a;

    fn get_pois(&self, recursive: bool) -> Result<Self::GetPois<'_>> {
        script_unimpl!()
    }
    type GetPois<'a>: Iterator<Item = Self::Poi> + 'a
    where
        Self: 'a;

    fn get_trails(&self, recursive: bool) -> Result<Self::GetTrails<'_>> {
        script_unimpl!()
    }
    type GetTrails<'a>: Iterator<Item = Self::Trail> + 'a
    where
        Self: 'a;

    fn get_display_name(&self) -> Result<String> {
        script_unimpl!()
    }
    fn is_default_toggle(&self) -> Result<bool> {
        script_unimpl!()
    }
    fn is_hidden(&self) -> Result<bool> {
        script_unimpl!()
    }
    fn is_separator(&self) -> Result<bool> {
        script_unimpl!()
    }
    fn is_root(&self) -> Result<bool> {
        script_unimpl!()
    }
    /// otherwise built in to pack
    fn is_dynamic(&self) -> Result<bool> {
        script_unimpl!()
    }
    fn get_id_name(&self) -> Result<String> {
        script_unimpl!()
    }
    fn get_id(&self) -> Result<CategoryId> {
        script_unimpl!()
    }
    fn get_parent(&self) -> Result<Option<Self::Category>> {
        script_unimpl!()
    }
    #[cfg(todo)]
    fn get_child<I>(&self, id: I) -> Result<Option<Self::Category>>
    where
        I: ScriptUserStr,
    {
        script_unimpl!()
    }
    #[cfg(todo)]
    fn get_or_add_category_from_namespace<N>(&self, namespace: N) -> Result<Self::Category>
    where
        N: ScriptUserStr,
    {
        let existing = namespace.with_str(|id| self.get_child(id));
        match existing {
            Err(e) => return Err(e),
            Ok(Some(cat)) => return Ok(cat),
            Ok(None) => script_unimpl!(),
        }
    }

    fn get_category_attr_dyn(&self, id: PackKeyId) -> Result<Option<PackValueCell>> {
        script_unimpl!()
    }
}
#[allow(unused_variables)]
pub trait CategoryHandleMut: CategoryHandle {
    fn hide(&self) -> Result<()> {
        script_unimpl!()
    }
    fn show(&self) -> Result<()> {
        script_unimpl!()
    }
    fn is_visible(&self) -> Result<bool> {
        script_unimpl!()
    }
    fn set_category_attr_dyn(&self, v: PackValueCell) -> Result<()> {
        script_unimpl!()
    }
}
pub trait CategoryHandleExt: CategoryHandle {
    #[cfg(todo)]
    fn get_descendents(&self) -> impl Iterator<Item = Result<Self::Category>> {
        let children = match self.get_children() {
            Ok(c) => None::<Result<Self::Category>>
                .into_iter()
                .chain(Some(c.map(Ok)).flatten()),
            Err(e) => Some(Err(e)).into_iter().chain(None.flatten()),
        };
        children.flat_map(|c| c.get_descendents())
    }
}
pub trait BehaviourHandle {}
#[allow(unused_variables)]
pub trait PackHandle: PathableHandleFactory {
    /// TODO: move to [Self::RootCategory]?
    fn get_category<I>(&self, id: I) -> Result<Option<Self::Category>>
    where
        I: ScriptUserStr,
    {
        script_unimpl!()
    }
    fn get_category_children<'a>(&'a self, parent: &'a Self::Category) -> Result<Self::GetCategories<'a>> {
        script_unimpl!()
    }
    type GetCategories<'a>: Iterator<Item = <Self as PackHandleFactory>::Category> + 'a
    where
        Self: 'a;

    /// recursive variant of [Self::get_category_children]
    ///
    /// TODO: guarantee bfs or dfs?
    fn get_category_descendents<'a>(
        &'a self,
        parent: &'a Self::Category,
    ) -> Result<Self::GetCategoriesRec<'a>> {
        script_unimpl!()
    }
    type GetCategoriesRec<'a>: Iterator<Item = <Self as PackHandleFactory>::Category> + 'a
    where
        Self: 'a;

    fn root_category(&self) -> Result<Self::RootCategory> {
        script_unimpl!()
    }
    type RootCategory: CategoryHandle<Category = <Self as PackHandleFactory>::Category>;

    fn category_roots(&self) -> Result<Self::RootCategories<'_>> {
        script_unimpl!()
    }
    type RootCategories<'a>: Iterator<Item = <Self as PackHandleFactory>::Category> + 'a
    where
        Self: 'a;
}
#[allow(unused_variables)]
pub trait PackHandleMut: PackHandle {
    /// `Pack:CreateMarker({iconFile = "icon.png", xpos=0, ypos=0, zpos=0})`
    fn create_poi<A>(&self, attrs: A) -> Result<Self::Poi>
    where
        //A: ScriptUserAttrs,
        A: IntoIterator<Item = PackValueCell>,
    {
        script_unimpl!()
    }
    fn remove_poi(&self, poi: &Self::Poi) -> Result<()> {
        script_unimpl!()
    }
    fn create_trail<A>(&self, attrs: A) -> Result<Self::Trail>
    where
        //A: ScriptUserAttrs,
        A: IntoIterator<Item = PackValueCell>,
    {
        script_unimpl!()
    }
    fn remove_trail(&self, trail: &Self::Trail) -> Result<()> {
        script_unimpl!()
    }
    /// TODO: move to [PackHandle::RootCategory]?
    fn create_category<N, A>(&self, id: N, attrs: A) -> Result<Self::Category>
    where
        N: ScriptUserStr,
        A: IntoIterator<Item = PackValueCell>,
    {
        script_unimpl!()
    }
    /// TODO: move to [PackHandle::RootCategory]?
    fn remove_category(&self, cat: &Self::Category) -> Result<()> {
        script_unimpl!()
    }
    /// `Category:GetOrAddCategoryFromNamespace("my.thing")`
    ///
    /// TODO: move to [PackHandle::RootCategory]?
    #[cfg(todo)]
    fn get_or_add_category_from_namespace<N>(&self, namespace: N) -> Result<Self::Category>
    where
        N: ScriptUserStr,
    {
        let existing = namespace.with_str(|id| self.get_category(id));
        match existing {
            Err(e) => return Err(e),
            Ok(Some(cat)) => return Ok(cat),
            Ok(None) => self.create_category(namespace, iter::empty()),
        }
    }
}
#[cfg(todo)]
pub trait ScriptUserGuid: ScriptUserHandle
where
    <Self as ScriptUserHandle>::Handle: UpcastHandle<dyn super::user::ScriptUserGuid>,
{
}
/// `IPathable`
#[allow(unused_variables)]
pub trait PathableHandle: PackHandleFactory {
    /// scoped to pack, should be unique across all pois+trails etc
    fn pathable_tag_index(&self) -> u32;
    fn pathable_tag_type(&self) -> MarkerType;

    fn get_marker_attr_dyn(&self, id: PackKeyId) -> Result<Option<PackValueCell>> {
        script_unimpl!()
    }
    fn get_guid(&self) -> Result<Self::Guid> {
        script_unimpl!()
    }
    fn get_map_id(&self) -> Result<MapID> {
        script_unimpl!()
    }

    fn get_behaviour_filtered(&self) -> Result<bool> {
        script_unimpl!()
    }
    fn get_focused(&self) -> Result<bool> {
        script_unimpl!()
    }
}
#[allow(unused_variables)]
pub trait PathableHandleMut: PathableHandle {
    fn set_marker_attr_dyn(&self, v: PackValueCell) -> Result<()> {
        script_unimpl!()
    }
    fn set_guid<G>(&self, guid: G) -> Result<()>
    where
        G: ScriptUserGuid,
    {
        script_unimpl!()
    }

    fn focus(&self) -> Result<()> {
        script_unimpl!()
    }
    fn unfocus(&self) -> Result<()> {
        script_unimpl!()
    }

    fn interact(&self, as_auto_trigger: bool) -> Result<()> {
        script_unimpl!()
    }
}

#[allow(unused_variables)]
pub trait ScriptApiStorage {
    fn insert_string<K, N, V>(&self, key: K, namespace: Option<N>, value: V) -> Result<Option<String>>
    where
        K: ScriptUserStr,
        N: ScriptUserStr,
        V: ScriptUserStr,
    {
        script_unimpl!()
    }
    fn remove_key<K, N>(&self, key: K, namespace: Option<N>) -> Result<()>
    where
        N: ScriptUserStr,
        K: ScriptUserStr,
    {
        script_unimpl!()
    }
    fn get_string<K, N>(&self, key: K, namespace: Option<N>) -> Result<Option<String>>
    where
        K: ScriptUserStr,
        N: ScriptUserStr,
    {
        script_unimpl!()
    }
}
