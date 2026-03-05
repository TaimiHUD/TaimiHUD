use {
    super::{g2, FerretResource},
    crate::render::{machine::FrameState, RenderState},
    core::{ffi::c_void, mem, ops, ptr::NonNull},
    glam::Vec4,
    std::collections::{btree_map, BTreeMap},
    taimi_d3d::dx11::{
        context::DeviceContext0,
        depth::ComparisonFunc,
        prelude::*,
        DepthState,
        DepthView,
        RenderTargetView,
    },
    windows::{
        core::InterfaceRef,
        Win32::Graphics::Direct3D11::{ID3D11DepthStencilView, ID3D11RenderTargetView},
    },
};

pub type D3dNn = NonNull<c_void>;
pub type D3dPtr = Option<D3dNn>;

pub(super) unsafe fn draw_point(context: &Dx11Context, cond: ProjectCondition) {
    let shadowboxing = &mut *g2!(&raw mut ferret.project.shadowboxing);
    let shadowboxing = {
        shadowboxing.as_mut().and_then(|point| match point.process(cond) {
            true => Some(point),
            false => None,
        })
    };
    if let Some(shadowboxing) = shadowboxing {
        shadowboxing.act(context.as_ref());
    }

    let target = &mut *g2!(&raw mut ferret.project.target);
    let target = {
        target.as_mut().and_then(|point| match point.process(cond) {
            true => Some(point),
            false => None,
        })
    };
    if let Some(target) = target {
        target.act(context.as_ref());
    }
}
type SetTargetsKey = (D3dPtr, D3dPtr);
pub(super) unsafe fn set_targets_pre(
    context: &Dx11Context,
    views: &[Option<RenderTargetView>],
    depth: Option<&DepthView>,
) -> SetTargetsKey {
    if !FerretResource::project_enabled() {
        return (None, None)
    }

    let depth_ptr = depth.map(|v| *v.as_d3d_raw());

    let mut render_views = views.iter().flatten();
    let target = {
        let target = &*g2!(&raw mut ferret.project.target);
        target.as_ref().and_then(|p| p.request.target)
    };
    let is_interesting_render = |v: &RenderTargetView| Some(*v.as_d3d_raw()) == target;
    let render_ptr = match render_views.next() {
        None => None,
        Some(view) if render_views.clone().count() == 0 || is_interesting_render(view) => Some(view),
        Some(view) => Some(render_views.find(|v| is_interesting_render(v)).unwrap_or(view)),
    }
    .map(|view| *view.as_d3d_raw());

    let seen = &mut *g2!(&raw mut ferret.project.seen);
    let now = g2!(*&ferret.project.frame_count);
    let mut cls_render = None;
    let cls_depth = depth_ptr.and_then(|p| seen.get(&p).map(|buf| buf.classification));
    if let Some(view_ptr) = render_ptr {
        match seen.entry(view_ptr) {
            btree_map::Entry::Occupied(mut e) => {
                let buf = e.get_mut();
                buf.last_seen = now;
                cls_render = Some(buf.classification);
                if buf.first_seen != now {
                    if buf.classification == ProjectClassification::New {
                        buf.classification = ProjectClassification::Unknown;
                    } else if cls_depth == Some(ProjectClassification::UI) {
                        buf.classification = ProjectClassification::UI;
                    }
                }
            },
            btree_map::Entry::Vacant(e) => {
                e.insert(ProjectBufferInfo {
                    classification: {
                        let buf_desc =
                            super::lens::get_view_dims(RenderTargetView::from_d3d_raw_ref(&view_ptr));
                        let format_ok = |format| match format {
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
                            | dxgi::DXGI_FORMAT_R32G8X24_TYPELESS => false,
                            f if f.0 >= dxgi::DXGI_FORMAT_BC1_TYPELESS.0
                                && f.0 <= dxgi::DXGI_FORMAT_BC5_SNORM.0 =>
                                false,
                            _ => true,
                        };
                        let size_ok = |w: u32, h: u32| {
                            let expected = g2!(*&ferret.display_size);
                            //dims.Width == expected.x as u32 && dims.Height == expected.y as u32
                            ((expected.x / expected.y) - (w as f32 / h as f32)).abs() <= 2e-4f32
                        };
                        let size_ok = match buf_desc {
                            Some(desc) if !format_ok(desc.Format) => false,
                            Some(desc) if !size_ok(desc.Width, desc.Height) => false,
                            _ => true,
                        };
                        match size_ok {
                            false => ProjectClassification::Unsupported,
                            true => ProjectClassification::New,
                        }
                    },
                    last_seen: now,
                    first_seen: now,
                    kind: ProjectBufferKind::RenderTarget,
                });
            },
        }
    }
    if let Some(depth_ptr) = depth_ptr {
        let buf = seen.entry(depth_ptr).or_insert_with(|| ProjectBufferInfo {
            classification: ProjectClassification::New,
            last_seen: now,
            first_seen: now,
            kind: ProjectBufferKind::DepthView,
        });
        buf.last_seen = now;
        if matches!(
            buf.classification,
            ProjectClassification::New | ProjectClassification::Unknown
        ) {
            let lens_cls = super::lens::LENSES
                .try_read()
                .ok()
                .and_then(|l| l.get(&(depth_ptr.as_ptr() as usize)).copied());
            match lens_cls {
                Some(super::lens::LensClass::World) if cls_render == Some(ProjectClassification::World) =>
                    buf.classification = ProjectClassification::World,
                Some(super::lens::LensClass::Test)
                    if cls_render == Some(ProjectClassification::Shadowbox) =>
                    buf.classification = ProjectClassification::Shadowbox,
                Some(super::lens::LensClass::World)
                    if !matches!(
                        cls_render,
                        None | Some(ProjectClassification::New | ProjectClassification::Unknown)
                    ) =>
                    buf.classification = ProjectClassification::Misc,
                Some(super::lens::LensClass::Test | super::lens::LensClass::UI)
                    if !matches!(
                        cls_render,
                        None | Some(ProjectClassification::New | ProjectClassification::Unknown)
                    ) =>
                    buf.classification = ProjectClassification::UI,
                Some(super::lens::LensClass::Sampled) if cls_render != Some(ProjectClassification::New) =>
                    if let Some(cls) = cls_render {
                        buf.classification = cls;
                    },
                Some(super::lens::LensClass::Unsupported)
                    if cls_render == Some(ProjectClassification::Unknown) =>
                    buf.classification = ProjectClassification::Unsupported,
                _ => (),
            }
        }
    }

    draw_point(context, ProjectCondition::Unbind);

    (render_ptr, depth_ptr)
}
pub(super) unsafe fn set_targets_post(context: &Dx11Context, (render_ptr, depth_ptr): SetTargetsKey) {
    let prev_depth = g2!(*&ferret.project.bound_depth);
    if prev_depth != depth_ptr {
        g2!(*&mut ferret.project.bound_depth_written = false);
    }
    g2!(*&mut ferret.project.bound_render = render_ptr);
    g2!(*&mut ferret.project.bound_depth = depth_ptr);
    draw_point(context, ProjectCondition::SetTargets);
    // TODO: hacky
    g2!(*&mut ferret.project.bound_depth_depthless = true);
}
type SetStateKey = (bool, bool);
pub(super) unsafe fn set_state_pre(
    context: &Dx11Context,
    state: Option<&DepthState>,
    stencil_ref: u32,
) -> SetStateKey {
    let desc = state.map(|state| state.get_desc());
    let (mut depthless, mut writing) = (false, true);
    if let Some(desc) = &desc {
        depthless = desc.DepthEnable.0 == 0
            || (desc.DepthWriteMask == d3d11::D3D11_DEPTH_WRITE_MASK_ZERO
                && desc.DepthFunc == ComparisonFunc::ALWAYS);
        writing = desc.DepthEnable.0 != 0 && desc.DepthWriteMask != d3d11::D3D11_DEPTH_WRITE_MASK_ZERO;
        //clobbering = desc.DepthFunc == ComparisonFunc::ALWAYS;
        if writing && !g2!(*&ferret.project.bound_depth_written) {
            let seen = &mut *g2!(&raw mut ferret.project.seen);
            if let Some(render_ptr) = g2!(*&ferret.project.bound_render) {
                if let Some(buf) = seen.get_mut(&render_ptr) {
                    if matches!(
                        buf.classification,
                        ProjectClassification::New | ProjectClassification::Unknown
                    ) {
                        match desc.DepthFunc {
                            ComparisonFunc::LESS if stencil_ref == 0 =>
                                buf.classification = ProjectClassification::World,
                            ComparisonFunc::LESS_EQUAL =>
                                buf.classification = ProjectClassification::Shadowbox,
                            _ => (),
                        }
                    }
                }
            }
        }
    }
    if depthless && !g2!(*&ferret.project.bound_depth_depthless) {
        let seen = &mut *g2!(&raw mut ferret.project.seen);
        if let Some(render_ptr) = g2!(*&ferret.project.bound_render) {
            if let Some(buf) = seen.get_mut(&render_ptr) {
                if matches!(
                    buf.classification,
                    ProjectClassification::New | ProjectClassification::Unknown
                ) {
                    buf.classification = ProjectClassification::Unknown;
                }
            }
        }
        draw_point(context, ProjectCondition::PreDepthless);
    }

    (depthless, writing)
}
pub(super) unsafe fn set_state_post(context: &Dx11Context, (depthless, writing): SetStateKey) {
    g2!(*&mut ferret.project.bound_depth_depthless = depthless);
    if writing {
        g2!(*&mut ferret.project.bound_depth_written = true);
    }
    if depthless {
        draw_point(context, ProjectCondition::Depthless);
    }
    if writing {
        draw_point(context, ProjectCondition::SetState);
    }
}
type ClearDepthKey = ();
#[inline]
pub(super) unsafe fn clear_depth_pre<V>(
    _context: &Dx11Context,
    _view: V,
    _flags: u32,
    _depth: f32,
    _fill_value: u8,
) -> ClearDepthKey {
    #[cfg(todo = "unnecessary")]
    draw_point(context, ProjectCondition::ClearDepthPre);
    ()
}
pub(super) unsafe fn clear_depth_post(context: &Dx11Context, (): ClearDepthKey) {
    g2!(*&mut ferret.project.bound_depth_cleared = true);
    draw_point(context, ProjectCondition::ClearDepth);
}

