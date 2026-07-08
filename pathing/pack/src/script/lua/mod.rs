//! <https://taimihud.com/docs/pathing/status/#script-api>

use {
    crate::{
        attributes::keys::Guid,
        script::{
            pathing::{
                self,
                InstanceGuid,
                MenuHandleMut,
                MenuInstance,
                ScriptApiDebugLog,
                ScriptApiEvent,
                ScriptApiLookup,
                ScriptApiMumble,
                ScriptApiPack,
                ScriptApiPackAssets,
                ScriptApiSpaceQuery,
                ScriptApiStorage,
                ScriptApiUser,
                ScriptApiVersion,
                ScriptApiVersionString,
            },
            user::{
                IntoUserHandle,
                ScriptSourceTag,
                ScriptUserAttrs,
                ScriptUserGuid,
                ScriptUserHandle,
                ScriptUserStr,
                ScriptUserUntyped,
                SourceTag,
            },
            value::Colour,
            Result,
            ScriptError,
        },
    },
    anyhow::anyhow,
    core::{
        borrow::Borrow,
        ffi::CStr,
        fmt,
        hash::{Hash, Hasher},
        marker::PhantomData,
        mem,
        ops,
    },
    mlua::{
        AnyUserData,
        ChunkMode,
        Error as LuaError,
        FromLua,
        FromLuaMulti,
        Function as LuaFunction,
        IntoLua,
        IntoLuaMulti,
        Lua,
        LuaOptions,
        MetaMethod,
        MultiValue,
        Result as LuaResult,
        StdLib,
        String as LuaString,
        Table,
        Thread as LuaThread,
        UserData,
        UserDataMethods,
        UserDataRef,
        UserDataRegistry,
        Value as LuaValue,
    },
    std::sync::Arc,
    taimi_hoard::lazyfmt,
};

#[cfg(todo)]
mod adapter;
pub mod attributes;
mod builtin;
mod debug;
mod event;
mod mumble;
mod pack;
mod persist;
mod require;
mod semver;
mod timespan;
mod ui;
mod vectors;

pub use self::{
    attributes::{AttrRegistration, GlobalInstanceAttrs, ScriptApiMarkerAttrs},
    builtin::ApiCoreRt,
    debug::GlobalInstanceDebugLog,
    event::GlobalInstanceEvent,
    mumble::GlobalInstanceMumble,
    pack::{
        GlobalInstancePack,
        GlobalInstancePackAssets,
        GlobalInstanceSpace,
        GlobalInstanceWorld,
        InstanceTableCategory,
        InstanceTablePoi,
        InstanceTableTrail,
        PackInstanceHandle,
    },
    persist::PersistInstanceStore,
    require::PackageApi,
    semver::{GlobalInstanceVersion, VersionInstanceSemVer},
    timespan::ITimeSpan,
    ui::{GlobalInstanceMenu, GlobalInstanceUiX, UiInstanceMenu},
    vectors::{IColour, ISize2, IVec2, IVec3},
};

pub type LuaGuid = Guid;
/// TODO: accept a handle too
pub type LuaCategory = LuaString;

#[inline]
pub fn lua2do() -> LuaError {
    anyhow2lua(anyhow!("TODO"))
}
/// TODO: e.into_boxed_dyn_error() once anyhow updates...
pub fn anyhow2lua(e: anyhow::Error) -> LuaError {
    let e: Box<dyn std::error::Error + Send + Sync> = e.into();
    LuaError::ExternalError(Arc::from(e))
}
pub fn to_lua_error(e: ScriptError) -> LuaError {
    if e.downcast_ref::<LuaError>().is_none() {
        return anyhow2lua(e)
    }
    let context = if let Some(s) = e.downcast_ref::<String>() {
        Some(s.clone())
    } else if let Some(s) = e.downcast_ref::<&str>() {
        Some((*s).into())
    } else {
        None
    };
    let e = match e.downcast::<LuaError>() {
        Ok(e) => e,
        #[cfg(todo = "unnecessary")]
        Err(e) => {
            // source and e.chain() might be fine as-is?
            let s = e.source().and_then(|s| s.downcast_ref::<LuaError>());
            if let Some(s) = s {
                s.clone()
            } else {
                return anyhow2lua(e)
            }
        },
        Err(e) => return anyhow2lua(e),
    };
    match context {
        Some(context) => LuaError::WithContext { context, cause: e.into() },
        None => e,
    }
}

/// TODO: share a singleton Rc/Arc and make these normal lua functions instead of relying on userdata,
/// then construct the pathing-compat api layer in lua using a more 1-to-1 backend
pub struct ScriptApiTable<A: ?Sized, T> {
    _api: PhantomData<A>,
    api: T,
}
impl<A: ?Sized, T> Borrow<T> for ScriptApiTable<A, T> {
    #[inline(always)]
    fn borrow(&self) -> &T {
        &self.api
    }
}
pub struct GlobalInstanceConstructors;
impl<T> UserData for ScriptApiTable<GlobalInstanceConstructors, T>
where
    T: 'static,
{
    fn register(reg: &mut UserDataRegistry<Self>) {
        Self::register_constructors(reg)
    }
}
impl<T> ScriptApiTable<GlobalInstanceConstructors, T>
where
    T: 'static,
{
    fn register_constructors<U>(reg: &mut UserDataRegistry<U>)
    where
        U: Borrow<T>,
    {
        IVec3::register_constructor(reg);
        IColour::<Colour>::register_constructor(reg);
        Guid::register_constructor(reg);
    }
}

pub struct LuaPack;
pub struct RuntimeLua {
    lua: Lua,
}

pub struct UnsafeRuntime;
impl RuntimeLua {
    #[inline]
    pub fn with_lua(lua: Lua) -> Self {
        Self { lua }
    }
    pub fn new_script_runtime(opts: LuaOptions, unsecured: Option<UnsafeRuntime>) -> LuaResult<Self> {
        match unsecured {
            None => Lua::new_with(StdLib::TABLE | StdLib::STRING | StdLib::MATH, opts),
            Some(UnsafeRuntime) => Ok({
                let lua = unsafe { Lua::unsafe_new_with(StdLib::ALL, opts) };
                lua.set_app_data(UnsafeRuntime);
                lua
            }),
        }
        .map(Self::with_lua)
    }

    pub fn is_unsecured(&self) -> Option<UnsafeRuntime> {
        Self::lua_is_unsecured(&self.lua)
    }
    pub fn lua_is_unsecured(lua: &Lua) -> Option<UnsafeRuntime> {
        match lua.app_data_ref::<UnsafeRuntime>() {
            Some(..) => Some(UnsafeRuntime),
            None => None,
        }
    }

    /// TODO
    pub fn pack_globals_shared(&self) -> Table {
        self.lua.globals()
    }

