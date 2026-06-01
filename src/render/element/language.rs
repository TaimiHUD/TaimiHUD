use crate::render::{
    element::prelude::*,
    i18n::{self, LanguageIdentifier, LANGUAGES_GAME},
};

#[derive(Debug, Clone, Default)]
pub struct LanguageSelection {
    pub language: Option<i18n::LanguageIdentifier>,
}

impl LanguageSelection {
    pub fn draw<'ui, U>(&mut self, ui: &'_ mut U) -> Option<Option<&LanguageIdentifier>>
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let selected_language = &*self.language.insert(i18n::current_language());

        let mut new_language = None;
        if let Some(languages) =
            with_i18n!("language", |msg| ui.begin_combo(msg, im_to_s!(selected_language)))
        {
            let available = i18n::available_languages();
            let supported_suffix = move |lang: i18n::unic_subtags::Language| {
                let is_supported = available.iter().any(|av| av.language == lang);
                fmt_opt((!is_supported).then_some(fmt_args!(" ({})", fl!("language-unsupported"))))
            };
            let lang_label = |l: &'static LanguageIdentifier| {
                ImStrId::new(l, fmt_args!("{l}{}", supported_suffix(l.language)))
            };
            for l in LANGUAGES_GAME.iter() {
                let is_selected = selected_language.language == l.language;
                let selected = ui.selectable(lang_label(l), is_selected);
                if selected {
                    new_language = Some(Some(l));
                }
            }
            let available = available.iter().filter(|&av| !LANGUAGES_GAME.contains(av));
            if available.clone().next().is_some() {
                ui.separator();
            }
            for l in available.clone() {
                if LANGUAGES_GAME.contains(l) {
                    continue
                }
                let is_selected = selected_language == l;
                let selected = ui.selectable(lang_label(l), is_selected);
                if selected {
                    new_language = Some(Some(l));
                }
            }
            #[cfg(feature = "extension-nexus")]
            {
                ui.separator();
                let nexus_langs = crate::exports::nexus::LANGUAGES_EXTRA
                    .iter()
                    .filter(|l| available.clone().any(|av| av.language == l.language));
                for l in nexus_langs {
                    let is_selected = selected_language.language == l.language;
                    let selected = ui.selectable(lang_label(l), is_selected);
                    if selected {
                        new_language = Some(Some(l));
                    }
                }
            }
            languages.end();
        } else if ui.is_item_right_clicked() {
            new_language = Some(None);
        }

        match new_language {
            None => None,
            Some(None) => {
                self.language = None;
                Some(None)
            },
            Some(Some(lang)) => Some(Some(&*self.language.insert(lang.clone()))),
        }
    }

    pub fn get_language(&mut self) -> &i18n::LanguageIdentifier {
        &*self.language.get_or_insert_with(i18n::current_language)
    }

    pub fn is_default(&mut self) -> bool {
        self.get_language().language == i18n::fallback_language().language
    }
}
