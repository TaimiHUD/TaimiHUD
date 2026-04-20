use {
    super::{g2, class::{ClassShared, BufferClass, BufferKind, BufferStateFlags}, lens::LensShared, GogglesFlags, GogglesShared, GogglesState, D3dPtr, D3dNn},
    crate::{
        exports::runtime::{
            log::DeferredLogger,
            statistics::{StatsCounter, StatsUnit, StatsRef, StatsDesc},
        },
        render::machine::{frame_log, FrameState, RenderMachine},
        settings::goggles::GogglesEnables,
        space::{
            engine::{DrawDescGoggles, DrawDescSpace, FrameContext},
            pack::render::Drawing,
        },
        RENDER_STATE,
    },
    arcffi::nn,
    core::{
        ffi::c_void,
        mem,
        ptr::{self, NonNull},
        cell::LazyCell,
        fmt,
    },
    glam::Vec4,
    glamour::{Point2, Rect, Size2},
    std::sync::atomic::{AtomicPtr, AtomicU32, AtomicU8},
    std::time::Instant,
    taimi_d3d::dx11::{
        prelude::*,
        context::DeviceContext0,
        RenderTargetView,
        DepthView,
    },
    taimi_hoard::flags::{BitSlice, BitsNative},
    taimi_meta::ui::{LocalContext, MapOpen},
    taimi_hoard::lazyfmt,
};

