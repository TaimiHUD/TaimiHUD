use {
    crate::controller::pathing::{
        registry::{LoadedPoiIndex, LoadedPoiNs, LoadedTrailIndex, LoadedTrailNs, PackInfoSignature},
        info::MapPackInfo,
        state::{LoadedCategory, LoadedPoi, LoadedTrail},
    },
    std::sync::Arc,
    taimi_hoard::{collections::lru::RecentlyUsed, loc::indexed::IndexedList},
    taimi_meta::packs::{CategoryPath, MapIndex, PoiPath, TrailPath},
    taimi_pack::Pack,
};

#[cfg(todo)]
use crate::controller::pathing::{
    filter::MapFilters,
    state::interactive::InteractivePoi,
    taimi_pack::attributes::keys::Guid,
};

#[derive(Debug, Clone)]
pub struct LoadedMapPack {
    pub map_id: MapIndex,
    pub info_sig: PackInfoSignature,
    pub used: RecentlyUsed,
    pub pois: Box<[LoadedPoi]>,
    #[cfg(todo)]
    pub poi_guids: Arc<[Guid]>,
    #[cfg(todo)]
    pub interactive_pois: Arc<[InteractivePoi]>,
    #[cfg(todo)]
    pub interactive_pois_nearby: BitVec,
    pub trails: Box<[LoadedTrail]>,
    #[cfg(todo)]
    pub trail_guids: Box<[Guid]>,
    pub categories: Arc<[LoadedCategory]>,
    #[cfg(todo)]
    pub filters: MapFilters,
}

impl LoadedMapPack {
    pub fn empty(map_id: MapIndex) -> Self {
        Self {
            map_id,
            info_sig: PackInfoSignature::EMPTY,
            used: RecentlyUsed::DEFAULT,
            #[cfg(todo)]
            interactive_pois: Default::default(),
            #[cfg(todo)]
            interactive_pois_nearby: Default::default(),
            pois: Default::default(),
            #[cfg(todo)]
            poi_guids: Default::default(),
            trails: Default::default(),
            #[cfg(todo)]
            trail_guids: Default::default(),
            categories: Default::default(),
            #[cfg(todo)]
            filters: Default::default(),
        }
    }

    pub fn from_pack(map_id: MapIndex, info: &MapPackInfo, pack: &Pack) -> Self {
        let pois = info.pois().map(|path| LoadedPoi::from_pack(path, pack)).collect();
        #[cfg(todo)]
        let poi_guids = info
            .poi_guid_filter(info.pois())
            .map(|path| {
                pack.pois
                    .get(path.path as usize)
                    .map(|poi| Guid::from(poi.guid))
                    .unwrap_or_default()
            })
            .collect();
        #[cfg(todo)]
        let interactive_pois = info
            .pois()
            .enumerate()
            .map(|(i, path)| InteractivePoi::from_pack(i as PoiIndex, path, pack))
            .filter(|ipoi| !ipoi.is_empty())
            .collect();
        let trails = info
            .trails()
            .map(|path| LoadedTrail::from_pack(path, pack))
            .collect();
        #[cfg(todo)]
        let trail_guids = info
            .trail_guid_filter(info.trails())
            .map(|path| {
                pack.trails
                    .get(path.path as usize)
                    .map(|trail| Guid::from(trail.guid))
                    .unwrap_or_default()
            })
            .collect();
        #[cfg(todo)]
        let filters = MapFilters::from_pack(info, active);

        let loaded = Self {
            map_id,
            info_sig: info.info_sig.clone(),
            #[cfg(todo)]
            interactive_pois_nearby: BitVec::new(),
            #[cfg(todo)]
            interactive_pois,
            pois,
            #[cfg(todo)]
            poi_guids,
            trails,
            #[cfg(todo)]
            trail_guids,
            #[cfg(todo)]
            filters,
            categories: Default::default(),
            used: RecentlyUsed::DEFAULT,
        };
        #[cfg(todo)]
        {
            loaded
                .interactive_pois_nearby
                .reserve_exact(loaded.interactive_pois.len());
        }

        loaded
    }

