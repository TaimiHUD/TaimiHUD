use {
    super::{sys, ImRendererBase},
    crate::{
        dx11::{ImDrawFrame as ImDrawFrame11, RenderBackend11},
        ScreenSpace,
    },
    arcffi::cstr::{cstr, Str0},
    core::{mem, ops, ptr::NonNull, slice},
    glamour::Rect,
    taimi_d3d::{
        dx11::{
            buffer::{BindFlags, BufferOf, TextureView2, Usage, D3D11_TEXTURE2D_DESC},
            DepthState,
            OMDepthState,
            RasterizerState,
            RenderTargetView,
            ScissorRect,
        },
        dxgi,
        state::PrimitiveTopology,
        D3dContextBindable,
        D3dContextBindableSlot,
    },
    taimi_ui::im::{io::ImDisplayDims, ImPtr},
    windows::{core::PCWSTR, Win32::UI::WindowsAndMessaging as wm},
};

type ImInstanceData = glam::Mat4;

pub struct ImRenderer11<'io> {
    pub backend: RenderBackend11,
    pub base: ImRendererBase<'io>,
    pub rt: Option<RenderTargetView>,
    pub draw: ImDrawFrame11,
    pub ib: Option<BufferOf<ImInstanceData>>,
    pub ib_dirty: bool,
    pub raster: Option<RasterizerState>,
    pub depth: Option<OMDepthState>,
    #[cfg(all(todo, imgui192))]
    pub viewport: ImPtr<'io, sys::ImGuiViewport>,
}
impl<'io> ImRenderer11<'io> {
    pub const RENDERER_NAME: &'static Str0 = cstr!(0"taimi_cover_ui_dx11");

    pub fn with_parts(backend: RenderBackend11, base: ImRendererBase<'io>) -> Self {
        Self {
            backend,
            base,
            rt: None,
            ib: None,
            ib_dirty: true,
            raster: None,
            depth: None,
            draw: ImDrawFrame11::default(),
        }
    }
    pub unsafe fn new180_unchecked(backend: RenderBackend11, io: NonNull<sys::ImGuiIO>) -> Self {
        let base = ImRendererBase::new180_unchecked(io);
        Self::with_parts(backend, base)
    }
    pub unsafe fn register(&mut self) -> anyhow::Result<()> {
        {
            let io = self.base.io_mut();
            io.BackendRendererName = Self::RENDERER_NAME.as_ptr() as *const _;
            io.BackendFlags |= sys::ImGuiBackendFlags_RendererHasVtxOffset as sys::ImGuiBackendFlags;
        }
        self.base.register_base()?;
        Ok(())
    }

    pub fn resize(&mut self, viewport: Rect<ScreenSpace>) {
        self.rt = None;
        self.ib_dirty |= self.ib.is_some();
        #[cfg(todo = "unnecessary")]
        self.draw.resize(viewport);
        let vp = &mut self.backend.viewport.viewport;
        vp.Width = viewport.size.width;
        vp.Height = viewport.size.height;
        vp.TopLeftX = viewport.origin.x;
        vp.TopLeftY = viewport.origin.y;
    }

