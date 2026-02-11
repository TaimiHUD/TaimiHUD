#[cfg(todo)]
use glam::f32::Affine3;
use {
    core::mem,
    glam::f32::{Affine3A, Mat3, Mat4, Vec3A},
    glamour::{FloatScalar, Matrix3, Matrix4, Scalar, Unit, Vector3, Vector4},
};

#[cfg(todo)]
#[derive(Debug, Copy, Clone, Default, PartialEq)]
#[repr(transparent)]
pub struct Mat34 {
    pub matrix: Mat43,
}

#[derive(Debug, Copy, Clone, Default, PartialEq)]
#[repr(C)]
pub struct Mat43<U: Unit = f32> {
    pub x_axis: Vector4<U>,
    pub y_axis: Vector4<U>,
    pub z_axis: Vector4<U>,
}

impl<U: Unit> Mat43<U> {
    pub const ZERO3: Self = Self {
        x_axis: Vector4::ZERO,
        y_axis: Vector4::ZERO,
        z_axis: Vector4::ZERO,
    };
    pub const IDENTITY: Self = Self {
        x_axis: Vector4::X,
        y_axis: Vector4::Y,
        z_axis: Vector4::Z,
    };
    #[inline]
    pub const fn new(x_axis: Vector4<U>, y_axis: Vector4<U>, z_axis: Vector4<U>) -> Self {
        Self { x_axis, y_axis, z_axis }
    }
    #[inline]
    pub fn from_cols<V: Into<Vector4<U>>>(x_axis: V, y_axis: V, z_axis: V) -> Self {
        Self::new(x_axis.into(), y_axis.into(), z_axis.into())
    }

    #[inline]
    pub const fn as_cols_array(&self) -> &[U::Scalar; 12] {
        unsafe { mem::transmute(self) }
    }

    pub fn to_cols_array(&self) -> [U::Scalar; 12] {
        *self.as_cols_array()
    }

    #[inline]
    pub const fn x_col(&self) -> Vector3<U> {
        Self::vec4to3(self.x_axis)
    }
    #[inline]
    pub const fn y_col(&self) -> Vector3<U> {
        Self::vec4to3(self.y_axis)
    }
    #[inline]
    pub const fn z_col(&self) -> Vector3<U> {
        Self::vec4to3(self.z_axis)
    }
    pub const W_COL: Vector3<U> = Vector3::ZERO;
    pub const W_AXIS: Vector4<U> = Vector4::W;

    pub const fn x_row(&self) -> Vector3<U> {
        Vector3::new(self.x_axis.x, self.y_axis.x, self.z_axis.x)
    }
    pub const fn y_row(&self) -> Vector3<U> {
        Vector3::new(self.x_axis.y, self.y_axis.y, self.z_axis.y)
    }
    pub const fn z_row(&self) -> Vector3<U> {
        Vector3::new(self.x_axis.z, self.y_axis.z, self.z_axis.z)
    }
    pub const fn w_row(&self) -> Vector3<U> {
        Vector3::new(self.x_axis.w, self.y_axis.w, self.z_axis.w)
    }

    #[inline]
    pub const fn as_cols(&self) -> &[Vector4<U>; 3] {
        unsafe { mem::transmute(self) }
    }

    #[inline]
    const fn vec4to3(axis: Vector4<U>) -> Vector3<U> {
        unsafe { mem::transmute_copy(&axis) }
    }
}
impl<U: Unit + Scalar> Mat43<U> {
    pub fn try_from_mat4(m4: Matrix4<U>) -> Option<Self> {
        match m4.w_axis {
            w if w != Vector4::W => None,
            _ => Some(Self::from_mat4_unchecked(m4)),
        }
    }

    pub fn try_from_mat4_translation(m4: Matrix4<U>) -> Option<Self> {
        let w_row = Vector4::<U>::new(m4.x_axis.w, m4.y_axis.w, m4.z_axis.w, m4.w_axis.w);
        match w_row {
            w if w != Vector4::<U>::W => None,
            _ => Some(Self::from_mat4_translation_unchecked(m4)),
        }
    }

