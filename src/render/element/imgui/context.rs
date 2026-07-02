use {
    crate::{
        render::element::{
            frame,
            im::{
                img::io::{ImContextStateExt, UiAllocatorFns, UiAllocatorRaw},
                prelude::*,
                ImContextExt,
            },
        },
        settings::{state::AddonHostName, UiConfig},
    },
    core::{
        fmt,
        marker::PhantomData,
        num::NonZero,
        ptr::{self, NonNull},
    },
    std::sync::{Arc, LazyLock},
};

pub struct UiContextCell<'c, A: UiAllocatorRaw = UiAllocatorFns> {
    version: NonZero<u32>,
    ctx: NonNull<()>,
    alloc: A,
    owned: Option<PhantomData<&'c ()>>,
}
impl<'c, A: UiAllocatorRaw> UiContextCell<'c, A> {
    /// TODO: fallback context impls (see [Self::context_mut] source for supported vers)
    #[inline]
    pub unsafe fn try_new_borrowed(version: NonZero<u32>, ctx: NonNull<()>, alloc: A) -> Option<Self> {
        match version {
            #[cfg(taimi_imgui = "192")]
            im192::VERSION_NUM => (),
            #[cfg(all(todo, not(taimi_imgui = "192")))]
            im192::VERSION_NUM => (),
            #[cfg(all(todo, not(taimi_imgui = "180")))]
            im180::VERSION_NUM => (),
            #[cfg(taimi_imgui = "180")]
            im180::VERSION_NUM => (),
            _ => return None,
        }
        Some(Self::with_parts(version, ctx, alloc, false))
    }
    #[inline]
    pub unsafe fn with_parts(version: NonZero<u32>, ctx: NonNull<()>, alloc: A, owned: bool) -> Self {
        Self {
            version,
            ctx,
            alloc,
            owned: owned.then_some(PhantomData),
        }
    }
    #[cfg(all(todo, taimi_imgui = "180"))]
    pub fn new_180() -> Self {
        Self::with_parts(im180::VERSION_NUM, ctx, alloc, true)
    }
    #[cfg(all(todo, taimi_imgui = "192"))]
    pub fn new_192() -> Self {
        Self::with_parts(im192::VERSION_NUM, ctx, alloc, true)
    }
    #[inline]
    pub fn context_mut(&mut self) -> &mut dyn ImContextExt {
        unsafe { UiContextCell::context_mut_for_version(self.ctx, self.version) }
    }
    #[inline]
    pub fn bind_alloc(&mut self) {
        let alloc = self.alloc.get_allocator_raw();
        unsafe { UiContextCell::bind_alloc_for(self.context_mut(), alloc) }
    }
    pub fn unbind_alloc(&mut self) {
        let context = unsafe { UiContextCell::context_mut_for_version(self.ctx, self.version) };
        match context.get_bound_allocator() {
            // XXX: older imgui versions lack a getter and always return None
            #[cfg(taimi_imgui = "180")]
            (None, ..) if self.version <= im180::VERSION_NUM => (),
            (Some(m), ..)
                if m as usize == self.alloc.get_allocator_raw().0.map(|p| p as usize).unwrap_or(0) =>
                (),
            _ => return,
        }
        unsafe {
            context.bind_allocator(
                Some(<dyn UiAllocatorRaw>::unset_malloc),
                Some(<dyn UiAllocatorRaw>::unset_free),
                ptr::null_mut(),
            );
        }
    }
    #[inline]
    pub unsafe fn destroy_context(&mut self) {
        match self.version {
            #[cfg(taimi_imgui = "180")]
            im180::VERSION_NUM => unsafe {
                im180::sys::igDestroyContext(self.ctx.as_ptr() as *mut im180::sys::ImGuiContext)
            },
            #[cfg(taimi_imgui = "192")]
            im192::VERSION_NUM => unsafe {
                im192::sys::igDestroyContext(self.ctx.as_ptr() as *mut im192::sys::ImGuiContext)
            },
            v => log::error!("how destroy context for {v}?"),
        }
    }
    #[inline]
    pub fn unbind(&mut self) {
        let context = self.context_mut();
        if context.is_bound() {
            unsafe {
                context.unbind_unchecked();
            }
        }
    }

