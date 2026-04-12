use {
    crate::{
        controller::pathing::{
            registry::{LoadedPoiPath, LoadedTrailPath},
            space::DrawSpace,
        },
        render::machine::RenderPosition,
        resources::shader::{ShaderLayout, ShaderLoader, ShaderPair},
        space::pack::{
            instance::{PoiVertex, PoiVertexBuffer},
            PackRenderData,
            PackRenderResources,
            PackRenderState,
            PoiCommonRenderData,
            PoiRender,
            TrailRender,
        },
    },
    arcffi::cstr::CStrBox,
    glam::Vec3A,
    glamour::Point3,
    std::{
        collections::BinaryHeap,
        ffi::CStr,
        fmt::{self, Write},
        iter,
        ops,
    },
    taimi_d3d::{
        dx11::prelude::*,
        shader::{ShaderDefinition, ShaderKind},
    },
    taimi_hoard::cmp::CmpIgnore,
    taimi_meta::{
        packs::TrailSectionPath,
        ui::{LocalContext, MapContext},
    },
};

/// BvhIter expected to produce distance sort keys, or `None`
/// for items that can ignore the distance priority queue
pub struct RenderOrderBuilder<'a, T, BvhIter> {
    pub bvh_iter: BvhIter,
    pub draw_order_heap: &'a mut BinaryHeap<HeapEntityOf<T>>,
}

pub struct RenderOrderSort {
    pub cam_origin: Vec3A,
    #[cfg(todo)]
    pub cam_dir: Vector3<DrawSpace>,
}
impl RenderOrderSort {
    pub fn with_camera(cam: &RenderPosition) -> Self {
        Self {
            cam_origin: cam.0.into(),
            #[cfg(todo)]
            cam_dir: cam.1,
        }
    }
    #[inline(always)]
    pub fn cam_dist_order_for(&self, position: Point3<DrawSpace>) -> i32 {
        self.cam_dist_order3a(position.into())
    }
    pub fn cam_dist_order3a(&self, position: Vec3A) -> i32 {
        #[cfg(todo)]
        return {
            // TODO: broken or inaccurate idk
            let cam_dist = (position - self.cam_origin).dot(self.cam_dir);
            let cam_dist = f32::to_bits(cam_dist) as i32;
            let cam_dist = cam_dist ^ ((cam_dist >> 30) as u32 >> 1) as i32;
            cam_dist
        };
        Self::dist_to_sort(position.distance_squared(self.cam_origin))
    }
    #[inline]
    pub fn dist_to_sort(dist: f32) -> i32 {
        Self::dist_to_sort_with(dist, Self::DIST_FACTOR)
    }
    #[inline(always)]
    pub fn dist_to_sort_with(dist: f32, factor: f32) -> i32 {
        let dist = dist * factor;
        dist.min(0x40000000i32 as f32) as i32
    }
    pub const DIST_FACTOR: f32 = 1_000_000.0f32;
    pub const DIST_FACTOR_CONSERVATIVE: f32 = Self::DIST_FACTOR * 0.15;
}
impl<'a, T, BvhIter> Iterator for RenderOrderBuilder<'a, T, BvhIter>
where
    BvhIter: Iterator<Item = (Option<i32>, T)>,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((cam_dist, entity)) = self.bvh_iter.next() {
            let cam_dist = match cam_dist {
                None => return Some(entity),
                Some(d) => d,
            };
            self.draw_order_heap
                .push(HeapEntity { cam_dist, value: CmpIgnore(entity) });
        }

        self.draw_order_heap.pop().map(|he| he.value.0)
    }
}

pub type RenderOrderHeap<T> = BinaryHeap<HeapEntity<CmpIgnore<T>>>;
pub type HeapEntityOf<T> = HeapEntity<CmpIgnore<T>>;
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct HeapEntity<T> {
    cam_dist: i32,
    value: T,
}

pub trait DrawSpaceEntity {
    fn is_arcrender(&self) -> bool;
    fn draw_trail_section(
        &mut self,
        pack_data: &PackRenderData,
        space_idx: usize,
        trail: &TrailRender,
        path: LoadedTrailPath,
        section: TrailSectionPath,
    ) -> bool;
    fn draw_poi(
        &mut self,
        pack_data: &PackRenderData,
        space_idx: usize,
        poi: &PoiRender,
        path: LoadedPoiPath,
    ) -> bool;
    #[inline(always)]
    fn poi_visible_override(
        &mut self,
        draw_state: &mut PackRenderState,
        pack_data: &PackRenderData,
        space_idx: usize,
        poi: &PoiRender,
        path: LoadedPoiPath,
    ) -> bool {
        let _ = pack_data;
        let _ = space_idx;
        let _ = poi;
        let _ = path;
        false
    }
    fn finish(&mut self);
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ShaderState {
    Trail,
    Poi,
}
impl ShaderState {
    pub fn type_id_c(state: Option<Self>) -> &'static CStr {
        match state {
            Some(Self::Poi) => c"1",
            Some(Self::Trail) => c"2",
            None => c"3",
        }
    }