    #[cfg(todo)]
    pub fn load_pack_entrypoint(&self) -> () {}

    pub fn lua_concat2<T>(lua: &Lua, v0: impl IntoLua, v1: impl IntoLua) -> LuaResult<T>
    where
        T: FromLua,
    {
        unsafe {
            lua.exec_raw::<(T,)>((v0, v1), |lua| {
                mlua::ffi::lua_concat(lua, 2);
            })
        }
        .map(|(r,)| r)
    }
    /// [Self::lua_concat2] but tostring() both arguments prior
    ///
    /// TODO: just take an iter and calculate offsets!
    pub fn lua_concat2_tostring(lua: &Lua, v0: impl IntoLua, v1: impl IntoLua) -> LuaResult<LuaString> {
        unsafe {
            lua.exec_raw::<(LuaString,)>((v0, v1), |lua| {
                if mlua::ffi::lua_isstring(lua, -2) == 0
                    && mlua::ffi::luaL_getmetafield(lua, -2, METAMETHOD_TOSTRING_C.as_ptr()) != 0
                {
                    mlua::ffi::lua_pushvalue(lua, -3);
                    mlua::ffi::lua_call(lua, 1, 1);
                    // pop result and swap onto -2
                    mlua::ffi::lua_replace(lua, -3);
                }
                if mlua::ffi::lua_isstring(lua, -1) == 0
                    && mlua::ffi::luaL_getmetafield(lua, -1, METAMETHOD_TOSTRING_C.as_ptr()) != 0
                {
                    match () {
                        #[cfg(todo = "unnecessary")]
                        _ => {
                            mlua::ffi::lua_pushvalue(lua, -2);
                            mlua::ffi::lua_call(lua, 1, 1);
                            mlua::ffi::lua_replace(lua, -2);
                        },
                        _ => {
                            mlua::ffi::lua_rotate(lua, -2, 1);
                            mlua::ffi::lua_call(lua, 1, 1);
                        },
                    }
                }
                mlua::ffi::lua_concat(lua, 2);
            })
        }
        .map(|(r,)| r)
    }
    pub fn lua_tostring(lua: &Lua, v: impl IntoLua, typename_fallback: bool) -> LuaResult<LuaString> {
        unsafe {
            lua.exec_raw::<(LuaString,)>((v,), |lua| {
                let mut ty = mlua::ffi::lua_type(lua, -1);
                loop {
                    match ty {
                        mlua::ffi::LUA_TSTRING => return,
                        mlua::ffi::LUA_TNUMBER => {
                            // XXX: LuaString::from_lua will lua_tostring but ideally want to avoid re-entry...
                            mlua::ffi::lua_tostring(lua, -1);
                            return
                        },
                        _ => (),
                    }
                    if mlua::ffi::luaL_getmetafield(lua, -1, METAMETHOD_TOSTRING_C.as_ptr()) == 0 {
                        break
                    }
                    mlua::ffi::lua_rotate(lua, -2, 1);
                    mlua::ffi::lua_call(lua, 1, 1);
                    ty = mlua::ffi::lua_type(lua, -1);
                }
                // XXX: Lua::coerce_string/LuaString::from_lua does not handle nil/bool, nor tables like `tostring()` would
                let typename = match ty {
                    _ if !typename_fallback => None,
                    mlua::ffi::LUA_TUSERDATA => Some(c"userdata"),
                    mlua::ffi::LUA_TLIGHTUSERDATA => Some(c"lightuserdata"),
                    mlua::ffi::LUA_TTABLE => Some(c"table"),
                    mlua::ffi::LUA_TNIL => Some(c"nil"),
                    mlua::ffi::LUA_TBOOLEAN => Some(match mlua::ffi::lua_toboolean(lua, -1) {
                        0 => c"false",
                        _ => c"true",
                    }),
                    mlua::ffi::LUA_TTHREAD => Some(c"thread"),
                    mlua::ffi::LUA_TFUNCTION => Some(c"function"),
                    // string/number already early return above
                    _ => None,
                };
                if let Some(typename) = typename {
                    mlua::ffi::lua_pop(lua, 1);
                    mlua::ffi::lua_pushstring(lua, typename.as_ptr());
                }
            })
        }
        .map(|(s,)| s)
    }
    /// NOTE: if __tostring metamethod isn't also defined, this seems likely to explode the stack
    /// (TODO: use a "raw" add or buffer etc instead?)
    pub fn imp_concat_tostring(lua: &Lua, (lhs, rhs): (LuaValue, LuaValue)) -> LuaResult<LuaString> {
        #[cfg(todo)]
        if !lhs.is_string() && !rhs.is_string() {
            return Err("expected string?")
        }
        super::RuntimeLua::lua_concat2_tostring(lua, lhs, rhs)
    }

    pub fn debug_display<'a>(&'a self, v: impl IntoLua + 'a) -> impl fmt::Display + 'a {
        Self::lua_debug_display(&self.lua, v)
    }
    pub fn lua_debug_display<'a>(lua: &'a Lua, v: impl IntoLua + 'a) -> impl fmt::Display + 'a {
        let v = v.into_lua(lua);
        lazyfmt::fmt_fn(move |f| {
            match &v {
                Err(e) => write!(f, "{e}"),
                Ok(LuaValue::Error(e)) => write!(f, "{e}"),
                Ok(LuaValue::Nil) => write!(f, "nil"),
                Ok(LuaValue::Integer(v)) => write!(f, "{v}"),
                Ok(LuaValue::Number(v)) => write!(f, "{v}"),
                Ok(LuaValue::Boolean(v)) => write!(f, "{v}"),
                Ok(LuaValue::String(v)) => write!(f, "{v:?}"),
                Ok(LuaValue::Function(v)) => write!(f, "{v:?}"),
                Ok(LuaValue::Thread(v)) => write!(f, "{v:?}"),
                Ok(LuaValue::Table(v)) => {
                    let mt = v.metatable();
                    let is_string = mt
                        .as_ref()
                        .map(|mt| match mt.raw_get::<LuaValue>(MetaMethod::ToString.name()) {
                            Err(..) | Ok(LuaValue::Nil) => false,
                            _ => true,
                        })
                        .unwrap_or(false);
                    let as_string = is_string.then(|| Self::lua_tostring(lua, v, false));
                    if let Some(Ok(v)) = as_string {
                        write!(f, "{}", v.display())
                    } else if Self::table_is_array(v) {
                        f.debug_list()
                            .entries(v.pairs::<DiscardValue, LuaValue>().map(|pair| {
                                let v = match pair {
                                    Ok((_, v)) => v,
                                    Err(e) => LuaValue::Error(e.into()),
                                };
                                lazyfmt::fmt_fn(move |f| {
                                    fmt::Display::fmt(&Self::lua_debug_display(lua, &v), f)
                                })
                            }))
                            .finish()
                    } else {
                        f.debug_map()
                            .entries(v.pairs::<LuaValue, LuaValue>().map(|pair| {
                                let (k, v) = match pair {
                                    Ok(pair) => pair,
                                    Err(e) => (LuaValue::Nil, LuaValue::Error(e.into())),
                                };
                                (
                                    lazyfmt::fmt_fn(move |f| {
                                        fmt::Display::fmt(&Self::lua_debug_display(lua, &k), f)
                                    }),
                                    lazyfmt::fmt_fn(move |f| {
                                        fmt::Display::fmt(&Self::lua_debug_display(lua, &v), f)
                                    }),
                                )
                            }))
                            .finish()
                    }
                    // TODO: check for debug print metamethod, tostring, etc!
                },
                Ok(LuaValue::UserData(ud)) => {
                    let mt = ud.metatable();
                    let is_string = mt
                        .as_ref()
                        .map(|mt| match mt.get::<LuaValue>(MetaMethod::ToString.name()) {
                            Err(..) | Ok(LuaValue::Nil) => false,
                            _ => true,
                        })
                        .unwrap_or(false);
                    let as_string = is_string.then(|| Self::lua_tostring(lua, ud, false));
                    if let Some(Ok(v)) = as_string {
                        write!(f, "{}", v.display())
                    } else {
                        write!(f, "{ud:?}")
                    }
                },
                Ok(LuaValue::LightUserData(ud)) =>
                    if let Ok(v) = Self::lua_tostring(lua, *ud, false) {
                        write!(f, "{}", v.display())
                    } else {
                        write!(f, "{ud:?}")
                    },
                v => write!(f, "{v:?}"),
            }
        })
    }
    pub fn table_is_array(t: &Table) -> bool {
        let mut empty = true;
        for (pair, exp) in t.pairs::<usize, DiscardValue>().zip(1usize..) {
            empty = false;
            let Ok((i, _)) = pair else { return false };
            if i != exp {
                return false
            }
        }
        !empty
    }
}