pub struct ProjectShared {
    bound_render: AtomicPtr<c_void>,
    pub drawing: AtomicU32,
    pub map_open: AtomicU8,
    pub method: ProjectMethod,
}
impl ProjectShared {
    pub const EMPTY: Self = Self {
        bound_render: AtomicPtr::new(ptr::null_mut()),
        drawing: AtomicU32::new(0),
        map_open: AtomicU8::new(0),
        method: ProjectMethod::DEFAULT,
    };
    pub(super) fn reset_end() {
        STATS_PROJECT_RENDER.reset(0);
        Self::bound_render_ref().store(ptr::null_mut(), GogglesShared::FLAGS_ORDERING);
        let drawing = Self::read_drawing();
        let enabled = GogglesShared::enabled();
        if Drawing::SPACE.is_in(drawing) {
            if enabled.contains(GogglesEnables::PROJECT_SHADOWBOXING) {
                Self::insert_drawing(ProjectAction::Shadowbox.bit());
            }
            if enabled.contains(GogglesEnables::PROJECT_REFLECTIONS) {
                Self::insert_drawing(Drawing::REFLECT | Drawing::REFLECT_BELOW);
            }
        }
        let drawing_map = ProjectAction::DrawMinimap.is_in(drawing) | ProjectAction::DrawMap.is_in(drawing);
        if drawing_map && !enabled.contains(GogglesEnables::PROJECT_MAP) {
            Self::mask_drawing(ProjectAction::DrawMinimap.bit() | ProjectAction::DrawMap.bit());
        }
    }
    /// TODO: frame may become invalid immediately after
    /// (but prior to [Self::on_set_targets_pre])
    #[inline]
    pub(super) fn on_set_targets_prior() -> Option<(D3dPtr, D3dPtr)> {
        if !ClassShared::frame_valid() || Self::drawing().is_empty() { return None }
        Some((
            ClassShared::bound_render_primary_ptr(),
            ClassShared::bound_depth_ptr(),
        ))
    }
    pub(super) fn on_set_targets_pre(
        context: &Dx11Context,
        views: &[Option<RenderTargetView>],
        _depth: Option<&DepthView>,
        _uavs: &[Option<d3d11::ID3D11UnorderedAccessView>],
        (unbound_primary, unbound_dv): (D3dPtr, D3dPtr),
    ) {
        let prev_bound_render = Self::bound_render().or(unbound_primary);

        let rtvs = views.iter().filter_map(|v| v.as_ref());
        let (drawing, method) = (Self::drawing(), Self::method());

        let render_unbound = prev_bound_render.as_ref().and_then(|prev|
            rtvs.clone().all(|rtv| rtv.as_d3d_raw() != prev).then_some(*prev)
        );
        let render_unbound = render_unbound
            .and_then(|key| ClassShared::with_seen2(key, |buf|
                buf.winner.then_some((key, buf.classification))
            ).flatten());

        let rtvs =  rtvs.map(|rtv| rtv.as_d3d_raw());
        let render_view_interest = if views.len() > 1 {
            rtvs
                .clone()
                .filter_map(|&key| ClassShared::with_seen2(key, |buf| {
                    let score = match buf.classification {
                        _ if !buf.winner => 0xf,
                        BufferClass::World if ProjectAction::Draw.is_in(drawing) => 0,
                        BufferClass::Pretty | BufferClass::Fallback if ProjectAction::Draw.is_in(drawing) => 1,
                        BufferClass::Target => {
                            #[cfg(todo)]
                            if drawing_world.is_none() && buf.bind_count <= 1 && needs_world_fallback() {
                                // if we missed it, sneak in a draw before it gets blitted
                                if let Some(worldkey) = is_fallback_world_needed() {
                                    let sort = cmp::Reverse((None, None));
                                    drawing_world = Some((sort, worldkey));
                                }
                            }
                            2
                        },
                        BufferClass::Minimap if ProjectAction::DrawMinimap.is_in(drawing) => 3,
                        BufferClass::Shadowbox if ProjectAction::Shadowbox.is_in(drawing) => 4,
                        BufferClass::World => 5,
                        BufferClass::FrameBuffer => 5,
                        BufferClass::Reflection => 6,
                        _ => 7,
                    };
                    (key, buf.winner.then_some(buf.classification), score)
                }))
                    .min_by_key(|(_, _, score)| *score)
        } else { None };
        Self::set_bound_render(render_view_interest.map(|(k, ..)| k));
        let draws = ProjectAction::iter_bits(drawing).filter_map(|act|
            render_unbound.and_then(|(target, cls)|
                method.actions_on_unbind(act, cls)
                    .min()
                    .and_then(|(score, action)| {
                        let target = match action.retarget {
                            #[cfg(todo)]
                            Some(reach) if Some(reach) == rebound_cls => rebound,
                            Some(reach) if reach != cls => Self::reach_for(reach),
                            _ => Some((target, unbound_dv)),
                        };
                        target.map(|(target, dv)| ((cls, target), act, dv))
                    })
            )
        );
        let prev_drawn = if frame_log!(::is_enabled()) {
            frame_log!(;"project(on=SetTargets) drawing={drawing:#x} unbound={}", render_unbound.is_some());
            if prev_bound_render != unbound_primary {
                Self::log_view("prev", prev_bound_render);
            }
            Self::log_view("unbound", unbound_primary);
            Self::log_view("unbound", unbound_dv);
            drawing
        } else { !drawing };
        let mut drawing = drawing;
        for ((cls, target), act, dv) in draws {
            let target = unsafe {
                RenderTargetView::from_d3d_raw_ref(&target)
            };
            let dv = dv.as_ref().map(|dv| unsafe {
                DepthView::from_d3d_raw_ref(dv)
            });
            Self::draw(context.as_ref(), target, dv, cls, act);
            drawing &= !act.bit();
        }
        if drawing == prev_drawn {
            for (i, view) in views.iter().enumerate() {
                let view_ptr = view.as_ref().map(|v| *v.as_d3d_raw());
                Self::log_view_dyn(&format_args!("bound#{i}"), view_ptr);
            }
            Self::log_view("bound", _depth.map(|dv| *dv.as_d3d_raw()));
            for (i, uav) in _uavs.iter().enumerate() {
                let uav_ptr = uav.as_ref().and_then(|v| NonNull::new(v.as_raw()));
                Self::log_view_dyn(&format_args!("uav#{i}"), uav_ptr);
            }
        }
    }
    fn log_view_of(name: &dyn core::fmt::Display, view_ptr: D3dNn, buf: &super::class::BufferInfo) {
        let winner = buf.winner.then(|| ClassShared::seen_class(buf.kind, buf.classification)).flatten();
        let kind = buf.kind.tag();
        let is_winner = match winner {
            Some(..) => " WIN",
            _ => "",
        };
        frame_log!(;"project({name}={kind}): {view_ptr:?}({:?}) ({:?}={}{is_winner}) fmt={:#x}{:?} ({:?})", buf.resource, buf.classification, buf.classification_score, buf.format.0 as u32, buf.size, buf.state.flags);
        let bcount = lazyfmt::or_empty(winner.map(|(_, _, bcount, _)| lazyfmt::MaybeFmt::new(move |f| write!(f, "/{bcount}"))));
        let bgen = lazyfmt::or_empty(winner.map(|(_, _, _, bgen)| lazyfmt::MaybeFmt::new(move |f| write!(f, "/{bgen}"))));
        frame_log!(;";; binds@{}={}{bgen} (#{}{bcount}) dgen={}",
            ClassShared::bind_generation(), buf.state.bind_generation, buf.state.bind_count, buf.state.depth_generation,
        );
        frame_log!(;";; dbinds={} writes={}({}) ro={}({}) disabled={}({}) null={}",
            buf.state.depth_binds_count(),
            buf.state.depth_binds_count_write(), buf.state.depth_binds_write,
            buf.state.depth_binds_count_readonly(), buf.state.depth_binds_readonly,
            buf.state.depth_binds_count_disabled(), buf.state.depth_binds_disabled,
            buf.state.depth_binds_null,
        );
    }
    #[inline(always)]
    fn log_view(name: &str, view_ptr: D3dPtr) {
        Self::log_view_dyn(&name, view_ptr)
    }
    fn log_view_dyn(name: &dyn fmt::Display, view_ptr: D3dPtr) {
        let Some(view_ptr) = view_ptr else {
            frame_log!(;"project({name}): NULL");
            return
        };
        let found = ClassShared::with_seen2(view_ptr, |buf| {
            Self::log_view_of(&name, view_ptr, buf);
            if let Some(assoc) = buf.associated() {
                let aseen = ClassShared::with_seen2(assoc, |abuf| {
                    Self::log_view_of(&format_args!("{name}/assoc"), assoc, abuf);
                });
                if aseen.is_none() {
                    frame_log!(;"project({name}/assoc): {assoc:?}");
                }
            }
        });
        if found.is_none() {
            frame_log!(;"project({name}): {view_ptr:?}");
        }
    }
    fn reach_for(reach: BufferClass) -> Option<(D3dNn, Option<D3dNn>)> {
        let mut dv = None;
        let newtarget = ClassShared::seen_class(BufferKind::RenderTarget, reach)
            .map(|(key, ..)| key);
        if let Some(target) = newtarget {
            let was_seen = ClassShared::with_seen2(target, |buf| {
                dv = buf.associated();
                match buf.classification {
                    BufferClass::Minimap => buf.seen_since(ClassShared::frame_count()) <= 1,
                    _ => buf.was_seen(),
                }
            });
            if !was_seen.unwrap_or(false) {
                return None
            }
        }
        newtarget.map(|r| (r, dv))
    }
    pub(super) fn on_set_state_prior() -> Option<(
        Option<(Option<BufferClass>, u32, u32)>,
        D3dNn,
        Option<(BufferClass, u32, u32)>,
    )> {
        if !ClassShared::frame_valid() { return None }
        let rtv = (!Self::drawing().is_empty())
            .then(|| Self::bound_render().or(ClassShared::bound_render_primary_ptr()))
            .flatten();
        rtv.and_then(|rtv| {
            let dv = ClassShared::with_current_dv(|_k, buf|
                ((buf.winner.then_some(buf.classification), buf.depth_binds_count_write(), buf.depth_binds_count_disabled()))
            );
            let (empty, rt) = match &dv {
                None | Some((None, ..)) => {
                    let rt = ClassShared::with_seen2(rtv, |buf| buf.winner.then_some(
                        (buf.classification, buf.depth_binds_count_write(), buf.depth_binds_count_disabled() + buf.depth_binds_null)
                    )).flatten();
                    (rt.is_none(), rt)
                },
                Some(..) => (false, None),
            };
            (!empty).then_some((dv, rtv, rt))
        })
    }
    pub(super) fn on_set_state_pre(
        context: &Dx11Context,
        (depth_prior, rtv, rt_prior): (Option<(Option<BufferClass>, u32, u32)>, D3dNn, Option<(BufferClass, u32, u32)>),
    ) {
        let cls = if let Some((cls, ..)) = rt_prior {
            cls
        } else if let Some((Some(cls), ..)) = depth_prior {
            cls
        } else {
            return
        };
        let (drawing, method) = (Self::drawing(), Self::method());
        let dv = LazyCell::new(
            || ClassShared::bound_depth_ptr()
        );
        let after = LazyCell::new(|| dv.and_then(|dv|
                ClassShared::with_seen2(dv, |buf|
                    (buf.depth_binds_count_write(), buf.depth_binds_count_disabled(), buf.depth_binds_null, buf.flags)
                )
        ));
        let target_is_depthless = LazyCell::new(|| {
            let (has_written_depth, prev_disabled) = match (depth_prior, rt_prior) {
                // after enough depth-disabled writes, assume it's drawing UI
                (Some((_, writes, disabled)), _) if disabled.saturating_sub(writes) >= Self::DEPTHLESS_ENOUGH =>
                    return true,
                (_, Some((_, writes, disabled))) if disabled.saturating_sub(writes) >= Self::DEPTHLESS_ENOUGH =>
                    return true,
                // wait for at least one depth-enabled write...
                (Some((_, writes @ 0..=Self::DEPTHLESS_MAX_WRITE, disabled)), _) =>
                    (writes, disabled),
                #[cfg(todo = "unnecessary")]
                (_, Some((_, writes @ 0..=1, disabled))) =>
                    (writes, disabled),
                _ => return false,
            };
            let after = &*after;
            #[cfg(todo = "unnecessary")]
            let after = after.or_else(|| {
                // XXX: it doesn't currently seem to unbind halfway through write, so this is irrelevant...
                ClassShared::with_seen2(rtv, |buf|
                    (buf.depth_binds_count_write(), buf.depth_binds_count_disabled() + buf.depth_binds_null)
                )
            });
            // trigger on transition state
            match after {
                Some((0, _, ..)) => false,
                Some((_, 0..=1, ..)) | Some((1, 1..=2, ..)) => false,
                &Some((write, disabled, null, ..)) if write == has_written_depth =>
                    disabled + null > prev_disabled,
                _ => false,
            }
        });
        let target_is_depthless1 = LazyCell::new(|| {
            if !*target_is_depthless { return false }
            match *after {
                Some((0..=2, disabled, null, ..)) if disabled + null >= 4 => true,
                _ => false,
            }
        });
        let target_is_depthless2_target_map = LazyCell::new(|| {
            if !*target_is_depthless { return false }
            match *after {
                Some((0..=2, disabled, null, ..)) if disabled + null > 6 => true,
                _ => false,
            }
        });
        let target_is_depthless2_target_minimap = LazyCell::new(|| {
            // hopefully draws over minimap
            #[cfg(todo)]
            if !*target_is_depthless { return false }
            match *after {
                Some((0..=2, disabled, null, ..)) if disabled + null >= 4 => true,
                Some((_, disabled, null, ..)) if disabled + null >= 4 => true,
                _ => false,
            }
        });
        let target_is_depthless3 = LazyCell::new(|| {
            if !*target_is_depthless { return false }
            match *after {
                Some((0..=3, disabled, null, ..)) if disabled + null >= 5 => true,
                _ => false,
            }
        });

        let draws = ProjectAction::iter_bits(drawing).filter_map(|act|
            method.actions_on_state(act, cls)
                .min()
                .and_then(|(score, action)| {
                    if let Some((_, _, _, flags)) = *after {
                        if !flags.contains(BufferStateFlags::CLEARED_DEPTH) { return None }
                    }
                    match action.after_depthless {
                        0 => (),
                        1 if *target_is_depthless => (),
                        2 if *target_is_depthless2_target_map => (),
                        3 if *target_is_depthless2_target_map => (),
                        21 if *target_is_depthless2_target_minimap => (),
                        3 if *target_is_depthless3 => (),
                        _ => return None,
                    }
                    let target = match action.retarget {
                        #[cfg(todo)]
                        Some(reach) if Some(reach) == rebound_cls => rebound,
                        Some(reach) if reach != cls => Self::reach_for(reach),
                        _ => Some((rtv, *dv)),
                    };
                    target.map(|(target, dv)| (target, dv, act, cls))
                })
        );

        for (target, dv, act, cls) in draws {
            let rtv = unsafe {
                RenderTargetView::from_d3d_raw_ref(&target)
            };
            let dv = dv.as_ref().map(|dv| unsafe {
                DepthView::from_d3d_raw_ref(dv)
            });
            Self::draw(context.as_ref(), rtv, dv, cls, act);
        }
    }
    pub(super) fn on_new_frame() {
        let lock_blocking = match () {
            #[cfg(todo = "unnecessary")]
            _ => flags.contains(GogglesFlags::PROJECT_BLOCKING),
            _ => false,
        };

        let mut state_lock = match lock_blocking {
            true => RENDER_STATE.lock().ok(),
            false => RENDER_STATE.try_lock().ok(),
        };
        let state = match state_lock.as_mut().and_then(|s| s.as_mut()) {
            Some(state) => if state.machine.goggles.is_enabled(GogglesEnables::PROJECT_ENABLE) {
                Some(state)
            } else { None },
            _ => None,
        };
        if let Some(state) = state {
            let render_slot = (&mut state.engine,);
            state.machine.goggles_new_frame(render_slot);
        }
        drop(state_lock);
    }
    const DEPTHLESS_ENOUGH: u32 = 3;
    const DEPTHLESS_MAX_WRITE: u32 = 3;
    #[cfg(todo)]
    const EARLY_MIN_WRITE: u32 = Self::DEPTHLESS_MAX_WRITE;
    const EARLY_MIN_WRITE: u32 = 2;
    fn draw(
        context: &DeviceContext0,
        target: &RenderTargetView,
        dv: Option<&DepthView>,
        cls: BufferClass,
        what: Drawing,
    ) {
        #[cfg(todo = "unnecessary")]
        if let ProjectAction::DebugDetect = what {
            return Self::detect_clear(context, target)
        }
        let flags = GogglesShared::flags();
        let metrics_pre = Instant::now();
        let mut state_lock = match flags.contains(GogglesFlags::PROJECT_BLOCKING) {
            true => RENDER_STATE.lock().ok(),
            false => RENDER_STATE.try_lock().ok(),
        };
        let mut state = match state_lock.as_mut().and_then(|s| s.as_mut()) {
            Some(state) => if state.machine.goggles_project_draw_start(what, cls) {
                Some(state)
            } else { None },
            _ => None,
        };
        let draw = match (what, state.as_mut()) {
            (_, Some(state)) if state.machine.is_ingame_paused() => None,
            (ProjectAction::DrawMinimap, Some(state)) if cls == BufferClass::Target && state.machine.is_ui_hidden() =>
                None,
            (Drawing::REFLECT | Drawing::REFLECT_BELOW, Some(state)) if cls == BufferClass::Reflection && !state.machine.goggles.is_enabled(GogglesEnables::PROJECT_REFLECTIONS) =>
                None,
            (ProjectAction::DrawMinimap, ..) => Some(LocalContext::MINIMAP),
            (ProjectAction::DrawMap, ..) => Some(LocalContext::MAP),
            _ => Some(LocalContext::World),
        };
        let (Some(state), Some(draw)) = (state, draw) else {
            if state_lock.is_none() {
                log::debug!(logger: DeferredLogger::BEST_EFFORT, "project lost a race");
            }
            return
        };
        FrameState::TAIMI.publish_set();
        let engine = match &mut state.engine {
            Some(Ok(engine)) => if engine.project_proceed(&mut state.machine, draw) {
                Some(engine)
            } else { None },
            #[cfg(todo = "unnecessary")]
            _ if state.machine.goggles.project.debug_detect => {
                Self::detect_clear(context, target);
                None
            },
            _ => None,
        };
        let target_dv = match what {
            #[cfg(todo)]
            ProjectAction::DrawObscured if matches!(cls, BufferClass::Shadowbox) && state.machine.goggles.project.project_shadow =>
                None,
            ProjectAction::DrawObscured if !matches!(cls, BufferClass::Target | BufferClass::Fallback /*| BufferClass::Shadowbox*/) =>
                dv.map(|dv| *dv.as_d3d_raw()),
            #[cfg(todo)]
            ProjectAction::DrawMinimap if matches!(cls, BufferClass::Minimap | BufferClass::Target) =>
                dv.map(|dv| *dv.as_d3d_raw()),
            #[cfg(todo)]
            ProjectAction::DrawMap if matches!(cls, BufferClass::World) =>
                dv.map(|dv| *dv.as_d3d_raw()),
            Drawing::REFLECT | Drawing::REFLECT_BELOW | ProjectAction::Shadowbox if matches!(cls, BufferClass::Shadowbox | BufferClass::Reflection) =>
                dv.map(|dv| *dv.as_d3d_raw()),
            ProjectAction::Draw if !state.machine.goggles.is_enabled(GogglesEnables::LENS_ENABLE) => None,
            ProjectAction::Draw => {
                let current = match cls {
                    BufferClass::World => dv.and_then(|dv| ClassShared::with_seen2(*dv.as_d3d_raw(), |buf|
                        LensShared::buf_is_valid_ongoing(buf).then_some(*dv.as_d3d_raw())
                    ).flatten()),
                    _ => None,
                };
                current.or_else(|| LensShared::read_selected().and_then(|dv| ClassShared::with_seen2(dv, |buf|
                    LensShared::buf_is_valid_ongoing(buf).then_some(dv)
                ).flatten()))
            },
            _ => None,
        };
        let depth = target_dv.as_ref().map(|dv| unsafe {
            DepthView::from_d3d_raw_ref(dv)
        });
        if let Some(engine) = engine {
            let target_size = ClassShared::with_seen2(*target.as_d3d_raw(), |buf| buf.size()).flatten();
            let vp = target_size.map(|(w, h)| Rect::new(Point2::ZERO, Size2::new(w as f32, h as f32)));
            let desc = DrawDescGoggles {
                depth_filled: target_dv.is_some(),
                projecting: true,
                #[cfg(todo)]
                inherit: is_unique_sure_why_not_idk,
                .. DrawDescGoggles::with_buffers(vp, depth, Some(target))
            };
            let mut desc = desc.to_space();
            desc.goggles.buffer_compat = vp.map(|vp| vp.size) == Some(engine.render_backend.display_size) && depth.is_some();
            desc.pass = what;
            let mut wispy = false;
            match (what, cls, target_dv) {
                (ProjectAction::DrawObscured, ..) => {
                    desc.depth_write = false;
                    if desc.goggles.target_depthview.is_none() {
                        desc.goggles.target_depthview = LensShared::read_selected().and_then(|l| ClassShared::with_seen2(l, |buf|
                                    LensShared::buf_is_valid_ongoing(buf).then_some(l)
                        ).flatten());
                    }
                    desc.depth_read = desc.goggles.target_depthview.is_some();
                },
                (ProjectAction::Draw, BufferClass::World, Some(..))
                    => {
                    desc.depth_write = true;
                    desc.depth_read = true;
                    desc.goggles.buffer_compat = true;
                },
                (ProjectAction::Shadowbox, _, Some(..)) => {
                    desc.depth_write = true;
                    desc.depth_read = true;
                    desc.colour_read = false;
                    desc.colour_write = false;
                },
                (ProjectAction::Draw, BufferClass::Shadowbox, dv) => {
                    desc.depth_write = false;
                    desc.depth_read = dv.is_some();
                    wispy = true;
                },
                (Drawing::REFLECT | Drawing::REFLECT_BELOW, BufferClass::Reflection, dv) => {
                    desc.depth_write = false;
                    desc.depth_read = false;
                    //wispy = true;
                },
                (ProjectAction::DrawMinimap | ProjectAction::DrawMap, _, Some(..)) => {
                    desc.goggles.buffer_compat = false;
                    //desc.depth_write = true;
                    desc.depth_read = true;
                },
                (_, cls, Some(..)) => {
                    desc.depth_write = false;
                    desc.depth_read = true;
                },
                _ => {
                    desc.depth_write = false;
                    desc.depth_read = false;
                },
            }
            #[cfg(todo)]
            if target_dv.is_none() {
                if let Some(dv) = dv {
                    desc.goggles.target_depthview = Some(*dv.as_d3d_raw());
                    desc.depth_write = false;
                    desc.depth_read = false;
                    desc.stencil_read = false;
                    desc.stencil_write = false;
                    desc.goggles.buffer_compat = false;
                }
            }
            if let (ProjectAction::Draw, BufferClass::Pretty | BufferClass::Fallback) = (what, cls) {
                desc.depth_write = false;
            }
            if frame_log!(::is_enabled()) {
                frame_log!(;"project({what:?}@{cls:?}): (write={} read={})", desc.depth_write, desc.depth_read);
                Self::log_view("project", desc.goggles.target_renderview);
                Self::log_view("project", desc.goggles.target_depthview);
                Self::log_view("env", Self::bound_render().or(ClassShared::bound_render_primary_ptr()));
                Self::log_view("env", ClassShared::bound_depth_ptr());
            }
            if state.machine.goggles.project.debug_detect {
                Self::draw_detect_clear(context, target, &desc.goggles, draw);
                if !wispy {
                    engine.drawing.drawn.insert(what);
                    if !state.machine.goggles.project.debug_detect_all {
                        ProjectShared::mask_drawing(what.bit());
                    }
                }
            } else {
                if let LocalContext::Map(..) = draw {
                    state.machine.lastminute_mumblelink_update();
                }
                let wispy = wispy.then_some(engine.drawing.drawn.contains(what));
                if wispy.is_some() {
                    engine.drawing.drawing.insert(what);
                    engine.drawing.drawn.remove(what);
                }
                let pass = desc.pass;
                engine.render_carefully(&mut state.machine, context, desc, draw);
                let succ = engine.drawing.drawn.contains(what);
                if let Some(prev) = wispy {
                    engine.drawing.drawn.set(what, prev);
                } else if succ {
                    ProjectShared::mask_drawing(what.bit());
                }
            }
        }
        state.machine.goggles_project_draw_end();
        drop(state_lock);
        FrameState::TAIMI.publish_clear();
        let amt = metrics_pre.elapsed().as_micros() as u64;
        STATS_PROJECT_RENDER.increment(amt);
    }
    const DETECT_COLOUR: Vec4 = Vec4::new(64.0 / 255.0, 224.0 / 255.0, 208.0 / 255.0, 0.7);
    /// TODO: scissor+quad when more representative, depth writes, etc
    fn draw_detect_clear(
        context: &DeviceContext0,
        target: &RenderTargetView,
        _desc: &DrawDescGoggles,
        _ctx: LocalContext,
    ) {
        Self::detect_clear(context, target);
    }
    fn detect_clear(
        context: &DeviceContext0,
        target: &RenderTargetView,
    ) {
        target.clear_rgba(context, Self::DETECT_COLOUR);
    }

