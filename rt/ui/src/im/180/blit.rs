use {
    super::{prelude::*, sys},
    core::mem,
};

pub struct DrawListAtMut<'a, 'p> {
    draw_list: DrawListAt<'a, 'p>,
}
impl ImSurfaceTarget for DrawListAtMut<'_, '_> {
    #[inline]
    fn clip_rect_min(&self) -> ImPos2<ImSpace> {
        self.draw_list.clip_rect_min()
    }
    #[inline]
    fn clip_rect_max(&self) -> ImPos2<ImSpace> {
        self.draw_list.clip_rect_max()
    }
}
impl ImBlitBatch for DrawListAtMut<'_, '_> {
    #[inline]
    fn bound_texture_dyn(&self) -> Option<&dyn ImTexture> {
        self.draw_list.bound_texture_dyn()
    }
    fn buffer_vertex_dyn(&self) -> Option<&dyn ImBufferBlob> {
        self.draw_list.buffer_vertex_dyn()
    }
    fn buffer_index_dyn(&self) -> Option<&dyn ImBufferBlob> {
        self.draw_list.buffer_index_dyn()
    }
}
#[inline(never)]
unsafe extern "C" fn user_cb_nop(_parent: *const sys::ImDrawList, _: *const sys::ImDrawCmd) {}
impl ImBlitBatchMut for DrawListAtMut<'_, '_> {
    /// TODO: clear ElemCount=0 or no-op UserCallback?
    fn discard_batch(&mut self) {
        let draw_cmd = self.cmd_mut();
        let draw_cmd = unsafe { draw_cmd.as_raw_mut() };
        match draw_cmd {
            draw_cmd if draw_cmd.UserCallback.is_some() => {
                draw_cmd.ElemCount = 0;
            },
            draw_cmd => {
                draw_cmd.UserCallback = Some(user_cb_nop);
            },
        }
    }
}
impl<'a, 'p> DrawListAtMut<'a, 'p> {
    #[inline]
    pub fn cmd_mut(&mut self) -> &mut ImPtr<'p, sys::ImDrawCmd> {
        unsafe { self.draw_list.cmd_mut_unchecked() }
    }
    #[inline]
    pub unsafe fn indices_mut(&mut self) -> &mut [sys::ImDrawIdx] {
        self.draw_list.indices_mut_unchecked()
    }
    #[inline]
    pub fn vertices_mut(&mut self) -> &mut [sys::ImDrawVert] {
        unsafe { self.draw_list.vertices_mut_unchecked() }
    }
}
impl<'a, 'p> ops::Deref for DrawListAtMut<'a, 'p> {
    type Target = DrawListAt<'a, 'p>;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.draw_list
    }
}

pub struct DrawListAt<'a, 'p> {
    draw_list: &'a ImPtr<'p, sys::ImDrawList>,
    draw_cmd: &'a ImPtr<'p, sys::ImDrawCmd>,
}
impl<'a, 'p> DrawListAt<'a, 'p> {
    #[inline]
    pub fn is_callback(&self) -> bool {
        self.draw_cmd.UserCallback.is_some()
    }
    #[inline]
    pub fn list(&self) -> &ImPtr<'p, sys::ImDrawList> {
        self.draw_list
    }
    #[inline]
    pub fn cmd(&self) -> &ImPtr<'p, sys::ImDrawCmd> {
        self.draw_cmd
    }
    #[inline]
    pub unsafe fn cmd_mut_unchecked(&mut self) -> &mut ImPtr<'p, sys::ImDrawCmd> {
        ImPtr::from_mut(self.draw_cmd.as_raw_mut_unchecked())
    }
    pub fn vertices(&self) -> &[sys::ImDrawVert] {
        let start = self.draw_cmd.VtxOffset as usize;
        let end = start + self.draw_cmd.ElemCount as usize;
        self.draw_list.vtx().vtx_slice().get(start..end).unwrap_or(&[])
    }
    pub unsafe fn vertices_mut_unchecked(&mut self) -> &mut [sys::ImDrawVert] {
        let start = self.draw_cmd.VtxOffset as usize;
        let end = start + self.draw_cmd.ElemCount as usize;
        unsafe {
            let draw_list = ImPtr::from_mut(self.draw_list.as_raw_mut_unchecked());
            draw_list
                .vtx_mut()
                .vtx_slice_mut()
                .get_mut(start..end)
                .unwrap_or(&mut [])
        }
    }
    pub fn indices(&self) -> &[sys::ImDrawIdx] {
        let start = self.draw_cmd.IdxOffset as usize;
        let end = start + self.draw_cmd.ElemCount as usize;
        self.draw_list.idx().idx_slice().get(start..end).unwrap_or(&[])
    }
    pub unsafe fn indices_mut_unchecked(&mut self) -> &mut [sys::ImDrawIdx] {
        let start = self.draw_cmd.IdxOffset as usize;
        let end = start + self.draw_cmd.ElemCount as usize;
        unsafe {
            let draw_list = ImPtr::from_mut(self.draw_list.as_raw_mut_unchecked());
            draw_list
                .idx_mut()
                .idx_slice_mut()
                .get_mut(start..end)
                .unwrap_or(&mut [])
        }
    }
}
impl ImBlitBatch for DrawListAt<'_, '_> {
    #[inline]
    fn bound_texture_dyn(&self) -> Option<&dyn ImTexture> {
        self.draw_list.bound_texture_dyn()
    }
    fn buffer_vertex_dyn(&self) -> Option<&dyn ImBufferBlob> {
        if self.is_callback() {
            return None
        }
        Some(match self {
            #[cfg(todo)]
            dl => DrawListAtVertices::from_ref(fl),
            dl => unsafe { ImPtr::from_ref(&dl.draw_list.VtxBuffer) },
        })
    }
    fn buffer_index_dyn(&self) -> Option<&dyn ImBufferBlob> {
        if self.is_callback() {
            return None
        }
        Some(DrawListAtIndices::from_ref(self))
    }
}
impl ImSurfaceTarget for DrawListAt<'_, '_> {
    #[inline]
    fn clip_rect_min(&self) -> ImPos2<ImSpace> {
        self.draw_cmd.clip_rect_min()
    }
    #[inline]
    fn clip_rect_max(&self) -> ImPos2<ImSpace> {
        self.draw_cmd.clip_rect_max()
    }
}

