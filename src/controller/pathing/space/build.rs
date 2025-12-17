use {
    crate::controller::{
        Controller,
        pathing::{
            registry::ActivePack,
            space::{SpacePack, SpacePoi, SpaceTrail, TrailParams},
            state::visible::{LoadedPoi, LoadedTrail, LoadedTrailGeometry},
        },
    },
    anyhow::Context,
    std::sync::Arc,
    taimi_meta::packs::{PoiPath, TrailPath, CategoryPath},
    taimi_pack::{attributes::RenderAttributes, Poi as PackPoi},
};

pub(super) struct SpaceLoader<'a> {
    pub active_pack: &'a mut SpacePack,
    pub loader: &'a mut dyn taimi_pack::loader::PackLoaderContext,
    pub device: &'a taimi_d3d::dx11::Dx11Device,
}

#[derive(Debug, Clone)]
pub struct SpacePoiBuilder {
    pub attrs: Arc<RenderAttributes>,
}

impl SpacePoiBuilder {
    pub fn build(self, path: PoiPath, loader: &mut SpaceLoader<'_>, poi: &LoadedPoi) -> anyhow::Result<SpacePoi> {
        let visibility = poi.visibility;
        let mut poi = SpacePoi::new(visibility, path.unscope(), CategoryPath::with_path(poi.category), self.attrs);
        poi.setup(loader.loader, loader.device)
            .map(move |()| poi)
    }

    pub fn build_empty() -> SpacePoi {
        SpacePoi::empty()
    }

    pub fn from_pack(poi: &PackPoi) -> Option<Self> {
        Some(SpacePoiBuilder {
            attrs: poi.attributes.render().clone().unwrap_or_default(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct SpaceTrailBuilder {
    pub attrs: Arc<RenderAttributes>,
    pub geometry: LoadedTrailGeometry,
}

impl SpaceTrailBuilder {
    pub fn build(self, path: TrailPath, loader: &mut SpaceLoader<'_>, trail: &LoadedTrail) -> anyhow::Result<SpaceTrail> {
        let visibility = trail.visibility;
        let sections = trail.sections.as_ref().map(|s| &s[..]).unwrap_or(&[]);
        let mut trail = SpaceTrail::new(self.attrs, self.geometry, sections, visibility, path.path, CategoryPath::with_path(trail.category), 0, loader.device)
        trail.setup(loader.loader, loader.device)
            .map(move |()| poi)
    }

    pub fn build_empty() -> SpaceTrail {
        SpaceTrail::empty()
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
        let geometry = Controller::try_run_blocking("calculating vertices", {
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
