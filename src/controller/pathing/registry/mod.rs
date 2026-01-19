#[cfg(doc)]
use taimi_pack::attributes::keys;
use {
    crate::{
        controller::pathing::{PackConfig, VisibilityFlagsExt},
        exports::runtime as rt,
        settings::source::sources::DataSourcePath,
    },
    anyhow::anyhow,
    bitvec::vec::BitVec,
    futures::{
        future::{self, Either},
        stream::{self, FusedStream, Stream, StreamExt},
        FutureExt,
    },
    rustc_hash::FxHasher,
    std::{
        cmp,
        collections::BTreeSet,
        error::Error as StdError,
        fmt,
        hash::{Hash, Hasher},
        iter,
        path::{Path, PathBuf},
        ptr,
        sync::Arc,
    },
    taimi_hoard::{
        iters::IterExt as _,
        loc::{indexed::IndexedList, LocationMut},
    },
    taimi_meta::{
        map::MapID,
        packs::{
            collections::{CategorySet, MapSet},
            CategoryIndex,
            CategoryPath,
            MapIndex,
            PackCategoryNs,
            VisibilityFlagSet,
            VisibilityFlags,
        },
    },
    taimi_pack::{
        category::{
            id::{AsFullId, CategoryId},
            Category,
            CategoryFlagSet,
            CategoryFlags,
        },
        pack::CategoryCollection,
        Pack,
    },
    taimi_sync::watched::watch,
    tokio::sync::{RwLock, RwLockMappedWriteGuard, RwLockWriteGuard},
    tokio_util::sync::ReusableBoxFuture,
};

#[doc(inline)]
pub use self::{
    active::{LoaderBox, PackActivateContext, PackActivateLoaded, PackFormat, PackLoader, SharedLoaderBox},
    namespace::*,
};

mod active;
mod namespace;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackInfo {
    pub format: PackFormat,
    pub roots: BTreeSet<PackRoot>,
    pub categories: Arc<PackCategoryInfo>,
    pub maps: MapSet,
}

impl PackInfo {
    /// TODO: deprecate this soon
    pub fn from_pack(pack: &Pack, format: PackFormat) -> Self {
        let roots = pack
            .categories
            .root_categories
            .iter()
            .filter_map(|id| pack.categories.all_categories.get_full(id))
            .map(|(i, _, cat)| {
                PackRoot::from_category(
                    CategoryPath::with_path(i as CategoryIndex),
                    cat,
                    Some(&pack.categories),
                )
            })
            .collect();

        let trail_maps = pack.trails.iter().filter_map(|trail| trail.map_id);
        let poi_maps = pack.pois.iter().map(|poi| poi.map_id);
        let maps = trail_maps
            .chain(poi_maps)
            .filter_map(|id| MapID::try_from(id).ok())
            .collect();

        let mut categories = PackCategoryInfo::from_collection(&pack.categories);
        let not_lonely = {
            let pois = pack.pois.iter().map(|m| &m.category);
            let trails = pack.trails.iter().map(|m| &m.category);
            pois.chain(trails)
                .filter_map(|c| pack.categories.all_categories.get_index_of(c.as_id()))
                .map(|i| CategoryPath::with_path(i as CategoryIndex))
        };
        categories.fill_lonely(not_lonely);

        PackInfo {
            format,
            roots,
            maps,
            categories: Arc::new(categories),
        }
    }

    pub fn primary_root(&self) -> Option<&PackRoot> {
        self.roots.iter().max_by_key(|root| {
            (
                !root.flags.is_separator(),
                !root.flags.is_hidden(),
                !root.flags.is_disabled(),
                root.child_count,
                &root.id,
            )
        })
    }
}

impl fmt::Display for PackInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.primary_root() {
            Some(root) => f.write_str(&root.display_name),
            None => fmt::Display::fmt(&self.format, f),
        }
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct PackInfoSignature {
    // TODO: consider atomic variant? sad that the traits are unstable...
    pub hash: u32,
}
impl PackInfoSignature {
    pub const EMPTY: Self = Self { hash: 0 };
    pub const INVALID: Self = Self { hash: u32::MAX };
    pub const HASHER_SEED: usize = 0x2673f7efbc5f2804u64 as usize;

