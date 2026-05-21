use {
    crate::{
        attributes::{
            cell::{
                AttrKeyValue,
                GetAttrDyn,
                PackKeyId,
                PackValueCell,
                PackValueOf,
                PackValueSet,
                SetAttrDyn,
            },
            keys,
            MarkerAttributes,
        },
        category::{id, CategoryId},
        script::{
            format_err,
            lua::{to_lua_error, IColour, LuaProxyOf, ScriptApiTable},
            pathing::imp::MarkerType,
            Result,
        },
    },
    core::{ffi::CStr, marker::PhantomData, mem},
    mlua::{
        BorrowedStr,
        FromLua,
        IntoLua,
        Lua,
        Result as LuaResult,
        UserData,
        UserDataMethods,
        UserDataRegistry,
        Value as LuaValue,
    },
    std::sync::LazyLock,
};

pub trait PathableAttr {
    const INDEX: &'static str;
    const INDEX_C: &'static CStr;
}
pub trait TrailHandleAttr: PathableAttr {}
pub trait PoiHandleAttr: PathableAttr {}

pathable_attr! { all;
    impl PathableAttr for {
        [Guid]: Guid,
        [TriggerRange]: TriggerRange,
        [MapId]: GameMap,
        [Category]: CategoryRef,
        // common but not documented as part of IPathable?
        [Tint]: Tint,
        [FadeNear]: FadeNear,
        [FadeFar]: FadeFar,
        [Alpha]: Alpha,
        [CanFade]: CanFade,
        [CullDirection]: Cull,
        [InGameVisibility]: InGameVisibility,
        [MapVisibility]: MapVisibility,
        [MiniMapVisibility]: MinimapVisibility,
        [ResetLength]: ResetLength,
        [Texture]: TextureFile,
        [ScriptTick]: ScriptTick,
        [ScriptFilter]: ScriptFilter,
        [ScriptOnce]: ScriptOnce,
    }
    impl CategoryHandleAttr for {
        // valid on categories but not markers themselves...
        [Name]: NameId,
        [DefaultToggle]: DefaultToggle,
        [IsHidden]: IsHidden,
        [IsSeparator]: IsSeparator,
        [DisplayName]: DisplayName,
    }
    impl TrailHandleAttr for {
        [TrailScale]: TrailScale,
        [TrailSampleColor]: MapTint,
        [IsWall]: IsWall,
        [AnimationSpeed]: AnimSpeed,
    }
    impl PoiHandleAttr for {
        [AutoTrigger]: AutoTrigger,
        [InvertBehavior]: InvertBehaviour,
        [HeightOffset]: HeightOffset,
        [MapDisplaySize]: MapDisplaySize,
        [MaxSize]: MaxSize,
        [MinSize]: MinSize,
        [Occlude]: Occlude,
        [Icon]: IconFile,
        // TODO: [Position]: Point3,
        // TODO: [RotationXyz]: Rotate,
        [ScaleOnMapWithZoom]: ScaleOnMapWithZoom,
        [Size]: IconSize,
        [TipName]: TipName,
        [TipDescription]: TipDescription,
        [PosX]: PositionX,
        [PosY]: PositionY,
        [PosZ]: PositionZ,
        [RotX]: RotateX,
        [RotY]: RotateY,
        [RotZ]: RotateZ,
        [ScriptTrigger]: ScriptTrigger,
        [ScriptFocus]: ScriptFocus,
        // interact behaviours
        [Copy]: CopyValue,
        [CopyMessage]: CopyMessage,
        [Info]: Info,
        [InfoRange]: InfoRange,
        [BounceBehavior]: Bounce,
        [BounceHeight]: BounceHeight,
        [BounceDuration]: BounceDuration,
        [BounceDelay]: BounceDelay,
        // these all use `.Category` on `IBehaviour` but w/e...
        [ShowCategory]: ShowCategory,
        [HideCategory]: HideCategory,
        [ToggleCategory]: ToggleCategory,
    }
}
#[cfg(todo)]
pathable_attr! {
    impl PoiHandleField for {
        DrawOrder,
        DistanceToPlayer,
        Focused,
    }
}

