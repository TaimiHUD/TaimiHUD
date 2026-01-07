use {core::mem, std::sync::Arc};

pub trait TaimiExtend<T> {
    fn current_len(&self) -> usize;
    fn extend_from<I: IntoIterator<Item = T>>(&mut self, items: I);
}
/// specialization :<
#[cfg(todo)]
impl<C, T> TaimiExtend<T> for C
where
    C: Extend<T>,
{
    fn extend_from<I: IntoIterator<Item = T>>(&mut self, items: I) {
        Extend::extend(self, items)
    }
}
impl<T, E> TaimiExtend<T> for Vec<E>
where
    T: Into<E>,
{
    fn current_len(&self) -> usize {
        self.len()
    }
    fn extend_from<I: IntoIterator<Item = T>>(&mut self, items: I) {
        Extend::extend(self, items.into_iter().map(Into::into))
    }
}
impl<T, E> TaimiExtend<T> for Box<[E]>
where
    T: Into<E>,
{
    fn current_len(&self) -> usize {
        self.len()
    }
    fn extend_from<I: IntoIterator<Item = T>>(&mut self, items: I) {
        let mut collection = Vec::from(mem::take(self));
        collection.extend_from(items);
        *self = collection.into_boxed_slice();
    }
}
impl<T, E> TaimiExtend<T> for Arc<[E]>
where
    T: Into<E>,
    E: Clone,
{
    fn current_len(&self) -> usize {
        self.len()
    }
    fn extend_from<I: IntoIterator<Item = T>>(&mut self, items: I) {
        let mut collection = Vec::from(&self[..]);
        collection.extend_from(items);
        *self = collection.into();
    }
}
