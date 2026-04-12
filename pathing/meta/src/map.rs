#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use {
    crate::coords::{GameSpace, MapLocalScale, MapSpace},
    glamour::{Box2, Point2, TransformMap},
};

pub type MapID = u32;

/// <https://wiki.guildwars2.com/wiki/API:1/maps>
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

#[derive(Debug, Clone, PartialOrd, PartialEq)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
pub struct MapProjection {
    #[cfg_attr(feature = "serde", serde(rename = "z"))]
    pub depth: MapProjectionDepth,
}
#[derive(Debug, Clone, PartialOrd, PartialEq)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
pub struct MapProjectionDepth {
    pub farz: f32,
}
impl MapProjectionDepth {
    pub const fn with_far_in(z_far: f32) -> Self {
        Self::with_farz(z_far * Self::FAR_FACTOR_INV)
    }
    pub const fn with_farz(farz: f32) -> Self {
        Self { farz }
    }
    pub const fn z_far_in(&self) -> f32 {
        self.farz * Self::FAR_FACTOR_IN
    }
    pub const fn z_near_in_reference(&self) -> f32 {
        self.farz * Self::FAR_NEAR_FACTOR_INV
    }
    pub fn z_near_in_raw(&self, fov_y_recip: f32) -> f32 {
        self.z_near_in_reference() * fov_y_recip
    }
    pub fn z_near_in(&self, fov_y: f32) -> f32 {
        self.z_near_in_raw(fov_y.recip()).min(Self::NEAR_MAX_IN)
    }

    /// `128*24`
    pub const FAR_FACTOR_IN: f32 = 3072.0f32;
    const FAR_FACTOR_INV: f32 = Self::FAR_FACTOR_IN.recip();
    #[cfg(todo)]
    pub const FAR_FACTOR_M: f32 = Self::FAR_FACTOR_IN * Self::DEFAULT_METRES_PER_INCH;
    pub const NEAR_FACTOR: f32 = match () {
        #[cfg(todo)]
        _ => 7.56f32.exp() + 1.024f32.recip().exp(),
        _ => 1922.5,
    };
    const NEAR_FACTOR_INV: f32 = Self::NEAR_FACTOR.recip();
    const FAR_NEAR_FACTOR_INV: f32 = Self::FAR_FACTOR_IN / Self::NEAR_FACTOR;
    pub const NEAR_MAX_IN: f32 = 25.0f32;
    pub const DEFAULT_NEAR_MAX_M: f32 = Self::NEAR_MAX_IN * Self::DEFAULT_METRES_PER_INCH;

    pub const DEFAULT_FALLBACK: Self = match () {
        #[cfg(todo)]
        _ => Self::with_farz(15.0),
        _ => Self::with_farz(8.0),
    };
    pub const DEFAULT_METRES_PER_INCH: f32 = match () {
        #[cfg(todo)]
        _ => MapLocalScale::METRES_PER_INCH * core::f32::consts::SQRT_2,
        _ => MapLocalScale::METRES_PER_INCH,
    };
}
impl From<MapProjection> for MapProjectionDepth {
    #[inline(always)]
    fn from(proj: MapProjection) -> Self {
        proj.depth
    }
}
impl From<&'_ MapProjection> for MapProjectionDepth {
    #[inline(always)]
    fn from(proj: &MapProjection) -> Self {
        proj.depth.clone()
    }
}
impl From<MapProjectionDepth> for MapProjection {
    #[inline(always)]
    fn from(depth: MapProjectionDepth) -> Self {
        Self { depth }
    }
}
impl From<&'_ MapProjectionDepth> for MapProjection {
    #[inline(always)]
    fn from(depth: &MapProjectionDepth) -> Self {
        depth.clone().into()
    }
}

