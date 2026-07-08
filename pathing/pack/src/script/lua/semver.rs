use {
    crate::script::{
        lua::{to_lua_error, DiscardValue, RuntimeLua, ScriptApiTable},
        pathing::{ScriptApiVersion, ScriptApiVersionString},
    },
    core::borrow::Borrow,
    mlua::{
        IntoLua,
        Lua,
        MetaMethod,
        Result as LuaResult,
        String as LuaString,
        UserData,
        UserDataFields,
        UserDataMethods,
        UserDataRegistry,
        Value as LuaValue,
    },
};

pub struct GlobalInstanceVersion;
impl<T> IntoLua for ScriptApiTable<GlobalInstanceVersion, T>
where
    T: ScriptApiVersion + 'static,
{
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        lua.create_table_from([
            (
                "Taimi",
                RuntimeLua::new_api_version_str(self.api.taimi_version().into_owned()).into_lua(lua)?,
            ),
            (
                "Api",
                RuntimeLua::new_api_version_str(self.api.taimi_api_version().into_owned()).into_lua(lua)?,
            ),
            (
                "Compat",
                RuntimeLua::new_api_version_str(self.api.blish_pathing_compat_version().into_owned())
                    .into_lua(lua)?,
            ),
            (
                "SemVer",
                LuaValue::UserData(lua.create_proxy::<ScriptApiTable<VersionInstanceSemVer, String>>()?),
            ),
        ])
        .map(LuaValue::Table)
    }
}

pub struct VersionInstanceSemVer;
impl<T> UserData for ScriptApiTable<VersionInstanceSemVer, T>
where
    T: ScriptApiVersionString + 'static,
{
    fn register(reg: &mut UserDataRegistry<Self>) {
        Self::register_version_string(reg)
    }
}
impl<T> ScriptApiTable<VersionInstanceSemVer, T>
where
    T: ScriptApiVersionString + 'static,
{
    fn register_version_string<U>(reg: &mut UserDataRegistry<U>)
    where
        U: Borrow<T>,
    {
        reg.add_field_method_get("version", |lua, this| {
            this.borrow().version_as_str()[..].into_lua(lua)
        });
        reg.add_method("IsVersionAtLeast", |_lua, this, (req,): (LuaString,)| {
            req.to_str()
                .and_then(|req| this.borrow().is_version_at_least(&req[..]).map_err(to_lua_error))
        });
        reg.add_method("Scrub", |lua, this, ()| {
            this.borrow().version_scrub_extra()[..].into_lua(lua)
        });
        reg.add_meta_method(MetaMethod::ToString.name(), |lua, this, ()| {
            this.borrow().version_as_str()[..].into_lua(lua)
        });
        // constructor...
        reg.add_function("New", |_lua, (_, v): (DiscardValue, LuaString)| {
            // TODO? RuntimeLua::new_api_version_str(v)
            v.to_str()
                .map(|v| RuntimeLua::new_api_version_str(String::from(&v[..])))
        });
    }
}

#[cfg(todo)]
impl ScriptApiVersionString for LuaString {}
