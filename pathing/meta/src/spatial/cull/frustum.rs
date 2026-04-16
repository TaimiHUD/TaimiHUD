use glamour::{Box3, Intersection};
use {
    crate::coords::LocalSpace as DrawSpace,
    crate::spatial::{
        aabb3box,
        cull::BvhQuery,
        MintConv,
    },
    bvh::aabb::{Aabb, IntersectsAabb},
    core::{mem, ops::{self, Range}},
    glamour::{Point3, Vector3},
    glam::{Vec3A, Vec4},
};

#[cfg(todo)]
pub type FrustumPlane = glamour::Vector4<DrawSpace>;
pub type FrustumPlane = Vec4;
pub type FrustumCorners = [FrustumPlane; 8];
#[derive(Copy, Clone)]
#[repr(C)]
pub struct MapFrustum {
    pub near: FrustumPlane,
    #[cfg(feature = "spatial")]
    pub far: FrustumPlane,
    pub left: FrustumPlane,
    #[cfg(feature = "spatial")]
    pub right: FrustumPlane,
    #[cfg(feature = "spatial")]
    pub up: FrustumPlane,
    #[cfg(feature = "spatial")]
    pub down: FrustumPlane,
    #[cfg(feature = "spatial")]
    pub corners: FrustumCorners,
    pub camera_up: Vec3A,
    pub camera_right: Vec3A,
}

impl MapFrustum {
    /// not working?
    pub fn from_camera_data(
        fov: f32,
        (pos, front, camera_up): (Point3<DrawSpace>, Vector3<DrawSpace>, Vector3<DrawSpace>),
        aspect_ratio: f32,
        depth: Range<f32>,
    ) -> Self {
        let p = pos.to_vec3a();
        let d = front.to_vec3a();
        #[cfg(todo = "unnecessary")]
        let d = d.normalize();
        let right = d.cross(camera_up.to_vec3a()).normalize();
        let up = right.cross(d).normalize();
        let tan_fov22 = (fov / 2.0).tan() * 2.0;
        Self::from_camera_data3a(tan_fov22, p, d, right, up, aspect_ratio, depth)
    }
    #[cfg(todo)]
    pub fn from_look(
        look: glam::Mat4,
    ) -> Self {
    }
    pub fn from_camera_data3a(
        tan_fov22: f32,
        p: Vec3A,
        d: Vec3A,
        right: Vec3A,
        up: Vec3A,
        aspect_ratio: f32,
        Range { start: near, end: far }: Range<f32>,
    ) -> Self {
        let h_near = tan_fov22 * near;
        let w_near = h_near * aspect_ratio;
        let h_far = tan_fov22 * far;
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

        let corners @ [
            ftr, ftl, fbr, fbl,
            ntr, ntl, nbr, nbl,
        ] = [
            ftr.extend(1.0), ftl.extend(1.0), fbr.extend(1.0), fbl.extend(1.0),
            ntr.extend(1.0), ntl.extend(1.0), nbr.extend(1.0), nbl.extend(1.0),
        ];

        let near_plane = points_to_plane4(ntl, ntr, nbl);
        let far_plane = points_to_plane4(ftr, ftl, fbr);
        let up_plane = points_to_plane4(ftl, ftr, ntl);
        let down_plane = points_to_plane4(fbr, fbl, nbr);
        let right_plane = points_to_plane4(ftr, fbr, ntr);
        let left_plane = points_to_plane4(ftl, ntl, fbl);

        Self {
            camera_up: up,
            camera_right: right,
            near: near_plane.into(),
            #[cfg(feature = "spatial")]
            far: far_plane.into(),
            left: left_plane.into(),
            #[cfg(feature = "spatial")]
            right: right_plane.into(),
            #[cfg(feature = "spatial")]
            up: up_plane.into(),
            #[cfg(feature = "spatial")]
            down: down_plane.into(),
            #[cfg(feature = "spatial")]
            corners,
        }
    }