type ClearColourKey = ();
#[inline]
pub(super) unsafe fn clear_colour_pre<V>(
    _context: &Dx11Context,
    _view: V,
    _colour: &[f32; 4],
) -> ClearColourKey {
    #[cfg(todo = "unnecessary")]
    draw_point(context, ProjectCondition::ClearColourPre);
    ()
}
pub(super) unsafe fn clear_colour_post(context: &Dx11Context, (): ClearColourKey) {
    g2!(*&mut ferret.project.bound_render_cleared = true);
    draw_point(context, ProjectCondition::ClearColour);
}

fn draw(context: &DeviceContext0, target: Option<&RenderTargetView>, depth: Option<&DepthView>) {
    FrameState::TAIMI.publish_set();
    let mut state = RenderState::lock();
    if let Some(state) = &mut *state {
        if let Some(Ok(engine)) = &mut state.engine {
            engine.render_carefully(&mut state.machine, context, target, depth);
        }
    }
    drop(state);
    FrameState::TAIMI.publish_clear();
}
const DETECT_COLOUR: Vec4 = Vec4::new(64.0 / 255.0, 224.0 / 255.0, 208.0 / 255.0, 0.7);
fn draw_detect(context: &DeviceContext0, target: &RenderTargetView) {
    FrameState::TAIMI.publish_set();
    target.clear_rgba(context, DETECT_COLOUR);
    FrameState::TAIMI.publish_clear();
}
fn shadowbox(context: &DeviceContext0, target: Option<&RenderTargetView>, depth: Option<&DepthView>) {
    g2!(*&mut ferret.project.state_shadowbox = true);
    draw(context, target, depth);
    g2!(*&mut ferret.project.state_shadowbox = false);
}

