#[macro_export]
macro_rules! impl_iter_wrap {
    () => {};
    (
        impl{$($imp:tt)*} ExactSizeIterator for $ty:ty, $iter_size:ty
            $(where{$($where:tt)*})?
        {
            $(
                let iter = |&$this_iter_ref:ident| $iter_ref:expr;
            )?
            $(fn $($inner_size:tt)+)?
        }
        $($($rest:tt)+)?
    ) => {
        impl<$($imp)*> ::core::iter::ExactSizeIterator for $ty where
            $iter_size: ::core::iter::ExactSizeIterator,
            $($($where)*)?
        {
            $(
                fn len(&self) -> usize {
                    let $this_iter_ref = self;
                    ::core::iter::ExactSizeIterator::len($iter_ref)
                }
            )?
            $(fn $($inner_size)+)?
        }
        impl<$($imp)*> ::core::iter::FusedIterator for $ty where
            $iter_size: ::core::iter::FusedIterator,
            $($($where)*)?
        {}
        $(impl_iter_wrap!{$($rest)*})?
    };
    (
        impl{$($imp:tt)*} Iterator for $ty:ty, $iter_map:ty
            $(where{$($where:tt)*})?
        {
            type Item = $item:ty;
            $(
                let iter = |$this_iter_into:ident| $iter_into:expr;
                let iter = |&$this_iter_ref:ident| $iter_ref:expr;
            )?
            $(
                let iter = |&mut $this_iter_mut:ident| $iter_mut:expr;
                let item = |&mut $this_map:ident, $id_map:ident| $map_item:expr;
            )?
            $(fn $($inner_iter:tt)+)?
        }
        $($($rest:tt)+)?
    ) => {
        impl<$($imp)*> Iterator for $ty where
            $iter_map: Iterator,
            $($($where)*)?
        {
            type Item = $item;
            $(
                fn next(&mut self) -> Option<Self::Item> {
                    let item = {
                        let $this_iter_mut = &mut *self;
                        $iter_mut.next()
                    };
                    let $this_map = &mut *self;
                    item.map(|$id_map| $map_item)
                }
                fn nth(&mut self, i: usize) -> Option<Self::Item> {
                    let item = {
                        let $this_iter_mut = &mut *self;
                        $iter_mut.nth(i)
                    };
                    let $this_map = &mut *self;
                    item.map(|$id_map| $map_item)
                }
            )?

            $(
                fn size_hint(&self) -> (usize, Option<usize>) {
                    let $this_iter_ref = self;
                    $iter_ref.size_hint()
                }
                fn count(self) -> usize {
                    let $this_iter_into = self;
                    $iter_into.count()
                }
            )?
            $(fn $($inner_iter)+)?
        }
    };
    (
        impl{$($imp:tt)*} DoubleEndedIterator for $ty:ty, $iter_map:ty
            $(where{$($where:tt)*})?
        {
            $(
                let iter = |&mut $this_iter_mut:ident| $iter_mut:expr;
                let item = |&mut $this_map:ident, $id_map:ident| $map_item:expr;
            )?
            $(fn $($inner_double:tt)+)?
        }
        $($($rest:tt)+)?
    ) => {
        impl<$($imp)*> ::core::iter::DoubleEndedIterator for $ty where
            $iter_map: ::core::iter::DoubleEndedIterator,
            $($($where)*)?
        {
            $(
                fn next_back(&mut self) -> Option<<Self as Iterator>::Item> {
                    let item = {
                        let $this_iter_mut = &mut *self;
                        ::core::iter::DoubleEndedIterator::next_back($iter_mut)
                    };
                    let $this_map = &mut *self;
                    item.map(|$id_map| $map_item)
                }
                fn nth_back(&mut self, n: usize) -> Option<<Self as Iterator>::Item> {
                    let item = {
                        let $this_iter_mut = &mut *self;
                        ::core::iter::DoubleEndedIterator::nth_back($iter_mut, n)
                    };
                    let $this_map = &mut *self;
                    item.map(|$id_map| $map_item)
                }
            )?
            $(fn $($inner_double)+)?
        }
        $(impl_iter_wrap!{$($rest)*})?
    };
}
pub use impl_iter_wrap;
