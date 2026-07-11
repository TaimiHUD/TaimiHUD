use {
    super::{instance::PoiVertex, PackRenderState},
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
    glam::{vec2, vec3, EulerRot, Mat4, Quat, Vec3, Vec3Swizzles},
    glamour::Vector2,
    std::{f32::consts::FRAC_PI_2, fmt, mem},
    taimi_d3d::{
        dx11::{
            buffer::{BufferOf, VertexBuffer},
            prelude::*,
        },
        state::PrimitiveTopology,
    },
    taimi_meta::{packs::id::MarkerId, ui::LocalContext},
    taimi_pack::attributes::{
        cell::{pack_attr, GetAttrDynExt, PackKeyId},
        keys::{self, GetAttr},
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
    ib_len: usize,

    pub fallback_texture: Option<TextureSlot>,
    pub fallback_texture2: Option<TextureSlot>,
    pub fallback_object: Option<(BufferOf<PoiVertex>, u32)>,
    pub fallback_textureo: Option<TextureSlot>,
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
            ib_len: 0,
            fallback_texture: None,
            fallback_texture2: None,
            fallback_object: None,
            fallback_textureo: None,
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
        self.ib_len = 0;
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
        let fallback_obj = self
            .fallback_object
            .is_none()
            .then(|| RenderMachine::logo_object());
        if let Some(Some((o, m))) = fallback_obj {
            #[cfg(taimi_debug)]
            use {crate::exports::runtime as rt, anyhow::Context};

            use crate::resources::obj_format::ObjModel;

            let o = match o {
                &[ref o] => Some(ObjModel::from_ref(o)),
                o => {
                    #[cfg(taimi_debug)]
                    log::debug!("where did Curve go? {o:?}");
                    None
                },
            };
            let _m = match m {
                &[ref m] => Some(m),
                m => {
                    #[cfg(taimi_debug)]
                    log::debug!("where did SVGMat go? {m:?}");
                    None
                },
            };
            let vertices = o.map(|o| {
                o.load(false)
                    .0
                    .into_iter()
                    .map(|v| {
                        PoiVertex::new(
                            // this is a model made for ants .-.
                            (glam::Vec3A::from(v.position) * 82.2f32).into(),
                            // we're weird...
                            glam::Vec2::new(v.texture.x, 1.0 - v.texture.y).into(),
                            Vector2::ZERO,
                        )
                    })
                    .collect::<Vec<_>>()
            });
            self.fallback_object = vertices.and_then(|v| {
                match PoiVertex::alloc(device, &v[..]) {
                    #[cfg(taimi_debug)]
                    vb => rt::log::debug_ok(vb.context("taimihud.obj")),
                    #[cfg(not(taimi_debug))]
                    vb => vb.ok(),
                }
                .map(|vb| (vb, v.len() as u32))
            });
            #[cfg(todo)]
            {
                self.fallback_mat = m;
            }
        }
        if self.fallback_object.is_some() && self.fallback_textureo.is_none() {
            if let Some(texture) = TEXTURES.lookup_loaded(RenderMachine::TEXTURE_GLYPH_HOLO_KEY) {
                self.fallback_textureo = texture;
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
        STATS_POI_INSTANCE_SIZE.reset_with(|| size_of_val(data_map) + size_of_val(data_world));
        let (poi_ib_world, poi_ib_map) = (
            BufferOf::new_with_data(device, Ok(data_world), ())?,
            BufferOf::new_with_data(device, Ok(data_map), ())?,
        );
        self.world_ib = Some(poi_ib_world);
        self.map_ib = Some(poi_ib_map);
        self.ib_len = ib_len;
        Ok(())
    }
    pub fn update_ib_at(
        &self,
        context: &Dx11Context,
        pack: &PackRenderData,
        machine: Option<&RenderMachine>,
        loaded_idx: usize,
    ) -> bool {
        if pack.render_poi_bookmark == 0 || loaded_idx >= self.ib_len {
            return false
        }
        let Some(poi) = pack.pois.get(loaded_idx) else { return false };
        let bookmark = pack.render_poi_bookmark + loaded_idx;
        let mut applied = false;
        let lidx =
            taimi_hoard::loc::Locator::new_path(taimi_meta::packs::MarkerIndex::with_poi(loaded_idx as _));
        let lpoi = pack
            .map_info
            .as_ref()
            .and_then(|mi| {
                crate::controller::pathing::shared::SharedMarkerRef::from_parts(
                    mi,
                    Some(&pack.map_state),
                    lidx,
                )
            })
            .and_then(|m| m.to_loaded_poi());
        let Some(ref lpoi) = lpoi else { return false };
        if let Some(ib) = &self.world_ib {
            unsafe {
                // TODO: if visible_in_space && !arcrender?
                ib.update_element_at(context, &poi.instance_data(lpoi), bookmark, 0);
            }
            applied = true;
        }
        if let (Some(ib), Some(machine)) = (&self.map_ib, machine) {
            // TODO: if visible_on_map?
            unsafe {
                ib.update_element_at(context, &poi.instance_data_map(lpoi, machine), bookmark, 0);
            }
            applied = true;
        }
        applied
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
    pub icon_unset: bool,
    pub anim: Option<f32>,
}
impl PoiRender {
    pub fn empty() -> Self {
        Self {
            icon_handle: None,
            icon: None,
            static_rotation: false,
            occlude: false,
            icon_unset: false,
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
        self.icon_unset = !poi.lpoi_info().marker_info().has_attr_of::<keys::IconFile>()
            && !poi.lpoi_info().marker_info().has_attr_of::<keys::Occlude>();
        if self.icon_unset {
            self.static_rotation = true;
        }
    }

    pub fn instance_data(&self, poi: &LoadedPoiRef) -> InstanceBufferData {
        let render = poi.render_attrs();
        let attrs = poi.poi_attrs();
        InstanceBufferData {
            world: Mat4::from_scale_rotation_translation(
                Vec3::splat(attrs.attr_or_default_into::<keys::IconSize, f32>() * 0.5),
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

    #[cfg(feature = "paths-dyn")]
    pub(crate) fn attr_dirties_render(key: PackKeyId) -> bool {
        pack_attr! { =id_is_in(key, [
            keys::IconSize, keys::MapDisplaySize, keys::MinSize, keys::MaxSize,
            keys::Rotate, keys::Occlude, keys::ScaleOnMapWithZoom,
            keys::Bounce, keys::BounceHeight, keys::BounceDuration, keys::BounceDelay,
            // render common
            keys::Alpha,
            keys::Tint,
            keys::InGameVisibility,
            keys::MapVisibility,
            keys::MinimapVisibility,
            keys::GameMap,
            keys::CanFade, keys::Cull, keys::FadeNear, keys::FadeFar,
        ]) }
    }

    /// TODO: texture manager should handle cleanup explcitly...
    #[inline]
    pub fn cleanup_background(mut self) {
        mem::forget(self.icon.take());
    }
}

pub static STATS_POI_INSTANCE_SIZE: Counter = Counter::DEFAULT;
