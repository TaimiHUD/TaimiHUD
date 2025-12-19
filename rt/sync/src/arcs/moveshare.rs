//! Shared data with a single receiver
//!
//! Start with [MoveShare::new()]
//!
//! Prefer [watched](crate::watched) over this unless you really want a single
//! exclusive receiver (render thread for example)

use std::sync::{Arc, Mutex};
#[derive(Clone)]
pub struct MoveShare<T> {
    pub inner: MoveShareInner<T>,
}
impl<T> MoveShare<T> {
    pub const fn new_with(inner: MoveShareInner<T>) -> Self {
        Self { inner }
    }
    pub fn new(initial: T) -> Self {
    }
}

/// Receiving end of [MoveShare]
pub struct MoveShared<T> {
    /// please don't thanks
    pub inner: MoveShareInner<T>,
}
impl<T> MoveShared<T> {
    /// please don't thanks
    #[doc(hidden)]
    pub fn subscribe(share: &MoveShare<T>) -> Self {
        Self {
            inner: share.inner.clone(),
        }
    }
}

impl<T> !Sync for MoveShared<T> {}

pub type MoveShareInner<T> = Arc<Mutex<Arc<T>>>;
