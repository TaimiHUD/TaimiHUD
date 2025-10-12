use {
    crate::space::{
        pack::{ActivePack, TrailSectionExt},
        resources::{Model, Texture, Vertex},
        DrawSpace, LocalContext,
    },
    anyhow::Context,
    core::f32,
    glamour::{Box3, Vector3, Vec3Swizzles},
    std::sync::Arc,
    taimi_d3d::dx11::{
        buffer::VertexBuffer,
        prelude::*,
    },
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

    // Map render data.
    pub map_vbuffer: Option<VertexBuffer>,

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
        let trail_width = params.width();
        let resolution = params.resolution();
        let smoothing = params.smoothing();
        let mut y_offset = {
            // mitigate z-fighting by fudging y values for (hopefully) unique trails
            let pack_signature = loader.pack.trails.len() + loader.pack.pois.len() + loader.pack.categories.all_categories.len();
            params.y_offset_for(pack_signature ^ (trail_idx.wrapping_mul(73)))
        };

        let texture_handle = trail.texture_name()
            .ok_or_else(|| anyhow::anyhow!("TODO: Add a fallback texture for trails"))?;
        let texture_handle = loader.register_texture(texture_handle);
        let texture = loader
            .get_or_load_texture(texture_handle, device)
            .context("Loading trail texture")?;

        let mut vertices: Vec<Vertex> = Vec::new();
        let mut section_bookmarks: Vec<u32> = vec![0];
        let mut section_bounds = Vec::new();

        for (isec, section) in trail.data.sections.iter().enumerate() {
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

            let mut cur_point = points[0];
            let mut last_offset = Vector3::ZERO;
            let mut flip_over = 1.0f32;
            let normal_offset = trail_width * trail.scale() / 2.0;
            let mut mod_distance = Vector3::ZERO;

            let mut distance = 0.0f32;
            for &next_point in points.iter().skip(1) {
                let path_direction = next_point - cur_point;
                let offset = path_direction.cross(Vector3::Y);
                let offset = if trail.is_wall() {
                    path_direction.cross(offset)
                } else {
                    offset
                };
                let offset = offset.normalize();

                if last_offset != Vector3::ZERO && offset.dot(last_offset) < 0.0 {
                    flip_over *= -1.0;
                }

                mod_distance = offset * normal_offset * flip_over;
                let normal_scale_dir = mod_distance.to_raw().normalize_or(
                    glam::vec3(1.0, 0.0, 1.0).normalize().copysign(mod_distance.to_raw())
                );

                vertices.push(Vertex {
                    position: (cur_point - mod_distance).into(),
                    colour: glam::Vec3::ONE,
                    normal: -normal_scale_dir,
                    texture: glam::vec2(1.0, distance / trail_width - 1.0),
                });
                vertices.push(Vertex {
                    position: (cur_point + mod_distance).into(),
                    colour: glam::Vec3::ONE,
                    normal: normal_scale_dir,
                    texture: glam::vec2(0.0, distance / trail_width - 1.0),
                });

                distance += path_direction.length();
                last_offset = offset;
                cur_point = next_point;
            }

            let normal_scale_dir = mod_distance.to_raw().normalize_or(
                glam::vec3(1.0, 0.0, 1.0).normalize().copysign(mod_distance.to_raw())
            );
            vertices.push(Vertex {
                position: (cur_point - mod_distance).into(),
                colour: glam::Vec3::ONE,
                normal: -normal_scale_dir,
                texture: glam::vec2(1.0, distance / trail_width - 1.0),
            });
            vertices.push(Vertex {
                position: (cur_point + mod_distance).into(),
                colour: glam::Vec3::ONE,
                normal: normal_scale_dir,
                texture: glam::vec2(0.0, distance / trail_width - 1.0),
            });

            section_bookmarks.push(vertices.len() as u32);
            section_bounds.push(section.bounds());
        }

        if vertices.is_empty() {
            log::info!(
                "Empty trail {}:{}",
                trail.category,
                trail.guid,
            );
        }

        let model = Model::from_vertices(vertices);
        let section_vbuffer = model.to_buffer(device).context("Creating trail vbuffer")?;

        Ok(ActiveTrail {
            trail_idx,
            category_idx,
            filtered: false,
            section_bounds,
            texture: texture.clone(),
            section_vbuffer,
            section_bookmarks,
            map_vbuffer: None,
            render_bookmark,
            y_offset,
        })
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
        (self.resolution() > Self::DEFAULT_RESOLUTION)
            .then_some(Self::DEFAULT_SMOOTHING)
    }

    pub fn resolution(&self) -> f32 {
        self.resolution
            .unwrap_or(Self::WIDTH_FACTOR / self.width())
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