    pub fn for_layout(layout: &ShaderLayout) -> Option<Self> {
        match layout {
            ShaderLayout::SpaceTrail => Some(Self::Trail),
            ShaderLayout::SpacePoi => Some(Self::Poi),
            _ => None,
        }
    }
}

/// original renderer
pub struct DrawSpacePack<'a> {
    pub state: Option<Option<ShaderState>>,
    pub poi_common: &'a PoiCommonRenderData,
    pub shaders: &'a ShaderLoader,
    pub shader_trail: Option<ShaderPair>,
    pub context: &'a Dx11Context,
}
impl<'a> DrawSpacePack<'a> {
    fn bind_common(&mut self) -> Option<()> {
        if self.state.is_some() {
            return Some(())
        }
        self.shader_trail = self.shaders.pair_named("trail").ok();
        self.poi_common.set_primitive(self.context);
        self.state = Some(None);
        Some(())
    }
    fn bind_trail(&mut self) -> Option<()> {
        if matches!(self.state, Some(Some(ShaderState::Trail))) {
            return Some(())
        }
        self.bind_common()?;

        let shader = self.shader_trail.as_ref()?;
        shader.set(self.context);
        self.state = Some(Some(ShaderState::Trail));
        Some(())
    }
    fn bind_poi(&mut self) -> Option<()> {
        if matches!(self.state, Some(Some(ShaderState::Poi))) {
            return Some(())
        }
        self.bind_common()?;

        self.poi_common.set(self.context);
        self.state = Some(Some(ShaderState::Poi));
        Some(())
    }
}
impl DrawSpaceEntity for DrawSpacePack<'_> {
    #[inline]
    fn is_arcrender(&self) -> bool {
        false
    }
    fn draw_trail_section(
        &mut self,
        _pack_data: &PackRenderData,
        _space_idx: usize,
        trail: &TrailRender,
        _lpath: LoadedTrailPath,
        section: TrailSectionPath,
    ) -> bool {
        if self.bind_trail().is_none() {
            return false
        }
        trail.bind_texture(self.context, self.poi_common, LocalContext::World);
        trail.draw_section(self.context, section, LocalContext::World);
        true
    }

    fn draw_poi(
        &mut self,
        pack_data: &PackRenderData,
        _space_idx: usize,
        poi: &PoiRender,
        lpath: LoadedPoiPath,
    ) -> bool {
        if self.bind_poi().is_none() {
            return false
        }
        poi.bind_texture(self.context, self.poi_common, LocalContext::World);
        poi.draw(
            self.context,
            pack_data.render_poi_bookmark + lpath.path as usize,
            LocalContext::World,
        );
        true
    }

    fn finish(&mut self) {}
}

