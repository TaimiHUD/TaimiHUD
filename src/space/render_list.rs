use {
    super::dx11::PerspectiveInputData,
    crate::marker::atomic::MapSpace,
    bvh::{
        aabb::{Bounded, IntersectsAabb},
        bounding_hierarchy::{BHShape, BoundingHierarchy},
        bvh::Bvh,
    },
    glamour::vec4,
    std::collections::BinaryHeap,
};

#[derive(Copy, Clone, Debug)]
pub struct RenderEntity {
    pub bounds: glamour::Box3<MapSpace>,
    pub position: glamour::Point3<MapSpace>,
    pub draw_ordered: bool,
    pub render_id: RenderId,
}

#[derive(Copy, Clone, Debug)]
pub enum RenderId {
    TrailSection {
        pack_idx: usize,
        trail_idx: usize,
        section: usize,
    },
    Poi {
        pack_idx: usize,
        poi_idx: usize,
    },
}

pub struct RenderListBuilder {
    pub entities: Vec<RenderEntity>,
    // Saving these to not reset their memory between builds
    entity_shapes: Vec<RenderEntityShape>,
    draw_order_heap: BinaryHeap<HeapEntity>,
}

impl RenderListBuilder {
    pub fn build(self) -> RenderList {
        let entities = self.entities;
        let mut shapes = self.entity_shapes;
        shapes.clear();
        shapes.reserve_exact(entities.len());
        let spatial_map = SpatialMap::build(&entities, shapes);
        RenderList {
            entities,
            spatial_map,
            draw_order_heap: self.draw_order_heap,
        }
    }
}

impl Default for RenderListBuilder {
    fn default() -> RenderListBuilder {
        RenderListBuilder {
            entities: Vec::with_capacity(8192),
            entity_shapes: Vec::with_capacity(8192),
            draw_order_heap: BinaryHeap::with_capacity(4096),
        }
    }
}

pub struct RenderList {
    entities: Vec<RenderEntity>,
    spatial_map: SpatialMap,
    draw_order_heap: BinaryHeap<HeapEntity>,
}

impl RenderList {
    pub fn rebuild(&mut self) -> RenderListBuilder {
        std::mem::take(&mut self.spatial_map.bvh.nodes);
        let mut builder = RenderListBuilder {
            entities: std::mem::take(&mut self.entities),
            entity_shapes: std::mem::take(&mut self.spatial_map.shapes),
            draw_order_heap: std::mem::take(&mut self.draw_order_heap),
        };
        builder.entities.clear();
        builder.entity_shapes.clear();
        builder
    }

    pub fn update(&mut self, index: usize) {
        self.spatial_map.shapes[index] = RenderEntityShape::new((index, &self.entities[index]));
        self.spatial_map
            .bvh
            .update_shapes(Some(&index), &mut self.spatial_map.shapes);
    }

    /// Gets visible entities in the correct draw order.
    pub fn get_entities_for_drawing<'rs>(
        &'rs mut self,
        cam_origin: glamour::Point3<MapSpace>,
        cam_dir: glamour::Vector3<MapSpace>,
        frustum: &'rs MapFrustum,
    ) -> impl Iterator<Item = &'rs RenderEntity> + 'rs {
        self.draw_order_heap.clear();
        RenderOrderBuilder {
            entities: &self.entities,
            bvh_iter: self.spatial_map.select_visible_entities(frustum),
            draw_order_heap: &mut self.draw_order_heap,
            cam_origin,
            cam_dir,
        }
    }
}

struct RenderOrderBuilder<'rs, BvhIter> {
    entities: &'rs [RenderEntity],
    bvh_iter: BvhIter,
    draw_order_heap: &'rs mut BinaryHeap<HeapEntity>,
    cam_origin: glamour::Point3<MapSpace>,
    cam_dir: glamour::Vector3<MapSpace>,
}

impl<'rs, BvhIter> Iterator for RenderOrderBuilder<'rs, BvhIter>
where
    BvhIter: Iterator<Item = usize>,
{
    type Item = &'rs RenderEntity;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(next) = self.bvh_iter.next() {
            let entity = &self.entities[next];
            if !entity.draw_ordered {
                return Some(entity);
            } else {
                let cam_dist = (entity.position - self.cam_origin).dot(self.cam_dir);
                let cam_dist = f32::to_bits(cam_dist) as i32;
                let cam_dist = cam_dist ^ ((cam_dist >> 30) as u32 >> 1) as i32;
                self.draw_order_heap.push(HeapEntity {
                    cam_dist,
                    idx: next,
                });
            }
        }

        self.draw_order_heap.pop().map(|he| &self.entities[he.idx])
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct HeapEntity {
    cam_dist: i32,
    idx: usize,
}

struct RenderEntityShape {
    bounds: bvh::aabb::Aabb<f32, 3>,
    entity_idx: usize,
    bh_idx: usize,
}

impl RenderEntityShape {
    fn new((entity_idx, entity): (usize, &RenderEntity)) -> Self {
        RenderEntityShape {
            bounds: bvh::aabb::Aabb {
                min: [
                    entity.bounds.min.x,
                    entity.bounds.min.y,
                    entity.bounds.min.z,
                ]
                .into(),
                max: [
                    entity.bounds.max.x,
                    entity.bounds.max.y,
                    entity.bounds.max.z,
                ]
                .into(),
            },
            entity_idx,
            bh_idx: 0,
        }
    }
}

