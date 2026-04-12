use {
    crate::{
        controller::{
            pathing::{
                registry::{LoadedMarkerPath, LoadedPoiPath, LoadedTrailPath, PackLoader, SharedLoaderBox},
                shared::{
                    LoadReport,
                    LocDisplay,
                    PathingShared,
                    SharedGameplayMap,
                    SharedPackInfo,
                    TrailGeometrySections,
                },
                state::{
                    LoadedMapInfo,
                    LoadedMapPack,
                    LoadedMaps,
                    LoadedTrail,
                    LoadedTrailGeometry,
                    LoadedTrailSection,
                },
                PathingController,
                PathingEvent,
            },
            Controller,
        },
        exports::runtime::{
            self as rt,
            textures::{TextureKey, TextureSlot},
        },
        resources::shader,
        space::{pack::render, engine::{SpaceEvent, Engine}},
        TEXTURES,
    },
    anyhow::{anyhow, Context},
    futures::future::{Either, Future},
    glam::Vec3,
    std::{
        collections::{btree_map, BTreeMap, BTreeSet},
        sync::{Arc, Mutex},
    },
    taimi_d3d::shader::{ShaderKind, ID3DInclude},
    taimi_hoard::loc::{LocationMut, LocationRef, Locator},
    taimi_meta::packs::{
        id::{MarkerId, MarkerIndexVariant},
        MapIndex,
        PackMapPath,
        PackPath,
    },
    taimi_pack::{
        attributes::{
            keys::{self, GetAttr},
            AttrString,
        },
        loader::PackLoaderContext,
        trail::{TrailData, TrlPath},
    },
    taimi_sync::watched::watch,
    tokio::task::AbortHandle,
};

#[doc(inline)]
#[allow(unused_imports)]
pub use self::{
    pack::{SpaceEntities, SpacePack, SpacePackCollection},
    poi::PoiScale,
    trail::{TrailParams, TrailScale, TrailTextureMap},
};
#[doc(no_inline)]
pub use super::PackSpace as DrawSpace;
pub use crate::controller::pathing::shared::{SpacePackShared, TextureLoadRequests, TrailGeometryRequests};

mod pack;
mod poi;
mod trail;

pub struct SpaceContext {
    pub packs: Arc<SpacePackCollection>,
    pub maps_rx: watch::Receiver<SharedGameplayMap>,
    pub trail_geometry: TrailGeometryRequests,
    pub texture_loads: TextureLoadRequests,
    pub inflight: BTreeSet<MarkerId>,
    inflight_resources: InflightResources,
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
            inflight_resources: Default::default(),
        }
    }
    pub fn mark_inputs_dirty(&mut self) {
        self.maps_rx.mark_changed();
    }
    pub async fn recv_trail_requests(
        trail_geometry: &mut TrailGeometryRequests,
        inflight: &BTreeSet<MarkerId>,
    ) -> impl Iterator<Item = LoadedTrailPath<PackMapPath>> {
        let reqs = trail_geometry
            .recv_requests(|path| {
                let id = SpacePackShared::trail_geometry_id(path);
                !inflight.contains(&id)
            })
            .await;
        reqs.into_iter()
    }
    pub async fn recv_texture_requests(
        texture_loads: &mut TextureLoadRequests,
        inflight: &BTreeSet<MarkerId>,
    ) -> impl Iterator<Item = LoadedMarkerPath<PackMapPath>> {
        let reqs = texture_loads
            .recv_requests(|path| {
                let id = MarkerId::for_marker(*path);
                !inflight.contains(&id)
            })
            .await;
        reqs.into_iter()
    }

    pub(crate) fn report_load(&mut self, loaded: LoadReport) {
        match loaded {
            LoadReport::Texture { path, texture, resource: _ } => {
                let id = MarkerId::for_marker(path);
                self.inflight.remove(&id);
                let texture = rt::log::warn_ok(texture);
                self.texture_loads.fill_request(path, texture);
            },
            LoadReport::TrailGeometry { path, geometry, section_info: _ } => {
                let lpath = path.root.rel(path.path.path);
                let id = SpacePackShared::trail_geometry_id(&lpath);
                self.inflight.remove(&id);
                let geometry = rt::log::warn_ok(geometry).unwrap_or_else(LoadedTrailGeometry::empty);
                self.trail_geometry.fill_request(lpath, geometry);
            },
        }
    }

    pub(super) fn collect_garbage(
        &mut self,
        (map_info, maps): (&LoadedMapInfo, &LoadedMaps),
        map_id: Option<MapIndex>,
        _aggressive: bool,
    ) {
        let packs_map_id = self.packs.map_id;
        let map_dirty = packs_map_id != map_id;

        let mut dirty = map_dirty;
        if map_dirty {
            Arc::make_mut(&mut self.packs).clear();
        } else if let Some(map_id) = map_id {
            let expired = match self.packs.expired_entities_dirty(map_id, map_info, maps) {
                Ok(expired) if expired.is_empty() => None,
                Ok(e) => Some(Some(e)),
                Err(()) => Some(None),
            };
            if let Some(expired) = expired {
                dirty = true;
                let packs = Arc::make_mut(&mut self.packs);
                match expired {
                    Some(expired) => {
                        // XXX: may not be necessary if rx is dirty immediately after anyway...
                        packs.invalidate_entities(&mut expired.into_iter())
                    },
                    None => packs.clear(),
                }
            }
        }
        if dirty {
            // XXX: may not be necessary if changes are published immediately after anyway...
            self.maps_rx.mark_changed();
        }
    }
}

