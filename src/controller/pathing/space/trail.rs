use {
    crate::{resources::Vertex, space::TextureSpace},
    bitvec::vec::BitVec,
    core::f32,
    glam::Vec3A,
    glamour::{Point2, Vec3Swizzles, Vector3},
    taimi_meta::coords::MapLocalScale,
    taimi_pack::trail::TrailSection,
};

#[derive(Debug, Clone)]
pub struct TrailParams {
    pub resolution: Option<f32>,
    pub width: f32,
    pub y_offset: f32,
    pub smoothing: Option<Option<f32>>,
}

impl TrailParams {
    /// Current hardcoded value in BlishHUD Pathing
    pub const DEFAULT: Self = Self {
        resolution: None,
        width: Self::DEFAULT_WIDTH,
        y_offset: 0.0,
        smoothing: None,
    };

    pub const DEFAULT_SMOOTHING: f32 = 5.5;
    pub const DEFAULT_RESOLUTION: f32 = 1.0 / 20.0;
    pub const DEFAULT_WIDTH: f32 = Self::WIDTH_FACTOR / Self::DEFAULT_RESOLUTION;
    pub const WIDTH_FACTOR: f32 = MapLocalScale::METRES_PER_INCH * 2.0;

    pub fn width(&self) -> f32 {
        self.width
    }

    pub fn smoothing(&self) -> Option<f32> {
        self.smoothing.unwrap_or_else(|| {
            (self.resolution() > Self::DEFAULT_RESOLUTION).then_some(Self::DEFAULT_SMOOTHING)
        })
    }

    pub fn resolution(&self) -> f32 {
        self.resolution
            .unwrap_or_else(|| Self::WIDTH_FACTOR / self.width())
    }

    pub fn y_offset_for(&self, idx: usize) -> f32 {
        //self.y_offset * (0.2 + (idx as f32 * f32::EPSILON * 100.0) % 0.8f32)
        let scale = 0x4000;
        let idx = idx % scale;
        self.y_offset * (0.5 + (idx as f32 * 0.5 / scale as f32))
    }

    /// mitigate z-fighting by fudging y values for (hopefully) unique trails
    #[cfg(todo)]
    pub fn y_offset_for_trail(&self, pack: &LoadedPack, path: TrailPath) -> f32 {
        if self.y_offset == 0.0 {
            return 0.0
        }
        let pack_signature = pack.trails.len() + pack.pois.len() + pack.categories.all_categories.len();
        self.y_offset_for(pack_signature ^ (path.path.wrapping_mul(73)))
    }

    pub const Y_OFFSET_SECTION_GAP: f32 = f32::EPSILON * 40.0;

    pub fn bake(&self) -> Self {
        Self {
            resolution: Some(self.resolution()),
            smoothing: Some(self.smoothing()),
            width: self.width(),
            y_offset: self.y_offset,
        }
    }
}

impl Default for TrailParams {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Expansion outward from trail edges
///
/// Pair with [TrailTextureMap::set_scale_from_expansion] for it to look
/// remotely natural.
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct TrailScale {
    pub normal_expansion: f32,
}

impl TrailScale {
    /// No expansion, standard sizing
    pub const DEFAULT: Self = Self::new(0.0);
    /// Invalid setting that will always require refreshing parameters
    pub const DIRTY: Self = Self::new(f32::NAN);

    pub const fn new(normal_expansion: f32) -> Self {
        Self { normal_expansion }
    }

    /// Convert from settings
    pub const fn with_scale(trail_scale: f32) -> Self {
        Self::new((trail_scale - 1.0) / 2.0)
    }

    pub const fn scale(&self) -> f32 {
        self.normal_expansion * 2.0 + 1.0
    }
}

impl Default for TrailScale {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
#[repr(C)]
pub struct TrailTextureMap {
    /// V coordinate offset
    pub v_offset: f32,
    /// V coordinate scaling
    pub v_scale: f32,
}

impl TrailTextureMap {
    pub const DEFAULT: Self = Self::new(1.0, 0.0);
    pub const UNTEXTURED: Self = Self::new(0.0, Self::UNTEXTURED_ANCHOR.y);
    pub const UNTEXTURED_ANCHOR: Point2<TextureSpace> = Point2::new(0.0, 0.39);

