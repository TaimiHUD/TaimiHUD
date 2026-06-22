use {
    super::prelude::*,
    arcffi::{cstr::CStrPtr, nn},
    core::{
        cell::UnsafeCell,
        ffi::{c_void, CStr},
        mem,
        num::NonZero,
        ptr::{self, NonNull},
    },
};

mod hook;

pub use self::hook::{ContextHookCallback, ContextHookRaw, ImContextHookInfo};

pub trait ImContext {
    const IMGUI_VERSION_NUM: Option<NonZero<u32>>;
    const IMGUI_VERSION_NAME: Option<&'static CStr>;
    type Context: ImContextState;
    type Io: ImIo;
    type Style: ImStyle;
    type PlatformIo: ImPlatformIo;
    type DrawIo: ImDrawIo;
    type DrawList: ImDrawTarget;
    fn get_context_ptr(&self) -> NonNull<Self::Context>;
    fn get_style_ptr(&self) -> NonNull<Self::Style>;
    fn get_draw_ptr(&self) -> NonNull<Self::DrawIo>;
    fn get_draw_fg_ptr(&self) -> NonNull<Self::DrawList>;
    fn get_draw_bg_ptr(&self) -> NonNull<Self::DrawList>;
    fn get_io_ptr(&self) -> NonNull<Self::Io>;
    fn get_pio_ptr(&self) -> NonNull<Self::PlatformIo>;
}
impl<'a, U: ?Sized> ImContext for &'a U
where
    U: ImContext,
{
    const IMGUI_VERSION_NUM: Option<NonZero<u32>> = <U as ImContext>::IMGUI_VERSION_NUM;
    const IMGUI_VERSION_NAME: Option<&'static CStr> = <U as ImContext>::IMGUI_VERSION_NAME;
    type Io = U::Io;
    type Style = U::Style;
    type Context = U::Context;
    type PlatformIo = U::PlatformIo;
    type DrawIo = U::DrawIo;
    type DrawList = U::DrawList;
    #[inline(always)]
    fn get_context_ptr(&self) -> NonNull<Self::Context> {
        ImContext::get_context_ptr(*self)
    }
    #[inline(always)]
    fn get_style_ptr(&self) -> NonNull<Self::Style> {
        ImContext::get_style_ptr(*self)
    }
    #[inline(always)]
    fn get_draw_ptr(&self) -> NonNull<Self::DrawIo> {
        ImContext::get_draw_ptr(*self)
    }
    #[inline(always)]
    fn get_draw_fg_ptr(&self) -> NonNull<Self::DrawList> {
        ImContext::get_draw_fg_ptr(*self)
    }
    #[inline(always)]
    fn get_draw_bg_ptr(&self) -> NonNull<Self::DrawList> {
        ImContext::get_draw_bg_ptr(*self)
    }
    #[inline(always)]
    fn get_io_ptr(&self) -> NonNull<Self::Io> {
        ImContext::get_io_ptr(*self)
    }
    #[inline(always)]
    fn get_pio_ptr(&self) -> NonNull<Self::PlatformIo> {
        ImContext::get_pio_ptr(*self)
    }
}
/// TODO: consider a bulk accessor for most info which can't change mid-frame to cut down on dispatches?
/// TODO: consider whether you really want dyn getters and duplicated fns vs supertraits btw
pub unsafe trait ImUi: ImUiContext {
    fn get_context_ptr_dyn(&self) -> NonNull<dyn ImContextState>;
}
unsafe impl<'a, U: ?Sized> ImUi for &'a U
where
    U: ImUi,
{
    #[inline(always)]
    fn get_context_ptr_dyn(&self) -> NonNull<dyn ImContextState> {
        ImUi::get_context_ptr_dyn(*self)
    }
}
pub unsafe trait ImUiContext {
    fn get_style_ptr_dyn(&self) -> NonNull<dyn ImStyle>;
    fn get_draw_ptr_dyn(&self) -> NonNull<dyn ImDrawIo>;
    fn get_draw_fg_ptr_dyn(&self) -> NonNull<dyn ImDrawTarget>;
    fn get_draw_bg_ptr_dyn(&self) -> NonNull<dyn ImDrawTarget>;
    fn get_io_ptr_dyn(&self) -> NonNull<dyn ImIo>;
    fn get_pio_ptr_dyn(&self) -> NonNull<dyn ImPlatformIo>;
    fn imgui_version_num(&self) -> Option<NonZero<u32>>;
    fn imgui_version_name(&self) -> Option<CStrPtr<'_>>;
}
unsafe impl<'a, U: ?Sized> ImUiContext for &'a U
where
    U: ImUiContext,
{
    #[inline(always)]
    fn get_style_ptr_dyn(&self) -> NonNull<dyn ImStyle> {
        ImUiContext::get_style_ptr_dyn(*self)
    }
    #[inline(always)]
    fn get_draw_ptr_dyn(&self) -> NonNull<dyn ImDrawIo> {
        ImUiContext::get_draw_ptr_dyn(*self)
    }
    #[inline(always)]
    fn get_draw_fg_ptr_dyn(&self) -> NonNull<dyn ImDrawTarget> {
        ImUiContext::get_draw_fg_ptr_dyn(*self)
    }
    #[inline(always)]
    fn get_draw_bg_ptr_dyn(&self) -> NonNull<dyn ImDrawTarget> {
        ImUiContext::get_draw_bg_ptr_dyn(*self)
    }
    #[inline(always)]
    fn get_io_ptr_dyn(&self) -> NonNull<dyn ImIo> {
        ImUiContext::get_io_ptr_dyn(*self)
    }
    #[inline(always)]
    fn get_pio_ptr_dyn(&self) -> NonNull<dyn ImPlatformIo> {
        ImUiContext::get_pio_ptr_dyn(*self)
    }
    #[inline(always)]
    fn imgui_version_num(&self) -> Option<NonZero<u32>> {
        ImUiContext::imgui_version_num(*self)
    }
    #[inline(always)]
    fn imgui_version_name(&self) -> Option<CStrPtr<'_>> {
        ImUiContext::imgui_version_name(*self)
    }
}
pub trait ImUiContextExt: ImUiContext {
    #[inline]
    fn with_io_dyn<R, F: FnOnce(&dyn ImIo) -> R>(&self, f: F) -> R {
        let io = unsafe { &*self.get_io_ptr_dyn().as_ptr() };
        f(io)
    }
    #[inline]
    fn with_io<R, F: FnOnce(&Self::Io) -> R>(&self, f: F) -> R
    where
        Self: ImContext,
    {
        let io = unsafe { &*self.get_io_ptr().as_ptr() };
        f(io)
    }
    #[inline]
    fn with_io_mut_dyn<R, F: FnOnce(&mut dyn ImIo) -> R>(&mut self, f: F) -> R {
        let io = unsafe { &mut *self.get_io_ptr_dyn().as_ptr() };
        f(io)
    }
    #[inline]
    fn with_io_mut<R, F: FnOnce(&mut Self::Io) -> R>(&mut self, f: F) -> R
    where
        Self: ImContext,
    {
        let io = unsafe { &mut *self.get_io_ptr().as_ptr() };
        f(io)
    }

