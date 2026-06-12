#[cfg(feature = "paths-lua")]
use core::mem;

use {
    super::ActivePack,
    crate::{
        exports::runtime::Counter,
        render::machine::RenderMachine,
        space::{
            dx11::{InstanceBufferData, RenderBackend},
            resources::{Model, ShaderPair, Texture, Vertex},
            DrawSpace,
        },
    },
    anyhow::Context,
    glam::{vec2, vec3, EulerRot, Mat4, Quat, Vec3, Vec3Swizzles, Vec4},
    glamour::{Box3, Point3, Vector2},
    std::{
        borrow::{Borrow, Cow},
        f32::consts::FRAC_PI_2,
        sync::Arc,
    },
    taimi_d3d::{
        dx11::{
            buffer::{BufferOf, VertexBuffer},
            prelude::*,
        },
        state::PrimitiveTopology,
    },
    taimi_meta::ui::{LocalContext, MapContext},
    taimi_pack::attributes::{
        cell::{pack_attr, AttrKeyValue, GetAttrDyn, PackKeyId, PackValueCell, SetAttrDyn},
        keys::{self, GetAttr, SetAttr},
    },
};

pub struct PoiCommonRenderData {
    // Common fixed data.
    /// POI shader.
    pub shaders: ShaderPair,
    /// Quad trianglestrip.
    quad_vb: VertexBuffer,
    /// Sky-facing geometry
    /// offset buffer directly if not passed to Draw()
    #[cfg(todo)]
    quad_vb_map: VertexBuffer,

    pub world_ib: Option<BufferOf<InstanceBufferData>>,
    pub map_ib: Option<BufferOf<InstanceBufferData>>,
}

// NOTES: Please reference https://github.com/blish-hud/Pathing/blob/main/Entity/StandardMarker.World.cs

impl PoiCommonRenderData {
    pub fn new(backend: &RenderBackend) -> anyhow::Result<PoiCommonRenderData> {
        let mut vertices = Vec::from(Self::quad(LocalContext::World));
        vertices.extend_from_slice(&Self::quad(LocalContext::MAP));

        let quad_vb = Model::from_vertices(vertices).to_buffer(&backend.device)?;
        let shaders = backend
            .shaders
            .pair_named("poi")
            .context("Failed to load POI shader")?;

        Ok(PoiCommonRenderData {
            shaders,
            #[cfg(todo)]
            quad_vb_map: VertexBuffer {
                offset: self.quad_vb.offset + self.quad_vb.stride * POI_QUAD_VERTICES.len() as u32,
                ..quad_vb.clone()
            },
            quad_vb,
            map_ib: None,
            world_ib: None,
        })
    }

    pub const VERTEX_COUNT: usize = POI_QUAD_VERTICES.len();
    pub const VERTEX_OFFSET_MAP: usize = Self::VERTEX_COUNT * 1;

    pub fn quad(ctx: LocalContext) -> [Vertex; 4] {
        match ctx {
            LocalContext::World => POI_QUAD_VERTICES,
            LocalContext::Map(..) => {
                let mut vertices = POI_QUAD_VERTICES;
                for vertex in &mut vertices {
                    vertex.position = vertex.position.xzy();
                    // we use normals to convey vertex expand direction for trails
                    // (POIs are scaled separately, so)
                    //vertex.normal = Vec3::Y;
                    vertex.normal = Vec3::ZERO;
                    vertex.texture.x = 1.0 - vertex.texture.x;
                }
                vertices
            },
        }
    }

    pub fn set(&self, device_context: &Dx11Context) {
        self.shaders.set(device_context);
        self.set_vertex(device_context, LocalContext::World);
        self.set_instance(device_context, LocalContext::World);
    }

    pub const SLOT_VB: u32 = 0;
    pub fn set_vertex(&self, device_context: &Dx11Context, ctx: LocalContext) {
        let vb = match ctx {
            #[cfg(todo)]
            LocalContext::Map(..) => &self.quad_vb_map,
            _ => &self.quad_vb,
        };
        vb.set(device_context, Self::SLOT_VB);
        //self.set_primitive();
    }

