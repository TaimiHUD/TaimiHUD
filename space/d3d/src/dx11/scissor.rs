pub use windows::Win32::Foundation::RECT;
use {
    crate::{dx11::prelude::*, D3dContextBindable},
    glamour::{Box2, Point2, Rect, Size2, Unit},
    num_traits::AsPrimitive,
    std::{mem, slice},
};

#[derive(Debug, Default, Copy, Clone, PartialEq)]
#[repr(transparent)]
pub struct ScissorRect {
    pub rect: RECT,
}

impl ScissorRect {
    pub const MAX_SCISSORS: usize =
        d3d11::D3D11_VIEWPORT_AND_SCISSORRECT_OBJECT_COUNT_PER_PIPELINE as usize;

    pub const EMPTY: Self = Self::with_rect(RECT { left: 0, top: 0, right: 0, bottom: 0 });

    pub const fn with_rect(rect: RECT) -> Self {
        Self { rect }
    }

    pub const fn from_ref(rect: &RECT) -> &Self {
        unsafe { mem::transmute(rect) }
    }
    pub const fn from_mut(rect: &RECT) -> &Self {
        unsafe { mem::transmute(rect) }
    }

    pub fn snapshot_count(context: &Dx11Context) -> usize {
        let mut scissor_count = 0u32;
        unsafe {
            context.RSGetScissorRects(&mut scissor_count, None);
        }
        scissor_count as usize
    }
    pub fn new_snapshot<const N: usize>(context: &Dx11Context) -> [Self; N] {
        let mut scissors = [RECT::default(); N];
        let mut scissor_count = scissors.len() as _;
        unsafe {
            context.RSGetScissorRects(&mut scissor_count, Some(scissors.as_mut_ptr()));
        }
        Self::array_from_raw(scissors)
    }
    /// TODO: unclear if null arg is required to get full/untruncated count or not
    pub fn new_snapshot_vec(context: &Dx11Context) -> Vec<Self> {
        let initial_len = 8.min(Self::MAX_SCISSORS);
        let mut scissors = Vec::<RECT>::with_capacity(initial_len);
        let mut scissor_count = 0;
        for _ in 0..2 {
            unsafe {
                let uninit = scissors.spare_capacity_mut();
                let capacity = uninit.len() as u32;
                scissor_count = capacity;
                context.RSGetScissorRects(&mut scissor_count, Some(uninit.as_mut_ptr() as *mut RECT));
                match scissor_count {
                    #[cfg(todo)]
                    scissor_count if scissor_count == capacity => {
                        // docs are unclear, so double-check?
                        scissors.reserve_exact(Self::snapshot_count(context) as usize);
                        scissor_count = Self::snapshot_count(context) as u32;
                    },
                    #[cfg(debug_assertions)]
                    scissor_count if scissor_count == capacity => {
                        // double-check that it doesn't truncate to our len...
                        debug_assert_eq!(Self::snapshot_count(context), scissor_count as usize);
                    },
                    _ => (),
                }
                if scissor_count > capacity {
                    scissor_count = capacity;
                    scissors.reserve_exact(scissor_count as usize);
                } else {
                    break
                }
            }
        }
        unsafe {
            scissors.set_len(scissor_count as usize);
            Self::vec_from_raw(scissors)
        }
    }

    /// Aligned to top-left origin (0, 0, 0)
    pub fn with_size<U: Unit>(size: Size2<U>) -> Self
    where
        U::Scalar: AsPrimitive<i32>,
    {
        Self::with_bounds(Box2::new(glamour::Point2::ZERO, size.to_vector().to_point()))
    }

    pub fn with_bounds<U: Unit>(rect: Box2<U>) -> Self
    where
        U::Scalar: AsPrimitive<i32>,
    {
        let rect = rect.as_::<i32>();
        Self::with_rect(RECT {
            left: rect.min.x,
            top: rect.min.y,
            right: rect.max.x,
            bottom: rect.max.y,
        })
    }
    pub fn with_bounds_rect<U: Unit>(rect: Rect<U>) -> Self
    where
        U::Scalar: AsPrimitive<i32>,
    {
        let rect = Rect::<i32>::new(rect.origin.as_(), rect.size.as_());
        Self::with_bounds(rect.to_box2())
    }