    #[inline]
    fn with_style_dyn<R, F: FnOnce(&dyn ImStyle) -> R>(&self, f: F) -> R {
        let style = unsafe { &*self.get_style_ptr_dyn().as_ptr() };
        f(style)
    }
    #[inline]
    fn with_style<R, F: FnOnce(&Self::Style) -> R>(&self, f: F) -> R
    where
        Self: ImContext,
    {
        let style = unsafe { &*self.get_style_ptr().as_ptr() };
        f(style)
    }
    #[inline]
    fn with_style_mut_dyn<R, F: FnOnce(&mut dyn ImStyle) -> R>(&mut self, f: F) -> R {
        let style = unsafe { &mut *self.get_style_ptr_dyn().as_ptr() };
        f(style)
    }
    #[inline]
    fn with_style_mut<R, F: FnOnce(&mut Self::Style) -> R>(&mut self, f: F) -> R
    where
        Self: ImContext,
    {
        let style = unsafe { &mut *self.get_style_ptr().as_ptr() };
        f(style)
    }
}
impl<U: ?Sized> ImUiContextExt for U where U: ImUiContext {}
/// TODO: rename to ImContext? the other one is a mess and is more of an ImBoundContext instance or something...
pub unsafe trait ImContextState: ImUiContext {
    fn get_ptr(&self) -> NonNull<c_void> {
        nn::nonnull_ref(self).cast()
    }

