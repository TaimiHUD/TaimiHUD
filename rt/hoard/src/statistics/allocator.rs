//! for debugging and diagnostics

use {
    crate::statistics::Counter,
    std::alloc::{GlobalAlloc, Layout, System},
};

pub struct CounterAllocator;

unsafe impl GlobalAlloc for CounterAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _size = STATS_ALLOC_SIZE.increment_by(|| layout.size());
        let p = System.alloc(layout);
        if p.is_null() {
            STATS_ALLOC_SIZE.decrement(_size);
        }
        p
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        STATS_ALLOC_SIZE.decrement_by(|| layout.size());
        System.dealloc(ptr, layout);
    }
}

impl CounterAllocator {
    pub const fn new() -> Self {
        Self
    }

    pub fn total_allocated() -> usize {
        STATS_ALLOC_SIZE.get() as usize
    }
}

pub static STATS_ALLOC_SIZE: Counter = Counter::DEFAULT;
