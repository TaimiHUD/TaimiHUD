use {
    super::{g2, GogglesShared, GogglesFlags},
    crate::{
        exports::runtime::{
            self as rt,
            statistics::{StatsCounter, StatsUnit, StatsRef, StatsDesc},
        },
        render::machine::frame_log,
        settings::goggles::GogglesEnables,
    },
    arcffi::nn,
    core::{
        ffi::c_void,
        ops,
        mem,
        ptr::{self, NonNull},
    },
    std::sync::atomic::{AtomicU32, AtomicPtr},
    std::collections::{btree_map, BTreeMap},
    std::time::Instant,
    taimi_d3d::dx11::{
        prelude::*,
        d3d11::ID3D11UnorderedAccessView,
        depth::ComparisonFunc,
        RenderTargetView,
        Resource,
        DepthView,
        DepthState,
    },
    taimi_hoard::vec::vec32_eq,
};
#[cfg(feature = "space")]
use crate::space::Engine;

pub type D3dNn = NonNull<c_void>;
pub type D3dPtr = Option<D3dNn>;

pub(super) unsafe fn set_targets(
    _context: &Dx11Context,
    views: &[Option<RenderTargetView>],
    depth: Option<&DepthView>,
    _uavs: &[Option<ID3D11UnorderedAccessView>],
) {
    if !ClassShared::frame_valid_pre() {
        frame_log!(";; ignoring ({:?})", GogglesShared::flags());
        return
    }

    let depth_ptr = depth.map(|v| *v.as_d3d_raw());
    let render_primary = match views {
        &[ref view] => view.as_ref(),
        &[ref view, ref rest @ ..] if rest.iter().all(|r| r.is_none()) => view.as_ref(),
        _ => None,
    };
    let render_views = views.iter().flatten();
    #[cfg(todo = "unnecessary")]
    let uaviews = _uavs.iter()
        .filter_map(|uav| uav.as_ref());

    let now = ClassShared::frame_count();
    let bind_generation = ClassShared::bind_generation();

    let next_bind_generation = bind_generation.wrapping_add(1);
    let last_bind_generation = ClassShared::bind_generation_limit();
    let is_unbind_all = depth_ptr.is_none() && render_primary.is_none() && views.len() == 1;
    if next_bind_generation > last_bind_generation || is_unbind_all {
        let prev = GogglesShared::flags_remove(GogglesFlags::CLASS_FRAME_ONGOING);
        if prev.contains(GogglesFlags::CLASS_FRAME_ONGOING) {
            ClassShared::mark_end_frame();
        }
        return
    }

    let seen = &mut *g2!(&raw mut ferret.class.seen);
    let views = render_views
        .map(|rt| (&rt.view, BufferKind::RenderTarget))
        .chain(depth.map(|dv| (&dv.view, BufferKind::DepthView)))
        ;
    #[cfg(todo = "unnecessary")]
    let views = views.chain(
        uaviews.map(|v| (View::from_d3d_ref(v), BufferKind::UnorderedAccessView))
    );
    let flags = GogglesShared::flags();
    let mut is_early_bind = false;
    for (view, kind) in views.clone() {
        let view_ptr = *view.as_d3d_raw();
        let buf = match seen.entry(view_ptr) {
            btree_map::Entry::Occupied(e) => {
                let buf = e.into_mut();
                let _prev_seen = mem::replace(&mut buf.last_seen, now);
                buf
            },
            btree_map::Entry::Vacant(e) => {
                let resource = view.get_resource().ok();
                let buf_desc = resource.as_ref().and_then(|r| r.as_texture2()).map(|t2| t2.desc());
                e.insert(BufferInfo {
                    classification: BufferClass::New,
                    resource: resource.as_ref().map(|r| *r.as_d3d_raw()),
                    associated: None,
                    size: buf_desc.as_ref().map(|desc| (desc.Width, desc.Height)).unwrap_or(BufferInfo::EMPTY_SIZE),
                    format: buf_desc.as_ref().map(|desc| desc.Format).unwrap_or(BufferInfo::EMPTY_FORMAT),
                    last_seen: now,
                    first_seen: now,
                    kind,
                    state: BufferInfo::EMPTY_STATE,
                })
            },
        };
        buf.state.mark_bound(next_bind_generation);
        match (buf.winner, buf.kind, buf.classification) {
            (_, _, BufferClass::Reflection | BufferClass::Shadowbox) =>
                is_early_bind = buf.bind_count == 1,
            (true, kind @ BufferKind::RenderTarget, cls @ BufferClass::Target)
                if Some(buf.bind_count) == ClassShared::seen_class(kind, cls).map(|(_, _, bcount, ..)| bcount)
                => {
                    // your days are numbered...
                    ClassShared::bind_generation_limit_ref().store(next_bind_generation.wrapping_add(ClassShared::FRAME_END_BREATHING_ROOM), GogglesShared::FLAGS_ORDERING);
                },
            _ => (),
        }
        match kind {
            BufferKind::RenderTarget => {
                buf.record_bind_uavs(_uavs);
                buf.record_bind_depth(depth);
                if let Some(dv) = depth {
                    buf.record_association(*dv.as_d3d_raw(), view_ptr, Some(false));
                }
            },
            BufferKind::DepthView => {
                //#[cfg(todo)]
                if let Some(view) = render_primary {
                    buf.record_association(*view.as_d3d_raw(), view_ptr, Some(false));
                }
            },
            BufferKind::UnorderedAccessView | BufferKind::ShaderResourceView => (),
        }
    }

    ClassShared::bind_generation_ref().store(next_bind_generation, GogglesShared::FLAGS_ORDERING);
    ClassShared::bound_depth_ref().store(nn::nonnull_ptr_mut(depth_ptr), GogglesShared::FLAGS_ORDERING);
    let render_primary_ptr = render_primary.map(|v| *v.as_d3d_raw());
    ClassShared::bound_render_primary_ref().store(nn::nonnull_ptr_mut(render_primary_ptr), GogglesShared::FLAGS_ORDERING);

    if is_early_bind && flags.class_cleared_inconsistent() {
        frame_log!("goggles; fallback frame trigger");
        GogglesShared::flags_insert(ClassShared::FLAGS_CLEARS);
        ClassShared::mark_new_frame();
    }
}
impl GogglesFlags {
    /// if game doesn't clear framebuffers at start of each frame,
    /// fall back to triggering frame start when early buffers bound
    ///
    /// clearing behaviour seems to depend on time of day, maybe combined with shadow or shader settings being set high?
    /// maybe the hook or present check just fails sometimes idk...
    fn class_cleared_inconsistent(self) -> bool {
        (self & (ClassShared::FLAGS_CLEARS | GogglesFlags::CLASS_CLEARED_INCONSISTENT)) == GogglesFlags::CLASS_CLEARED_INCONSISTENT
    }
}
pub(super) unsafe fn set_state(
    _context: &Dx11Context,
    state: Option<&DepthState>,
    stencil_ref: u32,
) {
    if !ClassShared::classifying_depth() { return }
    let desc = state.map(|state| state.get_desc());
    if let Some(desc) = &desc {
        let depth_ptr = ClassShared::bound_depth_ptr();
        let depth_en = desc.DepthEnable.0 != 0;
        let depth_write = desc.DepthWriteMask != d3d11::D3D11_DEPTH_WRITE_MASK_ZERO;
        let cmp = ComparisonFunc::try_from_d3d(desc.DepthFunc).ok();
        let depth = if depth_ptr.is_none() {
            Some(DepthSummary::ViewNull)
        } else if !depth_en {
            Some(DepthSummary::Disabled)
        } else if !depth_write {
            cmp.map(DepthSummary::ReadOnly)
        } else {
            cmp.map(DepthSummary::ReadWrite)
        };
        let render_ptr_primary = ClassShared::bound_render_primary_ptr();
        if let Some(depth) = depth {
            let is_strong_state = matches!(depth, DepthSummary::ReadWrite(..));
            if render_ptr_primary.is_some() || !ClassShared::classifying_render() {
                let keys = IntoIterator::into_iter([
                    render_ptr_primary,
                    depth_ptr,
                ]).flatten();
                ClassShared::bufs_mut(keys, |buf, _key| {
                    if is_strong_state {
                        if let (BufferKind::RenderTarget, Some(depth_ptr)) = (buf.kind, depth_ptr) {
                            buf.record_association(depth_ptr, _key, Some(true));
                        } else if let (BufferKind::DepthView, Some(render_ptr)) = (buf.kind, render_ptr_primary) {
                            buf.record_association(render_ptr, _key, None);
                        }
                    }
                    buf.state.record_depth(depth, stencil_ref);
                });
            } else if ClassShared::classifying_render() {
                ClassShared::bound_bufs_all_mut(|buf, _key| {
                    if is_strong_state {
                        if let (BufferKind::RenderTarget, Some(depth_ptr)) = (buf.kind, depth_ptr) {
                            buf.record_association(depth_ptr, _key, Some(true));
                        } else if let (BufferKind::DepthView, Some(render_ptr)) = (buf.kind, render_ptr_primary) {
                            buf.record_association(render_ptr, _key, None);
                        }
                    }
                    buf.state.record_depth(depth, stencil_ref);
                });
            }
        }
    }
}
#[inline]
pub(super) unsafe fn clear_depth(
    _context: &Dx11Context,
    view: &DepthView,
    _flags: u32,
    _depth: f32,
    _fill_value: u8,
) {
    if !ClassShared::frame_valid_pre() {
        frame_log!(";; ignoring ({:?})", GogglesShared::flags());
        return
    }
    let keys = [
        *view.as_d3d_raw(),
    ];
    ClassShared::bufs_mut(keys, |buf, _key| {
        buf.state.flags.insert(BufferStateFlags::CLEARED_DEPTH);
        #[cfg(todo = "unnecessary")]
        {
            buf.state.clear_depth = (_flags, _depth, _fill_value);
        }
    });
    let flags_prev = GogglesShared::flags_insert(GogglesFlags::CLASS_CLEARED_DEPTH);
    let was_pending = flags_prev & ClassShared::FLAGS_CLEARS == GogglesFlags::CLASS_CLEARED_COLOUR;
    if was_pending {
        ClassShared::mark_new_frame();
    }
}

