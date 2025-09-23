use crate::{
    prelude::*,
    D3dContextBindable, D3dContextBindableSlot,
};

mod token;
pub mod primitivetopology;

pub use self::{
    primitivetopology::PrimitiveTopology,
    token::D3dStateToken,
};

pub trait D3dState<D3DC: D3dContext> {
    fn empty_state(device: &D3DC::IDevice) -> anyhow::Result<Self> where
        Self: Sized;
    fn snapshot_state(context: &D3DC) -> Self where
        Self: Sized;

    fn restore_state(&self, context: &D3DC);

    fn restore_state_mut(&mut self, context: &D3DC) {
        self.restore_state(context)
    }
    fn discard_state_mut(&mut self) {}
}

pub struct BufferState<B> {
    pub slot: u32,
    pub buffer: B,
}

impl<D3DC: D3dContext, B> D3dContextBindable<D3DC> for BufferState<B> where
    B: D3dContextBindableSlot<D3DC>,
{
    fn set(&self, context: &D3DC) {
        self.buffer.set(context, self.slot)
    }
}