    #[inline(always)]
    pub fn bound_render_ref() -> &'static AtomicPtr<c_void> {
        unsafe {
            &*g2!(&raw const ferret.project2.bound_render)
        }
    }
    fn bound_render() -> D3dPtr {
        NonNull::new(
            Self::bound_render_ref().load(GogglesShared::FLAGS_ORDERING)
        )
    }
    fn set_bound_render(rtv: D3dPtr) {
        Self::bound_render_ref().store(nn::nonnull_ptr_mut(rtv), GogglesShared::FLAGS_ORDERING);
    }

    #[inline(always)]
    pub fn drawing_ref() -> &'static AtomicU32 {
        unsafe {
            &*g2!(&raw const ferret.project2.drawing)
        }
    }
    fn read_drawing() -> Drawing {
        Drawing::from_bits_retain(
            Self::drawing_ref().load(GogglesShared::ENABLED_ORDERING)
        )
    }
    fn write_drawing(v: Drawing) {
        Self::drawing_ref().store(v.bits(), GogglesShared::ENABLED_ORDERING);
    }

    fn drawing() -> Drawing {
        Drawing::from_bits_retain(
            Self::drawing_ref().load(GogglesShared::FLAGS_ORDERING)
        )
    }
    fn insert_drawing(v: Drawing) -> Drawing {
        Drawing::from_bits_retain(
            Self::drawing_ref().fetch_or(v.bits(), GogglesShared::FLAGS_ORDERING)
        )
    }
    fn mask_drawing(v: Drawing) -> Drawing {
        Drawing::from_bits_retain(
            Self::drawing_ref().fetch_and(!v.bits(), GogglesShared::FLAGS_ORDERING)
        )
    }
    pub(super) fn wants_draw() -> bool {
        !Self::drawing().is_empty()
    }

    fn method() -> ProjectMethod {
        g2!(*&ferret.project2.method)
    }
    fn write_method(method: ProjectMethod) {
        g2!(*&volatile mut ferret.project2.method = method)
    }
    fn read_method() -> ProjectMethod {
        g2!(*&volatile const ferret.project2.method)
    }

    #[inline(always)]
    pub fn map_open_ref() -> &'static AtomicU8 {
        unsafe {
            &*g2!(&raw const ferret.project2.map_open)
        }
    }
    fn map_open() -> (bool, bool) {
        let v = Self::map_open_ref().load(GogglesShared::FLAGS_ORDERING);
        ((v & 1) != 0, (v & 2) != 0)
    }
    fn write_map_open((open, anim): (bool, bool)) {
        let v = open as u8 | u8::from(anim) << 1;
        Self::map_open_ref().store(v, GogglesShared::ENABLED_ORDERING);
    }

    pub(super) fn has_drawn_space() -> bool {
        !ProjectAction::Draw.is_in(Self::drawing())
    }
}
static STATS_PROJECT_RENDER: StatsCounter = StatsCounter::DEFAULT;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, strum::IntoStaticStr, strum::VariantArray)]
pub enum ProjectMethod {
    Fuzzy,
    Early,
    Late,
    Shiny,
    Pretty,
    Conservative,
    Compatibility,
}
impl ProjectMethod {
    pub const DEFAULT: Self = match () {
        _ => Self::Fuzzy,
        #[cfg(todo)]
        _ => Self::Late,
    };