#[inline]
pub(super) unsafe fn clear_colour(
    _context: &Dx11Context,
    view: &RenderTargetView,
    colour: &[f32; 4],
) {
    if !ClassShared::frame_valid_pre() {
        frame_log!(";; ignoring ({:?})", GogglesShared::flags());
        return
    }
    let keys = [
        *view.as_d3d_raw(),
    ];
    ClassShared::bufs_mut(keys, |buf, _key| {
        buf.state.flags.insert(BufferStateFlags::CLEARED_COLOUR);
        buf.state.clear_colour = *colour;
    });
    let flags_prev = GogglesShared::flags_insert(GogglesFlags::CLASS_CLEARED_COLOUR);
    let was_pending = flags_prev & ClassShared::FLAGS_CLEARS == GogglesFlags::CLASS_CLEARED_DEPTH;
    if was_pending {
        ClassShared::mark_new_frame();
    }
}

pub struct ClassShared2 {
    expect_present_count: AtomicU32,
    frame_count: AtomicU32,
    inconsistent_clears: AtomicU32,
    bind_generation: AtomicU32,
    bind_generation_limit: AtomicU32,
    bound_depth: AtomicPtr<c_void>,
    bound_render_primary: AtomicPtr<c_void>,
}
impl ClassShared2 {
    pub const EMPTY: Self = Self {
        expect_present_count: AtomicU32::new(ClassShared::PRESENT_COUNT_DISABLED),
        frame_count: AtomicU32::new(0),
        bind_generation: AtomicU32::new(0),
        bind_generation_limit: AtomicU32::new(0),
        inconsistent_clears: AtomicU32::new(0),
        bound_depth: AtomicPtr::new(ptr::null_mut()),
        bound_render_primary: AtomicPtr::new(ptr::null_mut()),
    };
}
impl ClassShared {
    #[inline(always)]
    fn expect_present_count_ref() -> &'static AtomicU32 {
        unsafe {
            &*g2!(&raw const ferret.class2.expect_present_count)
        }
    }

    #[inline(always)]
    fn frame_count_ref() -> &'static AtomicU32 {
        unsafe {
            &*g2!(&raw const ferret.class2.frame_count)
        }
    }
    #[inline(always)]
    pub(super) fn frame_count() -> u32 {
        Self::frame_count_ref().load(GogglesShared::FLAGS_ORDERING)
    }
    #[inline(always)]
    pub fn read_frame_count() -> u32 {
        Self::frame_count_ref().load(GogglesShared::ENABLED_ORDERING)
    }

    #[inline(always)]
    fn bind_generation_ref() -> &'static AtomicU32 {
        unsafe {
            &*g2!(&raw const ferret.class2.bind_generation)
        }
    }
    #[inline(always)]
    pub(super) fn bind_generation() -> u32 {
        Self::bind_generation_ref().load(GogglesShared::FLAGS_ORDERING)
    }
    #[inline(always)]
    pub fn read_bind_generation() -> u32 {
        Self::bind_generation_ref().load(GogglesShared::ENABLED_ORDERING)
    }

    #[inline(always)]
    fn bind_generation_limit_ref() -> &'static AtomicU32 {
        unsafe {
            &*g2!(&raw const ferret.class2.bind_generation_limit)
        }
    }
    #[inline(always)]
    fn bind_generation_limit() -> u32 {
        Self::bind_generation_limit_ref().load(GogglesShared::FLAGS_ORDERING)
    }

    #[inline(always)]
    fn bound_depth_ref() -> &'static AtomicPtr<c_void> {
        unsafe {
            &*g2!(&raw const ferret.class2.bound_depth)
        }
    }
    #[inline(always)]
    pub(super) fn bound_depth_ptr() -> D3dPtr {
        NonNull::new(
            Self::bound_depth_ref().load(GogglesShared::FLAGS_ORDERING)
        )
    }

    #[inline(always)]
    fn bound_render_primary_ref() -> &'static AtomicPtr<c_void> {
        unsafe {
            &*g2!(&raw const ferret.class2.bound_render_primary)
        }
    }
    #[inline(always)]
    pub(super) fn bound_render_primary_ptr() -> D3dPtr {
        NonNull::new(
            Self::bound_render_primary_ref().load(GogglesShared::FLAGS_ORDERING)
        )
    }

    #[inline(always)]
    fn inconsistent_clears_ref() -> &'static AtomicU32 {
        unsafe {
            &*g2!(&raw const ferret.class2.inconsistent_clears)
        }
    }
    const INCONSISTENT_CLEARS_THRESHOLD: u32 = 28;

    fn mark_new_frame() {
        unsafe { if rt::arcdps_available() {
            rt::render_post();
        } }
        g2!(*&volatile mut ferret.class.frame_start = Some(Instant::now()));
        if !g2!(*&volatile const ferret.class.game_time) {
            // leave as an invalid frame while between maps
            return
        }
        GogglesShared::flags_insert(GogglesFlags::CLASS_FRAME_ONGOING);
        frame_log!("goggles; world/start");
        #[cfg(feature = "goggles2-project")]
        if super::project::ProjectShared::wants_draw() {
            super::project::ProjectShared::on_new_frame();
        }
    }
    fn mark_end_frame() {
        frame_log!("goggles; world/end");
        STATS_GAME_RENDER.reset_with(|| {
            let start = g2!(*&volatile const ferret.class.frame_start);
            start.map(|s| s.elapsed().as_micros() as u64).unwrap_or(0)
        });
        Self::bound_render_primary_ref().store(ptr::null_mut(), GogglesShared::FLAGS_ORDERING);
        Self::bound_depth_ref().store(ptr::null_mut(), GogglesShared::FLAGS_ORDERING);
        unsafe { if rt::arcdps_available() {
            if NonNull::new(rt::imgui::sys::igGetCurrentContext()) == crate::exports::arcdps::r#extern::arc_imgui_context_ptr() {
                rt::im_io_mut(|io| rt::render_pre(io));
            }
        } }
    }
}
type WinnerInfo = (D3dNn, i32, u32, u32);
pub struct ClassShared {
    pub frame_start: Option<Instant>,
    pub classify_tick: bool,
    pub cleanup_time: bool,
    pub game_time: bool,
    pub(super) seen: BTreeMap<D3dNn, BufferInfo>,
    pub(super) winners: BTreeMap<(BufferKind, BufferClass), WinnerInfo>,
    assoc: BTreeMap<D3dNn, D3dNn>,
}
impl ClassShared {
    pub const EMPTY: Self = Self {
        frame_start: None,
        classify_tick: false,
        cleanup_time: false,
        game_time: false,
        seen: BTreeMap::new(),
        winners: BTreeMap::new(),
        assoc: BTreeMap::new(),
    };
    #[cfg(todo)]
    pub const ENABLES: GogglesEnables = GogglesEnables::ENABLE;
    pub const ENABLES: GogglesEnables = GogglesEnables::from_bits_retain(
        GogglesEnables::LENS_ENABLE.bits()
        | GogglesEnables::PROJECT_ENABLE.bits()
    );
    const FRAME_END_BREATHING_ROOM: u32 = 3;
    const FLAGS_FRAME_VALID: GogglesFlags = GogglesFlags::from_bits_retain(
        GogglesFlags::CLASS_CLEARED_COLOUR.bits()
        | GogglesFlags::CLASS_CLEARED_DEPTH.bits()
        | GogglesFlags::CLASS_FRAME_ONGOING.bits()
    );
    const FLAGS_FRAME_INVALID: GogglesFlags = Self::FLAGS_CLEARS;
    const FLAGS_CLEARS: GogglesFlags = GogglesFlags::from_bits_retain(
        GogglesFlags::CLASS_CLEARED_COLOUR.bits()
        | GogglesFlags::CLASS_CLEARED_DEPTH.bits()
    );
    pub(super) fn frame_valid() -> bool {
        GogglesShared::flags().contains(Self::FLAGS_FRAME_VALID)
    }
    #[cfg(todo)]
    fn frame_valid_class() -> bool {
        match GogglesShared::flags() & Self::FLAGS_FRAME_VALID {
            Self::FLAGS_FRAME_VALID => true,
            Self::FLAGS_FRAME_INVALID => false,
            flags => Self::frame_valid_present()
                .unwrap_or(flags.contains(GogglesFlags::CLASS_FRAME_ONGOING)),
        }
    }
    fn frame_valid_pre() -> bool {
        match GogglesShared::flags() & Self::FLAGS_FRAME_VALID {
            Self::FLAGS_FRAME_INVALID => false,
            f if f.contains(GogglesFlags::CLASS_FRAME_ONGOING) => true,
            _ => Self::frame_valid_present().unwrap_or(false),
        }
    }
    fn frame_enabled() -> bool {
        GogglesShared::enabled().intersects(Self::ENABLES)
    }

