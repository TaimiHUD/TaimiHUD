use {
    crate::{
        dx11::prelude::*,
        state::{D3dState, D3dStateSnapshot},
        D3dContextBindable,
    },
    num_traits::AsPrimitive,
    std::{mem, ops, slice},
};

pub use crate::dx11::d3d11::D3D11_VIEWPORT;

#[derive(Debug, Default, Copy, Clone, PartialEq)]
#[repr(transparent)]
pub struct Viewport {
    pub viewport: D3D11_VIEWPORT,
}

impl Viewport {
    pub const MAX_VIEWPORTS: usize =
        d3d11::D3D11_VIEWPORT_AND_SCISSORRECT_OBJECT_COUNT_PER_PIPELINE as usize;

    pub const EMPTY: Self = Self::with_viewport(D3D11_VIEWPORT {
        TopLeftX: 0.0,
        TopLeftY: 0.0,
        Width: 0.0,
        Height: 0.0,
        MinDepth: 0.0,
        MaxDepth: 0.0,
    });

    pub const fn with_viewport(viewport: D3D11_VIEWPORT) -> Self {
        Self { viewport }
    }

    pub const fn from_ref(viewport: &D3D11_VIEWPORT) -> &Self {
        unsafe { mem::transmute(viewport) }
    }

    pub fn new_snapshot<const N: usize>(context: &Dx11Context) -> [Self; N] {
        let mut viewports = [D3D11_VIEWPORT::default(); N];
        let mut viewport_count = viewports.len() as _;
        unsafe {
            context.RSGetViewports(&mut viewport_count, Some(viewports.as_mut_ptr()));
        }
        Self::array_from_raw(viewports)
    }
    /// TODO: unclear if null arg is required to get full/untruncated count or not
    pub fn new_snapshot_vec(context: &Dx11Context) -> Vec<Self> {
        let mut capacity = 8.min(Self::MAX_VIEWPORTS) as u32;
        let mut viewports = Vec::<D3D11_VIEWPORT>::with_capacity(capacity as usize);
        let mut viewport_count = capacity;
        for _ in 0..2 {
            unsafe {
                let uninit = viewports.spare_capacity_mut();
                capacity = viewport_count;
                debug_assert!(uninit.len() >= viewport_count as usize);
                context.RSGetViewports(
                    &mut viewport_count,
                    Some(uninit.as_mut_ptr() as *mut D3D11_VIEWPORT),
                );
                match viewport_count {
                    #[cfg(todo)]
                    viewport_count if viewport_count == capacity => {
                        // docs are unclear, so double-check?
                        viewports.reserve_exact(Self::snapshot_count(context) as usize);
                        viewport_count = Self::snapshot_count(context) as u32;
                    },
                    #[cfg(debug_assertions)]
                    viewport_count if viewport_count == capacity => {
                        // double-check that it doesn't truncate to our len...
                        debug_assert_eq!(Self::snapshot_count(context), viewport_count as usize);
                    },
                    _ => (),
                }
                if viewport_count > capacity {
                    viewports.reserve_exact(viewport_count as usize);
                } else {
                    break
                }
            }
        }
        unsafe {
            viewports.set_len(viewport_count as usize);
            Self::vec_from_raw(viewports)
        }
    }
    pub fn snapshot_count(context: &Dx11Context) -> usize {
        let mut viewport_count = 0u32;
        unsafe {
            context.RSGetViewports(&mut viewport_count, None);
        }
        viewport_count as usize
    }

    /// Aligned to top-left origin (0, 0, 0)
    pub fn with_size<U: Unit>(size: Size3<U>) -> Self
    where
        U::Scalar: AsPrimitive<f32>,
    {
        let size = size.as_::<f32>();
        Self::with_viewport(D3D11_VIEWPORT {
            Width: size.width,
            Height: size.height,
            MaxDepth: size.depth,
            ..Self::EMPTY.viewport
        })
    }

