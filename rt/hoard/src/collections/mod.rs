pub use self::traits::{TaimiExtend, TaimiSet};

pub mod lru;
pub mod traits;

/// TODO: check if points to same slice .-.
#[inline]
pub fn slice_offset_from<T>(slice: &[T], entry: &T) -> usize {
    unsafe {
        (entry as *const T).offset_from_unsigned(slice.as_ptr())
    }
}
