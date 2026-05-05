//! TODO: redo and sort/move methods etc, this was rushed

use crate::im::prelude::*;

pub trait ImTreeStack<'ui> {
    fn tree_begin_dyn_untyped(
        &mut self,
        name: &mut dyn ImStr,
        untyped_flags: Option<u32>,
    ) -> UiTokenDyn<'ui>;
}
pub trait ImTree {}
pub trait ImTreeExt: ImTree {}
