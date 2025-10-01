use glamour::{
    Angle, Box2, Contains,
    Point2, Point3, Rect, Size2,
    Transform2, TransformMap, Unit,
    Vector2,
    Vec3Swizzles,
};

/// global coordinates / "continent"
/// (feet)
///
/// game internals, maps, api, ...
/// e.g. map_center
pub struct MapSpace;
impl Unit for MapSpace {
    type Scalar = f32;
}

/// local mumblelink coordinates
/// (meters)
///
/// e.g. local_player_pos
pub struct LocalSpace;
impl Unit for LocalSpace {
    type Scalar = f32;
}

/// internal draw coordinates
/// (inches)
pub struct GameSpace;
impl Unit for GameSpace {
    type Scalar = f32;
}

/// real pixels (imgui, etc)
///
/// e.g. mouse_pos
pub struct ScreenSpace;
impl Unit for ScreenSpace {
    type Scalar = f32;
}

/// minimap (compass) space
///
/// it's a subset of [ScreenSpace]
/// and exists within it as ...a rect boundary
/// realistically an offset from screenspace's origin,
/// plus clamping?
pub struct MinimapSpace;
impl Unit for MinimapSpace {
    type Scalar = f32;
}

/// unclamped [MinimapSpace]
///
/// closer to [FakeSpace] than it is to anything else?
pub struct WorldmapSpace;
impl Unit for WorldmapSpace {
    type Scalar = f32;
}

/// fake pixels (mumblelink-post-scale)
///
/// includes world-map o.o
/// e.g. compass_size
pub struct FakeSpace;
impl Unit for FakeSpace {
    type Scalar = f32;
}

pub type MapPoint = Point2<MapSpace>;

pub type LocalPoint = Point3<LocalSpace>;
/// TODO: new space for swizzled units
pub type LocalPoint2 = Point2<LocalSpace>;

pub type ScreenPoint = Point2<ScreenSpace>;
pub type ScreenBound = Rect<ScreenSpace>;
pub type ScreenVector = Vector2<ScreenSpace>;

pub type FakePoint = Point2<FakeSpace>;
pub type FakeVector = Vector2<FakeSpace>;
pub type FakeBound = Rect<FakeSpace>;

pub type MinimapPoint = Point2<MinimapSpace>;
pub type WorldmapPoint = Point2<WorldmapSpace>;
pub type MinimapBound = Rect<MinimapSpace>;
pub type WorldmapBound = Rect<WorldmapSpace>;

pub type ScreenToFake = Transform2<ScreenSpace, FakeSpace>;

pub type FakeToMinimap = Transform2<FakeSpace, MinimapSpace>;
pub type FakeToWorldmap = Transform2<FakeSpace, WorldmapSpace>;

pub type MinimapToMap = Transform2<MinimapSpace, MapSpace>;
pub type WorldmapToMap = Transform2<WorldmapSpace, MapSpace>;

pub type MapToLocal = Transform2<MapSpace, LocalSpace>;


impl FakeSpace {
    /// Conversion from [MinimapSpace::fake_bound_with](minimap bounds)
    pub fn to_minimap(fakespace_minimap_bound: FakeBound) -> FakeToMinimap {
        // without matrices, this would be: point - minimap_bound.min
        // with it, it's just a translation by the *negative*
        // of the minimap_bound, to represent the offset from
        // changing the origin from (0,0) as in fakespace
        // to min, or the top left point (not pixel, its scaled)
        // coordinate of the minimap
        Transform2::from_translation(-fakespace_minimap_bound.min().to_vector())
    }

    pub fn from_screen(scaling: f32) -> ScreenToFake {
        let screen_scaling_factor = Vector2::splat(1.0 / scaling);
        Transform2::from_scale(screen_scaling_factor)
    }

    pub fn screen_size(scaling: f32, size: Size2<ScreenSpace>) -> Size2<FakeSpace> {
        Self::from_screen(scaling)
            .map(size.to_vector())
            .to_size()
    }

    #[inline]
    pub fn bound(size: Size2<FakeSpace>) -> FakeBound {
        Rect::from_size(size)
    }

    /// worldmapspace is actually THE SAME as fakespace,
    ///
    /// it isn't confined at all. but it should still be contemplated about as
    /// "separate"; it's a mode!
    ///
    /// things within fakespace cannot be out of bounds on worldmapspace
    /// they are 1:1
    pub fn to_worldmap() -> FakeToWorldmap {
        Transform2::IDENTITY
    }

