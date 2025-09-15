use {
    arc_atomic::AtomicArc,
    glam::{Vec2, Vec3, Vec3Swizzles},
    glamour::{
        point3, Angle, Box2, Contains, Point2, Point3, Rect, Size2, Transform2, TransformMap, Unit,
        Vector2,
    },
    nexus::data_link::mumble::{MumblePtr, UiState},
    rand::prelude::*,
    std::{
        f32,
        sync::{Arc, LazyLock},
    },
};

pub static MARKERINPUTDATA: LazyLock<AtomicArc<MarkerInputData>> = LazyLock::new(|| AtomicArc::new(Arc::new(MarkerInputData::default())));

// global coordinates / "continent" (game internals, maps, api, ...)
// feet
// e.g. map_center
pub struct MapSpace;
impl Unit for MapSpace {
    type Scalar = f32;
}

// local coordinates (mumblelink)
// meters
// e.g. local_player_pos
pub struct LocalSpace;
impl Unit for LocalSpace {
    type Scalar = f32;
}

// real pixels (imgui, etc)
// e.g. mouse_pos
pub struct ScreenSpace;
impl Unit for ScreenSpace {
    type Scalar = f32;
}

// minimap space; it's a subset of screenspace
// and exists within it as ...a rect boundary
// realistically an offset from screenspace's origin,
// plus clamping?
pub struct MinimapSpace;
impl Unit for MinimapSpace {
    type Scalar = f32;
}

// worldmap space is the same as the above, except unclamped.
// it's basically closer to fakespace than it is to anything else?
pub struct WorldmapSpace;
impl Unit for WorldmapSpace {
    type Scalar = f32;
}

// fake pixels (mumblelink-post-scale)
// includes world-map o.o
// e.g. compass_size
pub struct FakeSpace;
impl Unit for FakeSpace {
    type Scalar = f32;
}

pub type MapPoint = Point2<MapSpace>;

pub type LocalPoint = Point3<LocalSpace>;

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

#[derive(Copy, Debug, PartialEq, Clone)]
pub struct MarkerInputData {
    pub scaling: f32,
    pub local_player_pos: Vec3,
    pub global_player_pos: Vec2,
    pub global_map: Vec2,
    pub compass_size: Vec2,
    pub compass_rotation: f32,
    pub map_scale: f32,
    pub perspective: CurrentPerspective,
    pub minimap_placement: MinimapPlacement,
    pub rotation_enabled: bool,
    pub display_size: Vec2,
    pub sign_obtainer: SignObtainer,
    pub map_id: u32,
}

#[derive(Copy, Debug, Default, PartialEq, Clone)]
pub struct SignObtainer {
    point1: Option<LocalGlobalHolder>,
    point2: Option<LocalGlobalHolder>,
}

impl SignObtainer {
    // TODO: reset on map change
    pub fn prepare(&mut self, local: LocalPoint, global: MapPoint) {
        // we need to be able to figure out the axis directions, let's do this
        // without web requests (to v2 maps api) by taking two points
        // the one thing we don't need to check is the height, and we shouldn't let that
        // skew our distance, either.
        if self.point2.is_none() {
            let local = local.xz();
            if let Some(point1) = self.point1 {
                // take point from 0.5m away in each direction, for accuracy
                if (local.x - point1.local.x).abs() > 5.0 && (local.y - point1.local.y).abs() > 5.0
                {
                    self.point2 = Some(LocalGlobalHolder { local, global });
                    // though, if it's less than a minimum, wipe it and try again
                    let test_sign = self.sign();
                    if test_sign
                        .cmple(Vec2::new(Self::meters_per_feet(), Self::meters_per_feet()))
                        .all()
                    {
                        self.point1 = None;
                        self.point2 = None;
                    }
                }
            } else {
                self.point1 = Some(LocalGlobalHolder { local, global });
            }
        }
        // once we have two points, this becomes a no-op other than the comparison
    }

