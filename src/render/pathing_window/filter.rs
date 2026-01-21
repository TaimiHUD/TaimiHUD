use {
    super::PathingWindowState,
    crate::{
        render::element::prelude::*,
        settings::state::ui::pathing::{PathingFilterFlags, PathingSearchFlags},
        render::{
            machine::RenderMachine,
            element::pack::CategoryFilter,
        },
        with_i18n,
    },
    regex::{Regex, RegexBuilder},
    std::collections::HashSet,
    taimi_meta::packs::{PackIndex, PackPath},
    taimi_pack::{category::CategoryFlags, Pack},
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
    pub fn clear(&mut self, candidate_mask: Option<&mut CategoryFilter>) {
        self.buffer.clear();
        self.matcher = None;
        self.clear_matches(candidate_mask);
    }
    pub fn clear_matches(&mut self, candidate_mask: Option<&mut CategoryFilter>) {
        #[cfg(todo = "unnecessary")]
        {
            self.search_candidates = Default::default();
        }
        if let Some(candidate_mask) = candidate_mask {
            candidate_mask.clear_search();
        }
    }
    pub fn clear_matches_active(&mut self, candidate_mask: Option<&mut CategoryFilter>) {
        #[cfg(todo = "unnecessary")]
        {
            self.search_candidates.clear();
        }
        if let Some(candidate_mask) = candidate_mask {
            candidate_mask.clear_search_active();
        }
    }

    pub fn commit<'p, P, I>(&mut self, candidate_mask: &mut CategoryFilter, packs: I)
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Pack>,
    {
        if self.buffer.is_empty() {
            self.clear_matches(Some(candidate_mask));
            return
        } else {
            self.clear_matches_active(Some(candidate_mask));
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

        for (i, pack) in packs.into_iter().enumerate() {
            let pack = pack.as_ref();
            let path: PackPath = PackPath::with_path(i as PackIndex);
            if let Some(mask) = candidate_mask.pack_mask.get_mut(&path) {
                mask.clear();
            }

            for (idx, (full_id, category)) in pack.categories.all_categories.iter().enumerate() {
                if self.flags.contains(PathingSearchFlags::IGNORE_ROOT) && category.flags.contains(CategoryFlags::ROOT) {
                    continue
                }
                if self.flags.contains(PathingSearchFlags::IGNORE_BRANCHES) && category.sub_categories.is_empty() {
                    continue
                }
                if self.flags.contains(PathingSearchFlags::IGNORE_LEAVES) && !category.sub_categories.is_empty() {
                    continue
                }
                if self.matches_name(category.display_name())
                    || self.matches_name(category.id().as_str())
                {
                    let mask = candidate_mask.pack_mask.entry(path).or_default();
                    if mask.as_bitslice().is_empty() {
                        mask.reserve_exact(pack.categories.all_categories.len());
                    }
                    #[cfg(todo = "unnecessary")]
                    {
                        self.search_candidates.insert(full_id.into());
                    }
                    if mask.insert_at(idx) { continue }
                    for sub_id in full_id.as_id().ancestors() {
                        #[cfg(todo = "unnecessary")]
                        {
                            self.search_candidates.insert(sub_id.into());
                        }
                        if let Some(parent_idx) = pack.categories.all_categories.get_index_of(sub_id) {
                            if mask.insert_at(parent_idx) {
                                // already been here
                                //break
                            }
                        }
                    }
                    if self.flags.contains(PathingSearchFlags::INCLUDE_CHILDREN) {
                        let mut children: Vec<_> = category.child_ids().collect();
                        while let Some(child_id) = children.pop() {
                            let Some((child_idx, _id, child)) = pack.categories.all_categories.get_full(child_id) else { continue };
                            if mask.insert_at(child_idx) { continue }
                            children.extend(child.child_ids());
                        }
                    }
                }
            }

            let mask = candidate_mask.pack_mask.entry(path).or_default();
            if mask.flags.is_empty() {
                mask.flags.push(false);
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

    #[cfg(todo = "unnecessary")]
    pub fn matches_id(&self, full_id: &str) -> bool {
        match self.buffer.is_empty() {
            false => self.search_candidates.contains(full_id),
            true => true,
        }
    }

    #[cfg(deleteme)]
    pub fn matches_category(&self, path: CategoryPath<PackPath>) -> bool {
        let cat_path: CategoryPath = path.unscope();
        self.candidate_mask
            .get(&path.root)
            .map(|mask| mask.contains(cat_path))
            .unwrap_or(false)
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
            self.search_state.clear(Some(&mut machine.pack_ui_state.pack_filters));
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