    /// maybe technically "safe" if only one instance (per version) is ever constructed...
    #[inline]
    pub fn bound_context(&mut self) -> &mut dyn ImContextExt {
        let ctx = unsafe { UiContextCell::context_mut_for_version(self.ctx, self.version) };
        #[cfg(todo = "unnecessary")]
        if ctx.is_bound() {
            return ctx
        }
        unsafe {
            UiContextCell::bind_alloc_for(ctx, self.alloc.get_allocator_raw());
            ctx.bind_unchecked();
        }
        ctx
    }
    #[inline]
    pub unsafe fn bound_ui_unchecked<'ui>(&mut self) -> &mut dyn ImDrawWindow<'ui> {
        self.bound_context().bound_mut_dyn_unchecked()
    }
    /// see [Self::bound_context]
    #[inline]
    pub fn bound_ui<'a, 'ui>(&'a mut self) -> &'a mut dyn ImDrawWindow<'ui>
    where
        'a: 'ui,
    {
        unsafe { self.bound_ui_unchecked() }
    }

    #[cfg(todo)]
    pub fn context_version_mutex(&self) -> &'static Mutex<()> {
        Self::context_mutex_for(self.version)
    }
}
impl UiContextCell<'_> {
    /// no field lifetime projection is dumb...
    unsafe fn context_mut_for_version<'a>(
        ctx: NonNull<()>,
        version: NonZero<u32>,
    ) -> &'a mut dyn ImContextExt {
        match version {
            #[cfg(taimi_imgui = "192")]
            im192::VERSION_NUM =>
                im192::ImGuiContext::context_mut_unchecked(&*ctx.cast().as_ptr()) as &mut dyn ImContextExt,
            #[cfg(all(todo, not(taimi_imgui = "192")))]
            im192::VERSION_NUM => im192::FallbackContext::context_mut_unchecked(&*ctx.cast().as_ptr())
                as &mut dyn ImContextExt,
            #[cfg(taimi_imgui = "180")]
            im180::VERSION_NUM =>
                im180::ImGuiContext::context_mut_unchecked(&*ctx.cast().as_ptr()) as &mut dyn ImContextExt,
            #[cfg(all(todo, not(taimi_imgui = "180")))]
            im180::VERSION_NUM => im180::FallbackContext::context_mut_unchecked(&*ctx.cast().as_ptr())
                as &mut dyn ImContextExt,
            #[cfg(debug_assertions)]
            v => unreachable!("imgui {v} unsupported"),
            #[cfg(not(debug_assertions))]
            _ => core::hint::unreachable_unchecked(),
        }
    }
    #[inline(always)]
    unsafe fn bind_alloc_for(ctx: &mut dyn ImContextExt, (malloc, free, userdata): UiAllocatorFns) {
        ctx.bind_allocator(malloc, free, userdata);
    }

    /// TODO: ensure imgui behaves even if rendering from multiple threads
    #[cfg(todo)]
    pub fn context_mutex_for(version: NonZero<u32>) -> &'static Mutex<()> {}
}
impl<A: UiAllocatorRaw> Drop for UiContextCell<'_, A> {
    fn drop(&mut self) {
        if let Some(PhantomData) = self.owned {
            self.unbind_alloc();
            unsafe {
                self.destroy_context();
            }
        } else {
            self.unbind();
            self.unbind_alloc();
        }
    }
}
unsafe impl<A: UiAllocatorRaw + Send> Send for UiContextCell<'_, A> {}
impl<A: UiAllocatorRaw> fmt::Debug for UiContextCell<'_, A> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("ImguiContext")
            .field(&self.version)
            .field(&format_args!("{:p}", self.ctx))
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct UiFrameStorage {
    /// TODO: noop :<
    pub waker: core::task::Waker,
    pub frame: frame::RenderFrameUi,
    pub container: UiFrameContainer,
}
impl UiFrameStorage {
    /// TODO: combine this type with RenderFrameUi (decide if arc stays or goes)
    pub fn new_root_scope<'a, 'f, 'ui>(
        &'a self,
        container: &'f mut frame::ContainerContextState,
    ) -> frame::FrameContainerScope<'ui, 'f, frame::EmptyContext>
    where
        'a: 'ui,
    {
        let frame_state = frame::RenderFrameUiMut {
            frame_state: UiFrameState::default(),
            waker_ctx: core::task::Context::from_waker(&self.waker),
        };
        frame::FrameContainerScope::new_root(frame_state, &self.frame, container)
    }
}
#[derive(Debug, Clone)]
pub struct UiFrameViewport {
    pub host: AddonHostName,
}
#[derive(Debug, Clone)]
pub struct UiFrameContainer {
    pub viewport: UiFrameViewport,
    pub kind: u16,
}
impl UiFrameContainer {
    pub const TYPE_UNKNOWN: u16 = 0;
    pub const TYPE_VIEWPORT_PRESENT: u16 = 1;
    pub const TYPE_FRAME_OPTIONS: u16 = 2;
    /// arcdps
    pub const TYPE_ARC_WINDOW_TOGGLES: u16 = 3;
    /// subset of recovery [options](Self::TYPE_FRAME_OPTIONS) when not fully loaded
    pub const TYPE_TAIMI_OPTIONS_SAFE: u16 = 4;
}
#[derive(Debug, Clone)]
pub struct UiContextStorage {
    pub state: UiState,
    pub config: Arc<UiConfig>,
}
impl UiContextStorage {
    pub fn to_frame_storage(&self, container: UiFrameContainer) -> UiFrameStorage {
        UiFrameStorage {
            frame: frame::RenderFrameUi {
                ui_config: (*self.config).clone(),
                ui_state: self.state.clone(),
            },
            container,
            ..Default::default()
        }
    }
    fn empty_ui_config() -> &'static Arc<UiConfig> {
        static EMPTY_CONFIG: LazyLock<Arc<UiConfig>> = LazyLock::new(Default::default);
        &EMPTY_CONFIG
    }
}
impl AsRef<UiFrameStorage> for UiFrameStorage {
    #[inline(always)]
    fn as_ref(&self) -> &Self {
        self
    }
}
impl Default for UiFrameStorage {
    fn default() -> Self {
        Self {
            waker: futures::task::noop_waker(),
            frame: Default::default(),
            container: Default::default(),
        }
    }
}
impl Default for UiFrameViewport {
    fn default() -> Self {
        Self {
            host: AddonHostName::All,
        }
    }
}
impl Default for UiFrameContainer {
    fn default() -> Self {
        Self {
            viewport: Default::default(),
            kind: Self::TYPE_UNKNOWN,
        }
    }
}
impl Default for UiContextStorage {
    fn default() -> Self {
        Self {
            state: Default::default(),
            config: Self::empty_ui_config().clone(),
        }
    }
}
