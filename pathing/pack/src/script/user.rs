use {
    crate::{attributes::keys::Guid, script::Result},
    anyhow::Context,
    core::{hash::Hash, result::Result as StdResult},
    std::{rc::Rc, sync::Arc},
};

pub trait ScriptUserString: ScriptUserStr + Sized {
    fn into_string(self) -> String {
        self.with_str(|s| s.into())
    }
}
pub trait ScriptUserStr: ScriptSourceTag {
    fn with_str<R, F: FnOnce(&str) -> R>(&self, f: F) -> R;
    fn clone_to_string(&self) -> String {
        self.with_str(|s| s.into())
    }
}
impl ScriptUserStr for str {
    #[inline]
    fn with_str<R, F: FnOnce(&str) -> R>(&self, f: F) -> R {
        f(self)
    }
}
impl ScriptUserStr for &'_ str {
    #[inline]
    fn with_str<R, F: FnOnce(&str) -> R>(&self, f: F) -> R {
        f(*self)
    }
}
impl ScriptUserStr for String {
    #[inline]
    fn with_str<R, F: FnOnce(&str) -> R>(&self, f: F) -> R {
        f(&self[..])
    }
}
impl ScriptUserStr for &'_ String {
    #[inline]
    fn with_str<R, F: FnOnce(&str) -> R>(&self, f: F) -> R {
        f(&self[..])
    }
}
pub trait ScriptUserGuid: ScriptSourceTag {
    type GuidAsStr: ?Sized + ScriptUserStr;
    fn user_guid_as_str(&self) -> Option<&Self::GuidAsStr>;
    fn try_with_guid<R, F>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&Guid) -> R,
    {
        match self.with_guid(f) {
            Ok(res) => Ok(res),
            Err(f) => self
                .user_guid_as_str()
                .map(|s| s.with_str(|s| s.parse().map(|guid| f(&guid)).context("invalid GUID")))
                .unwrap_or_else(|| Err(anyhow::anyhow!("expected GUID arg"))),
        }
    }
    fn with_guid<R, F>(&self, f: F) -> StdResult<R, F>
    where
        F: FnOnce(&Guid) -> R,
    {
        Err(f)
    }
}
impl<'a, G> ScriptUserGuid for &'a G
where
    G: ScriptUserGuid,
{
    type GuidAsStr = G::GuidAsStr;
    #[inline]
    fn with_guid<R, F>(&self, f: F) -> StdResult<R, F>
    where
        F: FnOnce(&Guid) -> R,
    {
        ScriptUserGuid::with_guid(*self, f)
    }
    #[inline]
    fn user_guid_as_str(&self) -> Option<&Self::GuidAsStr> {
        ScriptUserGuid::user_guid_as_str(*self)
    }
}
impl ScriptUserGuid for str {
    type GuidAsStr = Self;
    #[inline]
    fn user_guid_as_str(&self) -> Option<&Self::GuidAsStr> {
        Some(self)
    }
}
impl ScriptUserGuid for Guid {
    type GuidAsStr = Self;
    fn with_guid<R, F>(&self, f: F) -> StdResult<R, F>
    where
        F: FnOnce(&Guid) -> R,
    {
        Ok(f(self))
    }
    #[inline]
    fn user_guid_as_str(&self) -> Option<&Self::GuidAsStr> {
        Some(self)
    }
}
impl ScriptUserStr for Guid {
    #[inline]
    fn with_str<R, F: FnOnce(&str) -> R>(&self, f: F) -> R {
        let guid = self.to_string();
        f(&guid)
    }
}
impl IntoUserHandle for Guid {
    type IntoHandle = Self;
    #[inline]
    fn clone_into_handle(&self) -> Self::IntoHandle {
        *self
    }
    #[cfg(feature = "script-lua")]
    #[inline]
    fn to_lua_handle(&self, lua: &mlua::Lua) -> mlua::Result<mlua::Value> {
        mlua::IntoLua::into_lua(*self, lua)
    }
}

