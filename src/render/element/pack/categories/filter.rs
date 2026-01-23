use {
    crate::{
        controller::pathing::{
            registry::{PackCategoryInfo, LoadedCategoryNs, LoadedCategoryIndex},
            info::MapPackInfo,
            state::LoadedCategory,
            shared::SharedMapPackLoaded,
            PackConfig,
        },
        settings::state::ui::pathing::PathingFilterFlags,
    },
    std::{collections::BTreeMap, iter, sync::Arc},
    taimi_hoard::{
        flags::BitSet,
        loc::indexed::IndexedList,
    },
    taimi_meta::packs::{collections::CategorySet, CategoryPath, PackPath, VisibilityFlags, MapIndex, CategoryIndex},
    taimi_pack::{category::{Category, CategoryId}, Pack},
    futures::future::Either,
    regex::Regex,
};

pub type CategoryMaskState = BitSet;
pub type PackCategoryMasks = BTreeMap<PackPath, CategoryMaskState>;
#[derive(Debug, Clone, Default)]
pub struct CategoryFilter {
    #[cfg(todo)]
    pub pack_mask: PackCategoryMasks,
    pub flags: PathingFilterFlags,
    #[cfg(deleteme)]
    pub pack_filters: BTreeMap<PackPath, PackCategoryMask>,
    #[cfg(todo)]
    pub loaded: BTreeMap<PackMapPath, PackMapMask>,
}
#[derive(Debug, Clone, Default)]
pub struct CategoryFilterQuery {
    pub flags: PathingFilterFlags,
    pub search: Option<CategorySearchQuery>,
}
#[derive(Debug, Clone)]
pub struct CategorySearchQuery {
    pub matcher: Either<Regex, String>,
    #[cfg(todo)]
    pub flags: PathingSearchFlags,
}
impl CategorySearchQuery {
    pub fn with_matcher(matcher: Regex) -> Self {
        Self {
            matcher: Either::Left(matcher),
        }
    }
    pub fn with_fixed(query: String) -> Self {
        Self {
            matcher: Either::Right(query),
        }
    }
}
impl CategorySearchFilter for CategorySearchQuery {
    fn category_name_matches(&mut self, path: CategoryPath<PackPath>, id: &CategoryId, display_name: &Arc<str>) -> bool {
        CategorySearchFilter::category_name_matches(&mut &*self, path, id, display_name)
    }
}
impl CategorySearchFilter for &'_ CategorySearchQuery {
    fn category_name_matches(&mut self, path: CategoryPath<PackPath>, id: &CategoryId, display_name: &Arc<str>) -> bool {
        match &self.matcher {
            Either::Left(regex) => regex.is_match(&display_name),
            Either::Right(buffer) => display_name.contains(buffer),
        }
    }
}
#[cfg(todo)]
impl CategoryFilter {
    #[cfg(todo)]
    pub fn category_matches(&self, path: PackPath) -> Option<impl Iterator<Item = CategoryPath> + '_> {
        Self::category_matches_of(&self.pack_mask, path)
    }
    #[cfg(todo)]
    fn category_matches_of(pack_mask: &PackCategoryMask, path: PackPath) -> Option<impl Iterator<Item = CategoryPath> + '_> {
        if self.is_searching() { return None }
        let pack_filter = pack_mask.get(&path)?;
        Some(pack_filter.iter_of())
    }
    #[cfg(todo)]
    pub fn matches_category(&self, path: CategoryPath<PackPath>) -> bool {
        self.pack_mask.get(&path.root)
            .map(|mask| mask.contains(path.path))
            .unwrap_or(false)
    }
    #[cfg(todo)]
    pub fn generate_loaded(&mut self, map_path: PackMapPath, map_info: &MapPackInfo) -> Option<&mut PackMapMask> {
        let mask = Self::category_matches_of(&self.pack_mask, map_path.root)?;
        let loaded = self.loaded.entry(map_path)
            .or_default();

        loaded.category_search.clear();
        loaded.extend_search(map_info, &mut {mask});

        Some(loaded)
    }
    pub fn clear_search(&mut self) {
        self.pack_mask.clear();
        for pack in self.loaded.values_mut() {
            pack.clear_search();
        }
    }
    pub fn clear_search_active(&mut self) {
        for pack in self.pack_mask.values_mut() {
            pack.clear();
        }
        for pack in self.loaded.values_mut() {
            pack.clear_search_active();
        }
    }
}
#[derive(Debug, Clone, Default)]
pub struct PackCategoryMaskState {
    pub flags: PathingFilterFlags,
    pub search_candidates: Option<CategorySet>,
    pub loaded: Option<CategorySet>,
    pub loaded_sig: Option<MapIndex>,
    /// previously matched or were interacted with
    ///
    /// these may no longer match but should remain shown until
    /// a new query is made or state is wiped
    pub interest: CategorySet,
    pub enable: Option<CategoryEnableFilterState>,
    pub mask: PackCategoryMask,
}
impl PackCategoryMaskState {
    /// with roots
    pub fn reset_interest(&mut self, cats: Option<&PackCategoryInfo>) {
        self.interest = match cats {
            Some(cats) =>
                cats.root_paths().collect(),
            None => Default::default(),
        }
    }

