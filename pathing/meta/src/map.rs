use {
    crate::coords::{MapSpace, GameSpace},
    glamour::{Box2, Point2},
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
        let [[left, top], [right, bottom]] = self.map_rect;
        Box2::new(Point2::new(left as _, top as _), Point2::new(right as _, bottom as _))
    }

    #[inline]
    pub fn continent_rect(&self) -> Box2<MapSpace> {
        let [[left, top], [right, bottom]] = self.continent_rect;
        Box2::new(Point2::new(left as _, top as _), Point2::new(right as _, bottom as _))
    }
}

pub type MapType = String;