    pub const SLOT_IB: u32 = 1;
    pub fn set_instance(&self, device_context: &Dx11Context, ctx: LocalContext) {
        let vb = match ctx {
            LocalContext::World => &self.world_ib,
            LocalContext::Map(..) => &self.map_ib,
        };
        #[cfg(todo)]
        let vb = match vb {
            Some(vb) => vb,
            None => {
                log::warn!("can't draw without POI instance buffer");
                return
            },
        };
        vb.set(device_context, Self::SLOT_IB);
    }

    pub fn set_primitive(&self, device_context: &Dx11Context) {
        PrimitiveTopology::TriangleStrip.set(device_context);
    }

    pub fn clear(&mut self) {
        let _ = self.world_ib.take();
        let _ = self.map_ib.take();
    }

    pub fn is_empty(&self) -> bool {
        self.world_ib.is_none() && self.map_ib.is_none()
    }
}

const POI_QUAD_VERTICES: [Vertex; 4] = [
    Vertex {
        position: vec3(-1.0, -1.0, 0.0),
        colour: Vec3::ONE,
        normal: Vec3::Z,
        texture: vec2(1.0, 0.0),
    },
    Vertex {
        position: vec3(1.0, -1.0, 0.0),
        colour: Vec3::ONE,
        normal: Vec3::Z,
        texture: vec2(0.0, 0.0),
    },
    Vertex {
        position: vec3(-1.0, 1.0, 0.0),
        colour: Vec3::ONE,
        normal: Vec3::Z,
        texture: vec2(1.0, 1.0),
    },
    Vertex {
        position: vec3(1.0, 1.0, 0.0),
        colour: Vec3::ONE,
        normal: Vec3::Z,
        texture: vec2(0.0, 1.0),
    },
];

pub struct ActivePoi {
    pub poi_idx: usize,
    pub category_idx: usize,
    pub filtered: bool,
    pub bounds: Box3<DrawSpace>,
    pub position: Point3<DrawSpace>,
    pub rotation: Option<Quat>,
    pub tint: Vec4,
    pub opacity: f32,
    pub scale: f32,
    pub scale_map: f32,
    pub icon: Arc<Texture>,
    pub render_bookmark: u32,
    #[cfg(feature = "paths-lua")]
    pub ibd_dirty_space: bool,
    #[cfg(feature = "paths-lua")]
    pub ibd_dirty_map: bool,

    pub attr_vis_space: keys::InGameVisibility,
    pub attr_vis_map: keys::MapVisibility,
    pub attr_vis_minimap: keys::MinimapVisibility,
    #[cfg(todo)]
    pub attr_map_id: keys::GameMap,
}