    pub const fn meters_per_feet() -> f32 {
        0.3048f32
    }

    pub fn has_sign(&self) -> bool {
        self.point2.is_some()
    }

    pub fn sign(&self) -> Vec2 {
        // the most common value, held by 1009/1022 maps from the maps api endpoint is
        // 24.0, 24.0 (2 feet per continent unit).
        let default = Self::meters_per_feet() * 2.0;
        let default_vec2 = Vec2::new(default, -default);
        // if point1 and point2 are each >+/-0.5 away in x,y
        // then its always going to be -1 or 1 for each
        // i think it only matters if they are *different* signs
        if let (Some(point1), Some(point2)) = (self.point1, self.point2) {
            let difference_local: Vec2 = (point2.local - point1.local).into();
            let difference_global: Vec2 = (point2.global - point1.global).into();
            let result = difference_local / difference_global;
            // the smallest map ratio is Mistlock Sanctuary with [12, 12]
            // anything less than like... 8 is too small
            if result.cmple(default_vec2 * 0.6).all() {
                default_vec2
            } else {
                result
            }
        } else {
            // until we find it out, let's just go for Sure man it's the same why not
            default_vec2
        }
    }
}

#[derive(Copy, Debug, Default, PartialEq, Clone)]
pub struct LocalGlobalHolder {
    pub local: Point2<LocalSpace>,
    pub global: MapPoint,
}

#[allow(dead_code)]
impl MarkerInputData {
    // ultimate goals:
    // * screen to local, map
    // * map, local to screen
    //
    // TO-DOs:
    // - [ ] HANDLE ROTATION
    // - [ ] cache transformations per map load
    // - [x] screen <-> fake
    // - [x] fake <-> (minimap, worldmap)
    //   - [x] situational detect
    //   - [x] fake -> minimap
    //   - [x] fake -> worldmap
    //   --- via invertability ---
    //   - [x] minimap -> fake
    //   - [x] worldmap -> fake
    // - [x] (minimap, worldmap) <-> map
    //   - [x] minimap -> map
    //   - [x] worldmap -> map
    //   --- via invertability ---
    //   - [x] map -> minimap
    //   - [x] map -> worldmap
    // - [x] map <-> local
    //   - [x] map -> local
    //   --- via invertability ---
    //   - [x] local -> map

    /*
     *
     * PRIMITIVE TRANSFORMS, ETC!
     *
     */

    // the compass size is already in fakespace, but i have not yet
    // annotated it for the type that it truly is, because on the
    // controller side of my addon, i'm currently using Glam
    // and not Glamour. (given time i'll probably switch anything that
    // touches coordinates over to Glamour, because typing is cool)
    pub fn compass_size(&self) -> Size2<FakeSpace> {
        let compass_vector: FakeVector = self.compass_size.into();
        Size2::from_vector(compass_vector)
    }

    pub fn screen_to_fake(&self) -> Transform2<ScreenSpace, FakeSpace> {
        let screen_scaling_factor = Vector2::splat(1.0 / self.scaling);
        ScreenToFake::from_scale(screen_scaling_factor)
    }

    pub fn screen_bound(&self) -> ScreenBound {
        ScreenBound::from_size(self.display_size.into())
    }

    pub fn fake_bound(&self) -> FakeBound {
        let screen_bound = self.screen_bound();
        // unfortunately transform2 is exclusively a description of
        // matrix transformation, and cannot be used to provide
        // a scalar factor for a Size2, Rect2 or a Box2.
        let fb_size_in_sb = screen_bound.size / self.scaling;
        let fb_size: Size2<FakeSpace> = fb_size_in_sb.cast();
        FakeBound::from_size(fb_size)
    }