    pub const fn with_hash(hash: u32) -> Self {
        Self { hash }
    }

    /// [Self::hashpart_info] with [Self::hasher()]
    pub fn from_info(info: &PackInfo) -> Self {
        Self::hash_with(|state|
            Self::hashpart_info(state, info)
        )
    }

    #[inline]
    pub fn hash_with<F: FnOnce(&mut PackInfoHasher)>(f: F) -> Self {
        let mut hasher = Self::hasher();
        f(&mut hasher);
        Self { hash: Self::hasher_finish_u32(&hasher) }
    }
    pub fn hash_with_dyn<F: FnOnce(&mut dyn Hasher)>(f: F) -> Self {
        Self::hash_with(|h| f(h))
    }

    /// basic checks for pack changes
    pub fn hashpart_info<H: Hasher>(hasher: &mut H, info: &PackInfo) {
        (info.maps.len() as u16).hash(hasher);
        Self::hashpart_categories(hasher, &info.categories);
    }
    pub fn hashpart_categories<H: Hasher>(hasher: &mut H, categories: &PackCategoryInfo) {
        categories.all.hash(hasher);
        categories.roots.hash(hasher);
    }

    /// [FxHasher] with [Self::HASHER_SEED]
    pub fn hasher() -> PackInfoHasher {
        FxHasher::with_seed(Self::HASHER_SEED)
    }

    /// fix output of [FxHasher::finish]
    ///
    /// by default rotates to orient entropy in a way that improves common hashmap implementations,
    /// which is sub-optimal when we know the bucket size in advance (fixed to 32bits here)
    pub fn hasher_finish_u32(hasher: &FxHasher) -> u32 {
        match hasher.finish() {
            #[cfg(target_pointer_width = "32")]
            hash => {
                // hash is a usize so upper bits are 0 anyway on 32bit systems
                hash as u32
            },
            #[cfg(not(target_pointer_width = "32"))]
            hash => {
                hash.rotate_left(Self::REMAINING_ROTATION) as u32
            },
        }
    }
    /// for [Self::hasher_finish_u32]
    ///
    /// see https://github.com/rust-lang/rustc-hash/blob/1a998d5b89b04ba730d4cd249f811e8b48aa7d8c/src/lib.rs#L177-L180
    #[allow(unreachable_patterns, dead_code)]
    const FX_ROTATE: u32 = match () {
        #[cfg(target_pointer_width = "32")]
        _ => 15,
        _ => 26,
    };
    /// after having been [rotated](usize::rotate_left) by [Self::FX_ROTATE],
    /// we just want the most significant bits back
    const REMAINING_ROTATION: u32 = 32 - Self::FX_ROTATE;

    #[inline]
    pub const fn is_empty(&self) -> bool {
        matches!(*self, Self::EMPTY)
    }

    #[inline]
    pub const fn get(&self) -> Option<Self> {
        match self.is_empty() {
            true => None,
            false => Some(*self),
        }
    }
}
type PackInfoHasher = FxHasher;
#[cfg(todo)]
type PackInfoHasher = impl Hasher + Clone + 'static;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackCategoryInfo {
    pub all: Box<[PackCategory]>,
    pub roots: Box<[CategoryIndex]>,
    pub visibility: VisibilityFlagSet,
    /// [keys::IsSeparator]
    pub separators: CategorySet,
    /// [keys::IsHidden]
    pub hidden: CategorySet,
    /// \![keys::DefaultToggle]
    pub disabled: CategorySet,
    /// [keys::CopyValue] is valid on [self.separators]
    pub copyable: CategorySet,
    /// Categories that lack any marker children, toggling would be meaningless
    ///
    /// TODO: reconsider if this is useful
    /// Currently this also includes category parents, but this may change.
    pub lonely: CategorySet,
}

