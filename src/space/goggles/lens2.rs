use {
    super::{
        class::{BufferClass, BufferInfo, BufferKind, ClassShared},
        g2,
        D3dPtr,
        GogglesShared,
    },
    crate::space::{engine::FrameContext, pack::render::Drawing},
    arcffi::nn,
    core::{
        ffi::c_void,
        ptr::{self, NonNull},
    },
    glam::Vec2,
    std::sync::atomic::{AtomicBool, AtomicPtr},
    taimi_d3d::dx11::DepthView,
};

pub struct LensShared {
    pub selected: AtomicPtr<c_void>,
    pub compatible: AtomicBool,
}
impl LensShared {
    pub const EMPTY: Self = Self {
        selected: AtomicPtr::new(ptr::null_mut()),
        compatible: AtomicBool::new(false),
    };

    #[inline(always)]
    pub fn selected_ref() -> &'static AtomicPtr<c_void> {
        unsafe { &*g2!(&raw const ferret.lens.selected) }
    }
    #[inline]
    fn write_selected(selected: D3dPtr) {
        Self::selected_ref().store(nn::nonnull_ptr_mut(selected), GogglesShared::ENABLED_ORDERING);
    }
    #[inline]
    pub(super) fn read_selected() -> D3dPtr {
        NonNull::new(Self::selected_ref().load(GogglesShared::ENABLED_ORDERING))
    }

    #[inline(always)]
    pub fn compatible_ref() -> &'static AtomicBool {
        unsafe { &*g2!(&raw const ferret.lens.compatible) }
    }
    #[inline]
    fn write_compatible(compatible: bool) {
        Self::compatible_ref().store(compatible, GogglesShared::ENABLED_ORDERING);
    }
    #[inline]
    pub(super) fn read_compatible() -> bool {
        Self::compatible_ref().load(GogglesShared::ENABLED_ORDERING)
    }

    const DEPTH_WRITE_MIN_PASS1: u32 = 2;
    const DEPTH_WRITE_MIN_PASS2: u32 = 5;
    const DEPTH_WRITE_MIN: u32 = match () {
        #[cfg(todo)]
        _ => 4,
        _ => 3,
    };
    pub(super) fn buf_is_valid(buf: &BufferInfo) -> bool {
        Self::buf_is_valid_inner(buf, false, true)
    }
    pub(super) fn buf_is_valid_ongoing(buf: &BufferInfo) -> bool {
        if !buf.winner || buf.classification != BufferClass::World {
            return false
        }
        Self::buf_is_valid_inner(buf, buf.state.is_bound(ClassShared::bind_generation()), false)
    }
    fn buf_is_valid_inner(buf: &BufferInfo, is_bound: bool, strict: bool) -> bool {
        let write_min = match buf.bind_count {
            0 => false,
            1 if is_bound => false,
            1 if strict => buf.depth_binds_write <= Self::DEPTH_WRITE_MIN_PASS1,
            1 => buf.depth_binds_write < Self::DEPTH_WRITE_MIN_PASS1,
            2 if !strict => buf.depth_binds_write < Self::DEPTH_WRITE_MIN_PASS2,
            _ => buf.depth_binds_count_write() < Self::DEPTH_WRITE_MIN,
        };
        if write_min {
            return false
        }
        #[cfg(todo = "unnecessary")]
        if !ClassShared::buf_is_alive(buf) {
            return false
        }
        if let Some(Some((w, h))) = strict.then_some(buf.size()) {
            let expected = GogglesShared::display_size();
            if Vec2::new(w as f32, h as f32) != expected {
                return false
            }
        }
        true
    }
}

#[derive(Debug, Clone, Default)]
pub struct GogglesLens {
    available: bool,
}
impl GogglesLens {
    #[inline(always)]
    pub fn available(&self) -> bool {
        self.available
    }
    pub(crate) fn selected_ptr(&self) -> D3dPtr {
        self.available.then_some(LensShared::read_selected()).flatten()
    }
    pub fn with_selected_lens<R, F: FnOnce(Option<&DepthView>) -> R>(&self, f: F) -> R {
        let lens = self.selected_ptr();
        let dv = lens.as_ref().map(|p| unsafe { DepthView::from_d3d_raw_ref(p) });
        f(dv)
    }
    pub fn lens_compatible(&self) -> bool {
        LensShared::read_compatible()
    }
    pub fn enable(&mut self) {}
    pub fn disable(&mut self) {
        self.deselect();
    }
    fn deselect(&mut self) {
        self.available = false;
        LensShared::write_selected(None);
        LensShared::write_compatible(false);
    }
    pub fn act_pre_render_frame(&mut self, drawing: &mut FrameContext) {
        if !drawing.visible.contains(Drawing::SPACE) {
            return
        }
        let lens = match ClassShared::query_candidate(BufferKind::DepthView, BufferClass::World) {
            Some(lens) => {
                let (available, compatible) = ClassShared::with_seen2(lens, |buf| {
                    let avail = ClassShared::buf_is_alive(buf) && LensShared::buf_is_valid(buf);
                    let compat = buf.size().map(|(w, h)| {
                        let disp = GogglesShared::display_size();
                        Vec2::new(w as f32, h as f32).abs_diff_eq(disp, 1e-4)
                    });
                    (avail, compat.unwrap_or(true))
                })
                .unwrap_or((false, false));
                LensShared::write_compatible(compatible);
                available.then_some(lens)
            },
            None => None,
        };
        LensShared::write_selected(lens);
        self.available = lens.is_some();
    }
    pub(super) fn act_map_enter(&mut self, _hard: bool) {
        self.deselect();
    }
    pub(super) fn act_map_exit(&mut self) {
        self.deselect();
    }
}
