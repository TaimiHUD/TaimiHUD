use {
    super::{ActivePack, PoiExt},
    crate::{
        exports::runtime::Counter,
        render::machine::RenderMachine,
        space::{
            dx11::{InstanceBufferData, RenderBackend},
            resources::{Model, ShaderPair, Texture, Vertex},
            DrawSpace,
            LocalContext,
        },
    },
    anyhow::Context,
    glam::{vec2, vec3, Mat4, Vec3, Vec3Swizzles, Vec4},
    glamour::{Box3, Point3, Vector2},
    std::sync::Arc,
    taimi_d3d::{
        dx11::{
            buffer::{BufferOf, VertexBuffer},
            prelude::*,
        },
        state::PrimitiveTopology,
    },
    taimi_pack::Poi,
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
    pub tint: Vec4,
    pub opacity: f32,
    pub scale: f32,
    pub scale_map: f32,
    pub icon: Arc<Texture>,
}

impl ActivePoi {
    pub fn build(
        loader: &mut ActivePack,
        poi: &Poi,
        poi_idx: usize,
        category_idx: usize,
        device: &Dx11Device,
    ) -> anyhow::Result<ActivePoi> {
        let icon_handle = poi
            .icon_name()
            .ok_or_else(|| anyhow::anyhow!("POI is missing icon. TODO: default icon?"))?;
        let icon_handle = loader.register_texture(icon_handle);
        let icon = loader
            .get_or_load_texture(icon_handle, device)
            .context("Loading poi texture")?;

        let position = poi.position();
        let scale = poi.icon_scale();
        let scale_map = poi.attributes.map_display_size.unwrap_or(20.0);
        let tint = poi.tint();
        let opacity = poi.alpha();

        let edge_len = scale * 2.0;
        let max_diagonal = (edge_len.powi(2) * 2.0).sqrt();
        let bounds = Box3::from_origin_and_size(position, glamour::size3!(max_diagonal));

        Ok(ActivePoi {
            poi_idx,
            category_idx,
            filtered: false,
            bounds,
            position,
            tint,
            opacity,
            scale,
            scale_map,
            icon: icon.clone(),
        })
    }

    pub fn tint(&self) -> Vec4 {
        let mut tint = self.tint;
        tint.w *= self.opacity;
        tint
    }

    pub fn instance_data(&self) -> InstanceBufferData {
        InstanceBufferData {
            world: Mat4::from_translation(self.position.into()) * Mat4::from_scale(Vec3::splat(self.scale)),
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

    pub fn update(pack: &mut ActivePack, poi_idx: usize) {
        let _ = pack;
        let _ = poi_idx;
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
