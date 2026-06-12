use {
    crate::{render::machine::RenderPosition, space::DrawSpace},
    bitvec::vec::BitVec,
    glamour::{Box3, Intersection, Point3, Vector3, Vector4},
    std::ops::Range,
};
#[cfg(feature = "space-list")]
use {
    bvh::{
        aabb::{Bounded, IntersectsAabb},
        bounding_hierarchy::BHShape,
        bvh::Bvh,
    },
    glamour::vec4,
    std::collections::BinaryHeap,
};

#[derive(Copy, Clone, Debug)]
pub struct RenderEntity {
    pub bounds: Box3<DrawSpace>,
    pub position: Point3<DrawSpace>,
    pub draw_ordered: bool,
    pub render_id: Option<RenderId>,
}

impl RenderEntity {
    pub fn disable(&mut self) {
        self.render_id = None;
        self.draw_ordered = false;
    }
}

#[derive(Copy, Clone, Debug)]
pub enum RenderId {
    TrailSection { pack_idx: usize, trail_idx: usize, section: usize },
    Poi { pack_idx: usize, poi_idx: usize },
}

pub struct RenderListBuilder {
    pub entities: Vec<RenderEntity>,
    // Saving these to not reset their memory between builds
    #[cfg(feature = "space-list")]
    entity_shapes: Vec<RenderEntityShape>,
    #[cfg(feature = "space-list")]
    draw_order_heap: BinaryHeap<HeapEntity>,
}

impl RenderListBuilder {
    pub fn build(self) -> RenderList {
        let entities = self.entities;
        #[cfg(feature = "space-list")]
        let spatial_map = {
            let mut shapes = self.entity_shapes;
            shapes.clear();
            shapes.reserve_exact(entities.len());
            SpatialMap::build(&entities, shapes)
        };
        RenderList {
            #[cfg(feature = "space-list")]
            unpopulated: Default::default(),
            entities,
            #[cfg(feature = "space-list")]
            spatial_map,
            #[cfg(feature = "space-list")]
            draw_order_heap: self.draw_order_heap,
        }
    }
}

impl Default for RenderListBuilder {
    fn default() -> RenderListBuilder {
        RenderListBuilder {
            entities: Vec::new(),
            #[cfg(feature = "space-list")]
            entity_shapes: Vec::new(),
            #[cfg(feature = "space-list")]
            draw_order_heap: BinaryHeap::with_capacity(4096),
        }
    }
}

pub struct RenderList {
    entities: Vec<RenderEntity>,
    #[cfg(feature = "space-list")]
    spatial_map: SpatialMap,
    #[cfg(feature = "space-list")]
    draw_order_heap: BinaryHeap<HeapEntity>,
    #[cfg(feature = "space-list")]
    unpopulated: BitVec,
}

impl RenderList {
    pub const BOUNDS_NONE: Box3<DrawSpace> = Box3::new(
        Point3::new(-9000.0, -9000.0, -9000.0),
        Point3::new(-9001.0, -9001.0, -9001.0),
    );
    pub fn rebuild(&mut self) -> RenderListBuilder {
        //std::mem::take(&mut self.spatial_map.bvh.nodes);
        let mut builder = RenderListBuilder {
            entities: std::mem::take(&mut self.entities),
            #[cfg(feature = "space-list")]
            entity_shapes: std::mem::take(&mut self.spatial_map.shapes),
            #[cfg(feature = "space-list")]
            draw_order_heap: std::mem::take(&mut self.draw_order_heap),
        };
        builder.entities.clear();
        #[cfg(feature = "space-list")]
        {
            builder.entity_shapes.clear();
            self.unpopulated.clear();
        }
        builder
    }

    pub fn clear(&mut self) {
        self.entities.clear();
        #[cfg(feature = "space-list")]
        {
            self.draw_order_heap.clear();
            self.spatial_map.bvh.nodes = Vec::new();
            self.spatial_map.shapes.clear();
            self.unpopulated.clear();
        }
    }