pub trait ScriptUserAttrs: ScriptSourceTag {
    type AttrsKey: ScriptUserStr;
    type AttrsValue: ScriptUserHandle;
    type AttrsIntoIter<'a>: IntoIterator<Item = Result<(Self::AttrsKey, Self::AttrsValue)>>
    where
        Self: 'a;

    fn iter_user_attrs(&self) -> Self::AttrsIntoIter<'_>;
}

pub trait ScriptUserIterable: ScriptSourceTag {
    type UserValue: ScriptSourceTag;
    type UserIntoIter: IntoIterator<Item = Self::UserValue>;
}
pub trait ScriptUserUntyped: ScriptSourceTag {
    #[cfg(feature = "script-lua")]
    fn to_lua_value(&self) -> Option<mlua::Value> {
        None
    }
    #[cfg(feature = "script-lua")]
    fn to_lua_multi(&self) -> mlua::MultiValue {
        mlua::MultiValue::new()
    }
}

pub trait ScriptUserCallback<A>: ScriptUserHandle {}

pub trait ScriptUserHandle {
    type Handle: Clone + Eq + Hash;
    fn with_handle<R, F: FnOnce(&Self::Handle) -> R>(&self, f: F) -> R;
    #[inline]
    fn clone_handle(&self) -> Self::Handle {
        self.with_handle(Clone::clone)
    }

    #[cfg(feature = "script-lua")]
    fn to_lua_value(&self) -> Option<mlua::Value> {
        None
    }
}
impl ScriptUserHandle for () {
    type Handle = Self;
    #[inline]
    fn with_handle<R, F: FnOnce(&Self::Handle) -> R>(&self, f: F) -> R {
        f(self)
    }
}
impl<T> ScriptUserHandle for Box<T>
where
    Self: Clone + Eq + Hash,
{
    type Handle = Self;
    #[inline]
    fn with_handle<R, F: FnOnce(&Self::Handle) -> R>(&self, f: F) -> R {
        f(self)
    }
}
impl<T> ScriptUserHandle for Rc<T>
where
    Self: Clone + Eq + Hash,
{
    type Handle = Self;
    #[inline]
    fn with_handle<R, F: FnOnce(&Self::Handle) -> R>(&self, f: F) -> R {
        f(self)
    }
}
impl<T> ScriptUserHandle for Arc<T>
where
    Self: Clone + Eq + Hash,
{
    type Handle = Self;
    #[inline]
    fn with_handle<R, F: FnOnce(&Self::Handle) -> R>(&self, f: F) -> R {
        f(self)
    }
}

pub trait IntoUserHandle {
    #[cfg(todo)]
    type IntoHandle: Clone + Eq + Hash;
    type IntoHandle;

    fn into_handle(self) -> Self::IntoHandle
    where
        Self: Sized,
    {
        self.clone_into_handle()
    }
    fn clone_into_handle(&self) -> Self::IntoHandle;

    #[cfg(feature = "script-lua")]
    fn to_lua_handle(&self, lua: &mlua::Lua) -> mlua::Result<mlua::Value>;
}
#[cfg(feature = "script-lua")]
impl<H> mlua::IntoLua for &'_ dyn IntoUserHandle<IntoHandle = H>
where
    H: Clone + Eq + Hash,
{
    #[inline]
    fn into_lua(self, lua: &mlua::Lua) -> mlua::Result<mlua::Value> {
        self.to_lua_handle(lua)
    }
}
impl IntoUserHandle for () {
    type IntoHandle = Self;
    #[inline]
    fn clone_into_handle(&self) -> Self::IntoHandle {
        *self
    }
    #[cfg(feature = "script-lua")]
    #[inline]
    fn to_lua_handle(&self, lua: &mlua::Lua) -> mlua::Result<mlua::Value> {
        mlua::IntoLua::into_lua(mlua::Nil, lua)
    }
}

