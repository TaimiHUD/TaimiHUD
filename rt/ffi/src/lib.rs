//! most of this hopefully is just a temporary staging ground for things
//! before they're ready to be pulled into [arcffi]...

#[cfg(feature = "fnalloc")]
pub mod fnalloc;
#[cfg(feature = "windows")]
pub mod win32;

// allow this to serve as a package alias for convenience
#[rustfmt::skip]
pub use ::arcffi::{
    cstr,
    nn::{self, nonnull_ref, nonnull_unwrap, nonnull_unwrap_mut},
    // ptr::*
    transmute_unchecked, write_copy_of_slice, write_clone_of_slice,
    UserMallocFn, UserFreeFn,
    extern_fns,
    wide,
    windows,
    // std re-exports
    alloc,
    NonNull, c_void, c_int, c_uint, c_long, c_ulong, c_longlong, c_ulonglong, c_char, c_uchar, c_schar,
    c_wchar, c_senum, c_uenum, c_bool, c_bool32,
};