pub struct ProjectFerret {
    pub frame_count: u32,
    seen: BTreeMap<D3dNn, ProjectBufferInfo>,
    bound_depth: D3dPtr,
    bound_render: D3dPtr,
    #[cfg(todo)]
    bound_render_written: bool,
    bound_render_cleared: bool,
    bound_depth_cleared: bool,
    bound_depth_written: bool,
    bound_depth_depthless: bool,
    state_shadowbox: bool,

    pub target: Option<ProjectPoint>,
    pub shadowboxing: Option<ProjectPoint>,
    pub target_report: ProjectReport,
    pub shadowboxing_report: ProjectReport,
}
impl ProjectFerret {
    pub const EMPTY: Self = Self {
        frame_count: 0,
        seen: BTreeMap::new(),
        bound_depth: None,
        bound_render: None,
        #[cfg(todo)]
        bound_render_written: false,
        bound_render_cleared: false,
        bound_depth_cleared: false,
        bound_depth_written: false,
        bound_depth_depthless: false,
        state_shadowbox: false,
        target: None,
        shadowboxing: None,
        target_report: ProjectReport::EMPTY,
        shadowboxing_report: ProjectReport::EMPTY,
    };
    pub fn reset_frame(&mut self) {
        self.bound_depth = None;
        self.bound_render = None;
        #[cfg(todo)]
        {
            self.bound_render_written = false;
        }
        self.bound_render_cleared = false;
        self.bound_depth_cleared = false;
        self.bound_depth_written = false;
        self.bound_depth_depthless = false;

        let next_frame = self.frame_count.wrapping_add(1);
        let prev_frame = mem::replace(&mut self.frame_count, next_frame);
        self.seen.retain(|_k, buf| {
            if buf.is_gone(prev_frame) {
                return false
            }
            true
        });
        if let Some(target) = &mut self.target {
            if let (Some(key), Some(acq)) = (target.request.target, target.request.acquire) {
                if let Some(new_key) = ProjectBufferInfo::reacquire(&self.seen, prev_frame, &key, acq) {
                    target.request.target = Some(new_key);
                }
            }
        }
        if let Some(target) = &mut self.shadowboxing {
            if let (Some(key), Some(acq)) = (target.request.target, target.request.acquire) {
                if let Some(new_key) = ProjectBufferInfo::reacquire(&self.seen, prev_frame, &key, acq) {
                    target.request.target = Some(new_key);
                }
            }
        }
    }
    pub fn report_frame(&mut self) {
        if let Some(target) = &mut self.target {
            self.target_report = target.reset();
        }
        if let Some(target) = &mut self.shadowboxing {
            self.shadowboxing_report = target.reset();
        }
    }
}
unsafe impl Sync for ProjectFerret {}
unsafe impl Send for ProjectFerret {}