    const PRESENT_COUNT_THRESHOLD: u32 = 2;
    const PRESENT_COUNT_DISABLED: u32 = 0;
    const PRESENT_COUNT_FAILURE: bool = true;
    fn frame_valid_present() -> Option<bool> {
        match Self::expect_present_count_ref().load(GogglesShared::ENABLED_ORDERING) {
            Self::PRESENT_COUNT_DISABLED => None,
            count => {
                let ok = rt::with_dxgi_swap_chain(|sc| sc.get_last_present_count().ok())
                    .flatten()
                    .map(|latest| latest.wrapping_sub(count) <= Self::PRESENT_COUNT_THRESHOLD)
                    .unwrap_or(Self::PRESENT_COUNT_FAILURE);
                Some(ok)
            },
        }
    }

    /// TODO
    fn classifying_depth() -> bool {
        Self::frame_enabled()
    }
    /// TODO
    #[cfg(todo)]
    fn classifying_render() -> bool {
        Self::frame_valid()
    }
    fn classifying_render() -> bool { Self::classifying_depth() }
    pub(super) fn reset_end() {
        let order = GogglesShared::ENABLED_ORDERING;
        let prev_frame = Self::frame_count_ref().fetch_add(1, order);

        let is_ingame_world = g2!(*&volatile const ferret.is_ingame);
        let is_ingame = g2!(*&volatile const ferret.class.game_time);
        let seen = unsafe {
            &mut *g2!(&raw mut ferret.class.seen)
        };
        let winners = unsafe {
            &mut *g2!(&raw mut ferret.class.winners)
        };
        let classify_tick = Self::read_classify_tick();
        let cleanup_time = Self::take_cleanup_time();

        seen.retain(|key, buf| {
            let winprev = (buf.kind, buf.classification);
            let since = buf.seen_since(prev_frame);
            let forget = match buf.classification {
                BufferClass::Taimi | BufferClass::Target | BufferClass::New => false,
                _ if since > BufferInfo::TIME_GONE || cleanup_time => true,
                _ => false,
            };
            if forget {
                if buf.state.winner {
                    match winners.entry(winprev) {
                        btree_map::Entry::Occupied(e) if e.get().0 == *key => {
                            e.remove();
                        },
                        _ => (),
                    }
                }
                return false
            }
            if since <= 1 && is_ingame {
                buf.prepare_state(classify_tick, is_ingame_world);
            }
            let winkey = (buf.kind, buf.classification);
            let cls_is_competitive = || match winkey {
                _ if !is_ingame => false,
                (_, BufferClass::World | BufferClass::Target | BufferClass::Minimap) => true,
                (_, BufferClass::Shadowbox) => true,
                (_, BufferClass::Reflection) => true,
                (_, BufferClass::FrameBuffer) => true,
                (BufferKind::RenderTarget, BufferClass::Pretty | BufferClass::Fallback) => true,
                _ => false,
            };
            let score = buf.state.classification_score;
            if buf.winner {
                if winprev.1 == winkey.1 {
                    let bind_count_changed = false;
                    if classify_tick || (bind_count_changed && is_ingame) {
                        if let Some(winner) = winners.get_mut(&winkey) {
                            if winner.0 == *key {
                                winner.2 = buf.bind_count;
                                if buf.bind_count > 0 {
                                    winner.3 = buf.bind_generation;
                                }
                            } else {
                                buf.state.winner = false;
                            }
                        }
                    }
                } else {
                    match winners.entry(winprev) {
                        btree_map::Entry::Occupied(e) if e.get().0 == *key => {
                            e.remove();
                        },
                        _ => (),
                    }
                    buf.state.winner = false;
                }
            } else if since <= 1 && score > 0 && cls_is_competitive() {
                let won = match winners.entry(winkey) {
                    btree_map::Entry::Vacant(e) => {
                        e.insert((*key, score, buf.bind_count, buf.state.bind_generation));
                        true
                    },
                    btree_map::Entry::Occupied(e) if e.get().0 == *key =>
                        true,
                    btree_map::Entry::Occupied(e) if e.get().1 >= score /* || e.get().2 == 0 */ =>
                        false,
                    btree_map::Entry::Occupied(e) => {
                        let e = e.into_mut();
                        *e = (*key, score, buf.bind_count, buf.bind_generation);
                        true
                    },
                };
                buf.state.winner = won;
            }
            buf.clear_state();
            true
        });
        for (key, buf) in &mut *seen {
            if buf.state.winner {
                let winkey = (buf.kind, buf.classification);
                if winners.get(&winkey).map(|(k, ..)| k != key).unwrap_or(true) {
                    buf.state.winner = false;
                }
            }
        }

        Self::bind_generation_ref().store(0, order);
        let next_classify_tick = is_ingame && {
            let mut classify_tick = is_ingame_world && (Self::read_frame_count() & 0x7f) == 0x7f;
            let enabled = GogglesShared::enabled();
            if enabled.contains(GogglesEnables::LENS_ENABLE) && is_ingame_world {
                classify_tick |= !winners.contains_key(&(BufferKind::DepthView, BufferClass::World));
            }
            if enabled.contains(GogglesEnables::PROJECT_ENABLE) {
                if is_ingame_world {
                    classify_tick |= !winners.contains_key(&(BufferKind::RenderTarget, BufferClass::World));
                    classify_tick |= !winners.contains_key(&(BufferKind::RenderTarget, BufferClass::Minimap));
                }
                classify_tick |= !winners.contains_key(&(BufferKind::RenderTarget, BufferClass::Target));
            }
            classify_tick
        };
        g2!(*&volatile mut ferret.class.classify_tick = next_classify_tick);
        Self::bind_generation_limit_ref().store(u32::MAX, order);
        let expect_present_count = Self::expect_present_count_ref().load(order);
        frame_log!("goggles; SC frame#{expect_present_count}");
        let has_present_count = Self::expect_present_count_ref().load(order) != Self::PRESENT_COUNT_DISABLED;
        if has_present_count {
            Self::bound_render_primary_ref().store(ptr::null_mut(), order);
            Self::bound_depth_ref().store(ptr::null_mut(), order);
        } else if is_ingame {
            GogglesShared::flags_insert(GogglesFlags::CLASS_FRAME_ONGOING);
        }
        let flags = GogglesShared::flags();
        if flags.contains(GogglesFlags::CLASS_CLEARED_INCONSISTENT) {
            #[cfg(todo)]
            if !has_present_count && !winners.contains_key(&(BufferKind::DepthView, BufferClass::Shadowbox)) | !winners.contains_key(&(BufferKind::DepthView, BufferClass::Reflection)) {
                GogglesShared::flags_insert(Self::FLAGS_CLEARS);
            }
        } else if is_ingame_world {
            if flags.contains(Self::FLAGS_CLEARS) {
                Self::inconsistent_clears_ref().store(0, order);
            } else {
                let streak = Self::inconsistent_clears_ref().fetch_add(1, order);
                if streak >= Self::INCONSISTENT_CLEARS_THRESHOLD {
                    GogglesShared::flags_insert(GogglesFlags::CLASS_CLEARED_INCONSISTENT);
                }
            }
        }
    }
    pub(super) fn reset_frame(&mut self) {
        self.frame_start = None;
        let assocs = mem::take(&mut self.assoc).into_iter()
            .filter_map(|(key, assoc)| match self.seen.get(&assoc) {
                Some(buf) => Some((key, assoc, buf.classification, buf.state.flags, buf.state.clear_colour)),
                _ => None,
            }).collect::<Vec<_>>();
        for (key, assoc_key, assoc_cls, assoc_flags, assoc_colour) in assocs {
            let Some(buf) = self.seen.get_mut(&key) else { continue };
            let rt_flags = buf.state.flags;
            let rt_colour = buf.state.clear_colour;
            buf.state.flags.insert(assoc_flags);
            if assoc_flags.difference(rt_flags).contains(BufferStateFlags::CLEARED_COLOUR) {
                buf.state.clear_colour = assoc_colour;
            }
            match (buf.classification, assoc_cls) {
                (_, BufferClass::Unsupported) => continue,
                #[cfg(todo)]
                (BufferClass::Unknown, BufferClass::Unsupported) => {
                    buf.classification = BufferClass::Unsupported;
                    continue
                },
                #[cfg(todo)]
                (BufferClass::New, _) => continue,
                (BufferClass::Unknown, _) => (),
                _ => continue,
            }
            let rt_clear_colour = buf.state.clear_colour;
            //buf.state.depth_binds_count = etc; ?
            let Some(dbuf) = self.seen.get_mut(&assoc_key) else { continue };
            if rt_flags.difference(dbuf.state.flags).contains(BufferStateFlags::CLEARED_COLOUR) {
                dbuf.state.clear_colour = rt_colour;
            }
            dbuf.state.flags.insert(rt_flags);
            dbuf.state.clear_colour = rt_clear_colour;
        }
    }

