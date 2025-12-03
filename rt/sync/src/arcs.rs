use std::sync::Weak;

/// unallocated refs are [equivalent](Weak::ptr_eq) to [Weak::new()]
pub fn weak_is_null<T>(weak: &Weak<T>) -> bool {
    Weak::ptr_eq(weak, &Weak::new())
}

/// best-effort and not a replacement for [Weak::upgrade]
pub fn weak_is_dangling<T: ?Sized>(weak: &Weak<T>) -> bool {
    weak.strong_count() == 0
}
