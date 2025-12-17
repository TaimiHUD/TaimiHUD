//! "performance" counters

use core::marker::PhantomData;

pub use {self::counter::Counter, core::sync::atomic::Ordering};

pub mod allocator;
mod counter;

pub type PhantomOrdering = ();

#[derive(Debug)]
pub struct Dummy {
    pub count: PhantomData<isize>,
}

impl Dummy {
    pub const DEFAULT: Self = Self::new(0);

    #[inline]
    pub const fn new(_amt: isize) -> Self {
        Self { count: PhantomData }
    }

    #[inline(always)]
    pub fn increment_by<F: FnOnce() -> usize>(&self, _f: F) -> usize {
        0
    }
    #[inline(always)]
    pub fn decrement_by<F: FnOnce() -> usize>(&self, _f: F) -> usize {
        0
    }
    #[inline(always)]
    pub fn adjust_by<F: FnOnce() -> isize>(&self, _f: F) -> isize {
        0
    }
    #[inline(always)]
    pub fn reset_with<F: FnOnce() -> isize>(&self, _f: F) -> isize {
        0
    }

    #[inline(always)]
    pub fn get_any(&self) -> Option<isize> {
        None
    }
}

impl Default for Dummy {
    #[inline]
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl Clone for Dummy {
    #[inline]
    fn clone(&self) -> Self {
        Self::new(self.get())
    }
}

impl Dummy {
    pub const ORDERING: PhantomOrdering = ();

    #[inline(always)]
    pub fn increment(&self, _amt: usize) {}
    #[inline(always)]
    pub fn decrement(&self, _amt: usize) {}
    #[inline(always)]
    pub fn adjust(&self, _amt: isize) {}

    #[inline(always)]
    pub fn reset(&self, _amt: isize) {}

    #[inline(always)]
    pub fn get(&self) -> isize {
        0
    }

    #[inline(always)]
    pub fn count(&self) -> usize {
        0
    }
}

impl From<Dummy> for isize {
    fn from(_: Dummy) -> Self {
        0
    }
}
impl From<Dummy> for usize {
    fn from(_: Dummy) -> Self {
        0
    }
}
impl From<&'_ Dummy> for isize {
    fn from(_: &Dummy) -> Self {
        0
    }
}
impl From<&'_ Dummy> for usize {
    fn from(_: &Dummy) -> Self {
        0
    }
}

impl From<Dummy> for Counter {
    fn from(counter: Dummy) -> Self {
        Counter::new(counter.get())
    }
}
impl From<Counter> for Dummy {
    fn from(_: Counter) -> Self {
        Dummy::DEFAULT
    }
}
