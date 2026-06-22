use {
    crate::controller::pathing::{
        info::MapPackInfo,
        registry::{
            LoadedMarkerPath,
            LoadedPoiIndex,
            LoadedPoiNs,
            LoadedPoiPath,
            LoadedTrailIndex,
            LoadedTrailNs,
            LoadedTrailPath,
            PackInfoSignature,
        },
        state::{LoadedCategory, LoadedPoi, LoadedTrail},
    },
    std::sync::Arc,
    taimi_hoard::{collections::lru::RecentlyUsed, iters::IterExt as _, loc::indexed::IndexedList},
    taimi_meta::packs::{id::MarkerPath, CategoryPath, MapIndex, PoiPath, TrailPath},
    taimi_pack::{attributes::keys::Guid, Pack},
};

#[derive(Debug, Clone)]
pub struct LoadedMapPack {
    pub map_id: MapIndex,
    pub info_sig: PackInfoSignature,
    pub used: RecentlyUsed,
    pub pois: Box<[LoadedPoi]>,
    pub poi_guids: Arc<[Guid]>,
    pub trails: Box<[LoadedTrail]>,
    pub trail_guids: Arc<[Guid]>,
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
            pois: Default::default(),
            poi_guids: Default::default(),
            trails: Default::default(),
            trail_guids: Default::default(),
            categories: Default::default(),
        }
    }

    pub fn from_pack(map_id: MapIndex, info: &MapPackInfo, pack: &Pack) -> Self {
        let pois = info.pois().map(|path| LoadedPoi::from_pack(path, pack)).collect();
        let poi_guids = info
            .poi_guid_filter(info.pois())
            .map(|path| {
                pack.pois
                    .get(path.path as usize)
                    .map(|poi| Guid::from(poi.guid))
                    .unwrap_or_default()
            })
            .collect();
        let trails = info
            .trails()
            .map(|path| LoadedTrail::from_pack(path, pack))
            .collect();
        let trail_guids = info
            .trail_guid_filter(info.trails())
            .map(|path| {
                pack.trails
                    .get(path.path as usize)
                    .map(|trail| Guid::from(trail.guid))
                    .unwrap_or_default()
            })
            .collect();

        let loaded = Self {
            map_id,
            info_sig: info.info_sig.clone(),
            pois,
            poi_guids,
            trails,
            trail_guids,
            #[cfg(todo)]
            filters: MapFilters::from_pack(info, active),
            categories: Default::default(),
            used: RecentlyUsed::DEFAULT,
        };

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
    pub fn enum_pois_mut<'a, 'i>(
        &'a mut self,
        info: &'i MapPackInfo,
    ) -> impl Iterator<Item = (LoadedPoiPath, PoiPath, &'a mut LoadedPoi)> + 'i
    where
        'a: 'i,
    {
        self.lpois_mut()
            .iter_mut()
            .zip(info.pois())
            .lazy_map(|((lp, l), p)| (lp, p, l))
    }
    pub fn poi_guids<'a, 'i>(
        &'a self,
        info: &'i MapPackInfo,
    ) -> impl Iterator<Item = (PoiPath, LoadedPoiPath, &'a Guid)> + 'i
    where
        'a: 'i,
    {
        let pois = info.pois().zip(self.lpois().paths());
        info.poi_guid_filter(pois)
            .zip(self.poi_guids.iter())
            .lazy_map(|((p, lp), g)| (p, lp, g))
    }
    pub fn poi_guid_by_index<'a>(&'a self, info: &'_ MapPackInfo, path: LoadedPoiPath) -> Option<&'a Guid> {
        match info
            .poi_guid_filter(self.lpois().paths())
            .enumerate()
            .find(|(_, p)| *p >= path)
        {
            Some((i, p)) if p == path => self.poi_guids.get(i),
            _ => None,
        }
    }
    pub fn poi_guid_by_path<'a>(&'a self, info: &'_ MapPackInfo, path: PoiPath) -> Option<&'a Guid> {
        match info
            .poi_guid_filter(info.pois())
            .enumerate()
            .find(|(_, p)| *p >= path)
        {
            Some((i, p)) if p == path => self.poi_guids.get(i),
            _ => None,
        }
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
    pub fn enum_trails_mut<'a, 'i>(
        &'a mut self,
        info: &'i MapPackInfo,
    ) -> impl Iterator<Item = (LoadedTrailPath, TrailPath, &'a mut LoadedTrail)> + 'i
    where
        'a: 'i,
    {
        self.ltrails_mut()
            .iter_mut()
            .zip(info.trails())
            .lazy_map(|((lp, l), p)| (lp, p, l))
    }
    pub fn trail_guids<'a, 'i>(
        &'a self,
        info: &'i MapPackInfo,
    ) -> impl Iterator<Item = (TrailPath, LoadedTrailPath, &'a Guid)> + 'i
    where
        'a: 'i,
    {
        let trails = info.trails().zip(self.ltrails().paths());
        info.trail_guid_filter(trails)
            .zip(self.trail_guids.iter())
            .lazy_map(|((p, lp), g)| (p, lp, g))
    }
    pub fn marker_guids<'a, 'i>(
        &'a self,
        info: &'i MapPackInfo,
    ) -> impl Iterator<Item = (MarkerPath, LoadedMarkerPath, &'a Guid)> + 'i
    where
        'a: 'i,
    {
        let pois = self.poi_guids(info).lazy_map(|(p, lp, g)| {
            let mp: MarkerPath = p.pivot_from();
            let mlp: LoadedMarkerPath = lp.pivot_to();
            (mp, mlp, g)
        });
        let trails = self.trail_guids(info).lazy_map(|(p, lp, g)| {
            let mp: MarkerPath = p.pivot_from();
            let mlp: LoadedMarkerPath = lp.pivot_to();
            (mp, mlp, g)
        });
        pois.chain(trails)
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
