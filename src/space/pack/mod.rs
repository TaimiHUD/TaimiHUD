use {
    croner::Cron,
    anyhow::Context,
    glamour::{Box3, Point3, Vector3},
};

pub mod poi;
pub mod trail;

mod pack;

pub use {
    taimi_pack::{
        attributes::{self, MarkerAttributes},
        category::{self, Category},
        loader,
        poi::Poi,
        trail::{Trail, TrailSection},
        pack::Pack,
    },
    self::{
        pack::{
            PackCollection,
            ActivePack
        },
        poi::ActivePoi,
        trail::ActiveTrail,
    },
};

pub type PackSpace = crate::marker::atomic::LocalSpace;

pub trait MarkerAttributesExt {
    fn parse_schedule(&self) -> anyhow::Result<Option<Cron>>;
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
