use {
    anyhow::Context,
    crate::{
        exports::runtime::{self as rt, imgui},
        game_language_id,
        LANGUAGE_LOADER,
        load_language,
        with_i18n,
    },
    i18n_embed::LanguageLoader,
};

#[derive(Debug, Clone, Default)]
pub struct LanguageSelection {
    pub language: Option<unic_langid_impl::subtags::Language>,
}

impl LanguageSelection {
    pub fn draw(&mut self, ui: &imgui::Ui) {
        let selected_language = *self.language.insert(Self::get_current_language());
        let selected_language = selected_language.as_str();

        if let Some(languages) = with_i18n!("language", |msg| ui.begin_combo(&msg, selected_language)) {
            let mut new_language = None;
            for l in crate::LANGUAGES_GAME {
                let id = game_language_id(l);
                let selected = imgui::Selectable::new(id)
                    .selected(selected_language == id)
                    .build(ui);
                if selected {
                    new_language = Some(Ok(l));
                }
            }
            for id in crate::LANGUAGES_EXTRA {
                let selected = imgui::Selectable::new(id)
                    .selected(selected_language == id)
                    .build(ui);
                if selected {
                    new_language = Some(Err(id));
                }
            }
            languages.end();

            if let Some(new_language) = new_language {
                let id = match new_language {
                    Ok(l) => game_language_id(l),
                    Err(id) => id,
                };
                let res = load_language(id)
                    .map_err(anyhow::Error::msg)
                    .with_context(|| format!("loading i18n for {id}"));
                if let Err(e) = res {
                    log::error!("{e:#}");
                }
            }
        }
    }

    pub fn get_language(&mut self) -> &mut unic_langid_impl::subtags::Language {
        self.language.get_or_insert_with(Self::get_current_language)
    }

    pub fn get_current_language() -> unic_langid_impl::subtags::Language {
        LANGUAGE_LOADER.current_language().language
    }

    pub fn is_default(&mut self) -> bool {
        self.get_language().as_str() == game_language_id(rt::GameLanguage::English)
    }
}
