use taimi_hoard::loc::Locator;
use taimi_meta::packs::{MarkerIndexVariant, MarkerPath};

use crate::{controller::pathing::registry::LoadedPoiPath, exports::runtime::textures::TextureKey};

use {
    crate::{controller::{
        pathing::{
            registry::SharedLoaderBox, visible::{LoadedMapPack, LoadedTrailGeometry, LoadedTrail},
            PathingController, PathingEvent,
            registry::{LoadedTrailPath, LoadedMarkerPath, PackLoader},
            shared::{SharedGameplayMap, SharedLoaderPacksInfo, PathingShared, MapTrailInfo},
            visible::LoadedTrailSection,
        },
        Controller,
    }, exports::runtime as rt},
    anyhow::Context,
    futures::{future::Future, stream, FutureExt, StreamExt},
    std::collections::BTreeSet,
    std::sync::Arc,
    taimi_meta::packs::{id::MarkerId, PackMapPath, PoiPath, TrailPath},
    taimi_hoard::loc::{indexed::IndexedList, LocationRef, LocationMut},
    taimi_sync::watched::watch,
    taimi_pack::{
        attributes::{AttrString, TrailAttributes},
        trail::{TrlPath, TrailData},
    },
};
pub use self::{
    build::{SpacePoiBuilder, SpaceTrailBuilder, SpaceLoader, TrailGeometryRequests, TrailGeometryRequestsTx, TextureLoadRequests, TextureLoadRequestsTx},
    poi::PoiScale,
    trail::{TrailParams, TrailScale, TrailTextureMap},
    pack::{SpacePack, SpacePackCollection},
    shared::SpacePackShared,
};
pub use super::PackSpace as DrawSpace;

mod build;
mod pack;
mod poi;
mod shared;
mod trail;

type LoadedTrailSections = Arc<[LoadedTrailSection]>;
#[derive(Debug)]
pub enum LoadReport {
    TrailGeometry {
        path: Locator<PackMapPath, LoadedTrailPath>,
        geometry: anyhow::Result<LoadedTrailGeometry>,
        section_info: Option<LoadedTrailSections>,
    },
    Texture {
        path: LoadedMarkerPath<PackMapPath>,
        texture: anyhow::Result<TextureKey>,
    },
}

pub struct SpaceContext {
    pub packs: Arc<SpacePackCollection>,
    pub maps_rx: watch::Receiver<SharedGameplayMap>,
    pub trail_geometry: TrailGeometryRequests,
    pub texture_loads: TextureLoadRequests,
    pub inflight: BTreeSet<MarkerId>,
}
impl SpaceContext {
    pub fn subscribe_to(shared: &PathingShared) -> Self {
        let space = &shared.space;
        Self {
            trail_geometry: TrailGeometryRequests::subscribed_to(&space.trail_geometry),
            texture_loads: TextureLoadRequests::subscribed_to(&space.texture_loads),
            inflight: BTreeSet::new(),
            packs: space.collection.borrow().clone(),
            maps_rx: shared.gameplay.subscribe(),
        }
    }
    pub fn mark_inputs_dirty(&mut self) {
        self.maps_rx.mark_changed();
    }
    pub async fn recv_trail_requests(trail_geometry: &mut TrailGeometryRequests, inflight: &BTreeSet<MarkerId>) -> impl Iterator<Item = LoadedTrailPath<PackMapPath>> {
        let reqs = trail_geometry.recv_requests(|path| {
            let id = SpacePackShared::trail_geometry_id(path);
            !inflight.contains(&id)
        }).await;
        reqs.into_iter()
    }
    pub async fn recv_texture_requests(texture_loads: &mut TextureLoadRequests, inflight: &BTreeSet<MarkerId>) -> impl Iterator<Item = LoadedMarkerPath<PackMapPath>> {
        let reqs = texture_loads.recv_requests(|path| {
            let id = MarkerId::for_marker(*path);
            !inflight.contains(&id)
        }).await;
        reqs.into_iter()
    }

    pub(crate) fn report_load(&mut self, loaded: LoadReport) {
        match loaded {
            LoadReport::Texture { path, texture } => {
                let id = MarkerId::for_marker(path);
                self.inflight.remove(&id);
                let texture = rt::log::error_ok(texture);
                self.texture_loads.fill_request(path, texture);
            },
            LoadReport::TrailGeometry { path, geometry, section_info: _ } => {
                let lpath = path.root.rel(path.path.path);
                let id = SpacePackShared::trail_geometry_id(&lpath);
                self.inflight.remove(&id);
                let geometry = rt::log::error_ok(geometry)
                    .unwrap_or_else(LoadedTrailGeometry::empty);
                self.trail_geometry.fill_request(lpath, geometry);
            },
        }
    }
}