macro_rules! pathable_attr {
    (all;
        impl PathableAttr for {$(
            [$($key:tt)*]: $attr:ident,
        )+}
        impl CategoryHandleAttr for {$(
            [$($cat_key:tt)*]: $cat_attr:ident,
        )+}
        impl TrailHandleAttr for {$(
            [$($trail_key:tt)*]: $trail_attr:ident,
        )+}
        impl PoiHandleAttr for {$(
            [$($poi_key:tt)*]: $poi_attr:ident,
        )+}
    ) => {
        pub static MARKER_ATTRS: LazyLock<std::collections::BTreeMap<PackKeyId, AttrRegistration>> = LazyLock::new(|| {
            IntoIterator::into_iter([
                $(AttrRegistration::for_type::<$crate::attributes::keys::$attr>(),)*
                $(AttrRegistration::for_type::<$crate::attributes::keys::$cat_attr>(),)*
                $(AttrRegistration::for_type::<$crate::attributes::keys::$trail_attr>(),)*
                $(AttrRegistration::for_type::<$crate::attributes::keys::$poi_attr>(),)*
            ]).map(|reg| (reg.attr, reg)).collect()
        });
        $crate::script::lua::attributes::pathable_attr! {
            impl PathableAttr for {$(
                [$($key)*]: $attr,
            )+}
            impl CategoryHandleAttr for {$(
                [$($cat_key)*]: $cat_attr,
            )+}
            impl TrailHandleAttr for {$(
                [$($trail_key)*]: $trail_attr,
            )+}
            impl PoiHandleAttr for {$(
                [$($poi_key)*]: $poi_attr,
            )+}
        }
    };
    (
        impl PathableAttr for {$(
            [$key:ident]: $attr:ident,
        )+}
        $($rest:tt)*
    ) => {
        $crate::script::lua::attributes::pathable_attr! { @imp
            impl PathableAttr for {$(
                [$key]: $attr,
            )+}
            $($rest)*
        }
        $(
            impl TrailHandleAttr for $crate::attributes::keys::$attr {
            }
            impl PoiHandleAttr for $crate::attributes::keys::$attr {
            }
        )*
    };
    (
        impl TrailHandleAttr for {$(
                [$key:ident]: $attr:ident,
        )+}
        $($rest:tt)*
    ) => {
        $crate::script::lua::attributes::pathable_attr! { @imp
            impl PathableAttr for {$(
                [$key]: $attr,
            )+}
            $($rest)*
        }
        $(
            impl TrailHandleAttr for $crate::attributes::keys::$attr {
            }
        )*
    };
    (
        impl PoiHandleAttr for {$(
                [$key:ident]: $attr:ident,
        )+}
        $($rest:tt)*
    ) => {
        $crate::script::lua::attributes::pathable_attr! { @imp
            impl PathableAttr for {$(
                [$key]: $attr,
            )+}
            $($rest)*
        }
        $(
            impl PoiHandleAttr for $crate::attributes::keys::$attr {
            }
        )*
    };
    (
        impl CategoryHandleAttr for {
            $($imp:tt)*
        }
        $($rest:tt)*
    ) => {
        // TODO?
        $crate::script::lua::attributes::pathable_attr! {
            impl PathableAttr for {$($imp)*}
            $($rest)*
        }
    };
    (@imp
        impl PathableAttr for {$(
                [$key:ident]: $attr:ident,
        )+}
        $($($rest:tt)+)?
    ) => {
        $(
            impl PathableAttr for $crate::attributes::keys::$attr {
                const INDEX: &'static str = stringify!($key);
                const INDEX_C: &'static CStr = unsafe {
                    CStr::from_bytes_with_nul_unchecked(concat!(stringify!($key), "\0").as_bytes())
                };
            }
        )*
        $($crate::script::lua::attributes::pathable_attr! { $($rest)* })?
    };
}
pub(crate) use pathable_attr;

