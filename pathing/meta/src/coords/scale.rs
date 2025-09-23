use {
    crate::coords::{
        coord_newtype,
        GameSpace,
        LocalSpace,
        MapSpace,
    },
    glamour::{
        Box2, Size2, Vector2,
    },
};

/// [map coordinates](MapSpace) are in ft and inches
///
/// if we want [LocalSpace::to_local](local),
/// we have to convert ft to m
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct MapLocalScale {
    pub scale: Vector2<LocalSpace>,
}

impl MapLocalScale {
    /// the most common value, held by 1009/1022 maps from the maps api endpoint is
    /// 24.0, -24.0 (2 feet per continent unit)
    ///
    /// (local z+ is usually global y-, so for y scale negatively)
    pub const COMMON: Self = Self::with_game_scale(
        Vector2::new(24.0, -24.0)
    );

    /// 0.3048m / ft
    pub const METRES_PER_FEET: f32 = 1.0 / 3.28084;
    pub const METRES_PER_INCH: f32 = Self::METRES_PER_FEET / 12.0;

    pub fn with_scale(scale: Vector2<LocalSpace>) -> Self {
        Self {
            scale,
        }
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
            map_size.height / continent_size.height,
        );
        Self::with_game_scale(scale)
    }

    #[cfg(feature = "map-cache")]
    pub fn for_map(map_id: crate::map::MapID) -> Option<Self> {
        use {
            crate::map::{Map, MapID},
            std::{
                collections::BTreeMap,
                sync::LazyLock,
            },
        };

        #[cfg(not(feature = "gzip"))]
        const MAPS_SIGN_JSON: &'static str = include_str!("../../data/maps-sign.json");
        static MAPS_SIGN: LazyLock<BTreeMap<MapID, Map>> = LazyLock::new(|| {
            let maps = serde_json::from_str::<BTreeMap<MapID, Map>>(MAPS_SIGN_JSON);
            if let Err(_e) = &maps {
                log::error!("failed to deserialize map cache: {_e}");
            }
            maps.unwrap_or_default()
        });

        let map = MAPS_SIGN.get(&map_id)?;

        Some(Self::with_game_bounds(map.map_rect(), map.continent_rect()))
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
    // TODO: Vector3!
}

impl Default for MapLocalScale {
    fn default() -> Self {
        Self::COMMON
    }
}
