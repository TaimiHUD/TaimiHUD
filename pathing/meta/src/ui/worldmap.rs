use {
    crate::{
        coords::{
            coord_newtype,
            FakeSpace,
            LocalSpace,
            MapLocalScale,
            MapSpace,
            ScreenSpace,
            WorldmapSpace,
        },
        map::{Map, MapID},
        ui::{MapContext, MinimapPlacement, MinimapState, UiSize, UiState},
    },
    glamour::{
        Angle,
        Contains,
        FloatUnit,
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
    },
    num_traits::{ConstOne, NumCast},
    std::{fmt, time::Duration},
};

#[cfg(feature = "taimi_mumblelink")]
use crate::ui::mumblelink::gw2_mumble::{Context, Identity};

/// [MumbleLink context](https://wiki.guildwars2.com/wiki/API:MumbleLink#context)
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(
    feature = "mumblelink-arcloader",
    doc = "\nSee also: [gw2_mumble::Context](arcloader_mumblelink::gw2_mumble::Context)"
)]
pub struct MapCalibration {
    pub compass_position: MinimapPlacement,
    pub compass_size: Size2<FakeSpace>,
    /// [MumbleLink identity](https://wiki.guildwars2.com/wiki/API:MumbleLink#identity) `uisz`
    pub ui_size: UiSize,
    /// `mapScale`
    pub display_size: Size2<ScreenSpace>,
    pub dpi: f32,
    pub local_space: Option<MapLocalScale>,
    pub local_offset: Option<Point3<MapSpace>>,
}

impl MapCalibration {
    pub const DEFAULT: Self = Self {
        compass_position: MinimapPlacement::DEFAULT,
        // TODO: default size is what?
        compass_size: Size2::ZERO,
        ui_size: UiSize::Normal,
        display_size: Size2::new(1920.0, 1080.0),
        dpi: Self::DPI_REFERENCE,
        local_space: None,
        local_offset: None,
    };

    pub fn is_empty(&self) -> bool {
        self.compass_size.is_empty() || self.display_size.is_empty() || self.local_offset.is_none()
    }

    pub fn get(&self) -> Option<&Self> {
        match self.is_empty() {
            false => Some(self),
            true => None,
        }
    }

    pub const DISPLAY_SCALING_MAX: Size2<ScreenSpace> = Size2::new(1024.0, 768.0);

    /// Normalized scaling factor
    pub fn display_scale(&self) -> Vector2 {
        (self.display_size / Self::DISPLAY_SCALING_MAX)
            .to_vector()
            .min(Vector2::ONE)
            .to_untyped()
    }

    pub const DPI_REFERENCE: f32 = 96.0;

    #[inline]
    pub fn dpi_scale(&self) -> f32 {
        self.dpi / Self::DPI_REFERENCE
    }

    #[inline]
    pub fn display_size(&self) -> Size2<FakeSpace> {
        self.map(self.display_size)
    }

    pub fn fake_scaling(&self) -> <ScreenSpace as Unit>::Scalar {
        self.ui_size.normal_scale() * self.display_scale().min_element() * self.dpi_scale()
    }

    pub fn local_space(&self) -> MapLocalScale {
        self.local_space.unwrap_or_default()
    }

    #[inline]
    pub fn update_from_mumblelink_state(&mut self, ui_state: UiState) {
        self.compass_position = ui_state.into();
    }

    pub fn update_from_map(&mut self, map: &Map) {
        if self.local_space.is_none() {
            self.local_space = Some(map.map_scale());
        }
        if self.local_offset.is_none() {
            let offset = map.continent_map_origin();
            self.local_offset = Some(Point2::new(offset.x, offset.y).extend(0.0));
        }
    }

    pub fn bounds_for(&self, ctx: MapContext) -> Rect<FakeSpace> {
        match ctx {
            MapContext::Global => Rect::from_size(self.display_size()),
            MapContext::Minimap => self.compass_bounds(),
        }
    }