    fn is_bound(&self) -> bool;
    unsafe fn unbind_unchecked(&mut self);
    unsafe fn bind_unchecked(&mut self);
    unsafe fn bind_allocator(
        &mut self,
        malloc: Option<UserMallocFn>,
        free: Option<UserFreeFn>,
        userdata: *mut c_void,
    );
    fn get_bound_allocator(&self) -> UiAllocatorFns;
    type BoundContext<'a, 'ui>: ?Sized + ImContext + 'a
    where
        Self: Sized + 'a,
        'ui: 'a;
    unsafe fn bound_mut<'a, 'ui>(&'a mut self) -> &'a mut Self::BoundContext<'a, 'ui>
    where
        Self: Sized,
        'ui: 'a;

    unsafe fn add_hook_boxed(
        &mut self,
        hook: Box<dyn ContextHookCallback>,
        type_untyped: u32,
        owner: usize,
    ) -> Option<NonZero<usize>>;
    unsafe fn remove_hook_by_id(&mut self, id: NonZero<usize>);
    #[cfg(todo)]
    unsafe fn call_hooks_by_type(&mut self, type_untyped: u32);
}
pub trait ImContextStateExt: ImContextState {
    #[inline(always)]
    unsafe fn context_mut_unchecked_nn<'a>(ptr: &'a NonNull<UnsafeCell<Self>>) -> &'a mut Self {
        &mut *(&*ptr.as_ptr()).get()
    }
    #[inline(always)]
    unsafe fn context_mut_unchecked<'a>(ptr: &'a UnsafeCell<Self>) -> &'a mut Self {
        &mut *ptr.get()
    }
}
impl<U: ?Sized> ImContextStateExt for U where U: ImContextState {}
pub trait ImIo {
    fn display_size(&self) -> ImSize2<f32>;
    fn display_framebuffer_scale(&self) -> ImVec2<f32>;
    fn want_text_input(&self) -> bool;
    fn want_capture_mouse(&self) -> bool;
    fn want_capture_keyboard(&self) -> bool;
    fn key_is_down_untyped(&self, c: usize) -> bool;
    fn key_is_pressed_untyped(&self, c: usize) -> bool;
    fn key_from_alphanum(&self, c: u8) -> usize;
    fn button_is_down_untyped(&self, b: usize) -> bool;
    fn button_is_pressed_untyped(&self, b: usize) -> bool;
}
impl ImIo for () {
    fn display_size(&self) -> ImSize2<f32> {
        Default::default()
    }
    fn display_framebuffer_scale(&self) -> ImVec2<f32> {
        Default::default()
    }
    fn want_text_input(&self) -> bool {
        false
    }
    fn want_capture_mouse(&self) -> bool {
        false
    }
    fn want_capture_keyboard(&self) -> bool {
        false
    }
    fn key_is_down_untyped(&self, _: usize) -> bool {
        false
    }
    fn key_is_pressed_untyped(&self, _: usize) -> bool {
        false
    }
    fn key_from_alphanum(&self, _: u8) -> usize {
        0
    }
    fn button_is_down_untyped(&self, _: usize) -> bool {
        false
    }
    fn button_is_pressed_untyped(&self, _: usize) -> bool {
        false
    }
}
pub trait ImIoExt {
    fn key_is_pressed_alphanum(&self, c: u8) -> bool;
    fn key_is_down_alphanum(&self, c: u8) -> bool;
}
impl<T> ImIoExt for T where
    T: ?Sized + ImIo,
{
    #[inline]
    fn key_is_pressed_alphanum(&self, c: u8) -> bool {
        self.key_is_pressed_untyped(self.key_from_alphanum(c))
    }
    #[inline]
    fn key_is_down_alphanum(&self, c: u8) -> bool {
        self.key_is_down_untyped(self.key_from_alphanum(c))
    }
}
/// TODO
pub trait ImPlatformIo {}
impl ImPlatformIo for () {}
/// TODO
pub trait ImDrawIo {}
impl ImDrawIo for () {}

