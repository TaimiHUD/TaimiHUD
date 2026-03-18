#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use {
    crate::coords::{GameSpace, MapLocalScale, MapSpace},
    glamour::{Box2, Point2, TransformMap},
};

pub type MapID = u32;

/// https://wiki.guildwars2.com/wiki/API:1/maps
#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
pub struct Map {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_level: Option<isize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_level: Option<isize>,
    pub default_floor: Option<isize>,
    #[serde(rename = "type", default)]
    pub kind: Option<MapType>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub floors: Vec<isize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region_id: Option<isize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub continent_id: Option<isize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub continent_name: Option<String>,
    pub map_rect: [[i32; 2]; 2],
    pub continent_rect: [[i32; 2]; 2],
}

impl Map {
    #[inline]
    pub fn map_rect(&self) -> Box2<GameSpace> {
        let [[left, bottom], [right, top]] = self.map_rect;
        // axis inversion is only relevant for coordinate conversions, because a negative box would be awkward .-.
        // TODO: consider making this an explicit property of the transition from 3D/Z axis?
        let (top, bottom) = (bottom, top);
        Box2::new(
            Point2::new(left as _, top as _),
            Point2::new(right as _, bottom as _),
        )
    }

    #[inline]
    pub fn continent_rect(&self) -> Box2<MapSpace> {
        let [[left, top], [right, bottom]] = self.continent_rect;
        Box2::new(
            Point2::new(left as _, top as _),
            Point2::new(right as _, bottom as _),
        )
    }

    pub fn map_scale(&self) -> MapLocalScale {
        MapLocalScale::with_game_bounds(self.map_rect(), self.continent_rect())
    }

    #[cfg(todo = "unnecessary")]
    pub fn continent_map_origin(&self) -> Point2<MapSpace> {
        let offset = self.map_scale().map(Point2::ZERO - self.map_rect().min);
        self.continent_rect().min + offset
    }

    /// Offset from `Point2::<MapSpace>::ZERO` (continent origin)
    /// to `Point2::<GameSpace>::ZERO` (map origin is usually the centre, but often imperfect)
    pub fn continent_map_origin(&self) -> Point2<MapSpace> {
        let offset = self.map_scale().map(self.map_rect().center().to_vector());
        self.continent_rect().center() - offset
    }
}

pub type MapType = String;

#[cfg(feature = "map-cache")]
mod cache {
    use {
        crate::map::{Map, MapID},
        anyhow::Context,
        std::{borrow::Cow, collections::BTreeMap, sync::LazyLock},
    };

    #[cfg(feature = "map-cache")]
    pub struct MapCache;

    impl MapCache {
        pub fn lookup_map(&self, map_id: MapID) -> Option<Cow<'_, Map>> {
            static MAPS_CACHE: LazyLock<BTreeMap<MapID, Map>> = LazyLock::new(|| {
                let maps: Result<BTreeMap<MapID, Map>, _> = match () {
                    #[cfg(not(feature = "gzip"))]
                    () => serde_json::from_str(MapCache::maps_json()),
                    #[cfg(feature = "gzip")]
                    () => match MapCache::maps_json_gz().context("couldn't inflate map cache") {
                        Ok(json) => serde_json::from_str(&json),
                        Err(e) => Err(serde::de::Error::custom(e)),
                    },
                }
                .context("failed to deserialize map cache");
                if let Err(_e) = &maps {
                    log::error!("{_e:#}");
                }
                maps.unwrap_or_default()
            });

            let map = MAPS_CACHE.get(&map_id)?;

            Some(Cow::Borrowed(map))
        }

        #[cfg(not(feature = "gzip"))]
        pub(crate) fn maps_json() -> &'static str {
            const MAPS_JSON: &'static str = include_str!(env!("INC_MAP_CACHE"));
            MAPS_JSON
        }

        #[cfg(feature = "gzip")]
        pub(crate) fn maps_json_gz() -> anyhow::Result<String> {
            use async_compression::{
                codecs::{gzip::GzipDecoder, Decode},
                core::util::PartialBuffer,
            };

            const MAPS_JSON_GZ: &'static [u8] = include_bytes!(env!("INC_MAP_CACHE_GZ"));
            /// TODO: build script could export this as env var?
            const MAPS_JSON_LEN: usize = 0x64000;
            let mut input = PartialBuffer::new(MAPS_JSON_GZ);
            let mut out = PartialBuffer::new(vec![0u8; MAPS_JSON_LEN]);
            let mut decoder = GzipDecoder::new();
            while !input.unwritten().is_empty() {
                let res = decoder
                    .decode(&mut input, &mut out)
                    .context("GZIP decode failure");
                if res? {
                    break
                }
            }
            loop {
                let res = decoder.finish(&mut out).context("GZIP failed to finalize");
                if res? {
                    break
                }
                if out.unwritten().is_empty() {
                    anyhow::bail!("BUG: buffer ran out of room for map cache!");
                }
            }

            let len = out.written().len();
            let mut out = out.into_inner();
            out.truncate(len);
            String::from_utf8(out).context("decoded data not stringy enough")
        }
    }
}
#[cfg(feature = "map-cache")]
pub use self::cache::MapCache;

#[cfg(feature = "serde")]
#[test]
fn map_decode() {
    use std::collections::BTreeMap;

    let maps_json = match () {
        #[cfg(not(feature = "map-cache"))]
        () => include_str!("../data/maps-sign.json"),
        #[cfg(all(feature = "map-cache", not(feature = "gzip")))]
        () => MapCache::maps_json(),
        #[cfg(all(feature = "map-cache", feature = "gzip"))]
        () => MapCache::maps_json_gz().unwrap(),
    };
    let maps: BTreeMap<crate::map::MapID, Map> = serde_json::from_str(&maps_json).unwrap();

    const LIMIT: usize = 48;
    for (id, map) in maps.iter().take(LIMIT) {
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