/// arcrender
pub struct DrawSpaceArc<'a> {
    pub state: Option<Option<ShaderState>>,
    pub poi_common: &'a PoiCommonRenderData,
    pub resources: &'a PackRenderResources,
    pub context: &'a Dx11Context,
    pub last_quad: Option<&'a PoiVertexBuffer>,
}
impl<'a> DrawSpaceArc<'a> {
    fn bind_common(&mut self) -> Option<()> {
        if self.state.is_some() {
            return Some(())
        }

        let _ = self.resources.shader_variant?;
        #[cfg(todo)]
        let shaderp = self.resources.shader_p_trail.as_ref()?;
        let ib = self.resources.entities_ib.as_ref()?;
        let cb_p = self.resources.shared_cb_p.as_ref()?;
        let cb_v = self.resources.shared_cb_v.as_ref()?;
        ib.set(self.context, 1);
        cb_p.set(self.context, 0);
        cb_v.set(self.context, 0);
        self.poi_common.set_primitive(self.context);
        self.state = Some(None);
        Some(())
    }
    fn bind_trail(&mut self) -> Option<()> {
        if matches!(self.state, Some(Some(ShaderState::Trail))) {
            return Some(())
        }
        let (shaderv, shaderl) = self.resources.shader_trail.as_ref()?;
        self.bind_common()?;
        if let Some(shaderp) = &self.resources.shader_p_trail {
            shaderp.set(self.context);
        }

        shaderv.set(self.context);
        shaderl.set(self.context);
        self.last_quad = None;
        self.state = Some(Some(ShaderState::Trail));
        Some(())
    }
    fn bind_poi(&mut self) -> Option<()> {
        if matches!(self.state, Some(Some(ShaderState::Poi))) {
            return Some(())
        }
        let (shaderv, shaderl) = self.resources.shader_poi.as_ref()?;
        self.bind_common()?;
        if let Some(shaderp) = &self.resources.shader_p_poi {
            shaderp.set(self.context);
        }

        shaderv.set(self.context);
        shaderl.set(self.context);
        self.state = Some(Some(ShaderState::Poi));
        Some(())
    }
}
impl DrawSpaceEntity for DrawSpaceArc<'_> {
    #[inline]
    fn is_arcrender(&self) -> bool {
        true
    }
    fn draw_trail_section(
        &mut self,
        _pack_data: &PackRenderData,
        space_idx: usize,
        trail: &TrailRender,
        _lpath: LoadedTrailPath,
        section: TrailSectionPath,
    ) -> bool {
        if space_idx >= self.resources.len {
            return false
        }
        let vb = trail.section_vb_ng.as_ref().and_then(|vb| {
            trail
                .section_geometry_vertices(section.path)
                .map(|range| (vb, range))
        });
        let Some((vb, ops::Range { start, end })) = vb else { return false };
        if self.bind_trail().is_none() {
            return false
        }
        trail.bind_texture(self.context, self.poi_common, LocalContext::MAP);
        vb.set(self.context, 0);
        unsafe {
            self.context
                .DrawInstanced(end - start, 1, start, space_idx as u32);
        }
        true
    }

    fn draw_poi(
        &mut self,
        _pack_data: &PackRenderData,
        space_idx: usize,
        poi: &PoiRender,
        _lpath: LoadedPoiPath,
    ) -> bool {
        if space_idx >= self.resources.len {
            return false
        }
        if self.bind_poi().is_none() {
            return false
        }
        let vb_quad = match poi.occlude {
            true => self.resources.poi_vb_trans.as_ref(),
            _ => {
                #[cfg(todo)]
                if poi.icon.is_none() {
                    continue
                }
                self.resources.poi_vb.as_ref()
            },
        };
        let Some(vb) = vb_quad else { return false };
        if self.last_quad != Some(vb) {
            vb.set(self.context, 0);
            self.last_quad = Some(vb);
        }

        poi.bind_texture(self.context, self.poi_common, LocalContext::MAP);
        unsafe {
            self.context
                .DrawInstanced(PoiVertex::POI_QUAD.len() as u32, 1, 0, space_idx as u32);
        }
        true
    }
    fn poi_visible_override(
        &mut self,
        draw_state: &mut PackRenderState,
        pack_data: &PackRenderData,
        _space_idx: usize,
        poi: &PoiRender,
        path: LoadedPoiPath,
    ) -> bool {
        let Some(..) = poi.anim else { return false };
        let Some(now) = self.resources.anim_timestamp else { return false };
        let Some(lpath) = pack_data.map_path().map(|map_path| map_path.rel(path.path)) else {
            return false
        };
        let ongoing = match draw_state.poi_get_anim_end(lpath) {
            None => false,
            Some(end) => now < end,
        };
        #[cfg(todo = "unnecessary")]
        if !ongoing {
            draw_state.anim_stop.insert(lpath);
        }
        ongoing
    }

    fn finish(&mut self) {}
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ArcShaderVariant {
    Vanilla,
    Map,
    #[cfg(feature = "goggles")]
    Obscured,
    #[cfg(feature = "goggles2-project")]
    Shadowboxing,
    #[cfg(feature = "goggles2-project")]
    Reflection,
}
impl ArcShaderVariant {
    pub fn template_id(self, kind: ShaderKind, entity: Option<ShaderState>) -> Option<&'static str> {
        Some(match (self, kind, entity) {
            (_, ShaderKind::Vertex, Some(ShaderState::Poi)) => "poi-ng-v",
            (_, ShaderKind::Vertex, Some(ShaderState::Trail)) => "trail-ng-v",
            #[cfg(todo)]
            (_, ShaderKind::Pixel, Some(ShaderState::Poi) | _) => "poi-ng-p",
            (_, ShaderKind::Pixel, Some(ShaderState::Trail) | _) => "trail-ng-p",
            _ => {
                log::warn!("unexpected shader template request for {self:?}/{kind:?} ({entity:?})");
                return None
            },
        })
    }
    pub fn id(self, kind: ShaderKind, entity: Option<ShaderState>) -> Option<&'static str> {
        Some(match (self, kind, entity) {
            (Self::Vanilla, ShaderKind::Vertex, Some(ShaderState::Trail)) => "trail-ng",
            (Self::Vanilla, ShaderKind::Vertex, Some(ShaderState::Poi)) => "poi-ng",
            (Self::Vanilla, ShaderKind::Pixel, ..) => "trail-ng",
            #[cfg(todo)]
            (Self::Map, _, None) => None,
            #[cfg(feature = "goggles")]
            (Self::Obscured, k, e) => return Self::Vanilla.id(k, e),
            #[cfg(feature = "goggles2-project")]
            (Self::Shadowboxing, ShaderKind::Vertex, Some(ShaderState::Trail)) => "trail-sbox",
            #[cfg(feature = "goggles2-project")]
            (Self::Shadowboxing, ShaderKind::Vertex, Some(ShaderState::Poi)) => "poi-sbox",
            #[cfg(feature = "goggles2-project")]
            (Self::Shadowboxing, ShaderKind::Pixel, ..) => "trail-sbox",
            #[cfg(feature = "goggles2-project")]
            (Self::Reflection, ShaderKind::Vertex, Some(ShaderState::Trail)) => "trail-reflect",
            #[cfg(feature = "goggles2-project")]
            (Self::Reflection, ShaderKind::Vertex, Some(ShaderState::Poi)) => "poi-reflect",
            #[cfg(feature = "goggles2-project")]
            (Self::Reflection, ShaderKind::Pixel, ..) => "trail-reflect",
            _ => {
                log::warn!("unexpected shader request for {self:?}/{kind:?} ({entity:?})");
                return None
            },
        })
    }
    pub fn defines(
        self,
        _kind: ShaderKind,
        entity: Option<ShaderState>,
    ) -> impl Iterator<Item = ShaderDefinition> {
        let type_id = ShaderState::type_id_c(entity);
        let base_ty = match self {
            Self::Map => (c"SHADER_MAP", type_id),
            _ => (c"SHADER_SPACE", type_id),
        };
        let options = match self {
            #[cfg(feature = "goggles2-project")]
            Self::Shadowboxing => [Some((c"GOGGLES2_SHADOWBOXING", base_ty.0))],
            #[cfg(feature = "goggles2-project")]
            Self::Reflection => [Some((c"GOGGLES2_REFLECTING", base_ty.0))],
            #[cfg(feature = "goggles")]
            Self::Obscured => [Some((c"GOGGLES_OBSCURED", base_ty.0))],
            _ => [None],
        };
        iter::once(base_ty)
            .chain(IntoIterator::into_iter(options).flatten())
            .map(|(k, v)| ShaderDefinition {
                name: Some(CStrBox::new(k.to_owned())),
                definition: Some(CStrBox::new(v.to_owned())),
            })
    }
}
bitflags::bitflags! {
    #[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct Drawing: u32 {
        const SPACE = 0x0001;
        const MINIMAP = 0x0002;
        const GLOBALMAP = 0x0004;

        #[cfg(feature = "goggles")]
        /// first pass at reduced opacity
        const OBSCURED = 0x0010;
        #[cfg(feature = "goggles2-project")]
        const OBSCURED_SHADOWED = 0x0020;

        #[cfg(feature = "goggles2-project")]
        const REFLECT = 0x0100;
        #[cfg(feature = "goggles2-project")]
        const REFLECT_BELOW = 0x0200;
        #[cfg(feature = "goggles2-project")]
        const SHADOWBOX = 0x0400;

        const FLAG_SPACE_POI = 0x1000_0000;
        const FLAG_SPACE_TRAIL = 0x2000_0000;
        const FLAG_MAP_POI = 0x4000_0000;
        const FLAG_MAP_TRAIL = 0x8000_0000;
    }
}
impl Drawing {
    pub const FLAGS: Self = Self::from_bits_retain(Self::FLAGS_SPACE.bits() | Self::FLAGS_MAP.bits());
    pub const FLAGS_SPACE: Self =
        Self::from_bits_retain(Self::FLAG_SPACE_POI.bits() | Self::FLAG_SPACE_TRAIL.bits());
    pub const FLAGS_MAP: Self =
        Self::from_bits_retain(Self::FLAG_MAP_POI.bits() | Self::FLAG_MAP_TRAIL.bits());
    pub const PASSES: Self = Self::from_bits_truncate(0x00ff_ffff);
    pub const PRIMARY: Self =
        Self::from_bits_retain(Self::SPACE.bits() | Self::MINIMAP.bits() | Self::GLOBALMAP.bits());
    #[cfg(feature = "goggles")]
    pub const PASSES_OBSCURED: Self = Self::from_bits_retain({
        let obscured_proj = match () {
            #[cfg(feature = "goggles2-project")]
            _ => Self::OBSCURED_SHADOWED,
            #[cfg(not(feature = "goggles2-project"))]
            _ => Self::empty(),
        };
        Self::OBSCURED.bits() | obscured_proj.bits()
    });
    #[cfg(feature = "goggles2-project")]
    pub const PASSES_PROJECT: Self =
        Self::from_bits_retain(Self::PASSES_REFLECT.bits() | Self::SHADOWBOX.bits());
    #[cfg(feature = "goggles2-project")]
    pub const PASSES_REFLECT: Self =
        Self::from_bits_retain(Self::REFLECT.bits() | Self::REFLECT_BELOW.bits());
    #[cfg(feature = "goggles2-project")]
    pub const PASSES_INCOMPAT_LEGACY: Self = Self::from_bits_retain(
        Self::REFLECT_BELOW.bits() | Self::OBSCURED_SHADOWED.bits() | Self::SHADOWBOX.bits(),
    );