    /// a horrible hack...
    #[cfg(todo)]
    pub fn from_camera_data(
        (pos, camera_dir, camera_up): (Point3<DrawSpace>, Vector3<DrawSpace>, Vector3<DrawSpace>),
        // TODO: (aspect_ratio, fov): (f32, Vec2),
        Range { start: near, end: far }: Range<f32>,
    ) -> Self {
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
        #[cfg(feature = "spatial")]
        let near_bottomright = near_bottomleft + near_w * 2.0;
        let near_plane = points_to_plane(
            near_topleft.to_vec3a(),
            near_topright.to_vec3a(),
            near_bottomleft.to_vec3a(),
        );

        let far_focal_point = pos + camera_far;
        let far_width2 = far * fov_ratio;
        let far_h = camera_dir_up * far_width2;
        let far_w = camera_dir_right * far_width2;

        let far_topleft = far_focal_point + far_h - far_w;
        #[cfg(feature = "spatial")]
        let far_topright = far_topleft + far_w * 2.0;
        #[cfg(feature = "spatial")]
        let far_bottomright = far_topright - far_h * 2.0;
        let far_bottomleft = far_topleft - far_h * 2.0;
        #[cfg(feature = "spatial")]
        let far_plane = points_to_plane(
            far_topright.to_vec3a(),
            far_topleft.to_vec3a(),
            far_bottomright.to_vec3a(),
        );

        let left_plane = points_to_plane(
            far_topleft.to_vec3a(),
            near_topleft.to_vec3a(),
            far_bottomleft.to_vec3a(),
        );
        #[cfg(feature = "spatial")]
        let right_plane = points_to_plane(
            far_topright.to_vec3a(),
            far_bottomright.to_vec3a(),
            near_topright.to_vec3a(),
        );

        #[cfg(feature = "spatial")]
        let up_plane = points_to_plane(far_topleft.to_vec3a(), far_topright.to_vec3a(), near_topleft.to_vec3a());
        #[cfg(feature = "spatial")]
        let down_plane = points_to_plane(
            far_bottomright.to_vec3a(),
            far_bottomleft.to_vec3a(),
            near_bottomright.to_vec3a(),
        );

        Self {
            near: near_plane.into(),
            #[cfg(feature = "spatial")]
            far: far_plane.into(),
            left: left_plane.into(),
            #[cfg(feature = "spatial")]
            right: right_plane.into(),
            #[cfg(feature = "spatial")]
            up: up_plane.into(),
            #[cfg(feature = "spatial")]
            down: down_plane.into(),
            #[cfg(feature = "spatial")]
            corners: [
                far_topright, far_topleft, far_bottomright, far_bottomleft,
                near_topright, near_topleft, near_bottomright, near_bottomleft,
            ],
        }
    }

    #[inline]
    pub fn camera_dir(&self) -> Vec3A {
        Vec3A::from_vec4(self.near)
    }

    const PLANES: usize = 6;
    #[inline(always)]
    pub fn planes(&self) -> &[FrustumPlane; MapFrustum::PLANES] {
        unsafe { &*(self as *const Self as *const [FrustumPlane; MapFrustum::PLANES]) }
    }

    #[inline]
    pub fn planes_intersect_aabb(&self, aabb: &Aabb<f32, 3>) -> bool {
        Self::planes_intersect_all(self.planes().iter().copied(), aabb_corners(aabb))
    }
    #[inline(always)]
    fn planes_intersect_all<P, C>(planes: P, corners: C) -> bool where
        P: IntoIterator<Item = Vec4>,
        C: IntoIterator<Item = Vec4> + Clone,
    {
        planes.into_iter().all(|plane|
            // If any corner is inside this plane, move to the next.
            corners.clone().into_iter().any(|corner| plane.dot(corner) >= 0.0)
        )
    }
    #[cfg(todo = "unnecessary")]
    pub fn aabb_axis_intersection_filter(&self, aabb: &Aabb<f32, 3>) -> bool {
        if self.corners.iter().all(|corner| corner.x < aabb.min.x) { return false }
        if self.corners.iter().all(|corner| corner.x > aabb.max.x) { return false }
        if self.corners.iter().all(|corner| corner.y < aabb.min.y) { return false }
        if self.corners.iter().all(|corner| corner.y > aabb.max.y) { return false }
        if self.corners.iter().all(|corner| corner.z < aabb.min.z) { return false }
        if self.corners.iter().all(|corner| corner.z > aabb.max.z) { return false }
        true
    }
    /// all corners of our frustum are outside any plane of the aabb...
    pub fn aabb_axis_intersection_filter(&self, aabb: &Aabb<f32, 3>) -> bool {
        let aabb = aabb3box::<f32>(*aabb);
        let min = aabb.min.to_vec3a();
        let below = self.corners.iter().map(|&corner| Vec3A::from_vec4(corner).cmpge(min))
            .fold(glam::BVec3A::FALSE, |prev, cmp| prev | cmp);
        if !below.any() { return false }
        let max = aabb.max.to_vec3a();
        let above = self.corners.iter().map(|&corner| Vec3A::from_vec4(corner).cmple(max))
            .fold(glam::BVec3A::FALSE, |prev, cmp| prev | cmp);
        if !above.any() { return false }
        true
    }
}

impl IntersectsAabb<f32, 3> for MapFrustum {
    fn intersects_aabb(&self, aabb: &Aabb<f32, 3>) -> bool {
        let corners = aabb_corners(aabb);
        let outside = !Self::planes_intersect_all(self.planes().iter().copied(), corners.iter().copied());
        if outside {
            // All corners are outside this plane.
            return false
        }
        // filter out false positives
        self.aabb_axis_intersection_filter(aabb)
    }
}
impl BvhQuery<3> for MapFrustum {
    fn intersects_aabb_shape(&self, aabb: &Aabb<f32, 3>) -> bool {
        self.planes_intersect_aabb(aabb)
    }
    /// TODO: can simplify near/far plane checks if we assume billboard?
    fn intersects_aabb_poi(&self, aabb: &Aabb<f32, 3>) -> bool {
        let pos = glamour::Point3::<f32>::from_nalg(aabb.center()).to_vec3a();
        let icon_size = (aabb.max.x - aabb.min.x).powi(2) * 0.5;
        let right = self.camera_right * icon_size;
        let up = self.camera_up * icon_size;
        let corners = [
            (pos + up - right).extend(1.0),
            (pos - up - right).extend(1.0),
            (pos - up + right).extend(1.0),
            (pos + up + right).extend(1.0),
        ];
        Self::planes_intersect_all(self.planes().iter().copied(), corners)
    }
}