impl RuntimeLua {
    pub const PACK_ENTRYPOINT: &'static str = "pack.lua";
    pub const KEY_API_PACK: &'static str = "Pack";
    pub const KEY_API_INSTANCE: &'static str = "I";
    pub const KEY_API_DEBUG: &'static str = "Debug";
    pub const KEY_API_PATHING: &'static str = "Pathing";
    pub const KEY_API_MUMBLE: &'static str = "Mumble";
    pub const KEY_API_MENU: &'static str = "Menu";
    pub const KEY_API_WORLD: &'static str = "World";
    pub const KEY_API_CATEGORY: &'static str = "Category";
    pub const KEY_API_TAIMI: &'static str = "Taimi";
    pub const KEY_VERSION_PATHING: &'static str = "PathingVersion";
    pub const KEY_VERSION_TAIMI: &'static str = "TaimiVersion";
    pub const KEY_VERSION_TAIMI_API: &'static str = "TaimiApiVersion";

    #[inline(always)]
    pub fn lua(&self) -> &Lua {
        &self.lua
    }

    pub fn setup_api_rt(&self) -> LuaResult<()> {
        self.preload_builtin("@taimi/core/rt", ApiCoreRt)
    }
    pub fn new_api_log<A>(api: A) -> impl IntoLua
    where
        A: ScriptApiDebugLog + 'static,
    {
        ScriptApiTable {
            _api: PhantomData::<GlobalInstanceDebugLog>,
            api,
        }
    }
    pub fn setup_api_log<A>(&self, api: A) -> LuaResult<()>
    where
        A: ScriptApiDebugLog + 'static,
    {
        self.preload_builtin("@taimi/core/log", Self::new_api_log(api))
    }
    pub fn new_api_version<A: ScriptApiVersion + 'static>(api: A) -> impl IntoLua {
        ScriptApiTable {
            _api: PhantomData::<GlobalInstanceVersion>,
            api,
        }
    }
    pub fn new_api_version_str<A>(api: A) -> impl IntoLua
    where
        A: ScriptApiVersionString + 'static,
    {
        ScriptApiTable {
            _api: PhantomData::<VersionInstanceSemVer>,
            api,
        }
    }
    pub fn setup_api_version<A>(&self, api: A) -> LuaResult<()>
    where
        A: ScriptApiVersion + 'static,
    {
        self.preload_builtin("@taimi/core/version", Self::new_api_version(api))
    }
    pub fn new_api_attrs<A>(api: A) -> impl IntoLua
    where
        A: ScriptApiMarkerAttrs + 'static,
    {
        ScriptApiTable {
            _api: PhantomData::<GlobalInstanceAttrs>,
            api,
        }
    }
    pub fn setup_api_attrs(&self) -> LuaResult<()> {
        let api = Self::new_api_attrs(PhantomData::<attributes::MarkerAttrSet>);
        self.preload_builtin("@taimi/core/attrs", api)
    }

    pub fn new_api_persist_store<A>(api: A) -> impl IntoLua
    where
        A: ScriptApiStorage + 'static,
    {
        ScriptApiTable {
            _api: PhantomData::<PersistInstanceStore>,
            api,
        }
    }
    #[cfg(todo)]
    pub fn setup_api_storage<A>(&self, api: A) -> LuaResult<()>
    where
        A: ScriptApiStorage + 'static,
    {
        self.preload_builtin("@taimi/core/persist/store", Self::new_api_storage(api))
    }

    pub fn new_api_event<A>(api: A) -> impl IntoLua
    where
        A: ScriptApiEvent + 'static,
    {
        ScriptApiTable {
            _api: PhantomData::<GlobalInstanceEvent>,
            api,
        }
    }
    pub fn setup_api_event<A>(&self, api: A) -> LuaResult<()>
    where
        A: ScriptApiEvent + 'static,
    {
        self.preload_builtin("@taimi/core/event", Self::new_api_event(api))
    }
    pub fn new_api_bindings<GameControls, Control, I>(all_controls: I) -> impl IntoLua
    where
        I: IntoIterator<Item = Control>,
        I::IntoIter: 'static,
        Control: UserData + IntoLua + Clone + 'static,
        GameControls: UserData + 'static,
    {
        let all_controls = all_controls.into_iter().collect::<Vec<_>>();
        IntoLuaFn::new(move |lua| {
            let all_controls = all_controls.into_iter().map(|c| {
                let label = {
                    let label = c.clone();
                    IntoLuaFn::new(|lua| RuntimeLua::lua_tostring(lua, label, false).map(LuaValue::String))
                };
                (label, c)
            });
            lua.create_table_from([
                ("Control", LuaValue::UserData(lua.create_proxy::<Control>()?)),
                (
                    "GameControls",
                    LuaValue::UserData(lua.create_proxy::<GameControls>()?),
                ),
                (
                    "ControlNames",
                    LuaValue::Table(lua.create_table_from(all_controls)?),
                ),
            ])
            .map(LuaValue::Table)
        })
    }
    pub fn setup_api_bindings<GameControls, Control, I>(&self, all_controls: I) -> LuaResult<()>
    where
        I: IntoIterator<Item = Control>,
        I::IntoIter: 'static,
        Control: UserData + IntoLua + Clone + 'static,
        GameControls: UserData + 'static,
    {
        self.preload_builtin(
            "@taimi/core/bindings",
            Self::new_api_bindings::<GameControls, Control, I::IntoIter>(all_controls.into_iter()),
        )
    }
    pub fn new_api_ui_menu<A, H>() -> impl IntoLua + 'static
    where
        A: MenuInstance + 'static,
        A::RegisteredMenu: IntoUserHandle,
        A::Menu: IntoUserHandle,
        H: MenuHandleMut + 'static,
    {
        IntoLuaTable([
            (
                "Menu",
                LuaProxyOf::<ScriptApiTable<GlobalInstanceMenu, A>>::DEFAULT.type_erased(),
            ),
            (
                "MenuHandle",
                LuaProxyOf::<ScriptApiTable<UiInstanceMenu, H>>::DEFAULT.type_erased(),
            ),
        ])
    }
    pub fn setup_api_ui_menu<A, H>(&self) -> LuaResult<()>
    where
        A: MenuInstance + 'static,
        A::RegisteredMenu: IntoUserHandle,
        A::Menu: IntoUserHandle,
        H: MenuHandleMut + 'static,
    {
        self.preload_builtin("@taimi/core/ui/menu", Self::new_api_ui_menu::<A, H>())
    }

    pub fn new_api_ui_exchange<A>(api: A) -> impl IntoLua
    where
        A: ScriptApiUser + 'static,
    {
        ScriptApiTable {
            _api: PhantomData::<GlobalInstanceUiX>,
            api,
        }
    }
    pub fn new_instance_menu_root<A>(api: A) -> impl IntoLua
    where
        A: MenuInstance + 'static,
        A::RegisteredMenu: IntoUserHandle,
        A::Menu: IntoUserHandle,
    {
        ScriptApiTable {
            _api: PhantomData::<GlobalInstanceMenu>,
            api,
        }
    }
    pub fn new_instance_menu_mut<A>(api: A) -> impl IntoLua
    where
        A: MenuHandleMut + 'static,
    {
        ScriptApiTable {
            _api: PhantomData::<UiInstanceMenu>,
            api,
        }
    }
    pub fn setup_api_ui_exchange<A>(&self, api: A) -> LuaResult<()>
    where
        A: ScriptApiUser + 'static,
    {
        self.preload_builtin("@taimi/core/ui/exchange", Self::new_api_ui_exchange(api))
    }

    /// TODO: flatten to remove clone requirement
    pub fn new_api_mumble<A>(api: A) -> impl IntoLua
    where
        A: ScriptApiMumble + Clone + 'static,
    {
        let mumble = ScriptApiTable {
            _api: PhantomData::<GlobalInstanceMumble>,
            api,
        };
        IntoLuaFn::new(move |lua| {
            lua.create_table_from([(Self::KEY_API_MUMBLE, mumble)])
                .map(LuaValue::Table)
        })
    }
    pub fn setup_api_mumble<A>(&self, api: A) -> LuaResult<()>
    where
        A: ScriptApiMumble + Clone + 'static,
    {
        self.preload_builtin("@taimi/core/mumblelink", Self::new_api_mumble(api))
    }
    /// TODO: reorg...
    fn new_api_instance_constructors<A>(api: A) -> impl IntoLua
    where
        A: 'static,
    {
        ScriptApiTable {
            _api: PhantomData::<GlobalInstanceConstructors>,
            api,
        }
    }
    pub fn setup_api_vectors(&self) -> LuaResult<()> {
        let api = ();
        self.preload_builtin("@taimi/core/vectors", Self::new_api_instance_constructors(api))
    }
    pub fn new_api_root_category<A>(api: A) -> impl IntoLua
    where
        A: pathing::CategoryHandleMut + 'static,
    {
        ScriptApiTable {
            _api: PhantomData::<InstanceTableCategory>,
            api,
        }
    }
    pub fn new_api_pack_assets<A>(api: A) -> impl IntoLua
    where
        A: ScriptApiPackAssets + 'static,
    {
        ScriptApiTable {
            _api: PhantomData::<GlobalInstancePackAssets>,
            api,
        }
    }
    pub fn new_api_pack_info<A>(api: A, globals: Table) -> impl IntoLua
    where
        A: ScriptApiPack + 'static,
        for<'a> A::PackAssets<'a>: 'static,
        for<'a> A::PackStore<'a>: 'static,
        for<'a> A::PackWorld<'a>: 'static,
        for<'a> A::PackSpace<'a>: 'static,
        for<'a> A::PackMenu<'a>: MenuInstance + 'static,
        for<'a> <A::PackMenu<'a> as MenuInstance>::Menu: IntoUserHandle + 'static,
        for<'a> <A::PackMenu<'a> as MenuInstance>::RegisteredMenu: IntoUserHandle + 'static,
        <A::Pack as pathing::PackHandle>::RootCategory: IntoUserHandle + 'static,
        A::Pack: pathing::PackHandleMut + 'static,
    {
        IntoLuaFn::new(move |lua| {
            let api = ScriptApiTable {
                _api: PhantomData::<GlobalInstancePack>,
                api,
            }
            .into_lua(lua)?;
            if let Some(ud) = api.as_userdata() {
                ud.set_named_user_value(Self::PACK_INFO_USERDATA_GLOBALS, globals)?;
            }
            Ok(api)
        })
    }
    pub const PACK_INFO_USERDATA_GLOBALS: &'static str = "pack_globals";
    pub const PACK_HANDLE_USERDATA_INFO: &'static str = "pack_info";
    pub fn new_api_pack<A>(api: A) -> impl IntoLua
    where
        //A: ScriptApiPack + 'static, A::Pack: pathing::PackHandleMut,
        A: pathing::PackHandleMut + 'static,
    {
        ScriptApiTable {
            _api: PhantomData::<PackInstanceHandle>,
            api,
        }
    }
    pub fn new_api_world<A>(api: A) -> impl IntoLua
    where
        A: ScriptApiLookup + 'static,
        //A::Pack: pathing::PackHandleMut,
    {
        ScriptApiTable {
            _api: PhantomData::<GlobalInstanceWorld>,
            api,
        }
    }
    pub fn new_api_space<A>(api: A) -> impl IntoLua
    where
        A: ScriptApiSpaceQuery + 'static,
    {
        ScriptApiTable {
            _api: PhantomData::<GlobalInstanceSpace>,
            api,
        }
    }
    pub fn new_instance_category_mut<A>(api: A) -> impl IntoLua
    where
        A: pathing::CategoryHandleMut + 'static,
    {
        ScriptApiTable {
            _api: PhantomData::<InstanceTableCategory>,
            api,
        }
    }
    pub fn new_instance_poi_mut<A>(api: A) -> impl IntoLua
    where
        A: pathing::PoiHandleMut + 'static,
        A::Guid: IntoLua,
    {
        ScriptApiTable {
            _api: PhantomData::<InstanceTablePoi>,
            api,
        }
    }
    pub fn new_instance_trail_mut<A>(api: A) -> impl IntoLua
    where
        A: pathing::TrailHandleMut + 'static,
        A::Guid: IntoLua,
    {
        ScriptApiTable {
            _api: PhantomData::<InstanceTableTrail>,
            api,
        }
    }

    /// TODO?
    pub fn embedded_env(&self) -> Option<Table> {
        match self.is_unsecured() {
            Some(UnsafeRuntime) => None,
            None => None,
        }
    }

    pub fn prepare_script_attr_args(
        &self,
        name: &str,
        mut attr: &[u8],
        env: Table,
    ) -> Result<(LuaString, LuaFunction)> {
        if !attr.ends_with(b")") && attr.contains(&b'(') {
            log::warn!("unterminated!");
        }
        while let Some(body) = attr.strip_suffix(b")") {
            attr = body;
        }
        let mut split = attr.splitn(2, |&c| c == b'(');
        let fname = split.next().ok_or_else(|| super::format_err!("idfk"))?;
        let args = match split.next() {
            Some(a) => a,
            None => &[],
        };

        let args = if let Some(guid) = regex::bytes::Regex::new("[A-Za-z0-9/=+]{20,24}")
            .unwrap()
            .captures(args)
        {
            unsafe {
                format!(
                    "\"{}\"{}",
                    str::from_utf8_unchecked(guid.get(0).unwrap().as_bytes()),
                    str::from_utf8_unchecked(&args[guid.get(0).unwrap().len()..])
                )
            }
        } else {
            unsafe { str::from_utf8_unchecked(args) }.into()
        };
        let args = format!("return {args}");
        log::warn!("got: {args:?}");

        let args = self
            .lua
            .load(args)
            .set_environment(env)
            .set_mode(mlua::ChunkMode::Text)
            .set_name(name)
            .into_function()?;

        Ok((self.lua.create_string(fname)?, args))
    }
}