    /// info sig dirty, mark most items to be regenerated
    pub fn info_invalidated(&mut self) {
        self.clear_loaded();
        self.clear_search_candidates();
        self.enable = None;
        self.interest.clear();
    }

    pub fn clear_search_candidates(&mut self) {
        self.search_candidates = None;
    }
    pub fn is_dirty_search(&self, query: Option<&CategorySearchQuery>) -> bool {
        self.search_candidates.is_some() != query.is_some()
    }
    pub fn update_search_candidates<F: CategorySearchFilter>(&mut self, pack_path: PackPath, category_info: Option<&PackCategoryInfo>, pack_data: Option<Option<&Pack>>, mut filter: F) {
        if self.search_candidates.is_some() != pack_data.is_some() & category_info.is_some() {
            self.reset_interest(category_info);
        }
        let search_candidates = self.search_candidates.get_or_insert_default();
        search_candidates.clear();
        let (Some(cats), Some(pack_data)) = (category_info, pack_data) else { return };
        let mut matches_pd;
        let mut matches_fallback;
        let matches = match &pack_data {
            Some(pack_data) => {
                matches_pd = pack_data.categories.all_categories.iter().enumerate().filter_map(|(i, (_id, cat))| {
                    let cat_path: CategoryPath = CategoryPath::with_path(i as CategoryIndex);
                    filter.pack_category_matches(pack_path.rel(cat_path.path), cat)
                        .then_some(cat_path)
                });
                &mut matches_pd as &mut dyn Iterator<Item = CategoryPath>
            },
            None => {
                matches_fallback = cats.all().paths()
                    .filter(|p| filter.category_path_matches(pack_path.rel(p.path)));
                &mut matches_fallback as &mut dyn Iterator<Item = CategoryPath>
            },
        };
        search_candidates.extend(matches);
    }

