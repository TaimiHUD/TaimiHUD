use {
    super::class::ClassShared,
    crate::{
        exports::runtime as rt,
        render::machine::{frame_log, FrameState},
        settings::{goggles::GogglesEnables, pathing::SpaceSettings},
        space::{engine::FrameContext, pack::render::Drawing},
    },
    anyhow::Context,
    core::{ffi::c_void, ptr},
    glam::Vec2,
    std::{
        collections::BTreeSet,
        sync::atomic::{AtomicPtr, AtomicU32, Ordering},
        time::Instant,
    },
    sync_unsafe_cell::SyncUnsafeCell,
    taimi_d3d::dx11::{prelude::*, raster::RenderTargetView},
    taimi_meta::{
        packs::MapIndex,
        ui::{
            gameplay::{GameplayState, GameplayTransition},
            LocalContext,
        },
    },
};
#[cfg(feature = "goggles2-camera")]
use {
    crate::settings::pathing::CameraSource,
    core::{mem, ops},
};

#[cfg(feature = "goggles2-project")]
use super::project::ProjectMethod;
#[cfg(feature = "space")]
use crate::space::Engine;

pub struct GogglesShared {
    pub enabled: AtomicU32,
    pub flags: AtomicU32,
    pub display_size: Vec2,
    pub is_ingame: bool,
    pub dx11_context: AtomicPtr<c_void>,
    pub frame_buffers: BTreeSet<*mut c_void>,
    pub class: ClassShared,
    pub class2: super::class::ClassShared2,
    pub lens: super::lens::LensShared,
    #[cfg(feature = "goggles2-camera")]
    pub cam: super::camera::CameraShared,
    #[cfg(feature = "goggles2-project")]
    #[cfg(deleteme)]
    pub project: super::project::ProjectFerret,
    #[cfg(feature = "goggles2-project")]
    pub project2: super::project::ProjectShared,
}
impl GogglesShared {
    pub const DEFAULT: Self = Self {
        enabled: AtomicU32::new(0),
        flags: AtomicU32::new(GogglesFlags::DEFAULTS.bits()),
        display_size: Vec2::ZERO,
        is_ingame: false,
        dx11_context: AtomicPtr::new(ptr::dangling_mut()),
        frame_buffers: BTreeSet::new(),
        class: ClassShared::EMPTY,
        class2: super::class::ClassShared2::EMPTY,
        lens: super::lens::LensShared::EMPTY,
        #[cfg(feature = "goggles2-camera")]
        cam: super::camera::CameraShared::EMPTY,
        #[cfg(deleteme)]
        #[cfg(feature = "goggles2-project")]
        project: super::project::ProjectFerret::EMPTY,
        #[cfg(feature = "goggles2-project")]
        project2: super::project::ProjectShared::EMPTY,
    };
    #[inline(always)]
    pub fn get() -> *mut Self {
        static FERRET: SyncUnsafeCell<GogglesShared> = SyncUnsafeCell::new(GogglesShared::DEFAULT);
        FERRET.get()
    }
    #[inline]
    pub fn set_display_size(v: Vec2) {
        g2!(*&volatile mut ferret.display_size = v)
    }
    #[inline]
    pub fn display_size() -> Vec2 {
        g2!(*&volatile const ferret.display_size)
    }
    #[inline(always)]
    pub fn is_game_dx11(ctx: &Dx11Context) -> bool {
        if Self::dx11_context() != ctx.as_raw() {
            return false
        }
        FrameState::is_game() /*|| !RenderState::is_render_thread()*/
    }
    #[inline(always)]
    pub fn is_game_dx11_frame(ctx: &Dx11Context) -> bool {
        Self::is_game_dx11(ctx)
    }
    pub(super) fn acquire_write() -> Option<GogglesFrameToken> {
        #[cfg(todo)]
        if !RenderState::is_render_thread() {
            return None
        }
        Some(())
    }
    /// TODO
    #[inline(always)]
    pub(super) fn acquire_read() -> Option<GogglesFrameToken> {
        Self::acquire_write()
    }