impl Bounded<f32, 3> for RenderEntityShape {
    fn aabb(&self) -> bvh::aabb::Aabb<f32, 3> {
        self.bounds
    }
}

impl BHShape<f32, 3> for RenderEntityShape {
    fn set_bh_node_index(&mut self, bh_idx: usize) {
        self.bh_idx = bh_idx;
    }

    fn bh_node_index(&self) -> usize {
        self.bh_idx
    }
}

struct SpatialMap {
    shapes: Vec<RenderEntityShape>,
    bvh: Bvh<f32, 3>,
}

impl SpatialMap {
    fn build(entities: &[RenderEntity], mut shapes: Vec<RenderEntityShape>) -> SpatialMap {
        shapes.extend(entities.iter().enumerate().map(RenderEntityShape::new));
        let bvh = Bvh::build_par(&mut shapes);
        SpatialMap { shapes, bvh }
    }

    pub fn select_visible_entities<'a>(
        &'a self,
        frustum: &'a MapFrustum,
    ) -> impl Iterator<Item = usize> + 'a {
        self.bvh
            .traverse_iterator(frustum, &self.shapes)
            .map(|shape| shape.entity_idx)
    }
}

#[derive(Copy, Clone)]
pub struct MapFrustum(pub [glamour::Vector4<MapSpace>; 6]);

impl MapFrustum {
    pub fn from_camera_data(
        data: &PerspectiveInputData,
        aspect_ratio: f32,
        near: f32,
        far: f32,
    ) -> MapFrustum {
        let fov = data.fov;
        let p = data.pos;
        let d = data.front.normalize();
        let right = d.cross(glam::Vec3::new(0.0, 1.0, 0.0)).normalize();
        let up = right.cross(d).normalize();

        let tan_fov2 = (fov / 2.0).tan();
        let h_near = 2.0 * tan_fov2 * near;
        let w_near = h_near * aspect_ratio;
        let h_far = 2.0 * tan_fov2 * far;
        let w_far = h_far * aspect_ratio;

        let fc = p + d * far;
        let nc = p + d * near;
        let up_far = up * h_far / 2.0;
        let right_far = right * w_far / 2.0;
        let up_near = up * h_near / 2.0;
        let right_near = up * w_near / 2.0;

        let ftr = fc + up_far + right_far;
        let ftl = fc + up_far - right_far;
        let fbr = fc - up_far + right_far;
        let fbl = fc - up_far - right_far;

        let ntr = nc + up_near + right_near;
        let ntl = nc + up_near - right_near;
        let nbr = nc - up_near + right_near;
        let nbl = nc - up_near - right_near;

        let near_plane = points_to_plane(ntl, ntr, nbl);
        let far_plane = points_to_plane(ftr, ftl, fbr);
        let up_plane = points_to_plane(ftl, ftr, ntl);
        let down_plane = points_to_plane(fbr, fbl, nbr);
        let right_plane = points_to_plane(ftr, fbr, ntr);
        let left_plane = points_to_plane(ftl, ntl, fbl);

        MapFrustum([
            near_plane.into(),
            far_plane.into(),
            up_plane.into(),
            down_plane.into(),
            right_plane.into(),
            left_plane.into(),
        ])
    }
}

fn points_to_plane(p0: glam::Vec3, p1: glam::Vec3, p2: glam::Vec3) -> glam::Vec4 {
    let v = p1 - p0;
    let u = p2 - p0;
    let n = v.cross(u).normalize();
    let d = -n.dot(p0);
    glam::Vec4::new(n.x, n.y, n.z, d)
}

fn aabb_corners(aabb: &bvh::aabb::Aabb<f32, 3>) -> [glamour::Vector4<MapSpace>; 8] {
    [
        vec4!(aabb.min.x, aabb.min.y, aabb.min.z, 1.0),
        vec4!(aabb.max.x, aabb.min.y, aabb.min.z, 1.0),
        vec4!(aabb.min.x, aabb.max.y, aabb.min.z, 1.0),
        vec4!(aabb.max.x, aabb.max.y, aabb.min.z, 1.0),
        vec4!(aabb.min.x, aabb.min.y, aabb.max.z, 1.0),
        vec4!(aabb.max.x, aabb.min.y, aabb.max.z, 1.0),
        vec4!(aabb.min.x, aabb.max.y, aabb.max.z, 1.0),
        vec4!(aabb.max.x, aabb.max.y, aabb.max.z, 1.0),
    ]
}

impl IntersectsAabb<f32, 3> for MapFrustum {
    fn intersects_aabb(&self, aabb: &bvh::aabb::Aabb<f32, 3>) -> bool {
        // let corners = aabb_corners(aabb);
        // 'plane: for plane in self.0 {
        //     for corner in corners {
        //         // If any corner is inside this plane, move to the next.
        //         if plane.dot(corner) >= 0.0 {
        //             continue 'plane;
        //         }
        //     }
        //     // All corners are outside this plane.
        //     return false;
        // }
        true
    }
}
