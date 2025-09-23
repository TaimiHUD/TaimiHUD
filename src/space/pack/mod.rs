use {
    croner::Cron,
    anyhow::Context,
    glamour::{Box3, Point3, Vector3},
    crate::space::{LocalContext, MapContext},
};

pub mod poi;
pub mod trail;

pub(crate) mod pack;

pub use {
    taimi_pack::{
        attributes::MarkerAttributes,
        category::Category,
        loader,
        poi::Poi,
        trail::TrailSection,
        pack::Pack,
    },
    self::pack::{
        ActivePack,
        PackCollection,
        LoaderBox,
    },
};

pub type PackSpace = taimi_meta::coords::LocalSpace;

pub trait MarkerAttributesExt {
    fn parse_schedule(&self) -> anyhow::Result<Option<Cron>>;
    fn visibility_for_map(&self, map: MapContext) -> Option<bool>;
    fn visibility_for(&self, ctx: LocalContext) -> Option<bool>;
    fn is_visible_for(&self, ctx: LocalContext) -> bool {
        self.visibility_for(ctx).unwrap_or(true)
    }
    fn is_visible_for_map(&self, map: MapContext) -> bool {
        self.visibility_for_map(map).unwrap_or(true)
    }
}

impl MarkerAttributesExt for MarkerAttributes {
    fn parse_schedule(&self) -> anyhow::Result<Option<Cron>> {
        match &self.schedule {
            Some(schedule) => schedule.parse()
                .context("parsing marker schedule")
                .map(Some),
            None => Ok(None),
        }
    }

    fn visibility_for(&self, ctx: LocalContext) -> Option<bool> {
        match ctx {
            LocalContext::World => self.in_game_visibility,
            LocalContext::Map(map) => self.visibility_for_map(map),
        }
    }

    fn visibility_for_map(&self, map: MapContext) -> Option<bool> {
        match map {
            MapContext::Global => self.map_visibility,
            MapContext::Minimap => self.minimap_visibility,
        }
    }
}

pub trait PoiExt {
    fn marker_position(&self) -> Point3<PackSpace>;
    fn position(&self) -> Point3<PackSpace>;
    fn offset(&self) -> Vector3<PackSpace>;
}

impl PoiExt for Poi {
    fn offset(&self) -> Vector3<PackSpace> {
        let height_offset = self.height_offset();
        Vector3::new(0.0, height_offset, 0.0)
    }

    #[inline]
    fn marker_position(&self) -> Point3<PackSpace> {
        Point3::from_raw(self.position.into())
    }

    fn position(&self) -> Point3<PackSpace> {
        self.marker_position() + self.offset()
    }
}

#[cfg(todo)]
pub trait TrailExt {
}

#[cfg(todo)]
impl TrailExt for Trail {
}

pub trait TrailSectionExt {
    fn bounds(&self) -> Box3<PackSpace>;
}

impl TrailSectionExt for TrailSection {
    #[inline]
    fn bounds(&self) -> Box3<PackSpace> {
        let min = Point3::from_raw(self.bounds.min.to_raw());
        let max = Point3::from_raw(self.bounds.max.to_raw());
        Box3::new(min, max)
    }
}
