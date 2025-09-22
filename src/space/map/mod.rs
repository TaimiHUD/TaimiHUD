use {
    crate::{
        marker::atomic::MarkerInputData,
        space::DrawSpace,
    },
    glamour::{Angle, Point2, Point3, Box3, Box2, TransformMap, Vector2},
    taimi_meta::coords::{CurrentPerspective, FakeSpace, LocalSpace, MapSpace, MinimapSpace, ScreenSpace},
};

#[derive(Debug, Clone)]
pub struct MapTarget {
    pub bounds_draw: Box3<DrawSpace>,
    pub bounds_fake: Box2<FakeSpace>,
    pub bounds_screen: Box2<ScreenSpace>,
    pub bounds_global: Box2<MapSpace>,
    pub rotation: Option<Angle>,
    pub perspective: CurrentPerspective,
    pub scale_map: f32,
    pub scale_fake: f32,
    pub centre: Point2<DrawSpace>,
    pub local_per_map: Vector2<LocalSpace>,
}

impl MapTarget {
    pub const HEIGHT_OFFSET_BELOW: f32 = -200.0;
    pub const HEIGHT_OFFSET_ABOVE: f32 = 200.0;
    pub const HEIGHT_OFFSET_BELOW_MINI: f32 = -75.0;
    pub const HEIGHT_OFFSET_ABOVE_MINI: f32 = 95.0;

    pub fn new(map_data: &MarkerInputData) -> Self {
        // TODO: clean all this up
        let (map_rect_fake, fake_to_map, rotation) = match map_data.perspective {
            CurrentPerspective::Minimap => {
                let bound = map_data.fakespace_minimap_bound();
                let map = map_data.fake_to_minimap(bound)
                    .then(MinimapSpace::to_map(map_data.map_scale, None, map_data.map_pos(), map_data.minimap_bound().center()));
                    let rotation = map_data.rotation_enabled.then_some(glamour::Angle::from_radians(-map_data.compass_rotation));
                    (bound, map, rotation)
            },
            CurrentPerspective::Global => {
                let bound = map_data.fake_bound();
                let map = map_data.fake_to_worldmap()
                    .then(map_data.worldmap_to_map());
                    (bound, map, None)
            },
        };
        let map_bounds_fake = (map_rect_fake.min(), map_rect_fake.max());
        let map_bounds_fake = Box2::new(
            map_bounds_fake.0,
            map_bounds_fake.1,
        );
        let fake_to_screen = map_data.screen_to_fake().inverse();
        let map_bounds_screen = (
            fake_to_screen.map(map_bounds_fake.min),
            fake_to_screen.map(map_bounds_fake.max),
        );
        let map_bounds_screen = Box2::new(
            map_bounds_screen.0,
            map_bounds_screen.1,
        );
        let map_bounds_global = (
            fake_to_map.map(map_bounds_fake.min),
            fake_to_map.map(map_bounds_fake.max),
        );
        let map_to_local = map_data.map_to_local();
        let map_bounds_local = (
            map_to_local.map(map_bounds_global.0),
            map_to_local.map(map_bounds_global.1),
        );
        let map_bounds_local = Box2::new(
            map_bounds_local.0.min(map_bounds_local.1),
            map_bounds_local.0.max(map_bounds_local.1),
        );

        let (height_offset_below, height_offset_above) = match map_data.perspective {
            // TODO: stop using player position here...
            CurrentPerspective::Global => (Self::HEIGHT_OFFSET_BELOW, Self::HEIGHT_OFFSET_ABOVE),
            // XXX: adjust if airborne? need little height arrow indicators...
            CurrentPerspective::Minimap => (Self::HEIGHT_OFFSET_BELOW_MINI, Self::HEIGHT_OFFSET_ABOVE_MINI),
        };
        let bounds: Box3<LocalSpace> = Box3::new(
            Point3::new(map_bounds_local.min.x, map_data.player_pos_local().y + height_offset_below, map_bounds_local.min.y),
            Point3::new(map_bounds_local.max.x, map_data.player_pos_local().y + height_offset_above, map_bounds_local.max.y),
        );

        Self {
            bounds_draw: bounds.into(),
            bounds_screen: map_bounds_screen,
            bounds_fake: map_bounds_fake,
            bounds_global: Box2::new(map_bounds_global.0, map_bounds_global.1),
            rotation,
            perspective: map_data.perspective,
            scale_map: map_data.map_scale,
            scale_fake: map_data.scaling,
            centre: map_to_local.map(map_data.map_pos()),
            local_per_map: Vector2::from(map_data.sign_obtainer.sign()),
        }
    }
}
