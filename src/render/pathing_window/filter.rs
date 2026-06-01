use {
    super::PathingWindowState,
    crate::{
        render::element::prelude::*,
        settings::state::ui::pathing::{PathingFilterFlags, PathingSearchFlags},
        with_i18n,
    },
    regex::{Regex, RegexBuilder},
    std::collections::HashSet,
    taimi_pack::Pack,
};

#[derive(Clone)]
pub struct PathingSearchState {
    pub buffer: String,
    matcher: Option<Regex>,
    search_candidates: HashSet<String>,
    pub flags: PathingSearchFlags,
}

impl PathingSearchState {
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.matcher = None;
        self.clear_matches();
    }
    pub fn clear_matches(&mut self) {
        self.search_candidates.clear();
    }

    pub fn commit<'p, P, I>(&mut self, packs: I)
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Pack>,
    {
        self.search_candidates.clear();
        if self.buffer.is_empty() {
            return
        }
        self.matcher = {
            let pattern = regex::escape(&self.buffer);
            let matcher = RegexBuilder::new(&pattern)
                .case_insensitive(self.flags.contains(PathingSearchFlags::IGNORE_CASE))
                .ignore_whitespace(self.flags.contains(PathingSearchFlags::IGNORE_SPACE))
                .build();
            if let Err(e) = &matcher {
                log::warn!("search filter failure: {e:#}");
            }
            matcher.ok()
        };

        for pack in packs {
            let pack = pack.as_ref();

            for (full_id, category) in pack.categories.all_categories.iter() {
                if self.matches_name(category.display_name()) || self.matches_name(category.id().as_str()) {
                    self.search_candidates.insert(full_id.into());
                    let full_id = full_id.as_str();
                    let separators = full_id.rmatch_indices(".");
                    for (idx, _eu) in separators {
                        if let Some(sub_id) = full_id.get(..idx) {
                            self.search_candidates.insert(sub_id.into());
                        }
                    }
                }
            }
        }
    }

    pub fn matches_name(&self, name: &str) -> bool {
        match &self.matcher {
            Some(regex) => regex.is_match(name),
            #[cfg(todo = "unnecessary")]
            None if self.buffer.is_empty() => false,
            None => name.contains(&self.buffer),
        }
    }

    pub fn matches_id(&self, full_id: &str) -> bool {
        match self.buffer.is_empty() {
            false => self.search_candidates.contains(full_id),
            true => true,
        }
    }
}

impl Default for PathingSearchState {
    fn default() -> Self {
        Self {
            buffer: Default::default(),
            matcher: Default::default(),
            search_candidates: Default::default(),
            flags: PathingSearchFlags::DEFAULT,
        }
    }
}

impl PathingWindowState {
    pub fn draw_filters<'ui, U>(&mut self, ui: &mut U) -> bool
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let pushy = ui.push_id(c"pathing-search");
        let mut search_dirty = with_i18n!("pathing-search", |hint| ui.input_text_managed(
            c"",
            &mut self.search_state.buffer,
            64,
            Some(hint),
            None,
        ));
        ui.same_line();
        if ui.button(c"X") {
            self.search_state.clear();
            self.ui_state.write_if(|s| {
                s.search.query.clear();
                None
            });
        }
        if ui.is_item_hovered() {
            with_i18n!("pathing-search-clear", |msg| ui.tooltip_text(msg));
        }
        let search_flags = PathingSearchFlags::all()
            .iter()
            .filter_map(|search_flag| search_flag.as_str().map(|name| (search_flag, name)));
        for (flag, search_flag_name) in search_flags {
            ui.same_line();
            search_dirty |= with_i18n!(search_flag_name, |name| ui.checkbox_flags(
                name,
                &mut self.search_state.flags,
                flag
            ));
        }
        pushy.end();
        ui.dummy([4.0; 2]);
        with_i18n!("filter-options", |msg| ui.text(msg));
        let filters = PathingFilterFlags::USER
            .iter()
            .filter_map(|filter| filter.as_str().map(|name| (filter, name)));
        for (i, (flag, filter_name)) in filters.enumerate() {
            if i > 0 && i % 3 != 0 {
                ui.same_line();
            }
            with_i18n!(filter_name, |name| ui.checkbox_flags(
                name,
                &mut self.filter_state,
                flag
            ));
        }

        search_dirty
    }
}