    pub fn update_bounds(&mut self, index: usize, bounds: Box3<DrawSpace>) {
        let Some(e) = self.entities.get_mut(index) else { return };
        e.bounds = bounds;

        #[cfg(feature = "space-list")]
        if let Some(shape) = self.spatial_map.shape_for_entity_mut(index) {
            let new_bounds = RenderEntityShape::new((index, &*e)).bounds;
            if shape.bounds == new_bounds {
                return
            }

            #[cfg(todo)]
            if shape.bounds.contains(new_bounds.min) && shape.bounds.contains(new_bounds.max) {
                // TODO: technically should compare with parent bvh node bounds...
                shape.bounds = new_bounds;
                return
            } else {
                #[cfg(todo)]
                {
                    // XXX: bvh has a tree depth limit that that makes in-place updates useless because they can unbalance the tree...
                    shape.bounds = new_bounds;
                    self.spatial_map
                        .bvh
                        .update_shapes(Some(&index), &mut self.spatial_map.shapes);
                    return
                }
            }
            shape.disable();
        }
        #[cfg(feature = "space-list")]
        match self.unpopulated.get_mut(index).map(|mut b| *b = true) {
            Some(()) => (),
            None => {
                self.unpopulated.resize(index + 1, false);
                let mut b = unsafe { self.unpopulated.last_mut().unwrap_unchecked() };
                *b = true;
            },
        }
    }
    #[cfg(todo)]
    pub fn disable_shape(&mut self, entity_idx: usize) {
        if let Some(s) = self.spatial_map.shape_for_entity_mut(entity_idx) {
            s.disable();
        }
    }
    pub fn disable(&mut self, index: usize) {
        if let Some(e) = self.entities.get_mut(index) {
            e.disable();
        }
    }

    /// Gets visible entities in the correct draw order.
    pub fn get_entities_for_drawing<'rs>(
        &'rs mut self,
        (cam_origin, cam_dir, _cam_up): RenderPosition,
        frustum: &'rs MapFrustum,
    ) -> impl Iterator<Item = &'rs RenderEntity> + 'rs {
        match () {
            #[cfg(feature = "space-list")]
            () => {
                let trailing = self
                    .unpopulated
                    .iter_ones()
                    .filter_map(|i| match self.entities.get(i) {
                        Some(e) if frustum.intersects(&e.bounds) => Some(i),
                        _ => None,
                    });
                self.draw_order_heap.clear();
                RenderOrderBuilder {
                    entities: &self.entities,
                    bvh_iter: self.spatial_map.select_visible_entities(frustum).chain(trailing),
                    draw_order_heap: &mut self.draw_order_heap,
                    cam_origin,
                    cam_dir,
                }
            },
            #[cfg(not(feature = "space-list"))]
            () => {
                let _ = cam_origin;
                let _ = cam_dir;
                self.visible_entities(frustum)
            },
        }
    }

    pub fn visible_entities<'rs>(
        &'rs self,
        frustum: &'rs MapFrustum,
    ) -> impl Iterator<Item = &'rs RenderEntity> + 'rs {
        match () {
            #[cfg(feature = "space-list")]
            () => {
                let entities = &self.entities;
                let trailing = self
                    .unpopulated
                    .iter_ones()
                    .filter_map(|i| match self.entities.get(i) {
                        Some(e) if frustum.intersects(&e.bounds) => Some(e),
                        _ => None,
                    });
                self.spatial_map
                    .select_visible_entities(frustum)
                    .filter_map(move |i| entities.get(i))
                    .chain(trailing)
            },
            #[cfg(not(feature = "space-list"))]
            () => {
                self.entities.iter()
                //.filter(move |e| frustum.intersects(&e.bounds)).rev()
            },
        }
    }

    pub fn map_entities<'rs>(
        &'rs self,
        bounds: Box3<DrawSpace>,
    ) -> impl Iterator<Item = &'rs RenderEntity> + 'rs {
        // TODO: select_visible_entities?
        self.entities.iter().filter(move |e| bounds.intersects(&e.bounds))
    }

    /// TODO: on drop, rebuild spatial map - for now just call [self.end_entities_mut()]
    pub fn entities_mut<'rs>(&'rs mut self) -> &'rs mut Vec<RenderEntity> {
        &mut self.entities
    }

    pub fn entities_mut_end(&mut self) {
        #[cfg(feature = "space-list")]
        {
            let unpop = self.unpopulated.count_ones();
            let pop_end = self.unpopulated.len().max(self.spatial_map.shapes.len());
            let trailing = self.entities.len().saturating_sub(pop_end);
            if unpop + trailing < 0x14 {
                // for a small number of additions, keep them implicit for now...
                // TODO: no need to populate these bits if you just append the trailing indices when iterating...
                if self.unpopulated.len() < self.spatial_map.shapes.len() {
                    self.unpopulated.resize(self.spatial_map.shapes.len(), false);
                }
                self.unpopulated.resize(self.entities.len(), true);
                return
            }
            let mut shapes = std::mem::take(&mut self.spatial_map.shapes);
            shapes.clear();
            shapes.reserve(self.entities.len());
            self.spatial_map = SpatialMap::build(&self.entities, shapes);
            self.unpopulated.clear();
        };
    }

    pub fn entities_count(&self) -> usize {
        self.entities.len()
    }
}