impl IntoLua for keys::GameMap {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        self.0.into_lua(lua)
    }
}
impl FromLua for keys::GameMap {
    fn from_lua(value: LuaValue, lua: &Lua) -> LuaResult<Self> {
        FromLua::from_lua(value, lua).map(Self)
    }
}
impl IntoLua for keys::Tint {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        IColour::<glam::Vec4>::from(self.0).into_lua(lua)
    }
}
impl FromLua for keys::Tint {
    fn from_lua(value: LuaValue, lua: &Lua) -> LuaResult<Self> {
        FromLua::from_lua(value, lua).map(|c: IColour| Self(c.into()))
    }
}
impl IntoLua for keys::MapTint {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        IColour::<glam::Vec4>::from(self.0).into_lua(lua)
    }
}
impl FromLua for keys::MapTint {
    fn from_lua(value: LuaValue, lua: &Lua) -> LuaResult<Self> {
        FromLua::from_lua(value, lua).map(|c: IColour| Self(c.into()))
    }
}
impl IntoLua for keys::Cull {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        self.0.as_str().into_lua(lua)
    }
}
impl FromLua for keys::Cull {
    fn from_lua(value: LuaValue, lua: &Lua) -> LuaResult<Self> {
        BorrowedStr::from_lua(value, lua).and_then(|s| s[..].parse::<keys::Cull>().map_err(to_lua_error))
    }
}
impl IntoLua for keys::Bounce {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        self.0.as_str().into_lua(lua)
    }
}
impl FromLua for keys::Bounce {
    fn from_lua(value: LuaValue, lua: &Lua) -> LuaResult<Self> {
        BorrowedStr::from_lua(value, lua).and_then(|s| s[..].parse::<keys::Bounce>().map_err(to_lua_error))
    }
}
impl IntoLua for CategoryId {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        self.as_str().into_lua(lua)
    }
}
impl FromLua for CategoryId {
    fn from_lua(value: LuaValue, lua: &Lua) -> LuaResult<Self> {
        BorrowedStr::from_lua(value, lua).and_then(|id| {
            CategoryId::try_with_full_id(&id[..]).ok_or_else(|| {
                to_lua_error(format_err!("{}", CategoryId::<id::IdNameBox>::WITH_FULL_ID_ERR))
            })
        })
    }
}
#[cfg(todo)]
impl<T> IntoLua for CategoryId<T>
where
    Self: id::AsFullId,
{
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        id::AsFullId::id_to_str(&self).into_lua(lua)
    }
}

