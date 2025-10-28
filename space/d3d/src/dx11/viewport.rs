use {
    crate::{dx11::prelude::*, D3dContextBindable},
    std::{mem, slice},
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

    pub fn new_snapshot<const N: usize>(context: &Dx11Context) -> [Viewport; N] {
        let mut viewports = [D3D11_VIEWPORT::default(); N];
        let mut viewport_count = viewports.len() as _;
        unsafe {
            context.RSGetViewports(&mut viewport_count, Some(viewports.as_mut_ptr()));
        }
        Self::array_from_raw(viewports)
    }

    /// Aligned to top-left origin (0, 0, 0)
    pub const fn with_size<U: Unit<Scalar = f32>>(size: Size3<U>) -> Self {
        Self::with_viewport(D3D11_VIEWPORT {
            Width: size.width,
            Height: size.height,
            MaxDepth: size.depth,
            ..Self::EMPTY.viewport
        })
    }

    pub fn with_bounds<U: Unit>(bounds: Box3<U>) -> Self
    where
        U::Scalar: Into<f32>,
    {
        let size = Box2::new(bounds.min.truncate(), bounds.max.truncate()).size();
        let viewport = D3D11_VIEWPORT {
            TopLeftX: bounds.min.x.into(),
            TopLeftY: bounds.max.y.into(),
            Width: size.width.into(),
            Height: size.height.into(),
            MinDepth: bounds.min.z.into(),
            MaxDepth: bounds.max.z.into(),
        };
        Self { viewport }
    }

    pub fn is_empty(&self) -> bool {
        *self == Viewport::EMPTY
    }

    pub fn size2(&self) -> Size2<f32> {
        Size2::new(self.viewport.Width, self.viewport.Height)
    }

    pub fn slice_truncate(viewports: &[Self]) -> &[Self] {
        let len = match viewports.iter().rposition(|vp| !vp.is_empty()) {
            Some(len) => len,
            None => return viewports,
        };
        unsafe { viewports.get_unchecked(..len) }
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
