use core::mem;

#[inline]
pub const fn f32_bits<const N: usize>(f: [f32; N]) -> [u32; N] {
    unsafe {
        // XXX: transmute_unchecked is unstable...
        mem::transmute_copy(&f)
    }
}
#[inline]
pub fn vec32_bits<const N: usize, T>(f: T) -> [u32; N]
where
    T: Into<[f32; N]>,
{
    f32_bits(f.into())
}
pub fn vec32_eq<const N: usize, T>(lhs: T, rhs: T) -> bool
where
    T: Into<[f32; N]>,
{
    vec32_bits(lhs) == vec32_bits(rhs)
}
#[inline]
pub const fn f32_ibits<const N: usize>(f: [f32; N]) -> [i32; N] {
    unsafe {
        // XXX: transmute_unchecked is unstable...
        mem::transmute_copy(&f)
    }
}
#[inline]
pub fn vec32_ibits<const N: usize, T>(f: T) -> [i32; N]
where
    T: Into<[f32; N]>,
{
    f32_ibits(f.into())
}
