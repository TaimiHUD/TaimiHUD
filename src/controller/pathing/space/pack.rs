use {
    crate::{
        controller::pathing::{
            registry::PackVecOf,
            space::DrawSpace,
            shared::SharedGameplayMap,
        },
        space::render_list::{MapFrustum, RenderEntity, RenderId, RenderList, RenderListBuilder},
    },
    bitvec::vec::BitVec,
    taimi_meta::{
        spatial::{box3aabb, irrelevant_box3, BvhShape},
        packs::{id::{IdVariant, MarkerId}, PackIndex, PackPath, PoiIndex, TrailIndex, TrailSectionIndex},
    },
    glamour::Box3,
    std::{mem, sync::Arc, ops},
    bvh::aabb,
};

pub struct SpacePack {
    // Internal rendering data.
    #[cfg(todo)]
    pub render_list_bookmark: Option<usize>,
    #[cfg(todo)]
    poi_bookmark: usize,
}

impl SpacePack {
    pub fn new() -> Self {
        SpacePack {
            #[cfg(todo)]
            render_list_bookmark: Default::default(),
            #[cfg(todo)]
            poi_bookmark: Default::default(),
        }
    }

    pub fn clear(&mut self) {
        #[cfg(todo)]
        {
            self.render_list_bookmark = None;
            self.poi_bookmark = 0;
        }
    }
}

#[cfg(deleteme)]
impl SpacePack {
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
}

pub struct SpaceEntity {
    pub id: MarkerId,
    pub bounds: aabb::Aabb<f32, 3>,
}
impl SpaceEntity {
    pub fn invalid() -> Self {
        Self {
            id: MarkerId::EMPTY,
            bounds: box3aabb(irrelevant_box3::<DrawSpace>()),
        }
    }
}
impl aabb::Bounded<f32, 3> for SpaceEntity {
    fn aabb(&self) -> aabb::Aabb<f32, 3> {
        self.bounds
    }
}
pub struct SpaceEntities {
    pub entities: Vec<BvhShape<SpaceEntity>>,
}
impl SpaceEntities {
    pub fn new() -> Self {
        Self {
            entities: Vec::new(),
        }
    }

    pub fn retain<F: FnMut(&mut SpaceEntity) -> bool>(&mut self, mut cond: F) -> BitVec {
        let mut removed: BitVec = Default::default();
        removed.resize(self.entities.len(), false);
        for (i, e) in self.entities.iter_mut().enumerate() {
            if !cond(e) {
                *e = BvhShape::new(SpaceEntity::invalid());
                if let Some(mut b) = removed.get_mut(i) {
                    *b = true;
                }
            }
        }
        removed
    }
    pub fn remove_pack(&mut self, pack: PackPath) -> BitVec {
        self.retain(|e| match e.id.variant() {
            IdVariant::MarkerRegistered(p) => p.root != pack,
            IdVariant::MarkerLoaded(p) => p.root.root != pack,
            _ => true,
        })
    }
}

#[repr(transparent)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PackTextureHandle(usize);

pub struct SpacePackCollection {
    pub loaded_packs: PackVecOf<SpacePack>,
    pub render_entities: SpaceEntities,

    #[cfg(todo)]
    pub render_list: RenderList,
}

impl SpacePackCollection {
    pub fn new() -> SpacePackCollection {
        SpacePackCollection {
            loaded_packs: Default::default(),
            #[cfg(todo)]
            render_list: RenderListBuilder::default().build(),
            render_entities: SpaceEntities::new(),
        }
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

    #[cfg(todo)]
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

    #[cfg(todo)]
    #[cfg(feature = "goggles")]
    pub fn entities_obscured<'a>(
        &'a self,
        frustum: &'a MapFrustum,
    ) -> impl Iterator<Item = &'a RenderEntity> + 'a {
        self.render_list.visible_entities(frustum)
    }

    #[cfg(todo)]
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

    #[cfg(todo)]
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

    #[cfg(todo)]
    pub fn clear_active(&mut self) {
        self.render_list.clear();
        for pack in &mut self.loaded_packs {
            pack.clear();
        }

        #[cfg(deleteme)] {
            self.reset_poi_buffers();
        }
    }

    #[cfg(todo)]
    pub fn all_entities(&self, map: &SharedGameplayMap) -> impl Iterator<Item = MarkerId> + '_ {
    }
}
