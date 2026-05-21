// lua 5.2+ allocates
#[cfg(todo)]
use mlua::ffi::LUA_PRELOAD_TABLE;
use {
    crate::script::lua::{
        anyhow2lua,
        to_lua_error,
        DiscardValues,
        IntoLuaFn,
        LuaCallable,
        RuntimeLua,
        METAMETHOD_METATABLE,
    },
    anyhow::{anyhow, Context},
    core::ffi::CStr,
    mlua::{
        ffi,
        BorrowedBytes,
        ChunkMode,
        FromLuaMulti,
        Function as LuaFunction,
        IntoLua,
        Lua,
        MetaMethod,
        MultiValue,
        Result as LuaResult,
        String as LuaString,
        Table,
        Value as LuaValue,
    },
    std::borrow::Cow,
};

impl RuntimeLua {
    /// TODO: ffi::LUA_REGISTERED_MODULES_TABLE on luau?
    #[doc(alias = "LUA_LOADED_TABLE")]
    pub const REGISTRY_LOADED_C: &CStr = c"_LOADED";
    #[doc(alias = "LUA_PRELOAD_TABLE")]
    pub const REGISTRY_PRELOAD_C: &CStr = c"_PRELOAD";
    pub const STD_PACKAGE_C: &CStr = c"package";
    pub const STD_PACKAGE: &str = "package";
    pub const STD_PACKAGE_LOADED: &str = "loaded";
    pub const STD_PACKAGE_PRELOAD: &str = "preload";
    pub const STD_PACKAGE_REQUIRE: &str = "require";
    pub const STD_PACKAGE_LOADERS: &str = "loaders";