    pub fn lpois(&self) -> &IndexedList<LoadedPoiNs, LoadedPoiIndex, [LoadedPoi]> {
        IndexedList::from_ref(&self.pois[..])
    }
    pub fn lpois_mut(&mut self) -> &mut IndexedList<LoadedPoiNs, LoadedPoiIndex, [LoadedPoi]> {
        IndexedList::from_mut(&mut self.pois[..])
    }
    pub fn pois<'a, 'i>(
        &'a self,
        info: &'i MapPackInfo,
    ) -> impl Iterator<Item = (PoiPath, &'a LoadedPoi)> + 'i
    where
        'a: 'i,
    {
        info.pois().zip(self.pois.iter())
    }
    pub fn pois_mut<'a, 'i>(
        &'a mut self,
        info: &'i MapPackInfo,
    ) -> impl Iterator<Item = (PoiPath, &'a mut LoadedPoi)> + 'i
    where
        'a: 'i,
    {
        info.pois().zip(self.pois.iter_mut())
    }
    #[cfg(todo)]
    pub fn poi_guids<'a, 'i>(
        &'a self,
        info: &'i MapPackInfo,
    ) -> impl Iterator<Item = (PoiPath, &'a Guid)> + 'i
    where
        'a: 'i,
    {
        info.poi_guid_filter(info.pois()).zip(self.poi_guids.iter())
    }
    pub fn poi_at<'a>(&'a self, path: PoiPath<&'_ MapPackInfo>) -> Option<&'a LoadedPoi> {
        let info = path.root;
        info.poi_index(path.unscope())
            .and_then(|i| self.pois.get(i.path as usize))
    }
    pub fn poi_at_mut<'a>(&'a mut self, path: PoiPath<&'_ MapPackInfo>) -> Option<&'a mut LoadedPoi> {
        let info = path.root;
        info.poi_index(path.unscope())
            .and_then(|i| self.pois.get_mut(i.path as usize))
    }

    pub fn ltrails(&self) -> &IndexedList<LoadedTrailNs, LoadedTrailIndex, [LoadedTrail]> {
        IndexedList::from_ref(&self.trails[..])
    }
    pub fn ltrails_mut(&mut self) -> &mut IndexedList<LoadedTrailNs, LoadedTrailIndex, [LoadedTrail]> {
        IndexedList::from_mut(&mut self.trails[..])
    }
    pub fn trails<'a, 'i>(
        &'a self,
        info: &'i MapPackInfo,
    ) -> impl Iterator<Item = (TrailPath, &'a LoadedTrail)> + 'i
    where
        'a: 'i,
    {
        info.trails().zip(self.trails.iter())
    }
    pub fn trails_mut<'a, 'i>(
        &'a mut self,
        info: &'i MapPackInfo,
    ) -> impl Iterator<Item = (TrailPath, &'a mut LoadedTrail)> + 'i
    where
        'a: 'i,
    {
        info.trails().zip(self.trails.iter_mut())
    }
    #[cfg(todo)]
    pub fn trail_guids<'a, 'i>(
        &'a self,
        info: &'i MapPackInfo,
    ) -> impl Iterator<Item = (TrailPath, &'a Guid)> + 'i
    where
        'a: 'i,
    {
        info.trail_guid_filter(info.trails()).zip(&self.trail_guids)
    }
    pub fn trail_at<'a>(&'a self, path: TrailPath<&'_ MapPackInfo>) -> Option<&'a LoadedTrail> {
        let info = path.root;
        info.trail_index(path.unscope())
            .and_then(|i| self.trails.get(i.path as usize))
    }
    pub fn trail_at_mut<'a>(&'a mut self, path: TrailPath<&'_ MapPackInfo>) -> Option<&'a mut LoadedTrail> {
        let info = path.root;
        info.trail_index(path.unscope())
            .and_then(|i| self.trails.get_mut(i.path as usize))
    }

    pub fn categories<'a, 'i>(
        &'a self,
        info: &'i MapPackInfo,
    ) -> impl Iterator<Item = (CategoryPath, &'a LoadedCategory)> + 'i
    where
        'a: 'i,
    {
        info.categories().zip(self.categories.iter())
    }
    pub fn categories_mut<'a, 'i>(
        &'a mut self,
        info: &'i MapPackInfo,
    ) -> impl Iterator<Item = (CategoryPath, &'a mut LoadedCategory)> + 'i
    where
        'a: 'i,
    {
        let categories = Arc::make_mut(&mut self.categories);
        info.categories().zip(categories.iter_mut())
    }
    pub fn category_at<'a>(&'a self, path: CategoryPath<&'_ MapPackInfo>) -> Option<&'a LoadedCategory> {
        let info = path.root;
        info.category_index(path.unscope())
            .and_then(|i| self.categories.get(i.path as usize))
    }
    pub fn category_at_mut<'a>(
        &'a mut self,
        path: CategoryPath<&'_ MapPackInfo>,
    ) -> Option<&'a mut LoadedCategory> {
        let info = path.root;
        info.category_index(path.unscope())
            .and_then(|i| Arc::make_mut(&mut self.categories).get_mut(i.path as usize))
    }
}
