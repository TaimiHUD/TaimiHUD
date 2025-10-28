use {
    crate::exports::runtime::Counter,
    std::alloc::{GlobalAlloc, Layout, System},
};

pub struct TaimiAllocator;

unsafe impl GlobalAlloc for TaimiAllocator {
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

pub static STATS_ALLOC_SIZE: Counter = Counter::DEFAULT;
#[global_allocator]
pub static ALLOCATOR: TaimiAllocator = TaimiAllocator;