    pub fn with_bounds<U: Unit>(bounds: Box3<U>) -> Self
    where
        U::Scalar: AsPrimitive<f32>,
    {
        let top_left = bounds.min.with_y(bounds.max.y).as_::<f32>();
        let bottom_right_z = AsPrimitive::as_(bounds.max.z);
        let size = Box2::new(bounds.min.truncate(), bounds.max.truncate())
            .size()
            .as_::<f32>();
        let viewport = D3D11_VIEWPORT {
            TopLeftX: top_left.x,
            TopLeftY: top_left.y,
            Width: size.width,
            Height: size.height,
            MinDepth: top_left.z,
            MaxDepth: bottom_right_z,
        };
        Self { viewport }
    }

    pub fn is_empty(&self) -> bool {
        *self == Viewport::EMPTY
    }
    pub fn get(&self) -> Option<&Self> {
        (!self.is_empty()).then_some(self)
    }
    pub fn box2(&self) -> Box2<f32> {
        let min = Point2::new(
            self.viewport.TopLeftX,
            self.viewport.TopLeftY + self.viewport.Height,
        );
        let max = Point2::new(
            self.viewport.TopLeftX + self.viewport.Width,
            self.viewport.TopLeftY,
        );
        Box2::new(min, max)
    }
    pub fn box3(&self) -> Box3<f32> {
        let bounds = self.box2();
        Box3::new(
            bounds.min.extend(self.viewport.MinDepth),
            bounds.max.extend(self.viewport.MaxDepth),
        )
    }
    pub fn top_left(&self) -> Point2<f32> {
        Point2::new(self.viewport.TopLeftX, self.viewport.TopLeftY)
    }
    pub fn top_right(&self) -> Point2<f32> {
        Point2::new(self.viewport.TopLeftX + self.viewport.Width, self.viewport.TopLeftY)
    }
    pub fn bottom_left(&self) -> Point2<f32> {
        Point2::new(self.viewport.TopLeftX, self.viewport.TopLeftY + self.viewport.Height)
    }
    pub fn bottom_right(&self) -> Point2<f32> {
        Point2::new(self.viewport.TopLeftX + self.viewport.Width, self.viewport.TopLeftY + self.viewport.Height)
    }
    pub fn rect(&self) -> Rect<f32> {
        Rect::new(self.top_left(), self.size2())
    }
    pub fn size2(&self) -> Size2<f32> {
        Size2::new(self.viewport.Width, self.viewport.Height)
    }
    pub fn depth_range(&self) -> ops::RangeInclusive<f32> {
        self.viewport.MinDepth..=self.viewport.MaxDepth
    }
    pub fn size3(&self) -> Size3<f32> {
        self.size2()
            .extend(self.viewport.MaxDepth - self.viewport.MinDepth)
    }

    pub fn slice_truncate(viewports: &[Self]) -> &[Self] {
        let len = match viewports.iter().rposition(|vp| !vp.is_empty()) {
            Some(len) => len,
            None => return &[],
        };
        unsafe { viewports.get_unchecked(..=len) }
    }

    pub fn slice_as_raw(viewports: &[Self]) -> &[D3D11_VIEWPORT] {
        unsafe { mem::transmute(viewports) }
    }
    pub fn slice_from_raw(viewports: &[D3D11_VIEWPORT]) -> &[Self] {
        unsafe { mem::transmute(viewports) }
    }
    pub fn array_to_raw<const N: usize>(viewports: [Self; N]) -> [D3D11_VIEWPORT; N] {
        unsafe {
            // XXX: transmute_unchecked()
            mem::transmute_copy(&viewports)
        }
    }
    pub fn array_from_raw<const N: usize>(viewports: [D3D11_VIEWPORT; N]) -> [Self; N] {
        unsafe { mem::transmute_copy(&viewports) }
    }
    pub fn vec_from_raw(viewports: Vec<D3D11_VIEWPORT>) -> Vec<Self> {
        unsafe { mem::transmute(viewports) }
    }