    pub fn get_loaded(&self) -> LuaResult<Table> {
        self.named_registry_value(Self::REGISTRY_LOADED_C)
    }
    /// not strictly necessary but maintains compatibility with [Lua::preload_module]
    /// while also using the registry table regardless of lua version
    pub fn setup_package_preload(&self) -> LuaResult<Table> {
        let loaded = self.get_loaded()?;
        let preload = self.named_registry_value::<Option<Table>>(Self::REGISTRY_PRELOAD_C)?;
        let has_preload_registry = preload.is_some();
        let preload = match preload {
            Some(p) => Some(p),
            None => match loaded.get::<Option<Table>>(Self::STD_PACKAGE)? {
                None => None,
                Some(package) => Some(package.raw_get::<Table>(Self::STD_PACKAGE_PRELOAD)?),
            },
        };
        let preload = match preload {
            Some(p) => p,
            None => self.lua.create_table()?,
        };
        if !has_preload_registry {
            self.set_named_registry_value(Self::REGISTRY_PRELOAD_C, preload.clone())?;
        }
        Ok(preload)
    }
    pub fn setup_package_builtin(&self) -> LuaResult<()> {
        let preload = self.setup_package_preload()?;

        let globals = self.lua.globals();
        if globals.contains_key("require")? {
            // nothing to do, apparently the stdlib is present!
            return Ok(())
        }

        let loaded = self.get_loaded()?;

        let api = PackageApi::new(&self.lua, loaded.clone(), preload)?;
        let (lib, package, require) = api.into_parts(&self.lua)?;

        if !loaded.contains_key(Self::STD_PACKAGE)? {
            loaded.raw_set(Self::STD_PACKAGE, lib)?;
        }

        globals.raw_set(Self::STD_PACKAGE_REQUIRE, require)?;
        globals.raw_set(Self::STD_PACKAGE, package)?;

        Ok(())
    }
    #[inline]
    pub fn preload_builtin_fn<A, F>(&self, name: &'static str, loader: F) -> LuaResult<()>
    where
        F: FnOnce(&Lua, A) -> LuaResult<LuaValue> + 'static,
        A: FromLuaMulti,
    {
        let mut loader = Some(loader);
        self.lua
            .create_function_mut(move |lua, args: A| {
                let loader = loader
                    .take()
                    .with_context(|| format!("duplicate preload for module {name}"))
                    .map_err(to_lua_error)?;
                mlua::ErrorContext::with_context(loader(lua, args), |_| {
                    format!("loading built-in module {name}")
                })
            })
            .and_then(|f| self.lua.preload_module(name, f))
    }
    pub fn preload_builtin<A>(&self, name: &'static str, api: A) -> LuaResult<()>
    where
        A: IntoLua + 'static,
    {
        match () {
            #[cfg(todo)]
            //#[cfg(feature = "luau")]
            _ => {
                // TODO: or manual preload table setup, or lua.create_require_function(), or?
                self.lua.register_module(name, api)
            },
            _ => self.preload_builtin_fn(name, move |lua, _args: DiscardValues| api.into_lua(lua)),
        }
    }
    pub fn load_chunk_with(
        lua: &Lua,
        name: &'static str,
        data: &[u8],
        mode: ChunkMode,
        env: Option<Table>,
    ) -> LuaResult<LuaFunction> {
        let mut chunk = lua.load(data).set_mode(mode).set_name(name);
        if let Some(env) = env {
            chunk = chunk.set_environment(env);
        }
        chunk.into_function()
    }
    pub fn preload_embedded(&self, name: &'static str, data: Cow<'static, [u8]>) -> LuaResult<()> {
        let env = self.embedded_env();
        self.preload_builtin_fn(name, move |lua, (require_name, rest): (LuaString, MultiValue)| {
            let f = Self::load_chunk_with(lua, name, &data[..], ChunkMode::Text, env)?;
            f.call((require_name, rest))
        })
    }

    pub fn has_registry_value(&self, name: &CStr) -> bool {
        match () {
            #[cfg(todo = "unnecessary")]
            _ => unsafe {
                let mut present = false;
                self.lua
                    .exec_raw::<DiscardValues>((), |lua| {
                        present =
                            ffi::lua_getfield(lua, ffi::LUA_REGISTRYINDEX, name.as_ptr()) != ffi::LUA_TNIL;
                    })
                    .map(move |()| present)
            }
            .unwrap_or(false),
            _ => unsafe {
                self.lua.exec_raw_lua(|lua| {
                    let l = lua.state();
                    let present =
                        ffi::lua_getfield(l, ffi::LUA_REGISTRYINDEX, name.as_ptr()) != ffi::LUA_TNIL;
                    ffi::lua_pop(l, 1);
                    present
                })
            },
        }
    }
    pub fn named_registry_value<T: FromLuaMulti>(&self, name: &CStr) -> LuaResult<T> {
        // self.lua.named_registry_value(unsafe { str::from_utf8_unchecked(name.to_bytes()) })
        unsafe {
            self.lua.exec_raw::<T>((), |lua| {
                ffi::lua_getfield(lua, ffi::LUA_REGISTRYINDEX, name.as_ptr());
            })
        }
    }
    /// TODO: via ffi?
    pub fn set_named_registry_value<T: IntoLua>(&self, name: &CStr, v: T) -> LuaResult<()> {
        let key = unsafe { str::from_utf8_unchecked(name.to_bytes()) };
        self.lua.set_named_registry_value(key, v)
    }
    /// TODO: named_registry_value_or_insert(&T) where T: Clone?
    pub fn named_registry_table_or_init(&self, name: &CStr) -> LuaResult<Table> {
        match self.named_registry_value(name) {
            Ok(None) => self
                .lua
                .create_table()
                .and_then(|t| self.set_named_registry_value(name, t.clone()).map(move |()| t)),
            Ok(Some(t)) => Ok(t),
            Err(e) => Err(e),
        }
    }
}

pub struct PackageApi {
    pub loaded: Table,
    pub reg_loaded: Table,
    pub mt_loaded: Table,

    pub preload: Table,
    pub reg_preload: Table,
    pub mt_preload: Table,

    pub loaders: Table,
    pub mt_loaders: Table,
    pub protected: Table,
}
impl PackageApi {
    pub fn new(lua: &Lua, reg_loaded: Table, reg_preload: Table) -> LuaResult<Self> {
        let api = Self {
            loaded: lua.create_table()?,
            mt_loaded: lua.create_table()?,
            reg_loaded,
            reg_preload,
            preload: lua.create_table()?,
            mt_preload: lua.create_table()?,
            protected: lua.create_table()?,
            loaders: lua.create_table()?,
            mt_loaders: lua.create_table()?,
        };
        api.init().map(move |()| api)
    }

    fn init(&self) -> LuaResult<()> {
        self.mt_loaded
            .raw_set(METAMETHOD_METATABLE, self.protected.clone())?;
        self.mt_loaded
            .raw_set(MetaMethod::Index.name(), self.reg_loaded.clone())?;
        self.loaded.set_metatable(Some(self.mt_loaded.clone()))?;

        self.mt_preload
            .raw_set(METAMETHOD_METATABLE, self.protected.clone())?;
        self.mt_preload
            .raw_set(MetaMethod::Index.name(), self.reg_preload.clone())?;
        self.preload.set_metatable(Some(self.mt_preload.clone()))?;

        self.mt_loaders
            .raw_set(METAMETHOD_METATABLE, self.protected.clone())?;
        let readonly = IntoLuaFn::new(|lua| {
            lua.create_function(|_lua, DiscardValues| {
                Err::<(), _>(anyhow2lua(anyhow!("package.loaders unimplemented")))
            })
            .map(LuaValue::Function)
        });
        self.mt_loaders.raw_set(MetaMethod::NewIndex.name(), readonly)?;
        self.loaders.set_metatable(Some(self.mt_loaders.clone()))?;

        Ok(())
    }

    /// assumes `self.loaded` has already been checked
    ///
    /// TODO: you don't get trailing args with require nop
    /// TODO: fall back to `package.loaders`? aparently a loader can return a function that should be treated as the loader, and also return a string instead of failing with error
    pub fn require_load(&self, name: BorrowedBytes<'_>) -> LuaResult<LuaValue> {
        let loader = if let Some(preloader) = self.preload.get::<Option<LuaCallable>>(&name)? {
            Some(preloader)
        } else {
            None
        };
        let context = || {
            for p in self.preload.pairs() {
                let Ok(p) = p else { continue };
                let (k, v): (LuaValue, LuaValue) = p;
                log::debug!("preload[{k:?}] = {v:?}");
            }
            for p in self.reg_preload.pairs() {
                let Ok(p) = p else { continue };
                let (k, v): (LuaValue, LuaValue) = p;
                log::debug!("reg_preload[{k:?}] = {v:?}");
            }
            log::warn!(
                "preload[{name:?}] = {:?}",
                self.reg_preload.get::<Option<LuaCallable>>(&name)
            );
            let name = String::from_utf8_lossy(&name);
            format!("no loader found for package {name}")
        };
        let loader = loader.with_context(context).map_err(to_lua_error)?;

        let loaded: MultiValue = loader.call((&name,))?;
        let module = match loaded.into_iter().next() {
            None | Some(LuaValue::Nil) =>
                if let Some(loader_did_it) = self.loaded.get(&name)? {
                    loader_did_it
                } else {
                    self.reg_loaded.raw_set(&name, true)?;
                    LuaValue::Boolean(true)
                },
            Some(v) => {
                self.reg_loaded.raw_set(&name, v.clone())?;
                v
            },
        };
        // ditch the loader, don't unload your modules!
        if let Err(..) = self.preload.raw_remove(&name) {
            let _ = self.reg_preload.set(&name, mlua::Nil);
        }
        Ok(module)
    }

    /// `LUA_LOADED_TABLE["package"]`, `_G["package"]`, `_G["require"]`
    ///
    /// TODO: `_G.module()` and `package.seeall`
    pub fn into_parts(self, lua: &Lua) -> LuaResult<(Table, Table, LuaFunction)> {
        let (rl, rp) = (self.reg_loaded.clone(), self.reg_preload.clone());
        let (l, p) = (self.loaded.clone(), self.preload.clone());
        let loaders = self.loaders.clone();
        let api = lua.create_any_userdata(self)?;

        let require = lua.create_function({
            let api = api.clone();
            let l = l.clone();
            move |_lua, (name,): (BorrowedBytes,)| {
                if let Some(required) = l.get(&name)? {
                    // fast path for preloaded modules
                    return Ok(required)
                }
                api.borrow::<Self>().and_then(|api| api.require_load(name))
            }
        })?;

        let lib = lua.create_table_from([
            (RuntimeLua::STD_PACKAGE_LOADED, rl),
            (RuntimeLua::STD_PACKAGE_PRELOAD, rp),
        ])?;
        let package = lua.create_table_from([
            (RuntimeLua::STD_PACKAGE_LOADED, l),
            (RuntimeLua::STD_PACKAGE_PRELOAD, p),
            (RuntimeLua::STD_PACKAGE_LOADERS, loaders),
        ])?;
        Ok((lib, package, require))
    }
}