    #[inline(always)]
    pub const fn index(self) -> u8 {
        self.bits().trailing_zeros() as u8
    }
    #[inline(always)]
    pub const fn from_index(index: u8) -> Self {
        Self::from_bits_retain(1u32 << index)
    }
    #[inline(always)]
    pub const fn get_flags(self) -> Self {
        Self::from_bits_retain(self.bits() & Self::FLAGS.bits())
    }
    #[inline(always)]
    pub const fn get_pass(self) -> Self {
        Self::from_bits_retain(self.bits() & Self::PASSES.bits())
    }
    pub const fn from_local_context(ctx: LocalContext) -> Self {
        match ctx {
            LocalContext::World => Self::SPACE,
            LocalContext::MINIMAP => Self::MINIMAP,
            LocalContext::GLOBAL => Self::GLOBALMAP,
        }
    }
    #[inline(always)]
    pub fn from_context(ctx: impl Into<LocalContext>) -> Self {
        Self::from_local_context(ctx.into())
    }

    pub fn clear(&mut self) {
        *self = Self::empty();
    }
    pub fn set_pass(&mut self, pass: Self) {
        self.remove(Self::PASSES);
        self.insert(pass);
    }

    pub fn iter_passes(self) -> impl Iterator<Item = Self> {
        self.get_pass().into_iter()
    }