    pub(super) const ZERO32: u32 = 0.0f32.to_bits();
    pub(super) const NEGZERO32: u32 = (-0.0f32).to_bits();
    #[cfg(todo)]
    pub(super) const NEGONE32: u32 = (-1.0f32).to_bits();
    #[cfg(todo)]
    pub(super) const ONE32: u32 = 1.0f32.to_bits();
}
impl GogglesShared {
    #[inline(always)]
    pub fn dx11_context_ref() -> &'static AtomicPtr<c_void> {
        unsafe { &*g2!(&raw const ferret.dx11_context) }
    }
    #[inline(always)]
    pub(super) fn dx11_context() -> *mut c_void {
        let ctx = Self::dx11_context_ref().load(Self::ENABLED_ORDERING);
        // pointless but also would rather not reorder anything around this thanks
        std::sync::atomic::compiler_fence(Ordering::AcqRel);
        ctx
    }
    fn set_dx11_context(ctx: Option<&Dx11Context>) {
        let ptr = match ctx {
            None => ptr::dangling_mut(),
            Some(ctx) => ctx.as_raw(),
        };
        Self::dx11_context_ref().store(ptr, Self::ENABLED_ORDERING);
    }

    #[inline(always)]
    pub fn enabled_ref() -> &'static AtomicU32 {
        unsafe { &*g2!(&raw const ferret.enabled) }
    }
    #[inline]
    pub fn enabled() -> GogglesEnables {
        GogglesEnables::from_bits_retain(Self::enabled_ref().load(Self::ENABLED_ORDERING))
    }
    fn enabled_set(enabled: GogglesEnables) {
        Self::enabled_ref().store(enabled.bits(), Self::ENABLED_ORDERING);
    }
    #[inline]
    fn enabled_insert(enabled: GogglesEnables) {
        Self::enabled_ref().fetch_or(enabled.bits(), Self::ENABLED_ORDERING);
    }
    #[inline]
    fn enabled_remove(enabled: GogglesEnables) {
        Self::enabled_ref().fetch_and(!enabled.bits(), Self::ENABLED_ORDERING);
    }
    #[inline]
    fn enabled_toggle(enabled: GogglesEnables) {
        Self::enabled_ref().fetch_xor(enabled.bits(), Self::ENABLED_ORDERING);
    }
    /// TODO: consider interactions with an async overlay thread
    pub(super) const ENABLED_ORDERING: Ordering = Self::FLAGS_ORDERING;

    #[inline(always)]
    pub fn flags_ref() -> &'static AtomicU32 {
        unsafe { &*g2!(&raw const ferret.flags) }
    }
    #[inline]
    pub fn read_flags() -> GogglesFlags {
        GogglesFlags::from_bits_retain(Self::flags_ref().load(Self::ENABLED_ORDERING))
    }
    #[inline]
    pub(super) fn clear_flags(mask: GogglesFlags) {
        Self::flags_ref().fetch_and(!mask.bits(), Self::ENABLED_ORDERING);
    }
    #[inline]
    pub(super) fn flags() -> GogglesFlags {
        GogglesFlags::from_bits_retain(Self::flags_ref().load(Self::FLAGS_ORDERING))
    }
    #[inline]
    pub(super) fn flags_set(flags: GogglesFlags) {
        Self::flags_ref().store(flags.bits(), Self::FLAGS_ORDERING);
    }
    #[inline]
    pub(super) fn flags_insert(flags: GogglesFlags) -> GogglesFlags {
        GogglesFlags::from_bits_retain(Self::flags_ref().fetch_or(flags.bits(), Self::FLAGS_ORDERING))
    }
    #[inline]
    pub(super) fn flags_remove(flags: GogglesFlags) -> GogglesFlags {
        GogglesFlags::from_bits_retain(Self::flags_ref().fetch_and(!flags.bits(), Self::FLAGS_ORDERING))
    }
    #[inline]
    pub(super) fn flags_toggle(flags: GogglesFlags) -> GogglesFlags {
        GogglesFlags::from_bits_retain(Self::flags_ref().fetch_xor(flags.bits(), Self::FLAGS_ORDERING))
    }
    /// TODO: consider supporting multi-threaded contexts
    ///
    /// also beware of how [Self::reset_end] is invoked, ugh...
    pub(super) const FLAGS_ORDERING: Ordering = Ordering::Relaxed;

    pub(super) fn reset_end(enables: GogglesEnables) {
        GogglesShared::clear_flags(GogglesFlags::DISCARD_FRAME_END);
        frame_log!("goggles; post-render/end flags={:?}", Self::flags());
        if enables.intersects(ClassShared::ENABLES) {
            ClassShared::reset_end();
        }
        #[cfg(feature = "goggles2-project")]
        if enables.contains(GogglesEnables::PROJECT_ENABLE) {
            super::project::ProjectShared::reset_end();
        }
        frame_log!(";; flags2={:?}", Self::flags());
    }
}
unsafe impl Send for GogglesShared {}
unsafe impl Sync for GogglesShared {}

