#[cfg(feature = "scripts-lua")]
use mlua::{IntoLua, Lua, Result as LuaResult, Value as LuaValue};

#[derive(Debug, Clone, Default)]
pub struct ScriptHostNexus;

#[cfg(feature = "scripts-lua")]
impl IntoLua for ScriptHostNexus {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        lua.create_table_from([
            ("available", LuaValue::Boolean(false)),
            ("supported", LuaValue::Boolean(false)),
            ("HostSignal", LuaValue::Table(lua.create_table()?)),
        ])
        .map(LuaValue::Table)
    }
}