    // the conversion to use is dependent upon the current perspective,
    // derived from mumblelink data on whether or not the worldmap itself is open
    //
    // conversions as such are necessary:
    //
    // * fake -> minimap:
    //   (a confined, scaled screenspace (a confinement of fakespace))
    // * fake -> worldmap:
    //   (an unconfined, scaled screenspace)
    //
    // (* minimap -> map
    // * worldmap -> map):
    //   (a conversion of the Point coordinates into Continent coordinates,
    //   in ft and inches; confined or otherwise)
    //
    // it is unlikely one would want to directly use the underlying fake to mini
    // and fake to world, but it is VERY likely one will want to convert from
    // fake to map, and map to fake. (in reality, they'll actually want
    // screenspace to these things, but fake exists thanks to DPI, UI scalings)

    // due to a changing origin, this does not derive itself from
    // the fakespace related display_size stuff
    pub fn minimap_bound(&self) -> MinimapBound {
        let compass_size = self.compass_size();
        MinimapBound::from_size(compass_size.as_())
    }

    // this relies upon the fakespace display_size because it is the
    // boundary *within fakespace* for the minimap
    pub fn fakespace_minimap_bound(&self) -> FakeBound {
        let fakebound = self.fake_bound();
        // fake means we're already scaled proportionate to self.scaling,
        // or the scaling factor provided by Nexus, which is the coordinate system
        // that self.compass_size, the worldmap size and the UI offsets live within
        //
        // having a way to construct *typed scalars* would be nice
        let compass_size = self.compass_size();

        let max = match self.minimap_placement {
            MinimapPlacement::Top => fakebound.size.with_height(compass_size.height),
            MinimapPlacement::Bottom => fakebound.size - Size2::new(0.0, 37.0),
        };
        let min = max - compass_size;
        let min = min.to_vector().to_point();
        let max = max.to_vector().to_point();
        let minimap_bound: Box2<FakeSpace> = Box2::new(min, max);
        minimap_bound.to_rect()
    }

    pub fn fakespace_minimap_drag_bound(&self) -> FakeBound {
        let fakebound = self.fake_bound();
        // fake means we're already scaled proportionate to self.scaling,
        // or the scaling factor provided by Nexus, which is the coordinate system
        // that self.compass_size, the worldmap size and the UI offsets live within
        //
        // having a way to construct *typed scalars* would be nice
        let compass_size = self.compass_size();

        // the thing on the side on hover-over
        let width_to_ignore = 26.0;
        let width_bound = Size2::new(width_to_ignore, 0.0);

        let fakebound_size = fakebound.size - width_bound;

        let max = match self.minimap_placement {
            MinimapPlacement::Top => fakebound_size.with_height(compass_size.height),
            MinimapPlacement::Bottom => fakebound_size - Size2::new(0.0, 37.0),
        };
        let min = max - compass_size;
        let min = min.to_vector().to_point();
        let max = max.to_vector().to_point();
        let minimap_bound: Box2<FakeSpace> = Box2::new(min, max);
        minimap_bound.to_rect()
    }

    pub fn fake_to_minimap(&self, fakespace_minimap_bound: FakeBound) -> FakeToMinimap {
        // without matrices, this would be: point - minimap_bound.min
        // with it, it's just a translation by the *negative*
        // of the minimap_bound, to represent the offset from
        // changing the origin from (0,0) as in fakespace
        // to min, or the top left point (not pixel, its scaled)
        // coordinate of the minimap
        FakeToMinimap::from_translation(-fakespace_minimap_bound.min().to_vector())
    }

    pub fn map_fake_to_minimap(&self, point: FakePoint) -> Option<MinimapPoint> {
        let fakespace_minimap_bound = self.fakespace_minimap_bound();

        if fakespace_minimap_bound.contains(&point) {
            let fake_to_minimap = self.fake_to_minimap(fakespace_minimap_bound);
            Some(fake_to_minimap.map(point))
        } else {
            // the current point cannot be represented within the
            // coordinate system, since it is *fully bounded*,
            // this point would be out of bounds
            None
        }
    }

