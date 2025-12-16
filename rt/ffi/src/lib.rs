//! most of this hopefully is just a temporary staging ground for things
//! before they're ready to be pulled into [arcffi]...

#[cfg(feature = "fnalloc")]
pub mod fnalloc;
#[cfg(feature = "windows")]
pub mod win32;
