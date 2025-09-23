#[cfg(feature = "windows")]
pub mod win;

pub type RuntimeError = &'static str;
pub type RuntimeResult<T = ()> = Result<T, RuntimeError>;