    pub fn fakespace_worldmap_bound(&self) -> FakeBound {
        self.fake_bound()
    }

    pub fn worldmap_bound(&self) -> WorldmapBound {
        self.fakespace_worldmap_bound().as_()
    }

    pub fn fake_to_worldmap(&self) -> FakeToWorldmap {
        FakeToWorldmap::IDENTITY
    }

    pub fn map_fake_to_worldmap(&self, point: FakePoint) -> Option<WorldmapPoint> {
        // worldmapspace is actually THE SAME as fakespace,
        // it isn't confined at all. but it should still be contemplated about as
        // "separate"; it's a mode!
        //
        // things within fakespace cannot be out of bounds on worldmapspace
        // they are 1:1
        let fakespace_worldmap_bound = self.fakespace_worldmap_bound();
        if fakespace_worldmap_bound.contains(&point) {
            let fake_to_worldmap = self.fake_to_worldmap();
            Some(fake_to_worldmap.map(point))
        } else {
            // the current point cannot be represented within the
            // coordinate system, since it is *fully bounded*,
            // this point would be out of bounds
            None
        }
    }

    // worldmap and minimap both have the same scaling factor of
    // points (fakespace pixels) to continent coordinates (ft and inches)
    // there is very little in what differs between their conversion, in reality?

    pub fn worldmap_to_map(&self) -> WorldmapToMap {
        // the other thing to regard is the common coordinate between the worldmap/fakespace
        // and the map coordinates; the centre, for which is provided as already scaled
        //
        // if map_scale is pt -> continent, then we can regard this as:
        // distance = worldmap_point - worldmap_centre
        // distance_map = distance * map_scale
        // map_point = map_centre + distance_map
        //
        // with matrices, we want to make sure the scalar is being applied to the
        // distance, not the overall resulting coordinates
        let map_centre: Point2<MapSpace> = self.global_map.into();
        let worldmap_bound = self.worldmap_bound();
        let worldmap_centre = worldmap_bound.center();

        // to translate a point from worldspace into mapspace,
        WorldmapToMap::from_translation(-worldmap_centre.to_vector())
            .then_scale(
                // scale the distance by the scaling factor to take it from
                // worldmap to mapspace units
                Vector2::splat(self.map_scale),
            )
            .then_translate(
                // the map space centre is used as a vector
                // when combined with the distance vector,
                // it provides the full offset from the origin
                // in map space, so translate it as such
                map_centre.to_vector(),
            )
    }

    pub fn map_worldmap_to_map(&self, point: WorldmapPoint) -> MapPoint {
        // the scaling factor (map_scale) is applied uniformly to x,y
        // if there are DPI scaling factors, they have already been taken into account
        // as part of the conversion into fakespace
        let worldmap_to_map = self.worldmap_to_map();
        worldmap_to_map.map(point)
    }

