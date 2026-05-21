use {
    crate::script::{
        format_err,
        lua::{anyhow2lua, to_lua_error, DiscardValue, ScriptApiTable},
        pathing::{
            event::{EventReceiver, NotifyScript, SignalId},
            ScriptApiEvent,
        },
    },
    core::marker::PhantomData,
    mlua::{
        FromLua,
        FromLuaMulti,
        IntoLua,
        IntoLuaMulti,
        Lua,
        MetaMethod,
        MultiValue,
        Result as LuaResult,
        String as LuaString,
        Thread as LuaThread,
        UserData,
        UserDataFields,
        UserDataMethods,
        UserDataRegistry,
        Value as LuaValue,
    },
};

pub struct GlobalInstanceEvent;
impl<T> IntoLua for ScriptApiTable<GlobalInstanceEvent, T>
where
    T: ScriptApiEvent + 'static,
{
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        lua.create_table_from([
            (
                "NotifyMessage",
                LuaValue::UserData(lua.create_proxy::<NotifyScript<MultiValue>>()?),
            ),
            (
                "EventReceiver",
                LuaValue::UserData(lua.create_proxy::<EventReceiver<MultiValue>>()?),
            ),
            (
                "HostSignal",
                lua.create_table_from(self.api.all_notifications())
                    .map(LuaValue::Table)?,
            ),
            (
                "ScriptSignal",
                lua.create_table_from(self.api.all_signals())
                    .map(LuaValue::Table)?,
            ),
            (
                "Event",
                ScriptApiTable {
                    api: self.api,
                    _api: PhantomData::<EventInstance>,
                }
                .into_lua(lua)?,
            ),
        ])
        .map(LuaValue::Table)
    }
}
pub struct EventInstance;
impl<T> UserData for ScriptApiTable<EventInstance, T>
where
    T: ScriptApiEvent + 'static,
{
    fn register(reg: &mut UserDataRegistry<Self>) {
        reg.add_method("Mask", |_lua, this, (id,): (SignalId,)| {
            this.api.notifcation_mask(id).map_err(to_lua_error)
        });
        reg.add_method("Unmask", |_lua, this, (id,): (SignalId,)| {
            this.api.notifcation_unmask(id).map_err(to_lua_error)
        });
        reg.add_method(
            "SignalOob",
            |_lua, this, (co, msg): (LuaThread, NotifyScript<MultiValue>)| {
                this.api.notifcation_oob(co, msg).map_err(to_lua_error)
            },
        );
    }
}

impl<A> UserData for NotifyScript<A>
where
    A: FromLuaMulti + IntoLuaMulti + Clone + 'static,
{
    fn register(reg: &mut UserDataRegistry<Self>) {
        reg.add_function("new", |_lua, (id, args): (SignalId, A)| Ok(Self::new(id, args)));
        reg.add_function("New", |_lua, (_proxy, id, args): (DiscardValue, SignalId, A)| {
            Ok(Self::new(id, args))
        });
        reg.add_field_method_get("id", |_lua, this| Ok(this.id));
        reg.add_method("GetArgsPositional", |_lua, this, ()| Ok(this.args.clone()));
        reg.add_meta_method(MetaMethod::ToString, |_lua, this, ()| Ok(this.args.clone()));
    }
}
impl<A> FromLua for NotifyScript<A>
where
    //A: FromLuaMulti + Clone + 'static,
    Self: UserData + Clone + 'static,
    A: Default,
{
    fn from_lua(value: LuaValue, _lua: &Lua) -> LuaResult<Self> {
        match value {
            LuaValue::UserData(ud) => ud.borrow::<Self>().map(|v| v.clone()),
            LuaValue::Integer(id) => Ok(Self::empty(id as SignalId)),
            _ => Err(anyhow2lua(format_err!("expected event message"))),
        }
    }
}

impl<A> UserData for EventReceiver<A>
where
    A: FromLuaMulti + IntoLuaMulti + Clone + 'static,
{
    fn register(reg: &mut UserDataRegistry<Self>) {
        reg.add_function("new", |_lua, (receiver, args): (LuaString, A)| {
            receiver
                .to_str()
                .map(|receiver| Self::new(receiver[..].into(), args))
        });
        reg.add_function(
            "New",
            |_lua, (_proxy, receiver, args): (DiscardValue, LuaString, A)| {
                receiver
                    .to_str()
                    .map(|receiver| Self::new(receiver[..].into(), args))
            },
        );
        reg.add_field_method_get("receiver", |lua, this| this.receiver[..].into_lua(lua));
        reg.add_method("GetExtraArgsPositional", |_lua, this, ()| {
            Ok(this.user_args.clone())
        });
    }
}
