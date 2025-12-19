use {
    crate::{
        controller::pathing::{
            space::SpaceLoader,
            visible::LoadedPoi,
        },
        exports::runtime::Counter,
        render::machine::RenderMachine,
        space::{
            dx11::{InstanceBufferData, RenderBackend},
            resources::{Model, ShaderPair, Texture, Vertex},
            pack::PackRenderData,
        },
        TEXTURES,
    }, anyhow::Context, bitvec::vec::BitVec, glam::{vec2, vec3, Mat4, Vec3, Vec3Swizzles}, glamour::Vector2, std::sync::Arc, taimi_d3d::{
        dx11::{
            buffer::{BufferOf, VertexBuffer},
            prelude::*,
        },
        state::PrimitiveTopology,
    }, taimi_meta::ui::LocalContext,
    taimi_pack::attributes::AttrString,
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

    pub fallback_texture: Option<Arc<Texture>>,
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

    pub fn update(&mut self, device: &Dx11Device, machine: &RenderMachine, packs: &[PackRenderData]) -> anyhow::Result<()> {
        if self.fallback_texture.is_none() {
            self.fallback_texture = TEXTURES.lookup_resource(RenderMachine::TEXTURE_LOGO_KEY).flatten();
        }

        let ib_len = self.ib_len_for_packs(packs);
        let ib_dirty = !self.is_empty() && self.ib_len() == ib_len;
        if !ib_dirty {
            return Ok(())
        }

        self.rebuild_ib(device, machine, packs)?;

        Ok(())
    }
    pub fn rebuild_ib(&mut self, device: &Dx11Device, machine: &RenderMachine, packs: &[PackRenderData]) -> anyhow::Result<()> {
        let ib_len = self.ib_len_for_packs(packs);
        if ib_len == 0 {
            // usually we'd reserve one for trails but this probably means 0 packs loaded?
            return Ok(())
        }
        let mut data_world = vec![InstanceBufferData::IDENTITY; ib_len];
        let mut data_map = vec![InstanceBufferData::IDENTITY; ib_len];
        self.write_ib(machine, packs, &mut data_world, &mut data_map)?;

        let (data_world, data_map) = (&data_world[..], &data_map[..]);
        STATS_POI_INSTANCE_SIZE
            .reset_with(|| (size_of_val(data_map) + size_of_val(data_world)) as _);
        let (poi_ib_world, poi_ib_map) = (
            BufferOf::new_with_data(device, Ok(data_world), ())?,
            BufferOf::new_with_data(device, Ok(data_map), ())?,
        );
        self.world_ib = Some(poi_ib_world);
        self.map_ib = Some(poi_ib_map);
        Ok(())
    }
    pub fn write_ib(&self, machine: &RenderMachine, packs: &[PackRenderData], ib_world: &mut [InstanceBufferData], ib_map: &mut [InstanceBufferData]) -> anyhow::Result<()> {
        let ib_len = self.ib_len_for_packs(packs);
        if (ib_world.len() > 1 && ib_world.len() != ib_len) || (ib_map.len() > 1 && ib_map.len() != ib_len) {
            anyhow::bail!("expected {ib_len} POI instances, got {}(world) and {}(map) instead", ib_world.len(), ib_map.len());
        }
        let mut gaps: BitVec = BitVec::with_capacity(ib_len);
        gaps.resize(ib_len, false);
        for (_packi, pack) in packs.iter().enumerate() {
            for ((i, poi), lpoi) in pack.render_poi_bookmarks().zip(pack.pois.values().zip(pack.loaded_pois.values())) {
                let index = i as usize;
                if let Some(mut b) = gaps.get_mut(index) {
                    if *b {
                        log::error!("POI instance {i} of pack#{_packi} duplicated, ignoring???");
                        continue
                    }
                    *b = true;
                }
                if let Some(world) = ib_world.get_mut(index) {
                    *world = poi.instance_data(lpoi);
                }
                if let Some(map) = ib_map.get_mut(index) {
                    *map = poi.instance_data_map(machine, lpoi);
                }
            }
        }
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
    fn ib_len_for_packs(&self, packs: &[PackRenderData]) -> usize {
        packs.iter()
            .map(|p| p.render_poi_bookmarks().end as usize).max()
            .map(|l| l.min(1))
            .unwrap_or(0)
    }
    fn ib_len(&self) -> usize {
        let ib = self.world_ib.as_ref()
            .or(self.map_ib.as_ref());
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
    pub icon: Option<Arc<Texture>>,
    #[cfg(todo)]
    pub render_bookmark: u32,
}
impl PoiRender {
    pub const EMPTY: Self = Self {
        icon: None,
    };

    pub fn setup_texture(
        &mut self,
        loader: &mut SpaceLoader<'_>,
        icon_name: &AttrString,
    ) -> anyhow::Result<()> {
        if self.icon.is_some() { return Ok(()) }

        let handle = loader.register_texture(icon_name);
        let icon = loader
            .get_or_load_texture(handle)
            .context("Loading poi texture");

        match icon {
            Ok(icon) => {
                self.icon = Some(icon);
                Ok(())
            },
            Err(e) => {
                self.icon = None;
                Err(e)
            },
        }
    }

    pub fn instance_data(&self, poi: &LoadedPoi) -> InstanceBufferData {
        let render = poi.render_attrs();
        let attrs = poi.poi_attrs();
        InstanceBufferData {
            world: Mat4::from_translation(poi.position.into()) * Mat4::from_scale(Vec3::splat(attrs.icon_size())),
            colour: render.tint(),
        }
    }

    pub fn instance_data_map(&self, poi: &LoadedPoi, machine: &RenderMachine) -> InstanceBufferData {
        // pixels at 1.0 map scale, translated to local space, but quad is 2.0x2.0...
        let scale_map = poi.poi_attrs().map_display_size();
        let size = Vector2::splat(scale_map / 2.0);

        // TODO: DPI/UI scaling is irrelevant here right?
        let scale = size * machine.map.calibration.local_space().scale.abs();
        InstanceBufferData {
            world: Mat4::from_translation(poi.position.into())
                * Mat4::from_scale(scale.extend(scale.y).into()),
            colour: poi.render_attrs().tint(),
        }
    }

    pub fn bind_texture(&self, device_context: &Dx11Context, common: &PoiCommonRenderData, _ctx: LocalContext) {
        let texture = self.icon.as_ref()
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