    /// Fake pixels to [continent](MapSpace) coordinates
    ///
    /// map_scale is pt -> continent.
    /// the other thing to regard is the common coordinate between the worldmap/fakespace
    /// and the map coordinates; the centre, which is provided as already scaled
    ///
    /// worldmap and minimap both have the same scaling factor of
    /// points (fakespace pixels) to continent coordinates (ft and inches)
    /// there is very little in what differs between their conversion, in reality?
    pub fn to_map(map_scale: f32, map_rotation: Option<Angle>, map_centre: Point2<MapSpace>, fake_centre: Point2<FakeSpace>) -> Transform2<FakeSpace, MapSpace> {
        // we can regard this as:
        // distance = worldmap_point - worldmap_centre
        // distance_map = distance * map_scale
        // map_point = map_centre + distance_map
        //
        // with matrices, we want to make sure the scalar is being applied to the
        // distance, not the overall resulting coordinates
        // to translate a point from worldspace into mapspace,

        let trans = Transform2::from_translation(-fake_centre.to_vector());
        let trans = match map_rotation {
            Some(rotation) => trans.then_rotate(Angle::new(-rotation.radians)),
            None => trans,
        };
        trans
            .then_scale(
                // scale the distance by the scaling factor to take it from
                // worldmap to mapspace units
                Vector2::splat(map_scale),
            )
            .then_translate(
                // the map space centre is used as a vector
                // when combined with the distance vector,
                // it provides the full offset from the origin
                // in map space, so translate it as such
                map_centre.to_vector(),
            )
    }
}

impl WorldmapSpace {
    #[inline]
    pub fn bound(size: Size2<FakeSpace>) -> WorldmapBound {
        Self::fake_bound(Rect::from_size(size.as_()))
    }

    /// the scaling factor (map_scale) is applied uniformly to x,y
    ///
    /// if there are DPI scaling factors, they have already been taken into account
    /// as part of the conversion into fakespace
    pub fn to_map(map_scale: f32, map_centre: Point2<MapSpace>, worldmap_centre: WorldmapPoint) -> WorldmapToMap {
        let fake_centre = worldmap_centre.as_();
        let trans = FakeSpace::to_map(map_scale, None, map_centre, fake_centre);
        Self::fake_then(trans)
    }

    /// worldmapspace is actually THE SAME as fakespace,
    /// it isn't confined at all. but it should still be contemplated about as
    /// "separate"; it's a mode!
    pub fn fake_point(point: FakePoint, worldmap_bound: WorldmapBound) -> Option<WorldmapPoint> {
        // things within fakespace cannot be out of bounds on worldmapspace
        // they are 1:1
        let fakespace_worldmap_bound = worldmap_bound.as_();
        if fakespace_worldmap_bound.contains(&point) {
            //let fake_to_worldmap = FakeSpace::to_worldmap();
            let value = point.as_();
            Some(value)
        } else {
            // the current point cannot be represented within the
            // coordinate system, since it is *fully bounded*,
            // this point would be out of bounds
            None
        }
    }

    #[inline]
    pub fn fake_then<D>(trans: Transform2<FakeSpace, D>) -> Transform2<WorldmapSpace, D> where
        D: Unit<Scalar = <FakeSpace as Unit>::Scalar>,
    {
        //FakeSpace::to_minimap().then(trans);
        Transform2::from_matrix_unchecked(trans.matrix)
    }

    #[inline]
    pub fn fake_vec(v: FakeVector) -> Vector2<WorldmapSpace> {
        //Fake::to_worldmap().map(v)
        v.as_()
    }

    #[inline]
    pub fn fake_bound(b: FakeBound) -> WorldmapBound {
        //Fake::to_worldmap().map(b)
        b.as_()
    }
}

impl MinimapSpace {
    pub const BOTTOM_OFFSET: Size2<FakeSpace> = Size2::new(0.0, 37.0);
    /// the thing on the side on hover-over
    pub const EDGE_DEAD_ZONE: Size2<FakeSpace> = Size2::new(26.0, 0.0);

    pub fn to_map(map_scale: f32, minimap_rotation: Option<Angle>, map_centre: Point2<MapSpace>, minimap_centre: Point2<MinimapSpace>) -> MinimapToMap {
        let fake_centre = minimap_centre.as_();
        let trans = FakeSpace::to_map(map_scale, minimap_rotation, map_centre, fake_centre);
        Self::fake_then(trans)
    }

