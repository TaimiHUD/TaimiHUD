use core::result::Result as StdResult;

#[cfg(feature = "script-lua")]
pub mod lua;
pub mod pathing;
#[cfg(todo)]
pub mod runtime;
pub mod user;
pub mod value;

pub use {
    self::value::{Colour, TimeSpan, Vec2, Vec3},
    anyhow::{anyhow as format_err, bail},
};

#[cfg(feature = "script-lua")]
pub use self::lua::{LuaPack, RuntimeLua};

pub type ScriptError = anyhow::Error;
pub type Result<T> = StdResult<T, ScriptError>;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Unimplemented;

#[macro_export]
macro_rules! script_unimpl {
    () => {
        Err($crate::script::format_err! { "unimplemented" })
    };
    ($msg:literal $($args:tt)*) => {
        Err($crate::script::format_err! { concat! { "unimplemented:", $msg } $($args)* })
    };
}
pub use script_unimpl;
