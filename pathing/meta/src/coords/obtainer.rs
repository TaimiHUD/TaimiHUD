use {
    crate::coords::{LocalPoint, LocalSpace, MapLocalScale, MapPoint, MapSpace},
    glamour::{Box2, Box3, Point2, Size3, TransformMap},
};

/// we need to be able to figure out the axis directions, let's do this
/// without web requests (to v2 maps api) by taking two points
/// the one thing we don't need to check is the height, and we shouldn't let that
/// skew our distance, either.
#[derive(Copy, Debug, Default, PartialEq, Clone)]
pub struct SignObtainer {
    pub bounds: Box3<LocalSpace>,
    pub global: Box2<MapSpace>,
}

impl SignObtainer {
    pub const DEFAULT: Self = Self { bounds: Box3::ZERO, global: Box2::ZERO };

    pub fn is_empty(&self) -> bool {
        self.global == Box2::ZERO
    }

    pub fn clear(&mut self) {
        *self = Self::DEFAULT;
    }

    pub fn reset(&mut self, local: LocalPoint, global: MapPoint) {
        self.bounds = Box3::new(local, local);
        self.global.min = global;
        self.global.max = global;
    }

    pub fn update(&mut self, local: LocalPoint, global: MapPoint) {
        if self.is_empty() {
            self.reset(local, global);
            return
        }

        if self.bounds.min.x > local.x {
            self.bounds.min.x = local.x;
            self.global.min.x = global.x;
        }
        if self.bounds.min.z > local.z {
            self.bounds.min.z = local.z;
            self.global.min.y = global.y;
        }
        if self.bounds.min.y > local.y {
            self.bounds.min.y = local.y;
        }

        if self.bounds.max.x < local.x {
            self.bounds.max.x = local.x;
            self.global.max.x = global.x;
        }
        if self.bounds.max.z < local.z {
            self.bounds.max.z = local.z;
            self.global.max.y = global.y;
        }
        if self.bounds.max.y < local.y {
            self.bounds.max.y = local.y;
        }
    }

    pub fn bounds2(&self) -> Box2<LocalSpace> {
        Box2::new(LocalSpace::to2(self.bounds.min), LocalSpace::to2(self.bounds.max))
    }

    const MIN_DIM: f32 = MapLocalScale::METRES_PER_FEET * 10.0 * 1.5;
    const MIN_RANGE: f32 = Self::MIN_DIM * 2.0;
    const MIN_RANGE_SQUARED: f32 = Self::MIN_RANGE * Self::MIN_RANGE;
    pub fn has_scale(&self) -> bool {
        let bounds = self.bounds2();
        let range = bounds.min - bounds.max;
        range.abs().min_element() > Self::MIN_DIM && range.length_squared() >= Self::MIN_RANGE_SQUARED
    }

    pub fn get_scale(&self) -> Option<MapLocalScale> {
        if !self.has_scale() {
            return None
        }
        let bounds = self.bounds2();
        let local = bounds.max - bounds.min;
        let global = self.global.max - self.global.min;
        Some(MapLocalScale::with_scale(local / global.as_()))
    }

    pub fn centre(&self) -> (Point2<LocalSpace>, MapPoint) {
        (self.bounds2().center(), self.global.center())
    }

    const SIGNIFIANT_THRESHOLD: f32 = 0.2;
    pub fn is_significant(sign: MapLocalScale) -> bool {
        (sign.scale - MapLocalScale::COMMON.scale)
            .abs()
            .cmpgt(MapLocalScale::COMMON.scale * Self::SIGNIFIANT_THRESHOLD)
            .all()
    }

    pub fn set(&mut self, scale: MapLocalScale, local: LocalPoint, global: MapPoint) {
        let size = Size3::<LocalSpace>::splat(Self::MIN_DIM).to_vector() / 2.0;
        self.bounds = Box3::new(local - size, local + size);
        let global_size = scale.map(size.truncate());
        self.global = Box2::new(global - global_size, global + global_size);
    }

    pub fn scale(&self) -> MapLocalScale {
        match self.get_scale() {
            Some(sign) if Self::is_significant(sign) => sign,
            _ => MapLocalScale::COMMON,
        }
    }
}
