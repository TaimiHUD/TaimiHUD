/// TODO: deleteme
#[cfg(feature = "statistics")]
pub use taimi_hoard::statistics::allocator::{CounterAllocator as TaimiAllocator, STATS_ALLOC_SIZE};

use crate::exports::runtime::Counter;
#[global_allocator]
pub static ALLOCATOR: TaimiAllocator = TaimiAllocator::new();
#[cfg(not(feature = "statistics"))]
pub static STATS_ALLOC_SIZE: Counter = Counter::DEFAULT;
