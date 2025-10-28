use {
    crate::coords::{coord_newtype, GameSpace, LocalSpace, MapSpace},
    glamour::{Box2, Size2, Vector2},
};

/// [map coordinates](MapSpace) are in ft and inches
///
/// if we want [LocalSpace::to_local](local),
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

    /// 0.3048m / ft
    pub const METRES_PER_FEET: f32 = 1.0 / 3.28084;
    pub const METRES_PER_INCH: f32 = Self::METRES_PER_FEET / 12.0;

    pub fn with_scale(scale: Vector2<LocalSpace>) -> Self {
        Self { scale }
    }

    pub const fn with_map_scale(scale: Vector2<MapSpace>) -> Self {
        Self {
            scale: Vector2::new(
                scale.x * Self::METRES_PER_FEET,
                scale.y * Self::METRES_PER_FEET,
            ),
        }
    }

    /// From [game inches](GameSpace)
    pub const fn with_game_scale(scale: Vector2<GameSpace>) -> Self {
        Self {
            scale: Vector2::new(
                scale.x * Self::METRES_PER_INCH,
                scale.y * Self::METRES_PER_INCH,
            ),
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

    /// inches per global unit
    pub const fn scale_local(&self) -> Vector2<GameSpace> {
        Vector2::new(
            self.scale.x / Self::METRES_PER_INCH,
            self.scale.y / Self::METRES_PER_INCH,
        )
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
