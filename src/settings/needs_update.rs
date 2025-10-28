use {
    crate::fl,
    nexus::imgui::Ui,
    std::fmt::{self},
};

#[derive(PartialEq, Clone, Debug, Default)]
#[allow(dead_code)]
pub enum NeedsUpdate {
    #[default]
    Unknown,
    Error(String),
    Known(bool, String),
}

impl fmt::Display for NeedsUpdate {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use NeedsUpdate::*;
        match &self {
            Unknown => {
                let translation = fl!("update-unknown");
                write!(f, "{}", translation)
            },
            Error(e) => {
                let translation = fl!("update-error", error = e);
                write!(f, "{}", translation)
            },
            Known(true, id) => {
                let translation = fl!("update-available", version = id);
                write!(f, "{}", translation)
            },
            Known(false, _id) => {
                let translation = fl!("update-not-required");
                write!(f, "{}", translation)
            },
        }
    }
}

impl NeedsUpdate {
    #[allow(dead_code)]
    pub fn draw(&self, ui: &Ui) {
        let text = self.to_string();
        ui.text_wrapped(text);
    }
}
