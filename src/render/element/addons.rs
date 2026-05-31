use crate::{exports::runtime::imgui, settings::state::AddonHostName, with_i18n};

#[derive(Debug, Clone, Default)]
pub struct AddonHostSelection {
    pub host: Option<AddonHostName>,
    pub without_i18n: bool,
}

impl AddonHostSelection {
    /// disable [i18n](self.without_i18n) for use even if rest of addon is disabled
    pub const fn new_minimal(host: Option<AddonHostName>) -> Self {
        Self { host, without_i18n: true }
    }

    pub fn draw(&mut self, ui: &imgui::Ui, label_id: &str) -> Option<AddonHostName> {
        self.draw_inner(ui, label_id, false).flatten()
    }

    pub fn draw_opt(&mut self, ui: &imgui::Ui, label_id: &str) -> Option<Option<AddonHostName>> {
        self.draw_inner(ui, label_id, true)
    }

    fn draw_inner(
        &mut self,
        ui: &imgui::Ui,
        label_id: &str,
        optional: bool,
    ) -> Option<Option<AddonHostName>> {
        let mut new_selection = None;
        let none = match self.without_i18n {
            false => "disable",
            true => "Disable",
        };
        let combo = match self.without_i18n {
            false => {
                let host_id = self.host.map(|h| h.id()).unwrap_or(none);
                with_i18n!(label_id, |label| with_i18n!(host_id, |selected| ui
                    .begin_combo(&label, selected)))
            },
            true => {
                let host_name = self.host.map(|h| h.name()).unwrap_or(none);
                ui.begin_combo(label_id, host_name)
            },
        };
        if let Some(combo) = combo {
            let all = AddonHostName::ALL
                .into_iter()
                .map(Some)
                .chain(optional.then_some(None));
            for loader in all {
                let is_selected = self.host == loader;
                let selected = match self.without_i18n {
                    false => {
                        let id = loader.map(|l| l.id()).unwrap_or(none);
                        with_i18n!(id, |label| imgui::Selectable::new(&label)
                            .selected(is_selected)
                            .build(ui))
                    },
                    true => {
                        let name = loader.map(|l| l.name()).unwrap_or(none);
                        imgui::Selectable::new(&name).selected(is_selected).build(ui)
                    },
                };
                if selected {
                    new_selection = Some(loader);
                }
            }
            combo.end();
        }

        new_selection
    }
}