    pub fn read_classify_tick() -> bool {
        g2!(*&volatile const ferret.class.classify_tick)
    }
    pub fn take_cleanup_time() -> bool {
        let cleanup = g2!(*&volatile const ferret.class.cleanup_time);
        g2!(*&volatile mut ferret.class.cleanup_time = false);
        cleanup
    }

    #[inline(always)]
    fn with_mut<F: FnOnce(&mut ClassShared)>(f: F) {
        let class = g2!(&raw mut ferret.class);
        f(unsafe { &mut *class })
    }
    #[inline(always)]
    #[deprecated]
    pub(crate) fn with_seen<F: FnOnce(&BufferInfo)>(key: D3dNn, f: F) -> bool {
        Self::with_seen2(key, move |buf| f(buf)).is_some()
    }
    #[inline(always)]
    pub(crate) fn with_seen2<R, F: FnOnce(&BufferInfo) -> R>(key: D3dNn, f: F) -> Option<R> {
        if g2!(*&ferret.class.cleanup_time) { return None }
        let seen = unsafe {
            &*g2!(&raw const ferret.class.seen)
        };
        seen.get(&key).map(f)
    }
    #[inline]
    pub(crate) fn with_seen_class<R, F: FnOnce(D3dNn, &BufferInfo) -> R>(kind: BufferKind, cls: BufferClass, f: F) -> Option<R> {
        Self::seen_class(kind, cls)
            .and_then(|(key, ..)| Self::with_seen2(key, |buf| f(key, buf)))
    }
    #[inline]
    pub(crate) fn buf_is_alive(buf: &BufferInfo) -> bool {
        buf.last_seen == Self::read_frame_count()
    }
    #[inline]
    pub(super) fn with_current_dv<R, F: FnOnce(D3dNn, &BufferInfo) -> R>(f: F) -> Option<R> {
        let seen = unsafe {
            &*g2!(&raw const ferret.class.seen)
        };
        Self::bound_depth_ptr().and_then(|key| seen.get(&key)
            .map(|buf| f(key, buf))
        )
    }
    pub(super) fn seen_class(kind: BufferKind, cls: BufferClass) -> Option<WinnerInfo> {
        if g2!(*&ferret.class.cleanup_time) { return None }
        let winners = unsafe {
            &*g2!(&raw const ferret.class.winners)
        };
        winners.get(&(kind, cls)).copied()
    }
    pub(super) fn query_candidate(kind: BufferKind, cls: BufferClass) -> D3dPtr {
        if !g2!(*&volatile const ferret.class.game_time) { return None }
        Self::seen_class(kind, cls).map(|(key, ..)| key)
    }
    const BIND_GEN_HYSTERESIS: u32 = 3;
    pub(super) fn can_expect_upcoming(kind: BufferKind, cls: BufferClass) -> Option<bool> {
        let Some((key, _, bcount, bgen)) = Self::seen_class(kind, cls) else {
            return None
        };
        if bcount == 0 {
            return None
        }

        let gen = Self::bind_generation();
        if gen.saturating_sub(Self::BIND_GEN_HYSTERESIS) > bgen {
            return None
        }

        let seen = Self::with_seen2(key, |buf| buf.bind_count >= bcount && !buf.is_bound(gen));
        match seen {
            Some(toolate) => Some(!toolate),
            None => None,
        }
    }
}
unsafe impl Sync for ClassShared {}
unsafe impl Send for ClassShared {}

impl ClassShared {
    pub(crate) fn iter_ui() -> impl Iterator<Item = (D3dNn, BufferInfo)> {
        let seen = unsafe {
            &*g2!(&raw const ferret.class.seen)
        };
        let now = Self::frame_count();
        let clsok = |cls: BufferClass| match cls {
            #[cfg(todo)]
            cls if cls == _target => true,
            BufferClass::Taimi => false,
            BufferClass::New =>
                false,
            _ => true,
        };
        let mut candidates = seen.iter()
            .filter(|(_k, buf)| buf.age() >= BufferInfo::TIME_AGE_UI && !buf.is_lost(now) && clsok(buf.classification))
            .map(|(k, buf)| (k.clone(), buf.clone()))
            .collect::<Vec<_>>();
        candidates.sort_by_key(|(_k, buf)| buf.sort_key());
        candidates.into_iter()
    }
    pub(super) unsafe fn query_buf<'a, F>(mut f: F) -> Option<(&'a D3dNn, &'a BufferInfo)> where
        F: FnMut(&BufferInfo) -> bool,
    {
        let seen = unsafe {
            &*g2!(&raw const ferret.class.seen)
        };
        let now = Self::frame_count();
        seen.iter()
            .filter(|(_k, buf)|
                f(buf)
                && !buf.is_lost(now)
            ).min_by_key(|(_k, buf)| buf.sort_key())
    }
    pub fn query_dv<'a>(cls: BufferClass) -> Option<InterfaceRef<'a, d3d11::ID3D11DepthStencilView>> {
        unsafe {
        Self::query_buf(|buf| matches!(buf.kind, BufferKind::DepthView) && buf.classification == cls)
        }
            .map(|(k, _)| unsafe {
                InterfaceRef::from_raw(*k)
            })
    }
    pub fn query_rt_by_dv<'a>(cls: BufferClass) -> Option<InterfaceRef<'a, d3d11::ID3D11RenderTargetView>> {
        let assoc = unsafe {
            Self::query_buf(|buf| matches!(buf.kind, BufferKind::DepthView) && buf.associated.is_some() && buf.classification == cls)
        }
            .and_then(|(_k, buf)| buf.associated)?;

        let seen = unsafe {
            &*g2!(&raw const ferret.class.seen)
        };
        let now = Self::frame_count();
        match seen.get(&assoc) {
            Some(buf) if matches!(buf.kind, BufferKind::RenderTarget) && !buf.is_lost(now) =>
                Some(unsafe {
                    InterfaceRef::from_raw(assoc)
                }),
            _ => None,
        }
    }

    pub fn manually_classify(key: D3dNn, cls: Option<BufferClass>) {
        Self::with_mut(|this| {
            let set = cls.is_some();
            let cls = cls.unwrap_or(BufferClass::Unknown);
            let (wkey, won, winner) = if let Some(buf) = this.seen.get_mut(&key) {
                let wkey = (buf.kind, buf.classification);
                let won = buf.state.winner;
                buf.classification = cls;
                buf.state.classification_score = match set {
                    true => BufferState::SCORE_MANUAL,
                    _ => 0,
                };
                let w = match (set, cls) {
                    (true, BufferClass::World | BufferClass::Target | BufferClass::Shadowbox | BufferClass::Reflection | BufferClass::Pretty | BufferClass::Fallback | BufferClass::Minimap) => {
                        buf.state.winner = true;
                        Some((key, buf.state.classification_score, buf.state.bind_count, buf.state.bind_generation))
                    },
                    _ => {
                        buf.state.winner = false;
                        None
                    },
                };
                (wkey, won, w)
            } else { return };
            if won {
                this.winners.remove(&wkey);
            }
            if let Some(winner) = winner {
                let newkey = (wkey.0, cls);
                this.winners.insert(newkey, winner);
            }
        });
    }

    #[inline(never)]
    fn bufs_dyn_mut(keys: &mut dyn Iterator<Item = D3dNn>, f: &mut dyn FnMut(&mut BufferInfo, D3dNn)) {
        let seen = unsafe {
            &mut *g2!(&raw mut ferret.class.seen)
        };
        for key in keys {
            let Some(buf) = seen.get_mut(&key) else { continue };
            f(buf, key)
        }
    }
    #[inline(always)]
    fn bufs_mut<F, I>(keys: I, mut f: F) where
        F: FnMut(&mut BufferInfo, D3dNn),
        I: IntoIterator<Item = D3dNn>,
    {
        let mut keys = keys.into_iter();
        Self::bufs_dyn_mut(&mut keys, &mut f)
    }
    #[inline(always)]
    #[cfg(todo)]
    fn bound_bufs_mut<F>(f: F) where
        F: FnMut(&mut BufferInfo, D3dNn),
    {
        Self::bufs_mut(Self::bound_views_keys(), f)
    }
    fn bound_bufs_all_mut<F>(mut f: F) where
        F: FnMut(&mut BufferInfo, D3dNn),
    {
        let seen = unsafe { &mut *g2!(&raw mut ferret.class.seen) };
        let bind_generation = ClassShared::bind_generation();

        for (key, buf) in seen {
            if !buf.state.is_bound(bind_generation) { continue }
            f(buf, *key);
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, strum::IntoStaticStr, strum::VariantArray)]
pub enum BufferClass {
    World,
    Target,
    Shadowbox,
    /// water surface afaict
    Reflection,
    UI,
    Misc,
    /// bloom or highlights or something
    Pretty,
    Fallback,
    FrameBuffer,
    Taimi,
    Unknown,
    Minimap,
    New,
    Antialiasing,
    Unsupported,
}
#[derive(Debug, Copy, Clone, PartialOrd, Ord, PartialEq, Eq)]
pub enum BufferKind {
    DepthView,
    RenderTarget,
    UnorderedAccessView,
    ShaderResourceView,
}
impl BufferKind {
    pub fn tag(self) -> &'static str {
        match self {
            Self::DepthView => "DV",
            Self::RenderTarget => "RT",
            Self::UnorderedAccessView => "UA",
            Self::ShaderResourceView => "SR",
        }
    }
}
#[derive(Debug, Clone)]
pub struct BufferInfo {
    pub classification: BufferClass,
    pub kind: BufferKind,
    pub last_seen: u32,
    pub first_seen: u32,
    pub resource: D3dPtr,
    pub associated: D3dPtr,
    pub format: dxgi::DXGI_FORMAT,
    pub size: (u32, u32),
    pub state: BufferState,
}
impl BufferInfo {
    const TIME_GONE: u32 = 32;
    const TIME_LOST: u32 = 3;
    const TIME_AGE_MIN: u32 = 2;
    const TIME_AGE_NEW: u32 = 64;
    const TIME_AGE_UI: u32 = 48;
    const TIME_AGE_RECLASSIFY: u32 = 128;