    fn actions_on_unbind(self, act: Drawing, cls: BufferClass) -> impl Iterator<Item = (i32, MethodAction)> {
        let primary = match (act, cls, self) {
            (Drawing::OBSCURED_SHADOWED, BufferClass::Shadowbox, ProjectMethod::Early | ProjectMethod::Fuzzy) =>
                Some((0, None)),
            (ProjectAction::DrawObscured, BufferClass::World, ProjectMethod::Early | ProjectMethod::Fuzzy)
                if ClassShared::with_seen_class(BufferKind::DepthView, BufferClass::World, |_k, buf| buf.bind_count <= 1).unwrap_or(true)
                => None,
            #[cfg(todo)]
            (ProjectAction::DrawObscured, BufferClass::Fallback, ProjectMethod::Conservative) =>
                Some((0x10, None)),
            (ProjectAction::DrawObscured, BufferClass::Pretty, ProjectMethod::Shiny | ProjectMethod::Late) =>
                Some((0, None)),
            (ProjectAction::DrawObscured, BufferClass::World | BufferClass::Target, method) => {
                let method = match method {
                    m @ (ProjectMethod::Shiny | ProjectMethod::Pretty) => m,
                    _ => ProjectMethod::Late,
                };
                return method.actions_on_unbind(ProjectAction::Draw, cls)
            },
            (ProjectAction::DrawObscured, ..) => None,
            (ProjectAction::DrawMinimap, cls, method) => {
                let method = ProjectMethodMinimap::from(method);
                let next = match method {
                    // cheat because we don't have this already...
                    ProjectMethodMinimap::Eager =>
                        ClassShared::with_current_dv(|_, buf| buf.winner.then_some(buf.classification)).flatten(),
                    _ => None,
                }.unwrap_or(BufferClass::Unknown);
                return method.minimap_action_on_unbind(cls, next).map(|a| (0, a)).into_iter()
            },
            (ProjectAction::DrawMap, cls, method) => {
                let method = ProjectMethodMap::from(method);
                let (next, next_bind_count) = match (method, cls) {
                    // cheat because we don't have this already...
                    (ProjectMethodMap::Reliable | ProjectMethodMap::Eager, BufferClass::World) =>
                        ClassShared::with_current_dv(|_, buf|
                            buf.winner.then_some(buf.classification)
                            .map(|c| (c, buf.bind_count))
                        ).flatten(),
                    _ => None,
                }.unwrap_or((BufferClass::Unknown, 0));
                return method.map_action_on_unbind(cls, next, next_bind_count).map(|a| (0, a)).into_iter()
            },

            (_, BufferClass::Fallback, _) if ClassShared::bind_generation() <= 8 => None,
            (_, BufferClass::Pretty, _) if ClassShared::bind_generation() <= 12 => None,
            (Drawing::REFLECT | Drawing::REFLECT_BELOW, BufferClass::Reflection, _) => Some((0x08, None)),
            (ProjectAction::Draw, BufferClass::Shadowbox, Self::Pretty) => Some((0x10, None)),
            (ProjectAction::Shadowbox, _, Self::Shiny)
                if ClassShared::with_current_dv(|_, buf| buf.bind_count > 1 && buf.winner && matches!(buf.classification, BufferClass::World)).unwrap_or(false)
                => Some((0x10, Some(BufferClass::Shadowbox))),
            (ProjectAction::Shadowbox, BufferClass::Shadowbox, Self::Pretty) => None,
            (ProjectAction::Draw, BufferClass::Pretty, Self::Pretty) =>
                Some((0, None)),
            (ProjectAction::Draw, _, Self::Pretty, ) => None,
            (ProjectAction::Draw, _, Self::Conservative, ) => None,
            (ProjectAction::Draw, BufferClass::Pretty, Self::Fuzzy | Self::Compatibility) if ClassShared::with_current_dv(|_, buf| buf.winner && matches!(buf.classification, BufferClass::Target)).unwrap_or(false) => {
                // TODO: wouldn't be unreasonable for something to be bound in-between these two though...
                // in which case, reach to pretty prior? idk
                Some((0x80, None))
            },
            (ProjectAction::Draw, _, Self::Late)
                if ClassShared::with_current_dv(|_, buf| buf.winner && matches!(buf.classification, BufferClass::Target)).unwrap_or(false) =>
                Some((8, Some(BufferClass::World))),
            (ProjectAction::Draw, BufferClass::World, Self::Compatibility)
                if ClassShared::with_seen_class(BufferKind::DepthView, BufferClass::World, |_k, buf| buf.depth_binds_count_write() <= ProjectShared::DEPTHLESS_MAX_WRITE).unwrap_or(true)
                => None,
            (ProjectAction::Draw, BufferClass::World, Self::Early)
                if ClassShared::with_seen_class(BufferKind::DepthView, BufferClass::World, |_k, buf| buf.depth_binds_count_write() <= ProjectShared::EARLY_MIN_WRITE).unwrap_or(true)
                => None,
            (ProjectAction::Draw, BufferClass::World, Self::Fuzzy | Self::Shiny)
                if ClassShared::with_seen_class(BufferKind::DepthView, BufferClass::World, |_k, buf| buf.depth_binds_count_write() <= 1).unwrap_or(true)
                => None,
            (act, cls, Self::Fuzzy | Self::Conservative | Self::Compatibility | Self::Shiny | Self::Pretty)
                if matches!((act, cls), (ProjectAction::Draw, BufferClass::World) | (ProjectAction::DrawMinimap, BufferClass::Minimap))
                => Some((0x20, None)),
            (ProjectAction::Draw, BufferClass::Fallback, _)
                if ClassShared::can_expect_upcoming(BufferKind::RenderTarget, BufferClass::World) != Some(true)
                => Some((0xff, None)),
            (ProjectAction::Shadowbox, BufferClass::Shadowbox, _)
                => Some((0x08, None)),
            _ => None,
        };
        primary
            .map(|(score, retarget)| (score, MethodAction {
                retarget,
                .. Default::default()
            }))
        .into_iter()
        //.chain(fallback)
    }
    fn actions_on_state(self, act: Drawing, cls: BufferClass) -> impl Iterator<Item = (i32, MethodAction)> {
        let primary = match (act, cls, self) {
            #[cfg(todo)]
            (ProjectAction::DrawObscured, ..) => None,
            #[cfg(todo)]
            (ProjectAction::DrawMinimap, cls, method) =>
                return ProjectMethodMinimap::from(method).minimap_action_on_state(cls).map(|a| (0, a)).into_iter(),
            #[cfg(todo)]
            (ProjectAction::DrawMap, cls, method) =>
                return ProjectMethodMap::from(method).map_action_on_state(cls).map(|a| (0, a)).into_iter(),
            (ProjectAction::DrawMinimap | ProjectAction::DrawMap, ..) => None,
            #[cfg(todo)]
            (_, _, Self::Shiny) => None,
            (Drawing::REFLECT | Drawing::REFLECT_BELOW, BufferClass::Reflection, Self::Pretty) => Some((0x04, None)),
            (ProjectAction::Draw, BufferClass::World, Self::Early | Self::Shiny)
                if ClassShared::with_current_dv(|_, buf| buf.depth_binds_count_disabled() > 1 || buf.depth_binds_count_readonly() >= 1).unwrap_or(false)
                => Some((0x08, None)),
            (ProjectAction::Draw, BufferClass::Target, Self::Conservative)
            | (ProjectAction::DrawObscured, BufferClass::Target, _)
                => Some((0, None)),
            (ProjectAction::DrawMinimap | ProjectAction::DrawMap, BufferClass::Target, Self::Conservative)
            | (ProjectAction::DrawMap, BufferClass::Target, Self::Late | Self::Compatibility)
            | (ProjectAction::DrawMinimap, BufferClass::Target, Self::Compatibility)
                => Some((0, None)),
            _ => None,
        };
        primary
            .map(move |(score, retarget)| (score, MethodAction {
                retarget,
                after_depthless: (cls != BufferClass::World).then_some(1).unwrap_or(0),
                .. Default::default()
            }))
        .into_iter()
        //.chain(fallback)
    }
}
impl Default for ProjectMethod {
    #[inline(always)]
    fn default() -> Self { Self::DEFAULT }
}
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, strum::IntoStaticStr, strum::VariantArray)]
pub enum ProjectMethodMinimap {
    Eager,
    /// reliable but delayed by 1 frame
    Slow,
    #[cfg(todo)]
    Conservative,
    #[cfg(todo)]
    SlowShiny,
}
impl ProjectMethodMinimap {
    fn minimap_action_on_unbind(self, prior: BufferClass, next: BufferClass) -> Option<MethodAction> {
        match self {
            Self::Eager if matches!(next, BufferClass::Target) => Some(MethodAction {
                retarget: Some(BufferClass::Minimap),
                .. Default::default()
            }),
            Self::Slow if matches!(prior, BufferClass::Minimap) => Some(MethodAction {
                .. Default::default()
            }),
            _ => None,
        }
    }
    #[cfg(todo)]
    fn minimap_action_on_state(self, cls: BufferClass) -> Option<MethodAction> {
        match (cls, self) {
            #[cfg(todo)]
            (BufferClass::Target, Self::Conservative) => todo(),
            (BufferClass::Minimap, Self::SlowShiny) => todo(),
            _ => None,
        }
    }
}
impl From<ProjectMethod> for ProjectMethodMinimap {
    fn from(method: ProjectMethod) -> Self {
        match method {
            ProjectMethod::Compatibility
                => Self::Slow,
            #[cfg(todo)]
            | ProjectMethod::Late
                => Self::SlowShiny,
            | ProjectMethod::Fuzzy | ProjectMethod::Shiny | ProjectMethod::Pretty
            | ProjectMethod::Early
                => Self::Eager,
            _ => Self::Slow,
        }
    }
}
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, strum::IntoStaticStr, strum::VariantArray)]
pub enum ProjectMethodMap {
    Eager,
    Reliable,
    #[cfg(todo)]
    Conservative,
    #[cfg(todo)]
    Shiny,
}
impl ProjectMethodMap {
    fn map_action_on_unbind(self, prior: BufferClass, next: BufferClass, next_bind_count: u32) -> Option<MethodAction> {
        match (prior, next, self) {
            (BufferClass::World, BufferClass::Target, Self::Reliable | Self::Eager)
                if next_bind_count > 1
                => Some(MethodAction {
                    .. Default::default()
                }),
            (BufferClass::World, _, Self::Eager)
                if ClassShared::with_seen_class(BufferKind::DepthView, BufferClass::Target, |_, buf| buf.bind_count >= 1).unwrap_or(false)
                => Some(MethodAction {
                    .. Default::default()
                }),
            _ => None,
        }
    }
    #[cfg(todo)]
    fn map_action_on_state(self, cls: BufferClass) -> Option<MethodAction> {
        let map_open = ProjectShared::map_open();
    }
}
impl From<ProjectMethod> for ProjectMethodMap {
    fn from(method: ProjectMethod) -> Self {
        match method {
            #[cfg(todo)]
            ProjectMethod::Conservative
                => Self::Conservative,
            ProjectMethod::Compatibility
                => Self::Reliable,
            | ProjectMethod::Early | ProjectMethod::Late
                => Self::Eager,
            #[cfg(todo)]
            | ProjectMethod::Fuzzy | ProjectMethod::Shiny | ProjectMethod::Pretty
                => Self::Shiny,
            _ => Self::Reliable,
        }
    }
}