impl PackCategoryInfo {
    pub fn from_collection(collection: &CategoryCollection) -> Self {
        let all = PackCategory::build(collection);
        let roots = collection
            .root_categories
            .iter()
            .filter_map(|id| collection.all_categories.get_index_of(id))
            .map(|i| i as CategoryIndex)
            .collect();
        let visibility = collection
            .all_categories
            .values()
            .map(VisibilityFlags::from_pack_category)
            .collect();
        let (separators, hidden, disabled, copyable) = collection
            .all_categories
            .values()
            .enumerate()
            .map(|(i, cat)| (i as CategoryIndex, cat))
            .map(|(i, cat)| {
                (
                    cat.is_separator().then_some(i),
                    cat.is_hidden().then_some(i),
                    (!cat.default_toggle()).then_some(i),
                    cat.marker_attributes
                        .interaction
                        .as_ref()
                        .map(|i| i.copy_value.is_some())
                        .unwrap_or(false)
                        .then_some(i),
                )
            })
            .unzip4_flatten();

        Self {
            all: all.into_boxed_slice(),
            roots,
            visibility,
            separators,
            hidden,
            disabled,
            copyable,
            lonely: Default::default(),
        }
    }

    pub fn fill_lonely<C>(&mut self, with_children: C)
    where
        C: IntoIterator<Item = CategoryPath>,
    {
        let mut marker_parents: BitVec = BitVec::with_capacity(self.all.len());
        marker_parents.resize(self.all.len(), false);
        for m in with_children {
            if let Some(mut b) = marker_parents.get_mut(m.path as usize) {
                if b.replace(true) {
                    continue
                }
            } else {
                // who are you?
                continue
            }
            // mark up to the root now...
            let mut parent = m;
            while let Some(next) = self.parent_of(parent) {
                if let Some(mut p) = marker_parents.get_mut(next.path as usize) {
                    if p.replace(true) {
                        break
                    }
                }
                parent = next;
            }
        }
        for lonely in marker_parents.iter_zeros() {
            let cat = CategoryPath::with_path(lonely as CategoryIndex);
            let Some(info) = self.info_of(cat) else { continue };
            #[cfg(todo = "unnecessary")]
            if info.child().is_some() {
                // this may not even be right? you can have children that are all hidden or separators etc...
                continue
            };
            if self.lonely.insert(cat) {
                // XXX: reconsider whether to include parents in this collection or not...
                // it's easy to filter them out at least?
                let mut cat = cat;
                while let Some(parent) = self.parent_of(cat) {
                    if !self.lonely.insert(cat) {
                        break
                    }
                    cat = parent;
                }
            }
        }
    }

