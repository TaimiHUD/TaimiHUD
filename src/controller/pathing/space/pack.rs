use {
    super::{
        poi::SpacePoi,
        trail::SpaceTrail,
    },
    crate::{
        controller::pathing::{
            registry::ActivePack,
            space::DrawSpace,
        },
        space::render_list::{MapFrustum, RenderEntity, RenderId, RenderList, RenderListBuilder},
    },
    taimi_meta::packs::{PackIndex, PackPath, PoiIndex, TrailIndex, TrailSectionIndex},
    glamour::Box3,
    std::{mem, sync::Arc, ops},
};

pub struct SpacePack {
    pub pack: Option<Arc<ActivePack>>,
    pub active_trails: Vec<SpaceTrail>,
    pub active_pois: Vec<SpacePoi>,

    // Internal rendering data.
    pub render_list_bookmark: Option<usize>,
    render_poi_bookmark: usize,
    poi_bookmark: usize,
}

impl SpacePack {
    pub fn new() -> Self {
        SpacePack {
            pack: None,
            active_pois: Default::default(),
            active_trails: Default::default(),
            render_list_bookmark: Default::default(),
            render_poi_bookmark: Default::default(),
            poi_bookmark: Default::default(),
        }
    }

    fn prepare_new_map<P, T>(
        &mut self,
        pack_idx: PackIndex,
        pois: P,
        trails: T,
        render_entities: &mut Vec<RenderEntity>,
    ) where
        P: IntoIterator<Item = SpacePoi>,
        T: IntoIterator<Item = SpaceTrail>,
    {
        self.clear();
        self.render_list_bookmark = Some(render_entities.len());

        for mut trail in trails {
            let trail_idx = self.active_trails.len() as TrailIndex;
            trail.render_bookmark = render_entities.len() as _;
            for i_section in 0..trail.section_bounds.len() {
                let render_id = RenderId::TrailSection {
                    pack_idx,
                    trail_idx,
                    section: i_section as TrailSectionIndex,
                };
                let entity = RenderEntity {
                    bounds: trail.section_bounds[i_section],
                    position: trail.section_bounds[i_section].center(),
                    // TODO: just sort by y and reverse draw order if camera dir.y is negative? :p
                    // then only intersecting paths are an issue...
                    //draw_ordered: true,
                    draw_ordered: false,
                    render_id: match trail.is_empty() {
                        false => Some(render_id),
                        true => None,
                    },
                };
                render_entities.push(entity);
            }

            self.active_trails.push(trail);
        }

        self.poi_bookmark = render_entities.len();

        for poi in pois {
            let poi_idx = self.active_pois.len() as PoiIndex;
            let entity = RenderEntity {
                bounds: poi.bounds,
                position: poi.position,
                draw_ordered: true,
                render_id: match poi.is_empty() {
                    false => Some(RenderId::Poi { pack_idx, poi_idx }),
                    true => None,
                },
            };
            render_entities.push(entity);
            self.active_pois.push(poi);
        }
    }

    pub fn clear(&mut self) {
        self.active_trails.clear();
        self.active_pois.clear();
        self.render_list_bookmark = None;
        self.render_poi_bookmark = 0;
        self.poi_bookmark = 0;
    }

    pub fn render_poi_bookmarks(&self) -> ops::Range<PoiIndex> {
        match self.render_poi_bookmark {
            0 => 0..0,
            start => {
                let end = (start + self.active_pois.len()) as PoiIndex;
                let start = start as PoiIndex;
                start..end
            },
        }
    }
}

#[repr(transparent)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PackTextureHandle(usize);

pub struct SpacePackCollection {
    pub loaded_packs: Vec<SpacePack>,

    pub render_list: RenderList,
}

impl SpacePackCollection {
    pub fn new() -> anyhow::Result<SpacePackCollection> {
        Ok(SpacePackCollection {
            loaded_packs: Default::default(),
            render_list: RenderListBuilder::default().build(),
        })
    }

    pub fn clear(&mut self) {
        self.loaded_packs.clear();

        self.render_list.clear();
    }