pub struct AttrRegistration {
    attr: PackKeyId,
    index: &'static str,
    #[cfg(todo)]
    index_c: &'static CStr,
    from_lua_dyn: FromLuaDyn,
    vtable_lua: *const (),
}
impl AttrRegistration {
    pub fn for_type<T>() -> Self
    where
        T: Sized + PathableAttr + AttrKeyValueLua + AttrKeyValue,
    {
        Self {
            attr: <T as AttrKeyValue>::pack_key_of(),
            index: <T as PathableAttr>::INDEX,
            #[cfg(todo)]
            index_c: <T as PathableAttr>::INDEX_C,
            vtable_lua: <dyn AttrKeyValueLua>::vtable_ptr_for::<T>(),
            from_lua_dyn: <T as AttrKeyValueLua>::from_lua_dyn as FromLuaDyn,
        }
    }
    #[inline(always)]
    pub fn init_index() {
        core::hint::black_box(&*MARKER_ATTRS);
    }
    pub fn for_key(key: &str) -> Option<&'static Self> {
        Self::init_index();
        PackKeyId::lookup_by_attr(key).and_then(|a| Self::for_attr(a))
    }
    #[inline]
    pub fn for_attr(id: PackKeyId) -> Option<&'static Self> {
        MARKER_ATTRS.get(&id)
    }

    #[inline]
    pub fn attr(&self) -> PackKeyId {
        self.attr
    }
    #[inline]
    pub fn index(&self) -> &'static str {
        self.index
    }
    #[inline]
    pub fn from_lua_dyn(&self, value: LuaValue, lua: &Lua) -> LuaResult<PackValueCell> {
        (self.from_lua_dyn)(value, lua)
    }
    #[inline]
    pub fn get_from_lua_dyn(&self) -> FromLuaDyn {
        self.from_lua_dyn
    }
    #[inline]
    pub fn to_lua_dyn(&self, value: &PackValueCell, lua: &Lua) -> LuaResult<LuaValue> {
        self.to_lua_attr(value)
            .map_err(to_lua_error)
            .and_then(|a| a.into_lua_ref(lua))
    }
    #[inline]
    pub fn to_lua_attr(&'_ self, value: &PackValueCell) -> Result<&dyn AttrKeyValueLua> {
        let id = value.id();
        if !value.is_valid() {
            #[cfg(debug_assertions)]
            if value.flag_storage() != PackValueCell::FLAG_EMPTY {
                return Err(format_err!("nil {}", self.attr))
            }
            return Ok(&())
        } else if id != self.attr {
            return Err(format_err!(
                "expected attr {:?}: {} != {id}",
                self.attr,
                self.index
            ))
        }
        Ok(unsafe {
            let ptr =
                <dyn AttrKeyValueLua>::to_vtable_ptr(value.raw_ptr() as usize, self.vtable_lua as usize);
            &*(ptr as *const dyn AttrKeyValueLua)
        })
    }
    /// TODO: is this ever used in a context where `&mut Nil` is valid?
    #[inline]
    pub fn to_lua_attr_mut(&'_ self, value: &mut PackValueCell) -> Result<&mut dyn AttrKeyValueLua> {
        let id = value.id();
        if !value.is_valid() {
            return Err(format_err!("nil {}", self.attr))
        } else if id != self.attr {
            return Err(format_err!(
                "expected attr {:?}: {} != {id}",
                self.attr,
                self.index
            ))
        }
        Ok(unsafe {
            &mut *<dyn AttrKeyValueLua>::to_vtable_ptr(
                value.raw_ptr_mut() as usize,
                self.vtable_lua as usize,
            )
        })
    }
}
unsafe impl Sync for AttrRegistration {}
unsafe impl Send for AttrRegistration {}

pub unsafe trait AttrKeyValueLua: 'static {
    fn into_lua_ref(&self, lua: &Lua) -> LuaResult<LuaValue>;
    fn from_lua_dyn(value: LuaValue, lua: &Lua) -> LuaResult<PackValueCell>
    where
        Self: Sized;
    #[inline]
    fn cell_from_lua(value: LuaValue, lua: &Lua) -> LuaResult<PackValueOf<Self>>
    where
        Self: Sized + AttrKeyValue,
    {
        Self::from_lua_dyn(value, lua).map(|cell| unsafe { PackValueOf::new_unchecked(cell) })
    }
}
unsafe impl AttrKeyValueLua for () {
    fn into_lua_ref(&self, _: &Lua) -> LuaResult<LuaValue> {
        Ok(LuaValue::Nil)
    }
    fn from_lua_dyn(_: LuaValue, _: &Lua) -> LuaResult<PackValueCell> {
        Err(super::lua2do())
    }
}
impl dyn AttrKeyValueLua {
    pub fn vtable_ptr_for<T>() -> *const ()
    where
        T: Sized + AttrKeyValueLua,
    {
        let dummy = mem::MaybeUninit::<T>::uninit();
        let p: *const Self = &raw const *dummy.as_ptr();
        Self::vtable_ptr_of(p)
    }
    pub fn vtable_ptr_of(p: *const Self) -> *const () {
        let [_, vtbl] = unsafe { mem::transmute::<*const dyn AttrKeyValueLua, [*const (); 2]>(p) };
        vtbl
    }
    pub unsafe fn to_vtable_ptr(p: usize, vtbl: usize) -> *mut Self {
        unsafe { mem::transmute::<[usize; 2], *mut dyn AttrKeyValueLua>([p, vtbl]) }
    }
}
unsafe impl<T> AttrKeyValueLua for T
where
    T: AttrKeyValue + PathableAttr + IntoLua + FromLua + Clone + 'static,
{
    fn into_lua_ref(&self, lua: &Lua) -> LuaResult<LuaValue> {
        self.clone().into_lua(lua)
    }
    /// TODO: nil=empty sane in all cases or let impls decide via `from_lua`?
    /// or let methods decide via `Option<Self>`?
    #[inline(never)]
    fn from_lua_dyn(value: LuaValue, lua: &Lua) -> LuaResult<PackValueCell>
    where
        Self: Sized,
    {
        match value {
            LuaValue::Nil => Ok(PackValueCell::new_empty(T::pack_key_of())),
            value => Self::from_lua(value, lua).map(PackValueCell::new_boxed::<T>),
        }
    }
    #[inline]
    fn cell_from_lua(value: LuaValue, lua: &Lua) -> LuaResult<PackValueOf<Self>> {
        if value.is_nil() {
            return Err(to_lua_error(format_err!("{} was nil", T::pack_key_of())))
        }
        Self::from_lua_dyn(value, lua).map(|cell| unsafe { PackValueOf::new_unchecked(cell) })
    }
}
type FromLuaDyn = for<'a> fn(LuaValue, &'a Lua) -> LuaResult<PackValueCell>;

