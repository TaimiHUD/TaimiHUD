use {
    crate::coords::{MapLocalScale, MapSpace, GameSpace},
    glamour::{
        Box2, Point2,
        TransformMap,
    },
};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

pub type MapID = u32;

/// https://wiki.guildwars2.com/wiki/API:1/maps
#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
pub struct Map {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    min_level: Option<isize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    max_level: Option<isize>,
    default_floor: Option<isize>,
    #[serde(rename = "type", default)]
    kind: Option<MapType>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    floors: Vec<isize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    region_id: Option<isize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    region_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    continent_id: Option<isize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    continent_name: Option<String>,
    map_rect: [[i32; 2]; 2],
    continent_rect: [[i32; 2]; 2],
}

impl Map {
    #[inline]
    pub fn map_rect(&self) -> Box2<GameSpace> {
        let [[left, bottom], [right, top]] = self.map_rect;
        // axis inversion is only relevant for coordinate conversions, because a negative box would be awkward .-.
        // TODO: consider making this an explicit property of the transition from 3D/Z axis?
        let (top, bottom) = (bottom, top);
        Box2::new(Point2::new(left as _, top as _), Point2::new(right as _, bottom as _))
    }

    #[inline]
    pub fn continent_rect(&self) -> Box2<MapSpace> {
        let [[left, top], [right, bottom]] = self.continent_rect;
        Box2::new(Point2::new(left as _, top as _), Point2::new(right as _, bottom as _))
    }

    pub fn map_scale(&self) -> MapLocalScale {
        MapLocalScale::with_game_bounds(self.map_rect(), self.continent_rect())
    }

    #[cfg(todo = "unnecessary")]
    pub fn continent_map_origin(&self) -> Point2<MapSpace> {
        let offset = self.map_scale()
            .map(Point2::ZERO - self.map_rect().min);
        self.continent_rect().min + offset
    }

    /// Offset from `Point2::<MapSpace>::ZERO` (continent origin)
    /// to `Point2::<GameSpace>::ZERO` (map origin is usually the centre, but often imperfect)
    pub fn continent_map_origin(&self) -> Point2<MapSpace> {
        let offset = self.map_scale()
            .map(self.map_rect().center().to_vector());
        self.continent_rect().center() - offset
    }
}

pub type MapType = String;

#[cfg(feature = "map-cache")]
mod cache {
    use {
        crate::map::{Map, MapID},
        std::{
            borrow::Cow,
            collections::BTreeMap,
            sync::LazyLock,
        },
    };

    #[cfg(feature = "map-cache")]
    pub struct MapCache;

    impl MapCache {
        pub fn lookup_map(&self, map_id: MapID) -> Option<Cow<'_, Map>> {
            #[cfg(not(feature = "gzip"))]
            const MAPS_SIGN_JSON: &'static str = include_str!("../data/maps-sign.json");
            static MAPS_SIGN: LazyLock<BTreeMap<MapID, Map>> = LazyLock::new(|| {
                let maps = serde_json::from_str::<BTreeMap<MapID, Map>>(MAPS_SIGN_JSON);
                if let Err(_e) = &maps {
                    log::error!("failed to deserialize map cache: {_e}");
                }
                maps.unwrap_or_default()
            });

            let map = MAPS_SIGN.get(&map_id)?;

            Some(Cow::Borrowed(map))
        }
    }
}
#[cfg(feature = "map-cache")]
pub use self::cache::MapCache;

#[cfg(feature = "serde")]
#[test]
fn map_decode() {
    use std::collections::BTreeMap;

    const MAPS_SIGN: &'static str = include_str!("../data/maps-sign.json");
    let maps: BTreeMap<crate::map::MapID, Map> = serde_json::from_str(MAPS_SIGN).unwrap();

    for (id, map) in &maps {
        eprintln!("map#{id}: {map:#?}");
        let local_rect = map.map_rect();
        let global_rect = map.continent_rect();
        eprintln!("loc: {:?}", local_rect);
        eprintln!("glo: {:?}", global_rect);
        eprintln!("sz.loc: {:?}", local_rect.size());
        eprintln!("sz.glo: {:?}", global_rect.size());
        let scale = map.map_scale();
        let offset = map.continent_map_origin();
        eprintln!("{scale:?} @ {offset:?}");
        let skew = scale.map(map.map_rect().center().to_vector());
        eprintln!("skew: {skew:?} x 2ft");
    }

    //eprintln!("{maps:#?}");
}