#[derive(Debug, Copy, Clone, PartialEq, Eq, strum::IntoStaticStr, strum::VariantArray)]
pub enum ProjectCondition {
    Unbind,
    PreDepthless,
    Depthless,
    SetState,
    SetTargets,
    ClearColour,
    ClearDepth,
}
impl ProjectCondition {
    pub const DEFAULT_TARGET: Self = match () {
        _ => Self::PreDepthless,
        #[cfg(todo)]
        _ => Self::Depthless,
        #[cfg(todo)]
        _ => Self::SetState,
    };
    pub const DEFAULT_SHADOWBOX: Self = match () {
        #[cfg(todo)]
        _ => Self::Unbind,
        _ => Self::SetState,
    };
    #[inline]
    pub fn matches(self, rhs: Self) -> bool {
        self == rhs
    }
}

impl FerretResource {
    pub fn project_reset_frame() {
        let project = g2!(&raw mut ferret.project);
        unsafe {
            (&mut *project).reset_frame();
        }
    }
    pub fn project_report_frame() {
        let project = g2!(&raw mut ferret.project);
        unsafe {
            (&mut *project).report_frame();
        }
    }
    pub fn project_enabled() -> bool {
        let has_target = unsafe { (&*g2!(&raw const ferret.project.target)).is_some() };
        has_target
    }
    #[cfg(todo)]
    pub unsafe fn project_bound_render<'a>() -> &'a Option<RenderTargetView> {
        let raw: *const D3dPtr = g2!(&raw const ferret.project.bound_render);
        transmute(raw)
    }
    #[cfg(todo)]
    pub unsafe fn project_bound_depth<'a>() -> &'a Option<DepthView> {
        let raw: *const D3dPtr = g2!(&raw const ferret.project.bound_depth);
        transmute(raw)
    }
    pub unsafe fn project_bound_render<'a>() -> Option<InterfaceRef<'a, ID3D11RenderTargetView>> {
        g2!(*&ferret.project.bound_render).map(|v| InterfaceRef::from_raw(v))
    }
    pub unsafe fn project_bound_depth<'a>() -> Option<InterfaceRef<'a, ID3D11DepthStencilView>> {
        g2!(*&ferret.project.bound_depth).map(|v| InterfaceRef::from_raw(v))
    }
    pub(crate) fn project_hack_shadowbox() -> bool {
        g2!(*&ferret.project.state_shadowbox)
    }
    pub(crate) fn project_report_target() -> Option<ProjectReport> {
        Self::project_enabled().then_some(g2!(*&ferret.project.target_report))
    }
    pub(super) fn project_report_drawn() -> bool {
        let target = unsafe { &*g2!(&raw const ferret.project.target) };
        target
            .as_ref()
            .map(|t| t.count > *t.request.delay.start())
            .unwrap_or(true)
    }
    pub(crate) fn project_iter_ui(
        _target: ProjectClassification,
    ) -> impl Iterator<Item = (D3dNn, ProjectBufferInfo)> {
        let seen = unsafe { &*g2!(&raw const ferret.project.seen) };
        let now = g2!(*&ferret.project.frame_count);
        let clsok = |cls: ProjectClassification| match cls {
            #[cfg(todo)]
            cls if cls == _target => true,
            ProjectClassification::New => false,
            _ => true,
        };
        let mut candidates = seen
            .iter()
            .filter(|(_k, buf)| {
                buf.age() >= ProjectBufferInfo::TIME_AGE_UI
                    && !buf.is_lost(now)
                    && clsok(buf.classification)
            })
            .map(|(k, buf)| (k.clone(), buf.clone()))
            .collect::<Vec<_>>();
        candidates.sort_by_key(|(_k, buf)| buf.clone());
        candidates.into_iter()
    }
    fn project_set_with(
        target: &mut Option<ProjectPoint>,
        action: ProjectAction,
        p: D3dPtr,
        classification: Option<ProjectClassification>,
    ) {
        if p.is_none() {
            *target = None;
        }
        let target = target.get_or_insert_with(|| ProjectPoint {
            request: match action {
                ProjectAction::Draw => ProjectRequest::default_target(),
                ProjectAction::Shadowbox => ProjectRequest::default_shadowbox(),
                _ => ProjectRequest::fallback(),
            },
            count: 0,
            depth_written: false,
            action,
        });
        target.request.target = p;
        if let Some(cls) = classification {
            target.request.acquire = Some(cls);
        }
    }
    pub(crate) fn project_set_target(p: D3dPtr, classification: Option<ProjectClassification>) {
        let target = unsafe { &mut *g2!(&raw mut ferret.project.target) };
        Self::project_set_with(target, ProjectAction::Draw, p, classification)
    }
    fn project_set_request_with(
        target: &mut Option<ProjectPoint>,
        action: ProjectAction,
        request: ProjectRequest,
    ) {
        let target = target.get_or_insert_with(|| ProjectPoint {
            request: ProjectRequest::fallback(),
            count: 0,
            depth_written: false,
            action,
        });
        target.request = request;
    }
    pub(crate) fn project_set_target_request(request: Option<ProjectRequest>) {
        let target = unsafe { &mut *g2!(&raw mut ferret.project.target) };
        g2!(*&mut ferret.project.target_report = ProjectReport::EMPTY);
        let Some(request) = request else {
            *target = None;
            return
        };
        Self::project_set_request_with(target, ProjectAction::Draw, request);
    }
    pub(crate) fn project_set_shadowbox(p: D3dPtr, classification: Option<ProjectClassification>) {
        let target = unsafe { &mut *g2!(&raw mut ferret.project.shadowboxing) };
        g2!(*&mut ferret.project.shadowboxing_report = ProjectReport::EMPTY);
        Self::project_set_with(target, ProjectAction::Shadowbox, p, classification)
    }
    pub(crate) fn project_set_shadowbox_request(request: Option<ProjectRequest>) {
        let target = unsafe { &mut *g2!(&raw mut ferret.project.shadowboxing) };
        let Some(request) = request else {
            *target = None;
            return
        };
        Self::project_set_request_with(target, ProjectAction::Shadowbox, request);
    }
    pub(crate) fn project_target_buffer() -> Option<D3dPtr> {
        let target = unsafe { &*g2!(&raw const ferret.project.target) };
        target.as_ref().map(|t| t.request.target)
    }
    pub(crate) fn project_target_request() -> Option<ProjectRequest> {
        let target = unsafe { &*g2!(&raw const ferret.project.target) };
        target.as_ref().map(|t| t.request.clone())
    }
    pub(crate) fn project_shadowbox_buffer() -> Option<D3dPtr> {
        let target = unsafe { &*g2!(&raw const ferret.project.shadowboxing) };
        target.as_ref().map(|t| t.request.target)
    }
}

