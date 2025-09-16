use {
    arc_atomic::AtomicArc,
    glam::{Vec2, Vec3, Vec3Swizzles},
    glamour::{
        point3, Angle, Contains, Point2, Point3, Size2, Transform2, TransformMap,
    },
    nexus::data_link::mumble::{MumblePtr, UiState},
    rand::prelude::*,
    std::{
        f32,
        sync::{Arc, LazyLock},
    },
};

pub static MARKERINPUTDATA: LazyLock<AtomicArc<MarkerInputData>> = LazyLock::new(|| AtomicArc::new(Arc::new(MarkerInputData::default())));

pub use taimi_meta::coords::*;

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
        MapLocalScale::METRES_PER_FEET
    }

    pub fn has_sign(&self) -> bool {
        self.point2.is_some()
    }

    pub fn sign(&self) -> Vec2 {
        // the most common value, held by 1009/1022 maps from the maps api endpoint is
        // 24.0, 24.0 (2 feet per continent unit).
        let default_vec2 = MapLocalScale::COMMON.scale.to_raw();
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

    pub fn scale(&self) -> MapLocalScale {
        MapLocalScale::with_scale(self.sign().into())
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
        FakeSpace::from_screen(self.scaling)
    }

    pub fn screen_size(&self) -> Size2<ScreenSpace> {
        self.display_size.into()
    }

    pub fn screen_bound(&self) -> ScreenBound {
        ScreenBound::from_size(self.screen_size())
    }

    pub fn fake_size(&self) -> Size2<FakeSpace> {
        let size = self.screen_size();
        // unfortunately transform2 is exclusively a description of
        // matrix transformation, and cannot be used to provide
        // a scalar factor for a Size2, Rect2 or a Box2.
        //self.screen_to_fake().map(size)
        (size.to_raw() / self.scaling).into()
    }

    pub fn fake_bound(&self) -> FakeBound {
        FakeBound::from_size(self.fake_size())
    }

    pub fn minimap_bound(&self) -> MinimapBound {
        let compass_size = self.compass_size();
        MinimapBound::from_size(compass_size.as_())
    }

    pub fn fakespace_minimap_bound(&self) -> FakeBound {
        MinimapSpace::fake_bound_with(self.minimap_placement, self.compass_size().as_(), self.fake_size())
    }

    pub fn fakespace_minimap_drag_bound(&self) -> FakeBound {
        MinimapSpace::fake_bound_for_drag(self.minimap_placement, self.compass_size().as_(), self.fake_size())
    }

    pub fn fake_to_minimap(&self, fakespace_minimap_bound: FakeBound) -> FakeToMinimap {
        FakeSpace::to_minimap(fakespace_minimap_bound)
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
        WorldmapSpace::fake_point(point, self.worldmap_bound())
    }

    pub fn worldmap_to_map(&self) -> WorldmapToMap {
        WorldmapSpace::to_map(self.map_scale, self.map_pos(), self.worldmap_bound().center())
    }

    pub fn map_worldmap_to_map(&self, point: WorldmapPoint) -> MapPoint {
        let worldmap_to_map = self.worldmap_to_map();
        worldmap_to_map.map(point)
    }

    pub fn minimap_rotation(&self) -> Option<Angle> {
        match self.rotation_enabled {
            true => Some(Angle::from_radians(self.compass_rotation)),
            false => None,
        }
    }

    pub fn minimap_to_map(&self) -> MinimapToMap {
        MinimapSpace::to_map(self.map_scale, self.minimap_rotation(), self.map_pos(), self.minimap_bound().center())
    }

    pub fn map_minimap_to_map(&self, point: MinimapPoint) -> MapPoint {
        let minimap_to_map = self.minimap_to_map();
        minimap_to_map.map(point)
    }

    pub fn map_to_local(&self) -> MapToLocal {
        LocalSpace::from_map(self.sign_obtainer.scale(), self.player_pos_global(), self.player_pos_local2())
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
    pub fn player_pos_local2(&self) -> LocalPoint2 {
        LocalSpace::to2(self.player_pos_local())
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
