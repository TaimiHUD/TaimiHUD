pub use self::{
    macros::impl_iter_wrap,
    mapfn::LazyMapFn,
};

mod macros;
mod mapfn;

pub trait IterExt: Sized + Iterator {
    /// [LazyMapFn]
    #[inline]
    fn lazy_map<R, F: FnMut(Self::Item) -> R>(self, map: F) -> LazyMapFn<Self, F> {
        LazyMapFn::new(map, self)
    }
}
impl<I: Sized + Iterator> IterExt for I {}