#[derive(Debug, Clone)]
pub struct ProjectRequest {
    pub target: D3dPtr,
    pub cond: ProjectCondition,
    pub delay: ops::RangeInclusive<u32>,
    pub empty: bool,
    pub acquire: Option<ProjectClassification>,
    pub patient: bool,
    #[cfg(todo)]
    pub empty_depth: bool,
}
impl ProjectRequest {
    pub fn default_target() -> Self {
        ProjectRequest {
            target: None,
            cond: ProjectCondition::DEFAULT_TARGET,
            delay: 0..=0,
            empty: false,
            acquire: Some(ProjectClassification::World),
            patient: false,
        }
    }
    pub fn default_shadowbox() -> Self {
        ProjectRequest {
            target: None,
            cond: ProjectCondition::DEFAULT_SHADOWBOX,
            delay: 0..=0,
            empty: false,
            acquire: Some(ProjectClassification::Shadowbox),
            patient: false,
        }
    }
    pub fn fallback() -> Self {
        ProjectRequest {
            target: None,
            cond: ProjectCondition::Unbind,
            delay: 0..=0,
            empty: false,
            acquire: None,
            patient: true,
        }
    }
    #[inline]
    pub fn pre_match(&self, cond: ProjectCondition) -> bool {
        if !self.cond.matches(cond) {
            return false
        }
        if !g2!(*&ferret.project.bound_render_cleared) | !g2!(*&ferret.project.bound_depth_cleared) {
            return false
        }
        let render_ptr = g2!(*&ferret.project.bound_render);
        let depth_ptr = g2!(*&ferret.project.bound_depth);
        if let Some(target) = self.target {
            let is_render = render_ptr == Some(target);
            let is_depth = g2!(*&ferret.project.bound_depth) == Some(target);
            if !is_render && !is_depth {
                return false
            }
        } else if let Some(acquire) = self.acquire {
            let seen = unsafe { &*g2!(&raw const ferret.project.seen) };
            let cls = render_ptr.and_then(|p| seen.get(&p).map(|b| b.classification));
            let cls_depth = depth_ptr.and_then(|p| seen.get(&p).map(|b| b.classification));
            if cls != Some(acquire) || cls_depth != Some(acquire) {
                return false
            }
        }

        true
    }
}
#[derive(Debug, Clone)]
pub struct ProjectPoint {
    pub request: ProjectRequest,
    pub count: u32,
    pub depth_written: bool,
    pub action: ProjectAction,
}
impl ProjectPoint {
    pub fn process(&mut self, cond: ProjectCondition) -> bool {
        if !self.request.pre_match(cond) {
            return false
        }
        if g2!(*&ferret.project.bound_depth_written) {
            self.depth_written = true;
        }
        if self.request.empty == self.depth_written {
            return false
        }
        let count = self.count.saturating_add(1);
        let prev_count = mem::replace(&mut self.count, count);
        if !self.request.delay.contains(&prev_count) {
            return false
        }
        true
    }