    pub fn bind_set<V: AsRef<[D3D11_VIEWPORT]>>(context: &Dx11Context, viewports: V) {
        let viewports = viewports.as_ref();
        let viewports = match viewports.is_empty() {
            #[cfg(todo = "unnecessary")]
            true => None,
            _ => Some(viewports),
        };
        unsafe {
            context.RSSetViewports(viewports);
        }
    }
}

impl AsRef<D3D11_VIEWPORT> for Viewport {
    fn as_ref(&self) -> &D3D11_VIEWPORT {
        &self.viewport
    }
}
impl AsRef<[D3D11_VIEWPORT]> for Viewport {
    fn as_ref(&self) -> &[D3D11_VIEWPORT] {
        slice::from_ref(self.as_ref())
    }
}

impl<'a> From<&'a D3D11_VIEWPORT> for &'a Viewport {
    fn from(viewport: &'a D3D11_VIEWPORT) -> Self {
        Viewport::from_ref(viewport)
    }
}
impl<'a> From<&'a Viewport> for &'a D3D11_VIEWPORT {
    fn from(viewport: &'a Viewport) -> Self {
        &viewport.viewport
    }
}
impl From<D3D11_VIEWPORT> for Viewport {
    fn from(viewport: D3D11_VIEWPORT) -> Self {
        Self { viewport }
    }
}
impl From<Viewport> for D3D11_VIEWPORT {
    fn from(viewport: Viewport) -> Self {
        viewport.viewport
    }
}
impl<U: Unit> From<Size3<U>> for Viewport
where
    U::Scalar: AsPrimitive<f32>,
{
    fn from(viewport: Size3<U>) -> Self {
        Self::with_size(viewport)
    }
}
impl<U: Unit> From<Box3<U>> for Viewport
where
    U::Scalar: AsPrimitive<f32>,
{
    fn from(viewport: Box3<U>) -> Self {
        Self::with_bounds(viewport)
    }
}
impl<U: Unit> From<Size2<U>> for Viewport
where
    U::Scalar: AsPrimitive<f32>,
{
    fn from(viewport: Size2<U>) -> Self {
        Self::with_size(viewport.extend(num_traits::One::one()))
    }
}
impl<U: Unit> From<Box2<U>> for Viewport
where
    U::Scalar: AsPrimitive<f32>,
{
    fn from(viewport: Box2<U>) -> Self {
        Self::with_bounds(Box3::new(
            viewport.min.extend(num_traits::Zero::zero()),
            viewport.max.extend(num_traits::One::one()),
        ))
    }
}

impl D3dContextBindable<Dx11Context> for Viewport {
    fn set(&self, context: &Dx11Context) {
        Self::bind_set(context, self)
    }
}
impl D3dContextBindable<Dx11Context> for [Viewport] {
    fn set(&self, context: &Dx11Context) {
        let viewports = Viewport::slice_as_raw(self);
        Viewport::bind_set(context, viewports)
    }
}

impl_d3d! {
    impl{D3DC} D3dStateSnapshot<D3DC> for [Viewport; N];
    impl{D3DC} D3dState<D3DC> for [Viewport];
}
impl<const N: usize> D3dStateSnapshot<Dx11Context> for [Viewport; N] {
    fn empty_state(_: &Dx11Device) -> anyhow::Result<Self> {
        Ok([Viewport::EMPTY; N])
    }
    fn snapshot_state(context: &Dx11Context) -> Self {
        Viewport::new_snapshot::<N>(context)
    }
}
impl D3dStateSnapshot<Dx11Context> for Vec<Viewport> {
    fn empty_state(_: &Dx11Device) -> anyhow::Result<Self> {
        Ok(Vec::new())
    }
    fn snapshot_state(context: &Dx11Context) -> Self {
        Viewport::new_snapshot_vec(context)
    }
}
impl D3dState<Dx11Context> for [Viewport] {
    fn restore_state(&self, context: &Dx11Context) {
        match self {
            viewports => Viewport::slice_truncate(viewports).set(context),
            #[cfg(todo)]
            viewports => viewports.set(context),
        }
    }
}
