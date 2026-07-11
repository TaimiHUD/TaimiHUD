#[cfg(taimi_imgui = "180")]
pub use im180::sys::ImDrawVert as ImDrawVert180;
#[cfg(taimi_imgui = "192")]
pub use im192::sys::ImDrawVert as ImDrawVert192;
#[cfg(todo)]
pub use imXXX::sys::ImDrawIdx;
#[cfg(taimi_imgui = "180")]
pub use ImDrawVert180 as ImDrawVert;
#[cfg(all(taimi_imgui = "192", not(taimi_imgui = "180")))]
pub use ImDrawVert192 as ImDrawVert;
use {
    crate::render::element::im::prelude::*,
    core::{ffi::c_void, ptr::NonNull},
    glamour::{Box2, Point2},
    taimi_d3d::dx11::buffer::TextureView2,
};

/// TODO: standardize on u32?
pub type ImDrawIdx = u16;

/// TODO: or just paper over with conversions idk
#[cfg(all(taimi_imgui = "180", taimi_imgui = "192"))]
const ASSERT_VERT_SIZE: () = {
    use core::mem::size_of;
    let ok = size_of::<ImDrawVert180>() == size_of::<ImDrawVert192>();
    match ok {
        true => (),
        false => panic!("IMGUI_OVERRIDE_DRAWVERT_STRUCT_LAYOUT"),
    }
};

#[derive(Debug, Default)]
pub struct DeferredDrawCmd {
    /// XXX: storing TextureView2 would trigger refcount changes mid-frame, but w/e
    texture: Option<NonNull<c_void>>,
    offset_v: u32,
    offset_i: u32,
    pub clip: Box2<ImSpace>,
}
impl DeferredDrawCmd {
    #[inline]
    pub fn texture_view(&self) -> Option<&TextureView2> {
        self.texture
            .as_ref()
            .map(|t| unsafe { TextureView2::from_d3d_raw_ref(t) })
    }
    #[inline]
    pub fn offset_v(&self) -> usize {
        self.offset_v as _
    }
    #[inline]
    pub fn offset_i(&self) -> usize {
        self.offset_i as _
    }
}
#[derive(Debug)]
pub struct DeferredDrawCmds {
    pub draws: Vec<DeferredDrawCmd>,
    vertices: Vec<ImDrawVert>,
    indices: Vec<ImDrawIdx>,
}
impl DeferredDrawCmds {
    #[inline]
    pub fn empty() -> Self {
        #[cfg(all(taimi_imgui = "180", taimi_imgui = "192"))]
        let _ = ASSERT_VERT_SIZE;
        Self {
            draws: Vec::new(),
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    /// TODO: use the dyn traits made for this lol (impl on 192)
    /// TODO: insert at start instead?
    pub fn take_current_window<'ui, U>(&mut self, ui: &mut U)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        match ui.imgui_version_num() {
            #[cfg(taimi_imgui = "180")]
            Some(im180::VERSION_NUM) => unsafe {
                use im180::sys::ImVectorRaw;
                let dl = &mut *im180::sys::igGetWindowDrawList();
                let offset_v = self.vertices.len() as u32;
                let offset_i = self.indices.len() as u32;
                let vtx = &*(dl.VtxBuffer.data() as *const _ as *const [ImDrawVert]);
                self.vertices.extend_from_slice(vtx);
                self.indices.extend_from_slice(dl.IdxBuffer.data());
                self.draws
                    .extend(dl.CmdBuffer.data_mut().iter_mut().filter_map(|cmd| {
                        if cmd.UserCallback.is_some() || cmd.ElemCount == 0 {
                            return None
                        }
                        let draw = DeferredDrawCmd {
                            texture: NonNull::new(cmd.TextureId),
                            offset_v: offset_v + cmd.VtxOffset,
                            offset_i: offset_i + cmd.IdxOffset,
                            clip: Box2::new(
                                Point2::new(cmd.ClipRect.x, cmd.ClipRect.y),
                                Point2::new(cmd.ClipRect.z, cmd.ClipRect.w),
                            ),
                        };
                        cmd.ElemCount = 0;
                        Some(draw)
                    }));
            },
            #[cfg(taimi_imgui = "192")]
            Some(im192::VERSION_NUM) => unsafe {
                use im192::sys::ImVectorRaw;
                let dl = &mut *im192::sys::igGetWindowDrawList();
                let offset_v = self.vertices.len() as u32;
                let offset_i = self.indices.len() as u32;
                let vtx = &*(dl.VtxBuffer.data() as *const _ as *const [ImDrawVert]);
                self.vertices.extend_from_slice(vtx);
                self.indices.extend_from_slice(dl.IdxBuffer.data());
                self.draws
                    .extend(dl.CmdBuffer.data_mut().iter_mut().filter_map(|cmd| {
                        if cmd.UserCallback.is_some() || cmd.ElemCount == 0 {
                            return None
                        }
                        let texture = cmd
                            .TexRef
                            .tex_id()
                            .map(|id| id as usize as *mut c_void)
                            .and_then(NonNull::new);
                        let draw = DeferredDrawCmd {
                            texture,
                            offset_v: offset_v + cmd.VtxOffset,
                            offset_i: offset_i + cmd.IdxOffset,
                            clip: Box2::new(
                                Point2::new(cmd.ClipRect.x, cmd.ClipRect.y),
                                Point2::new(cmd.ClipRect.z, cmd.ClipRect.w),
                            ),
                        };
                        cmd.ElemCount = 0;
                        Some(draw)
                    }));
            },
            _ => {
                log::warn!("unsupported");
            },
        }
    }
}
impl Default for DeferredDrawCmds {
    fn default() -> Self {
        Self::empty()
    }
}