pub trait AsUi<'ui, U: ?Sized + ImUi + ImContext> {
    fn ui(&self) -> &U;
    unsafe fn immortal_ui<'a>(&'a self) -> &'ui U {
        mem::transmute(self.ui())
    }
    #[cfg(todo)]
    fn push_token_font<N>(&self, font: N) -> UiTokenDyn<'ui>
    where
        N: UiFont<'ui, U>,
        N::FontToken: Into<UiTokenDyn<'ui>>,
    {
        self.ui().push_font_token(font).into()
    }

    #[inline]
    unsafe fn get_style_ref(&self) -> &U::Style {
        &*self.ui().get_style_ptr().as_ptr()
    }

    #[inline]
    unsafe fn get_io_ref(&self) -> &U::Io {
        &*self.ui().get_io_ptr().as_ptr()
    }
}

impl<'ui, U: ?Sized + ImUiWindow + ImContext, T: ?Sized + AsUi<'ui, U>> AsUi<'ui, U> for &'_ T {
    #[inline(always)]
    fn ui(&self) -> &U {
        AsUi::ui(*self)
    }
}
impl<'ui, U: ?Sized + ImUiWindow + ImContext, T: ?Sized + AsUi<'ui, U>> AsUi<'ui, U> for &'_ mut T {
    #[inline(always)]
    fn ui(&self) -> &U {
        AsUi::ui(*self)
    }
}

pub type UiAllocatorFns = (Option<UserMallocFn>, Option<UserFreeFn>, *mut c_void);
pub trait UiAllocatorRaw {
    fn get_allocator_raw(&self) -> UiAllocatorFns;
}
impl UiAllocatorRaw for UiAllocatorFns {
    #[inline]
    fn get_allocator_raw(&self) -> UiAllocatorFns {
        *self
    }
}
impl UiAllocatorRaw for Box<dyn UiAllocatorRaw> {
    #[inline]
    fn get_allocator_raw(&self) -> UiAllocatorFns {
        UiAllocatorRaw::get_allocator_raw(&**self)
    }
}
impl<T> UiAllocatorRaw for &'_ T
where
    T: ?Sized + UiAllocatorRaw,
{
    #[inline]
    fn get_allocator_raw(&self) -> UiAllocatorFns {
        UiAllocatorRaw::get_allocator_raw(*self)
    }
}

impl dyn UiAllocatorRaw {
    #[inline(never)]
    pub unsafe extern "C" fn nop_free(_p: *mut c_void, _data: *mut c_void) {}
    #[inline(never)]
    pub unsafe extern "C" fn nop_malloc(_sz: usize, _data: *mut c_void) -> *mut c_void {
        ptr::null_mut()
    }
    #[inline(never)]
    pub unsafe extern "C" fn unset_free(_p: *mut c_void, _data: *mut c_void) {
        log::warn!("imgui free() unset");
    }
    #[inline(never)]
    pub unsafe extern "C" fn unset_malloc(_sz: usize, _data: *mut c_void) -> *mut c_void {
        log::error!("imgui malloc() unset");
        ptr::null_mut()
    }
}