    /// Area within map bounds that is (likely) safe to interact with
    /// to begin a drag or scroll motion
    pub fn interaction_bounds_for(&self, ctx: MapContext) -> Rect<FakeSpace> {
        let mut bounds = self.bounds_for(ctx);
        match ctx {
            MapContext::Global => {
                // TODO: be more intentional about how much of screen UI elements take up...
                let dead_zone_ratio = bounds.size * 0.2;
                bounds.size -= dead_zone_ratio;
                bounds.origin += dead_zone_ratio.to_vector() / 2.0;
            },
            MapContext::Minimap => {
                bounds.size -= MinimapPlacement::DEAD_ZONE_SIZE;
                bounds.origin += MinimapPlacement::INSET_DEAD_ZONE.to_vector();
            },
        }
        bounds
    }

    #[inline]
    pub fn clip_for(&self, ctx: MapContext, point: Point2<FakeSpace>) -> Option<Point2<FakeSpace>> {
        self.bounds_for(ctx).contains(&point).then_some(point)
    }

    pub fn clip_screen_for(
        &self,
        ctx: MapContext,
        point: Point2<ScreenSpace>,
    ) -> Option<Point2<ScreenSpace>> {
        self.clip_for(ctx, self.map(point)).map(|_| point)
    }

    pub fn set_offset(&mut self, local: Point3<LocalSpace>, global: Point2<MapSpace>) {
        self.local_offset = {
            let origin = self.local_space().map(LocalSpace::to2(local).to_vector());
            let global = global - origin;
            Some(Point3::new(global.x, global.y, local.y / self.z_scale()))
        };
    }

    pub fn clear_map(&mut self) {
        self.local_space = None;
        self.local_offset = None;
    }
}

#[cfg(feature = "taimi_mumblelink")]
impl MapCalibration {
    /// Update context data
    ///
    /// Excludes [identity data](self.update_from_mumblelink_identity)
    pub unsafe fn update_from_mumblelink_context_ptr(&mut self, context: *const Context) {
        use core::ptr::read_volatile;

        self.compass_size.height = read_volatile(&raw const (*context).compass_height).into();
        self.compass_size.width = read_volatile(&raw const (*context).compass_width).into();

        let ui_state = UiState::from(read_volatile(&raw const (*context).ui_state));
        self.update_from_mumblelink_state(ui_state);
        // TODO: try to get map translation from map id rect lookup!
        #[cfg(todo)]
        {
            use super::mumblelink::MumbleLink;
            let player_map = Point2::<MapSpace>::new(
                read_volatile(&raw const (*context).player_x),
                read_volatile(&raw const (*context).player_y),
            );
            let link = context.byte_sub(core::mem::offset_of!(MumbleLink, context)) as *const MumbleLink;
            let player_local = Point3::<LocalSpace>::new(
                read_volatile(&raw const (*link).avatar.position[0]),
                read_volatile(&raw const (*link).avatar.position[1]),
                read_volatile(&raw const (*link).avatar.position[2]),
            );
            self.set_offset(player_local, player_map);
        };
    }

    pub fn update_from_mumblelink_identity_data(&mut self, ui_size: UiSize, _map_id: MapID) -> bool {
        let changed = self.ui_size != ui_size;
        self.ui_size = ui_size;

        changed
    }

    pub fn update_from_mumblelink_identity(&mut self, identity: &Identity) -> bool {
        self.update_from_mumblelink_identity_data(identity.ui_scale.into(), identity.map_id.into())
    }

    #[cfg(feature = "nexus")]
    pub fn update_from_mumblelink_identity_nexus(
        &mut self,
        identity: &crate::ui::mumblelink::NexusIdentity,
    ) -> bool {
        let ui_size = UiSize::try_from(identity.ui_size).unwrap_or(UiSize::Normal);
        self.update_from_mumblelink_identity_data(ui_size, identity.map_id.into())
    }
}

impl Default for MapCalibration {
    fn default() -> Self {
        Self::DEFAULT
    }
}

pub trait MapUnit: FloatUnit {
    const CONTEXT: MapContext;

    type Rotation: Sized + Copy + Clone + Default + PartialEq + fmt::Debug;
    const ROTATION_DISABLED: Self::Rotation;
    const ROTATION: bool;
    fn get_rotation(angle: Self::Rotation) -> Option<Angle<<Self as Unit>::Scalar>>;
    fn rotation_from(angle: Option<Angle<<Self as Unit>::Scalar>>) -> Self::Rotation;
}

impl MapUnit for WorldmapSpace {
    const CONTEXT: MapContext = MapContext::Global;