#[cfg(todo)]
impl RefAttrs for Table {
    fn to_iterable_dyn<'rt>(
        &'_ self,
        ctx: RuntimeCtx<'rt, '_>,
    ) -> Result<ScriptIterableOf<'rt, (Value<'rt>, Value<'rt>)>> {
        ctx.try_alloc(self.pairs()).map(Into::into)
    }

    fn cloned_attrs_dyn<'rt>(&self, ctx: RuntimeCtx<'rt, '_>) -> Result<ScriptRef<dyn RefAttrs + 'rt>> {
        ctx.try_alloc(self.clone()).map(Into::into)
    }

    fn cloned_to_attrs<'rt>(&self, ctx: RuntimeCtx<'rt, '_>) -> Result<Attrs<'rt>> {
        self.pairs()
            .map(|pair| {
                let (k, v) = pair?;
                Ok((k.into(), v.cloned_value_dyn()?))
            })
            .collect();
    }
}
#[cfg(todo)]
impl RefObject for Table {
    fn cloned_value_dyn<'rt>(&self, ctx: RuntimeCtx<'rt, '_>) -> Result<Value<'rt>> {
        self.cloned_value(ctx)
    }
    #[cfg(todo)]
    fn consume(&mut self, ctx: RefCtx<'_>) -> Result<()> {
        *self = ctx.as_lua()?.empty_table()
    }
}
#[cfg(todo)]
impl RefValue for Table {
    fn as_attrs<'rt, 'a>(&'a self, _ctx: RuntimeCtx<'rt, 'a>) -> Option<&'a DynAttrs<'rt>>
    where
        'rt: 'a,
    {
        Some(self)
    }
    fn cloned_value<'rt>(&self, ctx: RuntimeCtx<'rt, '_>) -> Result<Value<'rt>> {
        ctx.try_alloc(self.clone()).map(Into::into)
    }
    fn hash_dyn(&self, mut hasher: &mut dyn Hasher, _ctx: Option<RefCtx<'_>>) -> Result<()> {
        hasher.write_usize(self.raw_len());
        for pair in self.pairs() {
            let (k, v) = pair?;
            k.hash(&mut hasher);
            v.hash(&mut hasher);
        }
        Ok(())
    }
}
impl ScriptUserAttrs for Table {
    type AttrsKey = LuaString;
    type AttrsValue = LuaValue;
    type AttrsIntoIter<'a> = Box<dyn Iterator<Item = Result<(Self::AttrsKey, Self::AttrsValue)>> + 'a>;
    #[cfg(todo)]
    type AttrsIntoIter<'a> = mlua::TablePairs<'a, Self::AttrsKey, Self::AttrsValue>;
    fn iter_user_attrs(&self) -> Self::AttrsIntoIter<'_> {
        let attrs = self.pairs().map(|v| v.map_err(From::from));
        Box::new(attrs) as Box<_>
    }
}
impl ScriptSourceTag for Table {
    fn user_src(&self) -> Option<SourceTag> {
        todo!()
    }
}

