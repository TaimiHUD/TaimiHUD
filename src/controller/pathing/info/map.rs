use {
    crate::controller::pathing::{
        registry::{
            LoadedCategoryIndex,
            LoadedCategoryNs,
            LoadedCategoryPath,
            LoadedMarkerPath,
            LoadedPoiIndex,
            LoadedPoiNs,
            LoadedPoiPath,
            LoadedTrailIndex,
            LoadedTrailNs,
            LoadedTrailPath,
            LoadedTrailSectionNs,
            LoadedTrailSectionPath,
            PackInfo,
            PackInfoSignature,
            PackMapPath,
            PackPath,
        },
        space::{DrawSpace, TrailParams},
        state::LoadedTrailSection,
    },
    bitvec::vec::BitVec,
    glamour::Box3,
    std::{iter, mem, sync::Arc},
    taimi_hoard::{
        flags::BitSet,
        iters::IterExt as _,
        loc::{indexed::IndexedList, LocationGet, LocationRef, NamespacePivotTo},
    },
    taimi_meta::packs::{
        collections::CategorySet,
        id::{MarkerIndex, MarkerIndexVariant, MarkerPath},
        CategoryIndex,
        CategoryPath,
        MapIndex,
        PackCategoryNs,
        PoiIndex,
        PoiPath,
        TrailIndex,
        TrailPath,
        TrailSectionIndex,
        TrailSectionNs,
        TrailSectionPath,
    },
    taimi_pack::{category::id::FullIdRef, pack::Pack, trail::TrailData},
};

#[derive(Debug, Clone)]
pub struct MapPackInfo {
    pub info_sig: PackInfoSignature,
    pub pois: BitVec,
    pub trails: BitVec,
    pub trail_info: IndexedList<LoadedTrailNs, LoadedTrailIndex, Box<[MapTrailInfo]>>,
    pub categories: Box<[CategoryIndex]>,
    /// TODO: not all GUIDs are needed at runtime,
    /// if for example the marker can't be interacted with
    #[cfg(todo)]
    pub poi_guid_mask: BitVec,
    #[cfg(todo)]
    pub trail_guid_mask: BitVec,
}

impl MapPackInfo {
    pub fn empty() -> Self {
        Self {
            info_sig: PackInfoSignature::EMPTY,
            pois: BitVec::new(),
            trails: BitVec::new(),
            trail_info: Default::default(),
            categories: Default::default(),
        }
    }

