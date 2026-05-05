use core::{ffi::c_void, ptr::NonNull};

#[derive(Debug)]
pub struct FallbackContext {
    pub ctx: NonNull<c_void>,
}
