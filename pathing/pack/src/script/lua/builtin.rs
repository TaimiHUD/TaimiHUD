use {
    crate::script::lua::{IntoLuaFn, RuntimeLua},
    mlua::{UserData, UserDataFields, UserDataRegistry, Value as LuaValue},
};

pub struct ApiCoreRt;
impl UserData for ApiCoreRt {
    fn register(reg: &mut UserDataRegistry<Self>) {
        reg.add_field(
            "is_unsecured",
            IntoLuaFn::new(|lua| Ok(LuaValue::Boolean(RuntimeLua::lua_is_unsecured(lua).is_some()))),
        );
        reg.add_field("is_stub", false);
        // TODO: lol
        reg.add_field("pathing_hack_autotrigger", false);
        reg.add_field("pathing_hack_interact", false);
        reg.add_field("pathing_hack_manualtrigger", false);
    }
}