impl ScriptSourceTag for LuaValue {
    fn user_src(&self) -> Option<SourceTag> {
        todo!()
    }
}
impl ScriptSourceTag for MultiValue {
    fn user_src(&self) -> Option<SourceTag> {
        todo!()
    }
}
impl ScriptSourceTag for LuaThread {
    fn user_src(&self) -> Option<SourceTag> {
        todo!()
    }
}
impl ScriptSourceTag for AnyUserData {
    fn user_src(&self) -> Option<SourceTag> {
        todo!()
    }
}

#[cfg(todo)]
impl ScriptValue for LuaPrimitive {}

impl ScriptUserUntyped for LuaValue {
    fn to_lua_value(&self) -> Option<LuaValue> {
        Some(self.clone())
    }
    fn to_lua_multi(&self) -> MultiValue {
        match self.clone() {
            LuaValue::Nil => MultiValue::new(),
            v => MultiValue::from_iter([v]),
        }
    }
}
impl ScriptUserUntyped for MultiValue {
    fn to_lua_value(&self) -> Option<LuaValue> {
        self.iter().next().cloned()
    }
    fn to_lua_multi(&self) -> MultiValue {
        self.clone()
    }
}

#[repr(transparent)]
pub struct DisplayLuaString(pub LuaString);
impl fmt::Display for DisplayLuaString {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0.display(), f)
    }
}
impl ScriptSourceTag for LuaString {
    fn user_src(&self) -> Option<SourceTag> {
        todo!()
    }
}
impl ScriptUserStr for LuaString {
    #[inline]
    fn with_str<R, F: FnOnce(&str) -> R>(&self, f: F) -> R {
        f(&self.to_string_lossy())
    }
}
impl ScriptUserStr for &'_ LuaString {
    #[inline]
    fn with_str<R, F: FnOnce(&str) -> R>(&self, f: F) -> R {
        ScriptUserStr::with_str(*self, f)
    }
}
impl ScriptUserGuid for LuaString {
    type GuidAsStr = Self;
    #[inline]
    fn user_guid_as_str(&self) -> Option<&Self::GuidAsStr> {
        Some(self)
    }
}
#[cfg(todo)]
impl RefStr for LuaString {
    fn as_display<'a, 'rt>(&'a self, ctx: BorrowedCtx<'rt, 'a>) -> Result<&'a dyn fmt::Display> {
        Ok(unsafe { mem::transmute::<&'a LuaString, &'a DisplayLuaString>(self) })
    }
    fn cloned_to_string(&self, ctx: RefCtx<'_>) -> Result<String> {
        Ok(self.to_string_lossy())
    }
    fn cloned_str_dyn<'rt>(&self, ctx: RuntimeCtx<'rt, '_>) -> Result<ScriptRefStr<'rt>> {
        ctx.try_alloc(self.clone()).map(Into::into)
    }
}
#[cfg(todo)]
impl RefObject for LuaString {
    fn cloned_value_dyn<'rt>(&self, ctx: RuntimeCtx<'rt, '_>) -> Result<Value<'rt>> {
        self.cloned_value(ctx)
    }
    #[cfg(todo)]
    fn consume(&mut self, ctx: RefCtx<'_>) -> Result<()> {
        *self = ctx.as_lua()?.empty_string()
    }
}
#[cfg(todo)]
impl RefValue for LuaString {
    fn as_str<'rt, 'a>(&'a self, ctx: RuntimeCtx<'rt, '_>) -> Option<&'a DynStr<'rt>> {
        Some(self)
    }
    fn cloned_value<'rt>(&self, ctx: RuntimeCtx<'rt, '_>) -> Result<Value<'rt>> {
        ctx.try_alloc(self.clone()).map(Into::into)
    }
    fn hash_dyn(&self, hasher: &mut dyn Hasher, _ctx: Option<RefCtx<'_>>) -> Result<()> {
        Ok(self.hash(hasher))
    }
}

impl ScriptUserHandle for Table {
    #[cfg(todo)]
    type Handle = LuaHandle<LuaValue>;
    type Handle = LuaHandle<Self>;
    fn clone_handle(&self) -> Self::Handle {
        LuaHandle(self.clone())
    }
    fn with_handle<R, F: FnOnce(&Self::Handle) -> R>(&self, f: F) -> R {
        f(LuaHandle::from_ref(self))
    }
    fn to_lua_value(&self) -> Option<LuaValue> {
        Some(LuaValue::Table(self.clone()))
    }
}
impl ScriptUserHandle for LuaString {
    type Handle = LuaHandle<Self>;
    fn clone_handle(&self) -> Self::Handle {
        LuaHandle(self.clone())
    }
    fn with_handle<R, F: FnOnce(&Self::Handle) -> R>(&self, f: F) -> R {
        f(LuaHandle::from_ref(self))
    }
    fn to_lua_value(&self) -> Option<LuaValue> {
        Some(LuaValue::String(self.clone()))
    }
}
impl ScriptUserHandle for &'_ LuaString {
    type Handle = LuaHandle<LuaString>;
    fn clone_handle(&self) -> Self::Handle {
        ScriptUserHandle::clone_handle(*self)
    }
    fn with_handle<R, F: FnOnce(&Self::Handle) -> R>(&self, f: F) -> R {
        ScriptUserHandle::with_handle(*self, f)
    }
    fn to_lua_value(&self) -> Option<LuaValue> {
        Some(LuaValue::String((*self).clone()))
    }
}
impl ScriptUserHandle for LuaValue {
    type Handle = LuaHandle<Self>;
    fn clone_handle(&self) -> Self::Handle {
        LuaHandle(self.clone())
    }
    fn with_handle<R, F: FnOnce(&Self::Handle) -> R>(&self, f: F) -> R {
        f(LuaHandle::from_ref(self))
    }
    fn to_lua_value(&self) -> Option<LuaValue> {
        Some(self.clone())
    }
}

