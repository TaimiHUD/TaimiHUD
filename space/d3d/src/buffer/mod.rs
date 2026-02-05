use {
    crate::{prelude::*, D3dContext, D3dContextBindableSlot, D3dDevice},
    std::{ffi, mem, ptr::NonNull},
};

pub mod dxgi;

pub unsafe trait D3dContextBindableVertexBuffer<D3DC: D3dContext>:
    D3dContextBindableSlot<D3DC>
{
    fn vertex_buffer_ptr(&self) -> *mut ffi::c_void;
    fn vertex_buffer_stride(&self) -> u32;
    fn vertex_buffer_offset(&self) -> u32;

    unsafe fn vertex_buffer_buffer(
        &self,
    ) -> Option<InterfaceRef<'_, <D3DC::IDevice as D3dDevice>::IBuffer>> {
        NonNull::new(self.vertex_buffer_ptr() as *mut _).map(|raw| InterfaceRef::from_raw(raw))
    }
}

unsafe impl<D3DC: D3dContext, B: ?Sized + D3dContextBindableVertexBuffer<D3DC>>
    D3dContextBindableVertexBuffer<D3DC> for &'_ B
{
    fn vertex_buffer_ptr(&self) -> *mut ffi::c_void {
        D3dContextBindableVertexBuffer::vertex_buffer_ptr(*self)
    }
    fn vertex_buffer_stride(&self) -> u32 {
        D3dContextBindableVertexBuffer::vertex_buffer_stride(*self)
    }
    fn vertex_buffer_offset(&self) -> u32 {
        D3dContextBindableVertexBuffer::vertex_buffer_offset(*self)
    }
    unsafe fn vertex_buffer_buffer(
        &self,
    ) -> Option<InterfaceRef<'_, <D3DC::IDevice as D3dDevice>::IBuffer>> {
        D3dContextBindableVertexBuffer::vertex_buffer_buffer(*self)
    }
}

pub unsafe trait D3dBufferData: Copy /* Pod*/ {
    fn stride() -> usize {
        mem::size_of::<Self>().max(mem::align_of::<Self>())
    }
}

#[macro_export]
macro_rules! d3d_impl_unsafe_bufferdata {
    ($(#[$meta:meta])* for{$($temp:tt)*} $(where{$($where_:tt)*})? $ty:ty; $($($rest:tt)+)?) => {
        $(#[$meta])*
        unsafe impl<$($temp)*> $crate::buffer::D3dBufferData for $ty
            $(where $($where_)*)?
        {}
        $(
            $crate::buffer::d3d_impl_unsafe_bufferdata! {
                $($rest)*
            }
        )?
    };
    (for $($(#[$meta:meta])* $ty:ty),+$(,)?; $($($rest:tt)+)?) => {
        $(
            $(#[$meta])*
            unsafe impl $crate::buffer::D3dBufferData for $ty {}
        )*
        $(
            $crate::buffer::d3d_impl_unsafe_bufferdata! {
                $($rest)*
            }
        )?
    };
}
pub use d3d_impl_unsafe_bufferdata;

d3d_impl_unsafe_bufferdata! {
    for{const N: usize, T} where{T: D3dBufferData} [T; N];
    for{T: D3dBufferData} (T,);
    for{T: D3dBufferData, U: D3dBufferData} (T, U);
    for{T: D3dBufferData, U: D3dBufferData, V: D3dBufferData} (T, U, V);
    for{T: D3dBufferData, U: D3dBufferData, V: D3dBufferData, X: D3dBufferData} (T, U, V, X);

    for{U: Unit} where{U::Scalar: D3dBufferData} glamour::Vector2<U>;
    for{U: Unit} where{U::Scalar: D3dBufferData} glamour::Vector3<U>;
    for{U: Unit} where{U::Scalar: D3dBufferData} glamour::Vector4<U>;
    for{U: Unit} where{U::Scalar: D3dBufferData} glamour::Point2<U>;
    for{U: Unit} where{U::Scalar: D3dBufferData} glamour::Point3<U>;
    for{U: Unit} where{U::Scalar: D3dBufferData} glamour::Point4<U>;
    for{U: Unit} where{U::Scalar: D3dBufferData} glamour::Size2<U>;
    for{U: Unit} where{U::Scalar: D3dBufferData} glamour::Size3<U>;
    for{U: Unit} where{U::Scalar: D3dBufferData} glamour::Box2<U>;
    for{U: Unit} where{U::Scalar: D3dBufferData} glamour::Box3<U>;
    for{U: Unit, D: Unit} where{U::Scalar: D3dBufferData} glamour::Transform2<U, D>;
    for{U: Unit, D: Unit} where{U::Scalar: D3dBufferData} glamour::Transform3<U, D>;
    for{S: Scalar + D3dBufferData} glamour::Angle<S>;
    for{S: Scalar + D3dBufferData} glamour::Matrix2<S>;
    for{S: Scalar + D3dBufferData} glamour::Matrix3<S>;
    for{S: Scalar + D3dBufferData} glamour::Matrix4<S>;
    for
        usize, isize,
        u8, u16, u32, u64, u128,
        i8, i16, i32, i64, i128,
        f32, f64,
        glam::Vec2, glam::Vec3, glam::Vec4, glam::Vec3A,
        glam::Mat2, glam::Mat3, glam::Mat4, glam::Mat3A,
        glam::DMat2, glam::DMat3, glam::DMat4,
        glam::Affine2, glam::Affine3A, glam::Quat,
        glam::DAffine2, glam::DAffine3, glam::DQuat,
        glam::DVec2, glam::DVec3, glam::DVec4,
        glam::BVec2, glam::BVec3, glam::BVec4,
        glam::IVec2, glam::IVec3, glam::IVec4,
        glam::UVec2, glam::UVec3, glam::UVec4,
        glam::I8Vec2, glam::I8Vec3, glam::I8Vec4,
        glam::U8Vec2, glam::U8Vec3, glam::U8Vec4,
        glam::I16Vec2, glam::I16Vec3, glam::I16Vec4,
        glam::U16Vec2, glam::U16Vec3, glam::U16Vec4,
        glam::I64Vec2, glam::I64Vec3, glam::I64Vec4,
        glam::U64Vec2, glam::U64Vec3, glam::U64Vec4,
        glam::USizeVec2, glam::USizeVec3, glam::USizeVec4,
    ;
}

#[cfg(target_arch = "x86")]
use core::arch::x86 as arch;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64 as arch;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
d3d_impl_unsafe_bufferdata! {
    for
        arch::__m128, arch::__m128d, arch::__m128i,
        arch::__m256, arch::__m256d, arch::__m256i,
        arch::__m512, arch::__m512d, arch::__m512i,
        // TODO: arch::__mXXXbh, __mXXXh, bf16,
    ;
}