    pub const fn new(v_scale: f32, v_offset: f32) -> Self {
        Self { v_scale, v_offset }
    }

    pub const fn with_tex_scale(v_scale: f32) -> Self {
        Self { v_scale, ..Self::DEFAULT }
    }

    pub fn set_scale_from_expansion(&mut self, scale: TrailScale) {
        let TrailScale { normal_expansion } = scale;
        let scale_trail_norm = match () {
            #[cfg(todo)]
            _ => (normal_expansion + 10.0 / 5.2) * -0.52 + 2.0,
            () => {
                let (e0, e1) = match () {
                    #[cfg(todo)]
                    _ => (2.38206f32, -0.45979f32),
                    _ => (2.22149f32, -0.388849f32),
                };
                let scalex = normal_expansion * 1.5;
                (e1 * (scalex + 2.0)).exp() * e0
            },
        };
        self.v_scale = scale_trail_norm.clamp(0.04, 0.99);
    }
}

impl Default for TrailTextureMap {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl TrailParams {
    /// Interpolate points to be no more than 1/resolution metres apart.
    ///
    /// TODO: colour is a hack,
    /// switch to compact vertex layout and provide as instance data instead
    pub fn interpolate_section_vertices(
        &self,
        vertices: &mut Vec<Vertex>,
        mut map_vertices: Option<&mut Vec<Vertex>>,
        section: &TrailSection,
        scale: f32,
        is_wall: bool,
        colour: Vector3<f32>,
        // TODO: map_colour? other params?
    ) {
        if section.points.is_empty() {
            return
        }
        let colour = colour.to_raw();
        let width = self.width();
        let resolution = self.resolution();
        // amount of distance^2 that spans 1/res
        // (point at which we want to split into multiple segments
        let dist_dist_threshold = resolution.recip().powi(2);
        let smoothing = self.smoothing();
        let mut points = Vec::with_capacity(section.points.len());
        let mut prev_point = None;
        let section_points = section.points.iter().map(|p| p.to_vec3a());
        let mut map_points: Option<BitVec> = map_vertices
            .is_some()
            .then(|| BitVec::with_capacity(section.points.len()));
        for mut point in section_points {
            point.y += self.y_offset;

            if let Some(prev_point) = prev_point.replace(point) {
                let dist_dist = prev_point.distance_squared(point);
                let segments = if dist_dist < dist_dist_threshold {
                    0u32
                } else {
                    (dist_dist.sqrt() * resolution) as u32
                };
                for i in 0..segments {
                    let s = (i + 1) as f32 / (segments + 1) as f32;
                    let position = match smoothing {
                        None => s,
                        // bias resolution near corners
                        Some(smoothing) => s.powi(if smoothing > 6.0 { 3 } else { 2 }),
                    };
                    let int_point = prev_point.lerp(point, position);
                    if let Some(mp) = &mut map_points {
                        let is_mp = match i {
                            #[cfg(todo)]
                            i => i & 1 == 1,
                            i => i & 1 == 0 && i > 0,
                        };
                        mp.push(is_mp);
                    }
                    points.push(int_point);
                }
            }
            if let Some(mp) = &mut map_points {
                mp.push(true);
            }
            points.push(point);
        }

        if let Some(smoothing) = smoothing {
            let mut points = &mut points[..];
            while let &mut [prev, mid, ..] = points {
                let next = points.get(2).copied().unwrap_or(mid);
                let target = prev.slerp(next, 0.5);
                let smooth = mid.xz().lerp(target.xz(), smoothing / 10.0);
                points[1] = smooth.extend(mid.y * 0.925 + target.y * 0.075).xzy().into();
                points = &mut points[1..];
            }
        }

        let mut cur_point = points[0];
        let mut last_offset = Vec3A::ZERO;
        let mut flip_over = 1.0f32;
        let normal_offset = width * scale / 2.0;
        let normal_scale_fallback = match () {
            #[cfg(todo = "unnecessary")]
            _ => Vec3A::new(1.0, 0.0, 1.0).normalize(),
            _ => {
                use core::f32::consts::SQRT_2;
                Vec3A::new(SQRT_2, 0.0, SQRT_2)
            },
        };
        // TODO: should map walls adjust normal scale so they're visually distinct or no?
        const MAP_NORMAL_SCALE: f32 = 1.4f32;
        let mut mod_distance = Vec3A::ZERO;
        // this shouldn't really be needed...
        let mut path_direction = Vec3A::ZERO;

        vertices.reserve(points.len() * 2);
        if let (Some(map_vertices), Some(map_points)) = (&mut map_vertices, &map_points) {
            map_vertices.reserve(map_points.count_ones() * 2);
        }
        let mut distance = 0.0f32;
        let mut map_points = map_points.into_iter().flat_map(|mp| mp.into_iter());
        for next_point in points.into_iter().skip(1) {
            let map_point = map_points.next();
            path_direction = next_point - cur_point;
            let flat_offset = path_direction.cross(Vec3A::Y);
            let offset = if is_wall { path_direction.cross(flat_offset) } else { flat_offset };
            let offset = offset.normalize();

            if last_offset != Vec3A::ZERO && offset.dot(last_offset) < 0.0 {
                flip_over *= -1.0;
            }

            mod_distance = offset * normal_offset * flip_over;
            let normal_scale_dir =
                mod_distance.normalize_or(Vec3A::new(1.0, 0.0, 1.0).normalize().copysign(mod_distance));

            let v0 = Vertex {
                position: (cur_point - mod_distance).into(),
                colour,
                normal: (-normal_scale_dir).into(),
                texture: glam::vec2(1.0, distance / width - 1.0),
            };
            let v1 = Vertex {
                position: (cur_point + mod_distance).into(),
                colour,
                normal: normal_scale_dir.into(),
                texture: glam::vec2(0.0, distance / width - 1.0),
            };
            if let (Some(true), Some(map_vertices)) = (map_point, &mut map_vertices) {
                let v01 = if is_wall {
                    let offset = flat_offset.normalize();
                    let map_distance = offset * normal_offset * flip_over * MAP_NORMAL_SCALE;
                    let normal_scale_dir =
                        map_distance.normalize_or(normal_scale_fallback.copysign(map_distance));
                    [
                        Vertex {
                            position: (cur_point - map_distance).into(),
                            colour,
                            normal: (-normal_scale_dir).into(),
                            texture: v0.texture,
                        },
                        Vertex {
                            position: (cur_point + map_distance).into(),
                            colour,
                            normal: normal_scale_dir.into(),
                            texture: v1.texture,
                        },
                    ]
                } else {
                    [v0.clone(), v1.clone()]
                };
                map_vertices.extend(v01);
            }
            vertices.extend([v0, v1]);

            distance += path_direction.length();
            last_offset = offset;
            cur_point = next_point;
        }

        let normal_scale_dir = mod_distance.normalize_or(normal_scale_fallback.copysign(mod_distance));
        let v0 = Vertex {
            position: (cur_point - mod_distance).into(),
            colour,
            normal: (-normal_scale_dir).into(),
            texture: glam::vec2(1.0, distance / width - 1.0),
        };
        let v1 = Vertex {
            position: (cur_point + mod_distance).into(),
            colour,
            normal: normal_scale_dir.into(),
            texture: glam::vec2(0.0, distance / width - 1.0),
        };
        if let Some(map_vertices) = &mut map_vertices {
            let vend = if is_wall {
                let flat_offset = path_direction.cross(Vec3A::Y);
                let offset = flat_offset.normalize();
                let map_distance = offset * normal_offset * flip_over * MAP_NORMAL_SCALE;
                let normal_scale_dir =
                    map_distance.normalize_or(normal_scale_fallback.copysign(map_distance));
                [
                    Vertex {
                        position: (cur_point - map_distance).into(),
                        colour,
                        normal: (-normal_scale_dir).into(),
                        texture: v0.texture,
                    },
                    Vertex {
                        position: (cur_point + map_distance).into(),
                        colour,
                        normal: normal_scale_dir.into(),
                        texture: v1.texture,
                    },
                ]
            } else {
                [v0.clone(), v1.clone()]
            };
            map_vertices.extend(vend);
        }
        vertices.extend([v0, v1]);
    }
}
