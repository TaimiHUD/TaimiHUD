use {crate::render::element::prelude::*, core::fmt};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct CheckboxState {
    pub status: ItemStatus,
    pub changed: ItemStatus,
}
impl CheckboxState {
    pub const FLAG_STATE: ItemStatus = ItemStatus::TRIGGER;
    pub const FLAG_CONTEXT: ItemStatus = ItemStatus::OPEN;

    #[inline]
    pub fn check_state(&self) -> bool {
        self.status.contains(Self::FLAG_STATE)
    }
}
#[derive(Debug)]
pub struct CheckboxScratch {
    pub label: String0,
}
#[derive(Debug)]
pub struct CheckboxDesc<L> {
    pub label: L,
}
#[derive(Debug)]
pub struct CheckboxDraw<'d, 's, L> {
    pub desc: &'d CheckboxDesc<L>,
    pub state: &'s mut CheckboxState,
    pub scratch: &'s mut CheckboxScratch,
}

impl<'d, 's, L> CheckboxDraw<'d, 's, L>
where
    L: fmt::Display,
{
    pub fn draw<'ui, U, C>(&mut self, ui: &mut U, context: &mut C)
    where
        U: ?Sized + ImDrawWindow<'ui>,
        C: ?Sized + AsRef<UiConfig> + AsRef<UiState> + AsMut<UiFrameState> + DrawContextSignal<'ui>,
    {
        if self.scratch.label.is_empty() {
            self.scratch.label = String0::format(&self.desc.label);
        }
        let mut state = self.state.check_state();
        let prev_status = self.state.status;
        if ui.checkbox(self.scratch.label.as_c_str(), &mut state) {
            self.state.status.toggle(CheckboxState::FLAG_STATE);
        } else {
            self.state.status.set(ItemStatus::HOVER, ui.item_is_hovered());
            #[cfg(todo)]
            {
                self.state
                    .status
                    .set(CheckboxState::FLAG_CONTEXT, ui.is_item_right_clicked());
            }
        }
        context.mask_and_signal_slot(&mut self.state.changed, self.state.status ^ prev_status);
    }
}