    pub fn root_paths(&self) -> impl Iterator<Item = CategoryPath> + Clone + '_ {
        self.roots.iter().lazy_map(|&p| CategoryPath::with_path(p))
    }
    /// TODO: sorted lookup? probably a bad idea when there's likely just 1 or 2 at the most though...
    pub fn is_root(&self, path: CategoryPath) -> bool {
        self.roots.contains(&path.path)
    }

    pub fn count(&self) -> usize {
        self.all.len()
    }

    pub fn info_of(&self, path: CategoryPath) -> Option<PackCategory> {
        self.all.get(path.path as usize).copied()
    }

    /// immediate, see [self.descendents_of] for recursion
    pub fn children_of(&self, path: CategoryPath) -> impl Iterator<Item = CategoryPath> + '_ {
        let firstborn = self.firstborn_of(path).into_iter();
        firstborn.flat_map(|firstborn| iter::once(firstborn).chain(self.younger_siblings_of(firstborn)))
    }

    /// DFS, excludes the path itself
    pub fn descendents_of(&self, path: CategoryPath) -> impl Iterator<Item = CategoryPath> + '_ {
        let mut cycle_limit = self.all.len();
        let target = self.info_of(path);
        let firstborn = target.and_then(|c| c.child());
        let mut stack: Vec<CategoryPath> = match &target {
            #[cfg(todo = "unnecessary")]
            Some(info) if firstborn.is_some() => {
                // rough count of categories under a root...
                let amt = (self.all.len() - self.lonely.len()) / self.roots.len();
                let depth = match info.parent() {
                    Some(p) => self.ancestors_of(p).count(),
                    None => 0,
                } + 1;
                let stride = self.younger_siblings_of(path).count();
                #[cfg(todo = "unnecessary")]
                let mut child_cap = amt / 4;

                let cap = 0x40;
                let cap = match amt.checked_ilog2() {
                    Some(est) => {
                        let depth_rem = est.saturating_sub(depth) + 1;
                        #[cfg(todo = "unnecessary")]
                        child_cap = 2usize.ipow2(depth_rem).min(self.all.len());
                        (depth_rem + 1) * 2
                    },
                    None => cap,
                };
                (
                    Vec::with_capacity(cap),
                    #[cfg(todo = "unnecessary")]
                    CategorySet::with_capacity(child_cap),
                )
            },
            Some(..) if firstborn.is_some() => Vec::with_capacity(0x40),
            _ => Vec::new(),
        };
        if let Some(firstborn) = firstborn {
            stack.push(CategoryPath::with_path(firstborn));
        }
        iter::from_fn(move || {
            loop {
                let next = stack.pop()?;
                match cycle_limit.checked_sub(1) {
                    Some(l) => cycle_limit = l,
                    None => {
                        // who knows, mistakes and/or corruption can happen!
                        log::error!(
                            "category descendents exceeded cycle limit, stuck at {next} while {} deep",
                            stack.len()
                        );
                        return None
                    },
                }
                let Some(next_info) = self.info_of(next) else { continue };
                if let Some(sibling) = next_info.sibling() {
                    stack.push(CategoryPath::with_path(sibling));
                }
                if let Some(child) = next_info.child() {
                    stack.push(CategoryPath::with_path(child));
                }
                break Some(next)
            }
        })
    }
    /// like [self.descendents_of()] but emits parents before children
    pub fn nested_descendents_of(&self, path: CategoryPath) -> DescendentIter<'_> {
        DescendentIter::with_root(self, path)
    }

    pub fn ancestors_of(&self, path: CategoryPath) -> impl Iterator<Item = CategoryPath> + '_ {
        let mut next = self.parent_of(path);
        iter::from_fn(move || {
            let current = next.take()?;
            next = self.parent_of(current);
            Some(current)
        })
    }

    pub fn younger_siblings_of(&self, path: CategoryPath) -> impl Iterator<Item = CategoryPath> + '_ {
        let mut next = self.sibling_of(path);
        iter::from_fn(move || {
            let current = next.take()?;
            next = self.sibling_of(current);
            Some(current)
        })
    }
    pub fn all_siblings_of(&self, path: CategoryPath) -> impl Iterator<Item = CategoryPath> + '_ {
        let parent = self.parent_of(path);
        let root_fallback = match parent {
            Some(..) => None,
            None => Some(self.root_paths()),
        };
        parent
            .into_iter()
            .flat_map(move |parent| self.children_of(parent))
            .chain(root_fallback.into_iter().flatten())
            .filter(move |p| *p != path)
    }

    pub fn parent_of(&self, path: CategoryPath) -> Option<CategoryPath> {
        self.info_of(path)
            .and_then(|c| c.parent())
            .map(CategoryPath::with_path)
    }
    pub fn sibling_of(&self, path: CategoryPath) -> Option<CategoryPath> {
        self.info_of(path)
            .and_then(|c| c.sibling())
            .map(CategoryPath::with_path)
    }
    pub fn firstborn_of(&self, path: CategoryPath) -> Option<CategoryPath> {
        self.info_of(path)
            .and_then(|c| c.child())
            .map(CategoryPath::with_path)
    }

    pub fn disabled(&self) -> impl Iterator<Item = CategoryPath> + Clone + '_ {
        self.disabled.iter().lazy_map(CategoryPath::with_path)
    }
    pub fn hidden(&self) -> impl Iterator<Item = CategoryPath> + Clone + '_ {
        self.hidden.iter().lazy_map(CategoryPath::with_path)
    }
    pub fn separators(&self) -> impl Iterator<Item = CategoryPath> + Clone + '_ {
        self.separators.iter().lazy_map(CategoryPath::with_path)
    }

    #[inline]
    pub fn all(&self) -> &IndexedList<PackCategoryNs, CategoryIndex, [PackCategory]> {
        IndexedList::from_ref(&self.all)
    }

    pub fn all_flags(&self) -> impl Iterator<Item = (CategoryPath, &PackCategory, CategoryFlags)> + Clone {
        self.all().iter().lazy_map(|(path, cat)| {
            let mut flag = CategoryFlags::empty();
            if cat.parent().is_none() {
                flag.insert(CategoryFlags::ROOT);
            }
            if self.hidden.contains(path) {
                flag.insert(CategoryFlags::HIDDEN);
            }
            if self.disabled.contains(path) {
                flag.insert(CategoryFlags::DISABLED);
            }
            if self.separators.contains(path) {
                flag.insert(CategoryFlags::SEPARATOR);
            }
            (path, cat, flag)
        })
    }
    pub fn lookup_flags(&self, path: CategoryPath) -> CategoryFlags {
        self.all_flags()
            .nth(path.path as usize)
            .map(|(_, _, flags)| flags)
            .unwrap_or(CategoryFlags::empty())
    }

    pub fn collect_all_flags(&self) -> PackCategoryFlags {
        let flags = self.all_flags().map(|(_, _, flags)| flags).collect();
        IndexedList::new(flags)
    }
}
pub type PackCategoryFlags<N = PackCategoryNs> = IndexedList<N, CategoryIndex, CategoryFlagSet>;
#[derive(Debug, Copy, Clone)]
struct DescendentIterNode {
    pub prev: CategoryPath,
    pub sibling: CategoryPath,
}
impl DescendentIterNode {
    pub const fn new(firstborn: CategoryPath) -> Self {
        Self { sibling: firstborn, prev: firstborn }
    }
    pub const fn depleted(prev: CategoryPath) -> Self {
        Self {
            prev,
            sibling: CategoryPath::new_path(CategoryIndex::MAX),
        }
    }

