//! unsorted synchronization primitives for multithreading and async use

pub use std::sync::PoisonError as StdPoisonError;

pub mod arcs;
pub mod poll_once;
pub mod scheduled;
pub mod typemap;
pub mod watched;

#[cfg(panic = "unwind")]
pub type PoisonError<T> = StdPoisonError<T>;
#[cfg(not(panic = "unwind"))]
pub type PoisonError<T> = StdPoisonError<(core::convert::Infallible, T)>;
pub fn drop_poison<T>(e: StdPoisonError<T>) -> PoisonError<()> {
    map_poison_std(e, drop)
}
pub fn map_poison_std<R, T, F: FnOnce(T) -> R>(e: StdPoisonError<T>, f: F) -> PoisonError<R> {
    match e {
        #[cfg(panic = "unwind")]
        e => PoisonError::new(f(e.into_inner())),
        #[cfg(not(panic = "unwind"))]
        _ => unsafe { core::hint::unreachable_unchecked() },
    }
}
pub fn map_poison<R, T, F: FnOnce(T) -> R>(e: PoisonError<T>, f: F) -> PoisonError<R> {
    match e {
        #[cfg(panic = "unwind")]
        e => map_poison_std(e, f),
        #[cfg(not(panic = "unwind"))]
        e => match e.into_inner() {
            (i, ..) => match i {},
        },
    }
}
