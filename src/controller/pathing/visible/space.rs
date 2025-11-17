use anyhow::Context;
use std::sync::Arc;
use crate::controller::pathing::registry::{PoiPath, TrailPath, ActivePack};
use crate::space::pack::{self as spacepack, trail::TrailParams};
use super::{LoadedPoi, LoadedTrail, LoadedTrailGeometry};
use taimi_pack::{attributes::keys, Poi as PackPoi};

pub struct SpaceLoader<'a> {
    pub active_pack: &'a mut spacepack::ActivePack,
    pub loader: &'a mut dyn taimi_pack::loader::PackLoaderContext,
    pub device: &'a taimi_d3d::dx11::Dx11Device,
}

#[derive(Debug, Clone)]
pub struct SpacePoiBuilder {
    pub icon_file: keys::IconFile,
    pub scale: keys::IconSize,
    pub scale_map: keys::MapDisplaySize,
    pub tint: keys::Tint,
    pub opacity: keys::Alpha,
}

impl SpacePoiBuilder {
    pub fn build(self, path: PoiPath, loader: &mut SpaceLoader<'_>, poi: &LoadedPoi) -> anyhow::Result<spacepack::poi::ActivePoi> {
        let visibility = poi.visibility;
        spacepack::poi::ActivePoi::new(loader.active_pack, loader.loader, visibility, path.path, poi, self.icon_file.as_str(), self.scale.into(), self.scale_map.into(), self.tint.into(), self.opacity.into(), loader.device)
    }

    pub fn build_empty() -> spacepack::poi::ActivePoi {
        spacepack::poi::ActivePoi::empty()
    }