    pub fn with_pack(map_id: MapIndex, pack: &Pack, info: &PackInfo) -> Self {
        let info_sig = PackInfoSignature::from_info(info);

        let id32 = map_id.get() as i32;
        let mut categories = {
            let category_estimate = pack.categories.all_categories.len() / 32;
            CategorySet::with_capacity(category_estimate)
        };
        let mut insert_cat = |category: &FullIdRef| -> bool {
            if let Some(idx) = pack.categories.all_categories.get_index_of(category) {
                categories.insert_index(idx)
            } else {
                true
            }
        };
        let mut filter_mapid = |map_id: i32, mut category: &FullIdRef| -> bool {
            if map_id == id32 {
                loop {
                    if !insert_cat(category) {
                        break
                    }
                    category = match category.parent() {
                        Some(parent) => parent,
                        None => break,
                    };
                }
                true
            } else {
                false
            }
        };
        let mut active_pois = pack
            .pois
            .iter()
            .enumerate()
            .filter(|(_i, poi)| filter_mapid(poi.map_id, poi.category.as_ref()))
            .lazy_map(|(i, _)| i);
        let pois = BitSet::collect_sorted(active_pois);
        // TODO: use some sort of space-efficient encoding like RLE for these masks
        // even just an initial offset or vec of bit group lengths (pos/neg for 0 vs 1) would help?
        let mut active_trails = pack
            .trails
            .iter()
            .enumerate()
            .filter(|(_i, trail)| filter_mapid(trail.map_id.unwrap_or(0), trail.category.as_ref()))
            .lazy_map(|(i, _)| i);
        let trails = BitSet::collect_sorted(active_trails);

        let categories = categories.into_index_boxed();

        Self {
            info_sig,
            pois: pois.into_flags(),
            trails: trails.into_flags(),
            categories,
            trail_info: Default::default(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.info_sig.is_empty()
            || ((self.trails.is_empty() || self.trails[..].not_any())
                && (self.pois.is_empty() || self.pois[..].not_any()))
    }

    /// None if \![Self::is_empty()]
    pub fn get(self) -> Option<Self> {
        (!self.is_empty()).then_some(self)
    }

    pub fn poi_count(&self) -> usize {
        self.pois.count_ones()
    }
    pub fn pois(&self) -> impl Iterator<Item = PoiPath> + '_ {
        self.pois
            .iter_ones()
            .lazy_map(|i| PoiPath::with_path(i as PoiIndex))
    }
    pub fn loaded_pois(&self) -> impl Iterator<Item = (LoadedPoiPath, PoiPath)> + '_ {
        self.pois()
            .enumerate()
            .lazy_map(|(i, path)| (LoadedPoiPath::with_path(i as LoadedPoiIndex), path))
    }
    #[cfg(todo)]
    pub(crate) fn poi_guid_mask(&self) -> impl Iterator<Item = bool> + '_ {
        self.poi_guid_mask.iter()
    }
    pub(crate) fn poi_guid_mask(&self) -> impl Iterator<Item = bool> + '_ {
        iter::repeat(true).take(self.poi_count())
    }
    pub(crate) fn poi_guid_filter<'a, I>(&'a self, iter: I) -> impl Iterator<Item = I::Item> + 'a
    where
        I: IntoIterator + 'a,
    {
        self.poi_guid_mask()
            .zip(iter)
            .filter_map(|(mask, v)| mask.then_some(v))
    }
    pub fn poi_index(&self, path: PoiPath) -> Option<LoadedPoiPath> {
        match () {
            #[cfg(todo = "unnecessary")]
            _ => self
                .pois()
                .position(|t| t.path == path.path)
                .map(|i| LoadedPoiPath::with_path(i as LoadedPoiIndex)),
            _ => match self.pois.get(path.path as usize) {
                None => None,
                Some(b) if !*b => None,
                Some(_) => Some(unsafe { self.poi_index_unchecked(path) }),
            },
        }
    }
    pub unsafe fn poi_index_unchecked(&self, path: PoiPath) -> LoadedPoiPath {
        let index = path.path as usize;
        let preceding = self.pois.get_unchecked(..index);
        LoadedPoiPath::with_path(preceding.count_ones() as LoadedPoiIndex)
    }
    /// TODO: `nth` isn't implemented on [bitvec::slice::IterOnes], it should
    /// probably popcnt instead?
    pub fn poi_path(&self, path: LoadedPoiPath) -> Option<PoiPath> {
        self.pois().nth(path.path as usize)
    }
    /// TODO?
    #[inline]
    pub unsafe fn poi_path_unchecked(&self, path: LoadedPoiPath) -> PoiPath {
        self.poi_path(path).unwrap_unchecked()
    }
    pub fn trail_count(&self) -> usize {
        self.trails.count_ones()
    }
    pub fn trails(&self) -> impl Iterator<Item = TrailPath> + '_ {
        self.trails
            .iter_ones()
            .lazy_map(|i| TrailPath::with_path(i as TrailIndex))
    }
    pub fn loaded_trails(&self) -> impl Iterator<Item = (LoadedTrailPath, TrailPath)> + '_ {
        self.trails()
            .enumerate()
            .lazy_map(|(i, path)| (LoadedTrailPath::with_path(i as LoadedTrailIndex), path))
    }
    #[cfg(todo)]
    pub(crate) fn trail_guid_mask(&self) -> impl Iterator<Item = bool> + '_ {
        self.trail_guid_mask.iter()
    }
    pub(crate) fn trail_guid_mask(&self) -> impl Iterator<Item = bool> + '_ {
        iter::repeat(true).take(self.trail_count())
    }
    pub(crate) fn trail_guid_filter<'a, I>(&'a self, iter: I) -> impl Iterator<Item = I::Item> + 'a
    where
        I: IntoIterator + 'a,
    {
        self.trail_guid_mask()
            .zip(iter)
            .filter_map(|(mask, v)| mask.then_some(v))
    }
    pub fn trail_index(&self, path: TrailPath) -> Option<LoadedTrailPath> {
        match () {
            #[cfg(todo = "unnecessary")]
            _ => self
                .trails()
                .position(|t| t.path == path.path)
                .map(|i| LoadedTrailPath::with_path(i as LoadedTrailIndex)),
            _ => match self.trails.get(path.path as usize) {
                None => None,
                Some(b) if !*b => None,
                Some(_) => Some(unsafe { self.trail_index_unchecked(path) }),
            },
        }
    }
    pub unsafe fn trail_index_unchecked(&self, path: TrailPath) -> LoadedTrailPath {
        let index = path.path as usize;
        let preceding = self.trails.get_unchecked(..index);
        LoadedTrailPath::with_path(preceding.count_ones() as LoadedTrailIndex)
    }
    /// TODO: `nth` isn't implemented on [bitvec::slice::IterOnes], it should
    /// probably popcnt instead?
    pub fn trail_path(&self, path: LoadedTrailPath) -> Option<TrailPath> {
        self.trails().nth(path.path as usize)
    }
    /// TODO?
    #[inline]
    pub unsafe fn trail_path_unchecked(&self, path: LoadedTrailPath) -> TrailPath {
        self.trail_path(path).unwrap_unchecked()
    }
    #[cfg(todo = "unnecessary")]
    pub fn trail_info(&self) -> &IndexedList<LoadedTrailNs, LoadedTrailIndex, [MapTrailInfo]> {
        self.trail_info.map_ref_as_slice()
    }
    pub fn category_count(&self) -> usize {
        self.categories.len()
    }
    pub fn category_max(&self) -> Option<CategoryIndex> {
        self.categories.iter().max().copied()
    }
    pub fn category_max_count(&self) -> CategoryIndex {
        self.category_max().map(|c| c + 1).unwrap_or(0)
    }
    #[inline(always)]
    pub fn categories_ref(&self) -> &IndexedList<LoadedCategoryNs, LoadedCategoryIndex, [CategoryPath]> {
        let categories =
            unsafe { mem::transmute::<&[CategoryIndex], &[CategoryPath]>(&self.categories[..]) };
        IndexedList::from_ref(categories)
    }
    pub fn categories(&self) -> impl Iterator<Item = CategoryPath> + '_ {
        self.categories.iter().lazy_map(|&i| CategoryPath::with_path(i))
    }
    #[cfg(todo)]
    pub fn loaded_categories(&self) -> impl Iterator<Item = (LoadedCategoryPath, CategoryPath)> + '_ {
        self.categories()
            .enumerate()
            .lazy_map(|(i, path)| (LoadedCategoryPath::with_path(i as LoadedCategoryIndex), path))
    }
    #[inline]
    pub fn loaded_categories(&self) -> impl Iterator<Item = (LoadedCategoryPath, CategoryPath)> + '_ {
        self.categories_ref()
            .map_data_to(|cats| cats.iter().copied())
            .into_iter()
    }
    pub fn category_path(&self, path: LoadedCategoryPath) -> Option<CategoryPath> {
        self.categories().nth(path.path as usize)
    }
    #[inline(always)]
    pub unsafe fn category_path_unchecked(&self, path: LoadedCategoryPath) -> CategoryPath {
        *self.categories_ref().map_ref_as_slice().index_unchecked(path)
    }
    pub fn category_index(&self, path: CategoryPath) -> Option<LoadedCategoryPath> {
        match () {
            #[cfg(todo = "unnecessary")]
            _ => self
                .categories()
                .position(|t| t.path == path.path)
                .map(|i| LoadedCategoryPath::with_path(i as CategoryIndex)),
            _ => self
                .categories
                .binary_search(&path.path)
                .ok()
                .map(|i| LoadedCategoryPath::with_path(i as LoadedCategoryIndex)),
        }
    }
    /// TODO?
    #[inline]
    pub unsafe fn category_index_unchecked(&self, path: CategoryPath) -> LoadedCategoryPath {
        self.category_index(path).unwrap_unchecked()
    }

    pub fn marker_index(&self, path: MarkerPath) -> Option<LoadedMarkerPath> {
        match path.path.variant() {
            MarkerIndexVariant::Category(index) => self
                .category_index(LoadedCategoryPath::with_path(index))
                .map(LoadedCategoryNs::loc_pivot_to),
            MarkerIndexVariant::Poi(index) => self
                .poi_index(LoadedPoiPath::with_path(index))
                .map(LoadedPoiNs::loc_pivot_to),
            MarkerIndexVariant::Trail(index) => self
                .trail_index(LoadedTrailPath::with_path(index))
                .map(LoadedTrailNs::loc_pivot_to),
            MarkerIndexVariant::TrailSection(index, section) =>
                self.trail_index(LoadedTrailPath::with_path(index)).map(|path| {
                    LoadedTrailSectionNs::loc_pivot_to({
                        let section: TrailSectionPath = TrailSectionPath::with_path(section);
                        LoadedTrailSectionPath::with_path(path.rel(section))
                    })
                }),
            MarkerIndexVariant::Invalid(..) | MarkerIndexVariant::Unknown(..) => {
                log::warn!("asked to index unrecognized marker path {path}");
                None
            },
        }
    }
    pub fn path_from_loaded(&self, loaded: MarkerPath<PackMapPath>) -> Option<MarkerPath<PackPath>> {
        self.marker_path(loaded.unscope()).map(|p| {
            let pack_path = loaded.root.root;
            pack_path.rel(p.path)
        })
    }
    /// TODO: use NamespaceTryConv or whatever?
    pub fn marker_path(&self, path: LoadedMarkerPath) -> Option<MarkerPath> {
        Some(match path.path.variant() {
            MarkerIndexVariant::Category(index) => {
                let p = self.category_path(LoadedCategoryPath::with_path(index))?;
                #[cfg(todo = "unnecessary")]
                let index = MarkerIndex::with_category(p.path);
                MarkerPath::with_path(MarkerIndex::from(p))
            },
            MarkerIndexVariant::Poi(index) => {
                let p = self.poi_path(LoadedPoiPath::with_path(index))?;
                #[cfg(todo = "unnecessary")]
                let index = MarkerIndex::with_poi(p.path);
                MarkerPath::with_path(MarkerIndex::from(p))
            },
            MarkerIndexVariant::Trail(index) => {
                let p = self.trail_path(LoadedTrailPath::with_path(index))?;
                #[cfg(todo = "unnecessary")]
                let index = MarkerIndex::with_trail(p.path);
                MarkerPath::with_path(MarkerIndex::from(p))
            },
            MarkerIndexVariant::TrailSection(index, section) => {
                let p = self.trail_path(LoadedTrailPath::with_path(index))?;
                let index = MarkerIndex::with_trail_section(p.path, section);
                #[cfg(todo)]
                let index = p.into();
                MarkerPath::with_path(index)
            },
            MarkerIndexVariant::Invalid(..) | MarkerIndexVariant::Unknown(..) => {
                log::warn!("asked for path of unrecognized marker index {path}");
                return None
            },
        })
    }
    pub unsafe fn marker_path_unchecked(&self, path: LoadedMarkerPath) -> MarkerPath {
        match path.path.namespace() {
            MarkerIndex::NS_POI => self
                .poi_path_unchecked(LoadedPoiPath::with_path(path.path.index_poi_unchecked()))
                .pivot_from(),
            MarkerIndex::NS_TRAIL => self
                .trail_path_unchecked(LoadedTrailPath::with_path(path.path.trail_index_unchecked()))
                .pivot_from(),
            MarkerIndex::NS_CAT => self
                .category_path_unchecked(LoadedCategoryPath::with_path(
                    path.path.index_category_unchecked(),
                ))
                .pivot_from(),
            _ => MarkerPath::with_path(MarkerIndex::UNK),
        }
    }

    pub fn is_trail_info_loaded(&self, path: LoadedTrailPath) -> bool {
        self.trail_info
            .lookup_ref(&path)
            .map(|info| info.is_loaded())
            .unwrap_or(false)
    }
    pub(crate) fn update_trail_section_info(
        &mut self,
        path: LoadedTrailPath,
        sections: Arc<[LoadedTrailSection]>,
    ) {
        let trail_info = self
            .trail_info
            .lookup_extend_with(path.path, MapTrailInfo::default);
        trail_info.sections = Some(IndexedList::new(sections));
    }
}
impl LocationGet<PackCategoryNs, CategoryIndex> for MapPackInfo {
    type LookupGet = LoadedCategoryPath;
    #[inline]
    fn lookup_get(&self, &loc: &CategoryPath) -> Option<Self::LookupGet> {
        self.category_index(loc)
    }
}
impl LocationGet<LoadedCategoryNs, LoadedCategoryIndex> for MapPackInfo {
    type LookupGet = CategoryPath;
    #[inline]
    fn lookup_get(&self, &loc: &LoadedCategoryPath) -> Option<Self::LookupGet> {
        self.category_path(loc)
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MapTrailInfo {
    pub sections: Option<IndexedList<TrailSectionNs, TrailSectionIndex, Arc<[LoadedTrailSection]>>>,
    pub y_offset: f32,
}
impl MapTrailInfo {
    pub fn is_loaded(&self) -> bool {
        self.sections.is_some()
    }

    #[inline]
    pub fn sections(&self) -> &IndexedList<TrailSectionNs, TrailSectionIndex, [LoadedTrailSection]> {
        self.sections
            .as_ref()
            .map(IndexedList::map_ref_as_slice)
            .unwrap_or(IndexedList::empty_ref())
    }

    pub fn update_with_data(&mut self, trl: &TrailData) {
        let data = trl.sections.iter().map(LoadedTrailSection::with_section);
        self.sections = Some(IndexedList::new(data.collect()));
    }

    pub fn get_y_offsets(&'_ self) -> impl Iterator<Item = f32> + 'static {
        let mut y_offset = self.y_offset;
        iter::from_fn(move || {
            let next_offset = (y_offset - TrailParams::Y_OFFSET_SECTION_GAP).max(0.0);
            match mem::replace(&mut y_offset, next_offset) {
                0.0 => None,
                off => Some(off),
            }
        })
    }
    pub fn y_offsets(&'_ self) -> impl Iterator<Item = (TrailSectionPath, f32)> {
        let mut y_offsets = self.get_y_offsets();
        self.sections()
            .paths()
            .zip(iter::repeat(0.0).map(move |fallback| y_offsets.next().unwrap_or(fallback)))
    }

    pub fn section_bounds(
        &self,
    ) -> impl Iterator<Item = (TrailSectionPath, &LoadedTrailSection, Box3<DrawSpace>)> {
        let mut y_offsets = self.get_y_offsets();
        self.sections().iter().map(move |(path, section)| {
            let mut bounds = section.bounds;
            match y_offsets.next() {
                None | Some(0.0) => (),
                Some(off) => {
                    bounds.min.y += off;
                    bounds.max.y += off;
                },
            }
            (path, section, bounds)
        })
    }
}