#[cfg(feature = "space-list")]
struct RenderOrderBuilder<'rs, BvhIter> {
    entities: &'rs [RenderEntity],
    bvh_iter: BvhIter,
    draw_order_heap: &'rs mut BinaryHeap<HeapEntity>,
    cam_origin: Point3<DrawSpace>,
    cam_dir: Vector3<DrawSpace>,
}

#[cfg(feature = "space-list")]
impl<'rs, BvhIter> Iterator for RenderOrderBuilder<'rs, BvhIter>
where
    BvhIter: Iterator<Item = usize>,
{
    type Item = &'rs RenderEntity;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(next) = self.bvh_iter.next() {
            let entity = &self.entities[next];
            let cam_dist = match entity.draw_ordered {
                false => return Some(entity),
                #[cfg(todo)]
                true => {
                    // TODO: broken or inaccurate idk
                    let cam_dist = (entity.position - self.cam_origin).dot(self.cam_dir);
                    let cam_dist = f32::to_bits(cam_dist) as i32;
                    let cam_dist = cam_dist ^ ((cam_dist >> 30) as u32 >> 1) as i32;
                    cam_dist
                },
                true => (entity.position.distance_squared(self.cam_origin) * 1_000_000.0)
                    .min(0x40000000i32 as f32) as i32,
            };
            self.draw_order_heap.push(HeapEntity { cam_dist, idx: next });
        }

        self.draw_order_heap.pop().map(|he| &self.entities[he.idx])
    }
}

#[cfg(feature = "space-list")]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct HeapEntity {
    cam_dist: i32,
    idx: usize,
}

#[cfg(feature = "space-list")]
struct RenderEntityShape {
    bounds: bvh::aabb::Aabb<f32, 3>,
    entity_idx: usize,
    bh_idx: usize,
}

#[cfg(feature = "space-list")]
impl RenderEntityShape {
    fn new((entity_idx, entity): (usize, &RenderEntity)) -> Self {
        RenderEntityShape {
            bounds: bvh::aabb::Aabb {
                min: [entity.bounds.min.x, entity.bounds.min.y, entity.bounds.min.z].into(),
                max: [entity.bounds.max.x, entity.bounds.max.y, entity.bounds.max.z].into(),
            },
            entity_idx,
            bh_idx: 0,
        }
    }
    fn disable(&mut self) {
        self.entity_idx = usize::MAX;
    }
    #[inline]
    pub fn is_disabled(&self) -> bool {
        self.entity_idx == usize::MAX
    }
}

#[cfg(feature = "space-list")]
impl Bounded<f32, 3> for RenderEntityShape {
    fn aabb(&self) -> bvh::aabb::Aabb<f32, 3> {
        self.bounds
    }
}

#[cfg(feature = "space-list")]
impl BHShape<f32, 3> for RenderEntityShape {
    fn set_bh_node_index(&mut self, bh_idx: usize) {
        self.bh_idx = bh_idx;
    }

    fn bh_node_index(&self) -> usize {
        self.bh_idx
    }
}

