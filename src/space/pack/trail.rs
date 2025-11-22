use {
    crate::{
        controller::pathing::{registry::{CategoryIndex, TrailIndex, TrailSectionIndex}, visible::{LoadedTrailGeometry, LoadedTrailSection}}, exports::runtime::{self as rt, Counter}, space::{
            pack::{poi::PoiCommonRenderData, ActivePack},
            resources::{Model, Texture},
            DrawSpace,
            LocalContext,
            TextureSpace,
        }
    }, anyhow::Context, core::f32, glamour::{Box3, Point2}, std::sync::Arc, taimi_d3d::dx11::{buffer::VertexBuffer, prelude::*}, taimi_pack::PackLoaderContext
};
use crate::controller::pathing::visible::VisibilityFlags;

pub struct ActiveTrail {
    #[cfg(todo = "unnecessary")]
    pub trail_idx: TrailIndex,
    pub category_idx: CategoryIndex,
    pub render_bookmark: u32,

    pub visibility: VisibilityFlags,

    // Segment data.
    pub section_bounds: Vec<Box3<DrawSpace>>,

    // World render data.
    pub texture: Option<Arc<Texture>>,
    pub section_vbuffer: Option<VertexBuffer>,
    pub section_bookmarks: Vec<u32>,
}

impl ActiveTrail {
    #[cfg(deleteme)]
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

            let mut cur_point = points[0];
            let mut last_offset = Vector3::ZERO;
            let mut flip_over = 1.0f32;
            let normal_offset = trail_width * trail.scale() / 2.0;
            let mut mod_distance = Vector3::ZERO;

            let mut distance = 0.0f32;
            for &next_point in points.iter().skip(1) {
                let path_direction = next_point - cur_point;
                let offset = path_direction.cross(Vector3::Y);
                let offset = if trail.is_wall() { path_direction.cross(offset) } else { offset };
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
                glam::vec3(1.0, 0.0, 1.0)
                    .normalize()
                    .copysign(mod_distance.to_raw()),
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

    pub fn new(
        active_pack: &mut ActivePack,
        loader: &mut dyn PackLoaderContext,
        texture_name: &str,
        geometry: LoadedTrailGeometry,
        sections: &[LoadedTrailSection],
        visibility: VisibilityFlags,
        trail_idx: TrailIndex,
        category_idx: CategoryIndex,
        render_bookmark: u32,
        device: &Dx11Device,
    ) -> anyhow::Result<ActiveTrail> {
        #[cfg(todo)]
        let mut y_offset = {
            // mitigate z-fighting by fudging y values for (hopefully) unique trails
            let pack_signature = loader.pack.trails.len()
                + loader.pack.pois.len()
                + loader.pack.categories.all_categories.len();
            params.y_offset_for(pack_signature ^ (trail_idx.wrapping_mul(73)))
        };

        let texture_handle = active_pack.register_texture(texture_name);
        let texture = active_pack
            .get_or_load_texture(texture_handle, loader, device)
            .context("Loading trail texture")
            .cloned();

        let mut section_bookmark = 0u32;
        let mut section_bookmarks = Vec::with_capacity(geometry.section_lengths.len());
        let mut section_bounds = Vec::with_capacity(geometry.section_lengths.len());
        let mut y_offsets = geometry.y_offsets.into_iter();
        for (section, &section_len) in sections.iter().zip(&geometry.section_lengths) {
            if section_len == 0 {
                continue
            }
            let mut bounds = section.bounds;
            if let Some(y_offset) = y_offsets.next() {
                bounds.min.y += y_offset;
                bounds.max.y += y_offset;
            }
            section_bookmarks.push(section_bookmark);
            section_bounds.push(bounds);
            section_bookmark += section_len;
        }
        section_bookmarks.push(section_bookmark);

        if geometry.vertices.is_empty() {
            log::info!("Empty trail {category_idx}/{trail_idx}");
        }

        let model = Model::from_vertices(geometry.vertices);
        let section_vbuffer = model.to_buffer(device).context("Creating trail vbuffer")?;
        STATS_TRAIL_VERTEX_SIZE.increment_by(|| section_vbuffer.size());

        Ok(ActiveTrail {
            #[cfg(todo = "unnecessary")]
            trail_idx,
            category_idx,
            visibility,
            section_bounds,
            texture: rt::log::warn_ok(texture),
            section_vbuffer: Some(section_vbuffer),
            section_bookmarks,
            render_bookmark,
        })
    }

    pub fn update(pack: &mut ActivePack, trail_idx: usize) {
        let _ = pack;
        let _ = trail_idx;
    }

    pub fn bind_texture(&self, device_context: &Dx11Context, common: &PoiCommonRenderData, _ctx: LocalContext) {
        let texture = self.texture.as_ref()
            .or_else(|| common.fallback_texture.as_ref());
        if let Some(texture) = texture {
            texture.set(device_context, 0);
        }
    }
    /// Draw a trail segment.
    /// PREREQUISITES: Trail shaders and texture must already be set.
    pub fn draw_section(&self, device_context: &Dx11Context, section: TrailSectionIndex, ctx: LocalContext) {
        let section = section as usize;

        if let Some(section_vbuffer) = &self.section_vbuffer {
            section_vbuffer.set(device_context, 0);
        }
        let (start, end) = match self.section_bookmarks.get(section..) {
            Some(&[start, end, ..]) => (start, end),
            _ => {
                log::error!("attempted to draw invalid section#{section} of trail in cat#{}", self.category_idx);
                return
            },
        };
        unsafe {
            //PrimitiveTopology::TriangleStrip.set(device_context);
            match ctx {
                LocalContext::World => device_context.Draw(
                    end - start,
                    start,
                ),
                LocalContext::Map(..) => device_context.DrawInstanced(
                    end - start,
                    1,
                    start,
                    0,
                ),
            }
        }
    }

    pub fn empty() -> Self {
        Self {
            #[cfg(todo = "unnecessary")]
            trail_idx: TrailIndex::MAX,
            category_idx: CategoryIndex::MAX,
            visibility: VisibilityFlags::empty(),
            section_bounds: Default::default(),
            texture: None,
            section_vbuffer: None,
            section_bookmarks: Default::default(),
            render_bookmark: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.section_bounds.is_empty()
    }
}

#[cfg(feature = "statistics")]
impl Drop for ActiveTrail {
    fn drop(&mut self) {
        if let Some(section_vbuffer) = &self.section_vbuffer {
            STATS_TRAIL_VERTEX_SIZE.decrement_by(|| section_vbuffer.size());
        }
    }
}

pub static STATS_TRAIL_VERTEX_SIZE: Counter = Counter::DEFAULT;

#[derive(Debug, Clone)]
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
