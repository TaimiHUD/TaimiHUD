pub use self::{collect::FlatCollect, macros::impl_iter_wrap, mapfn::LazyMapFn};

mod macros;
mod collect;
mod mapfn;
pub mod tree;

pub trait IterExt: Sized + Iterator {
    /// [LazyMapFn]
    #[inline]
    fn lazy_map<R, F: FnMut(Self::Item) -> R>(self, map: F) -> LazyMapFn<Self, F> {
        LazyMapFn::new(map, self)
    }
    #[inline]
    fn lazy_clone<'a, I: 'a>(self) -> LazyMapFn<Self, impl Fn(&'a I) -> I> where
        Self: Iterator<Item = &'a I>,
        I: Clone,
    {
        self.lazy_map(I::clone)
    }
    fn unzip_flatten<FromA, FromB, A, B>(self) -> (FromA, FromB) where
        Self: Iterator<Item = (A, B)>,
        A: IntoIterator,
        B: IntoIterator,
        FromA: Default + Extend<A::Item>,
        FromB: Default + Extend<B::Item>,
    {
        let (FlatCollect(a), FlatCollect(b)) = self.unzip();
        (a, b)
    }
    fn unzip3_flatten<FromA, FromB, FromC, A, B, C>(self) -> (FromA, FromB, FromC) where
        Self: Iterator<Item = (A, B, C)>,
        A: IntoIterator,
        B: IntoIterator,
        C: IntoIterator,
        FromA: Default + Extend<A::Item>,
        FromB: Default + Extend<B::Item>,
        FromC: Default + Extend<C::Item>,
    {
        let (FlatCollect(a), FlatCollect(b), FlatCollect(c)) = self.collect();
        (a, b, c)
    }
    fn unzip4_flatten<FromA, FromB, FromC, FromD, A, B, C, D>(self) -> (FromA, FromB, FromC, FromD) where
        Self: Iterator<Item = (A, B, C, D)>,
        A: IntoIterator,
        B: IntoIterator,
        C: IntoIterator,
        D: IntoIterator,
        FromA: Default + Extend<A::Item>,
        FromB: Default + Extend<B::Item>,
        FromC: Default + Extend<C::Item>,
        FromD: Default + Extend<D::Item>,
    {
        let (FlatCollect(a), FlatCollect(b), FlatCollect(c), FlatCollect(d)) = self.collect();
        (a, b, c, d)
    }
}
impl<I: Sized + Iterator> IterExt for I {}

/// `F` must return true for all zipped pairs, and iter lengths must match
pub fn all_zipped<F, IL, IR>(mut f: F, lhs: IL, rhs: IR) -> bool
where
    F: FnMut(IL::Item, IR::Item) -> bool,
    IL: IntoIterator,
    IR: IntoIterator,
{
    let mut lhs = lhs.into_iter();
    let mut rhs = rhs.into_iter();
    match (lhs.size_hint(), rhs.size_hint()) {
        ((_, Some(lhs)), (_, Some(rhs))) if lhs != rhs => return false,
        ((min, None), (_, Some(max))) | ((_, Some(max)), (min, None)) if min > max => return false,
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
pub fn any_zipped<F, IL, IR>(mut f: F, lhs: IL, rhs: IR) -> bool
where
    F: FnMut(IL::Item, IR::Item) -> bool,
    IL: IntoIterator,
    IR: IntoIterator,
{
    !all_zipped::<_, IL, IR>(move |lhs, rhs| !f(lhs, rhs), lhs, rhs)
}
