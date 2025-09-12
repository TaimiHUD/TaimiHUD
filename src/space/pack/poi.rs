use {
    super::{ActivePack, PoiExt},
    crate::space::{
        dx11::{RenderBackend, InstanceBuffer, InstanceBufferData, VertexBuffer},
        resources::{Model, ShaderPair, Texture, Vertex},
        DrawSpace, LocalContext,
    },
    anyhow::Context,
    glam::{vec2, vec3, Mat4, Vec3, Vec3Swizzles, Vec4},
    glamour::{Box3, Point3},
    std::sync::Arc,
    taimi_pack::Poi,
    windows::Win32::Graphics::{
        Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
        Direct3D11::{
            ID3D11Device, ID3D11DeviceContext,
        },
    },
};

pub struct PoiCommonRenderData {
    // Common fixed data.
    /// POI shader.
    pub shaders: ShaderPair,
    /// Quad trianglestrip.
    quad_vb: VertexBuffer,

    pub world_ib: Option<InstanceBuffer>,
    pub map_ib: Option<InstanceBuffer>,
}

// NOTES: Please reference https://github.com/blish-hud/Pathing/blob/main/Entity/StandardMarker.World.cs

impl PoiCommonRenderData {
    pub fn new(backend: &RenderBackend) -> anyhow::Result<PoiCommonRenderData> {
        let mut vertices = Vec::from(Self::quad(LocalContext::World));
        vertices.extend_from_slice(&Self::quad(LocalContext::MAP));

        let quad_vb = Model::from_vertices(vertices).to_buffer(&backend.device)?;

        Ok(PoiCommonRenderData {
            shaders: ShaderPair(
                backend
                    .shaders
                    .0
                    .get("poi")
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("Failed to load POI vertex shader"))?,
                backend
                    .shaders
                    .1
                    .get("poi")
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("Failed to load POI pixel shader"))?,
            ),
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
                }
                vertices
            },
        }
    }

    pub fn set(&self, device_context: &ID3D11DeviceContext) {
        self.shaders.set(device_context);
        self.set_vertex(device_context, LocalContext::World);
        self.set_instance(device_context, LocalContext::World);
    }

    pub const SLOT_VB: u32 = 0;
    pub fn set_vertex(&self, device_context: &ID3D11DeviceContext, ctx: LocalContext) {
        let offset = self.quad_vb.offset;
        #[cfg(todo)]
        let offset = match ctx {
            LocalContext::World => self.quad_vb.offset,
            // offset buffer directly if not passed to Draw()
            LocalContext::Map(..) => self.quad_vb.offset + self.quad_vb.stride * POI_QUAD_VERTICES.len() as u32,
        };
        unsafe {
            device_context.IASetVertexBuffers(
                Self::SLOT_VB,
                1,
                Some(&self.quad_vb.buffer as *const _ as *const _),
                Some(&self.quad_vb.stride),
                Some(&offset),
            );
            //self.set_primitive();
        }
    }

    pub const SLOT_IB: u32 = 1;
    pub fn set_instance(&self, device_context: &ID3D11DeviceContext, ctx: LocalContext) {
        let vb = match ctx {
            LocalContext::World => &self.world_ib,
            LocalContext::Map(..) => &self.map_ib,
        };
        let vb = match vb {
            Some(vb) => vb,
            None => {
                log::warn!("can't draw without POI instance buffer");
                return
            },
        };
        vb.set(device_context, Self::SLOT_IB);
    }

    pub fn set_primitive(&self, device_context: &ID3D11DeviceContext) {
        unsafe {
            device_context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP);
        }
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
        device: &ID3D11Device,
    ) -> anyhow::Result<ActivePoi> {
        let icon_handle = poi.icon_name()
            .ok_or_else(|| anyhow::anyhow!("POI is missing icon. TODO: default icon?"))?;
        let icon_handle = loader.register_texture(icon_handle);
        let icon = loader.get_or_load_texture(icon_handle, device)
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

    pub fn instance_data_map(&self) -> InstanceBufferData {
        use glamour::TransformMap;
        // pixels at 1.0 map scale, translated to local space, but quad is 2.0x2.0...
        let size_px = self.scale_map / 2.0;
        let scale = crate::marker::atomic::MarkerInputData::read()
            .map(|data| data.minimap_to_map_with(None, 1.0).then(data.map_to_local()).map(glamour::Vector2::<_>::splat(size_px)).x)
            .unwrap_or(size_px * 0.64f32);
        InstanceBufferData {
            world: Mat4::from_translation(self.position.into()) * Mat4::from_scale(Vec3::splat(scale)),
            colour: self.tint(),
        }
    }

    pub fn update(pack: &mut ActivePack, poi_idx: usize) {
        let _ = pack;
        let _ = poi_idx;
    }

    pub fn draw(&self, device_context: &ID3D11DeviceContext, render_idx: usize, ctx: LocalContext) {
        self.icon.set(device_context, 0);
        let voffset = match ctx {
            LocalContext::World => 0,
            LocalContext::Map(..) => PoiCommonRenderData::VERTEX_OFFSET_MAP as u32,
        };
        unsafe {
            device_context.DrawInstanced(PoiCommonRenderData::VERTEX_COUNT as u32, 1, voffset, render_idx as u32);
        }
        /*self.buffer.set(device_context, 1);
        unsafe {
            device_context.Draw(4, 0);
        }*/
    }
}
