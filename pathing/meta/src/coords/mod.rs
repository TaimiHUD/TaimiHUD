use {
    crate::ui::MinimapPlacement,
    glamour::{
        Angle,
        Box3,
        Contains,
        Matrix4,
        Point2,
        Point3,
        Rect,
        Size2,
        Transform2,
        Transform3,
        TransformMap,
        Unit,
        Vec3Swizzles,
        Vector2,
        Vector3,
        Vector4,
    },
};

mod macros;
mod obtainer;
mod scale;

pub use self::{macros::coord_newtype, obtainer::SignObtainer, scale::MapLocalScale};

coord_newtype! {
/// global coordinates / "continent"
/// (feet)
///
/// game internals, maps, api, ...
/// e.g. map_center
pub struct MapSpace([f32; 2]);
}

coord_newtype! {
/// local mumblelink coordinates
/// (meters)
///
/// e.g. local_player_pos
pub struct LocalSpace([f32; 3]);
}

coord_newtype! {
/// internal draw coordinates
/// (inches)
pub struct GameSpace([f32; 3]);
}

coord_newtype! {
/// real pixels (imgui, etc)
///
/// e.g. mouse_pos
pub struct ScreenSpace([f32; 2]);
}

coord_newtype! {
/// 3D [viewport draw space](https://learn.microsoft.com/en-us/windows/win32/direct3d9/projection-transform),
/// left-handed coordinates between `-1.0..=1.0` are visible while the depth ranges from `0.0..=1.0`.
///
/// basically just unscaled or proportional [ScreenSpace]
pub struct ClipSpace([f32; 3]);
}

coord_newtype! {
/// minimap (compass) space
///
/// (potentially) rotated [WorldmapSpace]
///
/// with no rotation enabled, equivalent to [MinimapSpace]
pub struct MinimapSpace([f32; 2]);
}

coord_newtype! {
/// minimap (compass) space
///
/// it's a subset of [ScreenSpace]
/// and exists within it as ...a rect boundary
/// realistically an offset from screenspace's origin,
/// plus clamping?
///
/// equivalent to [WorldmapSpace]
pub struct CompassSpace([f32; 2]);
}

coord_newtype! {
/// unclamped [MinimapSpace]
///
/// closer to [FakeSpace] than it is to anything else?
pub struct WorldmapSpace([f32; 2]);
}