    pub fn is_initial(&self) -> bool {
        self.sibling == self.prev
    }
}
pub struct DescendentIter<'a> {
    cats: &'a PackCategoryInfo,
    stack: Vec<DescendentIterNode>,
    cycle_limit: u32,
}
impl<'a> DescendentIter<'a> {
    pub fn with_root(cats: &'a PackCategoryInfo, root: CategoryPath) -> Self {
        let mut iter = Self::empty(cats);
        iter.start_from(root);
        iter
    }
    pub fn empty(cats: &'a PackCategoryInfo) -> Self {
        Self {
            cats,
            stack: Vec::new(),
            cycle_limit: cats.all.len() as _,
        }
    }
    pub fn start_from(&mut self, root: CategoryPath) {
        let target = self.cats.info_of(root);
        let firstborn = target.and_then(|c| c.child());
        let capacity = match &target {
            #[cfg(todo = "unnecessary")]
            Some(info) if firstborn.is_some() => {
                // rough count of categories under a root...
                let amt = (self.all.len() - self.lonely.len()) / self.roots.len();
                let depth = match info.parent() {
                    Some(p) => self.ancestors_of(p).count(),
                    None => 0,
                } + 1;
                let stride = self.younger_siblings_of(path).count();
                #[cfg(todo = "unnecessary")]
                let mut child_cap = amt / 4;

                let cap = 0x40;
                let cap = match amt.checked_ilog2() {
                    Some(est) => {
                        let depth_rem = est.saturating_sub(depth) + 1;
                        #[cfg(todo = "unnecessary")]
                        child_cap = 2usize.ipow2(depth_rem).min(self.all.len());
                        (depth_rem + 1) * 2
                    },
                    None => cap,
                };
                (
                    cap,
                    #[cfg(todo = "unnecessary")]
                    CategorySet::with_capacity(child_cap),
                )
            },
            Some(..) if firstborn.is_some() => 0x40,
            _ => 0,
        };
        self.stack.reserve(capacity);
        #[cfg(todo = "unnecessary")]
        {
            self.stack.push(DescendentIterNode::depleted(root));
        }
        if let Some(firstborn) = firstborn.map(CategoryPath::with_path) {
            self.stack.push(DescendentIterNode::new(firstborn));
        }
    }

