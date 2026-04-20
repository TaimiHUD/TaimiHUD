use {
    super::PackRenderState,
    crate::{
        controller::pathing::shared::{LoadedPoiRef, SharedPackInfo},
        exports::runtime::{
            textures::{TextureKey, TextureSlot},
            Counter,
        },
        render::machine::RenderMachine,
        space::{
            dx11::{InstanceBufferData, RenderBackend},
            pack::PackRenderData,
            resources::{Model, ShaderPair, Vertex},
        },
        TEXTURES,
    },
    anyhow::Context,
    glam::{vec2, vec3, EulerRot, Mat4, Quat, Vec3, Vec3Swizzles, Vec4},
    glamour::{Box3, Point3, Vector2},
    std::{
        borrow::{Borrow, Cow},
        f32::consts::FRAC_PI_2,
        fmt,
        mem,
        sync::Arc,
    },
    taimi_d3d::{
        dx11::{
            buffer::{BufferOf, VertexBuffer},
            prelude::*,
        },
        state::PrimitiveTopology,
    },
    taimi_meta::{
        packs::id::MarkerId,
        ui::{LocalContext, MapContext},
    },
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

    pub fallback_texture: Option<TextureSlot>,
    pub fallback_texture2: Option<TextureSlot>,
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
            fallback_texture: None,
            fallback_texture2: None,
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

    pub fn update_fallback(&mut self, device: &Dx11Device, _machine: &RenderMachine) {
        if self.fallback_texture.is_none() {
            if let Some(texture) = TEXTURES.lookup_loaded(RenderMachine::TEXTURE_LOGO_KEY) {
                self.fallback_texture = texture;
            }
        }
        if self.fallback_texture2.is_none() {
            if let Some(texture) = TEXTURES.lookup_loaded(RenderMachine::TEXTURE_LOGO_LINES_KEY) {
                self.fallback_texture2 = texture;
            }
        }
    }
    #[cfg(todo)]
    pub fn update(
        &mut self,
        device: &Dx11Device,
        machine: &RenderMachine,
        packs: &[PackRenderData],
    ) -> anyhow::Result<()> {
        if self.fallback_texture.is_none() {
            if let Some(texture) = TEXTURES.lookup_loaded(RenderMachine::TEXTURE_LOGO_KEY) {
                self.fallback_texture = texture;
            }
        }

        #[cfg(todo)]
        {
            // scratch this because len depends on both poi info being uptodate
            // *and* knowing if any packs have non-empty trails if pois=0
            let ib_len = self.ib_len_for_packs(packs);
            let ib_dirty = !self.is_empty() && self.ib_len() != ib_len;
            if !ib_dirty {
                return Ok(())
            }
        }

        self.rebuild_ib(device, machine, packs)?;

        Ok(())
    }
    pub fn rebuild_ib(
        &mut self,
        device: &Dx11Device,
        machine: &RenderMachine,
        packs: &[PackRenderData],
    ) -> anyhow::Result<()> {
        let ib_len = self.ib_len_for_packs(packs);
        if ib_len == 0 {
            // usually we'd reserve one for trails but this probably means 0 packs loaded?
            return Ok(())
        }
        let mut data_world = vec![InstanceBufferData::IDENTITY; ib_len];
        let mut data_map = vec![InstanceBufferData::IDENTITY; ib_len];
        self.write_ib(machine, packs, &mut data_world, &mut data_map)?;

        let (data_world, data_map) = (&data_world[..], &data_map[..]);
        STATS_POI_INSTANCE_SIZE.reset_with(|| (size_of_val(data_map) + size_of_val(data_world)));
        let (poi_ib_world, poi_ib_map) = (
            BufferOf::new_with_data(device, Ok(data_world), ())?,
            BufferOf::new_with_data(device, Ok(data_map), ())?,
        );
        self.world_ib = Some(poi_ib_world);
        self.map_ib = Some(poi_ib_map);
        Ok(())
    }
    pub fn write_ib(
        &self,
        machine: &RenderMachine,
        packs: &[PackRenderData],
        ib_world: &mut [InstanceBufferData],
        ib_map: &mut [InstanceBufferData],
    ) -> anyhow::Result<()> {
        let ib_len = self.ib_len_for_packs(packs);
        if (ib_world.len() > 1 && ib_world.len() != ib_len) || (ib_map.len() > 1 && ib_map.len() != ib_len)
        {
            anyhow::bail!(
                "expected {ib_len} POI instances, got {}(world) and {}(map) instead",
                ib_world.len(),
                ib_map.len()
            );
        }
        #[cfg(todo = "unnecessary")]
        let mut gaps: BitVec = {
            // currently we always start with a fresh pre-filled vec...
            let mut gaps = BitVec::with_capacity(ib_len);
            gaps.resize(ib_len, false);
            gaps
        };
        for (_packi, pack) in packs.iter().enumerate() {
            let Some(map_info) = &pack.map_info else { continue };
            for (i, (poi, lpoi)) in pack
                .render_poi_bookmarks()
                .zip(pack.pois.values().zip(pack.map_state.loaded_pois(map_info)))
            {
                let index = i as usize;
                #[cfg(todo = "unnecessary")]
                if let Some(mut b) = gaps.get_mut(index) {
                    if *b {
                        log::debug!("POI instance {i} of pack#{_packi} duplicated, ignoring???");
                        continue
                    }
                    *b = true;
                }
                if let Some(world) = ib_world.get_mut(index) {
                    *world = poi.instance_data(&lpoi);
                }
                if let Some(map) = ib_map.get_mut(index) {
                    *map = poi.instance_data_map(&lpoi, machine);
                }
            }
        }
        #[cfg(todo = "unnecessary")]
        for gap in gaps.iter_zeros() {
            // fill identity at start for trail drawing
            if let Some(world) = ib_world.get_mut(gap) {
                *world = InstanceBufferData::IDENTITY;
            }
            if let Some(map) = ib_map.get_mut(gap) {
                *map = InstanceBufferData::IDENTITY;
            }
        }

        Ok(())
    }
    pub(super) fn ib_len_for_packs(&self, packs: &[PackRenderData]) -> usize {
        packs
            .iter()
            .map(|p| p.render_poi_bookmarks().end as usize)
            .max()
            .map(|l| l.max(1))
            .unwrap_or(0)
    }
    pub(super) fn ib_len(&self) -> usize {
        let ib = self.world_ib.as_ref().or(self.map_ib.as_ref());
        let Some(ib) = ib else { return 0 };
        let count = ib.count();
        if count == 0 {
            log::debug!("TODO: is buffer.count() (ByteSize) reliable? shouldn't be 0 right...");
        }
        ib.count()
    }

    pub fn is_empty(&self) -> bool {
        self.world_ib.is_none() && self.map_ib.is_none()
    }

    /// whole thing lol
    #[inline]
    pub fn cleanup_background(self) {
        mem::forget(self);
    }
}
impl fmt::Debug for PoiCommonRenderData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("PoiCommonRenderData")
            .field("world_ib", &self.world_ib)
            .field("map_ib", &self.map_ib)
            .finish()
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

