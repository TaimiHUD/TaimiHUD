use crate::{prelude::*, state::{D3dStateSnapshot, D3dState}, D3dContextBindable, D3dContextBindableSlot};

#[must_use]
#[derive(Debug, Default)]
pub struct D3dStateToken<'c, S: ?Sized + D3dState<D3DC>, D3DC: D3dContext = crate::defaults::DxContext> {
    pub context: Option<InterfaceRef<'c, D3DC>>,
    pub state: S,
}

impl<'c, S: D3dStateSnapshot<D3DC>, D3DC: D3dContext> D3dStateToken<'c, S, D3DC> where
 S: D3dState<D3DC>,
{
    pub fn empty(device: &D3DC::IDevice) -> anyhow::Result<Self> {
        S::empty_state(device).map(|state| Self { state, context: None })
    }

    pub fn new_snapshot(context: &'c D3DC) -> Self {
        Self {
            state: S::snapshot_state(context),
            context: Some(context.to_ref()),
        }
    }
}
/// need interface cow from arcffi to make this work...
#[cfg(todo)]
impl<'c, S: D3dStateSnapshot<D3DC>, D3DC: D3dContext> D3dStateSnapshot<D3DC> for D3dStateToken<'c, S, D3DC> where
    S: D3dState<D3DC>,
{
    fn empty_state(device: &D3DC::IDevice) -> anyhow::Result<Self> {
        Self::empty(device)
    }
    fn snapshot_state(context: &'c D3DC) -> Self {
        Self::new_snapshot(context)
    }
}
impl<'c, S: D3dState<D3DC>, D3DC: D3dContext> D3dStateToken<'c, S, D3DC> {
    pub fn discard(mut self) {
        #[cfg(todo = "unnecessary")]
        {
            self.state.discard_state_mut();
        }
        self.discard_in_place();
        let _ = self.context.take();
        drop(self);
    }
    pub fn discard_in_place(&mut self) {
        self.context = None;
    }
    pub fn pop_in_place(&mut self) {
        self.restore_state();
        self.discard_in_place();
    }

    pub fn pop(self) {
        drop(self);
    }
}
impl<'c, S: ?Sized + D3dState<D3DC>, D3DC: D3dContext> D3dStateToken<'c, S, D3DC> {
    pub fn restore_state(&self) {
        if let Some(context) = &self.context {
            self.state.restore_state(&context);
        }
    }
}
#[cfg(todo)]
impl<'c, S: ?Sized + D3dStateMut<D3DC>, D3DC: D3dContext> D3dStateToken<'c, S, D3DC> {
    pub fn restore_state_mut(&mut self) {
        if let Some(context) = self.context.take() {
            self.state.restore_state_mut(&context);
        }
    }
}

impl<'c, S, D3DC: D3dContext> D3dContextBindable<D3DC> for D3dStateToken<'c, S, D3DC>
where
    S: D3dState<D3DC> + D3dContextBindable<D3DC>,
{
    fn set(&self, device_context: &D3DC) {
        self.state.set(device_context);
    }
}

impl<'c, S, D3DC: D3dContext> D3dContextBindableSlot<D3DC> for D3dStateToken<'c, S, D3DC>
where
    S: D3dState<D3DC> + D3dContextBindableSlot<D3DC>,
{
    fn set(&self, device_context: &D3DC, slot: u32) {
        self.state.set(device_context, slot);
    }
}

impl<'c, S: ?Sized + D3dState<D3DC>, D3DC: D3dContext> Drop for D3dStateToken<'c, S, D3DC> {
    fn drop(&mut self) {
        self.restore_state();
    }
}