#[inline(always)]
fn points_to_plane4(p0: Vec4, p1: Vec4, p2: Vec4) -> Vec4 {
    points_to_plane(
        Vec3A::from_vec4(p0),
        Vec3A::from_vec4(p1),
        Vec3A::from_vec4(p2),
    )
}
fn points_to_plane(p0: Vec3A, p1: Vec3A, p2: Vec3A) -> Vec4 {
    let v = p1 - p0;
    let u = p2 - p0;
    let n = v.cross(u).normalize();
    let d = -n.dot(p0);
    n.extend(d)
}
#[cfg(feature = "spatial")]
fn aabb_corners(aabb: &Aabb<f32, 3>) -> [FrustumPlane; 8] {
    [
        FrustumPlane::new(aabb.min.x, aabb.min.y, aabb.min.z, 1.0),
        FrustumPlane::new(aabb.max.x, aabb.min.y, aabb.min.z, 1.0),
        FrustumPlane::new(aabb.min.x, aabb.max.y, aabb.min.z, 1.0),
        FrustumPlane::new(aabb.max.x, aabb.max.y, aabb.min.z, 1.0),
        FrustumPlane::new(aabb.min.x, aabb.min.y, aabb.max.z, 1.0),
        FrustumPlane::new(aabb.max.x, aabb.min.y, aabb.max.z, 1.0),
        FrustumPlane::new(aabb.min.x, aabb.max.y, aabb.max.z, 1.0),
        FrustumPlane::new(aabb.max.x, aabb.max.y, aabb.max.z, 1.0),
    ]
}
#[cfg(todo)]
fn aabb_corners(aabb: &Aabb<f32, 3>) -> [FrustumPlane; 8] {
    let aabb = aabb3box::<DrawSpace>(*aabb);
    let min = aabb.min.to_vec3a().extend(1.0);
    let max = aabb.max.to_vec3a().extend(1.0);
    [
        min,
        min.with_x(max.x),
        min.with_y(max.y),
        max.with_z(min.z),
        min.with_z(max.z),
        max.with_y(min.y),
        max.with_x(min.x),
        max,
    ]
}

#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct LazyFrustum(pub MapFrustum);
impl LazyFrustum {
    pub const fn from_ref(frustum: &MapFrustum) -> &Self {
        unsafe {
            mem::transmute(frustum)
        }
    }
    #[inline]
    pub fn min_planes(&self) -> [&FrustumPlane; 2] {
        [
            &self.near,
            &self.left,
        ]
    }
}
impl ops::Deref for LazyFrustum {
    type Target = MapFrustum;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl IntersectsAabb<f32, 3> for LazyFrustum {
    fn intersects_aabb(&self, aabb: &Aabb<f32, 3>) -> bool {
        let corners = aabb_corners(aabb);
        MapFrustum::planes_intersect_all(IntoIterator::into_iter(self.min_planes()).copied(), corners)
    }
}

impl Intersection<Point3<DrawSpace>> for LazyFrustum {
    type Intersection = bool;

    fn intersects(&self, thing: &Point3<DrawSpace>) -> bool {
        self.intersection(thing) == Some(true)
    }

    fn intersection(&self, thing: &Point3<DrawSpace>) -> Option<Self::Intersection> {
        let point = thing.extend(1.0);
        for plane in self.min_planes() {
            if plane.dot(point.into()) < 0.0 {
                return Some(false)
            }
        }

        Some(true)
    }
}
impl Intersection<Aabb<f32, 3>> for LazyFrustum {
    type Intersection = bool;

    fn intersects(&self, thing: &Aabb<f32, 3>) -> bool {
        self.intersection(thing) == Some(true)
    }

    fn intersection(&self, thing: &Aabb<f32, 3>) -> Option<Self::Intersection> {
        self.intersection(&aabb3box(*thing))
    }
}

impl Intersection<Box3<DrawSpace>> for LazyFrustum {
    type Intersection = bool;

    fn intersects(&self, thing: &Box3<DrawSpace>) -> bool {
        self.intersection(thing) == Some(true)
    }

    fn intersection(&self, thing: &Box3<DrawSpace>) -> Option<Self::Intersection> {
        if self.intersects(&thing.min) || self.intersects(&thing.max) {
            Some(true)
        } else {
            Some(false)
        }
    }
}