    const EMPTY_STATE: BufferState = BufferState::EMPTY;
    const EMPTY_SIZE: (u32, u32) = (0, 0);
    const EMPTY_FORMAT: dxgi::DXGI_FORMAT = DxgiFormat::UNKNOWN;

    pub fn size(&self) -> Option<(u32, u32)> {
        match self.size {
            Self::EMPTY_SIZE => None,
            size => Some(size),
        }
    }
    pub fn format(&self) -> Option<dxgi::DXGI_FORMAT> {
        match self.format {
            Self::EMPTY_FORMAT => None,
            format => Some(format),
        }
    }
    pub fn seen_since(&self, now: u32) -> u32 {
        now.wrapping_sub(self.last_seen)
    }
    pub fn age(&self) -> u32 {
        self.last_seen.wrapping_sub(self.first_seen)
    }
    #[inline]
    pub fn is_gone(&self, now: u32) -> bool {
        self.seen_since(now.wrapping_add(1)) >= Self::TIME_GONE
    }
    #[inline]
    pub fn is_lost(&self, now: u32) -> bool {
        self.seen_since(now.wrapping_add(1)) > Self::TIME_LOST
    }

    pub fn sort_key(&self) -> (BufferClass, bool, BufferKind, i32, u32, u32) {
        (
            self.classification,
            !self.state.winner,
            self.kind,
            -self.state.classification_score,
            !(self.last_seen / 2),
            !self.age(),
        )
    }

    fn reacquire<K>(seen: &BTreeMap<K, Self>, now: u32, key: &K, acquire: BufferClass) -> Option<K> where
        K: Clone + Eq + Ord,
    {
        let buf = seen.get(key);
        let reacquire = match buf {
            None => true,
            Some(buf) => buf.is_lost(now),
        };
        if !reacquire { return None }
        let mut candidates = seen.iter()
            .filter(|(_k, buf)| *_k != key && buf.classification == acquire && buf.age() >= BufferInfo::TIME_AGE_MIN && !buf.is_gone(now));
        if let Some((cand, _)) = candidates.next() {
            Some(cand.clone())
        } else {
            None
        }
    }

    fn prepare_state(&mut self, commit: bool, is_ingame: bool) {
        match self.state.associated {
            Some(assoc) if self.associated.is_none() => {
                // weak association, but also the only one!
                self.associated = Some(assoc);
            },
            _ => (),
        }
        let age = self.age();
        match self.classification {
            BufferClass::New if age >= Self::TIME_AGE_MIN =>
                self.classification = BufferClass::Unknown,
            BufferClass::Unknown if commit || (is_ingame && age <= BufferInfo::TIME_AGE_NEW) => {
                self.classify_commit(is_ingame);
            },
            BufferClass::Antialiasing => (),
            BufferClass::Unsupported | BufferClass::Taimi | BufferClass::UI => (),
            #[cfg(todo)]
            BufferClass::FrameBuffer => (),
            _ if commit && age > BufferInfo::TIME_AGE_RECLASSIFY && !self.winner && self.classification_score != BufferState::SCORE_MANUAL =>
                self.classify_commit(is_ingame),
            _ => (),
        }
    }
    pub fn clear_state(&mut self) {
        if self.state.associated.is_some() {
            self.associated = None;
        }
        self.state.clear();
    }

    fn record_bind_uavs(&mut self, uavs: &[Option<ID3D11UnorderedAccessView>]) {
        self.state.bind_count_uavs += uavs.iter().filter(|uav| uav.is_some()).count() as u32;
    }
    fn record_bind_depth(&mut self, _depth: Option<&DepthView>) {
        #[cfg(todo = "unnecessary")]
        if depth.is_none() {
            self.state.depth_binds_null += 1;
        }
    }
    fn record_association(&mut self, target: D3dNn, key: D3dNn, mut strong: Option<bool>) {
        match self.associated {
            None if strong != Some(true) && self.state.associated.is_none() =>
                self.state.associated = Some(target),
            None => {
                self.associated = Some(target);
                if self.state.associated == Some(target) {
                    self.state.associated = None;
                    strong = strong.or(Some(true));
                }
            },
            Some(assoc) if assoc == target =>
                strong = strong.or(Some(true)),
            Some(assoc) if self.state.associated != Some(assoc) =>
                self.state.associated = Some(assoc),
            _ => (),
        }
        if strong.unwrap_or(false) {
            unsafe { &mut *g2!(&raw mut ferret.class.assoc) }.insert(key, target);
        }
    }
    pub fn associated(&self) -> D3dPtr {
        self.associated.or(self.state.associated)
    }

