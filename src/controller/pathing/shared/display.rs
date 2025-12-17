use {
    crate::controller::{
        pathing::{
            registry::{
                LoadedCategoryIndex,
                LoadedCategoryPath,
                LoadedMarkerPath,
                LoadedPoiPath,
                LoadedTrailPath,
                PackIndex,
                PackMapPath,
                PackPath,
            },
            shared::{SharedMapPackLoaded, SharedMapPackState, SharedPackInfo, SharedPackLoad},
        },
        Controller,
    },
    std::{fmt, mem, sync::Arc},
    taimi_hoard::{
        lazyfmt::{fmt_or, MaybeFmt},
        loc::{LocationRef, Locator},
    },
    taimi_meta::packs::{
        id::{MarkerId, MarkerIndexVariant, MarkerPath},
        CategoryIndex,
        CategoryPath,
        MapIndex,
        MapPath,
        PoiPath,
        TrailPath,
        TrailSectionPath,
    },
    taimi_pack::Pack,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct LocDisplay<L>(pub L);
impl<L> LocDisplay<L> {
    #[inline]
    pub const fn from_ref(l: &L) -> &Self {
        unsafe { mem::transmute(l) }
    }
    #[inline]
    pub fn from_mut(l: &mut L) -> &mut Self {
        unsafe { mem::transmute(l) }
    }
    pub fn rel<P>(self, path: P) -> Locator<Self, P> {
        Locator::with_parts(self, path)
    }
    pub fn rel_ref<P>(&self, path: P) -> Locator<&Self, P> {
        Locator::with_parts(self, path)
    }
}
impl<N, L> LocDisplay<Locator<N, L>> {
    pub fn root_ref(&self) -> &LocDisplay<N> {
        LocDisplay::from_ref(&self.0.root)
    }
    pub fn path_ref(&self) -> &LocDisplay<L> {
        LocDisplay::from_ref(&self.0.path)
    }
}
impl<L> fmt::Display for LocDisplay<&'_ L>
where
    LocDisplay<L>: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(LocDisplay::from_ref(self.0), f)
    }
}
fn with_shared_pack<R, F: FnOnce(&SharedPackLoad) -> R>(path: PackPath, f: F) -> Option<R> {
    if path.path == PackIndex::MAX {
        return None
    }
    let shared = Controller::with_sender(|s| s.pathing.as_ref().map(|p| p.shared.clone())).flatten()?;
    let packs = shared.packs.packs.borrow();
    let pack = packs.lookup_ref(&path)?;
    Some(f(pack))
}
fn with_shared_pack_data<R, F: FnOnce(&SharedPackLoad, Option<Arc<Pack>>) -> R>(
    path: PackPath,
    f: F,
) -> Option<R> {
    if path.path == PackIndex::MAX {
        return None
    }
    let shared = Controller::with_sender(|s| s.pathing.as_ref().map(|p| p.shared.clone())).flatten()?;
    let loaded = shared.packs.packs.borrow().lookup_ref(&path)?.loaded.clone();
    let loaded = loaded.borrow().pack.clone();
    let packs = shared.packs.packs.borrow();
    let pack = unsafe { packs.get_unchecked(path.path as usize) };
    Some(f(pack, loaded))
}
#[cfg(todo = "unused")]
fn with_shared_map<R, F: FnOnce(&SharedMapPackLoaded, Option<&SharedMapPackState>) -> R>(
    path: PackMapPath,
    f: F,
) -> Option<R> {
    if path.root.path == PackIndex::MAX || path.path == MapIndex::MAX {
        return None
    }
    let shared = Controller::with_sender(|s| s.pathing.as_ref().map(|p| p.shared.clone())).flatten()?;
    let gameplay = shared.gameplay.borrow();
    let (map_path, map_info) = gameplay.get_info_for(path.root)?;
    if map_path.path != path.path {
        // map not loaded
        return None
    }
    let map = gameplay.get_state(path);
    Some(f(map_info, map))
}
fn with_shared_pack_map<
    R,
    F: FnOnce(&SharedPackInfo, Option<&Pack>, &SharedMapPackLoaded, Option<&SharedMapPackState>) -> R,