#[cfg(feature = "windows")]
mod windows_heap_alloc {
    use {
        super::{UiAllocatorFns, UiAllocatorRaw},
        arcffi::windows::WinResult,
        core::{borrow::Borrow, ffi::c_void, mem, ptr::NonNull},
        windows::Win32::{
            Foundation::HANDLE,
            System::Memory::{GetProcessHeap, HeapAlloc, HeapCreate, HeapDestroy, HeapFree, HEAP_FLAGS},
        },
    };

    #[derive(Debug, Copy, Clone)]
    #[repr(transparent)]
    pub struct HeapAllocator<H: ?Sized = HANDLE> {
        heap: H,
    }
    impl<H: ?Sized> HeapAllocator<H> {
        #[inline(always)]
        pub const unsafe fn new(heap: H) -> Self
        where
            H: Sized,
        {
            Self { heap }
        }
        #[inline(always)]
        pub const unsafe fn from_ref(heap: &H) -> &Self {
            mem::transmute(heap)
        }
        #[inline(always)]
        pub const fn heap(&self) -> &H {
            &self.heap
        }
        #[inline(always)]
        pub unsafe fn heap_mut(&mut self) -> &mut H {
            &mut self.heap
        }
    }
    impl HeapAllocator {
        #[inline]
        pub fn process_heap() -> WinResult<Self> {
            Ok(unsafe { Self::new(GetProcessHeap()?) })
        }

        #[inline(never)]
        pub unsafe extern "C" fn im_malloc(sz: usize, data: *mut c_void) -> *mut c_void {
            let heap = HANDLE(data);
            HeapAlloc(heap, Default::default(), sz)
        }
        #[inline(never)]
        pub unsafe extern "C" fn im_free(p: *mut c_void, data: *mut c_void) {
            let heap = HANDLE(data);
            let p = NonNull::new(p).map(|p| p.as_ptr() as *const c_void);
            let _ = HeapFree(heap, Default::default(), p);
        }
    }
    impl<H> UiAllocatorRaw for HeapAllocator<H>
    where
        H: Borrow<HANDLE>,
    {
        #[inline]
        fn get_allocator_raw(&self) -> UiAllocatorFns {
            let heap = self.heap.borrow().0;
            (Some(HeapAllocator::im_malloc), Some(HeapAllocator::im_free), heap)
        }
    }

    #[derive(Debug)]
    #[repr(transparent)]
    pub struct PrivateHeap {
        heap: HANDLE,
    }
    impl PrivateHeap {
        #[inline]
        pub unsafe fn new(flags: HEAP_FLAGS, size_initial: usize, size_max: usize) -> WinResult<Self> {
            HeapCreate(flags, size_initial, size_max)
                .map(|h| Self::with_handle(h))
                .map_err(Into::into)
        }
        #[inline(always)]
        pub const unsafe fn with_handle(heap: HANDLE) -> Self {
            Self { heap }
        }
        #[inline(always)]
        pub const unsafe fn from_ref(heap: &HANDLE) -> &Self {
            mem::transmute(heap)
        }
        #[inline(always)]
        pub const fn heap(&self) -> &HANDLE {
            &self.heap
        }
        #[inline(always)]
        pub unsafe fn heap_mut(&mut self) -> &mut HANDLE {
            &mut self.heap
        }
        #[inline(always)]
        pub fn leak_into_handle(self) -> HANDLE {
            let handle = self.heap;
            mem::forget(self);
            handle
        }
    }
    impl UiAllocatorRaw for PrivateHeap {
        #[inline]
        fn get_allocator_raw(&self) -> UiAllocatorFns {
            unsafe { HeapAllocator::from_ref(&self.heap).get_allocator_raw() }
        }
    }
    impl Drop for PrivateHeap {
        fn drop(&mut self) {
            unsafe {
                if let Err(e) = HeapDestroy(self.heap) {
                    log::warn!("heap({:p}) cleanup failed: {e}", self.heap.0);
                }
            }
        }
    }
}
#[cfg(feature = "windows")]
pub use self::windows_heap_alloc::{HeapAllocator as WinHeapAllocator, PrivateHeap as WinPrivateHeap};
