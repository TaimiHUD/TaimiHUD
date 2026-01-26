use {
    crate::{
        controller::pathing::{
            registry::{PackCategoryInfo, LoadedCategoryNs, LoadedCategoryIndex, LoadedCategoryPath},
            info::MapPackInfo,
            state::LoadedCategory,
            shared::SharedMapPackLoaded,
            PackConfig,
        },
        settings::state::ui::pathing::{PathingFilterFlags, PathingSearchFlags},
    },
    std::{collections::BTreeMap, iter, mem, sync::Arc},
    taimi_hoard::{
        iters::IterExt as _,
        iters::tree::{TreeTraversal, PeekableDfsPre},
        flags::BitSet,
        loc::{indexed::IndexedList, LocationGet},
    },
    taimi_meta::packs::{collections::CategorySet, CategoryPath, PackPath, VisibilityFlags, MapIndex, CategoryIndex, PackCategoryNs},
    taimi_pack::{category::{Category, CategoryId}, Pack},
    taimi_sync::arcs::ArcPtrCmp,
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
impl CategoryFilterQuery {
    pub fn set_flags(&mut self, mut flags: PathingFilterFlags) {
        flags.canonicalize_enable_filter();
        self.flags = flags;
    }

    pub fn is_empty(&self) -> bool {
        !self.is_matching() && !self.has_enable()
    }
    pub fn is_matching(&self) -> bool {
        self.search.is_some() | self.flags.contains(PathingFilterFlags::CurrentMap)
    }
    fn has_enable(&self) -> bool {
        self.flags.intersects(PathingFilterFlags::FILTERS_ENABLE)
    }
}
#[derive(Debug, Clone)]
pub struct CategorySearchQuery {
    pub matcher: Either<Regex, String>,
    pub flags: PathingSearchFlags,
}
impl CategorySearchQuery {
    pub fn with_matcher(matcher: Regex) -> Self {
        Self {
            matcher: Either::Left(matcher),
            flags: PathingSearchFlags::DEFAULT,
        }
    }
    pub fn with_fixed(query: String) -> Self {
        Self {
            matcher: Either::Right(query),
            flags: PathingSearchFlags::DEFAULT,
        }
    }
    /// matches nothing
    pub fn negative() -> Self {
        Self {
            matcher: Either::Right(String::new()),
            flags: PathingSearchFlags::NEGATIVE,
        }
    }
    #[cfg(todo = "unused")]
    pub fn everything() -> Self {
        Self {
            matcher: Either::Right(String::new()),
            flags: PathingSearchFlags::empty(),
        }
    }

    pub fn matcher_matches(&self, s: &str) -> bool {
        match &self.matcher {
            Either::Left(regex) => regex.is_match(s),
            #[cfg(todo = "unnecessary")]
            Either::Right(buffer) if buffer.is_empty() => true,
            Either::Right(buffer) => s.contains(buffer),
        }
    }
    #[cfg(todo = "unused")]
    pub fn query_matches(&self, s: &str) -> bool {
        self.matcher_matches(s) ^ self.flags.contains(PathingSearchFlags::NEGATIVE)
    }
}
impl CategorySearchFilter for CategorySearchQuery {
    fn category_name_matches(&mut self, path: CategoryPath<PackPath>, id: &CategoryId, display_name: Option<&Arc<str>>) -> bool {
        CategorySearchFilter::category_name_matches(&mut &*self, path, id, display_name)
    }
}
impl CategorySearchFilter for &'_ CategorySearchQuery {
    fn category_name_matches(&mut self, _path: CategoryPath<PackPath>, id: &CategoryId, display_name: Option<&Arc<str>>) -> bool {
        let matches_name = display_name.as_ref().map(|name| self.matcher_matches(&name[..])).unwrap_or(false);
        let matches_id = self.flags.contains(PathingSearchFlags::INCLUDE_ID).then(|| self.matcher_matches(id.as_str())).unwrap_or(false);
        let matches = matches_name || matches_id;
        matches ^ self.flags.contains(PathingSearchFlags::NEGATIVE)
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
    pub category_info: Option<Arc<PackCategoryInfo>>,
    #[cfg(todo = "unnecessary")]
    pub hidden: CategorySet,
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
    pub mask_interest: PackCategoryMask,
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
    pub fn populate_interest(&mut self, cats: Option<&PackCategoryInfo>) {
        let cats = cats.or(self.category_info.as_ref().map(|cats| &**cats));
        if let Some(cats) = cats {
            self.interest.extend(cats.root_paths());
        }
    }
    pub fn extend_interest<I: IntoIterator<Item = CategoryPath>>(&mut self, paths: I) {
        self.interest.extend(paths);
    }

    /// info sig dirty, mark most items to be regenerated
    pub fn info_invalidated(&mut self) {
        self.clear_loaded();
        self.clear_search_candidates();
        self.enable = None;
        self.interest.clear();
    }
    pub fn set_flags(&mut self, mut flags: PathingFilterFlags) {
        flags.canonicalize_enable_filter();
        self.flags = flags;
    }

    pub fn clear_search_candidates(&mut self) {
        if self.search_candidates.is_some() {
            self.interest.clear();
        }
        self.search_candidates = None;
    }
    pub fn is_dirty_search(&self, query: Option<&CategorySearchQuery>) -> bool {
        self.search_candidates.is_some() != query.is_some()
    }
    pub fn update_search_candidates<F: CategorySearchFilter>(&mut self, pack_path: PackPath, category_info: Option<&PackCategoryInfo>, pack_data: Option<Option<&Pack>>, mut filter: F) {
        self.interest.clear();
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
    pub fn update_enable(&mut self, config: Option<&PackConfig>, category_info: Option<&PackCategoryInfo>, state: bool) {
        let enable = self.enable.get_or_insert_default();
        enable.query.state = state;
        let (Some(config), Some(category_info)) = (config, category_info) else {
            enable.clear();
            return
        };
        enable.refresh(config, category_info, None);
        #[cfg(todo)]
        let paths = CategoryEnableFilterInfo {
            state: enable,
            category_info,
            map_info,
        };
    }
    pub fn is_dirty_hidden(&self, category_info: Option<&Arc<PackCategoryInfo>>) -> bool {
        let prev = self.category_info.as_ref().map(|i| Arc::as_ptr(i) as *const _  as usize);
        let ptr = category_info.map(|i| Arc::as_ptr(i) as *const _  as usize);
        match (prev, ptr) {
            #[cfg(todo = "unnecessary")]
            (None, _) if self.flags.contains(PathingFilterFlags::ShowHidden) =>
                false,
            (prev, ptr) => prev != ptr,
        }
    }
    pub fn clear_hidden(&mut self) {
        #[cfg(todo = "unnecessary")]
        {
            //self.category_info = None;
            self.hidden = Default::default();
        }
    }
    pub fn update_hidden(&mut self, category_info: Option<&Arc<PackCategoryInfo>>) {
        match category_info {
            None if self.flags.contains(PathingFilterFlags::ShowHidden) =>
                self.clear_hidden(),
            _ => (),
        }
        self.category_info = category_info.cloned();
    }
    fn is_path_hidden(&self, path: CategoryPath) -> bool {
        if let Some(true) = self.category_info.as_ref().map(|i| i.hidden.contains(path)) {
            return true
        }

        #[cfg(todo = "unnecessary")]
        if self.hidden.contains(path) {
            return true
        }

        false
    }
    fn hidden_paths(category_info: Option<&PackCategoryInfo>) -> impl DoubleEndedIterator<Item = CategoryPath> + '_ {
        let hidden = category_info.as_ref().map(|i| i.hidden()).into_iter().flatten();
        #[cfg(todo = "unnecessary")]
        let hidden = hidden.chain(self.hidden.paths());
        hidden
    }

    pub fn clear_mask(&mut self) {
        self.mask.clear();
        self.mask_interest.clear();
    }
    pub fn refresh_mask(&mut self, category_info: Option<&PackCategoryInfo>) {
        self.mask_interest.clear();
        match self.is_active() {
            false => {
                self.mask.clear();
                return
            },
            true => {
                self.mask.prepare();
            }
        }
        let Some(cats) = category_info else {
            self.mask.finalize();
            return
        };
        let enable = self.enable.as_ref().map(|state| CategoryEnableFilterInfo {
            state,
        });
        if let Some(enable) = &enable {
            let as_filter = self.search_candidates.is_some();
            let as_filter_pre = as_filter | self.loaded.is_some();
            let paths = match as_filter_pre {
                true => enable.iter_for_filter(),
                false => enable.iter_matching(),
            };
            self.mask.init_mask_with(paths);
            if as_filter {
                self.mask.fill_to_root_with(cats, &mut enable.iter_for_filter());
            }
        }
        if let Some(loaded) = &self.loaded {
            // TODO: use full loaded set/arc if as_filter
            let as_filter = self.search_candidates.is_some();
            self.mask.prepare_mask_and(loaded);
            if as_filter {
                self.mask.fill_to_root_with(cats, &mut loaded.paths());
            }
        }
        if let Some(search) = &self.search_candidates {
            self.mask.prepare_mask_and(search);
        }
        if !self.flags.contains(PathingFilterFlags::ShowHidden) {
            let category_info = category_info.or(self.category_info.as_ref().map(|cats| &**cats));
            self.mask.prepare_mask_without(&mut Self::hidden_paths(category_info));
        }
        for root in cats.root_paths() {
            let mut nodes = cats.nested_descendents_of(root);
            while let Some(path) = nodes.next_node() {
                if self.mask.category_mask.contains(path) {
                    nodes.skip_to_sibling();
                    let Some(parent_path) = cats.parent_of(path) else { continue };
                    if self.mask_interest.category_mask.insert_at(parent_path) {
                        continue
                    }
                    self.mask_interest.fill_to_root_with(cats, &mut iter::once(parent_path));
                }
            }
        }
        if let Some(search) = &self.search_candidates {
            self.mask.fill_to_root_with(cats, &mut search.paths());
        } else if let Some(loaded) = &self.loaded {
            self.mask.fill_to_root_with(cats, &mut loaded.paths());
        } else if let Some(enable) = &enable {
            self.mask.fill_to_root_with(cats, &mut enable.iter_matching());
        }

        self.mask.finalize();
    }
    pub fn clear(&mut self) {
        self.clear_search_candidates();
        self.clear_loaded();
        self.clear_enable();
        self.clear_mask();
    }
    /// TODO
    pub fn clear_active(&mut self) {
        self.clear();
    }
    fn has_enable(&self) -> bool {
        self.flags.intersects(PathingFilterFlags::FILTERS_ENABLE)
    }
    pub fn is_active(&self) -> bool {
        self.search_candidates.is_some() || self.has_enable() || self.loaded.is_some()
    }
    pub fn any_visible(&self) -> bool {
        if !self.interest.is_empty() { return true }
        !self.all_filtered()
    }
    pub fn all_filtered(&self) -> bool {
        if self.flags.contains(PathingFilterFlags::CurrentMap) && self.loaded.as_ref().map(|l| l.is_empty()).unwrap_or(true) {
            return true
        }
        if self.search_candidates.as_ref().map(|s| s.is_empty()).unwrap_or(false) {
            return true
        }
        if self.enable.as_ref().map(|e| e.is_empty()).unwrap_or(false) {
            return true
        }
        !self.mask.has_any()
    }

    /// TODO: use a real iter adapter for the lockstep filter
    fn hidden_filter<'a, I: IntoIterator<Item = CategoryPath> + 'a>(&'a self, paths: I) -> impl Iterator<Item = CategoryPath> + 'a {
        let mut hidden = (!self.flags.contains(PathingFilterFlags::ShowHidden)).then_some(
            self.category_info.as_ref().map(|i| i.hidden().peekable())
        );
        paths.into_iter().filter(move |path| {
            match &mut hidden {
                Some(Some(hidden)) => {
                    while let Some(..) = hidden.next_if(|p| p < path) {}
                    hidden.peek() != Some(path)
                }
                _ => true,
            }
        })
    }
    pub fn iter_categories<'a>(&'a self, category_info: Option<&'a PackCategoryInfo>) -> impl Iterator<Item = CategoryPath> + 'a {
        let unfiltered = self.mask.has_all();
        let all = match (unfiltered, category_info) {
            (true, Some(cats)) => Some(self.hidden_filter(cats.all().paths())),
            _ => None,
        };
        let mask = (!unfiltered).then(|| {
            let interest = self.interest.paths().filter(|&path| !self.mask.category_mask.contains(path));
            self.mask.iter_categories().chain(interest)
        });
        all.into_iter().flatten().chain(mask.into_iter().flatten())
    }
    pub fn contains_category(&self, path: CategoryPath) -> bool {
        let contained = self.mask.category_mask.contains(path) ^ self.mask.invert;
        match contained {
            true if self.mask.category_mask.is_empty() && !self.flags.contains(PathingFilterFlags::ShowHidden) =>
                !self.is_path_hidden(path),
            c => c,
        }
    }
    pub fn visible_category(&self, path: CategoryPath) -> bool {
        match self.contains_category(path) {
            false if self.interest.contains(path) => true,
            c => c,
        }
    }
    pub fn is_matching(&self) -> bool {
        self.search_candidates.is_some() | self.enable.is_some()
    }
    pub fn matches_category(&self, path: CategoryPath) -> bool {
        let mut matching = true;
        if let Some(enable) = &self.enable {
            matching &= enable.matches_category(path);
        }
        if let Some(Some(search)) = matching.then_some(&self.search_candidates) {
            matching &= search.contains(path);
        }
        matching
    }
}
#[derive(Debug, Clone)]
pub struct PackCategoryMask {
    pub category_mask: BitSet,
    pub invert: bool,
}
impl PackCategoryMask {
    pub fn everything() -> Self {
        Self {
            category_mask: BitSet::empty(),
            invert: true,
        }
    }
    /// deleteme?
    #[cfg(todo)]
    pub fn refill_with<I>(&mut self, info: Option<&PackCategoryInfo>, cats: I) where
        I: IntoIterator<Item = CategoryPath> + Clone,
    {
        self.category_mask.clear();
        self.category_mask.extend(cats.clone());
        if let Some(info) = info {
            self.fill_to_root(info, &mut cats.into_iter());
        }
    }
    pub fn prepare(&mut self) {
        self.category_mask.clear();
        self.invert = true;
    }
    pub fn finalize(&mut self) {
        self.category_mask.truncate_to_fit();
        self.invert = false;
    }
    pub fn prepare_mask_and(&mut self, paths: &CategorySet) {
        if mem::replace(&mut self.invert, false) {
            self.category_mask.extend_sorted(paths.paths());
            return
        }
        for (path, mut mask) in self.category_mask.enum_paths_mut::<PackCategoryNs, CategoryIndex>() {
            if *mask && !paths.contains(path) {
                *mask = false;
            }
        }
    }
    pub fn prepare_mask_without(&mut self, paths: &mut dyn Iterator<Item = CategoryPath>) {
        if self.category_mask.is_empty() { return }
        for path in paths {
            self.category_mask.remove_at(path);
        }
    }
    pub fn init_mask_with<I>(&mut self, paths: I) where
        I: IntoIterator<Item = CategoryPath>,
    {
        match self.invert {
            true if self.category_mask.is_empty() => self.invert = false,
            false if self.category_mask.is_empty() => return,
            _ => {
                log::info!("BUG: filter mask already init?");
                return
            },
        }
        self.category_mask.extend(paths);
    }
    #[cfg(todo)]
    pub fn prepare_mask_sorted<I>(&mut self, paths: I) {
        for (path, mask) in self.category_mask.enum_paths_mut::<CategoryPath>() {
            if !*mask { continue }
            etc
        }
    }
    #[cfg(todo = "unused")]
    pub fn fill_to_root(&mut self, info: &PackCategoryInfo) {
        let mask = self.category_mask.clone();
        self.fill_to_root_with(info, &mut mask.iter_of())
    }
    pub fn fill_to_root_with(&mut self, info: &PackCategoryInfo, paths: &mut dyn Iterator<Item = CategoryPath>) {
        for path in paths {
            let mut filling = self.category_mask.contains(path);
            for parent_path in info.ancestors_of(path) {
                match filling {
                    true if self.category_mask.insert_at(parent_path) => {
                        // already been here or iter will emit it...
                        break
                    },
                    false if self.category_mask.contains(parent_path) =>
                        filling = true,
                    _ => (),
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
    #[cfg(todo)]
    pub fn is_empty(&self) -> bool {
        self.category_mask.is_empty() && !self.invert
    }
    pub fn has_any(&self) -> bool {
        match self.invert {
            #[cfg(todo = "unnecessary")]
            true => self.category_mask.flags.is_empty() || !self.category_mask.is_full(),
            true => true,
            false => !self.category_mask.is_empty(),
        }
    }
    pub fn has_all(&self) -> bool {
        let blacklist_empty = match &self.category_mask {
            #[cfg(todo = "unnecessary")]
            mask => mask.is_empty(),
            _ => true,
        };
        // TODO: && !self.is_active()?
        self.invert && blacklist_empty
    }
    pub fn clear(&mut self) {
        self.category_mask.clear();
        self.invert = true;
    }
    pub fn iter_categories(&self) -> impl DoubleEndedIterator<Item = CategoryPath> + '_ {
        self.category_mask.iter_of()
    }
}
impl Default for PackCategoryMask {
    fn default() -> Self {
        Self::everything()
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
    #[cfg(deleteme)]
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
        let loaded = IndexedList::<LoadedCategoryNs, LoadedCategoryIndex, _>::from_mut(&mut loaded);
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
        //self.effective.resize(loaded.end_path().path as usize, false);
        if let Some(damage) = damage {
            let mut loaded = loaded.map_mut_as_slice();
            struct FakePackMap<'a>(Option<&'a MapPackInfo>);
            impl LocationGet<PackCategoryNs, CategoryIndex> for FakePackMap<'_> {
                type LookupGet = LoadedCategoryPath;
                fn lookup_get(&self, &path: &CategoryPath) -> Option<LoadedCategoryPath> {
                    match self.0 {
                        Some(map_info) if map_info.category_index(path).is_none() => None,
                        _ => Some(path.unscope()),
                    }
                }
            }
            let info = FakePackMap(info);
            LoadedCategory::refresh_categories(&mut loaded, &info, cats, config, damage.as_ref());
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
    pub fn is_empty(&self) -> bool {
        match self.query.state {
            true => self.configured.is_empty(),
            false => self.configured.is_full(),
        }
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

    pub fn matches_category(&self, path: CategoryPath) -> bool {
        self.effective.contains(path) == self.query.state
    }
}
#[derive(Debug, Copy, Clone)]
pub struct CategoryEnableFilterInfo<'a> {
    pub state: &'a CategoryEnableFilterState,
}
impl CategoryEnableFilterInfo<'_> {
    /// for any given category matching query, find the most shallow parent(s)
    /// for which all their children agree
    ///
    /// e.g. if a category is enabled *and* all of its child cats are too,
    /// they are deemed less interesting.
    /// (conversely, a category with some disabled children may want to be seen)
    pub fn open_interest(&self, category_info: &PackCategoryInfo) -> BitSet {
        let mut interest = self.state.configured.clone();
        let query = self.state.query.state;
        if !query {
            let end = interest.end_len();
            interest.invert_to(..end);
        }
        for root in category_info.root_paths() {
            let dfs = category_info.descendents_of(root)
                .chain(iter::once(root));
            let mut parental_failures = Vec::new();
            let mut inconclusive = CategorySet::empty();
            for path in dfs {
                let parent_state = interest.contains(path);
                // if all children conform to query, they become uninteresting since
                // the parent serves as their representative!
                let mut children_conform = true;
                let mut rewind = Vec::new();
                for child_path in category_info.children_of(path) {
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
                for child_path in category_info.children_of(path) {
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

    pub fn iter_enabled(&self) -> impl Iterator<Item = CategoryPath> + Clone + '_ {
        self.state.effective.iter_of::<CategoryPath>()
    }
    pub fn iter_disabled(&self) -> impl Iterator<Item = CategoryPath> + Clone + '_ {
        self.state.effective.flags.iter_zeros()
            .lazy_map(|i| CategoryPath::with_path(i as CategoryIndex))
    }
    pub fn iter_disabled_conservative(&self) -> impl Iterator<Item = CategoryPath> + Clone + '_ {
        let configured = &self.state.configured;
        self.iter_disabled().filter(|&path| !configured.contains(path))
    }
    pub fn iter_matching(&self) -> Box<dyn Iterator<Item = CategoryPath> + '_> {
        match self.state.query.state {
            true => Box::new(self.iter_enabled()) as Box<dyn Iterator<Item = CategoryPath>>,
            false => Box::new(self.iter_disabled_conservative()) as Box<dyn Iterator<Item = CategoryPath>>,
        }
    }
    pub fn iter_for_filter(&self) -> Box<dyn Iterator<Item = CategoryPath> + '_> {
        match self.state.query.state {
            true => Box::new(self.iter_enabled()) as Box<dyn Iterator<Item = CategoryPath>>,
            false => Box::new(self.iter_disabled()) as Box<dyn Iterator<Item = CategoryPath>>,
        }
    }
}
#[cfg(todo)]
impl<'a> IntoIterator for CategoryEnableFilterInfo<'a> {
    type IntoIter = Box<dyn Iterator<Item = Self::Item> + Clone + 'a>;
    type Item = CategoryPath;
    fn into_iter(self) -> Self::IntoIter {
        self.iter_matching()
    }
}

pub trait CategorySearchFilter {
    #[inline]
    fn pack_category_matches(&mut self, path: CategoryPath<PackPath>, cat: &Category) -> bool {
        if self.category_path_matches(path) { return true }
        self.category_name_matches(path, &cat.full_id, cat.display_name.as_ref())
    }
    #[inline]
    fn category_path_matches(&mut self, path: CategoryPath<PackPath>) -> bool { false }
    #[inline]
    fn category_name_matches(&mut self, path: CategoryPath<PackPath>, id: &CategoryId, display_name: Option<&Arc<str>>) -> bool { false }
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
    fn category_name_matches(&mut self, path: CategoryPath<PackPath>, id: &CategoryId, display_name: Option<&Arc<str>>) -> bool {
        CategorySearchFilter::category_name_matches(*self, path, id, display_name)
    }
}