pub trait UpcastHandle<T: ?Sized> {
    fn cast_as_mut(&mut self) -> Option<&mut T>;
    fn cast_as(&self) -> Option<&T>;
}
#[cfg(todo)]
macro_rules! impl_upcast {
    (impl UpcastHandle for $trait_:path {} $($($rest:tt)+)?) => {
        impl<'a, T> UpcastHandle<dyn $trait_ + 'a> for T where
            T: $trait_ + 'a,
        {
            #[inline]
            fn cast_as_mut(&mut self) -> Option<&mut dyn $trait_> {
                Some(self)
            }
            #[inline]
            fn cast_as(&self) -> Option<&dyn $trait_> {
                Some(self)
            }
        }
        impl<'a, 'b> UpcastHandle<dyn $trait_ + 'b> for dyn $trait_ + 'a where
            'a: 'b,
        {
            #[inline]
            fn cast_as_mut(&mut self) -> Option<&mut (dyn $trait_ + 'a)> {
                Some(self)
            }
            #[inline]
            fn cast_as(&self) -> Option<&(dyn $trait_ + 'a)> {
                Some(self)
            }
        }
        impl<'a, 'b> UpcastHandle<dyn $trait_ + 'b> for Box<dyn $trait_ + 'a> where
            'a: 'b,
        {
            #[inline]
            fn cast_as_mut(&mut self) -> Option<&mut (dyn $trait_ + 'a)> {
                Some(self)
            }
            #[inline]
            fn cast_as(&self) -> Option<&(dyn $trait_ + 'a)> {
                Some(self)
            }
        }
        $($crate::script::user::impl_upcast! { $($rest)* })?
    };
}
#[cfg(todo)]
pub(crate) use impl_upcast;
impl<T, U> UpcastHandle<U> for Box<T>
where
    T: ?Sized + UpcastHandle<U>,
{
    #[inline]
    fn cast_as_mut(&mut self) -> Option<&mut U> {
        UpcastHandle::cast_as_mut(&mut **self)
    }
    #[inline]
    fn cast_as(&self) -> Option<&U> {
        UpcastHandle::cast_as(&**self)
    }
}
impl<T, U: ?Sized> UpcastHandle<U> for Rc<T>
where
    T: ?Sized + UpcastHandle<U>,
{
    #[inline]
    fn cast_as_mut(&mut self) -> Option<&mut U> {
        todo!("&mut Arc")
    }
    #[inline]
    fn cast_as(&self) -> Option<&U> {
        UpcastHandle::cast_as(&**self)
    }
}
impl<T, U: ?Sized> UpcastHandle<U> for Arc<T>
where
    T: ?Sized + UpcastHandle<U>,
{
    #[inline]
    fn cast_as_mut(&mut self) -> Option<&mut U> {
        todo!("&mut Arc")
    }
    #[inline]
    fn cast_as(&self) -> Option<&U> {
        UpcastHandle::cast_as(&**self)
    }
}
pub type SourceTag = usize;
pub trait ScriptSourceTag {
    fn user_src(&self) -> Option<SourceTag>;
}
impl<T> ScriptSourceTag for &'_ T
where
    T: ?Sized + ScriptSourceTag,
{
    #[inline]
    fn user_src(&self) -> Option<SourceTag> {
        ScriptSourceTag::user_src(*self)
    }
}
macro_rules! impl_source_tag {
    (impl ScriptSourceTag for $ty:ty {$($inner:tt)*} $($($rest:tt)+)?) => {
        impl $crate::script::user::ScriptSourceTag for $ty {
            #[inline]
            fn user_src(&self) -> Option<$crate::script::user::SourceTag> { None }
            $($inner)*
        }
        $($crate::script::user::impl_source_tag! { $($rest)* })?
    };
}
pub(crate) use impl_source_tag;
impl_source_tag! {
    impl ScriptSourceTag for () {}
    impl ScriptSourceTag for u32 {}
    impl ScriptSourceTag for [u8; 4] {}
    impl ScriptSourceTag for str {}
    impl ScriptSourceTag for String {}
    impl ScriptSourceTag for Guid {}
}
