use {
    core::{borrow::Borrow, cmp, hash::Hash},
    std::{collections::{BTreeMap, BTreeSet, HashMap}, sync::Arc},
};

pub trait TaimiSet<T: ?Sized> {
    fn set_contains(&self, elem: &T) -> bool;
}
impl<K, V, T: ?Sized> TaimiSet<T> for HashMap<K, V> where
    K: Borrow<T> + Hash + Eq,
    T: Hash + Eq,
{
    fn set_contains(&self, elem: &T) -> bool {
        self.contains_key(elem)
    }
}
impl<K, V, T: ?Sized> TaimiSet<T> for BTreeMap<K, V> where
    K: Borrow<T> + Ord,
    T: Ord,
{
    fn set_contains(&self, elem: &T) -> bool {
        self.contains_key(elem)
    }
}
impl<K, T: ?Sized> TaimiSet<T> for BTreeSet<K> where
    K: Borrow<T> + Ord,
    T: Ord,
{
    fn set_contains(&self, elem: &T) -> bool {
        self.contains(elem)
    }
}
#[cfg(todo)]
impl<E, T: ?Sized> TaimiSet<T> for [E] where
    E: PartialEq<T>,
{
    fn set_contains(&self, elem: &T) -> bool {
        self.iter().any(|e| e == elem)
    }
}
impl<E, T: ?Sized> TaimiSet<T> for [E] where
    E: Borrow<T>,
    T: PartialEq,
{
    fn set_contains(&self, elem: &T) -> bool {
        self.iter().any(|e| e.borrow() == elem)
    }
}
impl<E, T: ?Sized> TaimiSet<T> for Vec<E> where
    [E]: TaimiSet<T>,
{
    fn set_contains(&self, elem: &T) -> bool {
        TaimiSet::set_contains(&self[..], elem)
    }
}
impl<E, T: ?Sized> TaimiSet<T> for Box<[E]> where
    [E]: TaimiSet<T>,
{
    fn set_contains(&self, elem: &T) -> bool {
        TaimiSet::set_contains(&self[..], elem)
    }
}
impl<E, T: ?Sized> TaimiSet<T> for Arc<[E]> where
    [E]: TaimiSet<T>,
{
    fn set_contains(&self, elem: &T) -> bool {
        TaimiSet::set_contains(&self[..], elem)
    }
}
impl<C: ?Sized + TaimiSet<T>, T: ?Sized> TaimiSet<T> for &'_ C {
    fn set_contains(&self, elem: &T) -> bool {
        TaimiSet::set_contains(*self, elem)
    }
}
impl<C: ?Sized + TaimiSet<T>, T: ?Sized> TaimiSet<T> for &'_ mut C {
    fn set_contains(&self, elem: &T) -> bool {
        TaimiSet::set_contains(*self, elem)
    }
}
/// `true` contains everything, `false` has nothing
impl<T: ?Sized> TaimiSet<T> for bool {
    fn set_contains(&self, _elem: &T) -> bool {
        *self
    }
}
impl<C: /*?Sized +*/ TaimiSet<T>, T: ?Sized> TaimiSet<T> for cmp::Reverse<C> {
    fn set_contains(&self, elem: &T) -> bool {
        !self.0.set_contains(elem)
    }
}
impl<C: ?Sized, T: ?Sized> TaimiSet<T> for (C,) where
    C: PartialEq<T>,
{
    fn set_contains(&self, elem: &T) -> bool {
        &self.0 == elem
    }
}
