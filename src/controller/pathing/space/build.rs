use {
    crate::{
        controller::{
            pathing::{
                registry::{ActivePack, LoadedTrailPath, LoadedMarkerPath}, shared::SharedPackInfo, space::{SpacePack, TrailParams}, state::visible::{LoadedPoi, LoadedTrail, LoadedTrailGeometry, LoadedTrailGeometrySection, LoadedTrailSectionInfo},
                shared::{SharedResourceRequests, SharedResourceRequestsTx},
            }, Controller
        },
        exports::runtime::{
            self as rt,
            textures::{TextureKey, TextureSlot},
        },
        space::pack::{PoiRender, TrailRender},
        TEXTURES,
    },
    anyhow::Context,
    futures::future::Either,
    std::{collections::{btree_map, BTreeMap}, sync::Arc},
    taimi_meta::packs::{MarkerId, PackMapPath, PoiPath, TrailPath},
    taimi_pack::{attributes::{AttrString, RenderAttributes}, Poi as PackPoi},
    taimi_sync::watched::Watcher,
};

pub struct SpaceLoader<'a> {
    pub active_pack: &'a mut SpacePack,
    pub loader: &'a mut dyn taimi_pack::loader::PackLoaderContext,
    pub device: &'a taimi_d3d::dx11::Dx11Device,
}

type SpaceTextureHandle = (TextureKey, Result<TextureSlot, AttrString>);
impl<'a> SpaceLoader<'a> {
    fn texture_key(&mut self, texture: &AttrString) -> String {
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
    pub fn register_texture(info: &SharedPackInfo, texture: &AttrString) -> SpaceTextureHandle {
        let key = info.key_for_subresource(texture);
        let tex = TEXTURES.lookup_pair_with(&key, |canon, tex| canon.map(|canon| {
            let tex = (!tex.is_loading() && !tex.can_load()).then_some(tex.clone());
            (canon.clone(), tex)
        })).flatten();
        let (key, tex) = match tex {
            Some((key, tex)) => (key, tex),
            None => (key.into(), None),
        };
        let tex = tex.map(Ok).unwrap_or_else(|| Err(texture.clone()));
        (key, tex)
    }

    pub fn setup_texture(key: &mut Option<TextureKey>, slot: &mut Option<TextureSlot>, pack_info: &SharedPackInfo, texture: Option<&AttrString>) {
        let key = match key {
            Some(key) =>
                return Self::get_texture(&*key, slot),
            None => match texture {
                Some(texture) => key.insert(pack_info.key_for_subresource(texture)),
                None => return,
            },
        };
        *slot = TEXTURES.reserve_key_mut(key);
    }
    pub fn get_texture(key: &TextureKey, slot: &mut Option<TextureSlot>) {
        if Self::slot_loaded(slot.as_ref()) { return }
        *slot = TEXTURES.lookup_slot(key);
    }
    pub fn slot_loaded(slot: Option<&TextureSlot>) -> bool {
        slot
            .map(|slot| !slot.is_loading() && !slot.can_load())
            .unwrap_or(false)
    }
    #[cfg(deleteme)]
    pub fn setup_texture(&mut self, key: &mut TextureKey, slot: &mut Option<TextureSlot>) {
        Self::get_texture(key, slot)
    }

    pub fn get_or_load_texture(loader: &mut dyn taimi_pack::loader::PackLoaderContext, (mut key, tex): SpaceTextureHandle) -> (TextureKey, Option<TextureSlot>) {
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
        } else { None };
        let data = match data {
            None => {
                let res = loader.load_asset_dyn(&name[..]).and_then(|mut asset| {
                        let mut bytes = Vec::new();
                        asset.read_to_end(&mut bytes)
                            .map(move |_amt| bytes)
                            .map_err(Into::into)
                }).with_context(|| format!("reading texture {}", name));
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
}

#[derive(Debug, Clone)]
pub struct SpacePoiBuilder {
    pub attrs: Arc<RenderAttributes>,
}

impl SpacePoiBuilder {
    #[cfg(deleteme)]
    pub fn build(self, path: PoiPath, loader: &mut SpaceLoader<'_>, poi: &LoadedPoi) -> anyhow::Result<PoiRender> {
        let visibility = poi.visibility;
        let mut render = PoiRender::empty();
        let texture = poi.poi_attrs().icon_file.as_ref()
            .with_context(|| format!("{path} missing texture"))
            .map(|texture| render.setup_texture(loader, texture));
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
    #[cfg(deleteme)]
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
            .map(|texture| render.setup_texture(loader, texture));
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
    #[cfg(deleteme)]
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
        let sections = trail.section_info.sections(&geometry).collect::<Vec<_>>();
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
    #[cfg(deleteme)]
    pub async fn load_from_pack(path: TrailPath, active: Arc<ActivePack>, mut trail: LoadedTrail, params: TrailParams) -> (anyhow::Result<Self>, LoadedTrail, bool) {
        let sections_prev = trail.section_info.sections.as_ref().map(|s| Arc::as_ptr(s) as *const () as usize);
        let setup = Self::read_from_pack(path, &active, &mut trail, params).await
            .with_context(|| format!("loading {path} from {active}"));
        let sections_changed = trail.section_info.sections.as_ref().map(|s| Arc::as_ptr(s) as *const () as usize) != sections_prev;
        match setup {
            Ok(setup) =>
                (Ok(setup), trail, sections_changed),
            Err(e) => (Err(e), trail, sections_changed),
        }
    }
}

pub type TrailGeometryRequests = SharedResourceRequests<LoadedTrailPath<PackMapPath>, LoadedTrailGeometry>;
pub type TrailGeometryRequestsTx = SharedResourceRequestsTx<LoadedTrailPath<PackMapPath>, LoadedTrailGeometry>;
pub type TextureLoadRequests = SharedResourceRequests<LoadedMarkerPath<PackMapPath>, Option<TextureKey>>;
pub type TextureLoadRequestsTx = SharedResourceRequestsTx<LoadedMarkerPath<PackMapPath>, Option<TextureKey>>;
