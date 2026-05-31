use crate::{
    exports::runtime::imgui::{self, MouseButton},
    with_i18n,
};

#[derive(Debug, Clone, Default)]
pub struct ApiTokenInput {
    pub buffer: String,
}

impl ApiTokenInput {
    pub const DUMMY_CHAR: u8 = b'*';
    pub const DUMMY: &'static str = "***";
    /// protect user from themselves (and imgui input focus issues)
    ///
    /// this should still be well below what we expect from api tokens
    /// for github and arenanet, but beware if ever used for passwords?
    pub const MIN_TOKEN_LEN: usize = 10;

    pub fn new() -> Self {
        Self { buffer: String::new() }
    }

    pub fn update_preview(&mut self, has_token: bool) {
        self.buffer.clear();
        if has_token {
            self.buffer.push_str(Self::DUMMY);
        }
    }
    pub fn update_preview_with(&mut self, token: &str) {
        self.update_preview(!token.is_empty())
    }

    pub fn draw(&mut self, ui: &imgui::Ui, label_id: &str) -> Option<String> {
        let changed = self.draw_input(ui, label_id);
        self.draw_finish(ui, changed)
    }

    pub fn draw_input(&mut self, ui: &imgui::Ui, label_id: &str) -> bool {
        let changed = with_i18n(label_id, |label| {
            with_i18n!("add", |hint| {
                let is_unset = self.is_empty();
                let mut input = ui
                    .input_text(&label, &mut self.buffer)
                    .enter_returns_true(true)
                    .auto_select_all(true)
                    .password(true);
                if is_unset {
                    input = input.hint(&hint);
                }
                input.build()
            })
        });
        if !changed {
            let dirty = self.dirty_at();
            match dirty {
                None | Some(0) => (),
                Some(offset) => {
                    let _ = self.buffer.drain(..offset);
                },
            }
        }
        changed
    }

    pub fn draw_finish(&mut self, ui: &imgui::Ui, mut changed: bool) -> Option<String> {
        if !changed && self.is_dirty() {
            ui.same_line();
            if with_i18n!("save", |msg| ui.small_button(msg)) {
                changed = true;
            }
        }
        if !changed {
            return None
        }
        let token = match changed {
            true if self.buffer.len() < Self::MIN_TOKEN_LEN => {
                // protect user from themselves (and imgui input focus issues)
                log::info!("token input too short, ignoring");
                String::new()
            },
            true => self.buffer.clone(),
            false if ui.is_item_clicked_with_button(MouseButton::Right) => String::new(),
            false => return None,
        };
        self.update_preview_with(&token);
        Some(token)
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }
    pub fn is_dirty(&self) -> bool {
        self.dirty_at().is_some()
    }
    pub fn dirty_at(&self) -> Option<usize> {
        self.buffer.as_bytes().iter().position(|&c| c != Self::DUMMY_CHAR)
    }
}
