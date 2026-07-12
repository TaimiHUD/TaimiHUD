pub use ::taimi_ui::im::im180::{self as imui, sys};
use {
    arcffi::cstr::{cstr, Str0},
    core::{cell::UnsafeCell, marker::PhantomData, ptr::NonNull},
};

#[cfg(feature = "windows")]
pub use self::ImRenderer11 as ImRenderer;
#[cfg(not(feature = "windows"))]
pub use self::ImRendererBase as ImRenderer;

#[cfg(feature = "windows")]
pub mod win;
#[cfg(feature = "windows")]
pub use self::win::{windows_cursor_name_w, ImRenderer11};

pub struct ImRendererBase<'i> {
    pub io: NonNull<()>,
    pub _io: PhantomData<&'i sys::ImGuiIO>,
}

impl<'i> ImRendererBase<'i> {
    pub const RENDERER_NAME: &'static Str0 = cstr!(0"taimi_cover_ui");
    #[inline(always)]
    pub const unsafe fn with_parts(io: NonNull<()>) -> Self {
        Self { io, _io: PhantomData }
    }
    #[inline(always)]
    pub fn io_cell(&self) -> &'i UnsafeCell<sys::ImGuiIO> {
        let p = self.io.cast::<sys::ImGuiIO>().as_ptr() as *const sys::ImGuiIO as *const UnsafeCell<_>;
        unsafe { &*p }
    }
    #[inline(always)]
    pub fn io(&self) -> &sys::ImGuiIO {
        unsafe { &*self.io_cell().get() }
    }
    #[inline(always)]
    pub unsafe fn io_mut(&self) -> &mut sys::ImGuiIO {
        unsafe { &mut *self.io_cell().get() }
    }
    pub unsafe fn new180_unchecked(io: NonNull<sys::ImGuiIO>) -> Self {
        Self::with_parts(io.cast())
    }
    pub unsafe fn register(&mut self) -> anyhow::Result<()> {
        self.io_mut().BackendRendererName = Self::RENDERER_NAME.as_ptr() as *const _;
        self.register_base()
    }
    /// TODO?
    pub unsafe fn register_base(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
}
