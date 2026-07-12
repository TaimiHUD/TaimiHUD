use {
    crate::{
        ui::{ImDrawIdx, ImDrawVtx},
        ScreenSpace,
    },
    glam::{Affine2, Mat2},
    glamour::Rect,
    taimi_d3d::{
        dx11::{
            buffer::{IndexBuffer, VertexBuffer},
            depth::{ComparisonFunc, DepthState, D3D11_DEPTH_STENCIL_DESC},
            prelude::*,
            raster::{CullMode, RasterizerState, D3D11_RASTERIZER_DESC},
        },
        state::PrimitiveTopology,
        D3dContextBindable,
        D3dContextBindableSlot,
    },
};

#[derive(Debug, Clone, Default)]
pub struct ImDrawFrame {
    buffers: Vec<ImDrawFrameBuffer>,
    pub transform: Affine2,
    cap_b: usize,
}
impl ImDrawFrame {
    pub fn resize(&mut self, viewport: Rect<ScreenSpace>) {
        let mut size = viewport.size.to_raw();
        // coordinate fun...
        size.y = -size.y;
        let scale = 2.0f32 / size;
        let m = Mat2::from_diagonal(scale);
        #[cfg(todo = "unnecessary")]
        let translation = -(viewport.max().to_raw() + size) / size;
        let mut translation = viewport.origin.to_raw() * -scale - 1.0f32;
        translation.y = -translation.y;
        self.transform = Affine2::from_mat2_translation(m, translation);
    }
    pub fn reserve_buffers(&mut self, amt: usize) {
        if self.buffers.len() < amt {
            self.buffers.resize_with(amt, ImDrawFrameBuffer::default);
        }
        self.cap_b = amt;
    }
    pub fn spare_count_b(&self) -> usize {
        self.buffers.len() - self.cap_b
    }
    pub fn spare_count_v(&self) -> u32 {
        self.buffers.iter().map(|b| b.spare_count_v()).sum()
    }
    pub fn spare_count_i(&self) -> u32 {
        self.buffers.iter().map(|b| b.spare_count_i()).sum()
    }
    pub fn buffers(&self) -> &[ImDrawFrameBuffer] {
        unsafe { self.buffers[..].get_unchecked(..self.cap_b) }
    }
    pub fn buffers_mut(&mut self) -> &mut [ImDrawFrameBuffer] {
        unsafe { self.buffers[..].get_unchecked_mut(..self.cap_b) }
    }
    pub fn is_empty(&self) -> bool {
        self.buffers().iter().all(|b| b.cap_i == 0)
    }
    pub const DEPTH_DESC_OFF: D3D11_DEPTH_STENCIL_DESC = D3D11_DEPTH_STENCIL_DESC {
        DepthEnable: BOOL(0),
        DepthWriteMask: d3d11::D3D11_DEPTH_WRITE_MASK_ZERO,
        DepthFunc: ComparisonFunc::ALWAYS,
        ..DepthState::DESC_DEFAULT
    };
    pub const DESC_RASTER: D3D11_RASTERIZER_DESC = D3D11_RASTERIZER_DESC {
        CullMode: CullMode::NONE,
        DepthClipEnable: BOOL(0),
        ScissorEnable: BOOL(1),
        ..RasterizerState::DESC_DEFAULT
    };
    /// irrelevant since stencil disabled but may as well match imgui's sample renderer...
    pub const STENCIL_REF_OFF: u32 = 0u32;

    pub const PRIMITIVE: PrimitiveTopology = PrimitiveTopology::TriangleList;
}

#[derive(Debug, Clone, Default)]
pub struct ImDrawFrameBuffer {
    vertices: Option<VertexBuffer>,
    indices: Option<IndexBuffer>,
    cap_v: usize,
    cap_i: usize,
}
impl ImDrawFrameBuffer {
    pub fn reserve_space(&mut self, device: &Dx11Device, len_v: usize, len_i: usize) -> anyhow::Result<()> {
        use anyhow::Context;
        if len_v > self.cap_v {
            self.vertices = Some(
                VertexBuffer::new::<ImDrawVtx>(device, Some(len_v), Default::default()).context("VB")?,
            );
            self.cap_v = len_v;
        } else if let Some(buf) = &mut self.vertices {
            unsafe {
                buf.set_count(len_i as _);
            }
        }
        if len_i > self.cap_i {
            self.indices =
                Some(IndexBuffer::new::<ImDrawIdx>(device, len_i, Default::default()).context("VB")?);
            self.cap_i = len_i;
        } else if let Some(buf) = &mut self.indices {
            unsafe {
                buf.set_count(len_i as _);
            }
        }

        Ok(())
    }
    pub fn is_ready(&self) -> bool {
        self.cap_i > 0 && self.cap_v > 0
    }
    pub unsafe fn update_at_v_unchecked(&self, ctx: &Dx11Context, v: &[ImDrawVtx], offset: usize) {
        let buf = self.vertices.as_ref().unwrap_unchecked();
        match v.len() == self.cap_v {
            #[cfg(todo = "unnecessary")]
            true => buf.update_all_unchecked(ctx, v, offset, 0),
            _ => buf.update_at(ctx, v, offset, 0),
        };
    }
    pub unsafe fn update_at_i_unchecked(&self, ctx: &Dx11Context, i: &[ImDrawIdx], offset: usize) {
        let buf = self.indices.as_ref().unwrap_unchecked();
        match i.len() == self.cap_i {
            #[cfg(todo = "unnecessary")]
            true => buf.update_all_unchecked(ctx, i, offset, 0),
            _ => buf.update_at(ctx, i, offset, 0),
        };
    }
    pub fn count_v(&self) -> u32 {
        self.vertices.as_ref().map(|v| v.vertex_count()).unwrap_or(0)
    }
    pub fn count_i(&self) -> u32 {
        self.indices.as_ref().map(|v| v.index_count()).unwrap_or(0)
    }
    pub fn spare_count_v(&self) -> u32 {
        self.cap_v as u32 - self.count_v()
    }
    pub fn spare_count_i(&self) -> u32 {
        self.cap_i as u32 - self.count_i()
    }
    pub fn clear_v(&mut self) {
        self.cap_v = 0;
        self.vertices = None;
    }
    pub fn clear_i(&mut self) {
        self.cap_i = 0;
        self.indices = None;
    }
    pub fn clear(&mut self) {
        self.clear_v();
        self.clear_i();
    }
}
impl<C> D3dContextBindableSlot<C> for ImDrawFrameBuffer
where
    VertexBuffer: D3dContextBindableSlot<C>,
    IndexBuffer: D3dContextBindable<C>,
{
    fn set(&self, c: &C, slot: u32) {
        if let Some(v) = &self.vertices {
            v.set(c, slot);
        }
        if let Some(i) = &self.indices {
            i.set(c);
        }
    }
}