pub struct PoiRender {
    pub icon_handle: Option<TextureKey>,
    pub icon: Option<TextureSlot>,
    pub static_rotation: bool,
    pub occlude: bool,
    pub anim: Option<f32>,
}
impl PoiRender {
    pub fn empty() -> Self {
        Self {
            icon_handle: None,
            icon: None,
            static_rotation: false,
            occlude: false,
            anim: None,
        }
    }

    pub fn update(
        &mut self,
        _device: &Dx11Device,
        pack_info: &SharedPackInfo,
        lpoi: Option<LoadedPoiRef<'_>>,
    ) {
        let icon_name = lpoi.as_ref().and_then(|lpoi| lpoi.poi_attrs().icon_file.as_ref());
        pack_info.setup_texture(&mut self.icon_handle, &mut self.icon, icon_name);
    }
    pub fn report_incomplete(&self, id: &MarkerId, draw_state: &mut PackRenderState) -> bool {
        if matches!(
            self.icon,
            None | Some(TextureSlot::Reserved | TextureSlot::Loading)
        ) {
            if !draw_state.mark_incomplete(id) {
                return true
            }
        }
        false
    }
    pub fn needs_texture_info(&self) -> bool {
        self.icon.is_none() && self.icon_handle.is_none()
    }
    #[inline]
    pub fn is_billboard(&self) -> bool {
        !self.static_rotation
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
    pub fn populate_rotation(&mut self, poi: &LoadedPoiRef) {
        self.static_rotation = GetAttr::<keys::Rotate>::has_attr(&**poi.poi_attrs());
    }

    pub fn instance_data(&self, poi: &LoadedPoiRef) -> InstanceBufferData {
        let render = poi.render_attrs();
        let attrs = poi.poi_attrs();
        InstanceBufferData {
            world: Mat4::from_scale_rotation_translation(
                Vec3::splat(
                    GetAttr::<keys::IconSize>::get_attr_or_default(&**attrs)
                        .into_owned()
                        .into(),
                ),
                attrs.rotate.map(Self::rotation_from_xyz).unwrap_or_default(),
                poi.lpoi().position.into(),
            ),
            colour: render.tint(),
        }
    }

    pub fn instance_data_map(&self, lpoi: &LoadedPoiRef, machine: &RenderMachine) -> InstanceBufferData {
        // pixels at 1.0 map scale, translated to local space, but quad is 2.0x2.0...
        let scale_map = f32::from(
            GetAttr::<keys::MapDisplaySize>::get_attr_or_default(&**lpoi.poi_attrs()).into_owned(),
        );
        let size = Vector2::splat(scale_map / 2.0);

        // TODO: DPI/UI scaling is irrelevant here right?
        let scale = size * machine.map.calibration.local_space().scale.abs();
        InstanceBufferData {
            world: Mat4::from_translation(lpoi.lpoi().position.into())
                * Mat4::from_scale(scale.extend(scale.y).into()),
            colour: lpoi.render_attrs().tint(),
        }
    }

    pub fn bind_texture(
        &self,
        device_context: &Dx11Context,
        common: &PoiCommonRenderData,
        _ctx: LocalContext,
    ) {
        let texture = self
            .icon
            .as_ref()
            .and_then(TextureSlot::get)
            .or_else(|| common.fallback_texture.as_ref());
        if let Some(texture) = texture {
            texture.set(device_context, 0);
        }
    }

    /// PREREQUISITES: Poi shaders and texture must already be set.
    pub fn draw(&self, device_context: &Dx11Context, render_idx: usize, ctx: LocalContext) {
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

    #[cfg(feature = "paths-lua")]
    pub(crate) fn attr_dirties_render(key: PackKeyId) -> bool {
        pack_attr! { =id_is_in(key, [
            keys::InGameVisibility,
            keys::MapVisibility,
            keys::MinimapVisibility,
            keys::GameMap,
        ]) }
    }

    /// TODO: texture manager should handle cleanup explcitly...
    #[inline]
    pub fn cleanup_background(mut self) {
        mem::forget(self.icon.take());
    }
}

pub static STATS_POI_INSTANCE_SIZE: Counter = Counter::DEFAULT;

#[cfg(deleteme)]
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
#[cfg(deleteme)]
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

    pub(crate) fn is_visible_for_map(&self, ctx: MapContext) -> bool {
        match ctx {
            MapContext::Global => self.attr_vis_map.into(),
            MapContext::Minimap => self.attr_vis_minimap.into(),
        }
    }
    #[inline]
    pub fn rotation_xyz(&self) -> Vec3 {
        self.rotation.map(Self::rotation_to_xyz).unwrap_or(Vec3::ZERO)
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
}
#[cfg(deleteme)]
pack_attr! {
    impl Attr{keys::InGameVisibility} for &struct{ActivePoi}.attr_vis_space {}
    impl Attr{keys::MapVisibility} for &struct{ActivePoi}.attr_vis_map {}
    impl Attr{keys::MinimapVisibility} for &struct{ActivePoi}.attr_vis_minimap {}
}
#[cfg(deleteme)]
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
#[cfg(deleteme)]
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
#[cfg(deleteme)]
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
#[cfg(deleteme)]
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
#[cfg(deleteme)]
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
#[cfg(deleteme)]
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
#[cfg(deleteme)]
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
#[cfg(deleteme)]
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
#[cfg(deleteme)]
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
#[cfg(deleteme)]
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
#[cfg(deleteme)]
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
#[cfg(deleteme)]
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
#[cfg(deleteme)]
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
#[cfg(deleteme)]
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
#[cfg(deleteme)]
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
#[cfg(deleteme)]
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
#[cfg(deleteme)]
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
#[cfg(deleteme)]
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
#[cfg(deleteme)]
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
#[cfg(deleteme)]
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
#[cfg(deleteme)]
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
#[cfg(deleteme)]
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
#[cfg(deleteme)]
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
#[cfg(deleteme)]
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