    pub fn minimap_to_map_with(&self, minimap_rotation: Option<Angle>, map_scale: f32) -> MinimapToMap {
        // the other thing to regard is the common coordinate between the worldmap/fakespace
        // and the map coordinates; the centre, for which is provided as already scaled
        //
        // if map_scale is pt -> continent, then we can regard this as:
        // distance = worldmap_point - worldmap_centre
        // distance_map = distance * map_scale
        // map_point = map_centre + distance_map
        //
        // with matrices, we want to make sure the scalar is being applied to the
        // distance, not the overall resulting coordinates
        let map_centre: Point2<MapSpace> = self.global_map.into();
        let minimap_bound = self.minimap_bound();
        let minimap_centre = minimap_bound.center();
        let minimap_rotation = minimap_rotation.unwrap_or_default();

        // to translate a point from worldspace into mapspace,
        MinimapToMap::from_translation(-minimap_centre.to_vector().as_())
            .then_rotate(minimap_rotation)
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

    pub fn minimap_to_map(&self) -> MinimapToMap {
        self.minimap_to_map_with(match self.rotation_enabled {
            true => Some(Angle::from_radians(-self.compass_rotation)),
            false => None,
        }, self.map_scale)
    }

    pub fn map_minimap_to_map(&self, point: MinimapPoint) -> MapPoint {
        // the scaling factor (map_scale) is applied uniformly to x,y
        // if there are DPI scaling factors, they have already been taken into account
        // as part of the conversion into fakespace
        let minimap_to_map = self.minimap_to_map();
        minimap_to_map.map(point)
    }

    // finally, map to local
    // between map and local, the common coordinate is no longer the
    // centre of the map, it is in fact the player themselves.
    // thus, the distance is between the player, and a point!

    pub fn map_to_local(&self) -> MapToLocal {
        // map coordinates (continent) are in ft and inches
        // a foot is 0.3048 meters
        // a meter is 1/0.3048 feet
        // if we want local, we have to convert ft to m
        let signs = Vector2::from(self.sign_obtainer.sign());

        let map_player_pos: MapPoint = self.global_player_pos.into();
        let local_player_pos_xz: Point2<LocalSpace> = self.local_player_pos.xz().into();
        // to translate a point from mapspace into localspace,
        MapToLocal::from_translation(
            // first obtain the distance from the common point
            -map_player_pos.to_vector(),
        )
        .then_scale(
            // scale the distance by the scaling factor to take it from
            // mapspace to localspace units
            // ~~local z+ is global y-, so for y scale negatively~~
            // THAT WAS WRONG, EVERY MAP HAS ITS OWN AXES
            signs, //* Vector2::new(scaling_factor_meters_per_feet, scaling_factor_meters_per_feet)
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

    pub fn map_map_to_local(&self, point: MapPoint) -> LocalPoint {
        let map_to_local = self.map_to_local();
        let heightless_local = map_to_local.map(point);
        // the map is 2d space, therefore, for convenience, we shall assume
        // the wanted height is that of the player in this conversion.
        // converting from local -> map -> local is inherently
        // a lossy operation; you lose your third d (it's ok you have two more dont be sad)
        let player_height = self.local_player_pos.y;

        point3!(heightless_local.x, player_height, heightless_local.y)
    }

    pub fn map_local_to_map(&self, point: LocalPoint) -> MapPoint {
        let new_point = point.xz();
        let local_to_map = self.map_to_local().inverse();
        local_to_map.map(new_point)
    }

    /*
     *
     * Usable Transformations
     *
     */

    // choose, based upon the current situation (perspective)
    // how to convert the fake screen coordinate into continent
    pub fn map_fake_to_map(&self, point: FakePoint) -> Option<MapPoint> {
        match self.perspective {
            CurrentPerspective::Minimap => self
                .map_fake_to_minimap(point)
                .map(|intermediate| self.map_minimap_to_map(intermediate)),
            CurrentPerspective::Global => self
                .map_fake_to_worldmap(point)
                .map(|intermediate| self.map_worldmap_to_map(intermediate)),
        }
    }

    pub fn map_to_fake_tf(&self) -> Transform2<MapSpace, FakeSpace> {
        match self.perspective {
            CurrentPerspective::Minimap => {
                let map_to_minimap = self.minimap_to_map().inverse();

                let fakespace_minimap_bound = self.fakespace_minimap_bound();
                let minimap_to_fake = self.fake_to_minimap(fakespace_minimap_bound).inverse();

                let transforms = map_to_minimap.then(minimap_to_fake);
                transforms
            }
            CurrentPerspective::Global => {
                let map_to_worldmap = self.worldmap_to_map().inverse();

                let worldmap_to_fake = self.fake_to_worldmap().inverse();

                let transforms = map_to_worldmap.then(worldmap_to_fake);
                transforms
            }
        }
    }

    pub fn map_map_to_fake(&self, point: MapPoint) -> FakePoint {
        match self.perspective {
            CurrentPerspective::Minimap => {
                let map_to_minimap = self.minimap_to_map().inverse();

                let fakespace_minimap_bound = self.fakespace_minimap_bound();
                let minimap_to_fake = self.fake_to_minimap(fakespace_minimap_bound).inverse();

                let transforms = map_to_minimap.then(minimap_to_fake);
                transforms.map(point)
            }
            CurrentPerspective::Global => {
                let map_to_worldmap = self.worldmap_to_map().inverse();

                let worldmap_to_fake = self.fake_to_worldmap().inverse();

                let transforms = map_to_worldmap.then(worldmap_to_fake);
                transforms.map(point)
            }
        }
    }

    // map space to screenspace
    pub fn map_map_to_screen(&self, point: MapPoint) -> Option<ScreenPoint> {
        let fake_point = self.map_map_to_fake(point);
        let bound = match self.perspective {
            CurrentPerspective::Global => self.fakespace_worldmap_bound(),
            CurrentPerspective::Minimap => self.fakespace_minimap_bound(),
        };
        if !bound.contains(&fake_point) {
            return None;
        }
        let fake_to_screen = self.screen_to_fake().inverse();
        Some(fake_to_screen.map(fake_point))
    }

    // map space to screenspace
    pub fn map_map_to_screen_unchecked(&self, point: MapPoint) -> ScreenPoint {
        let fake_point = self.map_map_to_fake(point);
        let fake_to_screen = self.screen_to_fake().inverse();
        fake_to_screen.map(fake_point)
    }

    pub fn random_map_screen_coordinate(&self) -> ScreenPoint {
        let mut rng = rand::rng();
        let bound = match self.perspective {
            CurrentPerspective::Global => self.fakespace_worldmap_bound(),
            CurrentPerspective::Minimap => self.fakespace_minimap_drag_bound(),
        };
        let tf = self.screen_to_fake().inverse();
        let f_lb = tf.map(bound.min().round().map(|e| e + 2.0));
        let [f_lb_x, f_lb_y] = f_lb.as_array();
        let f_ub = tf.map(bound.max().round().map(|e| e - 2.0));
        let [f_ub_x, f_ub_y] = f_ub.as_array();
        let (lb_x, lb_y) = (*f_lb_x as u32, *f_lb_y as u32);
        let (ub_x, ub_y) = (*f_ub_x as u32, *f_ub_y as u32);
        let x = rng.random_range(lb_x..ub_x);
        let y = rng.random_range(lb_y..ub_y);
        ScreenPoint::new(x as f32, y as f32)
    }

    pub fn map_map_to_screen_drag(
        &self,
        point: MapPoint,
    ) -> (Option<ScreenPoint>, Option<ScreenVector>) {
        let fake_point = self.map_map_to_fake(point);
        let fake_to_screen = self.screen_to_fake().inverse();
        let bound = match self.perspective {
            CurrentPerspective::Global => self.fakespace_worldmap_bound(),
            CurrentPerspective::Minimap => self.fakespace_minimap_bound(),
        };
        // if the point isn't on screen, we can't return its screen coordinates but we can return
        // the amount of screen to move the minimap by to get it within bounds
        if !bound.contains(&fake_point) {
            let screen_centre = bound.center();
            let distance = fake_point - screen_centre;
            let distance = fake_to_screen.map(distance);
            // the current working distance should now be the distance to the point as an f32,
            // which isn't what we want; we want the actual Vector2 that encodes the distance
            return (None, Some(distance));
        }
        (Some(fake_to_screen.map(fake_point)), None)
    }

    // screenspace to map space
    pub fn map_screen_to_map(&self, point: ScreenPoint) -> Option<MapPoint> {
        let screen_to_fake = self.screen_to_fake();
        let fake = screen_to_fake.map(point);
        let map = self.map_fake_to_map(fake);
        map
    }

    pub fn map_screen_to_local(&self, point: ScreenPoint) -> Option<LocalPoint> {
        let map_point = self.map_screen_to_map(point)?;
        let local = self.map_map_to_local(map_point);
        Some(local)
    }

    pub fn is_empty(&self) -> bool {
        self.map_scale == 0.0
    }

    pub fn get(&self) -> Option<&Self> {
        (!self.is_empty()).then_some(self)
    }

    pub fn read() -> Option<Arc<Self>> {
        let data = MARKERINPUTDATA.load();
        (!data.is_empty()).then_some(data)
    }

    pub fn cloned() -> Self {
        (*MARKERINPUTDATA.load()).clone()
    }

    pub fn commit(self) {
        MARKERINPUTDATA.store(Arc::new(self));
    }

    pub fn from_render(display_size: Vec2) {
        let mut data = Self::cloned();
        data.display_size = display_size;
        data.commit();
    }

    pub fn reset_signobtainer() {
        let mut data = Self::cloned();
        data.sign_obtainer = SignObtainer::default();
        data.commit();
    }

    pub fn from_mapchange(map_id: u32) {
        let mut data = Self::cloned();
        data.map_id = map_id;
        data.sign_obtainer = SignObtainer::default();
        data.commit();
    }

    pub fn update_with_mumble_ptr_context(&mut self, mumble: &MumblePtr) {
        self.local_player_pos = Vec3::from_array(mumble.read_avatar().position);
        self.global_player_pos = Vec2::from_array(mumble.read_player_position());
        self.global_map = Vec2::from_array(mumble.read_map_center());
        self.compass_size = Vec2::new(mumble.read_compass_width() as f32, mumble.read_compass_height() as f32);
        self.compass_rotation = mumble.read_compass_rotation();
        self.map_scale = mumble.read_map_scale();
        let ui_state = mumble.read_ui_state();
        self.perspective = ui_state.into();
        self.minimap_placement = ui_state.into();
        self.rotation_enabled = ui_state.contains(UiState::DOES_COMPASS_HAVE_ROTATION_ENABLED);
    }

    #[inline]
    pub fn player_pos_local(&self) -> LocalPoint {
        Point3::from_raw(self.local_player_pos)
    }

    #[inline]
    pub fn player_pos_global(&self) -> MapPoint {
        Point2::from_raw(self.global_player_pos)
    }

    /// Centre of map
    #[inline]
    pub fn map_pos(&self) -> glamour::Point2<MapSpace> {
        Point2::from_raw(self.global_map)
    }
}

impl Default for MarkerInputData {
    fn default() -> Self {
        Self {
            scaling: 1.0,
            local_player_pos: Vec3::ZERO,
            global_player_pos: Vec2::ZERO,
            global_map: Vec2::ZERO,
            compass_size: Default::default(),
            compass_rotation: Default::default(),
            map_scale: 0.0,
            perspective: CurrentPerspective::default(),
            minimap_placement: MinimapPlacement::default(),
            rotation_enabled: false,
            display_size: Vec2::new(1920.0, 1080.0),
            sign_obtainer: Default::default(),
            map_id: 0,
        }
    }
}

#[derive(Debug, Default, PartialEq, Clone, Copy)]
pub enum CurrentPerspective {
    Global, // map_open: true,
    #[default]
    Minimap, // map_open: false,
}

impl From<UiState> for CurrentPerspective {
    fn from(ui_state: UiState) -> Self {
        match ui_state.contains(UiState::IS_MAP_OPEN) {
            true => Self::Global,
            false => Self::Minimap,
        }
    }
}

#[derive(Debug, Default, PartialEq, Clone, Copy)]
pub enum MinimapPlacement {
    Top,
    #[default]
    Bottom,
}

impl From<UiState> for MinimapPlacement {
    fn from(ui_state: UiState) -> Self {
        match ui_state.contains(UiState::IS_COMPASS_TOP_RIGHT) {
            true => Self::Top,
            false => Self::Bottom,
        }
    }
}