    pub fn pack_mut<'a>(&'a mut self, path: &PackPath) -> &'a mut SpacePack {
        let index = path.path as usize;
        if self.loaded_packs.len() <= index {
            self.loaded_packs.resize_with(index + 1, || SpacePack::new());
        }
        &mut self.loaded_packs[index]
    }

    #[cfg(todo = "deleteme?")]
    pub fn load_pack<P, T>(&mut self, pack_idx: PackIndex, pois: P, trails: T) -> anyhow::Result<()> where
        P: IntoIterator<Item = SpacePoi>,
        T: IntoIterator<Item = SpaceTrail>,
    {
        let pack = self
            .loaded_packs
            .get_mut(pack_idx as usize)
            .with_context(|| format!("unrecognized pack index {pack_idx}"))?;
        if pack.render_list_bookmark.is_some() {
            log::info!("skipping pack#{pack_idx}, already loaded?");
            return Ok(())
        }

        log::debug!("Preparing pack#{pack_idx} for rendering...");
        self.build_active_pack(pack_idx, pois, trails, None)?;

        if log::log_enabled!(log::Level::Info) {
            let pack = &self.loaded_packs[pack_idx as usize];
            if !pack.active_trails.is_empty() || !pack.active_pois.is_empty() {
                log::info!(
                    "Loaded {} trails and {} POIs from pack #{pack_idx}",
                    pack.active_trails.len(),
                    pack.active_pois.len(),
                );
            }
        }

        //self.recreate_buffers(device)?;
        self.mark_buffers_dirty();

        Ok(())
    }

    #[cfg(todo = "deleteme?")]
    fn build_active_pack<P, T>(
        &mut self,
        pack_idx: PackIndex,
        pois: P, trails: T,
        render_entities: Option<&mut Vec<RenderEntity>>,
    ) -> anyhow::Result<()> where
        P: IntoIterator<Item = SpacePoi>,
        T: IntoIterator<Item = SpaceTrail>,
    {
        let pack = self
            .loaded_packs
            .get_mut(pack_idx as usize)
            .with_context(|| format!("unrecognized pack index {pack_idx}"))?;

        let (entities, inplace) = match render_entities {
            Some(e) => (e, false),
            None => (self.render_list.entities_mut(), true),
        };
        let res = Ok(pack.prepare_new_map(pack_idx, pois, trails, entities));
        #[cfg(todo = "unnecessary")]
        if res.is_err() {
            //.with_context(|| format!("loading pack#{pack_idx}"));
            log::info!("pack#{pack_idx} failed to load, disabling...");
            if let Some(bookmark) = pack.render_list_bookmark {
                let _ = entities.drain(bookmark..);
                /*for entity in &mut self.render_list.entities_mut()[bookmark..] {
                    entity.disable();
                }*/
            }
            pack.clear();
            pack.cleanup_textures();
        }
        if inplace {
            self.render_list.entities_mut_end();
        }
        res
    }

    pub fn rebuild_active(&mut self) -> anyhow::Result<()> {
        let mut render_builder = self.render_list.rebuild();

        for (pack_idx, pack) in self.loaded_packs.iter_mut().enumerate() {
            let pois = mem::take(&mut pack.active_pois);
            let trails = mem::take(&mut pack.active_trails);
            pack.clear();
            pack.active_pois.reserve_exact(pois.len());
            pack.active_trails.reserve_exact(trails.len());
            pack.prepare_new_map(pack_idx as PackIndex, pois, trails, &mut render_builder.entities);
        }

        log::info!(
            "Loaded {} trails and {} POIs",
            self.loaded_packs
                .iter()
                .map(|p| p.active_trails.len())
                .sum::<usize>(),
            self.loaded_packs
                .iter()
                .map(|p| p.active_pois.len())
                .sum::<usize>(),
        );

        //let res = self.recreate_buffers(device)?;
        self.mark_buffers_dirty();
        let res = Ok(());

        self.render_list = render_builder.build();

        res
    }

    /// offset (starting len) currently = 1 to leave space for an identity buffer
    /// at index 0 for drawing trails with
    ///
    /// also [SpacePack::render_poi_bookmark] of 0 is treated as empty so uh don't
    /// use that
    pub fn allocate_poi_buffers(&mut self, mut offset: usize) -> usize {
        for pack in &mut self.loaded_packs {
            pack.render_poi_bookmark = offset;
            offset += pack.active_pois.len();
        }
        offset
    }

    pub fn reset_poi_buffers(&mut self) {
        for pack in &mut self.loaded_packs {
            pack.render_poi_bookmark = 0;
        }
    }

    #[cfg(feature = "goggles")]
    pub fn entities_obscured<'a>(
        &'a self,
        frustum: &'a MapFrustum,
    ) -> impl Iterator<Item = &'a RenderEntity> + 'a {
        self.render_list.visible_entities(frustum)
    }

    pub fn entities_map<'a>(
        &'a self,
        mut bounds: Box3<DrawSpace>,
    ) -> impl Iterator<Item = &'a RenderEntity> + 'a {
        // adding some wiggle room around the map edges...
        let buffer = bounds.size() * 0.15;
        bounds.min.x -= buffer.width;
        bounds.min.z -= buffer.depth;
        bounds.max.x += buffer.width;
        bounds.max.z += buffer.depth;

        self.render_list.map_entities(bounds)
    }

    pub fn deactivate(&mut self, pack_idx: PackIndex, cleanup: bool) {
        let Some(pack) = self.loaded_packs.get_mut(pack_idx as usize) else { return };
        if let Some(bookmark) = pack.render_list_bookmark {
            let bookmark_end = pack.poi_bookmark + pack.active_pois.len();
            let render_list = self.render_list.entities_mut();
            if bookmark_end >= render_list.len() {
                let _ = render_list.drain(bookmark..);
            } else {
                for entity in &mut render_list[bookmark..bookmark_end] {
                    entity.disable();
                }
            }
            self.render_list.entities_mut_end();
        }
        pack.clear();
        if cleanup {
            pack.cleanup_textures();
        }
    }

    pub fn clear_active(&mut self) {
        self.render_list.clear();
        for pack in &mut self.loaded_packs {
            pack.clear();
        }

        #[cfg(deleteme)] {
            self.reset_poi_buffers();
        }
    }
}