pub(super) type GogglesFrameToken = ();

bitflags::bitflags! {
    #[derive(Debug, Copy, Clone, Default, PartialOrd, Ord, PartialEq, Eq, Hash)]
    pub struct GogglesFlags: u32 {
        const CLASS_CLEARED_COLOUR = 0x0001;
        const CLASS_CLEARED_DEPTH = 0x0002;
        const CLASS_CLEARED_INCONSISTENT = 0x0004;
        const CLASS_FRAME_ONGOING = 0x0008;
        const PROJECT_BLOCKING = 0x0010;
        const PROJECT_REACH_WORLD = 0x0020;
    }
}

impl GogglesFlags {
    pub const DEFAULTS: Self = Self::from_bits_retain(0 | Self::PROJECT_REACH_WORLD.bits());
    pub const DISCARD_FRAME_END: Self = Self::from_bits_retain(
        Self::CLASS_CLEARED_COLOUR.bits()
            | Self::CLASS_CLEARED_DEPTH.bits()
            | Self::CLASS_FRAME_ONGOING.bits(),
    );
}

#[derive(Debug, Clone, Default)]
pub struct GogglesState {
    /// requested features
    pub enabled_config: GogglesEnables,
    /// mask of usable features
    pub available: GogglesEnables,
    pub active: GogglesEnables,
    pub lens: super::lens::GogglesLens,
    pub class: super::class::GogglesClass,
    #[cfg(feature = "goggles2-camera")]
    pub camera: super::camera::GogglesCamera,
    #[cfg(feature = "goggles2-project")]
    pub project: super::project::GogglesProject,
}
impl GogglesState {
    #[inline(always)]
    pub fn is_enabled(&self, flag: GogglesEnables) -> bool {
        self.active.contains(flag)
    }
    fn enable(&mut self) {
        log::debug!("Goggles setup: enabling...");
        let res = super::enable().context("goggles setup");
        if rt::log::error_ok(res).is_some() {
            self.active.insert(GogglesEnables::ENABLE);
            self.available.insert(GogglesEnables::SUPPORTED_FEATURES);
        } else {
            let _ = rt::log::debug_ok(super::disable().context("goggles cleanup"));
            self.active.remove(GogglesEnables::ENABLE);
            self.available.remove(GogglesEnables::ENABLE);
            self.enabled_config.remove(GogglesEnables::ENABLE);
        }
    }
    fn disable(&mut self) {
        log::debug!("Goggles setup: disabling...");
        GogglesShared::set_dx11_context(None);
        {
            let frame_buffers = unsafe { &mut *g2!(&raw mut ferret.frame_buffers) };
            frame_buffers.clear();
        }
        let _ = rt::log::debug_ok(super::disable().context("goggles cleanup"));
        self.active.remove(GogglesEnables::ENABLE);
        self.available.remove(GogglesEnables::FEATURE_ENABLES);
    }
    pub fn enabled_config_effective(&self) -> GogglesEnables {
        (self.enabled_config & self.available).omit_unavailable()
    }
    pub fn refresh_enables(&mut self) {
        let new_disables = !self.enabled_config_effective() & self.active;
        frame_log!(
            "refresh enables: effective={:?}; avail={:?}; new(off)={new_disables:?}",
            self.enabled_config_effective(),
            self.available
        );
        frame_log!(
            "refresh enables: active={:?}; enabled={:?}",
            self.active,
            self.enabled_config
        );
        GogglesShared::enabled_remove(new_disables);
        for disabled in new_disables & GogglesEnables::FEATURE_ENABLES {
            match disabled {
                GogglesEnables::ENABLE => {
                    // delaying this until the end
                    continue
                },
                GogglesEnables::LENS_ENABLE => {
                    self.lens.disable();
                    self.active.remove(GogglesEnables::OPTIONS_LENS);
                    self.available.remove(GogglesEnables::OPTIONS_LENS);
                },
                GogglesEnables::CAMERA_ENABLE => {
                    self.camera.camera_disable();
                    self.active.remove(GogglesEnables::OPTIONS_CAMERA);
                    self.available.remove(GogglesEnables::OPTIONS_CAMERA);
                },
                GogglesEnables::PROJECT_ENABLE => {
                    self.project.disable();
                    self.active.remove(GogglesEnables::OPTIONS_PROJECT);
                    self.available.remove(GogglesEnables::OPTIONS_PROJECT);
                },
                GogglesEnables::ARCRENDER_ENABLE => {
                    self.active.remove(GogglesEnables::OPTIONS_ARCRENDER);
                    self.available.remove(GogglesEnables::OPTIONS_ARCRENDER);
                },
                _ => (),
            }
            self.active.remove(disabled);
        }
        if new_disables.contains(GogglesEnables::ENABLE) {
            self.class.disable();
            self.disable();
            return
        }
        let enabled_config = self.enabled_config_effective();
        let new_enables = enabled_config ^ self.active;
        if new_enables.is_empty() {
            return
        }
        if new_enables.contains(GogglesEnables::ENABLE) && super::needs_setup() {
            frame_log!("goggles; waiting for setup...");
            return
        }
        for flag in new_enables {
            let on = enabled_config.contains(flag);
            let enabled = match (flag, on) {
                (GogglesEnables::ENABLE, true) => {
                    self.enable();
                    self.class.enable();
                    break
                },
                #[cfg(todo = "unnecessary")]
                (GogglesEnables::ENABLE, false) => disable(),
                (GogglesEnables::LENS_ENABLE, true) => {
                    self.lens.enable();
                    self.available.insert(GogglesEnables::OPTIONS_LENS);
                    true
                },
                #[cfg(feature = "goggles2-camera")]
                (GogglesEnables::CAMERA_ENABLE, true) => {
                    self.camera.camera_enable();
                    self.available.insert(GogglesEnables::OPTIONS_CAMERA);
                    true
                },
                #[cfg(feature = "goggles2-project")]
                (GogglesEnables::PROJECT_ENABLE, true) => {
                    self.project.enable();
                    self.available.insert(GogglesEnables::OPTIONS_PROJECT);
                    true
                },
                #[cfg(feature = "goggles2-camera")]
                (GogglesEnables::CAMERA_PERSPECTIVE, on) => {
                    self.camera.set_perspective(on);
                    on
                },
                #[cfg(feature = "goggles2-camera")]
                (GogglesEnables::CAMERA_DIR, on) => {
                    self.camera.set_dir(on);
                    on
                },
                #[cfg(feature = "goggles2-project")]
                (GogglesEnables::PROJECT_COMPAT_METHOD, on) => {
                    match (on, self.project.method()) {
                        (true, ProjectMethod::Compatibility) => (),
                        (true, _) | (false, ProjectMethod::Compatibility) =>
                            self.project.set_method(match on {
                                true => ProjectMethod::Compatibility,
                                false => ProjectMethod::DEFAULT,
                            }),
                        _ => (),
                    }
                    on
                },
                (e, on) if e.intersects(GogglesEnables::OPTIONS_MASK) => on,
                _ => continue,
            };
            if enabled {
                self.active.insert(flag);
            } else {
                self.active.remove(flag);
            }
        }
    }