#[derive(Debug, Clone, PartialEq)]
#[repr(transparent)]
pub struct LuaHandle<T = LuaValue>(pub T);
impl<T> LuaHandle<T> {
    #[inline(always)]
    pub const fn from_ref(v: &T) -> &Self {
        unsafe { mem::transmute(v) }
    }
}
#[cfg(todo)]
impl<T> PartialEq for LuaHandle<T> {
    fn eq(&self, rhs: &Self) -> bool {
        self.0.eq(&rhs.0)
    }
}
impl<T> Eq for LuaHandle<T> where Self: PartialEq {}
impl Hash for LuaHandle<Table> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_usize(self.0.raw_len());
        for pair in self.0.pairs() {
            #[cfg(debug_assertions)]
            let pair = pair.unwrap();
            #[cfg(not(debug_assertions))]
            let Ok(pair) = pair
            else {
                continue
            };
            let (k, v): (LuaValue, LuaValue) = pair;
            LuaHandle(k).hash(state);
            LuaHandle(v).hash(state);
        }
    }
}
impl Hash for LuaHandle<LuaString> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        if let Ok(s) = self.0.to_str() {
            s.hash(state)
        } else {
            self.0.as_bytes().hash(state)
        }
    }
}
impl Hash for LuaHandle<f64> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state)
    }
}
impl Hash for LuaHandle<LuaValue> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // TODO: prefixed type tag
        match self.0 {
            LuaValue::Table(ref v) => LuaHandle::from_ref(v).hash(state),
            LuaValue::String(ref v) => LuaHandle::from_ref(v).hash(state),
            LuaValue::Integer(v) => v.hash(state),
            LuaValue::Number(v) => LuaHandle(v).hash(state),
            LuaValue::Nil => ().hash(state),
            LuaValue::Boolean(v) => v.hash(state),
            LuaValue::Function(ref v) => v.to_pointer().hash(state),
            LuaValue::UserData(ref v) => v.to_pointer().hash(state),
            LuaValue::LightUserData(ref v) => v.0.hash(state),
            LuaValue::Error(ref v) => (&**v as *const LuaError as usize).hash(state),
            LuaValue::Thread(ref v) => v.to_pointer().hash(state),
            LuaValue::Other(..) => {
                log::warn!("unexpected LuaHandle type");
            },
        }
    }
}