    type Rotation = ();
    const ROTATION_DISABLED: Self::Rotation = ();
    const ROTATION: bool = false;
    #[inline]
    fn get_rotation(_: ()) -> Option<Angle<<Self as Unit>::Scalar>> {
        None
    }
    #[inline]
    fn rotation_from(_: Option<Angle<<Self as Unit>::Scalar>>) -> Self::Rotation {}
}

/// [MumbleLink context](https://wiki.guildwars2.com/wiki/API:MumbleLink#context)
#[cfg_attr(
    feature = "mumblelink-arcloader",
    doc = "\nSee also: [gw2_mumble::Context](arcloader_mumblelink::gw2_mumble::Context)"
)]
pub struct MapState<M: MapUnit = WorldmapSpace> {
    pub centre: Point2<MapSpace>,
    pub rotation: M::Rotation,
    pub scale: <M as Unit>::Scalar,
}

impl<M: MapUnit> MapState<M> {
    pub const DEFAULT: Self = Self {
        centre: Point2::ZERO,
        rotation: <M as MapUnit>::ROTATION_DISABLED,
        scale: <<M as Unit>::Scalar as ConstOne>::ONE,
    };

    #[inline]
    pub fn rotation(&self) -> Option<Angle<M::Scalar>> {
        M::get_rotation(self.rotation)
    }

    #[inline]
    pub fn counter_rotation(&self) -> Option<Angle<M::Scalar>> {
        self.rotation().map(|r| Angle::new(-r.radians))
    }

    #[inline]
    pub fn rotation_angle(&self) -> Angle<f32> {
        self.rotation()
            .and_then(|a| NumCast::from(a.radians).map(Angle::new))
            .unwrap_or_default()
    }

    #[inline]
    pub fn scale(&self) -> f32 {
        NumCast::from(self.scale).unwrap_or_default()
    }

    #[inline]
    pub fn set_rotation(&mut self, rotation: Option<Angle<M::Scalar>>) {
        self.rotation = M::rotation_from(rotation);
    }

    #[cfg(feature = "taimi_mumblelink")]
    pub unsafe fn update_from_mumblelink_context_ptr(&mut self, context: *const Context) {
        use core::ptr::read_volatile;

        #[cfg(todo = "unnecessary")]
        if MapContext::from(context) != M::CONTEXT {
            return
        }

        self.centre.x = read_volatile(&raw const (*context).map_center_x);
        self.centre.y = read_volatile(&raw const (*context).map_center_y);
        self.scale = NumCast::from(read_volatile(&raw const (*context).map_scale)).unwrap_or_default();

        if M::CONTEXT == MapContext::Minimap {
            let ui_state = UiState::from(read_volatile(&raw const (*context).ui_state));
            self.set_rotation(match ui_state.contains(UiState::CompassRotation) {
                true => NumCast::from(read_volatile(&raw const (*context).compass_rotation))
                    .map(Angle::from_radians),
                false => None,
            });
        }
    }
}

impl<M: MapUnit> Clone for MapState<M> {
    fn clone(&self) -> Self {
        Self {
            centre: self.centre.clone(),
            rotation: self.rotation.clone(),
            scale: self.scale.clone(),
        }
    }
}

impl<M: MapUnit> PartialEq for MapState<M> {
    fn eq(&self, rhs: &Self) -> bool {
        crate::coords::vec_eq(self.centre, rhs.centre)
            && self.rotation_angle().radians.to_bits() == rhs.rotation_angle().radians.to_bits()
            && self.scale().to_bits() == rhs.scale().to_bits()
    }
}

impl<M: MapUnit> Default for MapState<M> {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl<M: MapUnit> fmt::Debug for MapState<M> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("MapState")
            .field("centre", &self.centre)
            .field("rotation", &self.rotation)
            .field("scale", &self.scale)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(
    feature = "mumblelink-arcloader",
    doc = "\nSee also: [gw2_mumble::Context](arcloader_mumblelink::gw2_mumble::Context)"
)]
pub struct UiMap {
    pub calibration: MapCalibration,
    pub map: MapState,
    pub compass: MinimapState,
    pub player_pos: Point2<MapSpace>,
    pub context: MapContext,
}

impl UiMap {
    pub const DEFAULT: Self = Self {
        calibration: MapCalibration::DEFAULT,
        map: MapState::DEFAULT,
        compass: MinimapState::DEFAULT,
        player_pos: Point2::ZERO,
        context: MapContext::DEFAULT,
    };

