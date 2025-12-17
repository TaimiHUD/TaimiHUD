#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct PoiScale {
    pub expansion: f32,
}

impl PoiScale {
    /// No expansion, standard sizing
    pub const DEFAULT: Self = Self::new(0.0);

    pub const fn new(expansion: f32) -> Self {
        Self { expansion }
    }

    /// Convert from settings
    pub const fn with_scale(poi_scale: f32) -> Self {
        Self::new(poi_scale - 1.0)
    }

    pub const fn scale(&self) -> f32 {
        self.expansion + 1.0
    }
}

impl Default for PoiScale {
    fn default() -> Self {
        Self::DEFAULT
    }
}
