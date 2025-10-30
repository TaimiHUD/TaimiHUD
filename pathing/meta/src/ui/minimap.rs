use {
    crate::{
        coords::{
            coord_newtype,
            transform2_cast,
            CompassSpace,
            FakeSpace,
            MapSpace,
            MinimapSpace,
            WorldmapSpace,
        },
        ui::{MapCalibration, MapContext, MapState, MapUnit, UiState},
    },
    glamour::{Angle, Point2, Rect, Size2, Transform2, Unit, Vector2},
};

pub type MinimapState = MapState<MinimapSpace>;

impl MapUnit for MinimapSpace {
    const CONTEXT: MapContext = MapContext::Minimap;

    type Rotation = Option<Angle>;
    const ROTATION_DISABLED: Self::Rotation = None;
    const ROTATION: bool = true;
    #[inline]
    fn get_rotation(angle: Self::Rotation) -> Option<Angle> {
        angle
    }
    #[inline]
    fn rotation_from(angle: Option<Angle>) -> Self::Rotation {
        angle
    }
}

/// See also: [MinimapSpace](crate::coords::MinimapSpace)
#[derive(Debug, Default, PartialOrd, Ord, PartialEq, Eq, Clone, Copy, Hash)]
pub enum MinimapPlacement {
    #[default]
    /// Bottom-right
    Bottom,
    /// [Top-right](UiState::CompassTopRight)
    Top,
}

impl MinimapPlacement {
    pub const DEFAULT: Self = Self::Bottom;

    /// xp bar height + spacing
    ///
    /// TODO: 37.0 or 38?
    pub const BOTTOM_OFFSET: Size2<FakeSpace> = Size2::new(0.0, 36.5);
    /// the thing on the right side on hover-over
    pub const EDGE_DEAD_ZONE: Size2<FakeSpace> = Size2::new(26.0, 0.0);
    /// resize border/handle around outer edge
    ///
    /// TODO: may be anywhere from 4 to 7 fakepixels, haven't measured
    pub const INSET_DEAD_ZONE: Size2<FakeSpace> = Size2::new(8.0, 8.0);
    pub const DEAD_ZONE_SIZE: Size2<FakeSpace> = Size2::new(
        Self::EDGE_DEAD_ZONE.width + Self::INSET_DEAD_ZONE.width,
        Self::EDGE_DEAD_ZONE.height + Self::INSET_DEAD_ZONE.height,
    );

    pub fn ui_flag(self) -> UiState {
        match self {
            MinimapPlacement::Top => UiState::CompassTopRight,
            MinimapPlacement::Bottom => UiState::empty(),
        }
    }

    /// Calculate minimap position
    ///
    /// `ui_bounds` must have already been adjusted to remove any
    /// [offsets or borders](Self::BOTTOM_OFFSET).
    pub fn bounds<U: Unit>(self, compass_size: Size2<U>, mut ui_bounds: Rect<U>) -> Rect<U> {
        let origin = Point2::new(ui_bounds.size.width - compass_size.width, match self {
            MinimapPlacement::Top => <U::Scalar as num_traits::ConstZero>::ZERO,
            MinimapPlacement::Bottom => ui_bounds.size.height - compass_size.height,
        });
        ui_bounds.origin += origin;
        ui_bounds.size = compass_size;
        ui_bounds
    }
}

impl From<UiState> for MinimapPlacement {
    fn from(state: UiState) -> Self {
        match state.contains(UiState::CompassTopRight) {
            true => Self::Top,
            false => Self::Bottom,
        }
    }
}

impl From<MinimapPlacement> for UiState {
    fn from(ctx: MinimapPlacement) -> Self {
        ctx.ui_flag()
    }
}

impl From<MinimapPlacement> for MapContext {
    fn from(_: MinimapPlacement) -> Self {
        MapContext::Minimap
    }
}

impl MapCalibration {
    pub fn compass_bounds(&self) -> Rect<FakeSpace> {
        let bounds = self.display_size() - MinimapPlacement::BOTTOM_OFFSET;
        self.compass_position
            .bounds(self.compass_size, Rect::from_size(bounds))
    }
}

impl<M: MapUnit> MapState<M>
where
    WorldmapSpace: Unit<Scalar = <M as Unit>::Scalar>,
    CompassSpace: Unit<Scalar = <M as Unit>::Scalar>,
{
    pub fn to_compass(&self) -> Transform2<M, CompassSpace> {
        match M::ROTATION {
            true => self.rotation().map(Transform2::from_angle),
            false => None,
        }
        .unwrap_or(Transform2::IDENTITY)
    }

    pub fn from_compass(&self) -> Transform2<CompassSpace, M> {
        match M::ROTATION {
            true => self.counter_rotation().map(Transform2::from_angle),
            false => None,
        }
        .unwrap_or(Transform2::IDENTITY)
    }
}

