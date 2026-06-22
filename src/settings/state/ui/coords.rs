use {
    core::{cmp, ops},
    glam::Vec2,
    serde::{Deserialize, Serialize},
    taimi_hoard::vec::{vec32_eq, vec32_ibits},
};

pub struct UiSpace;
impl glamour::Unit for UiSpace {
    type Scalar = f32;
}

#[cfg(todo)]
pub type UiPoint = glamour::Point2<UiSpace>;
pub type UiPoint = UiVec2;

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[serde(from = "[f32; 2]", into = "[f32; 2]")]
#[repr(transparent)]
pub struct UiVec2 {
    pub vec2: Vec2,
}
#[allow(non_upper_case_globals)]
impl UiVec2 {
    pub const ZERO: Self = Self::with_vec2(Vec2::ZERO);
    pub const ONE: Self = Self::with_vec2(Vec2::ONE);
    pub const DEFAULT: Self = Self::ZERO;

    #[inline]
    pub const fn with_vec2(vec2: Vec2) -> Self {
        Self { vec2 }
    }
    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Self::with_vec2(Vec2::new(x, y))
    }
    #[inline]
    pub const fn with_x(self, x: f32) -> Self {
        Self::new(x, self.vec2.y)
    }
    #[inline]
    pub const fn with_y(self, y: f32) -> Self {
        Self::new(self.vec2.x, y)
    }

    #[inline]
    pub const fn is_zero(&self) -> bool {
        matches!(self.vec2, Vec2::ZERO)
    }
    #[inline]
    pub const fn get(&self) -> Option<&Self> {
        match self.is_zero() {
            true => None,
            false => Some(self),
        }
    }

    #[inline]
    pub const fn x(&self) -> f32 {
        self.vec2.x
    }
    #[inline]
    pub const fn y(&self) -> f32 {
        self.vec2.y
    }

    #[inline]
    pub const fn to_vector<U: glamour::Unit<Scalar = f32>>(self) -> glamour::Vector2<U> {
        glamour::Vector2::new(self.vec2.x, self.vec2.y)
    }
    #[inline]
    pub const fn to_size<U: glamour::Unit<Scalar = f32>>(self) -> glamour::Size2<U> {
        glamour::Size2::new(self.vec2.x, self.vec2.y)
    }
    #[inline]
    pub const fn to_point<U: glamour::Unit<Scalar = f32>>(self) -> glamour::Point2<U> {
        glamour::Point2::new(self.vec2.x, self.vec2.y)
    }
}
impl ops::MulAssign<Self> for UiVec2 {
    #[inline]
    fn mul_assign(&mut self, rhs: Self) {
        *self *= rhs.vec2;
    }
}
impl ops::MulAssign<Vec2> for UiVec2 {
    fn mul_assign(&mut self, rhs: Vec2) {
        self.vec2 *= rhs;
    }
}
impl ops::MulAssign<f32> for UiVec2 {
    fn mul_assign(&mut self, rhs: f32) {
        self.vec2 *= rhs;
    }
}
impl ops::Mul<Self> for UiVec2 {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self::Output {
        self * rhs.vec2
    }
}
impl ops::Mul<Vec2> for UiVec2 {
    type Output = Self;
    fn mul(self, rhs: Vec2) -> Self::Output {
        Self::with_vec2(self.vec2 * rhs)
    }
}
impl<T: glamour::Unit> ops::Mul<glamour::Vector2<T>> for UiVec2
where
    glamour::Vector2<T>: ops::Mul<Vec2>,
{
    type Output = <glamour::Vector2<T> as ops::Mul<Vec2>>::Output;
    fn mul(self, rhs: glamour::Vector2<T>) -> Self::Output {
        rhs * self.vec2
    }
}
impl ops::Mul<[f32; 2]> for UiVec2 {
    type Output = Self;
    fn mul(self, rhs: [f32; 2]) -> Self::Output {
        self * Self::from(rhs)
    }
}
impl ops::Mul<f32> for UiVec2 {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self::Output {
        Self::with_vec2(self.vec2 * rhs)
    }
}
impl ops::Div<Self> for UiVec2 {
    type Output = Self;
    fn div(self, rhs: Self) -> Self::Output {
        self / rhs.vec2
    }
}
impl ops::Div<Vec2> for UiVec2 {
    type Output = Self;
    fn div(self, rhs: Vec2) -> Self::Output {
        Self::with_vec2(self.vec2 / rhs)
    }
}
impl ops::Div<[f32; 2]> for UiVec2 {
    type Output = Self;
    fn div(self, rhs: [f32; 2]) -> Self::Output {
        self / Self::from(rhs)
    }
}
impl ops::Div<f32> for UiVec2 {
    type Output = Self;
    fn div(self, rhs: f32) -> Self::Output {
        Self::with_vec2(self.vec2 / rhs)
    }
}
impl PartialOrd for UiVec2 {
    #[inline]
    fn partial_cmp(&self, rhs: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(rhs))
    }
}
impl Ord for UiVec2 {
    fn cmp(&self, rhs: &Self) -> cmp::Ordering {
        vec32_ibits(self.vec2).cmp(&vec32_ibits(rhs.vec2))
    }
}
impl PartialEq for UiVec2 {
    fn eq(&self, rhs: &Self) -> bool {
        vec32_eq(self.vec2, rhs.vec2)
    }
}
impl Eq for UiVec2 {}
impl From<UiVec2> for Vec2 {
    #[inline]
    fn from(pos: UiVec2) -> Self {
        pos.vec2
    }
}
impl From<&'_ UiVec2> for Vec2 {
    #[inline]
    fn from(pos: &UiVec2) -> Self {
        pos.vec2
    }
}
impl<T: glamour::Unit<Scalar = f32>> From<UiVec2> for glamour::Vector2<T> {
    #[inline]
    fn from(pos: UiVec2) -> Self {
        glamour::Vector2::from_raw(pos.vec2)
    }
}
impl<T: glamour::Unit<Scalar = f32>> From<UiVec2> for glamour::Size2<T> {
    #[inline]
    fn from(pos: UiVec2) -> Self {
        glamour::Size2::from_raw(pos.vec2)
    }
}
impl<T: glamour::Unit<Scalar = f32>> From<UiVec2> for glamour::Point2<T> {
    #[inline]
    fn from(pos: UiVec2) -> Self {
        glamour::Point2::from_raw(pos.vec2)
    }
}
impl<T: glamour::Unit<Scalar = f32>> From<glamour::Vector2<T>> for UiVec2 {
    #[inline]
    fn from(pos: glamour::Vector2<T>) -> Self {
        Self::from(pos.to_raw())
    }
}
impl<T: glamour::Unit<Scalar = f32>> From<glamour::Size2<T>> for UiVec2 {
    #[inline]
    fn from(pos: glamour::Size2<T>) -> Self {
        Self::from(pos.to_raw())
    }
}
impl<T: glamour::Unit<Scalar = f32>> From<glamour::Point2<T>> for UiVec2 {
    #[inline]
    fn from(pos: glamour::Point2<T>) -> Self {
        Self::from(pos.to_raw())
    }
}
impl From<UiVec2> for [f32; 2] {
    #[inline]
    fn from(pos: UiVec2) -> Self {
        pos.vec2.into()
    }
}
impl From<&'_ UiVec2> for [f32; 2] {
    #[inline]
    fn from(pos: &UiVec2) -> Self {
        pos.vec2.into()
    }
}
impl From<[f32; 2]> for UiVec2 {
    #[inline]
    fn from(pos: [f32; 2]) -> Self {
        Self::with_vec2(Vec2::from_array(pos))
    }
}
impl From<Vec2> for UiVec2 {
    #[inline]
    fn from(pos: Vec2) -> Self {
        Self::with_vec2(pos)
    }
}
impl Default for UiVec2 {
    #[inline]
    fn default() -> Self {
        Self::DEFAULT
    }
}