    pub(crate) fn act_map_enter(&mut self, hard: bool, _map_id: MapIndex) {
        if self.is_classifying() {
            self.class.act_map_enter();
        }
        self.lens.act_map_enter(hard);
        #[cfg(feature = "goggles2-camera")]
        {
            self.camera.act_map_enter(hard, _map_id);
        }
    }
    /// TODO: doesn't track cutscene states very well, so ignore for now...
    pub(crate) fn act_map_exit(&mut self, _gameplay: GameplayState, _trans: GameplayTransition) {
        if self.is_classifying() {
            self.class.act_map_exit();
        }
        self.lens.act_map_exit();
        #[cfg(feature = "goggles2-camera")]
        {
            self.camera.act_map_exit();
        }
    }
    pub(crate) fn act_render_post(&mut self) {
        #[cfg(todo)]
        if !self.enabled {
            return
        }

        frame_log!("goggles; enabled: {:?}", self.active);
        GogglesShared::enabled_set(self.active);

        if self.is_classifying() {
            self.class.act_render_post();
        }

        #[cfg(feature = "goggles2-camera")]
        {
            self.camera.act_render_post();
        }
        #[cfg(feature = "goggles2-project")]
        #[cfg(deleteme)]
        if self.is_enabled(GogglesEnables::PROJECT_ENABLE) {
            self.project.act_render_post();
        }
    }
    pub(crate) fn act_render_post_late(&mut self) {
        #[cfg(feature = "goggles2-project")]
        if self.is_enabled(GogglesEnables::PROJECT_ENABLE) {
            self.project.act_render_post_late();
        }
        GogglesShared::reset_end(self.active);
    }
    pub(crate) fn act_pre_render_frame(
        &mut self,
        context: Option<&Dx11Context>,
        target: Option<&RenderTargetView>,
        drawing: &mut FrameContext,
    ) {
        match context {
            Some(context)
                if !self.is_enabled(GogglesEnables::ENABLE)
                    && (self.available & self.enabled_config).contains(GogglesEnables::ENABLE)
                    && super::needs_setup() =>
            {
                log::debug!("Goggles setup: preparing...");
                let vtable = context.vtable();
                // XXX: TODO, unused atm
                let vtbl_dv = None;
                let res = super::setup(vtable, vtbl_dv).context("goggles init");
                if rt::log::warn_ok(res).is_none() {
                    self.available.remove(GogglesEnables::ENABLE);
                }
            },
            _ => (),
        }
        GogglesShared::set_dx11_context(context);
        {
            let frame_buffers = unsafe { &mut *g2!(&raw mut ferret.frame_buffers) };
            if let Some(target) = target {
                if self.is_classifying() {
                    if let Ok(res) = target.get_resource() {
                        frame_buffers.insert(res.as_d3d_raw().as_ptr());
                    }
                    frame_buffers.insert(target.as_d3d_raw().as_ptr());
                }
            } else {
                frame_buffers.clear();
            }
        }
        if self.is_enabled(GogglesEnables::LENS_ENABLE) {
            self.lens.act_pre_render_frame(drawing);
        }
        #[cfg(feature = "goggles2-project")]
        if self.is_enabled(GogglesEnables::PROJECT_ENABLE) {
            self.project.act_pre_render_frame(context.is_some(), drawing);
        }
        if self.is_classifying() {
            self.class
                .act_pre_render_frame(drawing.visible.intersects(Drawing::PASSES_PRIMARY));
        }
        let is_ingame = drawing.drawing.has(LocalContext::World);
        g2!(*&mut ferret.is_ingame = is_ingame);
    }
    pub(crate) fn wants_d3d_context(&self, drawing: &FrameContext) -> bool {
        if frame_log!(::is_enabled()) {
            return true
        }
        if self.enabled_config.contains(GogglesEnables::ENABLE) && !self.is_enabled(GogglesEnables::ENABLE)
        {
            return true
        }
        match drawing.is_drawing() {
            #[cfg(todo)]
            drawing => drawing || self.active.intersects(GogglesEnables::FEATURE_ENABLES),
            _ => true,
        }
    }