    pub fn is_empty(&self) -> bool {
        *self == Self::EMPTY
    }

    pub fn box2(&self) -> Box2<i32> {
        Box2::new(
            Point2::new(self.rect.left, self.rect.top),
            Point2::new(self.rect.right, self.rect.bottom),
        )
    }
    pub fn box2_f32(&self) -> Box2<f32> {
        self.box2().as_()
    }
    pub fn size2(&self) -> Size2<i32> {
        self.box2().size()
    }

    pub fn slice_truncate(scissors: &[Self]) -> &[Self] {
        let len = match scissors.iter().rposition(|vp| !vp.is_empty()) {
            Some(len) => len,
            None => return scissors,
        };
        unsafe { scissors.get_unchecked(..len) }
    }

    pub fn slice_as_raw(scissors: &[Self]) -> &[RECT] {
        unsafe { mem::transmute(scissors) }
    }
    pub fn slice_from_raw(scissors: &[RECT]) -> &[Self] {
        unsafe { mem::transmute(scissors) }
    }
    pub fn array_to_raw<const N: usize>(scissors: [Self; N]) -> [RECT; N] {
        unsafe {
            // XXX: transmute_unchecked()
            mem::transmute_copy(&scissors)
        }
    }
    pub fn array_from_raw<const N: usize>(scissors: [RECT; N]) -> [Self; N] {
        unsafe { mem::transmute_copy(&scissors) }
    }
    pub fn vec_from_raw(scissors: Vec<RECT>) -> Vec<Self> {
        unsafe { mem::transmute(scissors) }
    }

    pub fn bind_set<V: AsRef<[RECT]>>(context: &Dx11Context, scissors: V) {
        let scissors = scissors.as_ref();
        let scissors = match scissors.is_empty() {
            #[cfg(todo = "unnecessary")]
            true => None,
            _ => Some(scissors),
        };
        unsafe {
            context.RSSetScissorRects(scissors);
        }
    }
}

impl AsRef<RECT> for ScissorRect {
    fn as_ref(&self) -> &RECT {
        &self.rect
    }
}
impl AsRef<[RECT]> for ScissorRect {
    fn as_ref(&self) -> &[RECT] {
        slice::from_ref(self.as_ref())
    }
}

impl<'a> From<&'a RECT> for &'a ScissorRect {
    fn from(scissor: &'a RECT) -> Self {
        ScissorRect::from_ref(scissor)
    }
}
impl<'a> From<&'a ScissorRect> for &'a RECT {
    fn from(scissor: &'a ScissorRect) -> Self {
        &scissor.rect
    }
}
impl From<RECT> for ScissorRect {
    fn from(rect: RECT) -> Self {
        Self { rect }
    }
}
impl From<ScissorRect> for RECT {
    fn from(scissor: ScissorRect) -> Self {
        scissor.rect
    }
}
impl<U: Unit> From<Rect<U>> for ScissorRect
where
    U::Scalar: AsPrimitive<i32>,
{
    fn from(scissor: Rect<U>) -> Self {
        Self::with_bounds_rect(scissor)
    }
}
impl<U: Unit> From<Box2<U>> for ScissorRect
where
    U::Scalar: AsPrimitive<i32>,
{
    fn from(scissor: Box2<U>) -> Self {
        Self::with_bounds(scissor)
    }
}
impl<U: Unit> From<Size2<U>> for ScissorRect
where
    U::Scalar: AsPrimitive<i32>,
{
    fn from(scissor: Size2<U>) -> Self {
        Self::with_size(scissor)
    }
}

impl D3dContextBindable<Dx11Context> for ScissorRect {
    fn set(&self, context: &Dx11Context) {
        Self::bind_set(context, self)
    }
}
impl D3dContextBindable<Dx11Context> for [ScissorRect] {
    fn set(&self, context: &Dx11Context) {
        let scissors = ScissorRect::slice_as_raw(self);
        ScissorRect::bind_set(context, scissors)
    }
}
