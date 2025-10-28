use crate::{prelude::*, state::D3dState, D3dContextBindable, D3dContextBindableSlot};

#[must_use]
#[derive(Debug, Default)]
pub struct D3dStateToken<'c, S: D3dState<D3DC>, D3DC: D3dContext = crate::defaults::DxContext> {
    pub context: Option<InterfaceRef<'c, D3DC>>,
    pub state: S,
}

impl<'c, S: D3dState<D3DC>, D3DC: D3dContext> D3dStateToken<'c, S, D3DC> {
    pub fn empty(device: &D3DC::IDevice) -> anyhow::Result<Self> {
        S::empty_state(device).map(|state| Self {
            state,
            context: None,
        })
    }

    pub fn new_snapshot(context: &'c D3DC) -> Self {
        Self {
            state: S::snapshot_state(context),
            context: Some(context.to_ref()),
        }
    }

    pub fn discard(mut self) {
        self.state.discard_state_mut();
        let _ = self.context.take();
        drop(self);
    }

    pub fn pop(self) {
        drop(self);
    }

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

impl<'c, S: D3dState<D3DC>, D3DC: D3dContext> Drop for D3dStateToken<'c, S, D3DC> {
    fn drop(&mut self) {
        self.restore_state_mut();
    }
}

#[cfg(todo)]
impl<'c, S: D3dState<D3DC>, D3DC: D3dContext> D3dState for D3dStateToken<'c, S, D3DC> {}
