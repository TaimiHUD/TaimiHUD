use std::sync::atomic::{AtomicIsize, Ordering};

#[derive(Debug)]
pub struct Counter {
    pub count: AtomicIsize,
}

impl Counter {
    pub const DEFAULT: Self = Self::new(0);

    #[inline]
    pub const fn new(amt: isize) -> Self {
        let _amt = amt;
        Self { count: AtomicIsize::new(_amt) }
    }

    #[inline(always)]
    pub fn increment_by<F: FnOnce() -> usize>(&self, f: F) -> usize {
        let amt = f();
        self.increment(amt);
        amt
    }

    #[inline(always)]
    pub fn decrement_by<F: FnOnce() -> usize>(&self, f: F) -> usize {
        let amt = f();
        self.decrement(amt);
        amt
    }

    #[inline(always)]
    pub fn reset_with<F: FnOnce() -> isize>(&self, f: F) -> isize {
        let amt = f();
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

impl From<Counter> for isize {
    fn from(counter: Counter) -> isize {
        counter.get()
    }
}
impl From<Counter> for usize {
    fn from(counter: Counter) -> usize {
        counter.get() as usize
    }
}
impl From<&'_ Counter> for isize {
    fn from(counter: &Counter) -> isize {
        counter.get()
    }
}
impl From<&'_ Counter> for usize {
    fn from(counter: &Counter) -> usize {
        counter.get() as usize
    }
}