#[repr(transparent)]
pub struct DrawListAtIndices<'d, 'p>(pub DrawListAt<'d, 'p>);
impl<'d, 'p> DrawListAtIndices<'d, 'p> {
    #[inline]
    pub const fn from_ref<'a>(draw_list: &'a DrawListAt<'d, 'p>) -> &'a Self {
        unsafe { mem::transmute(draw_list) }
    }
}
impl ImBufferBlobInfo for DrawListAtIndices<'_, '_> {
    #[inline]
    fn elem_stride(&self) -> usize {
        mem::size_of::<sys::ImDrawIdx>()
    }
    #[inline]
    fn blob_count(&self) -> usize {
        self.0.draw_cmd.ElemCount as usize
    }
}
impl ImBufferBlob for DrawListAtIndices<'_, '_> {
    fn blob_ptr(&self) -> NonNull<()> {
        let off = self.0.draw_cmd.VtxOffset;
        let ptr = NonNull::new(self.0.draw_list.IdxBuffer.get_ptr());
        ptr.map(|p| unsafe { p.add(off as usize) })
            .unwrap_or(NonNull::dangling())
            .cast()
    }
    #[inline]
    unsafe fn elem_read_unchecked(&self, dest: NonNull<()>, offset: usize, count: usize) {
        self.blob_read_unchecked(dest, offset, count)
    }
}
impl ImPtr<'_, sys::ImVector_ImDrawVert> {
    pub fn vtx_slice(&self) -> &[sys::ImDrawVert] {
        unsafe { self.data() }
    }
    pub fn vtx_slice_mut(&mut self) -> &mut [sys::ImDrawVert] {
        unsafe { self.as_raw_mut().data_mut() }
    }
}
impl ImBufferBlobInfo for ImPtr<'_, sys::ImVector_ImDrawVert> {
    #[inline]
    fn elem_stride(&self) -> usize {
        mem::size_of::<sys::ImDrawVert>()
    }
    #[inline]
    fn blob_count(&self) -> usize {
        self.as_raw().len()
    }
}
impl ImBufferBlob for ImPtr<'_, sys::ImVector_ImDrawVert> {
    #[inline]
    fn blob_ptr(&self) -> NonNull<()> {
        NonNull::new(self.as_raw().get_ptr())
            .unwrap_or(NonNull::dangling())
            .cast()
    }
    #[inline]
    unsafe fn elem_read_unchecked(&self, dest: NonNull<()>, offset: usize, count: usize) {
        self.blob_read_unchecked(dest, offset, count)
    }
}