type TrlLoadContext = (TrlPath, f32, bool);
impl PathingController {
    pub(super) async fn report_load(&mut self, mut loaded: LoadReport) {
        match &mut loaded {
            LoadReport::TrailGeometry { path, section_info, .. } => {
                let info = section_info.take()
                    .and_then(|si| self.map_info.lookup_mut(&path.root)
                        .map(|map_info| (si, map_info))
                    );
                if let Some((section_info, map_info)) = info {
                    let dirty = map_info.info.trail_info.lookup_ref(&path.path).and_then(|ti| ti.sections.as_ref())
                        .map(|si| &si.data[..]) != Some(&section_info[..]);
                    if dirty {
                        Arc::make_mut(&mut map_info.info).update_trail_section_info(path.path, section_info);
                        // TODO: batch/delay these updates weh
                        self.loader.shared.update_map_info(path.root, &map_info.info, true);
                    }
                }
            },
            LoadReport::Texture { .. } => (),
        }
        self.space.report_load(loaded);
    }
    fn texture_for_loaded_marker(map: &LoadedMapPack, path: LoadedMarkerPath) -> Option<&AttrString> {
        match path.path.variant() {
            MarkerIndexVariant::Poi(poii) => {
                let lpath: LoadedPoiPath = LoadedPoiPath::with_path(poii);
                map.lpois().lookup_ref(&lpath)?.poi_attrs().icon_file.as_ref()
            },
            MarkerIndexVariant::Trail(traili) | MarkerIndexVariant::TrailSection(traili, _) => {
                let lpath: LoadedTrailPath = LoadedTrailPath::with_path(traili);
                map.ltrails().lookup_ref(&lpath)?.trail_attrs().texture.as_ref()
            },
            _ => None,
        }
    }
    fn trl_for_loaded_trail(map: &LoadedMapPack, path: LoadedTrailPath) -> Option<TrlLoadContext> {
        map.ltrails().lookup_ref(&path).and_then(|ltrail| {
            let trl = ltrail.info().trl.as_ref()?;
            let attrs = ltrail.trail_attrs();
            Some((trl.clone(), attrs.scale(), attrs.is_wall()))
        })
    }
    pub(super) fn request_texture_load(&mut self, id: MarkerId) {
        if !self.space.inflight.insert(id.clone()) {
            log::info!("duplicate load request for {id}");
            return
        }
        let path = id.marker_path::<PackMapPath>().map(|path| {
            let texture = self.maps.lookup_ref(&path.root).and_then(|map| Self::texture_for_loaded_marker(map, path.unscope()).cloned());
            (path, texture)
        });
        let Some((path, texture)) = path else {
            log::error!("invalid texture load request for {id}");
            self.space.inflight.remove(&id);
            return
        };
        let load = Self::new_task_texture_load(self.loader.clone(), path, texture);
        let _cancel = self.tasks.spawn(load);
    }
    pub(super) fn request_trail_load(&mut self, id: MarkerId) {
        if !self.space.inflight.insert(id.clone()) {
            log::info!("duplicate load request for {id}");
            return
        }
        let path = id.marker_path::<PackMapPath>().and_then(|path| match path.path.variant() {
            MarkerIndexVariant::Trail(traili) | MarkerIndexVariant::TrailSection(traili, _) => {
                let lpath: LoadedTrailPath = LoadedTrailPath::with_path(traili);
                let trl = self.maps.lookup_ref(&path.root).and_then(|map|
                    Self::trl_for_loaded_trail(map, lpath)
                );
                Some((path.root.rel(lpath), trl))
            },
            _ => None,
        });
        let Some((path, trl)) = path else {
            log::error!("invalid trail load request for {id}");
            self.space.inflight.remove(&id);
            return
        };
        let load = Self::new_task_trail_load(self.loader.clone(), path, trl);
        let _cancel = self.tasks.spawn(load);
    }