struct TrackingContext {
    map_open: MapOpen,
}
enum FrameStage {
    World,
    PostProc,
    BlitSpace,
    BlitMap,
}
trait FrameLifecycleInfo {
    fn ongoing(&self) -> bool;
    fn world_clear(&self) -> Option<[f32; 4]>;
    fn bind_count(&self, kind: BufferKind, cls: BufferClass) -> (u32, bool);
    fn bind_seen(&self, kind: BufferKind, cls: BufferClass) -> bool {
        self.bind_count(kind, cls).0 > 0
    }
    /// if world rebound after Target, map or something likely being drawn on top
    fn world_in_pt2(&self) -> bool;
}
struct SharedInfo;
impl FrameLifecycleInfo for SharedInfo {
    fn ongoing(&self) -> bool {
        ClassShared::frame_valid()
    }
    fn world_clear(&self) -> Option<[f32; 4]> {
        ClassShared::with_seen_class(BufferKind::RenderTarget, BufferClass::World, |_k, buf|
            buf.state.flags.contains(BufferStateFlags::CLEARED_COLOUR).then_some(buf.state.clear_colour)
        ).flatten()
    }
    fn bind_count(&self, kind: BufferKind, cls: BufferClass) -> (u32, bool) {
        ClassShared::with_seen_class(kind, cls, |_k, buf| {
            let bound = buf.is_bound(ClassShared::bind_generation());
            (buf.bind_count, bound)
        }).unwrap_or((0, false))
    }
    fn world_in_pt2(&self) -> bool {
        match self.bind_count(BufferKind::RenderTarget, BufferClass::Target) {
            (0, _) => false,
            (_, false) if self.bind_seen(BufferKind::RenderTarget, BufferClass::Minimap) => false,
            (bc, false) => bc >= 1,
            _ => false,
        }
    }
}

