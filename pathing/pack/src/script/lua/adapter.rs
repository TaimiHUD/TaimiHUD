#[cfg(todo)]
impl<'rt> FromLua for Value<'rt> {
    fn from_lua(value: LuaValue, lua: &Lua) -> Result<Self> {
        Ok(match value {
            LuaValue::Nil => ().into(),
            LuaValue::String(v) => v.into(),
            LuaValue::Table(v) => v.into(),
            LuaValue::Boolean(v) => (v as isize).into(),
            LuaValue::Integer(v) => (v as isize).into(),
            LuaValue::Number(v) => (v as f32).into(),
            LuaValue::Function(v) => v.into(),
            v => Box::new(v).into(),
        })
    }
}
