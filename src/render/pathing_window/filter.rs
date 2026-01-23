use {
    super::PathingWindowState,
    crate::{
        render::element::prelude::*,
        settings::state::ui::pathing::{PathingFilterFlags, PathingSearchFlags},
        render::{
            machine::RenderMachine,
            element::pack::{CategorySearchQuery},
        },
        with_i18n,
    },
    regex::{Regex, RegexBuilder},
    taimi_hoard::str_opt,
};

#[derive(Clone)]
pub struct PathingSearchState {
    pub buffer: String,
    matcher: Option<Regex>,
    #[cfg(todo = "unnecessary")]
    search_candidates: HashSet<String>,
    /// TODO: BTreeSet<CategoryPath> instead?
    #[cfg(deleteme)]
    pub candidate_mask: BTreeMap<PackPath, BitSet>,
    pub flags: PathingSearchFlags,
}

impl PathingSearchState {
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.matcher = None;
    }
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    pub fn query_str(&self) -> Option<&String> {
        let buffer = str_opt(&self.buffer)?;
        match self.flags {
            flags if flags.contains(PathingSearchFlags::PATTERN_REGEX) && self.matcher.is_none() =>
                None,
            _ => Some(buffer),
        }
    }

    pub fn commit(&mut self) {
        self.clear();
        if self.buffer.is_empty() {
            self.buffer = Default::default();
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
    }
    pub fn to_query(&self) -> Option<CategorySearchQuery> {
        match &self.matcher {
            Some(matcher) => Some(CategorySearchQuery::with_matcher(matcher.clone())),
            None if self.buffer.is_empty() => None,
            None => Some(CategorySearchQuery::with_fixed(self.buffer.clone())),
        }
    }

    #[cfg(todo = "unused")]
    pub fn matches_name(&self, name: &str) -> bool {
        match &self.matcher {
            Some(regex) => regex.is_match(name),
            #[cfg(todo = "unnecessary")]
            None if self.buffer.is_empty() => false,
            None => name.contains(&self.buffer),
        }
    }

    #[cfg(todo = "unused")]
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
            #[cfg(todo = "unnecessary")]
            search_candidates: Default::default(),
            #[cfg(deleteme)]
            candidate_mask: Default::default(),
            flags: PathingSearchFlags::DEFAULT,
        }
    }
}

impl PathingWindowState {
    pub fn draw_filters<'ui, U>(&mut self, ui: &mut U, machine: &mut RenderMachine) -> bool
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
            machine.pack_ui_state.clear_search_filter();
            self.ui_state.write_if(|s| {
                s.search.query.clear();
                None
            });
        }
        if ui.is_item_hovered() {
            with_i18n!("pathing-search-clear", |msg| ui.tooltip_text(msg));
        }
        if !self.search_state.buffer.is_empty() {
            let search_flags = PathingSearchFlags::all()
                .iter()
                .filter_map(|search_flag| search_flag.as_str().map(|name| (search_flag, name)));
            for (i, (flag, search_flag_name)) in search_flags.enumerate() {
                if i % 3 != 2 {
                    ui.same_line();
                }
                search_dirty |= with_i18n!(search_flag_name, |name| ui.checkbox_flags(
                    name,
                    &mut self.search_state.flags,
                    flag
                ));
            }
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