    pub(crate) fn act_pre_render(&mut self, has_engine: bool, display_size: Vec2) {
        GogglesShared::set_display_size(display_size);
        GogglesShared::clear_flags(GogglesFlags::DISCARD_FRAME_END);
        if has_engine {
            self.available.insert(GogglesEnables::ENABLE);
        }
        self.refresh_enables();

        if self.is_classifying() {
            self.class.act_pre_render();
        }
        #[cfg(feature = "goggles2-camera")]
        {
            self.camera.act_pre_render(has_engine);
        }
        #[cfg(feature = "goggles2-project")]
        #[cfg(deleteme)]
        if self.is_enabled(GogglesEnables::PROJECT_ENABLE) {
            self.project.act_pre_render(has_engine);
        }
        if !has_engine {
            g2!(*&mut ferret.is_ingame = has_engine);
        }
    }
    pub(crate) fn settings_enables(&self, settings: &SpaceSettings) -> GogglesEnables {
        let mut enables = settings.goggles.enables();
        #[cfg(feature = "goggles2-camera")]
        if settings.camera_source == Some(CameraSource::Goggles2) {
            enables.insert(
                GogglesEnables::ENABLE | GogglesEnables::CAMERA_ENABLE | GogglesEnables::CAMERA_DIR,
            );
        }
        #[cfg(deleteme)]
        if settings.goggles.enabled() {
            enables.insert(GogglesEnables::ENABLE);
        }
        enables
    }
    pub(crate) fn act_enable(&mut self, enables: GogglesEnables) {
        self.enabled_config = enables;
    }
    #[cfg(feature = "space")]
    pub(crate) fn setup_engine(&mut self, engine: &Engine, enables: GogglesEnables) {
        self.class.setup_engine(engine, enables);
        self.act_enable(enables);
    }