#[cfg(feature = "map-cache")]
mod cache {
    use {
        crate::map::{Map, MapProjection, MapID},
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
                    () => match MapCache::maps_json_gz().context("inflating map cache") {
                        Ok(json) => serde_json::from_str(&json),
                        Err(e) => Err(serde::de::Error::custom(e)),
                    },
                }
                .context("deserialize map cache");
                if let Err(_e) = &maps {
                    log::error!("{_e:#}");
                }
                maps.unwrap_or_default()
            });

            let map = MAPS_CACHE.get(&map_id)?;

            Some(Cow::Borrowed(map))
        }

        pub fn lookup_map_projection(&self, map_id: MapID) -> Option<Cow<'_, MapProjection>> {
            static PROJ_CACHE: LazyLock<BTreeMap<MapID, MapProjection>> = LazyLock::new(|| {
                let maps: Result<BTreeMap<MapID, MapProjection>, _> = match () {
                    #[cfg(not(feature = "gzip"))]
                    () => serde_json::from_str(MapCache::projection_json()),
                    #[cfg(feature = "gzip")]
                    () => match MapCache::projection_json_gz().context("inflating project cache") {
                        Ok(json) => serde_json::from_str(&json),
                        Err(e) => Err(serde::de::Error::custom(e)),
                    },
                }
                .context("deserialize project cache");
                if let Err(_e) = &maps {
                    log::error!("{_e:#}");
                }
                maps.unwrap_or_default()
            });

            let map = PROJ_CACHE.get(&map_id)?;

            Some(Cow::Borrowed(map))
        }

        #[cfg(not(feature = "gzip"))]
        pub(crate) fn maps_json() -> &'static str {
            const MAPS_JSON: &'static str = include_str!(env!("INC_MAP_CACHE"));
            MAPS_JSON
        }
        #[cfg(feature = "gzip")]
        pub(crate) fn maps_json_gz() -> anyhow::Result<String> {
            const MAPS_JSON_GZ: &'static [u8] = include_bytes!(env!("INC_MAP_CACHE_GZ"));
            const MAPS_JSON_BUFLEN: usize = match usize::from_str_radix(env!("INC_MAP_CACHE_BUFLEN"), 10) {
                Ok(len) => len,
                Err(..) => panic!("GZ len"),
            };
            Self::decode_gz(MAPS_JSON_GZ, MAPS_JSON_BUFLEN)
        }

        #[cfg(not(feature = "gzip"))]
        pub(crate) fn projection_json() -> &'static str {
            const PROJ_JSON: &'static str = include_str!(env!("INC_MAP_PROJCACHE"));
            PROJ_JSON
        }
        #[cfg(feature = "gzip")]
        pub(crate) fn projection_json_gz() -> anyhow::Result<String> {
            const PROJ_JSON_GZ: &'static [u8] = include_bytes!(env!("INC_MAP_PROJ_CACHE_GZ"));
            const PROJ_JSON_BUFLEN: usize = match usize::from_str_radix(env!("INC_MAP_PROJ_CACHE_BUFLEN"), 10) {
                Ok(len) => len,
                Err(..) => panic!("GZ len"),
            };
            Self::decode_gz(PROJ_JSON_GZ, PROJ_JSON_BUFLEN)
        }

        #[cfg(feature = "gzip")]
        fn decode_gz(input: &'static [u8], len: usize) -> anyhow::Result<String> {
            use async_compression::{
                codecs::{gzip::GzipDecoder, Decode},
                core::util::PartialBuffer,
            };

            let mut input = PartialBuffer::new(input);
            let mut out = PartialBuffer::new(vec![0u8; len]);
            let mut decoder = GzipDecoder::new();
            let truncated = || anyhow::anyhow!("BUG: out of buffer for cache!");
            while !input.unwritten().is_empty() {
                let out_full = out.unwritten().is_empty();
                let res = decoder.decode(&mut input, &mut out).context("GZ decode");
                if res? {
                    break
                }
                if out_full && !input.unwritten().is_empty() {
                    return Err(truncated())
                }
            }
            loop {
                let res = decoder.finish(&mut out).context("GZ init");
                if res? {
                    break
                }
                if out.unwritten().is_empty() {
                    return Err(truncated())
                }
            }

            let len = out.written().len();
            let mut out = out.into_inner();
            out.truncate(len);
            String::from_utf8(out).context("unstringy")
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
        eprintln!("map#{id}: {:?}", map.kind);
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

#[cfg(feature = "serde")]
#[test]
fn project_decode() {
    use std::collections::BTreeMap;

    let project_json = match () {
        #[cfg(not(feature = "map-cache"))]
        () => include_str!("../data/projection.json"),
        #[cfg(all(feature = "map-cache", not(feature = "gzip")))]
        () => MapCache::projection_json(),
        #[cfg(all(feature = "map-cache", feature = "gzip"))]
        () => MapCache::projection_json_gz().unwrap(),
    };
    let projections: BTreeMap<MapID, MapProjection> = serde_json::from_str(&project_json).unwrap();

    const LIMIT: usize = 16;
    for (id, project) in projections.iter().take(LIMIT) {
        eprintln!("map#{id}: far={}\"", project.depth.z_far());
        for fov_deg in [33u32, 50, 57, 63, 70] {
            let fovy = (fov_deg as f32).to_radians();
            eprintln!("\tnear={}\"(@{fov_deg}°)", project.depth.z_near(fovy));
        }
    }

    //eprintln!("{maps:#?}");
}