    pub fn current_chain(
        &self,
    ) -> impl DoubleEndedIterator<Item = CategoryPath> + ExactSizeIterator + Clone + '_ {
        let fresh_child = self.peek_next_is_child();
        let mut chain = self.stack.iter();
        if fresh_child {
            let _ = chain.next_back();
        }
        chain.lazy_map(|node| node.prev)
    }
    /// of the last-produced node
    pub fn current_ancestors(
        &self,
    ) -> impl DoubleEndedIterator<Item = CategoryPath> + ExactSizeIterator + Clone + '_ {
        self.current_chain().rev().skip(1)
    }

    /// of the last-produced node
    pub fn depth(&self) -> usize {
        self.current_chain().count()
    }

    /// (path, depth)
    pub fn peek_next(&self) -> Option<(CategoryPath, usize)> {
        let mut chain = self.stack.iter().rev();
        while let Some(node) = chain.next() {
            if node.sibling.path != CategoryIndex::MAX {
                return Some((node.sibling, chain.count() + 1))
            }
        }
        None
    }
    pub fn peek_next_path(&self) -> Option<CategoryPath> {
        self.peek_next().map(|(path, _depth)| path)
    }
    pub fn peek_next_depth(&self) -> usize {
        self.peek_next().map(|(_path, depth)| depth).unwrap_or(0)
    }
    pub fn peek_next_is_child(&self) -> bool {
        self.stack.last().map(|node| node.is_initial()).unwrap_or(false)
    }
    pub fn peek_next_is_ancestor(&self) -> bool {
        self.stack
            .last()
            .map(|node| node.sibling.path == CategoryIndex::MAX)
            .unwrap_or(true)
    }
    /// repeat the latest value produced by the iter
    ///
    /// its depth was just [self.depth()]
    pub fn peek_prev(&self) -> Option<CategoryPath> {
        self.current_chain().rev().next()
    }

    /// skip any children of the current node (true if any existed)
    pub fn skip_to_sibling(&mut self) -> bool {
        let has_children = self.peek_next_is_child();
        if has_children {
            self.stack.pop();
        }
        has_children
    }
    fn skip_to_direct_parent(&mut self) -> bool {
        self.stack.pop();
        self.peek_next_is_ancestor()
    }

    /// true if there's still more to pop up from
    /// (or empty because the next would be the original root)
    ///
    /// false indicates parent had a sibling
    pub fn skip_to_parent(&mut self) -> bool {
        let _ = self.skip_to_sibling();
        self.skip_to_direct_parent()
    }
    /// returns amount of depth levels popped
    ///
    /// expect at least 1 while popping to direct parent of the latest node
    /// (unless iter was empty?)
    pub fn skip_up(&mut self) -> usize {
        let _ = self.skip_to_sibling();
        let mut count = 0;
        while !self.stack.is_empty() {
            count += 1;
            if !self.skip_to_direct_parent() {
                break
            }
        }
        count
    }
}
impl Iterator for DescendentIter<'_> {
    type Item = CategoryPath;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let current = self.stack.last_mut()?;
            let next_path = current.sibling;
            current.prev = next_path;
            let info = match self.cats.info_of(next_path) {
                None => {
                    // out of children at this level...
                    self.stack.pop();
                    continue
                },
                Some(i) => i,
            };
            current.sibling = CategoryPath::with_path(info.sibling().unwrap_or(CategoryIndex::MAX));
            match self.cycle_limit.checked_sub(1) {
                Some(l) => self.cycle_limit = l,
                None => {
                    // who knows, mistakes and/or corruption can happen!
                    log::error!(
                        "category descendents exceeded cycle limit, stuck at {next_path} while {} deep",
                        self.stack.len()
                    );
                    break None
                },
            }
            if let Some(firstborn) = info.child().map(CategoryPath::with_path) {
                self.stack.push(DescendentIterNode::new(firstborn));
            }

            break Some(next_path)
        }
    }
}