coord_newtype! {
    /*impl TransformMap<MinimapSpace, Output = Vector2<MapSpace>> for MapState<MinimapSpace> {
        fn map(&self, v) {
            self.to_map().map(v)
        }
    }*/
    impl TransformMap<MapSpace, Output = Point2<MinimapSpace>> for MapState<MinimapSpace> {
        fn map(&self, v) {
            (v.to_vector().as_() / Vector2::splat(self.scale)).to_point()
        }
    }
    impl TransformMap<MapSpace, Output = Vector2<MinimapSpace>> for MapState<MinimapSpace> {
        fn map(&self, v) {
            self.from_map().map(v)
        }
    }

    impl TransformMap<MinimapSpace, Output == Size2<CompassSpace>> for MapState<MinimapSpace> {
        fn map(&self, v) {
            v.as_()
        }
    }
    impl TransformMap<MinimapSpace, Output == Vector2<CompassSpace>> for MapState<MinimapSpace> {
        fn map(&self, v) {
            self.counter_rotation()
                .map(|r| Vector2::from_angle(r).rotate(v))
                .unwrap_or(v).as_()
        }
    }
    impl TransformMap<MinimapSpace, Output == Point2<CompassSpace>> for MapState<MinimapSpace> {
        fn map(&self, v) {
            self.map(v.to_vector()).to_point()
        }
    }

    impl TransformMap<CompassSpace, Output == Size2<MinimapSpace>> for MapState<MinimapSpace> {
        fn map(&self, v) {
            v.as_()
        }
    }
    impl TransformMap<CompassSpace, Output == Vector2<MinimapSpace>> for MapState<MinimapSpace> {
        fn map(&self, v) {
            self.rotation()
                .map(|r| Vector2::from_angle(r).rotate(v))
                .unwrap_or(v).as_()
        }
    }
    impl TransformMap<CompassSpace, Output == Point2<MinimapSpace>> for MapState<MinimapSpace> {
        fn map(&self, v) {
            self.map(v.to_vector()).to_point()
        }
    }
}

#[doc(hidden)]
impl MapCalibration {
    pub fn cast_compass_to_worldmap<S>(trans: Transform2<S, CompassSpace>) -> Transform2<S, WorldmapSpace>
    where
        S: Unit<Scalar = f32>,
    {
        transform2_cast(trans)
    }
    pub fn cast_minimap_to_worldmap<S>(trans: Transform2<S, MinimapSpace>) -> Transform2<S, WorldmapSpace>
    where
        S: Unit<Scalar = f32>,
    {
        transform2_cast(trans)
    }
    pub fn cast_worldmap_to_compass<D>(trans: Transform2<CompassSpace, D>) -> Transform2<WorldmapSpace, D>
    where
        D: Unit<Scalar = f32>,
    {
        transform2_cast(trans)
    }
    pub fn cast_worldmap_to_minimap<D>(trans: Transform2<MinimapSpace, D>) -> Transform2<WorldmapSpace, D>
    where
        D: Unit<Scalar = f32>,
    {
        transform2_cast(trans)
    }

    pub fn cast_compass_from_worldmap<S>(trans: Transform2<S, WorldmapSpace>) -> Transform2<S, CompassSpace>
    where
        S: Unit<Scalar = f32>,
    {
        transform2_cast(trans)
    }
    pub fn cast_minimap_from_worldmap<S>(trans: Transform2<S, WorldmapSpace>) -> Transform2<S, MinimapSpace>
    where
        S: Unit<Scalar = f32>,
    {
        transform2_cast(trans)
    }
}

impl MapCalibration {
    pub fn fake_to_compass(&self) -> Transform2<FakeSpace, CompassSpace> {
        let bounds = self.compass_bounds();
        Transform2::from_translation(-bounds.center().to_vector())
    }
    pub fn compass_to_fake(&self) -> Transform2<CompassSpace, FakeSpace> {
        let bounds = self.compass_bounds();
        Transform2::from_translation(bounds.center().to_vector().as_())
    }
}

pub struct CompassTransform;

coord_newtype! {
    impl TransformMap<CompassSpace, Output = Vec<WorldmapSpace>> for CompassTransform {
        fn map(&self, v) {
            v.as_()
        }
    }
    impl TransformMap<WorldmapSpace, Output = Vec<CompassSpace>> for CompassTransform {
        fn map(&self, v) {
            v.as_()
        }
    }
}
