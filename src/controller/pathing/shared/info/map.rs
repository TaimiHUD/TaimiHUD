use {
    crate::controller::pathing::registry::{
        PackLoader, LoadedPack, PackMapPath, PackPath,
        LoadedPoiPath, LoadedTrailPath,
    },
    bitvec::vec::BitVec,
    taimi_meta::packs::{
        id::{MarkerIndexVariant, MarkerIndex, MarkerPath},
        MapIndex,
        PoiIndex, PoiPath,
        TrailIndex, TrailPath,
        CategoryIndex, CategoryPath,
    },
    taimi_pack::category::id::FullIdRef,
    taimi_hoard::iters::IterExt as _,
};

#[derive(Debug, Clone)]
pub struct MapPackInfo {
    pub pois: BitVec,
    pub trails: BitVec,
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
            pois: BitVec::new(),
            trails: BitVec::new(),
            categories: Default::default(),
        }
    }

    pub fn with_pack(pack: &LoadedPack, map_id: MapIndex) -> Self {
        let Some(active) = &pack.active else {
            return Self::empty()
        };

        // TODO: this doesn't need to use the string ids anymore...
        let id32 = map_id.get() as i32;
        let mut categories = {
            let category_estimate = active.pack.categories.all_categories.len() / 32;
            Vec::<CategoryIndex>::with_capacity(category_estimate)
        };
        let mut insert_cat = |category: &FullIdRef| -> bool {
            if let Some(idx) = active.pack.categories.all_categories.get_index_of(category) {
                let idx = idx as CategoryIndex;
                let insert = categories.partition_point(|&i| i < idx);
                match categories.get(insert) {
                    Some(&i) if i == idx => false,
                    _ => {
                        categories.insert(insert, idx);
                        true
                    },
                }
            } else {
                true
            }
        };
        let mut filter_mapid = |map_id: i32, mut category: &FullIdRef| -> bool {
            if map_id == id32 {
                loop {
                    if !insert_cat(category) { break }
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
        let mut pois = BitVec::new();
        let mut active_pois = active.pack.pois.iter().enumerate()
            .filter(|(_i, poi)| filter_mapid(poi.map_id, poi.category.as_ref()))
            .map(|(i, _)| i)
            .rev();
        if let Some(i) = active_pois.next() {
            pois.reserve_exact(i + 1);
            pois.resize(i, false);
            pois.push(true);
        }
        for i in active_pois {
            pois.set(i, true);
        }
        // TODO: use some sort of space-efficient encoding like RLE for these masks
        // even just an initial offset or vec of bit group lengths (pos/neg for 0 vs 1) would help?
        let mut trails = BitVec::new();
        let mut active_trails = active.pack.trails.iter().enumerate()
            .filter(|(_i, trail)| filter_mapid(trail.map_id.unwrap_or(0), trail.category.as_ref()))
            .map(|(i, _)| i)
            .rev();
        if let Some(i) = active_trails.next() {
            trails.reserve_exact(i + 1);
            trails.resize(i, false);
            trails.push(true);
        }
        for i in active_trails {
            trails.set(i, true);
        }

        let categories = categories.into_boxed_slice();

        Self {
            pois,
            trails,
            categories,
        }
    }

    pub async fn load_from_pack(pack: &mut LoadedPack, map_id: MapIndex, _manager: &PackLoader) -> anyhow::Result<Self> {
        // TODO...
        Ok(Self::with_pack(&*pack, map_id))
    }

    pub fn is_empty(&self) -> bool {
        (self.trails.is_empty() || self.trails[..].not_any())
            && (self.pois.is_empty() || self.pois[..].not_any())
    }

    /// None if ![self.is_empty()]
    pub fn get(self) -> Option<Self> {
        (!self.is_empty()).then_some(self)
    }

    pub fn poi_count(&self) -> usize {
        self.pois.count_ones()
    }
    pub fn pois(&self) -> impl Iterator<Item = PoiPath> + '_ {
        self.pois.iter_ones()
            .lazy_map(|i| PoiPath::with_path(i as PoiIndex))
    }
    pub fn loaded_pois(&self) -> impl Iterator<Item = (LoadedPoiPath, PoiPath)> + '_ {
        self.pois().enumerate()
            .map(|(i, path)| (LoadedPoiPath::with_path(i as PoiIndex), path))
    }
    #[cfg(todo)]
    pub(crate) fn poi_guid_mask(&self) -> impl Iterator<Item = bool> + '_ {
        self.poi_guid_mask.iter()
    }
    #[cfg(todo)]
    pub(crate) fn poi_guid_mask(&self) -> impl Iterator<Item = bool> + '_ {
        iter::repeat(true).take(self.poi_count())
    }
    #[cfg(todo)]
    pub(crate) fn poi_guid_filter<'a, I>(&'a self, iter: I) -> impl Iterator<Item = I::Item> + 'a where
        I: IntoIterator + 'a,
    {
        self.poi_guid_mask().zip(iter)
            .filter_map(|(mask, v)| mask.then_some(v))
    }
    pub fn poi_index(&self, path: PoiPath) -> Option<LoadedPoiPath> {
        match () {
            #[cfg(todo = "unnecessary")]
            _ => self.pois().position(|t| t.path == path.path)
                .map(|i| LoadedTrailPath::with_path(i as TrailIndex)),
            _ => match self.pois.get(path.path as usize) {
                None => None,
                Some(b) if !*b =>
                    None,
                Some(_) => Some(unsafe {
                    let index = path.path as usize;
                    let preceding = self.pois.get_unchecked(..path.path as usize);
                    LoadedPoiPath::with_path(preceding.count_ones() as PoiIndex)
                }),
            },
        }
    }
    /// TODO: `nth` isn't implemented on [bitvec::slice::IterOnes], it should
    /// probably popcnt instead?
    pub fn poi_path(&self, path: LoadedPoiPath) -> Option<PoiPath> {
        self.pois().nth(path.path as usize)
    }
    pub fn trail_count(&self) -> usize {
        self.trails.count_ones()
    }
    pub fn trails(&self) -> impl Iterator<Item = TrailPath> + '_ {
        self.trails.iter_ones()
            .lazy_map(|i| TrailPath::with_path(i as TrailIndex))
    }
    pub fn loaded_trails(&self) -> impl Iterator<Item = (LoadedTrailPath, TrailPath)> + '_ {
        self.trails().enumerate()
            .map(|(i, path)| (LoadedTrailPath::with_path(i as TrailIndex), path))
    }
    #[cfg(todo)]
    pub(crate) fn trail_guid_mask(&self) -> impl Iterator<Item = bool> + '_ {
        self.trail_guid_mask.iter()
    }
    #[cfg(todo)]
    pub(crate) fn trail_guid_mask(&self) -> impl Iterator<Item = bool> + '_ {
        iter::repeat(true).take(self.trail_count())
    }
    #[cfg(todo)]
    pub(crate) fn trail_guid_filter<'a, I>(&'a self, iter: I) -> impl Iterator<Item = I::Item> + 'a where
        I: IntoIterator + 'a,
    {
        self.trail_guid_mask().zip(iter)
            .filter_map(|(mask, v)| mask.then_some(v))
    }
    pub fn trail_index(&self, path: TrailPath) -> Option<LoadedTrailPath> {
        match () {
            #[cfg(todo = "unnecessary")]
            _ => self.trails().position(|t| t.path == path.path)
                .map(|i| LoadedTrailPath::with_path(i as TrailIndex)),
            _ => match self.trails.get(path.path as usize) {
                None => None,
                Some(b) if !*b =>
                    None,
                Some(_) => Some(unsafe {
                    let index = path.path as usize;
                    let preceding = self.trails.get_unchecked(..path.path as usize);
                    LoadedTrailPath::with_path(preceding.count_ones() as TrailIndex)
                }),
            },
        }
    }
    /// TODO: `nth` isn't implemented on [bitvec::slice::IterOnes], it should
    /// probably popcnt instead?
    pub fn trail_path(&self, path: LoadedTrailPath) -> Option<TrailPath> {
        self.trails().nth(path.path as usize)
    }
    pub fn category_count(&self) -> usize {
        self.categories.len()
    }
    pub fn category_max(&self) -> Option<CategoryIndex> {
        self.categories.iter().max().copied()
    }
    pub fn category_max_count(&self) -> CategoryIndex {
        self.category_max()
            .map(|c| c + 1)
            .unwrap_or(0)
    }
    pub fn categories(&self) -> impl Iterator<Item = CategoryPath> + '_ {
        self.categories.iter().copied().map(CategoryPath::with_path)
    }
    pub fn category_index(&self, path: CategoryPath) -> Option<CategoryIndex> {
        self.categories[..].iter().position(|&c| c == path.path)
            .map(|i| i as CategoryIndex)
    }

    pub fn path_from_loaded(&self, loaded: MarkerPath<PackMapPath>) -> Option<MarkerPath<PackPath>> {
        let pack_path = loaded.root.root;
        Some(match loaded.path.variant() {
            MarkerIndexVariant::Poi(index) => {
                let p = self.poi_path(LoadedPoiPath::with_path(index))?;
                #[cfg(todo = "unnecessary")]
                let index = MarkerIndex::with_poi(p.path);
                let index = p.into();
                MarkerPath::with_parts(pack_path, index)
            },
            MarkerIndexVariant::Trail(index) => {
                let p = self.trail_path(LoadedTrailPath::with_path(index))?;
                #[cfg(todo = "unnecessary")]
                let index = MarkerIndex::with_trail(p.path);
                let index = p.into();
                MarkerPath::with_parts(pack_path, index)
            },
            MarkerIndexVariant::TrailSection(index, section) => {
                let p = self.trail_path(LoadedTrailPath::with_path(index))?;
                let index = MarkerIndex::with_trail_section(p.path, section);
                #[cfg(todo)]
                let index = p.into();
                MarkerPath::with_parts(pack_path, index)
            },
            _ => return None,
        })
    }
}
