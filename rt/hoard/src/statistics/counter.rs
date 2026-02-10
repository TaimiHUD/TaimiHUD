use {
    num_traits::AsPrimitive,
    std::sync::atomic::{AtomicIsize, Ordering},
};

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
    pub fn increment_by<A: AsPrimitive<usize>, F: FnOnce() -> A>(&self, f: F) -> A {
        let amt = f();
        self.increment(amt);
        amt
    }
    #[inline(always)]
    pub fn decrement_by<A: AsPrimitive<usize>, F: FnOnce() -> A>(&self, f: F) -> A {
        let amt = f();
        self.decrement(amt);
        amt
    }
    #[inline(always)]
    pub fn adjust_by<A: AsPrimitive<isize>, F: FnOnce() -> A>(&self, f: F) -> A {
        let amt = f();
        self.adjust(amt);
        amt
    }
    #[inline(always)]
    pub fn reset_with<A: AsPrimitive<isize>, F: FnOnce() -> A>(&self, f: F) -> A {
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

    pub fn increment(&self, amt: impl AsPrimitive<usize>) {
        self.count.fetch_add(amt.as_() as isize, Self::ORDERING);
    }
    pub fn decrement(&self, amt: impl AsPrimitive<usize>) {
        self.count.fetch_sub(amt.as_() as isize, Self::ORDERING);
    }
    pub fn adjust(&self, amt: impl AsPrimitive<isize>) {
        self.count.fetch_add(amt.as_(), Self::ORDERING);
    }

    pub fn reset(&self, amt: impl AsPrimitive<isize>) {
        self.count.store(amt.as_(), Self::ORDERING);
    }

    pub fn get(&self) -> isize {
        self.count.load(Self::ORDERING)
    }
    pub fn count(&self) -> usize {
        self.get() as usize
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
