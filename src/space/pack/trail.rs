use {
    crate::{
        exports::runtime::Counter,
        space::{
            pack::{ActivePack, TrailSectionExt},
            resources::{Model, Texture, Vertex},
            DrawSpace,
            TextureSpace,
        },
    },
    anyhow::Context,
    core::f32,
    glam::Vec3,
    glamour::{Box3, Point2, Point3, Vec3Swizzles, Vector3},
    std::sync::Arc,
    taimi_d3d::dx11::{buffer::VertexBuffer, prelude::*},
    taimi_meta::ui::LocalContext,
    taimi_pack::Trail,
};

pub struct ActiveTrail {
    pub trail_idx: usize,
    pub category_idx: usize,
    pub filtered: bool,
    pub render_bookmark: usize,

    // Segment data.
    pub section_bounds: Vec<Box3<DrawSpace>>,

    // World render data.
    pub texture: Arc<Texture>,
    pub section_vbuffer: VertexBuffer,
    pub section_bookmarks: Vec<u32>,

    pub y_offset: f32,
}

impl ActiveTrail {
    pub fn build(
        loader: &mut ActivePack,
        trail: &Trail,
        trail_idx: usize,
        category_idx: usize,
        params: &TrailParams,
        render_bookmark: usize,
        device: &Dx11Device,
    ) -> anyhow::Result<ActiveTrail> {
        let colour = trail.attributes.render().tint().truncate();
        let trail_width = params.width();
        let resolution = params.resolution();
        let smoothing = params.smoothing();
        let map_only = trail.attributes.in_game_visibility == Some(false);
        let is_wall = trail.is_wall() && {
            // geometry is shared between space and maps, so a paper-thin
            // vertical wall is meaningless if not intended to show in-game
            // (heart boundaries sets this combo)
            !map_only
        };
        let mut y_offset = {
            // mitigate z-fighting by fudging y values for (hopefully) unique trails
            let pack_signature = loader.pack.trails.len()
                + loader.pack.pois.len()
                + loader.pack.categories.all_categories.len();
            params.y_offset_for(pack_signature ^ (trail_idx.wrapping_mul(73)))
        };

        let texture_handle = trail
            .texture_name()
            .ok_or_else(|| anyhow::anyhow!("TODO: Add a fallback texture for trails"))?;
        let texture_handle = loader.register_texture(texture_handle);
        let trail_data = trail
            .read_trl_data(loader.loader())
            .context("Loading trail vertices")?;
        let texture = loader
            .get_or_load_texture(texture_handle, device)
            .context("Loading trail texture")?;

        let mut vertices: Vec<Vertex> = Vec::new();
        let mut section_bookmarks: Vec<u32> = vec![0];
        let mut section_bounds = Vec::new();

        for (isec, section) in trail_data.sections.iter().enumerate() {
            y_offset = (y_offset - f32::EPSILON * 40.0).max(0.0);

            if section.points.is_empty() {
                log::debug!("Section {isec} is empty.");
                continue;
            }

            // Interpolate points to be no more than 1/resolution metres apart.
            let mut points = Vec::with_capacity(section.points.len());
            let mut prev_point = None;
            for mut point in section.points.iter().copied() {
                point.y += y_offset;

                if let Some(prev_point) = prev_point.replace(point) {
                    let dist = prev_point.distance(point);
                    let segments = (dist * resolution) as i32;
                    for i in 0..segments {
                        let s = (i + 1) as f32 / (segments + 1) as f32;
                        let position = match smoothing {
                            None => s,
                            // bias resolution near corners
                            Some(smoothing) => s.powi(if smoothing > 6.0 { 3 } else { 2 }),
                        };
                        let int_point = prev_point.lerp(point, position);
                        points.push(int_point);
                    }
                }
                points.push(point);
            }

            if let Some(smoothing) = smoothing {
                let mut points = &mut points[..];
                while let &mut [prev, mid, ..] = points {
                    let next = points.get(2).copied().unwrap_or(mid);
                    let target = prev.slerp(next, 0.5);
                    let smooth = mid.xz().lerp(target.xz(), smoothing / 10.0);
                    points[1] = smooth.extend(mid.y * 0.925 + target.y * 0.075).xzy();
                    points = &mut points[1..];
                }
            }

            log::trace!(
                "Section {isec} added {} interpolation points ({} -> {}).",
                points.len() - section.points.len(),
                section.points.len(),
                points.len(),
            );

            Self::gen_points(
                &mut vertices,
                &points[..],
                trail_width,
                trail.scale(),
                is_wall,
                colour,
            );

            section_bookmarks.push(vertices.len() as u32);
            let bounds = match section.bounds() {
                bounds if !map_only => bounds,
                mut bounds => {
                    const MIN_MAP_HEIGHT_2: f32 = 80.0f32;
                    // relax vertical cull range for trails that aren't in space
                    // (heart boundaries sets all y values to 0 no matter how high the terrain is)
                    let mid_y = (bounds.max.y + bounds.min.y) * 0.5;
                    bounds.min.y = bounds.min.y.min(mid_y - MIN_MAP_HEIGHT_2);
                    bounds.max.y = bounds.max.y.max(mid_y + MIN_MAP_HEIGHT_2);
                    bounds
                },
            };
            section_bounds.push(bounds);
        }

        if vertices.is_empty() {
            log::info!("Empty trail {}:{}", trail.category, trail.guid,);
        }

        let model = Model::from_vertices(vertices);
        let section_vbuffer = model.to_buffer(device).context("Creating trail vbuffer")?;
        STATS_TRAIL_VERTEX_SIZE.increment_by(|| section_vbuffer.size());

        Ok(ActiveTrail {
            trail_idx,
            category_idx,
            filtered: false,
            section_bounds,
            texture: texture.clone(),
            section_vbuffer,
            section_bookmarks,
            render_bookmark,
            y_offset,
        })
    }

