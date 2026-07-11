use {super::ImBufferBlob, crate::im::prelude::*};

pub trait ImSurfaceTarget {
    fn clip_rect_min(&self) -> ImPos2<ImSpace>;
    fn clip_rect_max(&self) -> ImPos2<ImSpace>;
}
#[doc(alias = "ImDrawCmd")]
pub trait ImBlitBatch: ImSurfaceTarget {
    fn bound_texture_dyn(&self) -> Option<&dyn ImTexture>;
    fn buffer_vertex_dyn(&self) -> Option<&dyn ImBufferBlob>;
    fn buffer_index_dyn(&self) -> Option<&dyn ImBufferBlob>;
}
#[doc(alias = "ImDrawCmd")]
pub trait ImBlitBatchMut: ImBlitBatch {
    fn discard_batch(&mut self);
}
/// see also: [ImSurfaceTarget]
#[doc(alias = "ImDrawList")]
pub trait ImDrawTarget {
    fn add_line(
        &mut self,
        p0: ImPos2<ImSpace>,
        p1: ImPos2<ImSpace>,
        colour: ImColour,
        thickness: Option<f32>,
    );
    fn add_rect_untyped(
        &mut self,
        rect: Box2<ImSpace>,
        colour: ImColour,
        rounding: Option<f32>,
        filled_thickness: Option<Option<f32>>,
        flags_untyped: Option<u32>,
    );
    fn add_quad(
        &mut self,
        points: [ImPos2<ImSpace>; 4],
        colour: ImColour,
        filled_thickness: Option<Option<f32>>,
    );
    fn add_triangle(
        &mut self,
        points: [ImPos2<ImSpace>; 3],
        colour: ImColour,
        filled_thickness: Option<Option<f32>>,
    );
    fn add_circle(
        &mut self,
        mid: ImPos2<ImSpace>,
        radius: f32,
        colour: ImColour,
        segments: Option<u32>,
        filled_thickness: Option<Option<f32>>,
    );
    fn add_ngon(
        &mut self,
        mid: ImPos2<ImSpace>,
        radius: f32,
        colour: ImColour,
        segments: Option<u32>,
        filled_thickness: Option<Option<f32>>,
    );
    fn add_ellipse(
        &mut self,
        mid: ImPos2<ImSpace>,
        radius: ImVec2<ImSpace>,
        colour: ImColour,
        rot: Option<f32>,
        segments: Option<u32>,
        filled_thickness: Option<Option<f32>>,
    );

    #[cfg(todo)]
    fn add_text();
    #[cfg(todo)]
    fn add_bezier_cubic();
    #[cfg(todo)]
    fn add_bezier_quadratic();
}
pub trait ImDrawTargetExt: ImSurfaceTarget {
    fn clip_rect(&self) -> Box2<ImSpace> {
        Box2::new(self.clip_rect_min(), self.clip_rect_max())
    }
}
impl<T> ImDrawTargetExt for T where T: ?Sized + ImSurfaceTarget {}

impl ImSurfaceTarget for () {
    #[inline(always)]
    fn clip_rect_min(&self) -> ImPos2<ImSpace> {
        ImPos2::ZERO
    }
    #[inline(always)]
    fn clip_rect_max(&self) -> ImPos2<ImSpace> {
        ImPos2::ZERO
    }
}
#[allow(unused)]
impl ImDrawTarget for () {
    #[inline(always)]
    fn add_line(
        &mut self,
        p0: ImPos2<ImSpace>,
        p1: ImPos2<ImSpace>,
        colour: ImColour,
        thickness: Option<f32>,
    ) {
    }
    #[inline(always)]
    fn add_rect_untyped(
        &mut self,
        rect: Box2<ImSpace>,
        colour: ImColour,
        rounding: Option<f32>,
        filled_thickness: Option<Option<f32>>,
        flags_untyped: Option<u32>,
    ) {
    }
    #[inline(always)]
    fn add_quad(
        &mut self,
        points: [ImPos2<ImSpace>; 4],
        colour: ImColour,
        filled_thickness: Option<Option<f32>>,
    ) {
    }
    #[inline(always)]
    fn add_triangle(
        &mut self,
        points: [ImPos2<ImSpace>; 3],
        colour: ImColour,
        filled_thickness: Option<Option<f32>>,
    ) {
    }
    #[inline(always)]
    fn add_circle(
        &mut self,
        mid: ImPos2<ImSpace>,
        radius: f32,
        colour: ImColour,
        segments: Option<u32>,
        filled_thickness: Option<Option<f32>>,
    ) {
    }
    #[inline(always)]
    fn add_ngon(
        &mut self,
        mid: ImPos2<ImSpace>,
        radius: f32,
        colour: ImColour,
        segments: Option<u32>,
        filled_thickness: Option<Option<f32>>,
    ) {
    }
    #[inline(always)]
    fn add_ellipse(
        &mut self,
        mid: ImPos2<ImSpace>,
        radius: ImVec2<ImSpace>,
        colour: ImColour,
        rot: Option<f32>,
        segments: Option<u32>,
        filled_thickness: Option<Option<f32>>,
    ) {
    }
}