    fn classify_commit(&mut self, is_ingame: bool) {
        if let Some((cls, score)) = self.classify(is_ingame) {
            self.classification = cls;
            self.state.classification_score = score;
        }
    }
    pub(crate) fn classify(&self, is_ingame: bool) -> Option<(BufferClass, i32)> {
        if self.classify_is_framebuffer() {
            let confidence = 0xff80i32 + self.classify_part_drawable() / 8;
            return Some((BufferClass::FrameBuffer, confidence))
        }

        let cls = self.classify_scores().max_by_key(|(_, score)| *score);
        match cls {
            Some((cls, score)) if score > 0 =>
                Some((cls, score)),
            _ => None,
        }
    }
    /// include confidence score
    pub(crate) fn classify_scores(&self) -> impl Iterator<Item = (BufferClass, i32)> {
        let base = [
            (BufferClass::Target, self.classify_score_target()),
            (BufferClass::World, self.classify_score_space()),
            (BufferClass::Shadowbox, self.classify_score_shadowbox()),
            (BufferClass::Reflection, self.classify_score_water()),
            (BufferClass::Misc, self.classify_score_misc()),
            (BufferClass::Antialiasing, self.classify_score_antialiasing()),
            #[cfg(todo)]
            (BufferClass::UI, self.classify_score_ui()),
        ];
        let size = self.size();
        let sized = size.map(|_size| [
            (BufferClass::Fallback, self.classify_score_fallback()),
            (BufferClass::Pretty, self.classify_score_pretty()),
            (BufferClass::Minimap, self.classify_score_minimap()),
            (BufferClass::Unsupported, self.classify_score_unsupported()),
        ]).into_iter().flatten();
        IntoIterator::into_iter(base).chain(sized)
            //.chain(size.is_none().then(|| (BufferClass::Unsupported, 64)))
    }
    fn classify_is_framebuffer(&self) -> bool {
        self.resource.map(|r| unsafe {
            let frame_buffers = &*g2!(&raw const ferret.frame_buffers);
            frame_buffers.contains(&r.as_ptr())
        }).unwrap_or(false)
    }
    /// framebuffer target used for UI/HUD drawn on top of game world
    fn classify_score_target(&self) -> i32 {
        let mut confidence = 0i32;
        if self.flags.contains(BufferStateFlags::DEPTH_STENCIL_REF) {
            confidence -= 32
        }
        if self.flags.contains(BufferStateFlags::CLEARED_COLOUR) {
            confidence -= 16
        }
        if !self.flags.contains(BufferStateFlags::CLEARED_DEPTH) {
            confidence -= 4
        }

        match self.depth_binds_count_disabled() {
            0 => confidence -= 2,
            disabled if disabled >= 5 || disabled >= self.depth_binds_count() / 2 =>
                confidence += 24 + 12,
            _ => confidence -= 24,
        }
        if self.flags.contains(BufferStateFlags::DEPTH_WRITE_LE) {
            confidence -= 6
        }
        match self.depth_binds_count_write() {
            0 => confidence -= 4,
            1 => confidence += 2,
            amt if amt >= self.depth_binds_count() / 4 =>
                confidence -= 16,
            _ => confidence -= 6,
        }
        let total_generations = ClassShared::bind_generation();
        if self.bind_generation <= total_generations / 2 {
            confidence -= 8
        }

        confidence += self.classify_part_drawable() / 2;

        confidence
    }
    fn classify_part_drawable(&self) -> i32 {
        let mut confidence = match self.size() {
            None => -4i32,
            Some((w, h)) => {
                let expected = g2!(*&ferret.display_size);
                let h = h as f32;
                let ratio_delta = ((expected.x / expected.y) - (w as f32 / h)).abs();
                match ratio_delta <= 2e-4f32 {
                    true => {
                        let sampled = (1.0f32 - h / expected.y).abs();
                        if sampled < 2e-4f32 {
                            // ~identical
                            8
                        } else if sampled <= 0.251 {
                            6
                        } else if sampled <= 0.501 {
                            4
                        } else {
                            -8
                        }
                    },
                    false => -24i32,
                }
            },
        };
        match self.classify_format_ok() {
            true => confidence += 2,
            false => confidence -= 16,
        }
        confidence
    }
    fn classify_score_space(&self) -> i32 {
        let mut confidence = match self.flags.contains(BufferStateFlags::CLEARED_DEPTH) {
            #[cfg(todo)]
            false => -64i32,
            false => -18i32,
            true => 4,
        };
        if self.flags.contains(BufferStateFlags::DEPTH_STENCIL_REF) {
            confidence -= 32
        }
        match (self.flags.contains(BufferStateFlags::CLEARED_COLOUR), self.clear_colour) {
            (true, [_, _, _, a]) if a != 0.0 => confidence -= 6,
            (true, colour) if vec32_eq(colour, [0.0f32; 4]) => confidence -= 2,
            (true, _) => confidence += 18,
            #[cfg(todo)]
            (false, _) => confidence -= 14,
            (false, _) => confidence -= 10,
        }
        match self.depth_binds_count_readonly() {
            0 => confidence -= 4,
            _ => confidence += 8,
        }
        let total_generations = ClassShared::bind_generation();
        if self.bind_generation >= total_generations / 2 {
            confidence += 8
        } else if self.bind_generation <= total_generations / 4 {
            confidence -= 4
        }
        match self.depth_binds_count_disabled() {
            0..=1 => confidence -= 4,
            disabled_count if disabled_count <= self.depth_binds_count() / 2 =>
                confidence += 6,
            _ => (),
        }
        if self.flags.contains(BufferStateFlags::DEPTH_WRITE_LE) {
            confidence += 6
        } else {
            confidence -= 4
        }
        confidence += (self.depth_binds_count_write() * 2) as i32;
        confidence += self.classify_part_drawable();
        confidence
    }
    fn classify_score_shadowbox(&self) -> i32 {
        let mut confidence = match self.flags.contains(BufferStateFlags::CLEARED_DEPTH) {
            false => -32i32,
            true => 6,
        };
        if self.flags.contains(BufferStateFlags::DEPTH_STENCIL_REF) {
            confidence -= 32
        }
        match self.flags.contains(BufferStateFlags::CLEARED_COLOUR) {
            true if vec32_eq(self.clear_colour, [0.0f32; 4]) => confidence += 16,
            true => confidence -= 24,
            false => confidence -= 8,
        }
        if self.depth_binds_count_readonly() > 0 {
            confidence -= 8
        }
        if self.flags.contains(BufferStateFlags::DEPTH_WRITE_LE) {
            confidence += 4
        } else {
            confidence -= 4
        }
        match self.depth_binds_count_disabled() {
            0 => (),
            1 => confidence += 2,
            disabled_count => confidence -= (disabled_count * 2) as i32,
        }
        confidence += (self.depth_binds_count_write() * 2) as i32;
        let total_generations = ClassShared::bind_generation();
        if self.bind_generation >= total_generations / 3 {
            confidence -= 8
        } else if self.bind_generation <= total_generations / 4 {
            confidence += 8
        }
        confidence += self.classify_part_drawable() / 2;
        confidence
    }
    fn classify_score_water(&self) -> i32 {
        let mut confidence = match self.flags.contains(BufferStateFlags::CLEARED_DEPTH) {
            false => -32i32,
            true => 6,
        };
        if self.flags.contains(BufferStateFlags::DEPTH_STENCIL_REF) {
            confidence -= 32
        }
        match self.flags.contains(BufferStateFlags::CLEARED_COLOUR) {
            true => confidence -= 16,
            false => confidence += 8,
        }
        if self.depth_binds_count_readonly() > 0 {
            confidence += 4
        }
        if self.flags.contains(BufferStateFlags::DEPTH_WRITE_LE) {
            confidence += 4
        } else {
            confidence -= 4
        }
        confidence -= (self.depth_binds_count_disabled() * 4) as i32;
        confidence += (self.depth_binds_count_write() * 2) as i32;
        let total_generations = ClassShared::bind_generation();
        if self.bind_generation >= total_generations / 3 {
            confidence -= 8
        } else if self.bind_generation <= total_generations / 4 {
            confidence += 8
        }
        confidence += self.classify_part_drawable() / 2;
        confidence
    }
    fn classify_score_pretty(&self) -> i32 {
        let mut confidence = match self.depth_binds_null {
            0..=2 => -48i32,
            #[cfg(todo)]
            0..=12 if !(3..=5).contains(self.bind_count) => x,
            null @ 8..=14 => 16i32 + (null as i32 * 4) - self.depth_binds_count() as i32,
            3..=4 | _ => -8i32,
        };
        if self.flags.contains(BufferStateFlags::DEPTH_STENCIL_REF) {
            confidence -= 32
        }
        if self.flags.contains(BufferStateFlags::CLEARED_COLOUR) {
            confidence -= 16i32;
        }
        if self.flags.contains(BufferStateFlags::CLEARED_DEPTH) {
            confidence -= 32i32;
        }
        if self.depth_binds_count_write() > 0 {
            confidence -= 8i32;
        }
        let expected_size = g2!(*&ferret.display_size);
        let (w, h) = self.size;
        if w * 2 < expected_size.x as u32 && h * 2 < expected_size.y as u32 {
            confidence -= 48i32;
        }
        confidence += self.classify_part_drawable();
        confidence
    }
    fn classify_score_fallback(&self) -> i32 {
        let mut confidence = match self.bind_count {
            2 if self.depth_binds_null == 4 && self.depth_binds_count == 4 =>
                12i32,
            _ => -48i32,
        };
        if self.flags.contains(BufferStateFlags::DEPTH_STENCIL_REF) {
            confidence -= 32
        }
        if self.flags.contains(BufferStateFlags::CLEARED_COLOUR) {
            confidence -= 16i32;
        }
        if self.flags.contains(BufferStateFlags::CLEARED_DEPTH) {
            confidence -= 32i32;
        }
        if self.depth_binds_count_write() > 0 {
            confidence -= 8i32;
        }
        let expected_size = g2!(*&ferret.display_size);
        let (w, h) = self.size;
        if w * 2 < expected_size.x as u32 && h * 2 < expected_size.y as u32 {
            confidence -= 48i32;
        }
        confidence += self.classify_part_drawable() * 3 / 2;
        confidence
    }
    fn classify_score_minimap(&self) -> i32 {
        let (w, h) = self.size;
        let mut confidence = match w == h {
            false => -128i32,
            true => match w {
                0 => -8i32,
                1..=64 => -32i32,
                240 => -64i32,
                65..=512 => match w % 32 {
                    0 => 54,
                    _ => 42,
                },
                _ => 16,
            },
        };
        if self.flags.contains(BufferStateFlags::DEPTH_STENCIL_REF) {
            confidence -= 32
        }
        match self.flags.contains(BufferStateFlags::CLEARED_COLOUR) {
            true if vec32_eq(self.clear_colour, [0.0f32; 4]) => confidence += 6,
            true => confidence -= 8,
            false => confidence -= 2,
        }
        match self.flags.contains(BufferStateFlags::CLEARED_DEPTH) {
            true => confidence += 12,
            false => confidence -= 8,
        }
        match self.depth_binds_count_write() {
            0 => confidence -= 8,
            _ => (),
        }
        match self.depth_binds_count_disabled() {
            0 => confidence -= 8,
            _ => (),
        }
        confidence
    }
    fn classify_score_misc(&self) -> i32 {
        let mut confidence = match self.depth_binds_null {
            0 => -32i32,
            null => -1i32 + (null as i32 * 2) - self.depth_binds_count() as i32,
        };
        confidence += self.bind_count.saturating_sub(1) as i32 * 4;
        let expected_size = g2!(*&ferret.display_size);
        let (w, h) = self.size;
        if w != 0 && w * 2 < expected_size.x as u32 && h * 2 < expected_size.y as u32 {
            confidence += 16;
        }
        let last_generation = ClassShared::bind_generation();
        if self.bind_generation > last_generation * 2 / 5 {
            confidence += 4;
        }
        confidence -= self.depth_binds_count_write() as i32 * 2;
        if !self.classify_format_ok() {
            confidence += 16;
        }
        confidence
    }
    fn classify_format_ok(&self) -> bool {
        match self.kind {
            BufferKind::RenderTarget => Self::is_rt_format_ok(self.format),
            BufferKind::DepthView => Self::is_dv_format_ok(self.format),
            _ => true,
        }
    }
    fn classify_score_unsupported(&self) -> i32 {
        let expected = g2!(*&ferret.display_size);
        let (w, h) = self.size;
        //dims.Width == expected.x as u32 && dims.Height == expected.y as u32
        match ((expected.x / expected.y) - (w as f32 / h as f32)).abs() <= 2e-4f32 {
            true => -32,
            false => {
                match self.classify_format_ok() {
                    false => -28,
                    true => 50,
                }
            },
        }
    }
    fn classify_score_antialiasing(&self) -> i32 {
        let mut confidence = 0i32;
        if self.flags.contains(BufferStateFlags::DEPTH_STENCIL_REF) {
            confidence -= 32
        }
        if !self.flags.contains(BufferStateFlags::CLEARED_COLOUR) {
            confidence -= 8
        }
        if !self.flags.contains(BufferStateFlags::CLEARED_DEPTH) {
            confidence -= 8
        }
        if self.flags.contains(BufferStateFlags::DEPTH_WRITE_LE) {
            confidence -= 6
        }
        match self.depth_binds_write {
            0 => confidence -= 8,
            1 => confidence -= 4,
            2 => confidence += 8,
            3 => (),
            amt if amt >= self.depth_binds_count() / 4 =>
                confidence -= 16,
            _ => confidence -= 6,
        }
        match self.bind_count {
            0 | 1 => confidence -= 4,
            2 => confidence += 6,
            3 => confidence -= 12,
            _ => confidence -= 24,
        }
        match self.depth_binds_disabled {
            6 => confidence += 12,
            10 => confidence += 6,
            4..=12 => confidence += 4,
            _ => confidence -= 12,
        }
        let last_generation = ClassShared::bind_generation();
        if self.bind_generation < last_generation * 5 / 6 {
            confidence -= 28;
        }

        confidence += self.classify_part_drawable() / 2 - 2;
        confidence
    }
    #[cfg(todo)]
    fn classify_score_antialiasing(&self) -> i32 {
        let mut score = -8;
        if let Some((w, h)) = self.size() {
        let expected = g2!(*&ferret.display_size);
            match ((expected.x / expected.y) - (w as f32 / h as f32)).abs() <= 2e-4f32 {
                true => score += 12,
                false => score -= 24,
            }
        }
        if self.bind_count > 1 {
            score -= 16;
        }
        let disabled = self.depth_binds_count_disabled();
        match disabled as i32 - 48 {
            so_many if so_many > 0 =>  {
                score += 42i32 + (so_many * 2) as i32;
            },
            not_as_many => score += not_as_many,
        }
        if self.depth_binds_write > 1 {
            score -= 18;
        }
        let last_generation = ClassShared::bind_generation();
        if self.bind_generation < last_generation * 3 / 4 {
            score -= 38;
        }
        score
    }
    #[cfg(todo)]
    fn classify_score_ui(&self) -> i32 {
        match self.depth_binds_null {
            0 => -32i32,
            null => (null as i32 * 2) - self.depth_binds_count() as i32,
        }
    }
    fn is_dv_format_ok(_format: dxgi::DXGI_FORMAT) -> bool {
        true
    }
    fn is_rt_format_ok(format: dxgi::DXGI_FORMAT) -> bool {
        match format {
            | dxgi::DXGI_FORMAT_A8_UNORM
            | dxgi::DXGI_FORMAT_R1_UNORM
            | dxgi::DXGI_FORMAT_R8_TYPELESS
            | dxgi::DXGI_FORMAT_R8_UNORM
            | dxgi::DXGI_FORMAT_R8_SNORM
            | dxgi::DXGI_FORMAT_R8_SINT
            | dxgi::DXGI_FORMAT_R8G8_TYPELESS
            | dxgi::DXGI_FORMAT_R8G8_UNORM
            | dxgi::DXGI_FORMAT_R8G8_SNORM
            | dxgi::DXGI_FORMAT_R8G8_SINT
            | dxgi::DXGI_FORMAT_R16_TYPELESS
            | dxgi::DXGI_FORMAT_R16_FLOAT
            | dxgi::DXGI_FORMAT_R16_UNORM
            | dxgi::DXGI_FORMAT_R16_UINT
            | dxgi::DXGI_FORMAT_R16_SNORM
            | dxgi::DXGI_FORMAT_R16_SINT
            | dxgi::DXGI_FORMAT_X24_TYPELESS_G8_UINT
            | dxgi::DXGI_FORMAT_D32_FLOAT_S8X24_UINT
            | dxgi::DXGI_FORMAT_R32_FLOAT_X8X24_TYPELESS
            | dxgi::DXGI_FORMAT_X32_TYPELESS_G8X24_UINT
            | dxgi::DXGI_FORMAT_R32G8X24_TYPELESS
            => false,
            f if f.0 >= dxgi::DXGI_FORMAT_BC1_TYPELESS.0 && f.0 <= dxgi::DXGI_FORMAT_BC5_SNORM.0 =>
                false,
            _ => true,
        }
    }
}
/// too lazy to `.state` everywhere...
impl ops::Deref for BufferInfo {
    type Target = BufferState;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

#[derive(Debug, Clone)]
pub struct BufferState {
    pub associated: D3dPtr,
    pub winner: bool,
    pub flags: BufferStateFlags,
    pub bind_count: u32,
    pub bind_count_uavs: u32,
    pub depth_binds: Vec<(DepthSummary, u32)>,
    pub bind_generation: u32,
    pub depth_generation: u32,
    pub depth_binds_count: u32,
    pub depth_binds_disabled: u32,
    pub depth_binds_write: u32,
    pub depth_binds_null: u32,
    pub depth_binds_readonly: u32,
    pub classification_score: i32,
    pub clear_colour: [f32; 4],
    #[cfg(todo = "unnecessary")]
    pub clear_depth: (u32, f32, u8),
}
impl BufferState {
    pub const EMPTY: Self = Self {
        associated: None,
        winner: false,
        flags: BufferStateFlags::empty(),
        bind_count: 0,
        bind_count_uavs: 0,
        depth_binds: Vec::new(),
        depth_binds_count: 0u32,
        depth_binds_disabled: 0u32,
        depth_binds_write: 0u32,
        depth_binds_null: 0u32,
        depth_binds_readonly: 0u32,
        bind_generation: u32::MAX,
        depth_generation: u32::MAX,
        classification_score: 0,
        clear_colour: [0.0f32; 4],
    };
    const SCORE_MANUAL: i32 = 0x07_ffff;
    pub fn clear(&mut self) {
        self.associated = None;
        self.flags = BufferStateFlags::empty();
        self.bind_count = 0;
        self.bind_count_uavs = 0;
        self.bind_generation = u32::MAX;
        self.depth_generation = self.bind_generation;
        self.depth_binds_count = 0;
        self.depth_binds_disabled = 0;
        self.depth_binds_write = 0;
        self.depth_binds_null = 0;
        self.depth_binds_readonly = 0;
        self.depth_binds.clear();
    }
    pub fn was_seen(&self) -> bool {
        self.bind_generation != u32::MAX
    }
    pub fn is_bound(&self, bind_generation: u32) -> bool {
        self.bind_generation == bind_generation
    }
    pub fn mark_unbound(&mut self, bind_generation: u32) -> bool {
        if self.bind_generation < bind_generation { return false }
        #[cfg(todo)]
        {
            self.flags.remove(BufferStateFlags::BOUND);
        }
        self.discard_depth();
        true
    }
    pub fn is_depth_bound(&self) -> bool {
        self.depth_generation == self.bind_generation
    }
    pub fn depth_binds_count_write(&self) -> u32 {
        (self.depth_binds_write + 1) / 2
    }
    pub fn depth_binds_count_readonly(&self) -> u32 {
        (self.depth_binds_readonly + 1) / 2
    }
    pub fn depth_binds_count_disabled(&self) -> u32 {
        (self.depth_binds_disabled + 1) / 2
    }
    pub fn depth_binds_count(&self) -> u32 {
        self.depth_binds_count
    }
    pub fn record_depth(&mut self, depth: DepthSummary, stencil_ref: u32) {
        if stencil_ref != 0 {
            self.flags.insert(BufferStateFlags::DEPTH_STENCIL_REF);
        }
        if depth != DepthSummary::Unbound {
            self.depth_binds_count += 1;
        }
        match depth {
            DepthSummary::Discard | DepthSummary::Disabled | DepthSummary::WriteOnly => (),
            DepthSummary::ReadWrite(ComparisonFunc::Le) =>
                self.flags.insert(BufferStateFlags::DEPTH_WRITE_LE),
            _ => (),
        }
        match depth {
            DepthSummary::Discard =>
                self.depth_binds_disabled += 1,
            DepthSummary::Disabled =>
                self.depth_binds_disabled += 2,
            DepthSummary::ReadOnly(ComparisonFunc::Le) =>
                self.depth_binds_readonly += 2,
            DepthSummary::ReadOnly(..) =>
                self.depth_binds_readonly += 1,
            DepthSummary::ReadWrite(ComparisonFunc::Le) =>
                self.depth_binds_write += 2,
            DepthSummary::ReadWrite(..) =>
                self.depth_binds_write += 1,
            DepthSummary::ViewNull =>
                self.depth_binds_null += 1,
            DepthSummary::Unbound =>
                (),
        }
        let entry = (depth, stencil_ref);
        if self.depth_generation != self.bind_generation {
            match self.depth_binds.last() {
                #[cfg(todo)]
                Some(prev) if prev == entry => {
                    // why not deduplicate I guess?
                    self.depth_generation = self.bind_generation;
                    return
                },
                _ => {
                    self.discard_depth();
                },
            }
        }
        self.depth_binds.push(entry);
    }
    fn discard_depth(&mut self) {
        self.depth_generation = self.bind_generation;
        match self.depth_binds.last() {
            None | Some((DepthSummary::Unbound, _)) => (),
            Some(..) => {
                self.depth_binds.push((DepthSummary::Unbound, 0));
            },
        }
    }
    pub fn mark_bound(&mut self, bind_generation: u32) -> bool {
        let prev = mem::replace(&mut self.bind_generation, bind_generation);
        let delta = self.bind_generation.wrapping_sub(prev);
        if delta > 0 {
            self.bind_count += 1;
            self.discard_depth();
        }
        #[cfg(todo = "unnecessary")]
        if delta == 1 {
            self.flags.insert(BufferStateFlags::REBOUND);
        }
        delta <= 1
    }
}
bitflags::bitflags! {
    #[derive(Debug, Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Hash)]
    pub struct BufferStateFlags: u16 {
        const CLEARED_COLOUR = 0x01;
        const CLEARED_DEPTH = 0x02;
        /// bound without any associated depth view
        const DEPTH_VIEW_NULL = 0x10;
        const DEPTH_WRITE_LE = 0x20;
        const DEPTH_STENCIL_REF = 0x80;
    }
}
#[derive(Debug, Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub enum DepthSummary {
    Unbound,
    ViewNull,
    Disabled,
    ReadOnly(ComparisonFunc),
    ReadWrite(ComparisonFunc),
}
#[allow(non_upper_case_globals)]
impl DepthSummary {
    pub const Discard: Self = Self::ReadOnly(ComparisonFunc::Always);
    pub const WriteOnly: Self = Self::ReadWrite(ComparisonFunc::Always);
}

