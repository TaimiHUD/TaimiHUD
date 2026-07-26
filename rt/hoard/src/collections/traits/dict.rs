use {
    crate::collections::traits::TaimiCollection,
    std::collections::{BTreeMap, BTreeSet, HashMap, HashSet},
};

pub trait TaimiDict<K, V>: TaimiCollection {
    #[inline(always)]
    fn dict_len(&self) -> usize {
        self.collection_len()
    }
}
pub trait TaimiDictMut<K, V>: TaimiDict<K, V> {
    fn dict_retain_mut<F: FnMut(&K, &mut V) -> bool>(&mut self, f: F);

    /// returns amount removed
    fn dict_retain_mut_count<F: FnMut(&K, &mut V) -> bool>(&mut self, f: F) -> usize {
        let prev_len = self.dict_len();
        self.dict_retain_mut(f);
        prev_len - self.dict_len()
    }
    /// [Self::retain_mut] but additionally call `damage` for each item being removed
    fn dict_retain_mut_damaged<F: FnMut(&K, &mut V) -> bool, D: FnMut(&K, &mut V)>(
        &mut self,
        mut f: F,
        mut damage: D,
    ) {
        self.dict_retain_mut(move |k, v| {
            let retain = f(k, v);
            if !retain {
                damage(k, v);
            }
            retain
        });
    }
}
pub trait TaimiDictStorage:
    TaimiDict<<Self as TaimiDictStorage>::Key, <Self as TaimiDictStorage>::Value>
{
    type Key;
    type Value;
}

impl<K, V> TaimiDictStorage for BTreeMap<K, V> {
    type Key = K;
    type Value = V;
}
impl<K, V> TaimiDict<K, V> for BTreeMap<K, V> {}
impl<K, V> TaimiDictMut<K, V> for BTreeMap<K, V>
where
    K: Ord,
{
    #[inline]
    fn dict_retain_mut<F: FnMut(&K, &mut V) -> bool>(&mut self, f: F) {
        self.retain(f)
    }
}
impl<T> TaimiDictStorage for BTreeSet<T> {
    type Key = T;
    type Value = ();
}
impl<T> TaimiDict<T, ()> for BTreeSet<T> {}
impl<T> TaimiDictMut<T, ()> for BTreeSet<T>
where
    T: Ord,
{
    #[inline]
    fn dict_retain_mut<F: FnMut(&T, &mut ()) -> bool>(&mut self, mut f: F) {
        self.retain(|k| f(k, &mut ()))
    }
}

impl<K, V, S> TaimiDictStorage for HashMap<K, V, S> {
    type Key = K;
    type Value = V;
}
impl<K, V, S> TaimiDict<K, V> for HashMap<K, V, S> {}
impl<K, V, S> TaimiDictMut<K, V> for HashMap<K, V, S> {
    #[inline]
    fn dict_retain_mut<F: FnMut(&K, &mut V) -> bool>(&mut self, f: F) {
        self.retain(f)
    }
}
impl<T, S> TaimiDictStorage for HashSet<T, S> {
    type Key = T;
    type Value = ();
}
impl<T, S> TaimiDict<T, ()> for HashSet<T, S> {}
impl<T, S> TaimiDictMut<T, ()> for HashSet<T, S> {
    #[inline]
    fn dict_retain_mut<F: FnMut(&T, &mut ()) -> bool>(&mut self, mut f: F) {
        self.retain(|k| f(k, &mut ()))
    }
}
