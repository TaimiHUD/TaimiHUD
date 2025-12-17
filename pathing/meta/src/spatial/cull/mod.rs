use {
    crate::packs::id::{MarkerId, MarkerIndex},
    bvh::aabb::{Aabb, IntersectsAabb},
    core::{mem, ops},
};

pub use self::frustum::{LazyFrustum, MapFrustum};
mod frustum;

/// TODO: consider contains relationships to avoid testing children,
/// per-plane to invalidate cached results, etc.
pub trait BvhQuery<const D: usize>: IntersectsAabb<f32, D> {
    /// test a (likely small) shape
    #[inline]
    fn intersects_aabb_shape(&self, aabb: &Aabb<f32, D>) -> bool {
        self.intersects_aabb(aabb)
    }
    /// TODO: might be nice if we knew if it were actually a billboard...
    #[inline(always)]
    fn intersects_aabb_poi(&self, aabb: &Aabb<f32, D>) -> bool {
        self.intersects_aabb_shape(aabb)
    }
    #[inline]
    fn intersects_aabb_marker(&self, aabb: &Aabb<f32, D>, id: &MarkerId) -> bool {
        match id.get_marker_index().namespace() {
            MarkerIndex::NS_POI => self.intersects_aabb_poi(aabb),
            MarkerIndex::NS_TRAIL | _ => self.intersects_aabb(aabb),
        }
    }
}
impl<const D: usize> BvhQuery<D> for Aabb<f32, D> {
    /// TODO?
    #[inline]
    fn intersects_aabb_marker(&self, aabb: &Aabb<f32, D>, _id: &MarkerId) -> bool {
        self.intersects_aabb(aabb)
    }
}

#[derive(Debug, Copy, Clone)]
#[repr(transparent)]
pub struct BvhQueryOf<T: ?Sized>(pub T);
impl<T: ?Sized> BvhQueryOf<T> {
    #[inline(always)]
    pub const fn from_ref(query: &T) -> &Self {
        unsafe { mem::transmute(query) }
    }
}
impl<T: ?Sized> ops::Deref for BvhQueryOf<T> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &Self::Target { &self.0 }
}
impl<T: ?Sized, const D: usize> BvhQuery<D> for BvhQueryOf<T> where
    T: IntersectsAabb<f32, D>,
{
    /// TODO?
    #[inline]
    fn intersects_aabb_marker(&self, aabb: &Aabb<f32, D>, _id: &MarkerId) -> bool {
        self.intersects_aabb(aabb)
    }
}
impl<T: ?Sized, const D: usize> IntersectsAabb<f32, D> for BvhQueryOf<T> where
    T: IntersectsAabb<f32, D>,
{
    #[inline(always)]
    fn intersects_aabb(&self, aabb: &Aabb<f32, D>) -> bool {
        self.0.intersects_aabb(aabb)
    }
}
