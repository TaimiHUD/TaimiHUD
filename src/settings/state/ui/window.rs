use {
    super::coords::{UiPoint, UiVec2},
    core::ops,
    glam::Vec2,
    serde::{de, ser, Deserialize, Serialize},
    taimi_hoard::{
        is_default,
        vec::{vec32_eq, vec32_ibits},
    },
};

#[derive(Debug, Clone, Default, PartialOrd, Ord, PartialEq, Eq, Deserialize, Serialize)]
pub struct WindowState {
    #[serde(default, skip_serializing_if = "is_default")]
    pub open: WindowOpen,
    #[serde(default, skip_serializing_if = "UiVec2::is_zero")]
    pub size: UiVec2,
    #[serde(default, skip_serializing_if = "UiVec2::is_zero")]
    pub position_rel: UiVec2,
    #[serde(default, skip_serializing_if = "UiPoint::is_zero")]
    pub position_abs: UiPoint,
    #[serde(default, skip_serializing_if = "AnchorPosition::is_default")]
    pub anchor: AnchorPosition,
    /// screen corner
    #[serde(default, skip_serializing_if = "AnchorPosition::is_default")]
    pub anchor_screen: AnchorPosition,
}
impl WindowState {
    pub const MIN_SIZE: UiVec2 = UiVec2::new(48.0, 72.0);

    pub fn is_empty(&self) -> bool {
        match self {
            Self { size, .. } if !size.is_zero() => false,
            Self { position_rel, .. } if !position_rel.is_zero() => false,
            Self { position_abs, .. } if !position_abs.is_zero() => false,
            Self { anchor, .. } if !anchor.is_default() => false,
            Self { anchor_screen, .. } if !anchor_screen.is_default() => false,
            Self {
                open: WindowOpen::DEFAULT,
                anchor: _,
                anchor_screen: _,
                position_rel: _,
                position_abs: _,
                size: _,
            } => true,
            _ => false,
        }
    }
}
#[derive(Debug, Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Deserialize, Serialize)]
pub enum WindowOpen {
    Closed,
    /// "minimized"
    Collapsed,
    Open,
}
impl WindowOpen {
    pub const DEFAULT: Self = Self::Closed;

    #[inline]
    pub const fn new(open: bool) -> Self {
        match open {
            true => Self::Open,
            false => Self::Closed,
        }
    }
    #[inline]
    #[doc(alias = "is_open")]
    pub const fn is_active(&self) -> bool {
        !self.is_closed()
    }
    #[inline]
    pub const fn is_visible(&self) -> bool {
        matches!(self, Self::Open)
    }
    #[inline]
    pub const fn is_closed(&self) -> bool {
        matches!(self, Self::Closed)
    }
    #[inline]
    pub const fn is_collapsed(&self) -> bool {
        matches!(self, Self::Collapsed)
    }

    pub fn serialize<S: ser::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Open | Self::Closed => bool::from(*self).serialize(serializer),
            _ => ser::Serialize::serialize(self, serializer),
        }
    }
    pub fn deserialize<'de, D: de::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum WindowOpen {
            Open(bool),
            WindowOpen(self::WindowOpen),
        }
        impl From<WindowOpen> for self::WindowOpen {
            fn from(open: WindowOpen) -> Self {
                match open {
                    WindowOpen::Open(open) => open.into(),
                    WindowOpen::WindowOpen(open) => open,
                }
            }
        }
        WindowOpen::deserialize(deserializer).map(Into::into)
    }
}
impl From<bool> for WindowOpen {
    #[inline]
    fn from(open: bool) -> Self {
        Self::new(open)
    }
}
impl From<WindowOpen> for bool {
    #[inline]
    fn from(open: WindowOpen) -> Self {
        open.is_active()
    }
}
/// [Self::Closed]
impl Default for WindowOpen {
    #[inline]
    fn default() -> Self {
        Self::DEFAULT
    }
}
#[derive(Debug, Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
#[repr(transparent)]
pub struct AnchorPosition {
    pub anchor: UiVec2,
}
#[allow(non_upper_case_globals)]
impl AnchorPosition {
    pub const TopLeft: Self = Self::with_vec2(Vec2::ZERO);
    pub const TopRight: Self = Self::TopLeft.with_x(1.0);
    pub const BottomLeft: Self = Self::TopLeft.with_y(1.0);
    pub const BottomRight: Self = Self::with_vec2(Vec2::ONE);
    pub const Centre: Self = Self::new(0.5, 0.5);
    pub const TopCentre: Self = Self::TopLeft.with_x(Self::Centre.x());
    pub const BottomCentre: Self = Self::BottomLeft.with_x(Self::Centre.x());
    pub const LeftCentre: Self = Self::TopLeft.with_y(Self::Centre.y());
    pub const RightCenter: Self = Self::TopRight.with_y(Self::Centre.y());
    pub const DEFAULT: Self = Self::TopRight;

