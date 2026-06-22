use {
    super::PathingWindowState,
    crate::{
        render::{
            element::{pack::CategorySearchQuery, prelude::*},
            machine::RenderMachine,
        },
        settings::state::ui::pathing::{PathingFilterFlags, PathingSearchFlags},
        with_i18n,
    },
    regex::{Regex, RegexBuilder},
    std::{borrow::Cow, cmp},
    taimi_hoard::str_opt,
};

#[derive(Clone)]
pub struct PathingSearchState {
    pub buffer: String,
    matcher: Option<Regex>,
    pub flags: PathingSearchFlags,
}

impl PathingSearchState {
    pub fn clear(&mut self) {
        self.buffer = Default::default();
        self.matcher = None;
    }
    pub fn clear_active(&mut self) {
        self.buffer.clear();
        self.matcher = None;
    }
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    pub fn query_str(&self) -> Option<Option<&String>> {
        match str_opt(&self.buffer) {
            Some(..)
                if self.flags.contains(PathingSearchFlags::PATTERN_REGEX) && self.matcher.is_none() =>
                None,
            query => Some(query),
        }
    }

    pub fn commit(&mut self, partial: bool) -> bool {
        if self.buffer.is_empty() {
            self.matcher = None;
            return true
        }
        self.matcher = {
            let escape = !self.flags.contains(PathingSearchFlags::PATTERN_REGEX);
            let pattern = match escape {
                true => Cow::Owned(regex::escape(&self.buffer)),
                false => Cow::Borrowed(&self.buffer[..]),
            };
            let matcher = RegexBuilder::new(&pattern)
                .case_insensitive(self.flags.contains(PathingSearchFlags::IGNORE_CASE))
                .ignore_whitespace(self.flags.contains(PathingSearchFlags::IGNORE_SPACE))
                .build();
            match &matcher {
                Err(e) if partial && !escape =>
                // regex pattern may be incomplete, so leave prior matcher there for now
                    return false,
                Err(e) => log::warn!("search filter failure: {e:#}"),
                _ => (),
            }
            matcher.ok()
        };
        match &self.matcher {
            #[cfg(todo = "unnecessary")]
            matcher => matcher.is_some(),
            _ => true,
        }
    }
    pub fn to_query(&self) -> Option<CategorySearchQuery> {
        let mut query = match &self.matcher {
            Some(matcher) => Some(CategorySearchQuery::with_matcher(matcher.clone())),
            None if self.buffer.is_empty() => None,
            None if self.flags.contains(PathingSearchFlags::PATTERN_REGEX) =>
                return Some(CategorySearchQuery::negative()),
            None => Some(CategorySearchQuery::with_fixed(self.buffer.clone())),
        };
        if let Some(query) = &mut query {
            query.flags = self.flags;
        }
        query
    }
}

impl Default for PathingSearchState {
    fn default() -> Self {
        Self {
            buffer: Default::default(),
            matcher: Default::default(),
            flags: PathingSearchFlags::DEFAULT,
        }
    }
}

impl PathingWindowState {
    pub fn draw_filters<'ui, U>(&mut self, ui: &mut U, machine: &mut RenderMachine) -> Option<bool>
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
        let search_focus = ui.item_is_focused();
        let search_commit = search_focus && ui.with_io_dyn(|io| io.key_is_pressed_alphanum(b'\n'));
        if !self.search_state.buffer.is_empty() {
            ui.same_line();
            if ui.button("X") {
                self.search_state.clear();
                self.search_show_options = false;
                self.search_focus_latch = false;
                machine.pack_ui_state.clear_search_filter();
                self.ui_state.write_if(|s| {
                    s.search.query.clear();
                    None
                });
            } else if ui.is_item_hovered() {
                with_i18n!("pathing-search-clear", |msg| ui.tooltip_text(msg));
            }
        }
        if search_focus && ui.with_io_dyn(|io| io.want_text_input()) {
            self.search_focus_latch = true;
        }
        if !self.search_state.buffer.is_empty() || self.search_focus_latch {
            let options = {
                ui.same_line();
                let options_changed = with_i18n!("options", |label| ui
                    .checkbox(&label, &mut self.search_show_options));
                if options_changed && !self.search_show_options {
                    self.search_focus_latch = false;
                }
                self.search_show_options
            };
            let advanced = options;
            let search_flags = match options {
                false => PathingSearchFlags::empty(),
                true =>
                    PathingSearchFlags::USER
                        | advanced
                            .then_some(PathingSearchFlags::ADVANCED)
                            .unwrap_or(PathingSearchFlags::empty()),
            };
            let search_flags = search_flags
                .iter()
                .filter_map(|search_flag| search_flag.as_str().map(|name| (search_flag, name)));
            for (i, (flag, search_flag_name)) in search_flags.enumerate() {
                if i % 3 != 0 {
                    ui.same_line();
                }
                search_dirty |= with_i18n!(search_flag_name, |name| ui.checkbox_flags(
                    name,
                    &mut self.search_state.flags,
                    flag
                ));
            }
        }
        if search_commit {
            self.search_show_options = false;
            self.search_focus_latch = false;
        }
        pushy.end();
        with_i18n!("filter-options", |msg| ui.text(msg));
        {
            let _id = ui.push_id("filter-enable");
            let enable_id = |enable| match enable {
                None => "all",
                Some(true) => "enabled",
                Some(false) => "disabled",
            };
            let choices = [None, Some(true), Some(false)];
            let max_width = choices
                .iter()
                .map(|c| ui.calc_text_size(enable_id(*c))[0])
                .max_by(|a, b| a.partial_cmp(b).unwrap_or(cmp::Ordering::Less));
            let enable = self.ui_state.filter.flags.enable_filter();
            let enable_combo = {
                let preview = enable_id(enable);
                ui.same_line();
                ui.dummy([1.0, 1.0]);
                ui.same_line();
                if let Some(w) = max_width {
                    ui.item_prepare_width(w * 1.5);
                }
                with_i18n!(preview, |preview| ui.begin_combo("", &preview))
            };
            if let Some(_token) = enable_combo {
                for choice in choices {
                    let selected = choice == enable;
                    if with_i18n!(enable_id(choice), |label| ui.selectable(label, selected)) {
                        self.ui_state.filter.flags.set_enable_filter(choice);
                    }
                }
            } else if ui.is_item_right_clicked() {
                self.ui_state.filter.flags.set_enable_filter(None);
            }
        }
        if ui.cursor_pos()[0] < ui.content_region_max()[0] * 0.65 {
            ui.same_line();
            ui.dummy([1.0, 1.0]);
            ui.same_line();
        }
        let filters = (PathingFilterFlags::USER & !PathingFilterFlags::FILTERS_ENABLE)
            .iter()
            .filter_map(|filter| filter.as_str().map(|name| (filter, name)));
        for (i, (flag, filter_name)) in filters.enumerate() {
            if i > 0 && i % 3 != 0 {
                ui.same_line();
            }
            with_i18n!(filter_name, |name| ui.checkbox_flags(
                name,
                &mut self.ui_state.filter.flags,
                flag
            ));
        }

        match search_commit {
            true => Some(true),
            false => search_dirty.then_some(false),
        }
    }
}