#[derive(Debug, Clone, Default)]
pub struct GogglesClass {
    pub compat_present_count: bool,
    pub compat_clear_inconsistent: bool,
    pub active: bool,
}
impl GogglesClass {
    pub(super) fn act_render_post(&mut self) {
        ClassShared::with_mut(|c| c.reset_frame());
    }
    pub(super) fn act_pre_render(&mut self) {
        if GogglesShared::read_flags().contains(GogglesFlags::CLASS_FRAME_ONGOING) {
            ClassShared::mark_end_frame();
        }
    }
    pub(super) fn act_pre_render_frame(&mut self, ingame: bool) {
        if !ingame {
            g2!(*&volatile mut ferret.class.game_time = false);
        } else if self.active && !g2!(*&ferret.class.cleanup_time) {
            g2!(*&volatile mut ferret.class.game_time = true);
        }
    }
    pub(super) fn act_map_exit(&mut self) {
        g2!(*&volatile mut ferret.class.game_time = false);
        self.active = false;
    }
    pub(super) fn act_map_enter(&mut self) {
        g2!(*&volatile mut ferret.class.game_time = false);
        g2!(*&volatile mut ferret.class.cleanup_time = true);
        self.active = true;
    }
    #[inline]
    pub(super) fn enable(&mut self) {
        let counters = Self::STATS_COUNTERS.iter()
            .zip(Self::stats_counters_ref());
        for (desc, counter) in counters {
            counter.register(desc.clone());
        }

        g2!(*&volatile mut ferret.class.classify_tick = false);
        self.active = true;
    }
    #[inline]
    pub(super) fn disable(&mut self) {
        STATS_GAME_RENDER.reset(0);
        for desc in Self::STATS_COUNTERS {
            StatsRef::deregister(desc);
        }
        GogglesShared::clear_flags(GogglesFlags::CLASS_CLEARED_INCONSISTENT);

        // TODO: cleanup seen etc
        ClassShared::expect_present_count_ref().store(ClassShared::PRESENT_COUNT_DISABLED, GogglesShared::ENABLED_ORDERING);
        g2!(*&volatile mut ferret.class.classify_tick = false);
        g2!(*&volatile mut ferret.class.game_time = false);
        g2!(*&volatile mut ferret.class.frame_start = None);
        self.active = false;
        unsafe {
            let seen = &mut *g2!(&raw mut ferret.class.seen);
            seen.clear();
            let winners = &mut *g2!(&raw mut ferret.class.winners);
            winners.clear();
            let assoc = &mut *g2!(&raw mut ferret.class.assoc);
            assoc.clear();
        }
    }