impl ImPtr<'_, sys::ImVector_ImDrawIdx> {
    pub fn idx_slice(&self) -> &[sys::ImDrawIdx] {
        unsafe { self.data() }
    }
    pub unsafe fn idx_slice_mut(&mut self) -> &mut [sys::ImDrawIdx] {
        self.as_raw_mut().data_mut()
    }
}
impl ImBufferBlobInfo for ImPtr<'_, sys::ImVector_ImDrawIdx> {
    #[inline]
    fn elem_stride(&self) -> usize {
        mem::size_of::<sys::ImDrawIdx>()
    }
    #[inline]
    fn blob_count(&self) -> usize {
        self.as_raw().len()
    }
}
impl ImBufferBlob for ImPtr<'_, sys::ImVector_ImDrawIdx> {
    #[inline]
    fn blob_ptr(&self) -> NonNull<()> {
        NonNull::new(self.as_raw().get_ptr())
            .unwrap_or(NonNull::dangling())
            .cast()
    }
    #[inline]
    unsafe fn elem_read_unchecked(&self, dest: NonNull<()>, offset: usize, count: usize) {
        self.blob_read_unchecked(dest, offset, count)
    }
}

#[repr(transparent)]
#[cfg(todo)]
pub struct DrawListAtVertices<'a>(pub DrawListAt<'a>);

impl<'a> ImSurfaceTarget for ImPtr<'a, sys::ImDrawCmd> {
    #[inline]
    fn clip_rect_min(&self) -> ImPos2<ImSpace> {
        ImPos2::new(self.ClipRect.z, self.ClipRect.w)
    }
    #[inline]
    fn clip_rect_max(&self) -> ImPos2<ImSpace> {
        ImPos2::new(self.ClipRect.x, self.ClipRect.y)
    }
}

impl<'a> ImSurfaceTarget for ImPtr<'a, sys::ImDrawList> {
    fn clip_rect_max(&self) -> ImPos2<ImSpace> {
        // technically could fallback to viewport dims?
        let fallback = ImPos2::ZERO;
        unsafe {
            let head = self.CmdBuffer.data().last();
            head.map(|c| ImPtr::from_ref(c))
                .map(ImSurfaceTarget::clip_rect_max)
                .unwrap_or(fallback)
        }
    }
    fn clip_rect_min(&self) -> ImPos2<ImSpace> {
        unsafe {
            let head = self.CmdBuffer.data().last();
            head.map(|c| ImPtr::from_ref(c))
                .map(ImSurfaceTarget::clip_rect_min)
                .unwrap_or(ImPos2::ZERO)
        }
    }
}
impl<'l> ImPtr<'l, sys::ImDrawList> {
    pub fn cmd_buffers_mut<'a>(&'a mut self) -> impl Iterator<Item = DrawListAtMut<'a, 'l>> {
        let cmds = unsafe { self.CmdBuffer.data() };
        cmds.iter().map(|cmd| DrawListAtMut {
            draw_list: DrawListAt {
                draw_list: &*self,
                draw_cmd: unsafe { ImPtr::from_ref(cmd) },
            },
        })
    }
    #[inline]
    pub fn vtx(&self) -> &ImPtr<'l, sys::ImVector_ImDrawVert> {
        unsafe { ImPtr::from_ref(&self.VtxBuffer) }
    }
    #[inline]
    pub fn vtx_mut(&mut self) -> &mut ImPtr<'l, sys::ImVector_ImDrawVert> {
        unsafe { ImPtr::from_mut(&mut self.as_raw_mut().VtxBuffer) }
    }
    #[inline]
    pub fn idx(&self) -> &ImPtr<'l, sys::ImVector_ImDrawIdx> {
        unsafe { ImPtr::from_ref(&self.IdxBuffer) }
    }
    #[inline]
    pub fn idx_mut(&mut self) -> &mut ImPtr<'l, sys::ImVector_ImDrawIdx> {
        unsafe { ImPtr::from_mut(&mut self.as_raw_mut().IdxBuffer) }
    }
}
const TEX_ZERO: sys::ImTextureID = ptr::null_mut();
impl<'a> ImBlitBatch for ImPtr<'a, sys::ImDrawList> {
    fn bound_texture_dyn(&self) -> Option<&dyn ImTexture> {
        let cmd = unsafe { self.CmdBuffer.data() }.last()?;
        match cmd.TextureId {
            self::TEX_ZERO => None,
            #[cfg(not(feature = "imgui180-rs"))]
            ref id => Some(TextureId::from_sys_ref(id) as &dyn ImTexture),
            #[cfg(feature = "imgui180-rs")]
            _ => {
                log::warn!("ImDrawList::bound_texture_dyn unimplemented");
                None
            },
        }
    }
    #[inline]
    fn buffer_vertex_dyn(&self) -> Option<&dyn ImBufferBlob> {
        Some(self.vtx() as &dyn ImBufferBlob)
    }
    #[inline]
    fn buffer_index_dyn(&self) -> Option<&dyn ImBufferBlob> {
        Some(self.idx() as &dyn ImBufferBlob)
    }
}