#[cfg(feature = "space-list")]
struct SpatialMap {
    shapes: Vec<RenderEntityShape>,
    bvh: Bvh<f32, 3>,
}

#[cfg(feature = "space-list")]
impl SpatialMap {
    /// TODO: filter out disabled entities, then [Self::shape_for_entity_mut] must binary search
    fn build(entities: &[RenderEntity], mut shapes: Vec<RenderEntityShape>) -> SpatialMap {
        shapes.extend(entities.iter().enumerate().map(RenderEntityShape::new));
        let bvh = Bvh::build(&mut shapes);
        SpatialMap { shapes, bvh }
    }

    pub fn select_visible_entities<'a>(
        &'a self,
        frustum: &'a MapFrustum,
    ) -> impl Iterator<Item = usize> + 'a {
        self.bvh
            .traverse_iterator(frustum, &self.shapes)
            .filter_map(|shape| (!shape.is_disabled()).then_some(shape.entity_idx))
    }
    fn shape_for_entity_mut(&mut self, entity_idx: usize) -> Option<&mut RenderEntityShape> {
        let shape = self.shapes.get_mut(entity_idx)?;
        (!shape.is_disabled()).then_some(shape)
    }
}

#[derive(Copy, Clone)]
#[cfg(todo)]
pub struct MapFrustum(pub [Vector4<DrawSpace>; 6]);

#[derive(Copy, Clone)]
pub struct MapFrustum {
    pub near: Vector4<DrawSpace>,
    #[cfg(feature = "space-list")]
    pub far: Vector4<DrawSpace>,
    pub left: Vector4<DrawSpace>,
    #[cfg(feature = "space-list")]
    pub right: Vector4<DrawSpace>,
    #[cfg(feature = "space-list")]
    pub up: Vector4<DrawSpace>,
    #[cfg(feature = "space-list")]
    pub down: Vector4<DrawSpace>,
}