    #[inline(always)]
    pub fn has(&self, ctx: impl Into<Self>) -> bool {
        self.contains(ctx.into())
    }
    #[inline(always)]
    pub fn mark(&mut self, ctx: impl Into<Self>) {
        self.insert(ctx.into())
    }
    pub fn to_drawn(self) -> Self {
        match self.get_pass() {
            Self::SPACE => self & (Self::PASSES | Self::FLAGS_SPACE),
            Self::MINIMAP | Self::GLOBALMAP => self & (Self::PASSES | Self::FLAGS_MAP),
            pass => pass,
        }
    }

    pub fn pass_name(self) -> Option<&'static str> {
        Some(match self.get_pass() {
            Self::SPACE => "space",
            Self::MINIMAP => "minimap",
            Self::GLOBALMAP => "map",
            #[cfg(feature = "goggles")]
            Self::OBSCURED => "obscured",
            #[cfg(feature = "goggles2-project")]
            Self::OBSCURED_SHADOWED => "obscured(shadowed)",
            #[cfg(feature = "goggles2-project")]
            Self::REFLECT => "reflect",
            #[cfg(feature = "goggles2-project")]
            Self::REFLECT_BELOW => "reflect(below)",
            #[cfg(feature = "goggles2-project")]
            Self::SHADOWBOX => "shadowbox",
            _ => return None,
        })
    }
}
impl From<LocalContext> for Drawing {
    #[inline]
    fn from(ctx: LocalContext) -> Self {
        Self::from_local_context(ctx)
    }
}
impl From<MapContext> for Drawing {
    #[inline]
    fn from(ctx: MapContext) -> Self {
        Self::from_context(ctx)
    }
}
impl fmt::Display for Drawing {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.pass_name() {
            Some(name) => f.write_str(name),
            None => write!(f, "pass{:#x}", self.bits()),
        }
    }
}