coord_newtype! {
/// fake pixels (mumblelink-post-scale)
///
/// includes world-map o.o
/// e.g. compass_size
pub struct FakeSpace([f32; 2]);
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

impl ScreenSpace {
    #[inline]
    pub fn from_viewport(viewport: Rect<Self>) -> Transform2<ClipSpace, Self> {
        ClipSpace::to_viewport(viewport)
    }

    #[inline]
    pub fn from_viewport3(viewport: Rect<Self>) -> Transform3<ClipSpace, Self> {
        ClipSpace::to_viewport3(viewport)
    }
}

impl ClipSpace {
    pub const MIDPOINT: Point3<Self> = Point3::new(0.0, 0.0, 0.5);

    /// Visible section of space
    pub const VIEWPORT_BOUNDS: Box3<Self> = Box3::new(Point3::new(-1.0, -1.0, 0.0), Point3::ONE);

    #[inline]
    pub fn to_viewport<D>(viewport: Rect<D>) -> Transform2<Self, D>
    where
        D: Unit<Scalar = <ClipSpace as Unit>::Scalar>,
    {
        let size_2 = (viewport.size / 2.0).to_vector();
        Transform2::from_scale(size_2.as_::<ClipSpace>())
            .then_translate(viewport.origin.to_vector() + size_2)
    }

    #[inline]
    pub fn to_viewport3<D>(viewport: Rect<D>) -> Transform3<Self, D>
    where
        D: Unit<Scalar = <ClipSpace as Unit>::Scalar>,
    {
        let trans = Self::to_viewport::<D>(viewport).matrix;
        Transform3::from_matrix_unchecked(Matrix4::from_mat3(trans))
    }
}

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
        Self::from_screen(scaling).map(size.to_vector()).to_size()
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
    pub fn to_map(
        map_scale: f32,
        map_rotation: Option<Angle>,
        map_centre: Point2<MapSpace>,
        fake_centre: Point2<FakeSpace>,
    ) -> Transform2<FakeSpace, MapSpace> {
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

    #[inline]
    pub fn from_viewport(viewport: Rect<Self>) -> Transform2<ClipSpace, Self> {
        ClipSpace::to_viewport(viewport)
    }

    #[inline]
    pub fn from_viewport3(viewport: Rect<Self>) -> Transform3<ClipSpace, Self> {
        ClipSpace::to_viewport3(viewport)
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
    pub fn to_map(
        map_scale: f32,
        map_centre: Point2<MapSpace>,
        worldmap_centre: WorldmapPoint,
    ) -> WorldmapToMap {
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
    pub fn fake_then<D>(trans: Transform2<FakeSpace, D>) -> Transform2<WorldmapSpace, D>
    where
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
    pub fn to_map(
        map_scale: f32,
        minimap_rotation: Option<Angle>,
        map_centre: Point2<MapSpace>,
        minimap_centre: Point2<MinimapSpace>,
    ) -> MinimapToMap {
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
    pub fn fake_bound_with(
        minimap_placement: MinimapPlacement,
        compass_size: Size2<MinimapSpace>,
        display_size: Size2<FakeSpace>,
    ) -> FakeBound {
        // fake means we're already scaled proportionate to the scaling factor,
        // which is the coordinate system that self.compass_size, the worldmap size
        // and the UI offsets live within
        //
        // having a way to construct *typed scalars* would be nice

        let compass_size = compass_size.as_();

        match minimap_placement {
            placement => {
                let bounds = display_size - MinimapPlacement::BOTTOM_OFFSET.as_();
                placement.bounds(compass_size, Rect::from_size(bounds))
            },
            #[cfg(todo)]
            placement => {
                let max = match placement {
                    MinimapPlacement::Top => display_size.with_height(compass_size.height),
                    MinimapPlacement::Bottom => display_size - MinimapPlacement::BOTTOM_OFFSET,
                };
                let min = max - compass_size;
                let min = min.to_vector().to_point();
                let max = max.to_vector().to_point();
                let minimap_bound: Box2<FakeSpace> = Box2::new(min, max);
                minimap_bound.to_rect()
            },
        }
    }

    pub fn fake_bound_for_drag(
        minimap_placement: MinimapPlacement,
        compass_size: Size2<MinimapSpace>,
        display_size: Size2<FakeSpace>,
    ) -> FakeBound {
        let mut bounds = Self::fake_bound_with(minimap_placement, compass_size, display_size);
        bounds.origin += MinimapPlacement::INSET_DEAD_ZONE.to_vector();
        bounds.size -= MinimapPlacement::DEAD_ZONE_SIZE;
        bounds
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
    pub fn fake_then<D>(trans: Transform2<FakeSpace, D>) -> Transform2<MinimapSpace, D>
    where
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
    pub fn from_map(
        signs: MapLocalScale,
        map_player_pos: MapPoint,
        local_player_pos_xz: LocalPoint2,
    ) -> MapToLocal {
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

#[inline]
pub const fn f32_bits<const N: usize>(f: [f32; N]) -> [u32; N] {
    unsafe {
        // XXX: transmute_unchecked is unstable...
        core::mem::transmute_copy(&f)
    }
}
#[inline]
pub fn vec_bits<const N: usize, T>(f: T) -> [u32; N]
where
    T: Into<[f32; N]>,
{
    f32_bits(f.into())
}
pub fn vec_eq<const N: usize, T>(lhs: T, rhs: T) -> bool
where
    T: Into<[f32; N]>,
{
    vec_bits(lhs) == vec_bits(rhs)
}

pub fn transform2_cast<S2, D2, S, D>(trans: Transform2<S, D>) -> Transform2<S2, D2>
where
    S: Unit,
    S2: Unit<Scalar = S::Scalar>,
    D: Unit<Scalar = S::Scalar>,
    D2: Unit<Scalar = D::Scalar>,
    S2::Scalar: glamour::FloatScalar,
{
    Transform2::from_matrix_unchecked(trans.matrix)
}
pub fn transform3_cast<S2, D2, S, D>(trans: Transform3<S, D>) -> Transform3<S2, D2>
where
    S: Unit,
    S2: Unit<Scalar = S::Scalar>,
    D: Unit<Scalar = S::Scalar>,
    D2: Unit<Scalar = D::Scalar>,
    S2::Scalar: glamour::FloatScalar,
{
    Transform3::from_matrix_unchecked(trans.matrix)
}

/// XXX: [glam::Matrix4::look_to_lh] does not normalize the up cross vector,
/// but we do?
///
/// the difference is probably just a rounding error or we're wrong idk
pub fn camera_view<U: Unit>(pos: Point3<U>, front: Vector3<U>, up: Vector3<U>) -> Matrix4<U::Scalar>
where
    U::Scalar: glamour::FloatScalar + glamour::SignedScalar,
{
    match () {
        #[cfg(todo)]
        () => Matrix4::look_to_lh(pos.to_untyped(), front.to_untyped(), up.to_untyped()),
        () => {
            let f = -front;
            let right = f.cross(up).normalize();
            let up = right.cross(f).normalize();
            let pos = pos.to_vector();
            Matrix4::from_cols(
                right.extend(-pos.dot(right)).to_untyped(),
                up.extend(-pos.dot(up)).to_untyped(),
                front.extend(pos.dot(f)).to_untyped(),
                Vector4::W,
            )
            .transpose()
        },
    }
}

pub fn billboard_from_look(mut look: Matrix4<f32>) -> Matrix4<f32> {
    look.x_axis.x = -look.x_axis.x;
    //look.y_axis.x = -look.y_axis.x; (flippy)
    look.z_axis.x = -look.z_axis.x;
    look.w_axis = Vector4::W;
    /*look.x_axis.z = -look.x_axis.z;
    look.y_axis.z = -look.y_axis.z;
    look.z_axis.z = -look.z_axis.z;*/
    look = look.transpose();
    look.z_axis = -look.z_axis;
    look
}

#[test]
fn billboard_model_view() {
    use glamour::*;
    let signs = [
        Vector3::NEG_ONE,
        Vector3::ONE.with_x(-1.0),
        Vector3::ONE.with_y(-1.0),
        Vector3::ONE.with_z(-1.0),
        Vector3::new(-1.0, 1.0, -1.0),
        Vector3::new(1.0, -1.0, -1.0),
        Vector3::new(-1.0, -1.0, 1.0),
    ];
    for sign in signs {
        let front = (Vector3::<LocalSpace>::new(0.2, 0.3, 0.4) * sign).normalize();
        let pos = Point3::<LocalSpace>::ONE;
        let up = Vector3::<LocalSpace>::Y;
        let look = camera_view(pos, front, up);
        let billy = {
            let cam_right = front.cross(up).normalize();
            // XXX: avoid normalizing cam_up if using glam::look_to_lh...
            let cam_up = cam_right.cross(front).normalize();
            Matrix4::from_cols(
                cam_right.extend(0.0).to_untyped(),
                cam_up.extend(0.0).to_untyped(),
                (-front).extend(0.0).to_untyped(),
                Vector4::W,
            )
        };
        assert_eq!(billy, billboard_from_look(look));
    }
}