impl ActivePoi {
    pub fn build<A>(
        loader: &mut ActivePack,
        attrs: &A,
        poi_idx: usize,
        category_idx: usize,
        device: &Dx11Device,
        render_bookmark: usize,
    ) -> anyhow::Result<ActivePoi>
    where
        A: GetAttr<keys::Guid>
            + GetAttr<keys::CategoryRef>
            + GetAttr<keys::Tint>
            + GetAttr<keys::Alpha>
            + GetAttr<keys::IconFile>
            + GetAttr<keys::IconSize>
            + GetAttr<keys::MapDisplaySize>
            + GetAttr<keys::PositionX>
            + GetAttr<keys::PositionY>
            + GetAttr<keys::PositionZ>
            + GetAttr<keys::RotateX>
            + GetAttr<keys::RotateY>
            + GetAttr<keys::RotateZ>
            + GetAttr<keys::InGameVisibility>
            + GetAttr<keys::MapVisibility>
            + GetAttr<keys::MinimapVisibility>,
    {
        let icon_handle = GetAttr::<keys::IconFile>::get_attr(attrs)
            .ok_or_else(|| anyhow::anyhow!("POI is missing icon. TODO: default icon?"))?;
        let icon_handle = loader.register_texture(&icon_handle);
        let icon = loader
            .get_or_load_texture(icon_handle, device)
            .context("Loading poi texture")?;

        let position = Point3::new(
            f32::from(GetAttr::<keys::PositionX>::get_attr_or_default(attrs).into_owned()),
            f32::from(GetAttr::<keys::PositionY>::get_attr_or_default(attrs).into_owned()),
            f32::from(GetAttr::<keys::PositionZ>::get_attr_or_default(attrs).into_owned()),
        );
        let rotation = {
            let x = GetAttr::<keys::RotateX>::get_attr(attrs).map(|v| f32::from(v.into_owned()));
            let y = GetAttr::<keys::RotateY>::get_attr(attrs).map(|v| f32::from(v.into_owned()));
            let z = GetAttr::<keys::RotateZ>::get_attr(attrs).map(|v| f32::from(v.into_owned()));
            (x.is_some() | y.is_some() | z.is_some()).then(|| {
                Self::rotation_from_xyz(Vec3::new(
                    x.unwrap_or(0.0f32),
                    y.unwrap_or(0.0f32),
                    z.unwrap_or(0.0f32),
                ))
            })
        };
        let scale = f32::from(GetAttr::<keys::IconSize>::get_attr_or_default(attrs).into_owned());
        let scale_map = f32::from(GetAttr::<keys::MapDisplaySize>::get_attr_or_default(attrs).into_owned());
        let tint = Vec4::from(GetAttr::<keys::Tint>::get_attr_or_default(attrs).into_owned());
        let opacity = f32::from(GetAttr::<keys::Alpha>::get_attr_or_default(attrs).into_owned());

        let bounds = Self::bounds_for(position, scale);

        Ok(ActivePoi {
            poi_idx,
            category_idx,
            filtered: false,
            bounds,
            position,
            rotation,
            tint,
            opacity,
            scale,
            scale_map,
            icon: icon.clone(),
            render_bookmark: render_bookmark as u32,
            attr_vis_space: GetAttr::<keys::InGameVisibility>::get_attr_or_default(attrs).into_owned(),
            attr_vis_map: GetAttr::<keys::MapVisibility>::get_attr_or_default(attrs).into_owned(),
            attr_vis_minimap: GetAttr::<keys::MinimapVisibility>::get_attr_or_default(attrs).into_owned(),
            #[cfg(feature = "paths-lua")]
            ibd_dirty_space: true,
            #[cfg(feature = "paths-lua")]
            ibd_dirty_map: true,
        })
    }
    pub fn new_empty<A>(
        loader: &mut ActivePack,
        attrs: &A,
        poi_idx: usize,
        category_idx: usize,
        device: &Dx11Device,
        render_bookmark: usize,
    ) -> anyhow::Result<ActivePoi>
    where
        A: GetAttr<keys::Guid>
            + GetAttr<keys::Tint>
            + GetAttr<keys::Alpha>
            + GetAttr<keys::IconFile>
            + GetAttr<keys::IconSize>
            + GetAttr<keys::MapDisplaySize>
            + GetAttr<keys::PositionX>
            + GetAttr<keys::PositionY>
            + GetAttr<keys::PositionZ>
            + GetAttr<keys::RotateX>
            + GetAttr<keys::RotateY>
            + GetAttr<keys::RotateZ>
            + GetAttr<keys::InGameVisibility>
            + GetAttr<keys::MapVisibility>
            + GetAttr<keys::MinimapVisibility>,
    {
        let icon_handle = GetAttr::<keys::IconFile>::get_attr(attrs);
        let icon = icon_handle
            .map(|h| {
                let icon_handle = loader.register_texture(&h);
                loader
                    .get_or_load_texture(icon_handle, device)
                    .context("Loading poi icon")
                    .cloned()
            })
            .unwrap_or_else(|| {
                unsafe {
                    Texture::new_raw(
                        device,
                        &vec![0u8; 32 * 32],
                        [32, 32],
                        32,
                        taimi_d3d::DxgiFormat::A8_UNORM,
                    )
                }
                .map(Arc::new)
                .context("Preparing empty texture")
            })?;

        let position = Point3::new(
            f32::from(GetAttr::<keys::PositionX>::get_attr_or_default(attrs).into_owned()),
            f32::from(GetAttr::<keys::PositionY>::get_attr_or_default(attrs).into_owned()),
            f32::from(GetAttr::<keys::PositionZ>::get_attr_or_default(attrs).into_owned()),
        );
        let rotation = {
            let x = GetAttr::<keys::RotateX>::get_attr(attrs).map(|v| f32::from(v.into_owned()));
            let y = GetAttr::<keys::RotateY>::get_attr(attrs).map(|v| f32::from(v.into_owned()));
            let z = GetAttr::<keys::RotateZ>::get_attr(attrs).map(|v| f32::from(v.into_owned()));
            (x.is_some() | y.is_some() | z.is_some()).then(|| {
                Self::rotation_from_xyz(Vec3::new(
                    x.unwrap_or(0.0f32),
                    y.unwrap_or(0.0f32),
                    z.unwrap_or(0.0f32),
                ))
            })
        };
        let scale = f32::from(GetAttr::<keys::IconSize>::get_attr_or_default(attrs).into_owned());

        let bounds = match GetAttr::<keys::PositionX>::has_attr(attrs) {
            true => Self::bounds_for(position, scale),
            false => Self::DIRTY_BOUNDS,
        };

        Ok(ActivePoi {
            poi_idx,
            category_idx,
            filtered: false,
            bounds,
            position,
            tint: Vec4::from(GetAttr::<keys::Tint>::get_attr_or_default(attrs).into_owned()),
            opacity: f32::from(GetAttr::<keys::Alpha>::get_attr_or_default(attrs).into_owned()),
            scale,
            scale_map: f32::from(GetAttr::<keys::MapDisplaySize>::get_attr_or_default(attrs).into_owned()),
            icon: icon.clone(),
            render_bookmark: render_bookmark as u32,
            rotation,
            attr_vis_space: GetAttr::<keys::InGameVisibility>::get_attr_or_default(attrs).into_owned(),
            attr_vis_map: GetAttr::<keys::MapVisibility>::get_attr_or_default(attrs).into_owned(),
            attr_vis_minimap: GetAttr::<keys::MinimapVisibility>::get_attr_or_default(attrs).into_owned(),
            #[cfg(feature = "paths-lua")]
            ibd_dirty_space: true,
            #[cfg(feature = "paths-lua")]
            ibd_dirty_map: true,
        })
    }

