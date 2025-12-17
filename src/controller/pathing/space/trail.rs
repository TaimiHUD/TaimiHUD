use {
    crate::{
        controller::pathing::{
            state::{
                visible::{LoadedTrailGeometry, LoadedTrailSection},
                VisibilityFlags,
            },
            space::DrawSpace,
        },
        space::{
            resources::{Texture, Vertex},
            TextureSpace,
        },
    },
    core::f32,
    glamour::{Box3, Point2},
    std::sync::Arc,
    taimi_d3d::dx11::buffer::VertexBuffer,
    taimi_meta::packs::{CategoryIndex, CategoryPath, TrailPath},
    taimi_pack::{
        attributes::{RenderAttributes, TrailAttributes},
        PackLoaderContext
    },
};

pub struct SpaceTrail {
    #[cfg(todo = "unnecessary")]
    pub path: TrailPath,
    pub category: CategoryPath,
    pub render_bookmark: u32,

    pub visibility: VisibilityFlags,
    attrs: Arc<RenderAttributes>,
    overrides: Option<Box<RenderAttributes>>,

    // Segment data.
    pub section_bounds: Vec<Box3<DrawSpace>>,
    /// deallocated once uploaded to vertex buffer
    pub vertices: Vec<Vertex>,

    // World render data.
    pub texture: Option<Arc<Texture>>,
    pub section_vbuffer: Option<VertexBuffer>,
    pub section_bookmarks: Vec<u32>,
}

impl SpaceTrail {
    pub fn new(
        attrs: Arc<RenderAttributes>,
        geometry: LoadedTrailGeometry,
        sections: &[LoadedTrailSection],
        visibility: VisibilityFlags,
        path: TrailPath,
        category: CategoryPath,
    ) -> anyhow::Result<Self> {
        #[cfg(todo)]
        let mut y_offset = {
            // mitigate z-fighting by fudging y values for (hopefully) unique trails
            let pack_signature = loader.pack.trails.len()
                + loader.pack.pois.len()
                + loader.pack.categories.all_categories.len();
            params.y_offset_for(pack_signature ^ (trail.path.wrapping_mul(73)))
        };

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
            log::info!("Empty trail {category}/{path}");
        }

        Ok(Self {
            #[cfg(todo = "unnecessary")]
            path,
            category,
            visibility,
            attrs,
            overrides: None,
            section_bounds,
            texture: None,
            section_vbuffer: None,
            section_bookmarks,
            vertices: geometry.vertices,
            render_bookmark: 0,
        })
    }

    pub fn empty() -> Self {
        Self {
            #[cfg(todo = "unnecessary")]
            path: TrailPath::with_path(TrailIndex::MAX),
            category: CategoryPath::with_path(CategoryIndex::MAX),
            visibility: VisibilityFlags::empty(),
            attrs: super::EMPTY_RENDER_ATTRS.clone(),
            overrides: None,
            section_bounds: Default::default(),
            texture: None,
            section_vbuffer: None,
            section_bookmarks: Default::default(),
            render_bookmark: 0,
            vertices: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.section_bounds.is_empty()
    }

    pub fn clear_overrides(&mut self) {
        self.overrides = None;
    }
    pub fn set_overrides(&mut self, overrides: RenderAttributes) {
        let overrides = self.overrides.insert(Box::new(overrides));
        let _ = overrides.trail.get_or_insert_default();
        overrides.merge(&self.attrs);
    }
    #[inline]
    pub fn set_attrs(&mut self, overrides: Option<RenderAttributes>) {
        match overrides {
            Some(o) => self.set_overrides(o),
            None => self.clear_overrides(),
        }
    }

    pub fn render_attrs(&self) -> &RenderAttributes {
        self.overrides.as_ref().map(|a| &**a)
            .unwrap_or(&self.attrs)
    }
    pub fn trail_attrs(&self) -> &TrailAttributes {
        let trail = self.render_attrs().trail.as_ref()
            .map(|p| &**p);
        unsafe {
            trail.unwrap_unchecked()
        }
    }
}

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