type TrlLoadContext = (TrlPath, f32, bool, Vec3);
impl PathingController {
    pub(super) async fn report_load(&mut self, mut loaded: LoadReport) {
        match &mut loaded {
            LoadReport::TrailGeometry { path, section_info, .. } => {
                let info = section_info.take().and_then(|si| {
                    self.map_info
                        .lookup_mut(&path.root)
                        .map(|map_info| (si, map_info))
                });
                if let Some((section_info, map_info)) = info {
                    let dirty = map_info
                        .info
                        .trail_info
                        .lookup_ref(&path.path)
                        .and_then(|ti| ti.sections.as_ref())
                        .map(|si| &si.data[..])
                        != Some(&section_info[..]);
                    if dirty {
                        Arc::make_mut(&mut map_info.info)
                            .update_trail_section_info(path.path, section_info);
                        // TODO: batch/delay these updates weh
                        self.loader
                            .shared
                            .update_map_info(path.root, &map_info.info, true);
                    }
                }
            },
            LoadReport::Texture { .. } => (),
        }
        self.space.report_load(loaded);
    }
    fn texture_for_loaded_marker(
        map: &LoadedMapPack,
        path: LoadedMarkerPath,
        pack_path: PackPath,
    ) -> Option<&AttrString> {
        let tex = match path.path.variant() {
            MarkerIndexVariant::Poi(poii) => {
                let lpath: LoadedPoiPath = LoadedPoiPath::with_path(poii);
                let lpoi = map.lpois().lookup_ref(&lpath);
                if lpoi.is_none() {
                    log::debug!("BUG? tex req for missing {lpath} on {}", path.root);
                }
                lpoi?.poi_attrs().icon_file.as_ref()
            },
            MarkerIndexVariant::Trail(traili) | MarkerIndexVariant::TrailSection(traili, _) => {
                let lpath: LoadedTrailPath = LoadedTrailPath::with_path(traili);
                map.ltrails().lookup_ref(&lpath)?.trail_attrs().texture.as_ref()
            },
            _ => None,
        };
        if !crate::built_info::IS_TAGGED_VERSION && tex.is_none() {
            let path = LocDisplay(pack_path.rel(map.map_id).rel(path));
            log::debug!("WHY? no texture found on {path}");
        }
        tex
    }
    fn trl_for_loaded_trail(map: &LoadedMapPack, path: LoadedTrailPath) -> Option<TrlLoadContext> {
        map.ltrails().lookup_ref(&path).and_then(|ltrail| {
            let trl = ltrail.info().trl.as_ref()?;
            let attrs = ltrail.trail_attrs();
            Some((
                trl.clone(),
                GetAttr::<keys::TrailScale>::get_attr_or_default(attrs)
                    .into_owned()
                    .into(),
                GetAttr::<keys::IsWall>::get_attr_or_default(attrs)
                    .into_owned()
                    .into(),
                Vec4::from(GetAttr::<keys::Tint>::get_attr_or_default(attrs).into_owned()).truncate(),
            ))
        })
    }
    pub(super) fn request_texture_load(&mut self, id: MarkerId) -> bool {
        if !self.space.inflight.insert(id.clone()) {
            log::debug!("duplicate load request for {id}");
            return false
        }
        let path = id.marker_path::<PackMapPath>().map(|path| {
            let texture = self
                .maps
                .lookup_ref(&path.root)
                .map(|map| Self::texture_for_loaded_marker(map, path.unscope(), path.root.root).cloned());
            (path, texture)
        });
        let Some((path, texture)) = path else {
            log::error!("invalid texture load request for {id}");
            self.space.inflight.remove(&id);
            return false
        };
        if texture.is_none() {
            log::error!("maps missing {path} but tex requested???");
        }
        let texture = texture.flatten();
        let resources = self.space.inflight_resources.clone();
        let acq_loader = {
            let r = texture.as_ref().map(|texture| (texture, resources.lock()));
            if let Some((tex, Ok(mut resources))) = r {
                InflightResource::acquire_loader(
                    &mut resources,
                    id,
                    (path.root.root, RequestKind::Texture),
                    tex,
                )
            } else {
                true
            }
        };
        if acq_loader {
            let load =
                Self::new_task_texture_load(self.loader.clone(), resources, path, id, texture.clone());
            let resources = texture.map(|t| (t, self.space.inflight_resources.lock()));
            let cancel = self.tasks.spawn(load);
            if let Some((texture, Ok(mut resources))) = resources {
                let resource = resources
                    .entry((path.root.root, RequestKind::Texture, texture))
                    .or_default();
                let _ = resource.loader.get_or_insert(cancel);
            }
        }
        acq_loader
    }
    pub(super) fn request_trail_load(&mut self, id: MarkerId) {
        if !self.space.inflight.insert(id.clone()) {
            log::info!("duplicate load request for {id}");
            return
        }
        let path = id
            .marker_path::<PackMapPath>()
            .and_then(|path| match path.path.variant() {
                MarkerIndexVariant::Trail(traili) | MarkerIndexVariant::TrailSection(traili, _) => {
                    let lpath: LoadedTrailPath = LoadedTrailPath::with_path(traili);
                    let trl = self
                        .maps
                        .lookup_ref(&path.root)
                        .and_then(|map| Self::trl_for_loaded_trail(map, lpath));
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

    fn new_task_trail_load(
        manager: Arc<PackLoader>,
        path: Locator<PackMapPath, LoadedTrailPath>,
        trl: Option<TrlLoadContext>,
    ) -> impl Future<Output = anyhow::Result<PathingEvent>> + Send + 'static {
        Self::task_trail_load(manager, path, trl)
    }
    async fn task_trail_load(
        manager: Arc<PackLoader>,
        path: Locator<PackMapPath, LoadedTrailPath>,
        trl: Option<TrlLoadContext>,
    ) -> anyhow::Result<PathingEvent> {
        let loader = manager.pack_loader_for(path.root.root).await?;
        let geometry = async move {
            let trl = trl.with_context(|| {
                // TODO? late-load trl path from attrs
                let lpath = LocDisplay::from_ref(&path);
                anyhow!("missing trail path for {lpath}")
            })?;
            Self::trail_load_geometry(&manager, path, loader, trl).await
        }
        .await;
        let (geometry, section_info) = match geometry {
            Ok((geometry, section_info)) => (Ok(geometry), Some(section_info)),
            Err(e) => (Err(e), None),
        };
        let response = LoadReport::TrailGeometry { path, geometry, section_info };
        Ok(PathingEvent::ReportResourceLoaded(response))
    }
    async fn trail_load_geometry(
        manager: &PackLoader,
        path: Locator<PackMapPath, LoadedTrailPath>,
        loader: SharedLoaderBox,
        ctx: TrlLoadContext,
    ) -> anyhow::Result<(LoadedTrailGeometry, TrailGeometrySections)> {
        let (trl_path, scale, is_wall, colour) = ctx;
        let trl = Self::load_trail_data(loader, trl_path).await?;
        let y_sig = (path.root.root.path as usize) << 24 | path.path.path as usize;
        let params = manager.trail_params().await;
        let section_info = LoadedTrailSection::with_sections(&trl.sections).collect();
        Self::load_trail_geometry(trl, (scale, is_wall, colour), params, y_sig)
            .await
            .map(move |geo| (geo, section_info))
    }
    async fn load_trail_data(loader: SharedLoaderBox, path: TrlPath) -> anyhow::Result<TrailData> {
        let mut loader = loader.lock_owned().await;
        Controller::try_run_blocking("reading trl", move || path.read_trl_data(&mut *loader)).await
    }
    fn load_trail_geometry(
        trl: TrailData,
        (scale, is_wall, colour): (f32, bool, Vec3),
        params: TrailParams,
        y_sig: usize,
    ) -> impl Future<Output = anyhow::Result<LoadedTrailGeometry>> + Send + 'static {
        let y_offset = params.y_offset_for(y_sig);
        Controller::try_run_blocking("calculating vertices", move || {
            Ok(LoadedTrail::vertices_with_data(
                &trl,
                &params,
                scale,
                is_wall,
                y_offset,
                colour.into(),
            ))
        })
    }

    fn new_task_texture_load(
        manager: Arc<PackLoader>,
        resources: InflightResources,
        path: LoadedMarkerPath<PackMapPath>,
        id: MarkerId,
        texture: Option<AttrString>,
    ) -> impl Future<Output = anyhow::Result<PathingEvent>> + Send + 'static {
        async move {
            let pack_path = path.root.root;
            let res = Self::task_texture_load(manager, path, texture).await;
            match res {
                Ok(PathingEvent::ReportResourceLoaded(LoadReport::Texture {
                    path,
                    texture,
                    resource: Some(tex),
                })) => {
                    let mut r = resources.lock().ok();
                    let resource = r
                        .as_mut()
                        .and_then(|r| r.remove(&(pack_path, RequestKind::Texture, tex.clone())))
                        .map(|r| (r.inflight.len() != 1 || !r.inflight.contains(&id)).then_some(r))
                        .flatten();
                    let res = if let Some(resource) = resource {
                        let res = resource
                            .inflight
                            .iter()
                            .filter_map(|id| id.marker_path::<PackMapPath>())
                            .map(|path| {
                                let texture = match &texture {
                                    Ok(t) => Ok(t.clone()),
                                    Err(e) => Err(rt::log::anyhow_clone(e)),
                                };
                                LoadReport::Texture {
                                    path,
                                    texture,
                                    resource: Some(tex.clone()),
                                }
                            })
                            .map(PathingEvent::ReportResourceLoaded);
                        PathingEvent::FanOut(res.collect())
                    } else {
                        PathingEvent::ReportResourceLoaded(LoadReport::Texture {
                            path,
                            texture,
                            resource: Some(tex),
                        })
                    };
                    Ok(res)
                },
                res => res,
            }
        }
    }
    async fn task_texture_load(
        manager: Arc<PackLoader>,
        path: LoadedMarkerPath<PackMapPath>,
        texture: Option<AttrString>,
    ) -> anyhow::Result<PathingEvent> {
        let loader = manager.pack_loader_for(path.root.root).await?;
        let info = manager
            .shared
            .packs
            .packs
            .borrow()
            .lookup_ref(&path.root.root)
            .context("pack unrecognized")?
            .info
            .clone();
        let tex = match texture {
            Some(tex) => tex,
            None => {
                // we could check now but there are currently no meaningful scenarios where a loaded marker
                // would be missing its attrs at request time, so...
                // XXX: if we do late-load eventually, make sure to
                let texture = match () {
                    #[cfg(todo)]
                    _ => anyhow::bail!("TODO? late-load texture path for {path}"),
                    _ => {
                        let lpath = LocDisplay::from_ref(&path);
                        Err(anyhow!("missing texture attribute for {lpath}"))
                    },
                };
                let response = LoadReport::Texture { path, texture, resource: None };
                return Ok(PathingEvent::ReportResourceLoaded(response))
            },
        };
        let mut loader = loader.lock_owned().await;
        let texture = Controller::try_run_blocking("texture resource load", {
            let tex = tex.clone();
            move || {
                let texture = Self::register_shared_texture(&info, &tex);
                let (key, _slot) = Self::get_or_load_shared_texture(&mut *loader, texture);
                Ok(key)
            }
        })
        .await;
        #[cfg(todo = "unnecessary")]
        if texture.is_err() {
            TEXTURES.report_failure(info.subresource_key(&tex));
        }
        let response = LoadReport::Texture { path, texture, resource: Some(tex) };
        Ok(PathingEvent::ReportResourceLoaded(response))
    }

    pub(super) fn register_shared_texture(
        info: &SharedPackInfo,
        texture: &AttrString,
    ) -> SpaceTextureHandle {
        let key = info.key_for_subresource(texture);
        let tex = TEXTURES
            .lookup_pair_with(&key, |canon, tex| {
                canon.map(|canon| {
                    let tex = (!tex.is_loading() && !tex.can_load()).then_some(tex.clone());
                    (canon.clone(), tex)
                })
            })
            .flatten();
        let (key, tex) = match tex {
            Some((key, tex)) => (key, tex),
            None => (key.into(), None),
        };
        let tex = tex.map(Ok).unwrap_or_else(|| Err(texture.clone()));
        (key, tex)
    }
    pub(super) fn get_or_load_shared_texture(
        loader: &mut dyn PackLoaderContext,
        (mut key, tex): SpaceTextureHandle,
    ) -> (TextureKey, Option<TextureSlot>) {
        let name = match tex {
            Ok(TextureSlot::Loading) => return (key, None),
            Ok(slot) => return (key, Some(slot)),
            Err(name) => {
                // maybe pointless idk
                let _ = TEXTURES.try_canonicalize_key_mut(&mut key);
                name
            },
        };

        let data = if let Some(path) = loader.asset_absolute_path(&name[..]) {
            Some(path)
        } else {
            None
        };
        let data = match data {
            None => {
                let res = loader
                    .load_asset_dyn(&name[..])
                    .and_then(|mut asset| {
                        let mut bytes = Vec::new();
                        asset
                            .read_to_end(&mut bytes)
                            .map(move |_amt| bytes)
                            .map_err(Into::into)
                    })
                    .with_context(|| format!("reading texture {}", name));
                match res {
                    Ok(bytes) => Either::Left(bytes),
                    Err(e) => {
                        log::error!("{e:#}");
                        TEXTURES.report_failure(key.clone());
                        return (key, Some(TextureSlot::Unavailable))
                    },
                }
            },
            Some(path) => Either::Right(path),
        };

        match data {
            Either::Left(bytes) => {
                crate::texture_schedule_bytes(key.clone(), bytes);
            },
            Either::Right(path) => {
                crate::texture_schedule_file(key.clone(), path);
            },
        }
        (key, None)
    }

    pub async fn space_pack_rebuild_if_needed(&mut self) {
        if !self.space.packs.needs_bvh_rebuild() {
            return
        }
        let packs = {
            if Arc::strong_count(&self.space.packs) > 1 {
                self.space.packs = Arc::new(self.space.packs.clone_without_bvh());
            }
            Arc::make_mut(&mut self.space.packs)
        };
        packs.rebuild_bvh().await;
        Self::space_publish_packs(&self.loader, Some(&self.space.packs), Some(true));
    }

    /// map changed in a way that may be relevant to [SpacePackCollection] state
    pub async fn space_pack_updates(&mut self) {
        let map_id = self.gameplay_map();
        let (space_dirty, is_empty) = if let Some(map_id) = map_id {
            #[cfg(todo)]
            let entities_dirty = self.space.packs.needs_rebuild(map_id, &self.packs);
            let entities_dirty = true;
            let space_dirty = if entities_dirty {
                let space_packs = Arc::make_mut(&mut self.space.packs);
                let still_loading = self.loader.shared.packs.read_still_waiting().0;
                let bvh_dirty =
                    space_packs.rebuild_entities(map_id, &self.packs, &self.map_info, &self.maps, still_loading);
                match bvh_dirty {
                    Err(true) => {
                        if !still_loading {
                            space_packs.try_rebuild_bvh().await;
                        }
                        true
                    },
                    Err(false) => true,
                    Ok(()) => false,
                }
            } else {
                //self.space.packs.needs_bvh_rebuild()
                false
            };
            let is_empty = self.space.packs.is_empty();
            (space_dirty, is_empty)
        } else {
            let changed = match self.space.packs.map_id {
                None => false,
                #[cfg(todo)]
                Some(..) => {
                    self.space.packs = Arc::new(space::SpacePackCollection::new());
                    //Arc::make_mut(&mut self.space.packs).clear();
                    true
                },
                Some(..) => true,
            };
            (changed, true)
        };
        if space_dirty || is_empty {
            let packs = (!is_empty).then_some(&self.space.packs);
            Self::space_publish_packs(&self.loader, packs, None);
        }
    }
    fn space_publish_packs(
        loader: &PackLoader,
        packs: Option<&Arc<SpacePackCollection>>,
        notify: Option<bool>,
    ) -> bool {
        let new_copy = packs.cloned();
        let mut dirty = false;
        loader.shared.space.collection.send_if_modified(|shared| {
            if let Some(new_copy) = new_copy {
                *shared = new_copy;
                dirty = true;
            } else if !shared.is_empty() {
                Arc::make_mut(shared).clear();
                dirty = true;
            }
            notify.unwrap_or(dirty)
        });
        dirty
    }
    pub async fn debug_req_space_build(&mut self, entities: Option<bool>, bvh: Option<bool>) {
        let Some(map_id) = self.gameplay_map() else {
            log::warn!("mapless");
            return
        };
        let space_packs = Arc::make_mut(&mut self.space.packs);
        let bvh_dirty = {
            if let Some(true) = entities {
                space_packs.clear();
            }
            match entities {
                Some(false) => None,
                _ => {
                    log::info!("space entity rebuild...");
                    Some(space_packs.rebuild_entities(map_id, &self.packs, &self.map_info, &self.maps, false))
                },
            }
        };
        let _dirty = match (bvh, bvh_dirty) {
            (Some(true), _) | (None, Some(Err(true))) => {
                log::info!("space bvh rebuild...");
                space_packs.rebuild_bvh().await;
                true
            },
            (Some(false), _) => {
                log::warn!("clearing bvh, have fun!");
                space_packs.clear_bvh();
                true
            },
            (None, Some(Err(false))) => true,
            (None, Some(Ok(()))) => false,
            (None, None) => true,
        };
        log::info!("space updated");
        Self::space_publish_packs(&self.loader, Some(&self.space.packs), Some(true));
        log::info!("space shared");
    }
    pub(super) fn load_shader(&mut self, kind: ShaderKind, variant: render::ArcShaderVariant, entity: Option<render::ShaderState>, mut template: shader::ShaderDescription) {
        let manager = self.loader.clone();
        self.tasks.spawn(async move {
            let context = format!("compiling shader {variant:?}/{kind:?} from template {}", template.identifier);
            let id = variant.id(kind, entity)
                .with_context(|| format!("id missing when {context}???"))?;
            let permit = manager.load_throttle().acquire_owned().await;
            Controller::try_run_blocking(context, move || {
                template.defs.extend(variant.defines(kind, entity));
                template.defs.terminate();
                let dir = shader::ShaderDirectory::new();
                let includes = ID3DInclude::new(&dir);
                let source = dir.get_file_contents(&template.path)?;
                let bytecode = template.compile(&source, Some(&*includes))?;
                Engine::try_send(SpaceEvent::ProcessShader(id, kind, bytecode, template.identifier));
                drop(permit);
                Ok(PathingEvent::Nop)
            }).await
        });
    }
}
type SpaceTextureHandle = (TextureKey, Result<TextureSlot, AttrString>);
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum RequestKind {
    TrlData,
    Texture,
}
type InflightResourcesMap = BTreeMap<(PackPath, RequestKind, AttrString), InflightResource>;
type InflightResources = Arc<Mutex<InflightResourcesMap>>;
#[derive(Debug, Default)]
struct InflightResource {
    loader: Option<AbortHandle>,
    inflight: BTreeSet<MarkerId>,
}
impl InflightResource {
    fn acquire_loader(
        resources: &mut InflightResourcesMap,
        id: MarkerId,
        (path, kind): (PackPath, RequestKind),
        key: &AttrString,
    ) -> bool {
        #[cfg(todo = "unnecessary")]
        if loader.is_none() {
            return true
        }
        #[cfg(todo)]
        if let Some(resource) = resources.get_mut(&(path, kind, key)) {
            resource.inflight.insert(id);
            return false
        }
        let resource = resources.entry((path, kind, key.clone()));
        let acq = matches!(resource, btree_map::Entry::Vacant(..));
        let resource = resource.or_default();
        resource.inflight.insert(id);
        acq
    }
}