impl MapFrustum {
    #[cfg(todo)]
    pub fn from_camera_data(
        data: &PerspectiveInputData,
        aspect_ratio: f32,
        near: f32,
        far: f32,
    ) -> MapFrustum {
        let fov = data.fov();
        let p = data.pos;
        let d = data.front.normalize();
        let right = d.cross(glam::Vec3::Y).normalize();
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
        let right_near = right * w_near / 2.0;

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

    #[cfg(todo)]
    pub fn planes(&self) -> &[Vector4<DrawSpace>; 6] {
        self.0
    }

    pub fn from_camera_data(
        (pos, camera_dir, camera_up): (Point3<DrawSpace>, Vector3<DrawSpace>, Vector3<DrawSpace>),
        // TODO: (aspect_ratio, fov): (f32, Vec2),
        Range { start: near, end: far }: Range<f32>,
    ) -> MapFrustum {
        // TODO: higher accuracy/correctness using fov and perspective idk
        let camera_far = camera_dir * far;
        let camera_near = camera_dir * near;

        let camera_dir_right = camera_dir.cross(camera_up).normalize();
        let camera_dir_up = camera_dir_right.cross(camera_dir).normalize();

        let near_focal_point = pos + camera_near;
        let fov_ratio = 3.0; // or 2.0 horiz? but 140 should be a reasonable enough max fov .-.
        let near_width2 = near * fov_ratio;
        let near_h = camera_dir_up * near_width2;
        let near_w = camera_dir_right * near_width2;

        let near_topleft = near_focal_point + near_h - near_w;
        let near_topright = near_topleft + near_w * 2.0;
        let near_bottomleft = near_topleft - near_h * 2.0;
        #[cfg(feature = "space-list")]
        let near_bottomright = near_bottomleft + near_w * 2.0;
        let near_plane = points_to_plane(
            near_topleft.to_raw(),
            near_topright.to_raw(),
            near_bottomleft.to_raw(),
        );

        let far_focal_point = pos + camera_far;
        let far_width2 = far * fov_ratio;
        let far_h = camera_dir_up * far_width2;
        let far_w = camera_dir_right * far_width2;

        let far_topleft = far_focal_point + far_h - far_w;
        #[cfg(feature = "space-list")]
        let far_topright = far_topleft + far_w * 2.0;
        #[cfg(feature = "space-list")]
        let far_bottomright = far_topright - far_h * 2.0;
        let far_bottomleft = far_topleft - far_h * 2.0;
        #[cfg(feature = "space-list")]
        let far_plane = points_to_plane(
            far_topright.to_raw(),
            far_topleft.to_raw(),
            far_bottomright.to_raw(),
        );

        let left_plane = points_to_plane(
            far_topleft.to_raw(),
            near_topleft.to_raw(),
            far_bottomleft.to_raw(),
        );
        #[cfg(feature = "space-list")]
        let right_plane = points_to_plane(
            far_topright.to_raw(),
            far_bottomright.to_raw(),
            near_topright.to_raw(),
        );

        #[cfg(feature = "space-list")]
        let up_plane = points_to_plane(far_topleft.to_raw(), far_topright.to_raw(), near_topleft.to_raw());
        #[cfg(feature = "space-list")]
        let down_plane = points_to_plane(
            far_bottomright.to_raw(),
            far_bottomleft.to_raw(),
            near_bottomright.to_raw(),
        );

        MapFrustum {
            near: near_plane.into(),
            #[cfg(feature = "space-list")]
            far: far_plane.into(),
            left: left_plane.into(),
            #[cfg(feature = "space-list")]
            right: right_plane.into(),
            #[cfg(feature = "space-list")]
            up: up_plane.into(),
            #[cfg(feature = "space-list")]
            down: down_plane.into(),
        }
    }

    const PLANES: usize = size_of::<MapFrustum>() / size_of::<Vector4>();
    pub fn planes(&self) -> &[Vector4<DrawSpace>; MapFrustum::PLANES] {
        unsafe { &*(self as *const Self as *const [Vector4<DrawSpace>; MapFrustum::PLANES]) }
    }
}

fn points_to_plane(p0: glam::Vec3, p1: glam::Vec3, p2: glam::Vec3) -> glam::Vec4 {
    let v = p1 - p0;
    let u = p2 - p0;
    let n = v.cross(u).normalize();
    let d = -n.dot(p0);
    glam::Vec4::new(n.x, n.y, n.z, d)
}

#[cfg(feature = "space-list")]
fn aabb_corners(aabb: &bvh::aabb::Aabb<f32, 3>) -> [Vector4<DrawSpace>; 8] {
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

#[cfg(feature = "space-list")]
impl IntersectsAabb<f32, 3> for MapFrustum {
    fn intersects_aabb(&self, aabb: &bvh::aabb::Aabb<f32, 3>) -> bool {
        let corners = aabb_corners(aabb);
        'plane: for plane in self.planes() {
            for corner in corners {
                // If any corner is inside this plane, move to the next.
                if plane.dot(corner) >= 0.0 {
                    continue 'plane;
                }
            }
            // All corners are outside this plane.
            return false;
        }
        true
    }
}

impl Intersection<Point3<DrawSpace>> for MapFrustum {
    type Intersection = bool;

    fn intersects(&self, thing: &Point3<DrawSpace>) -> bool {
        self.intersection(thing) == Some(true)
    }

    fn intersection(&self, thing: &Point3<DrawSpace>) -> Option<Self::Intersection> {
        let point = thing.extend(1.0);
        for plane in self.planes() {
            if plane.dot(point.into()) < 0.0 {
                return Some(false)
            }
        }

        Some(true)
    }
}

impl Intersection<Box3<DrawSpace>> for MapFrustum {
    type Intersection = ();

    fn intersects(&self, thing: &Box3<DrawSpace>) -> bool {
        self.intersection(thing).is_some()
    }

    fn intersection(&self, thing: &Box3<DrawSpace>) -> Option<Self::Intersection> {
        let corners = thing.corners();
        let corners = corners.iter().map(|c| c.to_raw().extend(1.0));
        self.planes()
            .iter()
            .map(|p| glam::Vec4::from(*p))
            .all(|p| corners.clone().any(|c| p.dot(c) >= 0.0))
            .then_some(())
    }
}