    pub(crate) fn gen_points(
        vertices: &mut Vec<Vertex>,
        points: &[Point3],
        trail_width: f32,
        trail_scale: f32,
        is_wall: bool,
        colour: Vec3,
    ) {
        let mut cur_point = points[0];
        let mut last_offset = Vector3::ZERO;
        let mut flip_over = 1.0f32;
        let normal_offset = trail_width * trail_scale / 2.0;
        let mut mod_distance = Vector3::ZERO;

        let mut distance = 0.0f32;
        for &next_point in points.iter().skip(1) {
            let path_direction = next_point - cur_point;
            let offset = path_direction.cross(Vector3::Y);
            let offset = if is_wall { path_direction.cross(offset) } else { offset };
            let offset = offset.normalize();

            if last_offset != Vector3::ZERO && offset.dot(last_offset) < 0.0 {
                flip_over *= -1.0;
            }

            mod_distance = offset * normal_offset * flip_over;
            let normal_scale_dir = mod_distance.to_raw().normalize_or(
                glam::vec3(1.0, 0.0, 1.0)
                    .normalize()
                    .copysign(mod_distance.to_raw()),
            );

            vertices.push(Vertex {
                position: (cur_point - mod_distance).into(),
                colour,
                normal: -normal_scale_dir,
                texture: glam::vec2(1.0, distance / trail_width - 1.0),
            });
            vertices.push(Vertex {
                position: (cur_point + mod_distance).into(),
                colour,
                normal: normal_scale_dir,
                texture: glam::vec2(0.0, distance / trail_width - 1.0),
            });

            distance += path_direction.length();
            last_offset = offset;
            cur_point = next_point;
        }

        let normal_scale_dir = mod_distance.to_raw().normalize_or(
            glam::vec3(1.0, 0.0, 1.0)
                .normalize()
                .copysign(mod_distance.to_raw()),
        );
        vertices.push(Vertex {
            position: (cur_point - mod_distance).into(),
            colour,
            normal: -normal_scale_dir,
            texture: glam::vec2(1.0, distance / trail_width - 1.0),
        });
        vertices.push(Vertex {
            position: (cur_point + mod_distance).into(),
            colour,
            normal: normal_scale_dir,
            texture: glam::vec2(0.0, distance / trail_width - 1.0),
        });
    }

    pub fn update(pack: &mut ActivePack, trail_idx: usize) {
        let _ = pack;
        let _ = trail_idx;
    }

    /// Draw a trail segment.
    /// PREREQUISITES: Trail shaders must already be set.
    pub fn draw_section(&self, device_context: &Dx11Context, section: usize, ctx: LocalContext) {
        self.texture.set(device_context, 0);

        unsafe {
            self.section_vbuffer.set(device_context, 0);
            //PrimitiveTopology::TriangleStrip.set(device_context);
            match ctx {
                LocalContext::World => device_context.Draw(
                    self.section_bookmarks[section + 1] - self.section_bookmarks[section],
                    self.section_bookmarks[section],
                ),
                LocalContext::Map(..) => device_context.DrawInstanced(
                    self.section_bookmarks[section + 1] - self.section_bookmarks[section],
                    1,
                    self.section_bookmarks[section],
                    0,
                ),
            }
        }
    }
}

#[cfg(feature = "statistics")]
impl Drop for ActiveTrail {
    fn drop(&mut self) {
        STATS_TRAIL_VERTEX_SIZE.decrement_by(|| self.section_vbuffer.size());
    }
}

pub static STATS_TRAIL_VERTEX_SIZE: Counter = Counter::DEFAULT;

pub struct TrailParams {
    pub resolution: Option<f32>,
    pub width: f32,
    pub y_offset: f32,
    pub smoothing: Option<f32>,
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
    pub const WIDTH_FACTOR: f32 = 0.0254 * 2.0;

    pub fn width(&self) -> f32 {
        self.width
    }

    pub fn smoothing(&self) -> Option<f32> {
        (self.resolution() > Self::DEFAULT_RESOLUTION).then_some(Self::DEFAULT_SMOOTHING)
    }

    pub fn resolution(&self) -> f32 {
        self.resolution.unwrap_or(Self::WIDTH_FACTOR / self.width())
    }

    pub fn y_offset_for(&self, idx: usize) -> f32 {
        //self.y_offset * (0.2 + (idx as f32 * f32::EPSILON * 100.0) % 0.8f32)
        let scale = 0x4000;
        let idx = idx % scale;
        self.y_offset * (0.5 + (idx as f32 * 0.5 / scale as f32))
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
