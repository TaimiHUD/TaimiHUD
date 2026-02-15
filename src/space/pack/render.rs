use {
    crate::{
        controller::pathing::{
            registry::{LoadedPoiPath, LoadedTrailPath},
            space::DrawSpace,
        },
        resources::{ShaderLoader, ShaderPair},
        space::pack::{
            instance::{PoiVertex, PoiVertexBuffer},
            PackRenderData,
            PackRenderResources,
            PoiCommonRenderData,
            PoiRender,
            TrailRender,
        },
    },
    glamour::{Point3, Vector3},
    std::{collections::BinaryHeap, ops},
    taimi_d3d::dx11::prelude::*,
    taimi_hoard::cmp::CmpIgnore,
    taimi_meta::{packs::TrailSectionPath, ui::LocalContext},
};

/// BvhIter expected to produce positions, which should be [Point3::INFINITY]
/// for items that can ignore the distance priority queue
pub struct RenderOrderBuilder<'a, T, BvhIter> {
    pub bvh_iter: BvhIter,
    pub draw_order_heap: &'a mut BinaryHeap<HeapEntityOf<T>>,
    pub cam_origin: Point3<DrawSpace>,
    pub cam_dir: Vector3<DrawSpace>,
}

impl<'a, T, BvhIter> Iterator for RenderOrderBuilder<'a, T, BvhIter>
where
    BvhIter: Iterator<Item = (Point3<DrawSpace>, T)>,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((position, entity)) = self.bvh_iter.next() {
            let cam_dist = match position {
                pos if pos.x.is_infinite() => return Some(entity),
                #[cfg(todo)]
                true => {
                    // TODO: broken or inaccurate idk
                    let cam_dist = (position - self.cam_origin).dot(self.cam_dir);
                    let cam_dist = f32::to_bits(cam_dist) as i32;
                    let cam_dist = cam_dist ^ ((cam_dist >> 30) as u32 >> 1) as i32;
                    cam_dist
                },
                position => (position.distance_squared(self.cam_origin) * 1_000_000.0)
                    .min(0x40000000i32 as f32) as i32,
            };
            self.draw_order_heap
                .push(HeapEntity { cam_dist, value: CmpIgnore(entity) });
        }

        self.draw_order_heap.pop().map(|he| he.value.0)
    }
}

pub type RenderOrderHeap<T> = BinaryHeap<HeapEntity<CmpIgnore<T>>>;
pub type HeapEntityOf<T> = HeapEntity<CmpIgnore<T>>;
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct HeapEntity<T> {
    cam_dist: i32,
    value: T,
}

pub trait DrawSpaceEntity {
    fn draw_trail_section(
        &mut self,
        pack_data: &PackRenderData,
        space_idx: usize,
        trail: &TrailRender,
        path: LoadedTrailPath,
        section: TrailSectionPath,
    ) -> bool;
    fn draw_poi(
        &mut self,
        pack_data: &PackRenderData,
        space_idx: usize,
        poi: &PoiRender,
        path: LoadedPoiPath,
    ) -> bool;
    fn finish(&mut self);
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum ShaderState {
    Trail,
    Poi,
}

/// original renderer
pub struct DrawSpacePack<'a> {
    pub state: Option<Option<ShaderState>>,
    pub poi_common: &'a PoiCommonRenderData,
    pub shaders: &'a ShaderLoader,
    pub shader_trail: Option<ShaderPair>,
    pub context: &'a Dx11Context,
}
impl<'a> DrawSpacePack<'a> {
    fn bind_common(&mut self) -> Option<()> {
        if self.state.is_some() {
            return Some(())
        }
        self.shader_trail = self.shaders.pair_named("trail").ok();
        self.poi_common.set_primitive(self.context);
        self.state = Some(None);
        Some(())
    }
    fn bind_trail(&mut self) -> Option<()> {
        if matches!(self.state, Some(Some(ShaderState::Trail))) {
            return Some(())
        }
        self.bind_common()?;

        let shader = self.shader_trail.as_ref()?;
        shader.set(self.context);
        self.state = Some(Some(ShaderState::Trail));
        Some(())
    }
    fn bind_poi(&mut self) -> Option<()> {
        if matches!(self.state, Some(Some(ShaderState::Poi))) {
            return Some(())
        }
        self.bind_common()?;

        self.poi_common.set(self.context);
        self.state = Some(Some(ShaderState::Poi));
        Some(())
    }
}
impl DrawSpaceEntity for DrawSpacePack<'_> {
    fn draw_trail_section(
        &mut self,
        _pack_data: &PackRenderData,
        _space_idx: usize,
        trail: &TrailRender,
        _lpath: LoadedTrailPath,
        section: TrailSectionPath,
    ) -> bool {
        if self.bind_trail().is_none() {
            return false
        }
        trail.bind_texture(self.context, self.poi_common, LocalContext::World);
        trail.draw_section(self.context, section, LocalContext::World);
        true
    }

    fn draw_poi(
        &mut self,
        pack_data: &PackRenderData,
        _space_idx: usize,
        poi: &PoiRender,
        lpath: LoadedPoiPath,
    ) -> bool {
        if self.bind_poi().is_none() {
            return false
        }
        poi.bind_texture(self.context, self.poi_common, LocalContext::World);
        poi.draw(
            self.context,
            pack_data.render_poi_bookmark + lpath.path as usize,
            LocalContext::World,
        );
        true
    }

    fn finish(&mut self) {}
}