#[derive(Debug, Clone)]
pub struct MarkerAttrSet {
    pub attrs: PackValueSet,
    pub kind: MarkerType,
}
impl MarkerAttrSet {
    #[inline]
    pub fn new(kind: MarkerType) -> Self {
        Self { kind, attrs: Default::default() }
    }
    pub fn lookup_key(&self, key: &str) -> Option<&'static AttrRegistration> {
        AttrRegistration::for_key(&key)
    }
    #[cfg(todo)]
    pub fn lookup_index(&self, index: &str) -> Option<&'static AttrRegistration> {
        AttrRegistration::for_index(&key)
    }
    fn try_lookup_key(&self, key: &str) -> LuaResult<&'static AttrRegistration> {
        self.lookup_key(key)
            .ok_or_else(|| to_lua_error(format_err!("unrecognized attribute key {key}")))
    }
    #[cfg(todo)]
    fn try_lookup_index(&self, index: &str) -> LuaResult<&'static AttrRegistration> {
        self.lookup_index(index)
            .ok_or_else(|| to_lua_error(format_err!("unrecognized attribute index {index}")))
    }

    pub fn take_all(&mut self) -> Self {
        Self {
            kind: self.kind,
            attrs: mem::take(&mut self.attrs),
        }
    }
    pub fn drain_all(&mut self) -> impl Iterator<Item = PackValueCell> {
        mem::take(&mut self.attrs).into_iter()
    }
}
impl IntoIterator for MarkerAttrSet {
    type Item = PackValueCell;
    type IntoIter = std::collections::btree_set::IntoIter<PackValueCell>;
    fn into_iter(self) -> Self::IntoIter {
        self.attrs.into_iter()
    }
}
impl UserData for MarkerAttrSet {
    fn register(reg: &mut UserDataRegistry<Self>) {
        reg.add_function("new_poi", |_lua, ()| Ok(Self::new(MarkerType::Poi)));
        reg.add_function("new_trail", |_lua, ()| Ok(Self::new(MarkerType::Trail)));
        reg.add_function("new_category", |_lua, ()| Ok(Self::new(MarkerType::Category)));
        reg.add_method_mut("UnsetAttrByKey", |_lua, this, (key,): (BorrowedStr<'_>,)| {
            this.try_lookup_key(&key).map(|key| {
                this.attrs.remove(&key.attr);
            })
        });
        reg.add_method_mut("GetAttrByKey", |lua, this, (key,): (BorrowedStr<'_>,)| {
            this.try_lookup_key(&key)
                .and_then(|key| match this.attrs.get(&key.attr) {
                    Some(v) => key.to_lua_dyn(v, lua),
                    None => Ok(LuaValue::Nil),
                })
        });
        reg.add_method_mut(
            "SetAttrByKey",
            |lua, this, (key, value): (BorrowedStr<'_>, LuaValue)| {
                this.try_lookup_key(&key).and_then(|key| {
                    key.from_lua_dyn(value, lua).map(|v| {
                        this.attrs.replace(v);
                    })
                })
            },
        );
        #[cfg(todo)]
        {
            reg.add_method_mut("UnsetAttrByIndex", |_lua, this, (key,): (BorrowedStr<'_>,)| {
                this.try_lookup_index(&key).map(|key| {
                    this.attrs.remove(&key.attr);
                })
            });
            reg.add_method_mut("GetAttrByIndex", |lua, this, (key,): (BorrowedStr<'_>,)| {
                this.try_lookup_index(&key)
                    .and_then(|key| match this.attrs.get(&key.attr) {
                        Some(v) => key.to_lua_dyn(v, lua),
                        None => Ok(LuaValue::Nil),
                    })
            });
            reg.add_method_mut(
                "SetAttrByIndex",
                |lua, this, (key, value): (BorrowedStr<'_>, LuaValue)| {
                    this.try_lookup_index(&key).and_then(|key| {
                        key.from_lua_dyn(value, lua).map(|v| {
                            this.attrs.replace(v);
                        })
                    })
                },
            );
        }
    }
}

