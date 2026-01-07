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

/// `F` must return true for all zipped pairs, and iter lengths must match
pub fn all_zipped<F, IL, IR>(mut f: F, lhs: IL, rhs: IR) -> bool where
    F: FnMut(IL::Item, IR::Item) -> bool,
    IL: IntoIterator,
    IR: IntoIterator,
{
    let mut lhs = lhs.into_iter();
    let mut rhs = rhs.into_iter();
    match (lhs.size_hint(), rhs.size_hint()) {
        ((_, Some(lhs)), (_, Some(rhs))) if lhs != rhs =>
            return false,
        ((min, None), (_, Some(max))) | ((_, Some(max)), (min, None)) if min > max =>
            return false,
        _ => (),
    }
    while let Some(lhs) = lhs.next() {
        match rhs.next() {
            Some(rhs) => match f(lhs, rhs) {
                true => continue,
                _ => (),
            },
            _ => (),
        }
        return false
    }
    rhs.next().is_none()
}
pub fn any_zipped<F, IL, IR>(mut f: F, lhs: IL, rhs: IR) -> bool where
    F: FnMut(IL::Item, IR::Item) -> bool,
    IL: IntoIterator,
    IR: IntoIterator,
{
    !all_zipped::<_, IL, IR>(move |lhs, rhs| !f(lhs, rhs), lhs, rhs)
}