    pub fn tint(&self) -> Vec4 {
        let mut tint = self.tint;
        tint.w *= self.opacity;
        tint
    }
    #[inline]
    pub fn is_billboard(&self) -> bool {
        self.rotation.is_none()
    }
    #[inline]
    pub fn rotation_xyz(&self) -> Vec3 {
        self.rotation.map(Self::rotation_to_xyz).unwrap_or(Vec3::ZERO)
    }
    pub(crate) fn rotation_from_xyz(rot: Vec3) -> Quat {
        Quat::from_euler(
            EulerRot::XZY,
            rot.x.to_radians() - FRAC_PI_2,
            rot.y.to_radians(),
            -rot.z.to_radians(),
        )
    }
    pub(crate) fn rotation_to_xyz(rot: Quat) -> Vec3 {
        let (x, y, z) = rot.to_euler(EulerRot::XZY);
        Vec3::new(x + FRAC_PI_2, y, -z).map(f32::to_degrees)
    }

    pub fn instance_data(&self) -> InstanceBufferData {
        InstanceBufferData {
            world: Mat4::from_scale_rotation_translation(
                Vec3::splat(self.scale),
                self.rotation.unwrap_or_default(),
                self.position.into(),
            ),
            colour: self.tint(),
        }
    }

    pub fn instance_data_map(&self, machine: &RenderMachine) -> InstanceBufferData {
        // pixels at 1.0 map scale, translated to local space, but quad is 2.0x2.0...
        let size = Vector2::splat(self.scale_map / 2.0);

        // TODO: DPI/UI scaling is irrelevant here right?
        let scale = size * machine.map.calibration.local_space().scale.abs();
        InstanceBufferData {
            world: Mat4::from_translation(self.position.into())
                * Mat4::from_scale(scale.extend(scale.y).into()),
            colour: self.tint(),
        }
    }