    pub fn from_pack(poi: &PackPoi) -> Option<Self> {
        Some(SpacePoiBuilder {
            icon_file: poi.icon_name()?.into(),
            scale: poi.attributes.icon_size.map(Into::into).unwrap_or_default(),
            scale_map: poi.attributes.map_display_size.map(Into::into).unwrap_or_default(),
            tint: poi.attributes.tint.map(Into::into).unwrap_or_default(),
            opacity: poi.attributes.alpha.map(Into::into).unwrap_or_default(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct SpaceTrailBuilder {
    pub texture_file: keys::TextureFile,
    pub geometry: LoadedTrailGeometry,
}

impl SpaceTrailBuilder {
    pub fn build(self, path: TrailPath, loader: &mut SpaceLoader<'_>, trail: &LoadedTrail) -> anyhow::Result<spacepack::trail::ActiveTrail> {
        let visibility = trail.visibility;
        let sections = trail.sections.as_ref().map(|s| &s[..]).unwrap_or(&[]);
        spacepack::trail::ActiveTrail::new(loader.active_pack, loader.loader, self.texture_file.as_str(), self.geometry, sections, visibility, path.path, trail.category, 0, loader.device)
    }

    pub fn build_empty() -> spacepack::trail::ActiveTrail {
        spacepack::trail::ActiveTrail::empty()
    }

    #[cfg(todo = "unused")]
    pub fn from_pack(pack_trail: &PackTrail, trail: &LoadedTrail, params: &TrailParams, data: TrailData, texture_name: String) -> Option<Self> {
        let geometry = trail.vertices_for(&data, params);
        Some(SpaceTrailBuilder {
            geometry,
            texture_file: texture_name.into(),
        })
    }
    pub async fn read_from_pack(path: TrailPath, active: &ActivePack, trail: &mut LoadedTrail, params: TrailParams) -> anyhow::Result<Self> {
        let Some(pack_trail) = active.pack.trails.get(path.path as usize) else {
            anyhow::bail!("expected trail to exist")
        };
        let Some(texture_name) = pack_trail.texture_name().map(String::from) else {
            anyhow::bail!("no texture specified")
        };
        let trail_data = active.load_trail_data(path.path).await?;
        trail.populate_data(&trail_data);
        let geometry = crate::controller::Controller::try_run_blocking("calculating vertices", {
            let trail = trail.clone();
            move || Ok(trail.vertices_for(&trail_data, &params))
        }).await?;
        let setup = SpaceTrailBuilder {
            geometry,
            texture_file: texture_name.into(),
        };
        Ok(setup)
    }

    /// Also indicates if `trail` was [populated](LoadedTrail::populate_data) with section metadata
    pub async fn load_from_pack(path: TrailPath, active: Arc<ActivePack>, mut trail: LoadedTrail, params: TrailParams) -> (anyhow::Result<Self>, LoadedTrail, bool) {
        let sections_prev = trail.sections.as_ref().map(|s| Arc::as_ptr(s) as *const () as usize);
        let setup = Self::read_from_pack(path, &active, &mut trail, params).await
            .with_context(|| format!("loading {path} from {active}"));
        let sections_changed = trail.sections.as_ref().map(|s| Arc::as_ptr(s) as *const () as usize) != sections_prev;
        match setup {
            Ok(setup) =>
                (Ok(setup), trail, sections_changed),
            Err(e) => (Err(e), trail, sections_changed),
        }
    }
}

#[cfg(deleteme)]
#[derive(Debug, Clone)]
pub struct SpacePackBuilder {
    pub map_id: MapIndex,
    pub pois: Vec<(PoiPath<PackPath>, SpacePoiBuilder)>,
    pub trails: Vec<(TrailPath<PackPath>, SpaceTrailBuilder)>,
}

#[cfg(deleteme)]
impl SpacePackBuilder {
    pub async fn setup() -> Self {
        let Some(map_id) = ctx.gameplay_map() else {
            log::warn!("no active map to prepare pack {path} for");
            return Ok(())
        };
        let trail_params = self.trail_params().await;
        let key = path.rel(map_id);
        let Some(map_pack_info) = self.map_pack_info.get(&key) else {
            anyhow::bail!("map pack data for {path} on {map_id} not loaded?");
        };
        let Some(map_pack) = self.map_packs.get_mut(&key) else {
            anyhow::bail!("map pack data for {path} on {map_id} not loaded?");
        };
        let packs = Self::packs().read().await;
        let Some(pack) = packs.lookup_ref(&path) else {
            anyhow::bail!("pack {path} disappeared???")
        };
        let Some(active) = &pack.active else {
            anyhow::bail!("can't prepare pack {path} if it's not loaded")
        };

        let mut pois = Vec::with_capacity(map_pack_info.poi_count());
        for (poi_path, poi) in map_pack.pois(map_pack_info) {
            let pack_poi = active.pack.pois.get(poi_path.path as usize)
                .and_then(|poi| poi.icon_name().map(|icon| (poi, icon)));
            let setup = pack_poi.map(|(poi, icon)| SpacePoiBuilder {
                icon_file: icon.into(),
                scale: poi.attributes.icon_size.map(Into::into).unwrap_or_default(),
                scale_map: poi.attributes.map_display_size.map(Into::into).unwrap_or_default(),
                tint: poi.attributes.tint.map(Into::into).unwrap_or_default(),
                opacity: poi.attributes.alpha.map(Into::into).unwrap_or_default(),
            });
            let is_copy = pack_poi.and_then(|(poi, _)| poi.attributes.copy_value.as_ref()).is_some();
            pois.push((poi_path, poi.clone(), setup, is_copy));
        }
        pois.shrink_to_fit();

        let mut trails = Vec::with_capacity(map_pack_info.trail_count());
        for (trail_path, trail) in map_pack.trails_mut(map_pack_info) {
            let Some(texture_name) = active.pack.trails.get(trail_path.path as usize).and_then(|pack_trail|
                pack_trail.texture_name().map(String::from)
            ) else {
                log::info!("trail#{trail_path} missing texture");
                trails.push((trail_path, trail.clone(), None));
                continue;
            };
            let trail_data = match active.load_trail_data(trail_path.path).await {
                Ok(trail_data) => trail_data,
                Err(e) => {
                    log::error!("{e:#}");
                    trails.push((trail_path, trail.clone(), None));
                    continue
                },
            };
            trail.populate_data(&trail_data);
            // TODO: spawn_blocking all this and parallelize, also dispatch to render thread incrementally as data comes in
            let geometry = trail.vertices_for(&trail_data, &trail_params);
            let setup = SpaceTrailBuilder {
                geometry,
                texture_file: texture_name.into(),
            };
            trails.push((trail_path, trail.clone(), Some(setup)));
        }
        trails.shrink_to_fit();

        (map_id, pois, trails)
    }
}
