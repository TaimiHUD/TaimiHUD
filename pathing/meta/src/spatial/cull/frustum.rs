#[cfg(feature = "spatial")]
use glamour::vec4;
#[cfg(not(feature = "spatial"))]
use glamour::{vec4, Box3, Intersection};
use {
    crate::coords::LocalSpace as DrawSpace,
    bvh::aabb::IntersectsAabb,
    core::ops::Range,
    glamour::{Point3, Vector3, Vector4},
};

#[derive(Copy, Clone)]
pub struct MapFrustum {
    pub near: Vector4<DrawSpace>,
    #[cfg(feature = "spatial")]
    pub far: Vector4<DrawSpace>,
    pub left: Vector4<DrawSpace>,
    #[cfg(feature = "spatial")]
    pub right: Vector4<DrawSpace>,
    #[cfg(feature = "spatial")]
    pub up: Vector4<DrawSpace>,
    #[cfg(feature = "spatial")]
    pub down: Vector4<DrawSpace>,
}

impl MapFrustum {
    /// not working?
    pub fn from_camera_data_orig(
        (fov, p, front): (f32, Point3<DrawSpace>, Vector3<DrawSpace>),
        aspect_ratio: f32,
        near: f32,
        far: f32,
    ) -> Self {
        #[cfg(todo)]
        let (fov, p, d) = (perspectiveinputdata.fov(), data.pos, data.front);
        let p = p.to_raw();
        let d = front.normalize().to_raw();
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
        }
    }

    #[cfg(todo)]
    pub fn planes(&self) -> &[Vector4<DrawSpace>; 6] {
        self.0
    }

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
            near_topleft.to_raw(),
            near_topright.to_raw(),
            near_bottomleft.to_raw(),
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
            far_topright.to_raw(),
            far_topleft.to_raw(),
            far_bottomright.to_raw(),
        );

        let left_plane = points_to_plane(
            far_topleft.to_raw(),
            near_topleft.to_raw(),
            far_bottomleft.to_raw(),
        );
        #[cfg(feature = "spatial")]
        let right_plane = points_to_plane(
            far_topright.to_raw(),
            far_bottomright.to_raw(),
            near_topright.to_raw(),
        );

        #[cfg(feature = "spatial")]
        let up_plane = points_to_plane(far_topleft.to_raw(), far_topright.to_raw(), near_topleft.to_raw());
        #[cfg(feature = "spatial")]
        let down_plane = points_to_plane(
            far_bottomright.to_raw(),
            far_bottomleft.to_raw(),
            near_bottomright.to_raw(),
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

#[cfg(feature = "spatial")]
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

#[cfg(feature = "spatial")]
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

#[cfg(not(feature = "spatial"))]
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

#[cfg(not(feature = "spatial"))]
impl Intersection<Box3<DrawSpace>> for MapFrustum {
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
