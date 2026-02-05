use crate::{prelude::*, D3dContextBindable, D3dContextBindableSlot};

pub mod primitivetopology;
mod token;

pub use self::{primitivetopology::PrimitiveTopology, token::D3dStateToken};

pub trait D3dStateSnapshot<D3DC: D3dContext>: Sized {
    fn empty_state(device: &D3DC::IDevice) -> anyhow::Result<Self>;
    fn snapshot_state(context: &D3DC) -> Self;
}
pub trait D3dState<D3DC: D3dContext> {
    fn restore_state(&self, context: &D3DC);
}

#[cfg(todo)]
pub trait D3dStateMut<D3DC: D3dContext>: D3dState<D3DC> {
    fn is_state_empty(&self) -> bool { false }

    fn restore_state_mut(&mut self, context: &D3DC) {
        self.restore_state(context);
    }
    fn discard_state_mut(&mut self) {}
}

/// provided via macro instead
#[cfg(todo = "unnecessary")]
impl<D3DC: D3dContext, T: D3dState<D3DC>> D3dStateSnapshot<D3DC> for T where
    [T; 1]: D3dStateSnapshot<D3DC>,
{
    fn empty_state(device: &D3DC::IDevice) -> anyhow::Result<Self> {
        <[T; 1] as D3dStateSnapshot<D3DC>>::empty_state(device).map(|[s]| s)
    }
    fn snapshot_state(context: &D3DC) -> Self {
        let [s] = D3dStateSnapshot::snapshot_state(context);
        s
    }
}
/// provided via macro instead
#[cfg(todo)]
impl<D3DC: D3dContext, T: D3dState<D3DC>> D3dState<D3DC> for T where
    [T]: D3dState<D3DC>,
{
    fn restore_state(&self, context: &D3DC) {
        let s = slice::from_ref(self);
        D3dState::restore_state(s, context)
    }
}
/// provided via macro instead
#[cfg(todo)]
impl<D3DC: D3dContext, T: D3dState<D3DC>> D3dState<D3DC> for Vec<T> where
    [T]: D3dState<D3DC>,
{
    fn restore_state(&self, context: &D3DC) {
        D3dState::restore_state(&self[..], context)
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct BufferState<B> {
    pub slot: u32,
    pub buffer: B,
}

impl<B> BufferState<B> {
    #[inline]
    pub const fn new(slot: u32, buffer: B) -> Self {
        Self { slot, buffer }
    }
}
impl<D3DC: D3dContext, B> D3dContextBindable<D3DC> for BufferState<B>
where
    B: D3dContextBindableSlot<D3DC>,
{
    fn set(&self, context: &D3DC) {
        self.buffer.set(context, self.slot)
    }
}
impl<D3DC: D3dContext, B> D3dStateSnapshot<D3DC> for BufferState<B>
where
    B: D3dStateSnapshot<D3DC>,
{
    fn empty_state(device: &D3DC::IDevice) -> anyhow::Result<Self> {
        B::empty_state(device).map(|buffer| Self::new(0, buffer))
    }
    fn snapshot_state(context: &D3DC) -> Self {
        Self::new(0, B::snapshot_state(context))
    }
}

impl_d3d! {
    impl{D3DC, B} D3dState<D3DC> for BufferState<B>;
}