/// TODO: anything else interesting about the root category?
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackRoot {
    pub index: CategoryIndex,
    pub id: CategoryId,
    pub flags: CategoryFlags,
    pub display_name: Arc<str>,
    pub child_count: usize,
}

impl PackRoot {
    pub fn from_category(
        path: CategoryPath,
        category: &Category,
        collection: Option<&CategoryCollection>,
    ) -> Self {
        #[cfg(todo = "unnecessary")]
        if category.full_id != category.id {
            return None
        }
        Self {
            index: path.path,
            id: category.full_id.clone(),
            display_name: category.display_name.clone().unwrap_or_else(|| category.display_name().into()),
            flags: category.flags,
            child_count: match collection {
                Some(c) => c
                    .all_categories
                    .values()
                    .filter(|c| c.full_id.id_starts_with(&category.full_id))
                    .count(),
                None => category.sub_categories.len(),
            },
        }
    }

    pub fn path(&self) -> CategoryPath {
        CategoryPath::with_path(self.index)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackCategory {
    pub sibling: CategoryIndex,
    pub child: CategoryIndex,
    pub parent: CategoryIndex,
}

pub(crate) fn category_index_get(index: CategoryIndex) -> Option<CategoryIndex> {
    match index {
        CategoryIndex::MAX => None,
        index => Some(index),
    }
}
#[inline]
pub(crate) fn category_index_set(index: Option<CategoryIndex>) -> CategoryIndex {
    index.unwrap_or(CategoryIndex::MAX)
}
impl PackCategory {
    pub const EMPTY: Self = Self {
        sibling: CategoryIndex::MAX,
        child: CategoryIndex::MAX,
        parent: CategoryIndex::MAX,
    };

    pub fn sibling(&self) -> Option<CategoryIndex> {
        category_index_get(self.sibling)
    }
    pub fn set_sibling(&mut self, index: Option<CategoryIndex>) {
        self.sibling = category_index_set(index);
    }
    pub fn child(&self) -> Option<CategoryIndex> {
        category_index_get(self.child)
    }
    pub fn set_child(&mut self, index: Option<CategoryIndex>) {
        self.child = category_index_set(index);
    }
    pub fn parent(&self) -> Option<CategoryIndex> {
        category_index_get(self.parent)
    }
    pub fn set_parent(&mut self, index: Option<CategoryIndex>) {
        self.parent = category_index_set(index);
    }

    pub fn build(collection: &CategoryCollection) -> Vec<Self> {
        let mut cats = Vec::with_capacity(collection.all_categories.len());
        cats.resize(collection.all_categories.len(), PackCategory::EMPTY);
        for (idx, (_name, category)) in collection.all_categories.iter().enumerate() {
            let path: CategoryPath = CategoryPath::with_path(idx as CategoryIndex);
            let mut children = category.child_ids().filter_map(|child_full_id| {
                match collection.all_categories.get_full(child_full_id) {
                    None => {
                        log::warn!("child category {child_full_id} of {_name} not found");
                        None
                    },
                    Some((child_index, _child_full_id, _child)) => Some(child_index as CategoryIndex),
                }
            });
            let Some(mut child_index) = children.next() else {
                // empty or leaf category, nothing else to do here
                continue
            };
            match cats.get_mut(idx) {
                Some(cat) if cat.child().is_none() => {
                    cat.set_child(Some(child_index));
                },
                _ => (),
            };
            loop {
                if let Some(child) = cats.get_mut(child_index as usize) {
                    match child.parent() {
                        Some(p) if p != path.path => {
                            log::warn!(
                                "category {_name} child#{child_index} already has different parent #{p}?"
                            );
                        },
                        Some(..) => (),
                        None => {
                            child.set_parent(Some(path.path));
                        },
                    }
                    #[cfg(todo = "unnecessary")]
                    if let Some(sibling) = child.sibling() {
                        child_index = s;
                        continue
                    }
                    let Some(next_child) = children.next() else { break };
                    child.set_sibling(Some(next_child));
                    child_index = next_child;
                }
            }
        }
        cats
    }
}

impl Default for PackCategory {
    fn default() -> Self {
        Self::EMPTY
    }
}

#[derive(Debug, Clone)]
pub enum UnloadedReason {
    Disabled,
    /// Reserved index will not be reused
    Gravestone,
    Pending,
    Loading,
    UnknownFormat,
    LoadingFailed(Arc<dyn StdError + Send + Sync>),
}

impl UnloadedReason {
    pub fn can_reload(&self) -> bool {
        match self {
            UnloadedReason::Gravestone | UnloadedReason::Disabled | UnloadedReason::Loading => false,
            _ => true,
        }
    }
    pub fn can_reactivate(&self, explicit: bool) -> bool {
        match self {
            UnloadedReason::Pending => true,
            _ if explicit => self.can_reload(),
            _ => false,
        }
    }

    pub(crate) fn discriminant(&self) -> u8 {
        match self {
            Self::Pending => 1,
            Self::Loading => 2,
            Self::Disabled => 3,
            Self::Gravestone => 4,
            Self::UnknownFormat => 5,
            Self::LoadingFailed(..) => 6,
        }
    }
}

impl Eq for UnloadedReason {}
impl PartialEq for UnloadedReason {
    fn eq(&self, rhs: &Self) -> bool {
        match (self, rhs) {
            | (Self::Pending, Self::Pending)
            | (Self::Loading, Self::Loading)
            | (Self::Disabled, Self::Disabled)
            | (Self::Gravestone, Self::Gravestone)
            | (Self::UnknownFormat, Self::UnknownFormat) => true,
            (Self::LoadingFailed(e), Self::LoadingFailed(rhs)) => Arc::ptr_eq(e, rhs),
            _ => false,
        }
    }
}
impl Ord for UnloadedReason {
    fn cmp(&self, rhs: &Self) -> cmp::Ordering {
        let d = self.discriminant().cmp(&rhs.discriminant());
        match (d, self, rhs) {
            (cmp::Ordering::Equal, Self::LoadingFailed(lhs), Self::LoadingFailed(rhs)) =>
                Arc::as_ptr(lhs).cast::<()>().cmp(&Arc::as_ptr(rhs).cast::<()>()),
            (cmp, ..) => cmp,
        }
    }
}
impl PartialOrd for UnloadedReason {
    fn partial_cmp(&self, rhs: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(rhs))
    }
}
impl Hash for UnloadedReason {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let e = match self {
            Self::LoadingFailed(e) => Arc::as_ptr(e) as *const (),
            _ => ptr::null(),
        };
        (self.discriminant(), e).hash(state)
    }
}

impl fmt::Display for UnloadedReason {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Disabled => f.write_str("disabled"),
            Self::Gravestone => f.write_str("removed"),
            Self::Pending => f.write_str("not yet loaded"),
            Self::Loading => f.write_str("loading"),
            Self::UnknownFormat => f.write_str("expected TacO zip or folder"),
            Self::LoadingFailed(e) => write!(f, "{e:#}"),
        }
    }
}

impl StdError for UnloadedReason {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::LoadingFailed(e) => e.source(),
            _ => None,
        }
    }
}
