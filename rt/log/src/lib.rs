use std::sync::Arc;

mod ok;

pub use {
    self::ok::{debug_ok, debug_ok_with, error_ok, info_ok, log_ok, warn_ok},
    ::log::{
        self,
        debug,
        error,
        info,
        log,
        log_enabled,
        trace,
        warn,
        Level,
        LevelFilter,
        Log,
        Metadata,
        Record,
    },
    std::error::Error as StdError,
};

pub type DynError = dyn StdError + Send + Sync;
/// TODO: update anyhow?
pub fn anyhow_into_box(e: anyhow::Error) -> Box<DynError> {
    match e {
        #[cfg(todo)]
        e => e.into_boxed_dyn_error(),
        e => error_into_box(e),
    }
}
#[inline]
pub fn anyhow_into_arc(e: anyhow::Error) -> Arc<DynError> {
    error_into_arc(anyhow_into_box(e))
}
#[inline]
#[track_caller]
/// TODO
pub fn anyhow_clone(e: &anyhow::Error) -> anyhow::Error {
    anyhow::anyhow!("{e:#}")
}
#[inline]
pub fn error_into_box<E>(e: E) -> Box<DynError>
where
    E: Into<Box<DynError>>,
{
    e.into()
}
pub fn error_into_arc<E>(e: E) -> Arc<DynError>
where
    E: Into<Box<DynError>>,
{
    Arc::from(error_into_box(e))
}