    /// initialize depth and rasterizer state
    ///
    /// TODO: if rasterizer state disables depth testing,
    /// can we ignore depth state entirely? and we never bind a depthstencil buffer...
    pub fn setup_raster_state(&mut self) -> anyhow::Result<()> {
        let depth = DepthState::new_with_desc(&self.backend.device, &ImDrawFrame11::DEPTH_DESC_OFF)?;
        self.raster = Some(RasterizerState::new_with_desc(
            &self.backend.device,
            &ImDrawFrame11::DESC_RASTER,
        )?);
        self.depth = Some(OMDepthState::with_state(depth, ImDrawFrame11::STENCIL_REF_OFF));
        Ok(())
    }
    pub fn setup_frame(&mut self) -> anyhow::Result<()> {
        self.backend.setup_frame()?;
        if self.rt.is_none() {
            self.rt = Some(self.backend.new_surface()?);
        }
        Ok(())
    }
    pub fn setup_draw(&mut self, draw_data: &ImPtr<sys::ImDrawData>) -> anyhow::Result<()> {
        if !draw_data.Valid {
            anyhow::bail!("draw data invalid")
        }
        #[cfg(all(todo, imgui192))]
        if let Some(textures) = &draw_data.Textures {
            // texture uploads whee
        }
        self.draw.resize(Rect::new(
            draw_data.display_pos().cast(),
            draw_data.display_size().cast(),
        ));
        let draws = draw_data.draw_lists();
        self.setup_draw_for(draws.into_iter().map(|&dl| dl))
    }
    pub fn setup_draw_for<'l>(
        &mut self,
        draw_data: impl Iterator<Item = &'l ImPtr<'l, sys::ImDrawList>>,
    ) -> anyhow::Result<()> {
        let amt = match draw_data.size_hint() {
            (_, Some(max)) => max,
            (min, ..) => min,
        };
        self.draw.reserve_buffers(amt);
        let mut buffers = self.draw.buffers_mut().into_iter();
        for dl in draw_data {
            let buffer = match buffers.next() {
                #[cfg(debug_assertions)]
                b => b.expect("reserved"),
                #[cfg(all(todo, not(debug_assertions)))]
                b => unsafe { b.unwrap_unchecked() },
                #[cfg(not(debug_assertions))]
                Some(b) => b,
                #[cfg(not(debug_assertions))]
                None => continue,
            };
            let v = dl.vtx().vtx_slice();
            let i = dl.idx().idx_slice();
            buffer.reserve_space(&self.backend.device, v.len(), i.len())?;
            if let Some(context) = &self.backend.context {
                unsafe {
                    buffer.update_at_i_unchecked(context, i, 0);
                    let v = mem::transmute::<&[_], &[_]>(v);
                    buffer.update_at_v_unchecked(context, v, 0);
                }
            }
            // TODO: prepare any fonts/textures here too
        }
        let ib_dirty = !self.draw.is_empty() && (self.ib.is_none() | self.ib_dirty);
        if ib_dirty {
            let m = glam::Mat4::from({
                let mut m3 = glam::Mat3::from_mat2(self.draw.transform.matrix2);
                // 1 to -1
                m3.z_axis.z = 0.5f32;
                let trans = self.draw.transform.translation.extend(0.5f32);
                glam::Affine3A::from_mat3_translation(m3, trans)
            });
            match &mut self.ib {
                Some(ib) =>
                    if let Some(cx) = &self.backend.context {
                        ib.update_singleton(cx, &m);
                        self.ib_dirty = false;
                    },
                ib @ &mut None => {
                    *ib = Some(BufferOf::new_with_data(
                        &self.backend.device,
                        Ok(slice::from_ref(&m)),
                        (),
                    )?);
                    self.ib_dirty = false;
                },
            }
        }
        Ok(())
    }
    pub fn draw(&mut self, draw_data: &ImPtr<sys::ImDrawData>) {
        let draws = draw_data.draw_lists();
        self.draw_for(draws.into_iter().map(|&dl| dl), draw_data)
    }
    pub fn draw_for<'l>(
        &mut self,
        draw_data: impl IntoIterator<Item = &'l ImPtr<'l, sys::ImDrawList>>,
        dims: &impl ImDisplayDims,
    ) {
        let mut ibound = false;
        let mut clipped = None;
        let draw_data = draw_data.into_iter().zip(self.draw.buffers());
        for (dl, db) in draw_data {
            let mut bound = false;
            for cmd in dl.cmd_buffers() {
                let raw = cmd.cmd();
                if let Some(()) = cmd.try_call_user() {
                    continue
                } else if raw.idx_count() == 0 {
                    continue
                }
                let Some(context) = &self.backend.context else { continue };
                if !bound {
                    if !ibound {
                        if let Some(ib) = &self.ib {
                            ib.set(context, 1);
                        }
                        if let Some(depth) = &self.depth {
                            depth.set(context);
                        }
                        if let Some(raster) = &self.raster {
                            raster.set(context);
                        }
                        ibound = true;
                    }
                    db.set(context, 0);
                    bound = true;
                }
                let tex = NonNull::new(raw.TextureId);
                let tex = tex.as_ref().map(|t| unsafe { TextureView2::from_d3d_raw_ref(t) });
                if let Some(tex) = tex {
                    tex.set(context, 0);
                } else {
                    // TODO: unbind texture? bind a fallback? use different shader?
                    #[cfg(debug_assertions)]
                    log::debug!("draw texture empty");
                }
                let clip = raw.clip_rect();
                let need_clip = matches!(clipped.as_ref().map(|c| c == &clip), Some(false) | None);
                if need_clip {
                    clipped = Some(clip);
                    let offset = dims.display_pos();
                    let mut rect = clip;
                    rect.min -= offset.to_vector();
                    rect.max -= offset.to_vector();
                    ScissorRect::with_bounds(rect).set(context);
                }
                unsafe {
                    context.DrawIndexedInstanced(raw.ElemCount, 1, raw.IdxOffset, raw.VtxOffset as _, 0);
                    //context.DrawIndexed(raw.ElemCount, raw.IdxOffset, raw.VtxOffset as _);
                }
            }
        }
    }

    pub const PRIMITIVE: PrimitiveTopology = ImDrawFrame11::PRIMITIVE;
    pub const FONT_DESC_RGBA8: D3D11_TEXTURE2D_DESC = D3D11_TEXTURE2D_DESC {
        Width: 0u32,
        Height: 0u32,
        MipLevels: 1u32,
        ArraySize: 1u32,
        SampleDesc: dxgi::DXGI_SAMPLE_DESC { Count: 1u32, Quality: 0u32 },
        Usage: Usage::DEFAULT.to_d3d(),
        BindFlags: BindFlags::SHADER.to_raw(),
        CPUAccessFlags: 0u32,
        MiscFlags: 0u32,
        Format: dxgi::DXGI_FORMAT_R8G8B8A8_UNORM,
    };
    pub const FONT_DESC_A8: D3D11_TEXTURE2D_DESC = D3D11_TEXTURE2D_DESC {
        Format: dxgi::DXGI_FORMAT_A8_UNORM,
        ..Self::FONT_DESC_RGBA8
    };
}
impl<'io> ops::Deref for ImRenderer11<'io> {
    type Target = ImRendererBase<'io>;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}
impl ops::DerefMut for ImRenderer11<'_> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

pub fn windows_cursor_name_w(cursor: sys::ImGuiMouseCursor_) -> Option<PCWSTR> {
    Some(match cursor {
        sys::ImGuiMouseCursor_Arrow => wm::IDC_ARROW,
        sys::ImGuiMouseCursor_TextInput => wm::IDC_IBEAM,
        sys::ImGuiMouseCursor_Hand => wm::IDC_HAND,
        sys::ImGuiMouseCursor_NotAllowed => wm::IDC_NO,
        sys::ImGuiMouseCursor_ResizeEW => wm::IDC_SIZEWE,
        sys::ImGuiMouseCursor_ResizeNS => wm::IDC_SIZENS,
        sys::ImGuiMouseCursor_ResizeNESW => wm::IDC_SIZENESW,
        sys::ImGuiMouseCursor_ResizeNWSE => wm::IDC_SIZENWSE,
        sys::ImGuiMouseCursor_ResizeAll => wm::IDC_SIZEALL,
        sys::ImGuiMouseCursor_None | _ => return None,
    })
}