    pub fn clear_loaded(&mut self) {
        self.loaded = None;
        self.loaded_sig = None;
    }
    pub fn update_loaded(&mut self, category_info: Option<&PackCategoryInfo>, map_info: Option<&SharedMapPackLoaded>) {
        let loaded = self.loaded.get_or_insert_default();
        loaded.clear();
        self.loaded_sig = map_info.map(|info| info.path.path);
        let (Some(category_info), Some(map_info)) = (category_info, map_info) else { return };
        let paths = CategoryLoadedFilterInfo {
            category_info,
            loaded: &map_info.info,
        };
        loaded.extend(paths);
    }
    pub fn is_dirty_loaded(&self, map_info: Option<Option<&SharedMapPackLoaded>>) -> bool {
        if map_info.flatten().map(|info| info.path.path) != self.loaded_sig {
            return true
        }
        match (&self.loaded, map_info) {
            #[cfg(todo = "unnecessary")]
            (Some(loaded), Some(map_info)) => {
                if loaded.len() != map_info.map(|i| i.category_count()).unwrap_or(0) { return true }
            },
            (Some(..), None) | (None, Some(..)) => return true,
            _ => (),
        }
        false
    }
    pub fn clear_enable(&mut self) {
        self.enable = None;
    }
    pub fn is_dirty_enable(&self, enable: Option<bool>) -> bool {
        self.enable.as_ref().map(|e| e.query.state) != enable
    }
    pub fn update_enable(&mut self, config: Option<&PackConfig>, category_info: Option<&PackCategoryInfo>, map_info: Option<&SharedMapPackLoaded>, state: bool) {
        let map_info = map_info.map(|i| &*i.info);
        let enable = self.enable.get_or_insert_default();
        enable.query.state = state;
        let (Some(config), Some(category_info)) = (config, category_info) else {
            enable.clear();
            return
        };
        enable.refresh(config, category_info, map_info);
        #[cfg(todo)]
        let paths = CategoryEnableFilterInfo {
            state: enable,
            category_info,
            map_info,
        };
    }
    pub fn clear(&mut self) {
        self.clear_search_candidates();
        self.clear_loaded();
        self.clear_enable();
        self.mask.clear();
    }
    /// TODO
    pub fn clear_active(&mut self) {
        self.clear();
    }
    pub fn is_active(&self) -> bool {
        self.search_candidates.is_some() || self.flags.enable_filter().is_some() || self.loaded.is_some()
    }
    pub fn all_filtered(&self) -> bool {
        self.is_active() && self.mask.is_empty()
    }
}
#[derive(Debug, Clone, Default)]
pub struct PackCategoryMask {
    pub category_mask: BitSet,
}
impl PackCategoryMask {
    pub fn refill_with<I>(&mut self, info: &PackCategoryInfo, cats: I) where
        I: IntoIterator<Item = CategoryPath> + Clone,
    {
        self.category_mask.clear();
        self.category_mask.extend(cats.clone());
        self.fill_to_root(info, &mut cats.into_iter());
    }
    pub fn fill_to_root(&mut self, info: &PackCategoryInfo, cats: &mut dyn Iterator<Item = CategoryPath>) {
        for path in cats {
            for parent_path in info.ancestors_of(path) {
                if self.category_mask.insert_at(parent_path) {
                    // already been here or cats will emit it...
                    break
                }
            }
        }
    }
    #[cfg(deleteme)]
    pub fn fill_with(&mut self, info: &PackCategoryInfo, cats: &mut dyn Iterator<Item = CategoryPath>) {
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
    pub fn is_empty(&self) -> bool {
        self.category_mask.is_empty()
    }
    pub fn clear(&mut self) {
        self.category_mask.clear();
    }
}
#[cfg(todo)]
impl PackMaMask {
    /// TODO: this could be loaded indices instead which would be a more compact bitset, just enumerate or w/e...
    #[cfg(todo)]
    pub fn extend_search(&mut self, map_info: &MapPackInfo, pack_mask: &mut dyn Iterator<Item = CategoryPath>) {
        let mut cat_paths_loaded = map_info.categories().peekable();
        let loaded_mask = pack_mask
            .filter(|cat_path| {
                while let Some(..) = cat_paths_loaded.next_if(|p| *p < cat_path) {}
                cat_paths_loaded.peek() == Some(cat_path)
            });
        self.category_search.extend(loaded_mask);
        self.mark_searching();
    }
    pub fn is_searching(&self) -> bool {
        !self.category_search.flags.is_empty()
    }
    fn mark_searching(&mut self) {
        if self.category_search.is_empty() {
            self.category_search.flags.push(false);
        }
    }

    pub fn matches_category(&self, path: CategoryPath) -> bool {
        self.category_search.contains(path)
    }
    fn category_matches(&self) -> impl Iterator<Item = CategoryPath> + '_ {
        self.category_search.iter_of()
    }

    pub fn clear_search_active(&mut self) {
        self.category_search.clear();
    }
    pub fn clear_search(&mut self) {
        self.category_search = Default::default();
    }
}

