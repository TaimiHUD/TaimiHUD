//! TODO: redo and sort/move methods etc, this was rushed

use {crate::im::prelude::*, arcffi::cstr::CStrPtr};

pub trait ImTableStack<'ui> {
    fn begin_table_dyn_untyped(
        &mut self,
        ident: &mut dyn ImStr,
        columns: u32,
        untyped_flags: Option<u32>,
        outer_size: Option<ImSize2>,
        inner_width: Option<f32>,
    ) -> Option<UiTokenDyn<'ui>>;
}
pub trait ImTable {
    fn table_current_row(&self) -> u32;
    fn table_current_column(&self) -> u32;
    fn table_column_name(&self, column: u32) -> Option<CStrPtr<'_>>;
    fn table_column_count(&self) -> u32;
    fn table_column_set_width(&mut self, column: u32, width: f32);

    fn table_header_height(&self) -> f32;
    fn table_header_row(&mut self);

    fn table_advance_column(&mut self, column: u32) -> bool;
    fn table_next_column(&mut self) -> bool;
    fn table_next_row_with(&mut self, min_height: Option<f32>);
    fn table_header_dyn(&mut self, text: &mut dyn ImStr);
    /// ew
    fn table_next_row_untyped(&mut self, untyped_flags: Option<u32>, min_height: Option<f32>);
    /// ew
    fn table_column_setup_dyn_untyped(
        &mut self,
        name: Option<&mut dyn ImStr>,
        untyped_flags: Option<u32>,
        init_size: Option<f32>,
        user_id: u32,
    );
}
pub trait ImTableExt: ImTable {
    #[inline(always)]
    fn table_next_row(&mut self) {
        self.table_next_row_with(None)
    }
    #[inline(always)]
    fn table_headers_row(&mut self) {
        self.table_header_row()
    }
    #[inline(always)]
    fn table_get_row_index(&self) -> u32 {
        self.table_current_row()
    }

    #[inline(always)]
    fn next_column(&mut self)
    where
        Self: ImTableLegacy,
    {
        self.table_legacy_columns_next();
    }
    #[inline(always)]
    fn columns<I>(&mut self, count: u32, ident: I, border: bool)
    where
        Self: ImTableLegacy,
        I: IntoImStrId,
    {
        let mut ident = ident.im_into_id();
        self.table_legacy_columns_dyn(count, &mut ident, border)
    }

    #[inline(always)]
    fn begin_table_with_flags<'ui, I>(
        &mut self,
        mut ident: I,
        columns: usize,
        flags: imw::DynFlagsContainer,
    ) -> Option<UiTokenDyn<'ui>>
    where
        Self: ImTableStack<'ui>,
        I: ImStrExt,
    {
        ident.with_imstr_dyn(|ident| {
            self.begin_table_dyn_untyped(ident, columns as _, flags.untyped_flags(), None, None)
        })
    }
    #[inline(always)]
    fn table_column_setup_untyped<S>(
        &mut self,
        mut name: Option<S>,
        untyped_flags: Option<u32>,
        init_size: Option<f32>,
        user_id: u32,
    ) where
        S: ImStr,
    {
        let name = name.as_mut().map(|n| n as &mut dyn ImStr);
        self.table_column_setup_dyn_untyped(name, untyped_flags, init_size, user_id);
    }
    #[inline]
    fn table_sort_is_dirty(&self) -> bool
    where
        Self: ImTableSort,
    {
        self.table_sort_specs_dyn()
            .map(|sort| sort.is_dirty())
            .unwrap_or(false)
    }
    #[inline]
    fn table_sort_dyn_if_dirty(&mut self) -> Option<&mut dyn ImTableSortSpecs>
    where
        Self: ImTableSort,
    {
        self.table_sort_specs_mut_dyn()
            .and_then(|sort| sort.is_dirty().then_some(sort))
    }
}
impl<U: ?Sized + ImTable> ImTableExt for U {}
pub trait ImTableLegacy {
    fn table_legacy_columns_dyn(&mut self, count: u32, ident: &mut dyn ImStr, border: bool);
    fn table_legacy_columns_next(&mut self);
}

pub trait ImTableSort {
    #[cfg(todo)]
    fn with_table_sort_specs_dyn_mut(&mut self, f: &mut dyn FnMut(&mut dyn TableSortSpecs));
    fn table_sort_specs_dyn(&self) -> Option<&dyn ImTableSortSpecs>;
    fn table_sort_specs_mut_dyn(&mut self) -> Option<&mut dyn ImTableSortSpecs>;
}
pub trait ImTableSortSpecs {
    fn is_dirty(&self) -> bool;
    fn set_dirty(&mut self, dirty: bool);
    /// TODO: consider the `dyn_slice` crate if unstable is acceptable?
    /// or a dyn collection trait otherwise idk :<
    fn specs<'a>(&'a self) -> Box<dyn Iterator<Item = &'a dyn ImTableSortColumn> + 'a>;
    #[cfg(todo)]
    fn specs(&self) -> DynSlice<'_, dyn ImTableSortColumn>;
    #[cfg(todo)]
    fn specs_mut(&mut self);
}
pub trait ImTableSortColumn {
    fn user_id(&self) -> u32;
    /// index
    fn column(&self) -> u32;
    /// TODO: currently redundant because specs will be ordered by imgui
    fn priority(&self) -> isize;
    /// TODO: ordering enum? cmp::Ordering?
    fn is_ascending(&self) -> Option<bool>;
}
pub trait ImTableSortSpecsExt: ImTableSortSpecs {
    #[inline(always)]
    fn mark_clean(&mut self) {
        self.set_dirty(false);
    }
}
impl<U: ?Sized + ImTableSortSpecs> ImTableSortSpecsExt for U {}