/// TODO: use a state machine to track eligibility over time
/// (forget an enum, dyn trait list produced at start of frame, likelihood of completion can weight score, etc)
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
struct MethodAction {
    #[cfg(todo = "unnecessary")]
    what: Drawing,
    retarget: Option<BufferClass>,
    after_depthless: u8,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, strum::IntoStaticStr, strum::VariantArray)]
#[repr(u8)]
#[cfg(deleteme)]
pub enum ProjectAction {
    Nop = 1,
    DebugDetect,
    DrawObscured,
    Draw,
    DrawMinimap,
    DrawMap,
    Shadowbox,
}
#[cfg(deleteme)]
impl ProjectAction {
    #[inline(always)]
    pub const fn index(self) -> u8 {
        self as _
    }
    #[inline]
    pub const fn bit(self) -> u16 {
        1u16 << self.index() as u32
    }
    pub fn iter_bits(mut bits: u16) -> impl Iterator<Item = Self> {
        iter::from_fn(move || {
            while bits != 0 {
                let bit = bits.trailing_zeros();
                bits &= !1u16.unbounded_shl(bit);
                if let Some(v) = Self::from_index(bit as _) {
                    return Some(v)
                }
            }
            None
        })
    }
    const INDEX_MIN: u8 = Self::Nop.index() as _;
    const INDEX_MAX: u8 = Self::Shadowbox.index() as _;
    pub const fn from_index(index: u8) -> Option<Self> {
        match index {
            Self::INDEX_MIN..=Self::INDEX_MAX => Some(unsafe {
                Self::from_index_unchecked(index)
            }),
            _ => None,
        }
    }
    #[inline]
    pub const fn is_in(self, bits: u16) -> bool {
        bits & self.bit() != 0
    }
    #[inline(always)]
    pub const unsafe fn from_index_unchecked(index: u8) -> Self {
        mem::transmute(index)
    }
}
/// TODO: deleteme
pub struct ProjectAction;
/// TODO: deleteme
#[allow(non_upper_case_globals)]
impl ProjectAction {
    const DrawMap: Drawing = Drawing::GLOBALMAP;
    const DrawMinimap: Drawing = Drawing::MINIMAP;
    const Draw: Drawing = Drawing::SPACE;
    const Shadowbox: Drawing = Drawing::SHADOWBOX;
    const DrawObscured: Drawing = Drawing::OBSCURED;
    #[inline(always)]
    pub fn iter_bits(bits: Drawing) -> impl Iterator<Item = Drawing> {
        bits.iter_passes()
    }
}
/// TODO: deleteme
impl Drawing {
    #[deprecated] #[inline(always)]
    fn is_in(self, other: Self) -> bool { other.contains(self) }
    #[deprecated] #[inline(always)]
    fn bit(self) -> Self { self }
    #[cfg(deleteme)]
    fn to_pass(self) -> u32 {
        match self.get_pass() {
            Drawing::GLOBALMAP => DrawDescSpace::PASS_MAP,
            Drawing::MINIMAP => DrawDescSpace::PASS_MINIMAP,
            Drawing::OBSCURED => DrawDescSpace::PASS_OBSCURED,
            Drawing::OBSCURED_SHADOWED => DrawDescSpace::PASS_OBSCURED_SHADOWED,
            Drawing::SHADOWBOX => DrawDescSpace::PASS_SHADOWBOXING,
            Drawing::REFLECT => DrawDescSpace::PASS_REFLECTING,
            Drawing::REFLECT_BELOW => DrawDescSpace::PASS_REFLECTING_BELOW,
            Drawing::SPACE | _ => DrawDescSpace::PASS_SPACE,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct GogglesProject {
    pub debug_detect: bool,
    pub debug_detect_all: bool,
    pub project_depth_fill: bool,
    pub project_viewport_force: bool,
    pub project_shadow: bool,
    pub project_shadowbox: bool,
    pub project_blend_force: bool,
    pub project_projecting: bool,
    drawing_mask: Drawing,
}
impl GogglesProject {
    /// TODO: here so UI can display debug stats prior to reset bleh...
    pub(super) fn act_render_post_late(&mut self) {
        ProjectShared::write_drawing(mem::replace(&mut self.drawing_mask, Drawing::empty()));
    }
    /// TODO: deleteme idk this is probably pointless now
    pub(crate) fn act_pre_render_frame(&mut self, _has_context: bool, drawing: &mut FrameContext) {
        //self.is_drawing = machine.frame_context.is_some();
        let is_drawing = drawing.prepared && drawing.is_drawing();
        if is_drawing {
            self.drawing_mask = drawing.drawing;
            ProjectShared::write_map_open(drawing.map_anim.shape());
        } else {
            self.drawing_mask = Drawing::empty();
        }
    }
    const STATS_COUNTERS: &[StatsDesc; 1] = {
        let sec = "stats-render";
        &[
            StatsDesc {
                detailed: true,
                .. StatsDesc::new(sec, "stats-render-time-project")
            },
        ]
    };
    fn stats_counters_ref() -> [StatsRef; 1] {
        [
            StatsRef::with_counter(&STATS_PROJECT_RENDER, StatsUnit::Time),
        ]
    }
    pub(super) fn enable(&mut self) {
        let counters = Self::STATS_COUNTERS.iter()
            .zip(Self::stats_counters_ref());
        for (desc, counter) in counters {
            counter.register(desc.clone());
        }
    }
    pub(super) fn disable(&mut self) {
        ProjectShared::write_drawing(Drawing::empty());
        STATS_PROJECT_RENDER.reset(0);
        for desc in Self::STATS_COUNTERS {
            StatsRef::deregister(desc);
        }
    }
    pub(crate) fn is_projecting(&self) -> bool {
        self.project_projecting
    }

    pub(crate) fn set_method(&mut self, method: ProjectMethod) {
        ProjectShared::write_method(method);
    }
    pub(crate) fn method(&self) -> ProjectMethod {
        ProjectShared::read_method()
    }

    pub(crate) fn undrawn(&self) -> Drawing {
        ProjectShared::read_drawing()
    }
}
impl GogglesState {
    pub(crate) fn project_wants_flush(&self) -> bool {
        self.active.contains(GogglesEnables::PROJECT_COMPAT_FLUSH)
    }
}
impl RenderMachine {
    fn goggles_project_draw_start(&mut self, what: Drawing, cls: BufferClass) -> bool {
        if !self.goggles.is_enabled(GogglesEnables::PROJECT_ENABLE) { return false }

        self.goggles.project.project_projecting = true;
        self.mumblelink_frames.render_offset_space = 1;

        let time_passed = match cls {
            BufferClass::Target | BufferClass::Minimap | BufferClass::FrameBuffer =>
                true,
            _ => false,
        };
        if time_passed {
            self.lastminute_mumblelink_update();
        }

        #[cfg(feature = "goggles2-camera")]
        if let ProjectAction::Draw = what {
            self.goggles_update_camera(false);
        }
        true
    }
    fn goggles_project_draw_end(&mut self) {
        self.goggles.project.project_projecting = false;
        self.goggles.project.project_shadowbox = false;
        self.mumblelink_frames.render_offset_space = 0;
    }
}