    pub fn is_empty(&self) -> bool {
        self.calibration.is_empty()
    }

    pub fn get(&self) -> Option<&Self> {
        match self.is_empty() {
            false => Some(self),
            true => None,
        }
    }

    pub fn scale(&self) -> f32 {
        self.scale_for(self.context)
    }

    pub fn scale_for(&self, ctx: MapContext) -> f32 {
        match ctx {
            MapContext::Minimap => self.compass.scale,
            MapContext::Global => self.map.scale,
        }
    }

    pub fn scale_mut(&mut self) -> &mut f32 {
        self.scale_for_mut(self.context)
    }

    pub fn scale_for_mut(&mut self, ctx: MapContext) -> &mut f32 {
        match ctx {
            MapContext::Minimap => &mut self.compass.scale,
            MapContext::Global => &mut self.map.scale,
        }
    }

    pub fn centre(&self) -> Point2<MapSpace> {
        self.centre_for(self.context)
    }
    pub fn centre_for(&self, ctx: MapContext) -> Point2<MapSpace> {
        match ctx {
            MapContext::Minimap => self.compass.centre,
            MapContext::Global => self.map.centre,
        }
    }

    pub fn rotation(&self) -> Option<Angle> {
        self.rotation_for(self.context)
    }
    pub fn rotation_for(&self, ctx: MapContext) -> Option<Angle> {
        match ctx {
            MapContext::Minimap => self.compass.rotation(),
            MapContext::Global => self.map.rotation(),
        }
    }

    pub fn bounds(&self) -> Rect<FakeSpace> {
        self.calibration.bounds_for(self.context)
    }
    pub fn interaction_bounds(&self) -> Rect<FakeSpace> {
        self.calibration.interaction_bounds_for(self.context)
    }

    pub fn clip(&self, point: Point2<FakeSpace>) -> Option<Point2<FakeSpace>> {
        self.calibration.clip_for(self.context, point)
    }
    pub fn clip_screen(&self, point: Point2<ScreenSpace>) -> Option<Point2<ScreenSpace>> {
        self.calibration.clip_screen_for(self.context, point)
    }

    /// Update context data
    ///
    /// Excludes [identity data](self.update_from_mumblelink_identity)
    #[cfg(feature = "taimi_mumblelink")]
    pub unsafe fn update_from_mumblelink_context_ptr(&mut self, context: *const Context) {
        use core::ptr::read_volatile;

        let ui_state = UiState::from(read_volatile(&raw const (*context).ui_state));
        self.context = ui_state.into();

        self.calibration.update_from_mumblelink_context_ptr(context);

        match self.context {
            MapContext::Global => self.map.update_from_mumblelink_context_ptr(context),
            MapContext::Minimap => self.compass.update_from_mumblelink_context_ptr(context),
        }
        self.player_pos.x = read_volatile(&raw const (*context).player_x);
        self.player_pos.y = read_volatile(&raw const (*context).player_y);
    }

    #[cfg(feature = "mumblelink-arcloader")]
    pub fn update_from_mumblelink(&mut self, mumblelink: arcloader_mumblelink::gw2_mumble::MumblePtr) {
        if mumblelink.read_ui_version() == 0 {
            return
        }

        unsafe {
            let context = &raw const (*mumblelink.as_ptr()).context;
            self.update_from_mumblelink_context_ptr(context)
        }
    }

    #[cfg(all(feature = "nexus", feature = "mumblelink-nexus"))]
    pub fn update_from_mumblelink_nexus(&mut self, mumblelink: crate::ui::mumblelink::nexus::MumblePtr) {
        if mumblelink.read_ui_version() == 0 {
            return
        }

        unsafe {
            let context = &raw const (*mumblelink.as_ptr()).context;
            self.update_from_mumblelink_context_ptr(context as *const Context)
        }
    }

    pub fn update_from_mumblelink_state(&mut self, ui_state: UiState) {
        self.calibration.update_from_mumblelink_state(ui_state);
        self.context = ui_state.into();
    }
}

impl Default for UiMap {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug, Copy, Clone, Default, PartialEq, PartialOrd)]
pub enum MapOpen {
    #[default]
    Closed,
    Opening {
        /// Seconds
        elapsed: f32,
    },
    Open,
    Closing {
        /// Seconds
        elapsed: f32,
    },
}