impl UserData for MarkerAttributes {
    fn register(reg: &mut UserDataRegistry<Self>) {
        reg.add_function("new", |_lua, ()| Ok(Self::default()));
        reg.add_method_mut("UnsetAttrByKey", |_lua, this, (key,): (BorrowedStr<'_>,)| {
            AttrRegistration::for_key(&key)
                .ok_or_else(|| to_lua_error(format_err!("unrecognized attr {key}")))
                .map(|key| {
                    this.set_attr_dyn(PackValueCell::new_empty(key.attr()));
                })
        });
        reg.add_method_mut("GetAttrByKey", |lua, this, (key,): (BorrowedStr<'_>,)| {
            // TODO: variant of to_lua_dyn with &dyn AttrKeyValue receiver to avoid cloning into cell!
            AttrRegistration::for_key(&key)
                .ok_or_else(|| to_lua_error(format_err!("unrecognized attr {key}")))
                .and_then(|key| match this.get_attr_dyn(key.attr()) {
                    Some(v) => key.to_lua_dyn(v.into_owned().inner(), lua),
                    None => Ok(LuaValue::Nil),
                })
        });
        reg.add_method_mut(
            "SetAttrByKey",
            |lua, this, (key, value): (BorrowedStr<'_>, LuaValue)| {
                AttrRegistration::for_key(&key)
                    .ok_or_else(|| to_lua_error(format_err!("unrecognized attr {key}")))
                    .and_then(|key| {
                        key.from_lua_dyn(value, lua).map(|v| {
                            this.set_attr_dyn(v);
                        })
                    })
            },
        );
    }
}

pub trait ScriptApiMarkerAttrs {
    type MarkerAttrsConstructor: IntoLua;
    fn marker_attrs_constructor(&mut self) -> Self::MarkerAttrsConstructor;
}
impl ScriptApiMarkerAttrs for PhantomData<MarkerAttrSet> {
    type MarkerAttrsConstructor = LuaProxyOf<MarkerAttrSet>;
    fn marker_attrs_constructor(&mut self) -> Self::MarkerAttrsConstructor {
        Default::default()
    }
}
#[cfg(todo)]
impl ScriptApiMarkerAttrs for PhantomData<MarkerAttributes> {
    type MarkerAttrsConstructor = super::LuaProxyOf<MarkerAttributes>;
    fn marker_attrs_constructor(&mut self) -> Self::MarkerAttrsConstructor {
        Default::default()
    }
}
pub struct GlobalInstanceAttrs;
impl<T> IntoLua for ScriptApiTable<GlobalInstanceAttrs, T>
where
    T: ScriptApiMarkerAttrs + 'static,
{
    fn into_lua(mut self, lua: &Lua) -> LuaResult<LuaValue> {
        let marker_attrs = self.api.marker_attrs_constructor().into_lua(lua)?;
        lua.create_table_from([("MarkerAttributes", marker_attrs)])
            .map(LuaValue::Table)
    }
}