    pub unsafe fn act(&mut self, context: &DeviceContext0) {
        let render = FerretResource::project_bound_render();
        let render = render.as_ref().map(|r| RenderTargetView::from_d3d_ref(r));
        match self.action {
            ProjectAction::Nop => (),
            ProjectAction::DebugDetect => {
                let Some(target) = render else { return };
                draw_detect(context, target)
            },
            ProjectAction::Draw => {
                let Some(target) = render else { return };
                let depth = FerretResource::project_bound_depth();
                let depth = depth.as_ref().map(|d| d.as_ref());
                draw(context, Some(target), depth)
            },
            ProjectAction::Shadowbox => {
                let depth = FerretResource::project_bound_depth();
                let depth = depth.as_ref().map(|d| d.as_ref());
                shadowbox(context, render, depth)
            },
        }
    }

    pub fn reset(&mut self) -> ProjectReport {
        let last_repeat = self.count.checked_sub(1);
        let report = ProjectReport {
            count: self.count,
            acted: last_repeat
                .map(|c| c >= *self.request.delay.start())
                .unwrap_or(false),
        };
        self.count = 0;
        self.depth_written = false;
        self.request.delay = match last_repeat {
            Some(c) if self.request.patient => (*self.request.delay.end()).min(c)..=c,
            None if self.request.patient => 0..=(*self.request.delay.end()).min(2),
            _ => 0..=0,
        };
        report
    }
}
#[derive(Debug, Clone)]
pub struct ProjectReport {
    pub count: u32,
    pub acted: bool,
}
impl ProjectReport {
    pub const EMPTY: Self = ProjectReport { count: 0, acted: false };
}
#[derive(Debug, Copy, Clone, PartialEq, Eq, strum::IntoStaticStr, strum::VariantArray)]
pub enum ProjectAction {
    Nop,
    DebugDetect,
    Draw,
    Shadowbox,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, strum::IntoStaticStr)]