impl MapOpen {
    pub const DEFAULT: Self = Self::Closed;
    pub const DURATION: Duration = Duration::from_millis(480);
    pub const MAX_DURATION: Duration = Duration::from_millis(720);

    pub const fn with_open(open: bool) -> Self {
        match open {
            true => Self::Open,
            false => Self::Closed,
        }
    }

    pub const fn with_open_anim(open: bool, elapsed: f32) -> Self {
        match open {
            true => Self::Opening { elapsed },
            false => Self::Closing { elapsed },
        }
    }

    /// [MumbleLink flag](UiState::MAP_OPEN) edge event.
    ///
    /// Though it is set as soon as the open animation begins,
    /// it will not clear until closing animation is complete.
    pub const fn with_state_event(state: UiState) -> Self {
        match state.bits() & UiState::MapOpen.bits() {
            0 => Self::Closed,
            _ => Self::Opening { elapsed: 0.0 },
        }
    }

    /// Including [Self::Closing]
    pub const fn is_visible(&self) -> bool {
        match self {
            Self::Closed => false,
            _ => true,
        }
    }

    pub const fn is_open(&self) -> bool {
        match self {
            Self::Closed | Self::Closing { .. } => false,
            _ => true,
        }
    }

    pub const fn is_anim(&self) -> bool {
        match self {
            Self::Opening { .. } | Self::Closing { .. } => true,
            _ => false,
        }
    }

    pub const fn shape(&self) -> (bool, bool) {
        (self.is_open(), self.is_anim())
    }

    pub const fn max_duration(relaxed: bool) -> Duration {
        match relaxed {
            true => Self::MAX_DURATION,
            false => Self::DURATION,
        }
    }

    pub const fn while_elapsed(self, elapsed: f32) -> Self {
        match self.is_open() {
            true => Self::Opening { elapsed },
            false => Self::Closing { elapsed },
        }
    }

    pub const fn cap(self, relaxed: bool) -> Self {
        let cap = Self::max_duration(relaxed);
        match self {
            Self::Opening { elapsed } | Self::Closing { elapsed } if elapsed >= cap.as_secs_f32() =>
                Self::with_open(self.is_open()),
            open => open,
        }
    }

    pub const fn visible_context(&self) -> MapContext {
        match self.is_visible() {
            true => MapContext::Global,
            false => MapContext::Minimap,
        }
    }

    /// Unlike [self.is_visible()],
    /// prefer minimap when closing
    pub const fn primary_context(&self) -> MapContext {
        match self {
            MapOpen::Closed | MapOpen::Closing { .. } => MapContext::Minimap,
            _ => MapContext::Global,
        }
    }

    pub fn elapsed(self) -> Option<Duration> {
        self.elapsed_s().map(Duration::from_secs_f32)
    }

    pub fn elapsed_mut(&mut self) -> Option<&mut f32> {
        match self {
            Self::Opening { elapsed } | Self::Closing { elapsed } => Some(elapsed),
            Self::Open | Self::Closed => None,
        }
    }

    pub const fn elapsed_s(self) -> Option<f32> {
        match self {
            Self::Opening { elapsed } | Self::Closing { elapsed } => Some(elapsed),
            Self::Open | Self::Closed => None,
        }
    }

    /// Animation completion out of 1.0
    pub fn progress(&self) -> Option<f32> {
        match self {
            Self::Opening { elapsed } => Some(elapsed / Self::DURATION.as_secs_f32()),
            Self::Closing { elapsed } => Some(elapsed / Self::MAX_DURATION.as_secs_f32()),
            Self::Open | Self::Closed => None,
        }
    }

    pub fn progress_open(&self) -> Option<f32> {
        match (self, self.progress()) {
            (Self::Closing { .. }, Some(p)) => Some((1.0 - p).max(0.0)),
            (_, p) => p,
        }
    }

    pub fn scalar(&self) -> Option<f32> {
        let progress = self.progress()?.min(1.0);
        const INV: f32 = 1.73; //3.0f32.sqrt();
        Some(match self {
            Self::Opening { .. } => progress.powf(INV),
            _ => progress.powi(3),
        })
    }
}