    #[inline]
    pub fn from_mat3<M: Into<Matrix3<U>>>(m3: M) -> Self {
        Self::from_mat3_translation(m3.into(), Vector3::ZERO)
    }
    #[inline]
    pub fn from_mat3_translation(m3: Matrix3<U>, w: Vector3<U>) -> Self {
        Self::from_cols(
            m3.x_axis.extend(w.x),
            m3.y_axis.extend(w.y),
            m3.z_axis.extend(w.z),
        )
    }

    pub const fn to_mat4(self) -> Matrix4<U> {
        Matrix4 {
            x_axis: self.x_axis,
            y_axis: self.y_axis,
            z_axis: self.z_axis,
            w_axis: Vector4::W,
        }
    }
    pub fn to_mat4_translation(self) -> Matrix4<U>
    where
        U: FloatScalar,
    {
        Matrix4::from_mat3_translation(self.to_mat3(), self.w_row())
    }
    pub const fn to_mat3(self) -> Matrix3<U> {
        Matrix3 {
            x_axis: self.x_col(),
            y_axis: self.y_col(),
            z_axis: self.z_col(),
        }
    }

    #[inline]
    pub const fn from_mat4_ref_unchecked(m4: &Matrix4<U>) -> &Self {
        unsafe { mem::transmute(m4) }
    }
    #[inline]
    pub const fn from_mat4_unchecked(m4: Matrix4<U>) -> Self {
        unsafe { mem::transmute_copy(&m4) }
    }
    #[inline]
    pub fn from_mat4_translation_unchecked(mut m4: Matrix4<U>) -> Self {
        m4.x_axis.w = m4.w_axis.x;
        m4.y_axis.w = m4.w_axis.y;
        m4.z_axis.w = m4.w_axis.z;
        Self::from_mat4_unchecked(m4)
    }

    pub fn transpose(self) -> Self
    where
        U::Scalar: FloatScalar,
    {
        Self::from_mat3_translation(self.to_mat3().transpose(), self.w_row())
    }
}
impl Mat43 {
    #[cfg(todo)]
    #[inline]
    pub fn from_affine3(a3: Affine3) -> Self {
        Self::from_mat4_translation_unchecked(Mat4::from(a3))
    }
    /// TODO: transpose?
    #[inline]
    pub fn from_affine3a(a3: Affine3A) -> Self {
        Self::from_mat4_translation_unchecked(Matrix4::from(Mat4::from(a3)))
    }

    #[cfg(todo)]
    pub const fn to_affine3(self) -> Affine3 {
        Affine3::from_cols(self.x_col(), self.y_col(), self.z_col(), self.w_row())
    }
    /// TODO: it's fine if we leave w non-zero right?
    pub fn to_affine3a(self) -> Affine3A {
        Affine3A::from_cols(
            Vec3A::from_vec4(self.x_axis.into()),
            Vec3A::from_vec4(self.y_axis.into()),
            Vec3A::from_vec4(self.z_axis.into()),
            Vec3A::from(self.w_row()),
        )
    }
}
impl From<Affine3A> for Mat43 {
    #[inline]
    fn from(a3: Affine3A) -> Self {
        Self::from_affine3a(a3)
    }
}
impl From<Mat3> for Mat43 {
    #[inline]
    fn from(m3: Mat3) -> Self {
        Self::from_mat3(m3)
    }
}
impl<U: Unit + Scalar> From<Matrix3<U>> for Mat43<U> {
    #[inline]
    fn from(m3: Matrix3<U>) -> Self {
        Self::from_mat3(m3)
    }
}
impl<U: Unit + Scalar> From<Mat43<U>> for Matrix3<U> {
    #[inline]
    fn from(m43: Mat43<U>) -> Self {
        m43.to_mat3()
    }
}