pub enum ProjectClassification {
    World,
    Shadowbox,
    UI,
    Misc,
    Unknown,
    New,
    Unsupported,
}
impl ProjectClassification {
    pub const DEFAULT_TARGET: Self = Self::World;
    pub const DEFAULT_SHADOWBOX: Self = Self::Shadowbox;
}
#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq)]
pub enum ProjectBufferKind {
    DepthView,
    RenderTarget,
}
#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq)]
pub struct ProjectBufferInfo {
    pub classification: ProjectClassification,
    pub kind: ProjectBufferKind,
    pub last_seen: u32,
    pub first_seen: u32,
}
impl ProjectBufferInfo {
    const TIME_GONE: u32 = 32;
    const TIME_LOST: u32 = 2;
    const TIME_AGE_MIN: u32 = 2;
    const TIME_AGE_UI: u32 = 64;

    pub fn seen_since(&self, now: u32) -> u32 {
        now.wrapping_sub(self.last_seen)
    }
    pub fn age(&self) -> u32 {
        self.last_seen.wrapping_sub(self.first_seen)
    }
    #[inline]
    pub fn is_gone(&self, now: u32) -> bool {
        self.seen_since(now) >= Self::TIME_GONE
    }
    #[inline]
    pub fn is_lost(&self, now: u32) -> bool {
        self.seen_since(now) >= Self::TIME_LOST
    }

    fn reacquire<K>(
        seen: &BTreeMap<K, Self>,
        now: u32,
        key: &K,
        acquire: ProjectClassification,
    ) -> Option<K>
    where
        K: Clone + Eq + Ord,
    {
        let buf = seen.get(key);
        let reacquire = match buf {
            None => true,
            Some(buf) => buf.is_lost(now),
        };
        if !reacquire {
            return None
        }
        let mut candidates = seen.iter().filter(|(_k, buf)| {
            *_k != key
                && buf.classification == acquire
                && buf.age() >= ProjectBufferInfo::TIME_AGE_MIN
                && !buf.is_gone(now)
        });
        if let Some((cand, _)) = candidates.next() {
            Some(cand.clone())
        } else {
            None
        }
    }
}
