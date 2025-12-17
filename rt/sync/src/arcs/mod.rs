use {
    crate::typemap::{empty_any_map_sync, AnyMapSync},
    std::{
        ffi::CStr,
        mem::MaybeUninit,
        sync::{Arc, RwLock, Weak},
    },
};

#[cfg(todo)]
pub use self::iters::{ArcSliceAt, ArcSliceIter};
pub use self::ptrcmp::ArcPtrCmp;

pub mod iters;
pub mod moveshare;
mod ptrcmp;

/// unallocated refs are [equivalent](Weak::ptr_eq) to [Weak::new()]
pub fn weak_is_null<T>(weak: &Weak<T>) -> bool {
    Weak::ptr_eq(weak, &Weak::new())
}

/// best-effort and not a replacement for [Weak::upgrade]
pub fn weak_is_dangling<T: ?Sized>(weak: &Weak<T>) -> bool {
    weak.strong_count() == 0
}

/// I'd care to make this correct but `Arc::is_unique` is unstable so
/// don't be pathological thanks
pub fn arc_is_unique<T>(arc: &Arc<T>) -> bool {
    Arc::strong_count(arc) == 1 && Arc::weak_count(arc) == 0
}

pub trait DefaultStatic {
    fn default_static() -> &'static Self;
}
impl DefaultStatic for str {
    fn default_static() -> &'static Self {
        ""
    }
}
impl<T: Sized> DefaultStatic for [T] {
    fn default_static() -> &'static Self {
        &[]
    }
}
impl DefaultStatic for CStr {
    fn default_static() -> &'static Self {
        c""
    }
}

/// `Box<T>`s that are expected to never be cleaned up or removed, thus can be &'static
///
/// TODO: consider actually using Box::leak if destructors on shutdown
/// is a terrible idea (it is a bad one, but...)
static DEFAULTS: RwLock<AnyMapSync> = RwLock::new(empty_any_map_sync());
fn leak_default_static<T>(v: Box<T>) -> &'static T {
    // TODO: consider try_write + downgrade silliness?
    log::error!("leak_default_static should not be hit");
    Box::leak(v)
}
pub fn default_static_of<T: Send + Sync + Default + 'static>() -> &'static T {
    get_default_static(T::default)
}
/// this is so dumb...
fn get_default_static<T: Send + Sync + 'static, F: FnOnce() -> T>(default: F) -> &'static T {
    let static_ = DEFAULTS
        .read()
        .map_err(drop)
        .map(|defaults| defaults.get::<Box<T>>().map(|d| &**d as *const T));
    let default = match static_ {
        Ok(Some(d)) => Ok(d),
        Err(..) => Err(Box::new(default())),
        Ok(None) => {
            let default = Box::new(default());
            match DEFAULTS.write() {
                Err(..) => Err(default),
                Ok(mut defaults) => Ok(&**defaults.entry::<Box<T>>().or_insert(default) as *const T),
            }
        },
    };
    match default {
        Ok(default) => unsafe { &*default },
        Err(default) => {
            log::error!("static defaults poisoned?");
            return leak_default_static(default)
        },
    }
}
impl<const N: usize, T: Sized + Default + Send + Sync + 'static> DefaultStatic for [T; N] {
    fn default_static() -> &'static Self {
        get_default_static(|| unsafe {
            let mut default = MaybeUninit::<[T; N]>::uninit();
            {
                let mut out = default.as_mut_ptr() as *mut T;
                for _ in 0..N {
                    out.write(T::default());
                    out = out.add(1);
                }
            }
            default.assume_init()
        })
    }
}
impl<T: Sized + Default + Send + Sync + 'static> DefaultStatic for Box<T> {
    fn default_static() -> &'static Self {
        get_default_static(|| Box::new(T::default()))
    }
}
impl<T: Sized + Send + Sync + 'static> DefaultStatic for Box<[T]> {
    fn default_static() -> &'static Self {
        get_default_static(Box::default)
    }
}
impl DefaultStatic for Box<str> {
    fn default_static() -> &'static Self {
        get_default_static(Box::default)
    }
}
impl<T: Sized + Default + Send + Sync + 'static> DefaultStatic for Arc<T> {
    fn default_static() -> &'static Self {
        get_default_static(|| Arc::new(T::default()))
    }
}
impl DefaultStatic for Arc<str> {
    fn default_static() -> &'static Self {
        get_default_static(Arc::default)
    }
}
impl<T: Sized + Send + Sync + 'static> DefaultStatic for Arc<[T]> {
    fn default_static() -> &'static Self {
        get_default_static(Arc::default)
    }
}
/// TODO: `Weak::new()` or downgraded `Arc::default_static()`???
#[cfg(todo)]
impl<T: Sized + Default + Send + Sync + 'static> DefaultStatic for Weak<T> {
    fn default_static() -> &'static Self {
        get_default_static(|| Arc::downgrade(Arc::default_static()))
    }
}