    #[cfg(feature = "paths-lua")]
    pub fn update(pack: &mut ActivePack, active_poi_idx: usize) -> (bool, bool, bool) {
        let poi = match pack.active_pois.get_index_mut(active_poi_idx) {
            #[cfg(debug_assertions)]
            None => unreachable!("poi#{active_poi_idx} missing"),
            p => unsafe { p.unwrap_unchecked().1 },
        };
        let bounds_dirty = poi.bounds_dirty();
        if bounds_dirty {
            poi.regen_bounds()
        }

        (
            bounds_dirty,
            mem::take(&mut poi.ibd_dirty_space),
            mem::take(&mut poi.ibd_dirty_map),
        )
    }

    pub fn draw(&self, device_context: &Dx11Context, render_idx: usize, ctx: LocalContext) {
        self.icon.set(device_context, 0);
        let voffset = match ctx {
            LocalContext::World => 0,
            LocalContext::Map(..) => PoiCommonRenderData::VERTEX_OFFSET_MAP as u32,
        };
        unsafe {
            device_context.DrawInstanced(
                PoiCommonRenderData::VERTEX_COUNT as u32,
                1,
                voffset,
                render_idx as u32,
            );
        }
        /*self.buffer.set(device_context, 1);
        unsafe {
            device_context.Draw(4, 0);
        }*/
    }

    pub(crate) fn is_visible_for_map(&self, ctx: MapContext) -> bool {
        match ctx {
            MapContext::Global => self.attr_vis_map.into(),
            MapContext::Minimap => self.attr_vis_minimap.into(),
        }
    }

    fn bounds_for(position: Point3<DrawSpace>, icon_scale: f32) -> Box3<DrawSpace> {
        let edge_len = icon_scale * 2.0;
        let max_diagonal = (edge_len.powi(2) * 2.0).sqrt();
        Box3::from_origin_and_size(position, glamour::size3!(max_diagonal))
    }
    pub(crate) fn is_dirty(&self) -> bool {
        let dirty = self.bounds_dirty();
        #[cfg(feature = "paths-lua")]
        let dirty = self.ibd_dirty_map | self.ibd_dirty_space | dirty;
        dirty
    }
    pub(crate) fn bounds_dirty(&self) -> bool {
        !self.bounds.max.x.is_finite()
    }
    const DIRTY_BOUNDS: Box3<DrawSpace> = Box3::new(Point3::NEG_INFINITY, Point3::INFINITY);
    fn mark_bounds_dirty(&mut self) {
        self.bounds = Self::DIRTY_BOUNDS;
    }
    fn regen_bounds(&mut self) {
        self.bounds = Self::bounds_for(self.position, self.scale);
    }

    #[cfg(feature = "paths-lua")]
    pub(crate) fn attr_dirties_render(key: PackKeyId) -> bool {
        pack_attr! { =id_is_in(key, [
            keys::InGameVisibility,
            keys::MapVisibility,
            keys::MinimapVisibility,
            keys::GameMap,
        ]) }
    }
}

pub static STATS_POI_INSTANCE_SIZE: Counter = Counter::DEFAULT;

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct PoiScale {
    pub expansion: f32,
}

impl PoiScale {
    /// No expansion, standard sizing
    pub const DEFAULT: Self = Self::new(0.0);

    pub const fn new(expansion: f32) -> Self {
        Self { expansion }
    }

    /// Convert from settings
    pub const fn with_scale(poi_scale: f32) -> Self {
        Self::new(poi_scale - 1.0)
    }

    pub const fn scale(&self) -> f32 {
        self.expansion + 1.0
    }
}

impl Default for PoiScale {
    fn default() -> Self {
        Self::DEFAULT
    }
}

