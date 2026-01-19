#[cfg_attr(todo = "unnecessary", derive(PartialEq, Eq, PartialOrd, Ord, Hash))]
#[derive(Debug, Copy, Clone, Default)]
#[repr(transparent)]
pub struct FlatCollect<T>(pub T);

impl<T> FlatCollect<T> {
    #[inline]
    pub fn into_inner(self) -> T {
        let Self(inner) = self;
        inner
    }

    pub fn flatten_into<I>(dest: &mut T, iter: I) where
        I: IntoIterator,
        I::Item: IntoIterator,
        T: Extend<<I::Item as IntoIterator>::Item>,
    {
        dest.extend(iter.into_iter().flatten())
    }
    pub fn collect_from<I>(iter: I) -> T where
        I: IntoIterator,
        I::Item: IntoIterator,
        T: Default + Extend<<I::Item as IntoIterator>::Item>,
    {
        let mut dest = T::default();
        Self::flatten_into(&mut dest, iter);
        dest
    }
}
impl<T, E> Extend<E> for FlatCollect<T> where
    E: IntoIterator,
    T: Extend<E::Item>,
{
    #[inline]
    fn extend<I: IntoIterator<Item = E>>(&mut self, iter: I) {
        Self::flatten_into(&mut self.0, iter)
    }
}
impl<T, E> FromIterator<E> for FlatCollect<T> where
    E: IntoIterator,
    T: FromIterator<E::Item>,
{
    #[inline]
    fn from_iter<I: IntoIterator<Item = E>>(iter: I) -> Self {
        Self(iter.into_iter().flatten().collect())
    }
}