    /// Minimap screen location in [FakeSpace]
    ///
    /// the conversion to use is dependent upon the current perspective,
    /// derived from mumblelink data on whether or not the worldmap itself is open
    ///
    /// conversions as such are necessary:
    ///
    /// * fake -> minimap:
    ///   (a confined, scaled screenspace (a confinement of fakespace))
    /// * fake -> worldmap:
    ///   (an unconfined, scaled screenspace)
    ///
    /// (* minimap -> map
    /// * worldmap -> map):
    ///   (a conversion of the Point coordinates into Continent coordinates,
    ///   in ft and inches; confined or otherwise)
    ///
    /// it is unlikely one would want to directly use the underlying fake to mini
    /// and fake to world, but it is VERY likely one will want to convert from
    /// fake to map, and map to fake. (in reality, they'll actually want
    /// screenspace to these things, but fake exists thanks to DPI, UI scalings)
    ///
    /// due to a changing origin, this does not derive itself from
    /// the fakespace related display_size stuff
    ///
    /// this relies upon the fakespace display_size because it is the
    /// boundary *within fakespace* for the minimap
    pub fn fake_bound_with(minimap_placement: MinimapPlacement, compass_size: Size2<MinimapSpace>, display_size: Size2<FakeSpace>) -> FakeBound {
        // fake means we're already scaled proportionate to self.scaling,
        // or the scaling factor provided by Nexus, which is the coordinate system
        // that self.compass_size, the worldmap size and the UI offsets live within
        //
        // having a way to construct *typed scalars* would be nice

        let compass_size = compass_size.as_();

        let max = match minimap_placement {
            MinimapPlacement::Top => display_size.with_height(compass_size.height),
            MinimapPlacement::Bottom => display_size - Self::BOTTOM_OFFSET,
        };
        let min = max - compass_size;
        let min = min.to_vector().to_point();
        let max = max.to_vector().to_point();
        let minimap_bound: Box2<FakeSpace> = Box2::new(min, max);
        minimap_bound.to_rect()
    }

    pub fn fake_bound_for_drag(minimap_placement: MinimapPlacement, compass_size: Size2<MinimapSpace>, display_size: Size2<FakeSpace>) -> FakeBound {
        // fake means we're already scaled proportionate to scaling,
        // or the scaling factor provided by Nexus, which is the coordinate system
        // that compass_size, the worldmap size and the UI offsets live within
        //
        // having a way to construct *typed scalars* would be nice

        let width_bound = Self::EDGE_DEAD_ZONE;
        let compass_size = compass_size.as_();
        let fakebound_size = display_size - width_bound;

        let max = match minimap_placement {
            MinimapPlacement::Top => fakebound_size.with_height(compass_size.height),
            MinimapPlacement::Bottom => fakebound_size - Self::BOTTOM_OFFSET,
        };
        let min = max - compass_size;
        let min = min.to_vector().to_point();
        let max = max.to_vector().to_point();
        let minimap_bound: Box2<FakeSpace> = Box2::new(min, max);
        minimap_bound.to_rect()
    }

    #[inline]
    pub fn bound(compass_size: Size2<MinimapSpace>) -> MinimapBound {
        Self::fake_bound(Rect::from_size(compass_size.as_()))
    }

    /// Conversion from [MinimapSpace::fake_bound_with](minimap bounds)
    pub fn to_fake(fakespace_minimap_bound: FakeBound) -> Transform2<MinimapSpace, FakeSpace> {
        FakeSpace::to_minimap(fakespace_minimap_bound).inverse()
    }

    #[inline]
    pub fn fake_then<D>(trans: Transform2<FakeSpace, D>) -> Transform2<MinimapSpace, D> where
        D: Unit<Scalar = <FakeSpace as Unit>::Scalar>,
    {
        //FakeSpace::to_minimap().then(trans);
        Transform2::from_matrix_unchecked(trans.matrix)
    }

    #[inline]
    pub fn fake_vec(v: FakeVector) -> Vector2<MinimapSpace> {
        //Fake::to_worldmap().map(v)
        v.as_()
    }

    #[inline]
    pub fn fake_bound(b: FakeBound) -> MinimapBound {
        //Fake::to_worldmap().map(b)
        b.as_()
    }
}

impl LocalSpace {
    #[inline]
    pub fn to2(point: LocalPoint) -> LocalPoint2 {
        point.xz()
    }

