use {
    crate::{
        controller::{
            Controller,
            pathing::{
                registry::ActivePack,
                space::{SpacePack, TrailParams},
                state::visible::{LoadedPoi, LoadedTrail, LoadedTrailGeometry, LoadedTrailSectionInfo, LoadedTrailGeometrySection},
            },
        },
        exports::runtime as rt,
        space::pack::{TrailRender, PoiRender},
        resources::Texture,
    },
    anyhow::Context,
    std::sync::Arc,
    taimi_meta::packs::{PoiPath, TrailPath},
    taimi_pack::{attributes::{AttrString, RenderAttributes}, Poi as PackPoi},
};

pub struct SpaceLoader<'a> {
    pub active_pack: &'a mut SpacePack,
    pub loader: &'a mut dyn taimi_pack::loader::PackLoaderContext,
    pub device: &'a taimi_d3d::dx11::Dx11Device,
}

type SpaceTextureHandle = String;
impl<'a> SpaceLoader<'a> {
    pub fn register_texture(&mut self, texture: &AttrString) -> SpaceTextureHandle {
        let texture = &texture[..];
        let pack_name: Option<&str> = match &self.active_pack {
            #[cfg(todo)]
            SpacePack { pack: Some(pack), .. } => p.pack.pack.name,
            _ => None,
        };
        let name = match pack_name {
            Some(name) => name,
            None => {
                log::error!("texture for EMPTY PACK?");
                "pack_unspecified_TODO"
            },
        };
        format!("{name}{texture}")
    }

    pub fn get_or_load_texture(&mut self, handle: SpaceTextureHandle) -> anyhow::Result<Arc<Texture>> {
        Err(anyhow::anyhow!("TODO: get_or_load_texture {handle}"))
    }
}

#[derive(Debug, Clone)]
pub struct SpacePoiBuilder {
    pub attrs: Arc<RenderAttributes>,
}

impl SpacePoiBuilder {
    pub fn build(self, path: PoiPath, loader: &mut SpaceLoader<'_>, poi: &LoadedPoi) -> anyhow::Result<PoiRender> {
        let visibility = poi.visibility;
        let mut render = PoiRender::empty();
        let texture = poi.poi_attrs().icon_file.as_ref()
            .with_context(|| format!("{path} missing texture"))
            .and_then(|texture| render.setup_texture(loader, texture));
        let _ = rt::log::error_ok(texture);
        Ok(render)
    }

    pub fn build_empty() -> PoiRender {
        PoiRender::empty()
    }

    pub fn from_pack(poi: &PackPoi) -> Option<Self> {
        Some(SpacePoiBuilder {
            attrs: poi.attributes.render.clone().unwrap_or_default(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct SpaceTrailBuilder {
    pub attrs: Arc<RenderAttributes>,
    pub geometry: LoadedTrailGeometry,
    pub sections: Vec<LoadedTrailSectionInfo>,
}

impl SpaceTrailBuilder {
    pub fn build(self, path: TrailPath, loader: &mut SpaceLoader<'_>, trail: &LoadedTrail) -> anyhow::Result<TrailRender> {
        let visibility = trail.visibility;
        let sections = trail.sections.as_ref().map(|s| &s[..]).unwrap_or(&[]);
        let mut render = TrailRender::empty();
        if self.geometry.vertices.is_empty() {
            log::info!("empty trail {path}");
        } else {
            render.setup_geometry(loader, self.geometry)?;
        }
        let texture = trail.trail_attrs().texture.as_ref()
            .with_context(|| format!("{path} missing texture"))
            .and_then(|texture| render.setup_texture(loader, texture));
        let _ = rt::log::error_ok(texture);
        Ok(render)
    }

    pub fn build_empty() -> TrailRender {
        TrailRender::empty()
    }

    #[cfg(todo = "unused")]
    pub fn from_pack(pack_trail: &PackTrail, trail: &LoadedTrail, params: &TrailParams, data: TrailData, texture_name: String) -> Option<Self> {
        let geometry = trail.vertices_for(&data, params);
        Some(SpaceTrailBuilder {
            geometry,
            attrs: pack_trail.attributes.render.clone().unwrap_or_default(),
        })
    }
    pub async fn read_from_pack(path: TrailPath, active: &ActivePack, trail: &mut LoadedTrail, params: TrailParams) -> anyhow::Result<Self> {
        let Some(pack_trail) = active.pack.trails.get(path.path as usize) else {
            anyhow::bail!("expected trail to exist")
        };
        let attrs = pack_trail.attributes.render.clone();
        let trail_data = active.load_trail_data(path.path).await?;
        trail.populate_data(&trail_data);
        let geometry = Controller::try_run_blocking("calculating vertices", {
            let trail = trail.clone();
            move || Ok(trail.vertices_for(&trail_data, &params))
        }).await?;
        let sections = trail.sections(&geometry).collect::<Vec<_>>();
        let cap = LoadedTrailSectionInfo::cap(&sections);
        trail.populate_geometry_info(sections.iter().map(LoadedTrailGeometrySection::with_info), cap);
        let setup = SpaceTrailBuilder {
            geometry,
            sections,
            attrs: attrs.unwrap_or_default(),
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
