use {
    crate::script::{
        lua::{to_lua_error, ScriptApiTable},
        pathing::ScriptApiStorage,
    },
    core::borrow::Borrow,
    mlua::{BorrowedStr, UserData, UserDataMethods, UserDataRegistry},
};

pub struct PersistInstanceStore;
impl<T> UserData for ScriptApiTable<PersistInstanceStore, T>
where
    T: ScriptApiStorage + 'static,
{
    fn register(reg: &mut UserDataRegistry<Self>) {
        Self::register_persist_store(reg)
        // TODO: constructor...
    }
}
impl<T> ScriptApiTable<PersistInstanceStore, T>
where
    T: ScriptApiStorage + 'static,
{
    fn register_persist_store<U>(reg: &mut UserDataRegistry<U>)
    where
        U: Borrow<T>,
    {
        reg.add_method(
            "InsertString",
            |_lua, this, (n, k, v): (BorrowedStr, BorrowedStr, Option<BorrowedStr>)| {
                let (k, n, v) = match (&n, &k, &v) {
                    (k, v, None) => (&k[..], None, &v[..]),
                    (n, k, Some(v)) => (&k[..], Some(&n[..]), &v[..]),
                };
                this.borrow()
                    .insert_string(&k[..], n, &v[..])
                    .map_err(to_lua_error)
            },
        );
        reg.add_method(
            "RemoveKey",
            |_lua, this, (n, k): (BorrowedStr, Option<BorrowedStr>)| {
                let (k, n) = match (&n, &k) {
                    (k, None) => (&k[..], None),
                    (n, Some(k)) => (&k[..], Some(&n[..])),
                };
                this.borrow().remove_key(&k[..], n).map_err(to_lua_error)
            },
        );
        reg.add_method(
            "GetString",
            |_lua, this, (n, k): (BorrowedStr, Option<BorrowedStr>)| {
                let (k, n) = match (&n, &k) {
                    (k, None) => (&k[..], None),
                    (n, Some(k)) => (&k[..], Some(&n[..])),
                };
                this.borrow().get_string(k, n).map_err(to_lua_error)
            },
        );
    }
}
#[cfg(todo)]
impl<T> ScriptApiTable<GlobalInstanceStorage, T>
where
    T: ScriptApiStorage + 'static,
{
    fn register_persist_store<U>(reg: &mut UserDataRegistry<U>)
    where
        U: Borrow<T>,
    {
        reg.add_method("UpsertValue", |_lua, this, (k, v): (BorrowedStr, BorrowedStr)| {
            this.borrow().insert_string(&k[..], &v[..]).map_err(to_lua_error)
        });
        reg.add_method("DeleteValue", |_lua, this, (k,): (BorrowedStr,)| {
            this.borrow().remove_key(&k[..]).map_err(to_lua_error)
        });
        reg.add_method("ReadValue", |_lua, this, (k,): (BorrowedStr,)| {
            this.borrow().get_string(&k[..]).map_err(to_lua_error)
        });
    }
}