impl From<bool> for MapOpen {
    fn from(open: bool) -> Self {
        Self::with_open(open)
    }
}
impl From<UiState> for MapOpen {
    fn from(state: UiState) -> Self {
        Self::with_open(state.contains(UiState::MapOpen))
    }
}
impl From<MapContext> for MapOpen {
    fn from(cx: MapContext) -> Self {
        Self::with_open(cx == MapContext::Global)
    }
}
impl From<MapOpen> for MapContext {
    fn from(open: MapOpen) -> Self {
        open.primary_context()
    }
}
impl From<MapOpen> for bool {
    fn from(open: MapOpen) -> Self {
        open.is_visible()
    }
}

impl MapCalibration {
    #[inline]
    pub fn to_screen(&self) -> Transform2<FakeSpace, ScreenSpace> {
        Transform2::from_scale(Vector2::splat(self.fake_scaling()))
    }

    #[inline]
    pub fn to_fake(&self) -> Transform2<ScreenSpace, FakeSpace> {
        Transform2::from_scale(Vector2::splat(1.0 / self.fake_scaling()))
    }
    #[inline]
    pub fn to_fake3(&self) -> Transform3<ScreenSpace, FakeSpace> {
        Transform3::from_scale(Vector2::splat(1.0 / self.fake_scaling()).extend(1.0))
    }

    pub fn map_to_local(&self) -> Transform2<MapSpace, LocalSpace> {
        Transform2::from_translation(-self.local_offset.unwrap_or_default().xy().to_vector())
            .then_scale(self.local_space().scale)
    }
    pub fn local_to_map(&self) -> Transform2<LocalSpace, MapSpace> {
        Transform2::from_scale(Vector2::splat(1.0) / self.local_space().scale)
            .then_translate(self.local_offset.unwrap_or_default().xy().to_vector())
    }

    pub fn z_scale(&self) -> f32 {
        match () {
            // just convert units from ft, reusing continent rect feels weird
            #[cfg(todo)]
            _ => self.local_space().scale.x,
            _ => MapLocalScale::COMMON.scale.x,
        }
    }
}

coord_newtype! {
    impl TransformMap<FakeSpace, Output = Vec2<ScreenSpace>> for MapCalibration {
        #[inline]
        fn map(&self, v) {
            (v.to_untyped() * self.fake_scaling()).as_()
        }
    }
    impl TransformMap<ScreenSpace, Output = Vec2<FakeSpace>> for MapCalibration {
        #[inline]
        fn map(&self, v) {
            (v.to_untyped() / self.fake_scaling()).as_()
        }
    }

    impl TransformMap<FakeSpace, Output = Vec3<ScreenSpace>> for MapCalibration {
        #[inline(always)]
        fn map(&self, v) {
            self.map(v.truncate()).extend(v.z)
        }
    }
    impl TransformMap<ScreenSpace, Output = Vec3<FakeSpace>> for MapCalibration {
        #[inline(always)]
        fn map(&self, v) {
            self.map(v.truncate()).extend(v.z)
        }
    }
    impl TransformMap<ScreenSpace, Output = Vec4<FakeSpace>> for MapCalibration {
        #[inline(always)]
        fn map(&self, v) {
            self.map(v.truncate()).extend(v.w)
        }
    }
    impl TransformMap<FakeSpace, Output = Vec4<ScreenSpace>> for MapCalibration {
        #[inline(always)]
        fn map(&self, v) {
            self.map(v.truncate()).extend(v.w)
        }
    }

    impl TransformMap<LocalSpace, Output = Vector2<MapSpace>> for MapCalibration {
        #[inline]
        fn map(&self, v) {
            self.local_space().map(v)
        }
    }
    impl TransformMap<LocalSpace, Output = Point2<MapSpace>> for MapCalibration {
        #[inline]
        fn map(&self, v) {
            self.local_to_map().map(v)
        }
    }
    impl TransformMap<MapSpace, Output = Vector2<LocalSpace>> for MapCalibration {
        #[inline]
        fn map(&self, v) {
            self.local_space().map(v)
        }
    }
    impl TransformMap<MapSpace, Output = Point2<LocalSpace>> for MapCalibration {
        #[inline]
        fn map(&self, v) {
            self.map_to_local().map(v)
        }
    }
    impl TransformMap<LocalSpace, Output = Vec3<MapSpace>> for MapCalibration {
        #[inline]
        fn map(&self, v) {
            self.map(v.xz()).extend(v.y / self.z_scale())
        }
    }
    impl TransformMap<MapSpace, Output = Vec3<LocalSpace>> for MapCalibration {
        #[inline]
        fn map(&self, v) {
            self.map(v.xy()).extend(v.z * self.z_scale()).xzy()
        }
    }
}

