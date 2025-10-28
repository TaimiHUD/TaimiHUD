#[cfg(feature = "statistics")]
use std::sync::atomic::AtomicIsize;
#[cfg(feature = "statistics")]
pub use std::sync::atomic::Ordering;

#[derive(Debug)]
pub struct Counter {
    #[cfg(feature = "statistics")]
    pub count: AtomicIsize,
}

impl Counter {
    pub const DEFAULT: Self = Self::new(0);

    #[inline]
    pub const fn new(amt: isize) -> Self {
        let _amt = amt;
        Self {
            #[cfg(feature = "statistics")]
            count: AtomicIsize::new(_amt),
        }
    }

    #[inline(always)]
    pub fn increment_by<F: FnOnce() -> usize>(&self, _f: F) -> usize {
        let amt = match () {
            #[cfg(feature = "statistics")]
            () => _f(),
            #[cfg(not(feature = "statistics"))]
            () => 0,
        };
        self.increment(amt);
        amt
    }

    #[inline(always)]
    pub fn decrement_by<F: FnOnce() -> usize>(&self, _f: F) -> usize {
        let amt = match () {
            #[cfg(feature = "statistics")]
            () => _f(),
            #[cfg(not(feature = "statistics"))]
            () => 0,
        };
        self.decrement(amt);
        amt
    }

    #[inline(always)]
    pub fn reset_with<F: FnOnce() -> isize>(&self, _f: F) -> isize {
        let amt = match () {
            #[cfg(feature = "statistics")]
            () => _f(),
            #[cfg(not(feature = "statistics"))]
            () => 0,
        };
        self.reset(amt);
        amt
    }

    #[inline(always)]
    pub fn get_any(&self) -> Option<isize> {
        match self.get() {
            0 => None,
            amt => Some(amt),
        }
    }
}

#[cfg(feature = "statistics")]
impl Counter {
    pub const ORDERING: Ordering = Ordering::Relaxed;

    pub fn increment(&self, amt: usize) {
        self.count.fetch_add(amt as isize, Self::ORDERING);
    }

    pub fn decrement(&self, amt: usize) {
        self.count.fetch_sub(amt as isize, Self::ORDERING);
    }

    pub fn reset(&self, amt: isize) {
        self.count.store(amt, Self::ORDERING);
    }

    pub fn get(&self) -> isize {
        self.count.load(Self::ORDERING)
    }
}

impl Default for Counter {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl Clone for Counter {
    fn clone(&self) -> Self {
        Self::new(self.get())
    }
}

#[cfg(not(feature = "statistics"))]
impl Counter {
    pub const ORDERING: () = ();

    #[inline(always)]
    pub fn increment(&self, _amt: usize) {}

    #[inline(always)]
    pub fn decrement(&self, _amt: usize) {}

    #[inline(always)]
    pub fn reset(&self, _amt: isize) {}

    #[inline(always)]
    pub fn get(&self) -> isize {
        0
    }
}