/// arcrender
pub struct DrawSpaceArc<'a> {
    pub state: Option<Option<ShaderState>>,
    pub poi_common: &'a PoiCommonRenderData,
    pub resources: &'a PackRenderResources,
    pub context: &'a Dx11Context,
    pub last_quad: Option<&'a PoiVertexBuffer>,
}
impl<'a> DrawSpaceArc<'a> {
    fn bind_common(&mut self) -> Option<()> {
        if self.state.is_some() {
            return Some(())
        }

        let shaderp = self.resources.shader_p.as_ref()?;
        let ib = self.resources.entities_ib.as_ref()?;
        let cb_p = self.resources.shared_cb_p.as_ref()?;
        let cb_v = self.resources.shared_cb_v.as_ref()?;
        shaderp.set(self.context);
        ib.set(self.context, 1);
        cb_p.set(self.context, 0);
        cb_v.set(self.context, 0);
        self.poi_common.set_primitive(self.context);
        self.state = Some(None);
        Some(())
    }
    fn bind_trail(&mut self) -> Option<()> {
        if matches!(self.state, Some(Some(ShaderState::Trail))) {
            return Some(())
        }
        let (shaderv, shaderl) = self.resources.shader_trail.as_ref()?;
        self.bind_common()?;

        shaderv.set(self.context);
        shaderl.set(self.context);
        self.last_quad = None;
        self.state = Some(Some(ShaderState::Trail));
        Some(())
    }
    fn bind_poi(&mut self) -> Option<()> {
        if matches!(self.state, Some(Some(ShaderState::Poi))) {
            return Some(())
        }
        let (shaderv, shaderl) = self.resources.shader_poi.as_ref()?;
        self.bind_common()?;

        shaderv.set(self.context);
        shaderl.set(self.context);
        self.state = Some(Some(ShaderState::Poi));
        Some(())
    }
}
impl DrawSpaceEntity for DrawSpaceArc<'_> {
    fn draw_trail_section(
        &mut self,
        _pack_data: &PackRenderData,
        space_idx: usize,
        trail: &TrailRender,
        _lpath: LoadedTrailPath,
        section: TrailSectionPath,
    ) -> bool {
        if space_idx >= self.resources.len {
            return false
        }
        let vb = trail.section_vb_ng.as_ref().and_then(|vb| {
            trail
                .section_geometry_vertices(section.path)
                .map(|range| (vb, range))
        });
        let Some((vb, ops::Range { start, end })) = vb else { return false };
        if self.bind_trail().is_none() {
            return false
        }
        trail.bind_texture(self.context, self.poi_common, LocalContext::MAP);
        vb.set(self.context, 0);
        unsafe {
            self.context
                .DrawInstanced(end - start, 1, start, space_idx as u32);
        }
        true
    }

    fn draw_poi(
        &mut self,
        _pack_data: &PackRenderData,
        space_idx: usize,
        poi: &PoiRender,
        _lpath: LoadedPoiPath,
    ) -> bool {
        if space_idx >= self.resources.len {
            return false
        }
        if self.bind_poi().is_none() {
            return false
        }
        let vb_quad = match poi.occlude {
            true => self.resources.poi_vb_trans.as_ref(),
            _ => {
                #[cfg(todo)]
                if poi.icon.is_none() {
                    continue
                }
                self.resources.poi_vb.as_ref()
            },
        };
        let Some(vb) = vb_quad else { return false };
        if self.last_quad != Some(vb) {
            vb.set(self.context, 0);
            self.last_quad = Some(vb);
        }

        poi.bind_texture(self.context, self.poi_common, LocalContext::MAP);
        unsafe {
            self.context
                .DrawInstanced(PoiVertex::POI_QUAD.len() as u32, 1, 0, space_idx as u32);
        }
        true
    }

    fn finish(&mut self) {}
}