impl<M: MapUnit> MapState<M>
where
    MapSpace: Unit<Scalar = <M as Unit>::Scalar>,
{
    pub fn from_map(&self) -> Transform2<MapSpace, M> {
        let trans = Transform2::from_translation(self.centre.to_vector().map(|s| -s));
        trans.then_scale(Vector2::splat(
            <<M as Unit>::Scalar as ConstOne>::ONE / self.scale,
        ))
    }

    pub fn to_map(&self) -> Transform2<M, MapSpace> {
        //self.from_map().inverse()
        Transform2::from_scale(Vector2::splat(self.scale)).then_translate(self.centre.to_vector())
    }
}

impl<M: MapUnit> MapState<M>
where
    WorldmapSpace: Unit<Scalar = <M as Unit>::Scalar>,
{
    pub fn to_worldmap(&self) -> Transform2<M, WorldmapSpace> {
        Transform2::IDENTITY
    }

    pub fn from_worldmap(&self) -> Transform2<WorldmapSpace, M> {
        Transform2::IDENTITY
    }
}

coord_newtype! {
    /*impl TransformMap<WorldmapSpace, Output = Vector2<MapSpace>> for MapState<WorldmapSpace> {
        fn map(&self, v) {
            self.to_map().map(v)
        }
    }*/
    impl TransformMap<MapSpace, Output = Vector2<WorldmapSpace>> for MapState<WorldmapSpace> {
        fn map(&self, v) {
            v.as_() / Vector2::splat(self.scale)
        }
    }
    impl TransformMap<MapSpace, Output = Point2<WorldmapSpace>> for MapState<WorldmapSpace> {
        fn map(&self, v) {
            self.from_map().map(v)
        }
    }
}

impl MapCalibration {
    pub fn fake_to_worldmap(&self) -> Transform2<FakeSpace, WorldmapSpace> {
        let bounds = self.display_size();
        let mid = bounds / 2.0;
        Transform2::from_translation(-mid.to_vector())
    }
    pub fn worldmap_to_fake(&self) -> Transform2<WorldmapSpace, FakeSpace> {
        let bounds = self.display_size();
        let mid = bounds / 2.0;
        Transform2::from_translation(mid.to_vector().as_())
    }
}

impl UiMap {
    pub fn fake_to_worldmap_for(&self, ctx: MapContext) -> Transform2<FakeSpace, WorldmapSpace> {
        match ctx {
            MapContext::Minimap => MapCalibration::cast_minimap_to_worldmap(
                self.calibration
                    .fake_to_compass()
                    .then(self.compass.from_compass()),
            ),
            MapContext::Global => self.calibration.fake_to_worldmap(), // no need for then(self.map.to_worldmap())
        }
    }
    pub fn worldmap_to_fake_for(&self, ctx: MapContext) -> Transform2<WorldmapSpace, FakeSpace> {
        match ctx {
            MapContext::Minimap => MapCalibration::cast_worldmap_to_minimap(
                self.compass.to_compass().then(self.calibration.compass_to_fake()),
            ),
            MapContext::Global => self.calibration.worldmap_to_fake(), // no need for from_worldmap
        }
    }

    pub fn map_to_worldmap_for(&self, ctx: MapContext) -> Transform2<MapSpace, WorldmapSpace> {
        match ctx {
            MapContext::Minimap => self.compass.from_map().then(self.compass.to_worldmap()),
            MapContext::Global => self.map.from_map(), // no need for then(self.map.to_worldmap())
        }
    }
    pub fn worldmap_to_map_for(&self, ctx: MapContext) -> Transform2<WorldmapSpace, MapSpace> {
        match ctx {
            MapContext::Minimap => self.compass.from_worldmap().then(self.compass.to_map()),
            MapContext::Global => self.map.to_map(), // no need for from_worldmap
        }
    }
}

// TODO: coord_newtype! { impl TransformMap<X, Output = Vector2<Y>> for UiMap }