/// all [loaded categories](MapPackInfo::categories)
/// with markers on-map (leaves only)
#[derive(Debug, Copy, Clone)]
pub struct CategoryLoadedFilterInfo<'a> {
    pub category_info: &'a PackCategoryInfo,
    pub loaded: &'a MapPackInfo,
}
impl<'a> IntoIterator for CategoryLoadedFilterInfo<'a> {
    type IntoIter = Box<dyn Iterator<Item = Self::Item> + 'a>;
    type Item = CategoryPath;
    fn into_iter(self) -> Self::IntoIter {
        let cats = self.category_info;
        let loaded = self.loaded.categories().filter(move |&path| match cats.info_of(path) {
            Some(cat) => cat.child().is_none() && !cats.lonely.contains(path),
            _ => false,
        });
        Box::new(loaded) as Box<_>
    }
}

#[derive(Debug, Clone, Default)]
pub struct CategoryEnableFilterQuery {
    pub state: bool,
}
#[derive(Debug, Clone, Default)]
pub struct CategoryEnableFilterState {
    pub query: CategoryEnableFilterQuery,
    pub configured: BitSet,
    pub effective: BitSet,
}
impl CategoryEnableFilterState {
    pub fn new(query: CategoryEnableFilterQuery, config: &PackConfig, cats: &PackCategoryInfo, info: Option<&MapPackInfo>) -> Self {
        let mut filter = Self {
            query,
            configured: Default::default(),
            effective: Default::default(),
        };
        filter.refresh(config, cats, info);
        filter
    }
    /// TODO: damage? we've thrown out the full flags though...
    pub fn refresh(&mut self, config: &PackConfig, cats: &PackCategoryInfo, info: Option<&MapPackInfo>) -> bool {
        // just pretend every one is loaded...
        let paths = cats.all().paths()
            .map(|p| (p.unscope(), p));
        let mut loaded = vec![LoadedCategory::INVALID; cats.count()];
        let loaded = IndexedList::<LoadedCategoryNs, LoadedCategoryIndex, _>::from_mut(&mut loaded[..]);
        let mut dirty = false;
        let damage: Option<Option<CategorySet>> = match LoadedCategory::populate_vis(&mut loaded.data, None, paths, cats, config) {
            #[cfg(todo)]
            false => None,
            #[cfg(todo)]
            true => {
                let damage = self.configured.iter()
                    .zip(loaded.iter())
                    .filter_map(|(vis, (path, loaded))| match *vis == loaded {
                        true => None,
                        false => Some(path),
                    }).collect::<CategorySet>();
                match damage.is_empty() {
                    true => None,
                    false => Some(Some(damage)),
                }
            },
            _ => Some(None),
        };
        self.configured.resize(loaded.end_path().path as usize, false);
        for (path, l) in loaded.iter() {
            let configured = l.visibility.contains(VisibilityFlags::DEFAULT_TOGGLE);
            dirty |= self.configured.insert_at_if(path, configured) != configured;
        }
        if let Some(damage) = damage {
            LoadedCategory::refresh_categories(loaded, info, cats, config, damage.as_mut());
            for (path, l) in loaded.iter() {
            let effective = l.visibility.is_visible();
                dirty |= self.effective.insert_at_if(path, effective) != effective;
            }
        }
        dirty
    }

    pub fn clear(&mut self) {
        self.configured.clear();
        self.effective.clear();
    }

