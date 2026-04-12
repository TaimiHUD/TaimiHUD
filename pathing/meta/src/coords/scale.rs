use {
    crate::coords::{coord_newtype, GameSpace, LocalSpace, MapSpace},
    glam::Vec3A,
    glamour::{Box2, Size2, Vector2},
};

/// [map coordinates](MapSpace) are in ft and inches
///
/// if we want [local](LocalSpace::from_map),
/// we have to convert ft to m
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct MapLocalScale {
    /// scale stored as m/global,
    /// where global is typically normalized as 2ft
    pub scale: Vector2<LocalSpace>,
}

impl MapLocalScale {
    /// the most common value, held by 1009/1022 maps from the maps api endpoint is
    /// 24.0, -24.0 (2 feet per continent unit)
    ///
    /// (local z+ is usually global y-, so for y scale negatively)
    pub const COMMON: Self = Self::with_game_scale(Vector2::new(24.0, -24.0));

    /// 39.368552322?
    #[cfg(todo)]
    pub const INCHES_PER_METRE: f32 = 39.369999;
    /// precision chosen to match RTAPI
    pub const INCHES_PER_METRE: f32 = 39.37;
    #[cfg(todo)]
    pub const INCHES_PER_METRE: f32 = (1.0f64 / Self::METRES_PER_INCH as f64) as f32;
    /// 3.28084ft / m or 0.3048m / ft
    pub const METRES_PER_FEET: f32 = (Self::METRES_PER_INCH as f64 * 12.0f64) as f32;
    #[cfg(todo)]
    pub const METRES_PER_INCH: f32 = Self::METRES_PER_FEET / 12.0;
    pub const METRES_PER_INCH: f32 = Self::INCHES_PER_METRE.recip();
    /// ~0.02540098? ~0.02540001?
    #[cfg(todo)]
    pub const METRES_PER_INCH: f32 = 0.02540005;
    pub(super) const INCHES_PER_METRE3A: Vec3A = Vec3A::new(
        Self::INCHES_PER_METRE,
        Self::INCHES_PER_METRE,
        -Self::INCHES_PER_METRE,
    );
    pub(super) const METRES_PER_INCH3A: Vec3A = Vec3A::new(
        Self::METRES_PER_INCH,
        -Self::METRES_PER_INCH,
        Self::METRES_PER_INCH,
    );

    pub fn with_scale(scale: Vector2<LocalSpace>) -> Self {
        Self { scale }
    }

    pub const fn with_map_scale(scale: Vector2<MapSpace>) -> Self {
        Self {
            scale: Vector2::new(scale.x * Self::METRES_PER_FEET, scale.y * Self::METRES_PER_FEET),
        }
    }

    /// From [game inches](GameSpace)
    pub const fn with_game_scale(scale: Vector2<GameSpace>) -> Self {
        Self {
            scale: Vector2::new(scale.x * Self::METRES_PER_INCH, scale.y * Self::METRES_PER_INCH),
        }
    }

    /// Calculate using [Map](crate::map::Map::map_rect) info obtained from the API
    pub fn with_game_bounds(map_rect: Box2<GameSpace>, continent_rect: Box2<MapSpace>) -> Self {
        Self::with_game_size(map_rect.size(), continent_rect.size())
    }

    pub fn with_game_size(map_size: Size2<GameSpace>, continent_size: Size2<MapSpace>) -> Self {
        let scale = Vector2::new(
            map_size.width / continent_size.width,
            -map_size.height / continent_size.height,
        );
        Self::with_game_scale(scale)
    }

    /// inches per global unit (typically ~24)
    pub const fn scale_local(&self) -> Vector2<GameSpace> {
        Vector2::new(
            self.scale.x * Self::INCHES_PER_METRE,
            self.scale.y * Self::INCHES_PER_METRE,
        )
    }

    /// length of a global unit (~2ft) in metres
    pub fn scale_length(&self) -> f32 {
        match () {
            _ => self.scale.length(),
            #[cfg(todo)]
            _ => self.scale.element_sum() * (core::f32::consts::SQRT_2/2),
        }
    }

    /// length of a global unit (~2ft) in inches
    pub fn scale_local_length(&self) -> f32 {
        match () {
            _ => self.scale_local().length(),
            #[cfg(todo)]
            _ => self.scale_local().element_sum() * (core::f32::consts::SQRT_2/2)
        }
    }

    #[cfg(feature = "map-cache")]
    pub fn for_map(map_id: crate::map::MapID) -> Option<Self> {
        use crate::map::MapCache;
        let map = MapCache.lookup_map(map_id)?;
        Some(map.map_scale())
    }
}

coord_newtype! {
    impl TransformMap<MapSpace, Output = Vector2<LocalSpace>> for MapLocalScale {
        fn map(&self, v) {
            v.as_() * self.scale
        }
    }
    impl TransformMap<LocalSpace, Output = Vector2<MapSpace>> for MapLocalScale {
        fn map(&self, v) {
            (v / self.scale).as_()
        }
    }
    impl TransformMap<GameSpace, Output = Vector2<MapSpace>> for MapLocalScale {
        fn map(&self, v) {
            (v / self.scale_local()).as_()
        }
    }
    // TODO: Vector3!
}

impl Default for MapLocalScale {
    fn default() -> Self {
        Self::COMMON
    }
}