    pub fn record_present_count(&mut self, count: u32) {
        let value = match count.wrapping_add(1) {
            _ if self.compat_present_count => ClassShared::PRESENT_COUNT_DISABLED,
            // if 0 is used as sentinel, record one behind as long as threshold allows for it
            ClassShared::PRESENT_COUNT_DISABLED => count,
            next => next,
        };
        ClassShared::expect_present_count_ref().store(value, GogglesShared::ENABLED_ORDERING);
    }

    #[inline(always)]
    pub(super) fn latest_frame_timestamp(&self) -> Option<Instant> {
        g2!(*&volatile const ferret.class.frame_start)
    }

    const STATS_COUNTERS: &[StatsDesc; 1] = {
        let sec = "stats-render";
        &[
            StatsDesc {
                detailed: true,
                .. StatsDesc::new(sec, "stats-render-time-game")
            },
        ]
    };
    fn stats_counters_ref() -> [StatsRef; 1] {
        [
            StatsRef::with_counter(&STATS_GAME_RENDER, StatsUnit::Time),
        ]
    }

    #[cfg(feature = "space")]
    pub(super) fn setup_engine(&mut self, engine: &Engine, _enables: GogglesEnables) {
        let rtv = &engine.render_backend.depth_handler.render_target_view.views;
        let dv = engine.render_backend.depth_handler.render_target_view.depth.as_ref();

        let now = ClassShared::bind_generation();
        let buf_rt = BufferInfo {
            classification: BufferClass::Taimi,
            associated: dv.map(|v| *v.as_d3d_raw()),
            resource: rtv.get_resource().ok().map(|r| *r.as_d3d_raw()),
            size: (engine.render_backend.display_size.width as _, engine.render_backend.display_size.height as _),
            format: BufferInfo::EMPTY_FORMAT,
            last_seen: now,
            first_seen: now,
            kind: BufferKind::RenderTarget,
            state: BufferState {
                classification_score: BufferState::SCORE_MANUAL,
                .. BufferInfo::EMPTY_STATE
            },
        };
        let buf_dv = dv.map(|dv| BufferInfo {
            classification: BufferClass::Taimi,
            associated: Some(*rtv.as_d3d_raw()),
            resource: dv.get_resource().ok().map(|r| *r.as_d3d_raw()),
            size: (engine.render_backend.display_size.width as _, engine.render_backend.display_size.height as _),
            format: BufferInfo::EMPTY_FORMAT,
            last_seen: now,
            first_seen: now,
            kind: BufferKind::RenderTarget,
            state: BufferState {
                classification_score: BufferState::SCORE_MANUAL,
                .. BufferInfo::EMPTY_STATE
            },
        });

        fn copy_buf(out: &mut BufferInfo, buf: &BufferInfo) {
            out.classification = buf.classification;
            out.associated = buf.associated;
            out.resource = buf.resource;
            out.size = buf.size;
            out.kind = buf.kind;
            out.state.classification_score = buf.state.classification_score;
        }
        ClassShared::with_mut(move |c| {
            match c.seen.entry(*rtv.as_d3d_raw()) {
                btree_map::Entry::Vacant(e) => {
                    e.insert(buf_rt);
                },
                btree_map::Entry::Occupied(e) =>
                    copy_buf(e.into_mut(), &buf_rt),
            }
            if let (Some(dv), Some(buf_dv)) = (dv, buf_dv) {
                match c.seen.entry(*dv.as_d3d_raw()) {
                    btree_map::Entry::Vacant(e) => {
                        e.insert(buf_dv);
                    },
                    btree_map::Entry::Occupied(e) =>
                        copy_buf(e.into_mut(), &buf_dv),
                }
            }
        });
    }

    pub fn compat_clear_inconsistent(&self) -> bool {
        GogglesShared::read_flags().contains(GogglesFlags::CLASS_CLEARED_INCONSISTENT)
    }
    pub fn set_compat_clear_inconsistent(&mut self, v: bool) {
        if v {
            GogglesShared::flags_insert(GogglesFlags::CLASS_CLEARED_INCONSISTENT);
            ClassShared::inconsistent_clears_ref().store(ClassShared::INCONSISTENT_CLEARS_THRESHOLD, GogglesShared::ENABLED_ORDERING);
        } else {
            GogglesShared::clear_flags(GogglesFlags::CLASS_CLEARED_INCONSISTENT);
        }
    }
}
static STATS_GAME_RENDER: StatsCounter = StatsCounter::DEFAULT;

impl super::GogglesState {
    #[inline(always)]
    pub fn is_classifying(&self) -> bool {
        self.active.intersects(ClassShared::ENABLES)
    }
    pub fn class_offer_clear_inconsistent(&self) -> bool {
        #[cfg(feature = "goggles2-project")]
        use crate::space::pack::render::Drawing;

        if self.class.compat_clear_inconsistent() { return true }
        if !self.is_classifying() { return false }
        #[cfg(feature = "goggles2-project")]
        if self.project.undrawn().contains(Drawing::SPACE) {
            return true
        }
        !GogglesShared::read_flags().contains(ClassShared::FLAGS_CLEARS)
    }
}