    /// Scale to [MapSpace] using a common reference point
    ///
    /// continent coordinates are in ft and inches
    /// if we want local, we have to convert ft to m
    pub fn from_map(signs: MapLocalScale, map_player_pos: MapPoint, local_player_pos_xz: LocalPoint2) -> MapToLocal {
        // finally, map to local
        // between map and local, the common coordinate is no longer the
        // centre of the map, it is in fact the player themselves.
        // thus, the distance is between the player, and a point!

        // a foot is 0.3048 meters
        // a meter is 1/0.3048 feet

        // to translate a point from mapspace into localspace,
        Transform2::from_translation(
            // first obtain the distance from the common point
            -map_player_pos.to_vector(),
        )
        .then_scale(
            // scale the distance by the scaling factor to take it from
            // mapspace to localspace units
            // ~~local z+ is global y-, so for y scale negatively~~
            // THAT WAS WRONG, EVERY MAP HAS ITS OWN AXES
            signs.scale, //* Vector2::new(scaling_factor_meters_per_feet, scaling_factor_meters_per_feet)
        )
        .then_translate(
            // the player's position is used as a vector
            // when combined with the distance vector,
            // it provides the full offset from the origin
            // in local space, so translate it as such
            //
            // the player's local position is a coordinate in 3D space
            // to translate the 2D point, we must drop the height
            // in our scheme, this is the Y coordinate
            local_player_pos_xz.to_vector(),
        )
    }
}

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
    pub fn for_map(map_id: u32) -> Option<Self> {
        use {
            crate::map::Map,
            std::{
                collections::BTreeMap,
                sync::LazyLock,
            },
        };

        #[cfg(not(feature = "gzip"))]
        const MAPS_SIGN_JSON: &'static str = include_str!("../data/maps-sign.json");
        static MAPS_SIGN: LazyLock<BTreeMap<u32, Map>> = LazyLock::new(|| {
            let maps = serde_json::from_str::<BTreeMap<u32, Map>>(MAPS_SIGN_JSON);
            if let Err(_e) = &maps {
                log::error!("failed to deserialize map cache: {_e}");
            }
            maps.unwrap_or_default()
        });

        let map = MAPS_SIGN.get(&map_id)?;

        Some(Self::with_game_bounds(map.map_rect(), map.continent_rect()))
    }
}

impl Default for MapLocalScale {
    fn default() -> Self {
        Self::COMMON
    }
}

#[derive(Debug, Default, PartialOrd, Ord, PartialEq, Eq, Clone, Copy, Hash)]
pub enum CurrentPerspective {
    Global, // map_open: true,
    #[default]
    Minimap, // map_open: false,
}

#[derive(Debug, Default, PartialOrd, Ord, PartialEq, Eq, Clone, Copy, Hash)]
pub enum MinimapPlacement {
    Top,
    #[default]
    Bottom,
}

#[cfg(feature = "mumblelink-nexus")]
mod mumblelink_nexus_impl {
    use {
        gw2_mumble_nexus::UiState,
        super::{CurrentPerspective, MinimapPlacement},
    };

    impl From<UiState> for MinimapPlacement {
        fn from(ui_state: UiState) -> Self {
            match ui_state.contains(UiState::IS_COMPASS_TOP_RIGHT) {
                true => Self::Top,
                false => Self::Bottom,
            }
        }
    }

    impl From<UiState> for CurrentPerspective {
        fn from(ui_state: UiState) -> Self {
            match ui_state.contains(UiState::IS_MAP_OPEN) {
                true => Self::Global,
                false => Self::Minimap,
            }
        }
    }
}

#[cfg(feature = "mumblelink-arcloader")]
mod mumblelink_arcloader_impl {
    use {
        arcloader_mumblelink::gw2_mumble::UiState,
        super::{CurrentPerspective, MinimapPlacement},
    };

    impl From<UiState> for MinimapPlacement {
        fn from(ui_state: UiState) -> Self {
            match ui_state.contains(UiState::IS_COMPASS_TOP_RIGHT) {
                true => Self::Top,
                false => Self::Bottom,
            }
        }
    }

    impl From<UiState> for CurrentPerspective {
        fn from(ui_state: UiState) -> Self {
            match ui_state.contains(UiState::IS_MAP_OPEN) {
                true => Self::Global,
                false => Self::Minimap,
            }
        }
    }
}
