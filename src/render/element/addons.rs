use crate::{
    exports::runtime::imgui,
    settings::state::AddonHostName,
    with_i18n,
};

#[derive(Debug, Clone, Default)]
pub struct AddonHostSelection {
    pub host: Option<AddonHostName>,
}

impl AddonHostSelection {
    pub fn draw(&mut self, ui: &imgui::Ui, label_id: &str) -> Option<AddonHostName> {
        self.draw_inner(ui, label_id, false).flatten()
    }

    pub fn draw_opt(&mut self, ui: &imgui::Ui, label_id: &str) -> Option<Option<AddonHostName>> {
        self.draw_inner(ui, label_id, true)
    }

    fn draw_inner(&mut self, ui: &imgui::Ui, label_id: &str, optional: bool) -> Option<Option<AddonHostName>> {
        let mut new_selection = None;
        let none = "disable";
        let host_id = self.host.map(|h| h.id()).unwrap_or(none);
        let combo = with_i18n!(label_id, |label| with_i18n!(host_id, |selected|
            ui.begin_combo(&label, selected)
        ));
        if let Some(combo) = combo {
            let all = AddonHostName::ALL.into_iter().map(Some)
                .chain(optional.then_some(None));
            for loader in all {
                let id = loader.map(|l| l.id()).unwrap_or(none);
                let selected = with_i18n!(id, |msg| imgui::Selectable::new(&msg)
                    .selected(self.host == loader)
                    .build(ui));
                if selected {
                    new_selection = Some(loader);
                }
            }
            combo.end();
        }

        new_selection
    }
}
