pub use self::{
    collection::TaimiCollection,
    dict::{TaimiDict, TaimiDictMut, TaimiDictStorage},
    extend::TaimiExtend,
    seq::{
        TaimiSeq,
        TaimiSeqGet,
        TaimiSeqIndex,
        TaimiSeqIndexMut,
        TaimiSeqKey,
        TaimiSeqMut,
        TaimiSeqStorage,
    },
    set::TaimiSet,
};
mod dict;
mod extend;
mod seq;
mod set;

mod collection {
    use std::{
        collections::{BTreeMap, BTreeSet, HashMap, HashSet},
        rc::Rc,
        sync::Arc,
    };

    pub trait TaimiCollection {
        fn collection_len(&self) -> usize;
    }

    impl<K, V> TaimiCollection for BTreeMap<K, V> {
        #[inline(always)]
        fn collection_len(&self) -> usize {
            self.len()
        }
    }
    impl<T> TaimiCollection for BTreeSet<T> {
        #[inline(always)]
        fn collection_len(&self) -> usize {
            self.len()
        }
    }
    impl<K, V, S> TaimiCollection for HashMap<K, V, S> {
        #[inline(always)]
        fn collection_len(&self) -> usize {
            self.len()
        }
    }
    impl<T, S> TaimiCollection for HashSet<T, S> {
        #[inline(always)]
        fn collection_len(&self) -> usize {
            self.len()
        }
    }
    impl<T> TaimiCollection for Vec<T> {
        #[inline(always)]
        fn collection_len(&self) -> usize {
            self.len()
        }
    }

    impl<T, const N: usize> TaimiCollection for [T; N] {
        #[inline(always)]
        fn collection_len(&self) -> usize {
            self.len()
        }
    }

    impl<T> TaimiCollection for [T] {
        #[inline(always)]
        fn collection_len(&self) -> usize {
            self.len()
        }
    }
    impl<T> TaimiCollection for Box<[T]> {
        #[inline(always)]
        fn collection_len(&self) -> usize {
            self.len()
        }
    }
    impl<T> TaimiCollection for Arc<[T]> {
        #[inline(always)]
        fn collection_len(&self) -> usize {
            self.len()
        }
    }
    impl<T> TaimiCollection for Rc<[T]> {
        #[inline(always)]
        fn collection_len(&self) -> usize {
            self.len()
        }
    }

    impl TaimiCollection for str {
        #[inline(always)]
        fn collection_len(&self) -> usize {
            self.len()
        }
    }
    impl TaimiCollection for Box<str> {
        #[inline(always)]
        fn collection_len(&self) -> usize {
            self.len()
        }
    }
    impl TaimiCollection for Arc<str> {
        #[inline(always)]
        fn collection_len(&self) -> usize {
            self.len()
        }
    }
    impl TaimiCollection for Rc<str> {
        #[inline(always)]
        fn collection_len(&self) -> usize {
            self.len()
        }
    }
}
