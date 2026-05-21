use {
    crate::{
        attributes::cell::PackValueCell,
        script::{
            format_err,
            lua::{to_lua_error, HandleToLua, ScriptApiTable},
            pathing::{MenuDesc, MenuHandle, MenuHandleMut, MenuInstance, ScriptApiUser},
            user::IntoUserHandle,
        },
    },
    core::{borrow::Borrow, time::Duration},
    mlua::{
        BorrowedStr,
        IntoLua,
        Lua,
        Result as LuaResult,
        UserData,
        UserDataMethods,
        UserDataRegistry,
        Value as LuaValue,
    },
};

pub struct GlobalInstanceUiX;
impl<T> IntoLua for ScriptApiTable<GlobalInstanceUiX, T>
where
    T: ScriptApiUser + 'static,
{
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let api = lua.create_any_userdata(self.api)?;
        let clipboard_send = lua.create_function({
            let api = api.clone();
            move |_lua, (value, msg): (BorrowedStr, Option<BorrowedStr>)| {
                api.borrow::<T>().and_then(|api| {
                    api.set_clipboard(&value[..], msg.as_ref().map(|m| &m[..]))
                        .map_err(to_lua_error)
                })
            }
        })?;
        let info_start = lua.create_function({
            let api = api.clone();
            move |_lua, (msg,): (BorrowedStr,)| {
                api.borrow::<T>()
                    .and_then(|api| api.info_show(&msg[..]).map_err(to_lua_error))
            }
        })?;
        let info_end = lua.create_function({
            let api = api.clone();
            move |_lua, (msg,): (BorrowedStr,)| {
                api.borrow::<T>()
                    .and_then(|api| api.info_hide(&msg[..]).map_err(to_lua_error))
            }
        })?;
        let info_notify = lua.create_function({
            let api = api.clone();
            move |_lua, (msg, dur): (BorrowedStr, Option<f32>)| {
                api.borrow::<T>().and_then(|api| {
                    api.info_notify(&msg[..], dur.map(Duration::from_secs_f32))
                        .map_err(to_lua_error)
                })
            }
        })?;
        lua.create_table_from([
            ("clipboard_send", clipboard_send),
            ("info_start", info_start),
            ("info_end", info_end),
            ("info_notify", info_notify),
        ])
        .map(LuaValue::Table)
    }
}

pub struct GlobalInstanceMenu;
impl<T> UserData for ScriptApiTable<GlobalInstanceMenu, T>
where
    T: MenuInstance + 'static,
    T::RegisteredMenu: IntoUserHandle,
    T::Menu: IntoUserHandle,
{
    fn register(reg: &mut UserDataRegistry<Self>) {
        ScriptApiTable::<UiInstanceMenu, T>::register_ui_menu_desc(reg);
        Self::register_ui_menu_host(reg);
    }
}
impl<T> ScriptApiTable<GlobalInstanceMenu, T>
where
    T: MenuInstance + 'static,
    T::RegisteredMenu: IntoUserHandle,
    T::Menu: IntoUserHandle,
{
    fn register_ui_menu_host<U>(reg: &mut UserDataRegistry<U>)
    where
        U: Borrow<T>,
    {
        reg.add_method("Register", |_lua, this, (id,)| {
            this.borrow()
                .register_id(id)
                .map_err(to_lua_error)
                .map(HandleToLua)
        });
        reg.add_method("RemoveId", |_lua, this, (m, recursive): (BorrowedStr, bool)| {
            this.borrow()
                .remove_id(m[..].as_ref(), recursive)
                .map_err(to_lua_error)
        });
        reg.add_method("LookupId", |_lua, this, (m,): (BorrowedStr,)| {
            this.borrow()
                .lookup_id(m[..].as_ref())
                .map_err(to_lua_error)
                .map(|m| m.map(HandleToLua))
        });
        reg.add_method(
            "GenId",
            |_lua, this, (parent, name): (Option<BorrowedStr>, Option<BorrowedStr>)| {
                let parent = parent.as_ref().map(|p| p[..].as_ref());
                let name = name.as_ref().map(|n| n[..].as_ref());
                this.borrow().gen_id(parent, name).map_err(to_lua_error)
            },
        );
    }
}
#[cfg(todo)]
impl<T> IntoLua for ScriptApiTable<GlobalInstanceMenu, T>
where
    T: ScriptApiMenu + 'static,
{
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        self.current_menu().etc();
    }
}

pub struct UiInstanceMenu;
impl<T> UserData for ScriptApiTable<UiInstanceMenu, T>
where
    T: MenuHandleMut + 'static,
{
    fn register(reg: &mut UserDataRegistry<Self>) {
        Self::register_ui_menu_desc(reg);
        Self::register_ui_menu(reg);
        Self::register_ui_menu_mut(reg);
    }
}
impl<T> ScriptApiTable<UiInstanceMenu, T>
where
    T: MenuDesc + 'static,
{
    fn register_ui_menu_desc<U>(reg: &mut UserDataRegistry<U>)
    where
        U: Borrow<T>,
    {
        reg.add_method("GetId", |_lua, this, ()| {
            this.borrow().get_id().map_err(to_lua_error)
        });
        reg.add_method("GetAttrByKey", |lua, this, (key,): (BorrowedStr<'_>,)| {
            super::AttrRegistration::for_key(&key)
                .ok_or_else(|| to_lua_error(format_err!("unrecognized menu attribute {key}")))
                .and_then(|attr| {
                    Borrow::<T>::borrow(&*this)
                        .get_menu_attr_dyn(attr.attr())
                        .map_err(to_lua_error)
                        .and_then(move |v| v.map(|v| attr.to_lua_dyn(&v, lua)).transpose())
                })
        });
    }
}
impl<T> ScriptApiTable<UiInstanceMenu, T>
where
    T: MenuHandle + 'static,
{
    fn register_ui_menu<U>(reg: &mut UserDataRegistry<U>)
    where
        U: Borrow<T>,
    {
        reg.add_method("GetState", |_lua, this, ()| {
            this.borrow().get_check_state().map_err(to_lua_error)
        });
    }
}
impl<T> ScriptApiTable<UiInstanceMenu, T>
where
    T: MenuHandleMut + 'static,
{
    fn register_ui_menu_mut<U>(reg: &mut UserDataRegistry<U>)
    where
        U: Borrow<T>,
    {
        reg.add_method("SetState", |_lua, this, (v,): (Option<bool>,)| {
            this.borrow().set_check_state(v).map_err(to_lua_error)
        });
        reg.add_method(
            "SetAttrByKey",
            |lua, this, (key, value): (BorrowedStr<'_>, LuaValue)| {
                super::AttrRegistration::for_key(&key)
                    .ok_or_else(|| to_lua_error(format_err!("unrecognized menu attribute {key}")))
                    .and_then(|attr| attr.from_lua_dyn(value, lua))
                    .and_then(|v| {
                        Borrow::<T>::borrow(&*this)
                            .set_menu_attr_dyn(v)
                            .map_err(to_lua_error)
                    })
            },
        );
        reg.add_method("UnsetAttrByKey", |_lua, this, (key,): (BorrowedStr<'_>,)| {
            super::AttrRegistration::for_key(&key)
                .ok_or_else(|| to_lua_error(format_err!("unrecognized menu attribute {key}")))
                .and_then(|attr| {
                    Borrow::<T>::borrow(&*this)
                        .set_menu_attr_dyn(PackValueCell::new_empty(attr.attr()))
                        .map_err(to_lua_error)
                })
        });
    }
}
