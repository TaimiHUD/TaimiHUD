use {
    crate::script::{
        lua::{to_lua_error, IntoLuaFn, RuntimeLua, ScriptApiDebugLog, ScriptApiTable},
        user::ScriptUserStr,
        Result,
    },
    mlua::{IntoLua, Lua, MultiValue, Result as LuaResult, String as LuaString, Value as LuaValue},
};

pub struct GlobalInstanceDebugLog;
impl GlobalInstanceDebugLog {
    #[inline]
    pub fn named_fns<S, L>() -> [(&'static str, fn(&L, S) -> Result<()>); 5]
    where
        L: ?Sized + ScriptApiDebugLog,
        S: ScriptUserStr,
    {
        [
            ("Error", ScriptApiDebugLog::error as fn(&L, S) -> Result<()>),
            ("Warn", ScriptApiDebugLog::warn),
            ("Info", ScriptApiDebugLog::info),
            ("Debug", ScriptApiDebugLog::debug),
            ("Print", ScriptApiDebugLog::print),
        ]
    }
}
impl<T> IntoLua for ScriptApiTable<GlobalInstanceDebugLog, T>
where
    T: ScriptApiDebugLog + 'static,
{
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let this = lua.create_any_userdata(self.api)?;
        lua.create_table_from(
            IntoIterator::into_iter(GlobalInstanceDebugLog::named_fns::<LuaString, T>()).map(|(n, f)| {
                let this = this.clone();
                (
                    n,
                    IntoLuaFn::new(move |lua| {
                        lua.create_function(move |lua, msg: MultiValue| {
                            let mut msgs = msg.into_iter();
                            let mut msg =
                                RuntimeLua::lua_tostring(lua, msgs.next().unwrap_or(LuaValue::Nil), true)?;
                            while let Some(next) = msgs.next() {
                                msg = RuntimeLua::lua_concat2(
                                    lua,
                                    &msg,
                                    RuntimeLua::lua_tostring(lua, next, true)?,
                                )?;
                            }
                            this.borrow::<T>()
                                .and_then(move |api| f(&*api, msg).map_err(to_lua_error))
                        })
                        .map(LuaValue::Function)
                    }),
                )
            }),
        )
        .map(LuaValue::Table)
    }
}
#[cfg(todo)]
impl<T> UserData for ScriptApiTable<GlobalInstanceDebugLog, T>
where
    T: ScriptApiDebugLog + 'static,
{
    fn register(reg: &mut UserDataRegistry<Self>) {
        Self::register_debug_log(reg)
    }
}
#[cfg(todo)]
impl<T> ScriptApiTable<GlobalInstanceDebugLog, T>
where
    T: ScriptApiDebugLog + 'static,
{
    fn register_debug_log<U>(reg: &mut UserDataRegistry<U>)
    where
        U: Borrow<T>,
    {
        let log_fns = GlobalInstanceDebugLog::<LuaString, T>::named_fns();
        for (name, f) in log_fns {
            reg.add_function(name, move |_lua, (msg,): (LuaString,)| {
                f(&this.borrow(), msg).map_err(to_lua_error)
            });
        }
    }
}

#[cfg(todo)]
pub struct GlobalInstanceDebugWatch;
#[cfg(todo)]
impl<T> UserData for ScriptApiTable<GlobalInstanceDebugWatch, T>
where
    T: ScriptApiDebugWatch + 'static,
{
    fn register(reg: &mut UserDataRegistry<Self>) {
        Self::register_debug_watch(reg)
    }
}
#[cfg(todo)]
impl<T> ScriptApiTable<GlobalInstanceDebugWatch, T>
where
    T: ScriptApiDebugWatch + 'static,
{
    fn register_debug_watch<U>(reg: &mut UserDataRegistry<U>)
    where
        U: Borrow<T>,
    {
        reg.add_method("Watch", |_lua, this, (key, value): (LuaString, LuaValue)| {
            this.borrow().watch(key, value).map_err(to_lua_error)
        });
        reg.add_method("ClearWatch", |_lua, this, (key,): (LuaString,)| {
            this.borrow().clear_watch(key).map_err(to_lua_error)
        });
    }
}
