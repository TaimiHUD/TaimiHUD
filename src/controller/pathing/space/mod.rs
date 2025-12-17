use {
    crate::{controller::pathing::{
        PathingController, PathingEvent,
    }, exports::runtime as rt},
    futures::{future::Future, stream, FutureExt, StreamExt}, std::sync::{Arc, LazyLock},
    self::build::SpaceLoader,
    taimi_meta::packs::{PackMapPath, PoiPath, TrailPath},
    taimi_hoard::loc::LocationRef,
};
pub use self::{
    build::{SpacePoiBuilder, SpaceTrailBuilder},
    poi::{SpacePoi, PoiScale},
    trail::{SpaceTrail, TrailParams, TrailScale, TrailTextureMap},
    pack::SpacePack,
};
pub use super::PackSpace as DrawSpace;

mod build;
mod pack;
mod poi;
mod trail;

impl PathingController {
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

pub type SetupPoi = (PoiPath, Option<SpacePoiBuilder>, SpacePoi);
pub type SetupTrail = (TrailPath, Option<SpaceTrailBuilder>, SpaceTrail);
static EMPTY_RENDER_ATTRS: LazyLock<Arc<taimi_pack::attributes::RenderAttributes>> = LazyLock::new(||
    Arc::new(Default::default())
);