#[cfg(todo = "unnecessary")]
impl IntoLua for Guid {}
/// TODO: intern these for shallow ptr cmp?
impl UserData for Guid {
    fn register(reg: &mut UserDataRegistry<Self>) {
        reg.add_method("ToBase64", |_lua, this, ()| {
            this.to_base_64().map_err(to_lua_error)
        });
        // TODO: how do I reuse/alias the method above? ><
        reg.add_meta_method(MetaMethod::ToString.name(), |_lua, this, ()| {
            this.to_base_64().map_err(to_lua_error)
        });
        reg.add_meta_function(MetaMethod::Concat.name(), RuntimeLua::imp_concat_tostring);
    }
}
impl Guid {
    fn register_constructor<U>(reg: &mut UserDataRegistry<U>) {
        reg.add_function("Guid", |_lua, (guid,): (LuaString,)| {
            Self::from_base_64(guid).map_err(to_lua_error)
        })
    }
}
impl FromLua for Guid {
    fn from_lua(value: LuaValue, lua: &Lua) -> LuaResult<Self> {
        match value {
            LuaValue::String(s) => Self::from_base_64(s).map_err(to_lua_error),
            v => UserDataRef::<Self>::from_lua(v, lua).map(|ud| (*ud).clone()),
        }
    }
}

struct HandleToLua<T>(pub T);
impl<T: IntoUserHandle> IntoLua for HandleToLua<T> {
    #[inline]
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        self.0.to_lua_handle(lua)
    }
}

pub struct IntoLuaFn<F>(pub F);
impl<F> IntoLuaFn<F> {
    #[inline(always)]
    pub const fn new(f: F) -> Self
    where
        F: FnOnce(&Lua) -> LuaResult<LuaValue>,
    {
        Self(f)
    }
    #[inline(always)]
    pub fn new_mut(f: F) -> Self
    where
        F: FnMut(&Lua) -> LuaResult<LuaValue>,
    {
        Self(f)
    }
    #[inline(always)]
    pub const fn new_multi(f: F) -> Self
    where
        F: FnOnce(&Lua) -> LuaResult<MultiValue>,
    {
        Self(f)
    }
    #[inline(always)]
    pub fn new_multi_mut(f: F) -> Self
    where
        F: FnMut(&Lua) -> LuaResult<MultiValue>,
    {
        Self(f)
    }
}
impl<F> IntoLua for IntoLuaFn<F>
where
    F: FnOnce(&Lua) -> LuaResult<LuaValue>,
{
    #[inline]
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        (self.0)(lua)
    }
}
#[cfg(todo)]
impl<F> IntoLuaMulti for IntoLuaFn<F>
where
    F: FnOnce(&Lua) -> LuaResult<MultiValue>,
{
    #[inline]
    fn into_lua_multi(self, lua: &Lua) -> LuaResult<MultiValue> {
        (self.0)(lua)
    }
}
impl<F> IntoLuaMut for IntoLuaFn<F>
where
    F: FnMut(&Lua) -> LuaResult<LuaValue>,
{
    #[inline]
    fn into_lua_mut(&mut self, lua: &Lua) -> LuaResult<LuaValue> {
        (self.0)(lua)
    }
}
impl<F> IntoLuaMultiMut for IntoLuaFn<F>
where
    F: FnMut(&Lua) -> LuaResult<MultiValue>,
{
    #[inline]
    fn into_lua_multi_mut(&mut self, lua: &Lua) -> LuaResult<MultiValue> {
        (self.0)(lua)
    }
}
pub struct ProxyAliasRO<T> {
    thunk: T,
}
impl<T> ProxyAliasRO<T> {
    pub fn new(thunk: T) -> Self {
        Self { thunk }
    }
}
impl<T> IntoLua for ProxyAliasRO<T>
where
    T: IntoLua + 'static,
{
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let mut thunk = Some(self.thunk);
        let mut value = None::<LuaValue>;
        let index_f = lua.create_function_mut(move |lua, (_t, key): (Table, LuaValue)| {
            let value = if let Some(thunk) = thunk.take() {
                &*value.insert(thunk.into_lua(lua)?)
            } else {
                value.as_ref().ok_or_else(|| anyhow2lua(anyhow!("empty alias")))?
            };
            match (value, key) {
                (LuaValue::Table(value), key) => value.get::<LuaValue>(key),
                (LuaValue::UserData(ud), key) => userdata_get(lua, ud, key),
                _ => Err(anyhow2lua(anyhow!("bad alias target"))),
            }
        })?;
        let newindex_f = lua.create_function(|_lua, _: DiscardValues| {
            Err::<LuaValue, _>(anyhow2lua(anyhow!("read-only alias")))
        })?;
        lua.create_table_from([
            (MetaMethod::Index.name(), index_f),
            (MetaMethod::NewIndex.name(), newindex_f),
        ])
        .map(LuaValue::Table)
    }
}

fn userdata_get<V: FromLua>(lua: &Lua, ud: &AnyUserData, key: LuaValue) -> LuaResult<V> {
    let metatable = ud.metatable()?;
    let Some(index) = metatable.get::<Option<LuaFunction>>(MetaMethod::Index.name())? else {
        return match key {
            LuaValue::String(key) => key.to_str().and_then(|key| metatable.get::<V>(key)),
            _ => V::from_lua(LuaValue::Nil, lua),
        }
    };
    index.call((ud, key))
}

pub struct DiscardValue;
impl FromLua for DiscardValue {
    #[inline(always)]
    fn from_lua(_: LuaValue, _lua: &Lua) -> LuaResult<Self> {
        Ok(Self)
    }
}
pub struct DiscardValues;
impl FromLuaMulti for DiscardValues {
    #[inline(always)]
    fn from_lua_multi(_: MultiValue, _lua: &Lua) -> LuaResult<Self> {
        Ok(Self)
    }
}
#[cfg(todo)]
impl IntoLua for Guid {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        self.id_to_str().into_lua(lua)
    }
}

/// TODO: account for __call metatable fn?
pub type LuaCallable = LuaFunction;

pub const METAMETHOD_METATABLE: &'static str = "__metatable";
pub const METAMETHOD_TOSTRING_C: &'static CStr = c"__tostring";