>(
    path: PackMapPath,
    f: F,
) -> Option<R> {
    if path.root.path == PackIndex::MAX || path.path == MapIndex::MAX {
        return None
    }
    let shared = Controller::with_sender(|s| s.pathing.as_ref().map(|p| p.shared.clone())).flatten()?;
    let (info, loaded) = {
        let packs = shared.packs.packs.borrow();
        let pack = packs.lookup_ref(&path.root)?;
        (pack.info.clone(), pack.loaded.clone())
    };
    let pack = loaded.borrow().pack.clone();
    let gameplay = shared.gameplay.borrow();
    let (map_path, map_info) = gameplay.get_info_for(path.root)?;
    if map_path.path != path.path {
        // map not loaded
        return None
    }
    let map = gameplay.get_state(path);
    Some(f(&info, pack.as_ref().map(|p| &**p), map_info, map))
}
impl fmt::Display for LocDisplay<(Option<&'_ SharedPackInfo>, PackPath)> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let &Self((info, path)) = self;
        let datasource = info.and_then(|i| i.datasource.as_ref());
        if let Some(datasource) = datasource {
            return fmt::Display::fmt(&datasource.path, f)
        }
        let file = info.and_then(|info| match info.path.as_os_str().is_empty() {
            true => None,
            false => info.path.file_stem(),
        });
        if let Some(file) = file {
            return fmt::Display::fmt(&file.display(), f)
        }

        if path.path == PackIndex::MAX {
            fmt::Display::fmt(&path.swap(-1i32), f)
        } else {
            fmt::Display::fmt(&path, f)
        }
    }
}
impl fmt::Display for LocDisplay<PackPath> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.0.path == PackIndex::MAX {
            None
        } else {
            with_shared_pack(self.0, |load| {
                fmt::Display::fmt(&LocDisplay((Some(&*load.info), self.0)), f)
            })
        }
        .unwrap_or_else(|| fmt::Display::fmt(&LocDisplay((None::<&SharedPackInfo>, self.0)), f))
    }
}
impl fmt::Display for LocDisplay<(Option<&'_ SharedPackInfo>, PackMapPath)> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let &Self((info, path)) = self;
        let map: MapPath = Locator::with_path(path.path);
        let root = LocDisplay((info, path.root));
        fmt::Display::fmt(&Locator::with_parts(root, map), f)
    }
}
impl fmt::Display for LocDisplay<PackMapPath> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // TODO: lookup if gameplay map info exists on map or not
        let map: MapPath = Locator::with_path(self.0.path);
        let path = self.root_ref().rel_ref(LocDisplay(map));
        fmt::Display::fmt(&path, f)
    }
}
impl fmt::Display for LocDisplay<CategoryPath> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.0.path == CategoryIndex::MAX {
            fmt::Display::fmt(&self.0.swap(-1i32), f)
        } else {
            fmt::Display::fmt(&self.0, f)
        }
    }
}
impl fmt::Display for LocDisplay<LoadedCategoryPath> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.0.path == LoadedCategoryIndex::MAX {
            fmt::Display::fmt(&self.0.swap(-1i32), f)
        } else {
            fmt::Display::fmt(&self.0, f)
        }
    }
}
impl fmt::Display for LocDisplay<MapPath> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.0.path == MapIndex::MAX {
            fmt::Display::fmt(&self.0.swap(-1i32), f)
        } else {
            fmt::Display::fmt(&self.0, f)
        }
    }
}
impl fmt::Display for LocDisplay<MarkerId> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(path) = self.0.marker_path::<PackMapPath>() {
            fmt::Display::fmt(&LocDisplay(path), f)
        } else if let Some(path) = self.0.marker_path::<PackPath>() {
            fmt::Display::fmt(&LocDisplay(path), f)
        } else {
            fmt::Display::fmt(&self.0, f)
        }
    }
}
impl fmt::Display for LocDisplay<(Option<&'_ SharedPackInfo>, Option<&'_ Pack>, CategoryPath)> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let &Self((info, data, path)) = self;
        if let Some(info) = info.and_then(|i| i.info.as_ref()) {
            if let Some(root) = info.roots.iter().find(|r| r.path() == path) {
                return fmt::Display::fmt(&root.id, f)
            }
        }
        let cat = data.and_then(|data| data.categories.all_categories.get_index(path.path as usize));
        if let Some((_id, cat)) = cat {
            fmt::Display::fmt(&cat.full_id, f)
        } else {
            fmt::Display::fmt(&LocDisplay(&path), f)
        }
    }
}
impl fmt::Display for LocDisplay<CategoryPath<PackPath>> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let path: CategoryPath = Locator::with_path(self.0.path);
        fmt::Display::fmt(&LocDisplay(Locator::with_parts(self.0.root, path)), f)
    }
}
impl fmt::Display
    for LocDisplay<(
        Option<&'_ SharedPackInfo>,
        Option<&'_ Pack>,
        Locator<PackPath, CategoryPath>,
    )>
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let &Self((info, data, path)) = self;
        let root = LocDisplay((info, path.root));
        let path = LocDisplay((info, data, path.path));
        fmt::Display::fmt(&Locator::with_parts(root, path), f)
    }
}
impl fmt::Display for LocDisplay<Locator<PackPath, CategoryPath>> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.0.path.path == CategoryIndex::MAX {
            None
        } else {
            with_shared_pack_data(self.0.root, |load, data| {
                fmt::Display::fmt(
                    &LocDisplay((Some(&*load.info), data.as_ref().map(|pd| &**pd), self.0)),
                    f,
                )
            })
        }
        .unwrap_or_else(|| {
            fmt::Display::fmt(&LocDisplay((None::<&SharedPackInfo>, None::<&Pack>, self.0)), f)
        })
    }
}
impl fmt::Display
    for LocDisplay<(
        Option<&'_ SharedPackInfo>,
        Option<&'_ Pack>,
        Option<&'_ SharedMapPackLoaded>,
        Locator<PackMapPath, LoadedCategoryPath>,
    )>
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let &Self((info, pack, map_info, path)) = self;
        let root = LocDisplay((info, path.root));
        let cat = match path.path.path {
            LoadedCategoryIndex::MAX => None,
            _ => map_info.and_then(|mi| mi.category_path(path.path)),
        };
        match cat {
            Some(cat_path) => {
                let path = LocDisplay((info, pack, cat_path));
                fmt::Display::fmt(&Locator::with_parts(root, path), f)
            },
            None => {
                let path = LocDisplay(path.path);
                fmt::Display::fmt(&Locator::with_parts(root, path), f)
            },
        }
    }
}
impl fmt::Display for LocDisplay<Locator<PackMapPath, LoadedCategoryPath>> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        with_shared_pack_map(self.0.root, |info, pack, map_info, _map| {
            fmt::Display::fmt(&LocDisplay((Some(info), pack, Some(map_info), self.0)), f)
        })
        .unwrap_or_else(|| {
            fmt::Display::fmt(
                &LocDisplay((
                    None::<&SharedPackInfo>,
                    None::<&Pack>,
                    None::<&SharedMapPackLoaded>,
                    self.0,
                )),
                f,
            )
        })
    }
}
impl fmt::Display for LocDisplay<Locator<PackMapPath, LoadedPoiPath>> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let lpath = &self.0.path;
        with_shared_pack_map(self.0.root, |info, pack, map_info, _map| {
            let root = LocDisplay((Some(info), self.0.root));
            if let Some(lpoi) = map_info.pois().lookup_ref(&self.0.path) {
                let cat = LocDisplay((Some(info), pack, lpoi.category_path));
                let root = Locator::with_parts(root, cat);
                let poi_path = map_info.poi_path(self.0.path).map(Locator::into_path);
                let lpath = lpath.map_path(|lpoii| {
                    fmt_or(
                        poi_path.map(move |poii| MaybeFmt::new(move |f| write!(f, "{lpoii}={poii}"))),
                        lpoii,
                    )
                });
                fmt::Display::fmt(&root.rel(lpath), f)
            } else {
                fmt::Display::fmt(&root.rel_ref(lpath), f)
            }
        })
        .unwrap_or_else(|| {
            let root = LocDisplay((None::<&SharedPackInfo>, self.0.root));
            fmt::Display::fmt(&root.rel_ref(lpath), f)
        })
    }
}
impl fmt::Display for LocDisplay<Locator<PackMapPath, LoadedTrailPath>> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let lpath = &self.0.path;
        with_shared_pack_map(self.0.root, |info, pack, map_info, _map| {
            let root = LocDisplay((Some(info), self.0.root));
            if let Some(ltrail) = map_info.trails().lookup_ref(&self.0.path) {
                let cat = LocDisplay((Some(info), pack, ltrail.category_path));
                let trail_path = map_info.trail_path(self.0.path).map(Locator::into_path);
                let lpath = lpath.map_path(|ltraili| {
                    fmt_or(
                        trail_path
                            .map(move |traili| MaybeFmt::new(move |f| write!(f, "{ltraili}={traili}"))),
                        ltraili,
                    )
                });
                let lpath = Locator::with_parts(root, cat).rel(lpath);
                match &ltrail.trl {
                    Some(trl) => fmt::Display::fmt(&lpath.rel(&trl.path[..]), f),
                    None => fmt::Display::fmt(&lpath, f),
                }
            } else {
                fmt::Display::fmt(&root.rel_ref(lpath), f)
            }
        })
        .unwrap_or_else(|| {
            let root = LocDisplay((None::<&SharedPackInfo>, self.0.root));
            fmt::Display::fmt(&root.rel_ref(lpath), f)
        })
    }
}
impl fmt::Display for LocDisplay<LoadedMarkerPath<PackMapPath>> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let path: LoadedMarkerPath = LoadedMarkerPath::with_path(self.0.path);
        fmt::Display::fmt(&LocDisplay(self.0.root.rel(path)), f)
    }
}
impl fmt::Display for LocDisplay<Locator<PackMapPath, LoadedMarkerPath>> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0.path.path.variant() {
            MarkerIndexVariant::Category(cati) => {
                let lcat: LoadedCategoryPath = LoadedCategoryPath::with_path(cati);
                fmt::Display::fmt(&LocDisplay(self.0.root.rel(lcat)), f)
            },
            MarkerIndexVariant::Poi(poii) => {
                let lpoi: LoadedPoiPath = LoadedPoiPath::with_path(poii);
                fmt::Display::fmt(&LocDisplay(self.0.root.rel(lpoi)), f)
            },
            MarkerIndexVariant::Trail(traili) => {
                let ltrail: LoadedTrailPath = LoadedTrailPath::with_path(traili);
                fmt::Display::fmt(&LocDisplay(self.0.root.rel(ltrail)), f)
            },
            MarkerIndexVariant::TrailSection(traili, sectioni) => {
                let ltrail: LoadedTrailPath = LoadedTrailPath::with_path(traili);
                let section: TrailSectionPath = TrailSectionPath::with_path(sectioni);
                fmt::Display::fmt(&LocDisplay(self.0.root.rel(ltrail)).rel(section), f)
            },
            _ => fmt::Display::fmt(&self.root_ref().rel_ref(self.0.path), f),
        }
    }
}
impl fmt::Display for LocDisplay<Locator<PackPath, PoiPath>> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let lpath = &self.0.path;
        with_shared_pack_data(self.0.root, |load, pack| {
            let root = LocDisplay((Some(&*load.info), self.0.root));
            let poi = pack
                .as_ref()
                .and_then(|pack| pack.pois.get(self.0.path.path as usize).map(|poi| (pack, poi)))
                .map(|(pack, poi)| {
                    (
                        pack,
                        poi,
                        pack.categories.all_categories.get_full(poi.category.as_id()),
                    )
                });
            match poi {
                #[cfg(todo = "unnecessary")]
                Some((pack, poi, Some((cati, _id, cat)))) => {
                    let cat_path: CategoryPath = CategoryPath::with_path(cati);
                    let cat = LocDisplay((Some(&load), cat_path));
                    fmt::Display::fmt(&Locator::with_parts(root, cat).rel(lpath), f)
                },
                Some((_pack, _poi, Some((_cati, _id, cat)))) =>
                    fmt::Display::fmt(&Locator::with_parts(root, cat.full_id.as_str()).rel(lpath), f),
                _ => fmt::Display::fmt(&root.rel_ref(&self.0.path), f),
            }
        })
        .unwrap_or_else(|| {
            let root = LocDisplay((None::<&SharedPackInfo>, self.0.root));
            fmt::Display::fmt(&root.rel_ref(lpath), f)
        })
    }
}
impl fmt::Display for LocDisplay<Locator<PackPath, TrailPath>> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let lpath = &self.0.path;
        with_shared_pack_data(self.0.root, |load, pack| {
            let root = LocDisplay((Some(&*load.info), self.0.root));
            let trail = pack
                .as_ref()
                .and_then(|pack| {
                    pack.trails
                        .get(self.0.path.path as usize)
                        .map(|trail| (pack, trail))
                })
                .map(|(pack, trail)| {
                    (
                        pack,
                        trail,
                        trail.trail_path().ok(),
                        pack.categories.all_categories.get_full(trail.category.as_id()),
                    )
                });
            match trail {
                #[cfg(todo = "unnecessary")]
                Some((pack, trail, Some((cati, _id, cat)))) => {
                    let cat_path: CategoryPath = CategoryPath::with_path(cati);
                    let cat = LocDisplay((Some(&*load.info), cat_path));
                    fmt::Display::fmt(&Locator::with_parts(root, cat).rel(lpath), f)
                },
                Some((_pack, _trail, trl, Some((_cati, _id, cat)))) => {
                    let path = Locator::with_parts(root, cat.full_id.as_str()).rel(lpath);
                    match trl {
                        Some(trl) => fmt::Display::fmt(&path.rel(&trl.path[..]), f),
                        None => fmt::Display::fmt(&path.rel(&path), f),
                    }
                },
                _ => fmt::Display::fmt(&root.rel_ref(&self.0.path), f),
            }
        })
        .unwrap_or_else(|| {
            let root = LocDisplay((None::<&SharedPackInfo>, self.0.root));
            fmt::Display::fmt(&root.rel_ref(lpath), f)
        })
    }
}
impl fmt::Display for LocDisplay<MarkerPath<PackPath>> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let path: MarkerPath = MarkerPath::with_path(self.0.path);
        fmt::Display::fmt(&LocDisplay(self.0.root.rel(path)), f)
    }
}
impl fmt::Display for LocDisplay<Locator<PackPath, MarkerPath>> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0.path.path.variant() {
            MarkerIndexVariant::Category(cati) => {
                let cat: CategoryPath = CategoryPath::with_path(cati);
                fmt::Display::fmt(&LocDisplay(self.0.root.rel(cat)), f)
            },
            MarkerIndexVariant::Poi(poii) => {
                let poi: PoiPath = PoiPath::with_path(poii);
                fmt::Display::fmt(&LocDisplay(self.0.root.rel(poi)), f)
            },
            MarkerIndexVariant::Trail(traili) => {
                let trail: TrailPath = TrailPath::with_path(traili);
                fmt::Display::fmt(&LocDisplay(self.0.root.rel(trail)), f)
            },
            MarkerIndexVariant::TrailSection(traili, sectioni) => {
                let trail: TrailPath = TrailPath::with_path(traili);
                let section: TrailSectionPath = TrailSectionPath::with_path(sectioni);
                fmt::Display::fmt(&LocDisplay(self.0.root.rel(trail)).rel(section), f)
            },
            _ => fmt::Display::fmt(&self.root_ref().rel_ref(self.0.path), f),
        }
    }
}