    #[inline]
    pub fn latest_frame_timestamp(&self) -> Option<Instant> {
        self.is_classifying()
            .then(|| self.class.latest_frame_timestamp())
            .flatten()
    }
}

#[cfg(feature = "goggles2-camera")]
pub(super) fn find_all(data: &[u32], needle: &[f32], eps: f32) -> Option<ops::RangeInclusive<usize>> {
    #[cfg(todo)]
    if needle.is_empty() {
        return None
    }
    let mut found = Vec::new();
    'data: for (i, &d) in data.iter().enumerate() {
        if matches!(d, GogglesShared::ZERO32 | GogglesShared::NEGZERO32) {
            continue
        }
        let f = f32::from_bits(d);
        let fabs = f.abs();
        for (ni, &n) in needle.iter().enumerate() {
            let nabs = n.abs();
            if !((fabs - nabs).abs() <= eps) {
                continue
            }
            if found.len() <= ni {
                found.resize(ni + 1, usize::MAX);
            }
            let fi = unsafe { found.get_unchecked_mut(ni) };
            if *fi != usize::MAX {
                continue
            }
            *fi = i;
            if found.iter().filter(|&&fi| fi != usize::MAX).count() >= needle.len() {
                break 'data
            }
            #[cfg(todo)]
            break;
        }
    }
    let found = found.iter().filter(|&&fi| fi != usize::MAX);
    if found.clone().count() >= needle.len() {
        found
            .clone()
            .min()
            .and_then(move |&min| found.max().map(|&max| min..=max))
    } else {
        None
    }
}

/// TODO: deprecate/remove?
#[cfg(feature = "goggles2-camera")]
pub(super) trait FerretPattern {
    fn search<'d>(&self, data: &'d [u32], granularity: usize) -> Option<&'d [u32]>;
}