pub trait IntoLuaMut {
    fn into_lua_mut(&mut self, lua: &Lua) -> LuaResult<LuaValue>;
}
impl<T> IntoLuaMut for Option<T>
where
    T: IntoLua,
{
    fn into_lua_mut(&mut self, lua: &Lua) -> LuaResult<LuaValue> {
        self.take()
            .ok_or_else(|| anyhow2lua(anyhow!("unique value reused")))
            .and_then(|v| v.into_lua(lua))
    }
}
impl IntoLua for &'_ mut dyn IntoLuaMut {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        self.into_lua_mut(lua)
    }
}
impl IntoLua for &'_ mut Box<dyn IntoLuaMut> {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        IntoLuaMut::into_lua_mut(&mut **self, lua)
    }
}
impl IntoLua for Box<dyn IntoLuaMut> {
    fn into_lua(mut self, lua: &Lua) -> LuaResult<LuaValue> {
        IntoLuaMut::into_lua_mut(&mut *self, lua)
    }
}
pub trait IntoLuaMultiMut {
    fn into_lua_multi_mut(&mut self, lua: &Lua) -> LuaResult<MultiValue>;
}
impl<T> IntoLuaMultiMut for Option<T>
where
    T: IntoLuaMulti,
{
    fn into_lua_multi_mut(&mut self, lua: &Lua) -> LuaResult<MultiValue> {
        self.take()
            .ok_or_else(|| anyhow2lua(anyhow!("unique value reused")))
            .and_then(|v| v.into_lua_multi(lua))
    }
}
impl IntoLuaMulti for &'_ mut dyn IntoLuaMultiMut {
    fn into_lua_multi(self, lua: &Lua) -> LuaResult<MultiValue> {
        self.into_lua_multi_mut(lua)
    }
}
impl IntoLuaMulti for &'_ mut Box<dyn IntoLuaMultiMut> {
    fn into_lua_multi(self, lua: &Lua) -> LuaResult<MultiValue> {
        IntoLuaMultiMut::into_lua_multi_mut(&mut **self, lua)
    }
}
impl IntoLuaMulti for Box<dyn IntoLuaMultiMut> {
    fn into_lua_multi(mut self, lua: &Lua) -> LuaResult<MultiValue> {
        IntoLuaMultiMut::into_lua_multi_mut(&mut *self, lua)
    }
}

#[repr(transparent)]
pub struct LuaProxyOf<T>(pub PhantomData<T>);
impl<T> LuaProxyOf<T> {
    pub const DEFAULT: Self = Self(PhantomData);
}
impl<T> LuaProxyOf<T>
where
    T: UserData + 'static,
{
    #[inline]
    pub fn lua_proxy(lua: &Lua) -> LuaResult<AnyUserData> {
        lua.create_proxy::<T>()
    }
    pub fn lua_proxy_value(lua: &Lua) -> LuaResult<LuaValue> {
        Self::lua_proxy(lua).map(LuaValue::UserData)
    }

    #[inline]
    pub fn type_erased(self) -> IntoLuaFn<fn(&Lua) -> LuaResult<LuaValue>> {
        IntoLuaFn::new(Self::lua_proxy_value)
    }
}
impl<T> Copy for LuaProxyOf<T> {}
impl<T> Clone for LuaProxyOf<T> {
    #[inline(always)]
    fn clone(&self) -> Self {
        Self(PhantomData)
    }
}
impl<T> Default for LuaProxyOf<T> {
    #[inline(always)]
    fn default() -> Self {
        Self(PhantomData)
    }
}
impl<T> fmt::Debug for LuaProxyOf<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("LuaProxy").finish()
    }
}
impl<T> IntoLua for LuaProxyOf<T>
where
    T: UserData + 'static,
{
    #[inline]
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        lua.create_proxy::<T>().map(LuaValue::UserData)
    }
}

#[derive(Debug, Copy, Clone)]
pub struct IntoLuaTable<T>(pub T);
impl<T, K, V> IntoLua for IntoLuaTable<T>
where
    T: IntoIterator<Item = (K, V)>,
    K: IntoLua,
    V: IntoLua,
{
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        lua.create_table_from(self.0).map(LuaValue::Table)
    }
}
#[derive(Debug, Copy, Clone)]
pub struct IntoLuaArray<T>(pub T);
impl<T, V> IntoLua for IntoLuaArray<T>
where
    T: IntoIterator<Item = V>,
    V: IntoLua,
{
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        lua.create_sequence_from(self.0).map(LuaValue::Table)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct LuaSrc<S: ?Sized = str>(pub S);
impl<S: ?Sized> LuaSrc<S> {
    #[inline]
    pub const fn new(s: S) -> Self
    where
        S: Sized,
    {
        Self(s)
    }
    #[inline]
    pub const fn from_ref(s: &S) -> &Self {
        unsafe { mem::transmute(s) }
    }
}
impl<S> LuaSrc<S>
where
    S: ?Sized + AsRef<[u8]>,
{
    pub fn to_function(&self, lua: &Lua, name_context: Option<&str>) -> LuaResult<LuaFunction> {
        let chunk = lua.load(self.as_ref()).set_mode(ChunkMode::Text);
        match name_context {
            Some(n) => chunk.set_name(n),
            None => chunk,
        }
        .into_function()
    }
}
impl<S: ?Sized> ops::Deref for LuaSrc<S> {
    type Target = S;
    #[inline]
    fn deref(&self) -> &S {
        &self.0
    }
}
impl<S> AsRef<[u8]> for LuaSrc<S>
where
    S: ?Sized + AsRef<[u8]>,
{
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}
impl<S> AsRef<str> for LuaSrc<S>
where
    S: ?Sized + AsRef<str>,
{
    #[inline]
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}
impl<S> Borrow<str> for LuaSrc<S>
where
    S: ?Sized + Borrow<str>,
{
    #[inline]
    fn borrow(&self) -> &str {
        self.0.borrow()
    }
}
impl<S> Borrow<[u8]> for LuaSrc<S>
where
    S: ?Sized + Borrow<[u8]>,
{
    #[inline]
    fn borrow(&self) -> &[u8] {
        self.0.borrow()
    }
}
impl Borrow<LuaSrc<str>> for LuaSrc<String> {
    #[inline]
    fn borrow(&self) -> &LuaSrc<str> {
        LuaSrc::from_ref(&self[..])
    }
}
impl Borrow<LuaSrc<[u8]>> for LuaSrc<Vec<u8>> {
    #[inline]
    fn borrow(&self) -> &LuaSrc<[u8]> {
        LuaSrc::from_ref(&self[..])
    }
}
impl ToOwned for LuaSrc<str> {
    type Owned = LuaSrc<String>;
    #[inline]
    fn to_owned(&self) -> Self::Owned {
        LuaSrc::new(self.0.into())
    }
}
impl ToOwned for LuaSrc<[u8]> {
    type Owned = Box<LuaSrc<[u8]>>;
    #[inline]
    fn to_owned(&self) -> Self::Owned {
        Box::<[u8]>::from(&self.0).into()
    }
}
impl<S: ?Sized> From<Box<S>> for Box<LuaSrc<S>> {
    #[inline]
    fn from(s: Box<S>) -> Self {
        unsafe { mem::transmute(s) }
    }
}
