use {
    crate::space::TextureSpace,
    core::f32,
    glamour::Point2,
};

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

    /// mitigate z-fighting by fudging y values for (hopefully) unique trails
    #[cfg(todo)]
    pub fn y_offset_for_trail(&self, pack: &LoadedPack, path: TrailPath) -> f32 {
        if self.y_offset == 0.0 {
            return 0.0
        }
        let pack_signature = pack.trails.len()
            + pack.pois.len()
            + pack.categories.all_categories.len();
        self.y_offset_for(pack_signature ^ (path.path.wrapping_mul(73)))
    }

    pub const Y_OFFSET_SECTION_GAP: f32 = f32::EPSILON * 40.0;
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