#[cfg(feature = "goggles2-camera")]
pub(super) fn print_ferret(data: &[u32], offset: usize, len: usize) {
    if !frame_log!(::is_game()) {
        return
    }
    let displen = ((len + 3) / 4).max(16);
    let prior = (offset > 0)
        .then_some(offset.saturating_sub(16))
        .map(|off| {
            core::iter::once_with(move || {
                frame_log!(;"preceding 16:");
                unsafe { data.get_unchecked(off..offset) }
            })
        })
        .into_iter()
        .flatten();
    let found = unsafe { data.get_unchecked(offset..) };
    let chunks = found.chunks(4).take(displen).chain(prior);
    for chunk in chunks {
        use core::fmt::Write;
        let mut line = String::new();
        for &v in chunk {
            let _ = write!(&mut line, "  {:4.09}", f32::from_bits(v));
        }
        frame_log!(;"\t::{line}");
    }
}
#[cfg(feature = "goggles2-camera")]
pub(super) fn search_ferret<F, M>(
    data: &[u32],
    granularity: usize,
    mut filter: F,
    mut matcher: M,
    matchlen: Option<usize>,
) -> Option<&[u32]>
where
    F: FnMut(&[u32]) -> bool,
    M: FnMut(&[u32]) -> Option<usize>,
{
    let mut haystack = data;
    let mut pmatch = None;
    while !haystack.is_empty() {
        let next = haystack.get(granularity..).unwrap_or(&[]);
        let search = mem::replace(&mut haystack, next);
        if !filter(search) {
            continue
        }
        let offset = unsafe { search.as_ptr().offset_from(data.as_ptr()) } as usize;
        //frame_log!("- pre-match @{offset:#x}?");
        let mut matchlen = matchlen;
        if let Some(len) = matcher(search) {
            frame_log!("- actual match offset={offset:#x}");
            print_ferret(data, offset, len);
            matchlen = Some(len);
            pmatch = Some(unsafe { search.get_unchecked(..len) });
            #[cfg(todo)]
            break;
        }
        if let Some(amt) = matchlen {
            haystack = search.get(amt.max(granularity)..).unwrap_or(&[]);
        }
    }
    pmatch
}

macro_rules! g2 {
    (&raw mut ferret$(.$field:ident)+) => {
        match () {
            #[allow(unused_unsafe)]
            () => unsafe {
                &raw mut (*$crate::space::goggles::GogglesShared::get())$(.$field)+
            },
        }
    };
    (&raw const ferret$(.$field:ident)+) => {
        match () {
            #[allow(unused_unsafe)]
            () => unsafe {
                &raw const (*$crate::space::goggles::GogglesShared::get())$(.$field)+
            },
        }
    };
    (*&volatile mut ferret$(.$field:ident)+ = $v:expr$(;)?) => {
        match $v {
            #[allow(unused_unsafe)]
            ferret_v_ => unsafe {
                ::core::ptr::write_volatile(&raw mut (*$crate::space::goggles::GogglesShared::get())$(.$field)+, ferret_v_)
            },
        }
    };
    (*&volatile const ferret$(.$field:ident)+) => {
        match () {
            #[allow(unused_unsafe)]
            () => unsafe {
                ::core::ptr::read_volatile(&raw const (*$crate::space::goggles::GogglesShared::get())$(.$field)+)
            },
        }
    };
    (*&mut ferret$(.$field:ident)+ = $v:expr$(;)?) => {
        match $v {
            #[allow(unused_unsafe)]
            ferret_v_ => unsafe {
                ::core::ptr::write(&raw mut (*$crate::space::goggles::GogglesShared::get())$(.$field)+, ferret_v_)
            },
        }
    };
    (*&ferret$(.$field:ident)+) => {
        match () {
            #[allow(unused_unsafe)]
            () => unsafe {
                // read_volatile shouldn't matter...
                ::core::ptr::read(&raw const (*$crate::space::goggles::GogglesShared::get())$(.$field)+)
            },
        }
    };
}
pub(crate) use g2;