    /// whether [Self::configured_categories] contains `path`
    #[cfg(todo)]
    pub fn is_category_configured(&self, path: CategoryPath) -> bool {
        self.configured.contains(path) == self.query.state
    }
    /// parent state not inherited
    #[cfg(todo)]
    pub fn configured_categories(&self) -> impl Iterator<Item = CategoryPath> + '_ {
        match self.query.state {
            true => Box::new(self.configured.iter_of::<CategoryPath>()) as Box<dyn Iterator<Item = CategoryPath>>,
            false => Box::new(
                self.configured.iter_zeros().lazy_map(|i| CategoryPath::with_path(i as CategoryIndex))
            ) as Box<dyn Iterator<Item = CategoryPath>>,
        }
    }
}
pub struct CategoryEnableFilterInfo<'a> {
    pub state: &'a CategoryEnableFilterState,
    pub category_info: &'a PackCategoryInfo,
    pub map_info: &'a MapPackInfo,
}
impl CategoryEnableFilterInfo<'_> {
    /// for any given category matching query, find the most shallow parent(s)
    /// for which all their children agree
    ///
    /// e.g. if a category is enabled *and* all of its child cats are too,
    /// they are deemed less interesting.
    /// (conversely, a category with some disabled children may want to be seen)
    pub fn open_interest(&self) -> BitSet {
        let mut interest = self.state.configured.clone();
        let query = self.state.query.state;
        if !query {
            let end = interest.end_len();
            interest.invert_to(..end);
        }
        let cats = self.category_info;
        for root in cats.root_paths() {
            let dfs = cats.descendents_of(root)
                .chain(iter::once(root));
            let mut parental_failures = Vec::new();
            let mut inconclusive = CategorySet::empty();
            for path in dfs {
                let parent_state = interest.contains(path);
                // if all children conform to query, they become uninteresting since
                // the parent serves as their representative!
                let mut children_conform = true;
                let mut rewind = Vec::new();
                for child_path in cats.children_of(path) {
                    match interest.remove_at(child_path) {
                        Some(true) => {
                            if parent_state {
                                rewind.push(child_path);
                            }
                        },
                        Some(false) => {
                            children_conform = false;
                            match parent_state {
                                false if inconclusive.contains(child_path) => {
                                    // previously inconclusive, so now any remaining children will be wrong
                                    parental_failures.push(child_path);
                                    #[cfg(todo = "unnecessary")]
                                    {
                                        inconclusive.remove(child_path);
                                    }
                                },
                                true => {
                                    // we should be okay to break out early here, remaining children don't matter anymore
                                    break
                                },
                                _ => (),
                            }
                        },
                        None => {
                            // ???
                            continue
                        },
                    }
                }
                if parent_state && !children_conform {
                    // only some are interesting, so put them back
                    interest.extend(rewind);
                    // the parent remains inconclusive
                    inconclusive.insert(path);
                }
                if !children_conform || !parent_state {
                    interest.remove_at(path);
                }
            }
            while let Some(path) = parental_failures.pop() {
                for child_path in cats.children_of(path) {
                    match interest.remove_at(child_path) {
                        Some(false) if inconclusive.contains(child_path) => {
                            parental_failures.push(child_path);
                        },
                        _ => (),
                    }
                }
            }
        }
        interest
    }
}
#[cfg(todo)]
impl<'a> IntoIterator for CategoryEnableFilterInfo<'a> {
    type IntoIter = Box<dyn Iterator<Item = Self::Item> + 'a>;
    type Item = CategoryPath;
    fn into_iter(self) -> Self::IntoIter {
        let cats = self.category_info;
        let configured = interest.iter_of::<CategoryPath>().collect::<CategorySet>();
        Box::new(configured.into_iter()) as Box<_>
    }
}

pub trait CategorySearchFilter {
    #[inline]
    fn pack_category_matches(&mut self, path: CategoryPath<PackPath>, cat: &Category) -> bool {
        if self.category_path_matches(path) { return true }
        self.category_name_matches(path, &cat.full_id, &cat.display_name)
    }
    #[inline]
    fn category_path_matches(&mut self, path: CategoryPath<PackPath>) -> bool { false }
    #[inline]
    fn category_name_matches(&mut self, path: CategoryPath<PackPath>, id: &CategoryId, display_name: &Arc<str>) -> bool { false }
}
impl<F: CategorySearchFilter> CategorySearchFilter for &'_ mut F {
    #[inline]
    fn pack_category_matches(&mut self, path: CategoryPath<PackPath>, cat: &Category) -> bool {
        CategorySearchFilter::pack_category_matches(*self, path, cat)
    }
    #[inline]
    fn category_path_matches(&mut self, path: CategoryPath<PackPath>) -> bool {
        CategorySearchFilter::category_path_matches(*self, path)
    }
    #[inline]
    fn category_name_matches(&mut self, path: CategoryPath<PackPath>, id: &CategoryId, display_name: &Arc<str>) -> bool {
        CategorySearchFilter::category_name_matches(*self, path, id, display_name)
    }
}
