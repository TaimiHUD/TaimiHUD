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
    fn is_state_empty(&self) -> bool {
        false
    }

    fn restore_state_mut(&mut self, context: &D3DC) {
        self.restore_state(context);
    }
    fn discard_state_mut(&mut self) {}
}

/// provided via macro instead
#[cfg(todo = "unnecessary")]
impl<D3DC: D3dContext, T: D3dState<D3DC>> D3dStateSnapshot<D3DC> for T
where
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
impl<D3DC: D3dContext, T: D3dState<D3DC>> D3dState<D3DC> for T
where
    [T]: D3dState<D3DC>,
{
    fn restore_state(&self, context: &D3DC) {
        let s = slice::from_ref(self);
        D3dState::restore_state(s, context)
    }
}
/// provided via macro instead
#[cfg(todo)]
impl<D3DC: D3dContext, T: D3dState<D3DC>> D3dState<D3DC> for Vec<T>
where
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

pub trait D3dContextStateExt: D3dContext {
    #[inline]
    fn get_snapshot<'c, S>(&'c self) -> D3dStateToken<'c, S, Self>
    where
        S: D3dState<Self> + D3dStateSnapshot<Self>,
    {
        D3dStateToken::new_snapshot(self)
    }
    #[inline]
    fn get_snapshot_buffers<'c, S>(&'c self) -> D3dStateToken<'c, BufferState<S>, Self>
    where
        S: D3dStateSnapshot<Self>,
        BufferState<S>: D3dState<Self>,
    {
        BufferState::new(0, S::snapshot_state(self)).to_state_token(self.to_ref())
    }
}
impl<D3DC: D3dContext> D3dContextStateExt for D3DC {}
pub trait D3dStateExt<D3DC: D3dContext>: D3dState<D3DC> {
    #[inline]
    fn to_state_token<'c>(self, context: InterfaceRef<'c, D3DC>) -> D3dStateToken<'c, Self, D3DC>
    where
        Self: Sized,
    {
        D3dStateToken { context: Some(context), state: self }
    }
}
impl<T, D3DC: D3dContext> D3dStateExt<D3DC> for T where T: D3dState<D3DC> {}