    #[inline]
    pub const fn with_vec(anchor: UiVec2) -> Self {
        Self { anchor }
    }
    #[inline]
    pub const fn with_vec2(anchor: Vec2) -> Self {
        Self::with_vec(UiVec2::with_vec2(anchor))
    }
    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Self::with_vec2(Vec2::new(x, y))
    }
    #[inline]
    pub const fn with_x(self, x: f32) -> Self {
        Self::new(x, self.y())
    }
    #[inline]
    pub const fn with_y(self, y: f32) -> Self {
        Self::new(self.x(), y)
    }

    #[inline]
    pub const fn x(&self) -> f32 {
        self.anchor.x()
    }
    #[inline]
    pub const fn y(&self) -> f32 {
        self.anchor.y()
    }

    #[inline]
    pub const fn is_default(&self) -> bool {
        self.anchor.is_zero()
    }
}
impl<T: glamour::Unit> ops::Mul<glamour::Vector2<T>> for AnchorPosition
where
    glamour::Vector2<T>: ops::Mul<Vec2>,
{
    type Output = <glamour::Vector2<T> as ops::Mul<Vec2>>::Output;
    fn mul(self, rhs: glamour::Vector2<T>) -> Self::Output {
        rhs * self.anchor.vec2
    }
}
impl<T: glamour::Unit> ops::Mul<glamour::Size2<T>> for AnchorPosition
where
    glamour::Vector2<T>: ops::Mul<Vec2>,
{
    type Output = <glamour::Vector2<T> as ops::Mul<Vec2>>::Output;
    fn mul(self, rhs: glamour::Size2<T>) -> Self::Output {
        self * rhs.to_vector()
    }
}
impl<T: glamour::Unit> ops::Mul<glamour::Box2<T>> for AnchorPosition
where
    glamour::Vector2<T>: ops::Mul<Vec2>,
    glamour::Point2<T>: ops::Add<<glamour::Vector2<T> as ops::Mul<Vec2>>::Output>,
{
    type Output = <glamour::Point2<T> as ops::Add<<glamour::Vector2<T> as ops::Mul<Vec2>>::Output>>::Output;
    fn mul(self, rhs: glamour::Box2<T>) -> Self::Output {
        self * rhs.to_rect()
    }
}
impl<T: glamour::Unit> ops::Mul<glamour::Rect<T>> for AnchorPosition
where
    glamour::Vector2<T>: ops::Mul<Vec2>,
    glamour::Point2<T>: ops::Add<<glamour::Vector2<T> as ops::Mul<Vec2>>::Output>,
{
    type Output = <glamour::Point2<T> as ops::Add<<glamour::Vector2<T> as ops::Mul<Vec2>>::Output>>::Output;
    fn mul(self, rhs: glamour::Rect<T>) -> Self::Output {
        rhs.origin + self * rhs.size
    }
}
impl From<AnchorPosition> for UiVec2 {
    #[inline]
    fn from(pos: AnchorPosition) -> Self {
        pos.anchor
    }
}
impl From<AnchorPosition> for Vec2 {
    #[inline]
    fn from(pos: AnchorPosition) -> Self {
        pos.anchor.into()
    }
}
impl<T: glamour::Unit<Scalar = f32>> From<AnchorPosition> for glamour::Vector2<T> {
    #[inline]
    fn from(pos: AnchorPosition) -> Self {
        pos.anchor.into()
    }
}
impl From<AnchorPosition> for [f32; 2] {
    #[inline]
    fn from(pos: AnchorPosition) -> Self {
        pos.anchor.into()
    }
}
impl Default for AnchorPosition {
    #[inline]
    fn default() -> Self {
        Self::DEFAULT
    }
}
#[cfg(todo)]
bitflags! {
    #[derive(Debug, Copy, Clone, Default, PartialOrd, Ord, PartialEq, Eq)]
    pub struct PositionAnchor: u8 {
        const BOTTOM = 0x01;
        const RIGHT = 0x02;
        const CENTRE_X = 0x04;
        const CENTRE_Y = 0x08;
    }
}
#[cfg(todo)]
#[allow(non_upper_case_globals)]
impl PositionAnchor {
    pub const TopLeft: Self = Self::empty();
    pub const TopRight: Self = Self::RIGHT;
    pub const BottomLeft: Self = Self::BOTTOM;
    pub const BottomRight: Self = Self::from_bits_retain(Self::BOTTOM.bits() | Self::RIGHT.bits());
}