pack_attr! {
    impl Attr{keys::InGameVisibility} for &struct{ActivePoi}.attr_vis_space {}
    impl Attr{keys::MapVisibility} for &struct{ActivePoi}.attr_vis_map {}
    impl Attr{keys::MinimapVisibility} for &struct{ActivePoi}.attr_vis_minimap {}
}
impl GetAttr<keys::PositionX> for ActivePoi {
    #[inline]
    fn has_attr(&self) -> bool {
        true
    }
    #[inline]
    fn get_attr_ref(&self) -> Option<&keys::PositionX> {
        Some(keys::PositionX::from_ref(&self.position.x))
    }
}
impl SetAttr<keys::PositionX> for ActivePoi {
    fn set_attr(&mut self, value: keys::PositionX) {
        let x = f32::from(value);
        let dirty = self.position.x != x;
        self.position.x = x;
        if dirty {
            #[cfg(feature = "paths-lua")]
            {
                self.ibd_dirty_space = true;
            }
            self.mark_bounds_dirty();
        }
    }
}
impl GetAttr<keys::PositionY> for ActivePoi {
    #[inline]
    fn has_attr(&self) -> bool {
        true
    }
    #[inline]
    fn get_attr_ref(&self) -> Option<&keys::PositionY> {
        Some(keys::PositionY::from_ref(&self.position.y))
    }
}
impl SetAttr<keys::PositionY> for ActivePoi {
    fn set_attr(&mut self, value: keys::PositionY) {
        let y = f32::from(value);
        let dirty = self.position.y != y;
        self.position.y = y;
        if dirty {
            #[cfg(feature = "paths-lua")]
            {
                self.ibd_dirty_space = true;
            }
            self.mark_bounds_dirty();
        }
    }
}
impl GetAttr<keys::PositionZ> for ActivePoi {
    #[inline]
    fn has_attr(&self) -> bool {
        true
    }
    #[inline]
    fn get_attr_ref(&self) -> Option<&keys::PositionZ> {
        Some(keys::PositionZ::from_ref(&self.position.z))
    }
}
impl SetAttr<keys::PositionZ> for ActivePoi {
    fn set_attr(&mut self, value: keys::PositionZ) {
        let z = f32::from(value);
        let dirty = self.position.z != z;
        self.position.z = z;
        if dirty {
            #[cfg(feature = "paths-lua")]
            {
                self.ibd_dirty_space = true;
            }
            self.mark_bounds_dirty();
        }
    }
}
impl GetAttr<keys::Rotate> for ActivePoi {
    #[inline]
    fn has_attr(&self) -> bool {
        self.rotation.is_some()
    }
    #[inline]
    fn get_attr(&self) -> Option<Cow<'_, keys::Rotate>> {
        Some(Cow::Owned(keys::Rotate::from(self.rotation_xyz())))
    }
}
impl SetAttr<keys::Rotate> for ActivePoi {
    fn set_attr(&mut self, value: keys::Rotate) {
        self.rotation = Some(Self::rotation_from_xyz(value.into()));
        #[cfg(feature = "paths-lua")]
        {
            self.ibd_dirty_space = true;
        }
    }
    fn unset_attr(&mut self) {
        #[cfg(feature = "paths-lua")]
        {
            self.ibd_dirty_space = self.rotation.is_some();
        }
        self.rotation = None;
    }
}
impl GetAttr<keys::RotateX> for ActivePoi {
    #[inline]
    fn has_attr(&self) -> bool {
        self.rotation.is_some()
    }
    #[inline]
    fn get_attr(&self) -> Option<Cow<'_, keys::RotateX>> {
        Some(Cow::Owned(keys::RotateX::from(self.rotation_xyz().x)))
    }
}
impl SetAttr<keys::RotateX> for ActivePoi {
    fn set_attr(&mut self, value: keys::RotateX) {
        let rot = self.rotation_xyz();
        let x = f32::from(value);
        let dirty = rot.x != x;
        if dirty {
            self.rotation = Some(Self::rotation_from_xyz(rot.with_x(x)));
            #[cfg(feature = "paths-lua")]
            {
                self.ibd_dirty_space = true;
            }
        }
    }
    fn unset_attr(&mut self) {
        self.set_attr(keys::RotateX::default());
        if let Some(true) = self.rotation.map(|r| r.is_near_identity()) {
            SetAttr::<keys::Rotate>::unset_attr(self)
        }
    }
}
impl GetAttr<keys::RotateY> for ActivePoi {
    #[inline]
    fn has_attr(&self) -> bool {
        self.rotation.is_some()
    }
    #[inline]
    fn get_attr(&self) -> Option<Cow<'_, keys::RotateY>> {
        Some(Cow::Owned(keys::RotateY::from(self.rotation_xyz().y)))
    }
}
impl SetAttr<keys::RotateY> for ActivePoi {
    fn set_attr(&mut self, value: keys::RotateY) {
        let rot = self.rotation_xyz();
        let y = f32::from(value);
        let dirty = rot.y != y;
        if dirty {
            self.rotation = Some(Self::rotation_from_xyz(rot.with_y(y)));
            #[cfg(feature = "paths-lua")]
            {
                self.ibd_dirty_space = true;
            }
        }
    }
    fn unset_attr(&mut self) {
        self.set_attr(keys::RotateY::default());
        if let Some(true) = self.rotation.map(|r| r.is_near_identity()) {
            SetAttr::<keys::Rotate>::unset_attr(self)
        }
    }
}
impl GetAttr<keys::RotateZ> for ActivePoi {
    #[inline]
    fn has_attr(&self) -> bool {
        self.rotation.is_some()
    }
    #[inline]
    fn get_attr(&self) -> Option<Cow<'_, keys::RotateZ>> {
        Some(Cow::Owned(keys::RotateZ::from(self.rotation_xyz().z)))
    }
}
impl SetAttr<keys::RotateZ> for ActivePoi {
    fn set_attr(&mut self, value: keys::RotateZ) {
        let rot = self.rotation_xyz();
        let z = f32::from(value);
        let dirty = rot.z != z;
        if dirty {
            self.rotation = Some(Self::rotation_from_xyz(rot.with_z(z)));
            #[cfg(feature = "paths-lua")]
            {
                self.ibd_dirty_space = true;
            }
        }
    }
    fn unset_attr(&mut self) {
        self.set_attr(keys::RotateZ::default());
        if let Some(true) = self.rotation.map(|r| r.is_near_identity()) {
            SetAttr::<keys::Rotate>::unset_attr(self)
        }
    }
}
impl GetAttr<keys::IconSize> for ActivePoi {
    #[inline]
    fn has_attr(&self) -> bool {
        //self.scale_map != keys::IconSize::DEFAULT.0
        true
    }
    #[inline]
    fn get_attr_ref(&self) -> Option<&keys::IconSize> {
        Some(keys::IconSize::from_ref(&self.scale))
    }
}
impl SetAttr<keys::IconSize> for ActivePoi {
    fn set_attr(&mut self, value: keys::IconSize) {
        let scale = f32::from(value);
        let dirty = self.scale != scale;
        self.scale = scale;
        if dirty {
            self.mark_bounds_dirty();
        }
    }
}
impl GetAttr<keys::MapDisplaySize> for ActivePoi {
    #[inline]
    fn has_attr(&self) -> bool {
        //self.scale_map != keys::MapDisplaySize::DEFAULT.0
        true
    }
    #[inline]
    fn get_attr_ref(&self) -> Option<&keys::MapDisplaySize> {
        Some(keys::MapDisplaySize::from_ref(&self.scale_map))
    }
}
impl SetAttr<keys::MapDisplaySize> for ActivePoi {
    fn set_attr(&mut self, value: keys::MapDisplaySize) {
        let scale_map = f32::from(value);
        let _dirty = self.scale_map != scale_map;
        self.scale_map = scale_map;
        #[cfg(feature = "paths-lua")]
        if _dirty {
            self.ibd_dirty_map = true;
        }
    }
}
impl GetAttr<keys::Tint> for ActivePoi {
    #[inline]
    fn has_attr(&self) -> bool {
        //self.tint.0 != keys::Colour::WHITE
        true
    }
    #[inline]
    fn get_attr_ref(&self) -> Option<&keys::Tint> {
        Some(self.tint.borrow())
    }
}
impl SetAttr<keys::Tint> for ActivePoi {
    fn set_attr(&mut self, value: keys::Tint) {
        let tint = Vec4::from(value);
        #[cfg(todo = "unnecessary")]
        let dirty = self.tint != tint;
        #[cfg(feature = "paths-lua")]
        let dirty = true;
        self.tint = tint;
        #[cfg(feature = "paths-lua")]
        if dirty {
            self.ibd_dirty_space = true;
            self.ibd_dirty_map = true;
        }
    }
}
impl GetAttr<keys::Alpha> for ActivePoi {
    #[inline]
    fn has_attr(&self) -> bool {
        //self.opacity < 1.0
        true
    }
    #[inline]
    fn get_attr_ref(&self) -> Option<&keys::Alpha> {
        Some(keys::Alpha::from_ref(&self.opacity))
    }
}
impl SetAttr<keys::Alpha> for ActivePoi {
    fn set_attr(&mut self, value: keys::Alpha) {
        let opacity = f32::from(value);
        #[cfg(feature = "paths-lua")]
        let dirty = self.opacity != opacity;
        self.opacity = opacity;
        #[cfg(feature = "paths-lua")]
        if dirty {
            self.ibd_dirty_space = true;
            self.ibd_dirty_map = true;
        }
    }
}
impl GetAttrDyn for ActivePoi {
    fn holds_attr_dyn(key: PackKeyId) -> bool {
        pack_attr! { =id_is_in(key, [
            keys::Alpha,
            keys::Tint,
            keys::IconSize,
            keys::MapDisplaySize,
            // keys::Position,
            keys::PositionX, keys::PositionY, keys::PositionZ,
            keys::Rotate, keys::RotateX, keys::RotateY, keys::RotateZ,
            keys::InGameVisibility, keys::MapVisibility, keys::MinimapVisibility,
        ]) }
    }
    fn has_attr_dyn(&self, key: PackKeyId) -> bool {
        pack_attr! { imp GetAttrDyn::has_attr_dyn(self, key) in [
            keys::Alpha,
            keys::Tint,
            keys::IconSize,
            keys::MapDisplaySize,
            keys::PositionX, keys::PositionY, keys::PositionZ,
            keys::RotateX, keys::RotateY, keys::RotateZ,
            keys::Rotate,
            keys::InGameVisibility, keys::MapVisibility, keys::MinimapVisibility,
        ] }
        .unwrap_or(false)
    }
    fn get_attr_dyn_ref(&self, key: PackKeyId) -> Option<&dyn AttrKeyValue> {
        pack_attr! { imp GetAttrDyn::get_attr_dyn_ref(self, key) in [
            keys::Alpha,
            keys::Tint,
            keys::IconSize,
            keys::MapDisplaySize,
            keys::PositionX, keys::PositionY, keys::PositionZ,
            keys::InGameVisibility, keys::MapVisibility, keys::MinimapVisibility,
        ] }
        .flatten()
    }
    fn get_attr_dyn(&self, key: PackKeyId) -> Option<Cow<'_, dyn AttrKeyValue>> {
        pack_attr! { imp GetAttrDyn::get_attr_dyn(self, key) in [
            keys::Rotate, keys::RotateX, keys::RotateY, keys::RotateZ,
        ] }
        .unwrap_or_else(|| self.get_attr_dyn_ref(key).map(Cow::Borrowed))
    }
    fn iter_attrs_dyn(&self) -> impl Iterator<Item = std::borrow::Cow<'_, dyn AttrKeyValue>> + '_ {
        pack_attr! { imp GetAttrDyn::iter_attrs_dyn(self) in [
            keys::Alpha,
            keys::Tint,
            keys::IconSize,
            keys::MapDisplaySize,
            keys::PositionX, keys::PositionY, keys::PositionZ,
            keys::Rotate, keys::RotateX, keys::RotateY, keys::RotateZ,
            keys::InGameVisibility, keys::MapVisibility, keys::MinimapVisibility,
        ] }
    }
}
impl SetAttrDyn for ActivePoi {
    fn set_attr_dyn(&mut self, value: PackValueCell) -> bool {
        pack_attr! { imp SetAttrDyn::set_attr_dyn(self, value) in [
            keys::Alpha,
            keys::Tint,
            keys::IconSize,
            keys::MapDisplaySize,
            keys::PositionX, keys::PositionY, keys::PositionZ,
            keys::RotateX, keys::RotateY, keys::RotateZ,
            keys::Rotate,
            keys::InGameVisibility, keys::MapVisibility, keys::MinimapVisibility,
        ] }
    }
}