    fn new_task_trail_load(manager: Arc<PackLoader>, path: Locator<PackMapPath, LoadedTrailPath>, trl: Option<TrlLoadContext>) -> impl Future<Output = anyhow::Result<PathingEvent>> + Send + 'static {
        Self::task_trail_load(manager, path, trl)
    }
    async fn task_trail_load(manager: Arc<PackLoader>, path: Locator<PackMapPath, LoadedTrailPath>, trl: Option<TrlLoadContext>) -> anyhow::Result<PathingEvent> {
        let loader = Self::pack_loader_for(&manager, path.root.root).await?;
        let geometry = async move {
            let trl = trl.context("TODO: late-load trl context")?;
            Self::trail_load_geometry(&manager, path, loader, trl).await
        }.await;
        let (geometry, section_info) = match geometry {
            Ok((geometry, section_info)) => (Ok(geometry), Some(section_info)),
            Err(e) => (Err(e), None),
        };
        let response = LoadReport::TrailGeometry {
            path,
            geometry,
            section_info,
        };
        Ok(PathingEvent::ReportLoaded(response))
    }
    async fn trail_load_geometry(manager: &PackLoader, path: Locator<PackMapPath, LoadedTrailPath>, loader: SharedLoaderBox, ctx: TrlLoadContext) -> anyhow::Result<(LoadedTrailGeometry, LoadedTrailSections)> {
        let (trl_path, scale, is_wall) = ctx;
        let trl = Self::load_trail_data(loader, trl_path).await?;
        let y_sig = (path.root.root.path as usize) << 24 | path.path.path as usize;
        let params = manager.trail_params().await;
        let section_info = LoadedTrailSection::with_sections(&trl.sections).collect();
        Self::load_trail_geometry(trl, (scale, is_wall), params, y_sig).await
            .map(move |geo| (geo, section_info))
    }
    async fn load_trail_data(loader: SharedLoaderBox, path: TrlPath) -> anyhow::Result<TrailData> {
        let mut loader = loader.lock_owned().await;
        Controller::try_run_blocking("reading trl", move || {
            path.read_trl_data(&mut *loader)
        }).await
    }
    fn load_trail_geometry(trl: TrailData, (scale, is_wall): (f32, bool), params: TrailParams, y_sig: usize) -> impl Future<Output = anyhow::Result<LoadedTrailGeometry>> + Send + 'static {
        let y_offset = params.y_offset_for(y_sig);
        Controller::try_run_blocking("calculating vertices", move || {
            Ok(LoadedTrail::vertices_with_data(&trl, &params, scale, is_wall, y_offset))
        })
    }

    fn new_task_texture_load(manager: Arc<PackLoader>, path: LoadedMarkerPath<PackMapPath>, texture: Option<AttrString>) -> impl Future<Output = anyhow::Result<PathingEvent>> + Send + 'static {
        Self::task_texture_load(manager, path, texture)
    }
    async fn task_texture_load(manager: Arc<PackLoader>, path: LoadedMarkerPath<PackMapPath>, texture: Option<AttrString>) -> anyhow::Result<PathingEvent> {
        let loader = Self::pack_loader_for(&manager, path.root.root).await?;
        let info = manager.shared.packs.packs.borrow().lookup_ref(&path.root.root)
            .context("pack unrecognized")?
            .info.clone();
        let tex = texture
            .with_context(|| format!("TODO: late-load texture path for {path}"))?;
        let mut loader = loader.lock_owned().await;
        let texture = Controller::try_run_blocking("texture resource load", move || {
            let texture = SpaceLoader::register_texture(&info, &tex);
            let (key, _slot) = SpaceLoader::get_or_load_texture(&mut *loader, texture);
            Ok(key)
        }).await;
        #[cfg(todo = "unnecessary")]
        if texture.is_err() {
            crate::TEXTURES.report_failure(info.subresource_key(&tex));
        }
        let response = LoadReport::Texture {
            path,
            texture,
        };
        Ok(PathingEvent::ReportLoaded(response))
    }

    #[cfg(todo)]
    pub async fn setup_pack(&mut self,
        path: PackMapPath,
        setup_trails: Vec<SetupTrail>,
        setup_pois: Option<Vec<SetupPoi>>,
    ) -> anyhow::Result<()> {
        let active_weak = {
            let pack_data = self.loader.shared.packs.data.borrow();
            SharedPacks::pack_at(&pack_data, path.root).cloned()
        };
        let Some(active_weak) = active_weak else {
            anyhow::bail!("cannot setup for unloaded {path}")
        };
        let setup_map_id = path.path;
        let setup = move |state: &mut RenderState| -> anyhow::Result<()> {
            let Some(Ok(engine)) = &mut state.engine else { return Ok(()) };
            let map_id = {
                log::debug!("TODO: check map_id before render setup");
                Some(setup_map_id)
            };
            match map_id {
                None => (),
                Some(map_id) if map_id == setup_map_id => (),
                map_id => {
                    log::info!("prepared pack {path} for map#{setup_map_id}, but now {map_id:?}");
                    return Ok(())
                },
            }
            // TODO: sanity check that we still want to load this?
            let spacepack = engine.packs.pack_mut(&path.root);
            let needs_rebuild = spacepack.render_list_bookmark.is_some();
            spacepack.clear();

            let loader = {
                let Some(active) = Weak::upgrade(&active_weak) else {
                    anyhow::bail!("can't prepare pack {path} if it's not loaded")
                };
                drop(active_weak);
                active.loader.clone()
            };
            let mut loader = loader.blocking_lock();
            let mut loader = SpaceLoader {
                active_pack: spacepack,
                loader: &mut *loader,
                device: &engine.render_backend.device,
            };
            // XXX: this whole PackCollection type needs a rework, so collect to vec for now...
            let pois = setup_pois.map(|p| p.into_iter().map(|(path, setup, poi)| {
                let poi = setup.map(|setup| setup.build(path, &mut loader, &poi));
                match poi {
                    Some(Ok(poi)) => poi,
                    Some(Err(e)) => {
                        log::warn!("Preparing PoI#{path}: {e:#}");
                        SpacePoiBuilder::build_empty()
                    },
                    None =>
                        SpacePoiBuilder::build_empty(),
                }
            }).collect::<Vec<_>>());
            let trails = setup_trails.into_iter().map(|(path, setup, trail)| {
                let trail = setup.map(|setup| setup.build(path, &mut loader, &trail));
                if let Some(Err(e)) = &trail {
                    log::warn!("Preparing trail#{path}: {e:#}");
                }
                match trail {
                    Some(Ok(trail)) => trail,
                    _ =>
                        SpaceTrailBuilder::build_empty(),
                }
            }).collect::<Vec<_>>();
            if needs_rebuild {
                if let Some(pois) = pois {
                    spacepack.active_pois = pois;
                }
                spacepack.active_trails = trails;
                engine.packs.rebuild_active(&engine.render_backend.device)
            } else {
                engine.packs.load_pack(&engine.render_backend.device, path.root.path, pois.unwrap_or_default(), trails)
            }
        };
        ctx.spawn_render(RenderTaskPriority::Normal, move |state| {
            let res = setup(state);
            rt::log::error_ok(res);
        });
        Ok(())
    }

    #[cfg(todo)]
    pub async fn prepare_pack(&mut self, ctx: &mut PathingEventContext, path: PackMapPath) {
        let Some(info) = self.map_pack_info.get(&path) else { return };
        let Some(map) = self.map_packs.get(&path) else { return };
        let Some(active) = Self::packs().read().await.lookup_ref(&path.root).and_then(|p| p.active.clone()) else { return };

        let pois = map.pois(info).map(move |(poi_path, poi)| {
            let setup = active.pack.pois.get(poi_path.path as usize)
                .and_then(SpacePoiBuilder::from_pack);
            (poi_path, setup, poi.clone())
        });

        let pois = pois.collect();
        self.prepare_trails(ctx, path, Some(pois)).await
    }

    #[cfg(todo)]
    pub async fn prepare_trails(&mut self, ctx: &mut PathingEventContext, path: PackMapPath, pois: Option<Vec<SetupPoi>>) {
        let Some(info) = self.map_pack_info.get(&path) else { return };
        let Some(map) = self.map_packs.get(&path) else { return };
        let Some(active) = Self::packs().read().await.lookup_ref(&path.root).and_then(|p| p.active.clone()) else { return };
        let params = self.trail_params().await;

        let trails = map.trails(info).map(move |(trail_path, trail)|
            SpaceTrailBuilder::load_from_pack(trail_path, active.clone(), trail.clone(), params.clone())
                .map(move |(setup, trail, updated)|
                    (trail_path, setup, trail, updated)
                )
        );

        Self::prepare_trails_spawn(ctx, path, trails, pois)
    }

    const LOAD_TRAIL_PARALLEL: usize = 12;
    #[cfg(todo)]
    pub fn prepare_trails_spawn<F, T>(ctx: &mut PathingEventContext, path: PackMapPath, trails: T, pois: Option<Vec<SetupPoi>>) where
        F: Future<Output = (TrailPath, anyhow::Result<SpaceTrailBuilder>, SpaceTrail, bool)> + Send + 'static,
        T: IntoIterator<Item = F>,
    {
        let trails: Vec<_> = trails.into_iter().collect();
        ctx.tasks.spawn(async move {
            let trails = stream::iter(trails).buffered(Self::LOAD_TRAIL_PARALLEL);
            tokio::pin!(trails);
            let mut out = Vec::new();
            let mut map_updates = Vec::new();
            while let Some((trail_path, setup, trail, changed)) = trails.next().await {
                if changed {
                    map_updates.push((trail_path, trail.clone()));
                }
                let setup = rt::log::warn_ok(setup);
                out.push((trail_path, setup, trail));
            }
            let map_updates = (!map_updates.is_empty()).then_some(PathingEvent::UpdateMapTrails {
                path,
                updates: map_updates,
            });
            // TODO: actually submit these to renderer incrementally, don't wait for full vec!
            let trails = (!out.is_empty() || pois.is_some()).then_some(PathingEvent::SetupTrails {
                path,
                trails: out,
                pois,
            });
            Some(PathingEvent::FanOut(
                map_updates.into_iter().chain(trails)
                .collect()
            ))
        });
    }
}

pub type SetupPoi = (PoiPath, Option<SpacePoiBuilder>);
pub type SetupTrail = (TrailPath, Option<SpaceTrailBuilder>);
