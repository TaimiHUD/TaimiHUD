use {crate::im::prelude::*, core::any::Any, glamour::Box2};

pub trait ImTexture {
    #[cfg(feature = "imgui180")]
    fn im180_texture_id(&self) -> Option<sys180::ImTextureID> {
        None
    }
    #[cfg(feature = "imgui192")]
    fn im192_texture_ref(&self) -> Option<sys192::ImTextureRef> {
        None
    }
    fn as_any(&self) -> Option<&dyn Any> {
        None
    }
    #[cfg(todo)]
    #[inline(always)]
    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        let any = self.as_any().map(nn::nonnull_ref);
        any.map(|a| unsafe { &mut *a.as_ptr() })
    }

    /// vertex colour
    fn tint(&self) -> ImColour {
        ImColour::ONE
    }
    fn uv_bounds(&self) -> Box2<f32> {
        Box2::new(ImPos2::ZERO, ImPos2::ONE)
    }
}
pub trait ImTextureExt: ImTexture {
    #[inline(always)]
    fn tinted<C: Into<ImColour>>(&self, tint: C) -> TintedTexture<&Self> {
        TintedTexture::new(self, tint.into())
    }
}
impl<'a, T: ?Sized + ImTexture> ImTextureExt for T {}
impl<'a, T: ?Sized> ImTexture for &'a T
where
    T: ImTexture,
{
    #[cfg(feature = "imgui180")]
    #[inline(always)]
    fn im180_texture_id(&self) -> Option<sys180::ImTextureID> {
        ImTexture::im180_texture_id(*self)
    }
    #[cfg(feature = "imgui192")]
    #[inline(always)]
    fn im192_texture_ref(&self) -> Option<sys192::ImTextureRef> {
        ImTexture::im192_texture_ref(*self)
    }
    #[inline(always)]
    fn as_any(&self) -> Option<&dyn Any> {
        ImTexture::as_any(*self)
    }
    #[inline(always)]
    fn tint(&self) -> ImColour {
        ImTexture::tint(*self)
    }
    #[inline(always)]
    fn uv_bounds(&self) -> Box2<f32> {
        ImTexture::uv_bounds(*self)
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct TintedTexture<T> {
    pub texture: T,
    pub colour: ImColour,
    pub uv: Box2<f32>,
}
impl<T> TintedTexture<T> {
    pub fn new(texture: T, colour: ImColour) -> Self {
        Self {
            texture,
            colour,
            uv: Box2::new(ImPos2::INFINITY, ImPos2::INFINITY),
        }
    }
}
impl<'a, T> ImTexture for TintedTexture<T>
where
    T: ImTexture,
{
    #[cfg(feature = "imgui180")]
    #[inline(always)]
    fn im180_texture_id(&self) -> Option<sys180::ImTextureID> {
        ImTexture::im180_texture_id(&self.texture)
    }
    #[cfg(feature = "imgui192")]
    #[inline(always)]
    fn im192_texture_ref(&self) -> Option<sys192::ImTextureRef> {
        ImTexture::im192_texture_ref(&self.texture)
    }
    /// &self.texture
    #[inline(always)]
    fn as_any(&self) -> Option<&dyn Any> {
        ImTexture::as_any(&self.texture)
    }

    #[inline(always)]
    fn tint(&self) -> ImColour {
        self.colour
    }
    fn uv_bounds(&self) -> Box2<f32> {
        let mut out = self.uv;
        if !out.as_scalar_array().iter().any(|v| v.is_infinite()) {
            return out
        }
        let tex_uv = self.texture.uv_bounds();
        let coords = out.as_scalar_array_mut().iter_mut().zip(tex_uv.as_scalar_array());
        for (out, tex) in coords {
            if out.is_infinite() {
                *out = *tex;
            }
        }
        out
    }
}
