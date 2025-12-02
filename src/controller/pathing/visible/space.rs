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
        let render = poi.attributes.render.as_ref()?;
        let attrs = render.poi.as_ref()?;
        Some(SpacePoiBuilder {
            // TODO: allow empty?
            icon_file: keys::IconFile(keys::File(attrs.icon_file.as_ref()?.clone())),
            scale: attrs.icon_size.map(Into::into).unwrap_or_default(),
            scale_map: attrs.map_display_size.map(Into::into).unwrap_or_default(),
            tint: render.tint.map(Into::into).unwrap_or_default(),
            opacity: render.alpha.map(Into::into).unwrap_or_default(),
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
        let render = pack_trail.attributes.render.as_ref();
        let Some(texture_name) = render.as_ref().and_then(|render| render.trail.texture.clone()) else {
            // TODO: allow empty?
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
            texture_file: keys::TextureFile(keys::File(texture_name)),
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
