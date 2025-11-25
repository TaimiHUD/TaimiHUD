use {
    super::PathingWindowState,
    crate::{
        controller::pathing::registry::PackPath,
        exports::runtime::imgui::Ui,
        with_i18n,
    }, bitflags::bitflags, bitvec::vec::BitVec, regex::{Regex, RegexBuilder}, std::{collections::{BTreeMap, HashSet}, str::FromStr}, taimi_pack::Pack,
};

bitflags! {
    #[derive(PartialEq, Copy, Clone)]
    pub struct PathingFilterState: u8 {
        const Enabled = 1;
        const Disabled = 1 << 1;
        const CurrentMap = 1 << 2;
        const IgnoreRoot = 1 << 3;
        const IgnoreLeaves = 1 << 4;
        const IgnoreBranches = 1 << 5;
        const ShowHidden = 1 << 6;
    }
}

impl Default for PathingFilterState {
    fn default() -> Self {
        Self::Enabled | Self::Disabled | Self::IgnoreRoot
    }
}

impl FromStr for PathingFilterState {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "enabled" => Self::Enabled,
            "disabled" => Self::Disabled,
            "ignore-root" => Self::IgnoreRoot,
            "ignore-leaf" => Self::IgnoreLeaves,
            "ignore-branch" => Self::IgnoreBranches,
            "show-hidden" => Self::ShowHidden,
            "current-map" => Self::CurrentMap,
            _ => anyhow::bail!("unsupported filter option `{s}`"),
        })
    }
}

impl PathingFilterState {
    pub fn bit_as_str(self) -> Option<&'static str> {
        Some(match self.into_iter().next()? {
            Self::Enabled => "enabled",
            Self::Disabled => "disabled",
            Self::CurrentMap => "current-map",
            Self::IgnoreRoot => "ignore-root",
            Self::IgnoreLeaves => "ignore-leaf",
            Self::IgnoreBranches => "ignore-branch",
            Self::ShowHidden => "show-hidden",
            _ => return None,
        })
    }
}

#[derive(Clone)]
pub struct PathingSearchState {
    pub buffer: String,
    matcher: Option<Regex>,
    search_candidates: HashSet<String>,
    pub candidate_mask: BTreeMap<PackPath, BitVec>,
    pub ignore_space: bool,
    pub ignore_case: bool,
}

impl PathingSearchState {
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.matcher = None;
        self.search_candidates.clear();
        self.candidate_mask.clear();
    }

    pub fn commit<'p, P: Iterator<Item = (PackPath, &'p Pack)>>(&mut self, packs: P) {
        self.search_candidates.clear();
        if self.buffer.is_empty() {
            return
        }
        self.matcher = {
            let pattern = regex::escape(&self.buffer);
            let matcher = RegexBuilder::new(&pattern)
                .case_insensitive(self.ignore_case)
                .ignore_whitespace(self.ignore_space)
                .build();
            if let Err(e) = &matcher {
                log::warn!("search filter failure: {e:#}");
            }
            matcher.ok()
        };

        for (path, pack) in packs {
            {
                let mask = self.candidate_mask.entry(path).or_default();
                mask.clear();
                mask.resize(pack.categories.all_categories.len(), false);
            }

            for (idx, (full_id, category)) in pack.categories.all_categories.iter().enumerate() {
                if self.matches_name(&category.display_name) || self.matches_name(category.id().as_str()) {
                    let mask = self.candidate_mask.entry(path).or_default();
                    self.search_candidates.insert(full_id.into());
                    mask.set(idx, true);
                    for sub_id in full_id.as_id().ancestors() {
                        self.search_candidates.insert(sub_id.into());
                        if let Some(parent_idx) = pack.categories.all_categories.get_index_of(sub_id) {
                            mask.set(parent_idx, true);
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
            candidate_mask: Default::default(),
            ignore_case: true,
            ignore_space: true,
        }
    }
}

impl PathingWindowState {
    pub fn draw_filters(&mut self, ui: &Ui) -> bool {
        let pushy = ui.push_id("pathing-search");
        let mut search_dirty = ui
            .input_text("", &mut self.search_state.buffer)
            .hint("Search")
            .build();
        ui.same_line();
        if ui.button("X") {
            self.search_state.clear();
        }
        if ui.is_item_hovered() {
            with_i18n!("searchbar-clear", |msg| ui.tooltip_text(msg));
        }
        ui.same_line();
        search_dirty |= with_i18n!("case-insensitive", |label|
            ui.checkbox(&label, &mut self.search_state.ignore_case)
        );
        ui.same_line();
        search_dirty |= with_i18n!("ignore-whitespace", |label|
            ui.checkbox(&label, &mut self.search_state.ignore_space)
        );
        pushy.pop();
        ui.dummy([4.0; 2]);
        with_i18n!("filter-options", |msg| ui.text(msg));
        let filters = PathingFilterState::all()
            .iter()
            .filter_map(|filter| filter.bit_as_str().map(|name| (filter, name)));
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
