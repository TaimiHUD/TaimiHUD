use {
    super::{ActivePack, PoiExt},
    crate::space::{
        dx11::{RenderBackend, InstanceBuffer, InstanceBufferData, VertexBuffer},
        resources::{Model, ShaderPair, Texture, Vertex},
        DrawSpace,
    },
    anyhow::Context,
    glam::{vec2, vec3, Mat4, Vec3, Vec4},
    glamour::{Box3, Point3},
    std::sync::Arc,
    taimi_pack::Poi,
    windows::Win32::Graphics::{
        Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
        Direct3D11::{
            ID3D11Buffer, ID3D11Device, ID3D11DeviceContext, D3D11_BIND_CONSTANT_BUFFER,
            D3D11_BUFFER_DESC, D3D11_SUBRESOURCE_DATA, D3D11_USAGE_DEFAULT,
        },
    },
};

pub struct PoiCommonRenderData {
    // Common fixed data.
    /// POI shader.
    pub shaders: ShaderPair,
    /// Quad trianglestrip.
    quad_vb: VertexBuffer,
}

// NOTES: Please reference https://github.com/blish-hud/Pathing/blob/main/Entity/StandardMarker.World.cs

impl PoiCommonRenderData {
    pub fn new(backend: &RenderBackend) -> anyhow::Result<PoiCommonRenderData> {
        let quad_vb = Model::from_vertices(POI_QUAD_VERTICES.into()).to_buffer(&backend.device)?;

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
        })
    }

    pub fn set(&self, device_context: &ID3D11DeviceContext) {
        self.shaders.set(device_context);
        unsafe {
            device_context.IASetVertexBuffers(
                0,
                1,
                Some(&self.quad_vb.buffer as *const _ as *const _),
                Some(&self.quad_vb.stride),
                Some(&self.quad_vb.offset),
            );
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

fn create_poi_cb(device: &ID3D11Device) -> anyhow::Result<ID3D11Buffer> {
    let constant_buffer_desc = D3D11_BUFFER_DESC {
        ByteWidth: size_of::<PoiSpriteData>().next_multiple_of(16) as u32,
        //Usage: D3D11_USAGE_DYNAMIC,
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
        //CPUAccessFlags: D3D11_CPU_ACCESS_WRITE,
        CPUAccessFlags: 0,
        MiscFlags: 0,
        StructureByteStride: 0,
    };

    let initial = PoiSpriteData {
        model: Default::default(),
        tint: Default::default(),
    };

    let constant_subresource_data = D3D11_SUBRESOURCE_DATA {
        pSysMem: &initial as *const PoiSpriteData as *const _,
        .. D3D11_SUBRESOURCE_DATA::default()
    };

    let mut constant_buffer_ptr: Option<ID3D11Buffer> = None;
    let constant_buffer = unsafe {
        device
            .CreateBuffer(
                &constant_buffer_desc,
                Some(&constant_subresource_data),
                Some(&mut constant_buffer_ptr),
            )
            .context("Creating POI ConstantBuffer")?;
        constant_buffer_ptr.expect("ptr should never be NULL on S_OK")
    };
    Ok(constant_buffer)
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct PoiSpriteData {
    model: Mat4,
    tint: Vec4,
}

pub struct ActivePoi {
    pub poi_idx: usize,
    pub category_idx: usize,
    pub filtered: bool,
    pub bounds: Box3<DrawSpace>,
    pub position: Point3<DrawSpace>,
    #[cfg(todo)]
    pub tint: Vec4,
    #[cfg(todo)]
    pub opacity: f32,
    #[cfg(todo)]
    pub scale: f32,
    pub icon: Arc<Texture>,
    pub buffer: InstanceBuffer,
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
        let tint = poi.tint();
        let opacity = poi.alpha();

        let buffer = {
            let data = InstanceBufferData {
                world: Mat4::from_translation(position.into()) * Mat4::from_scale(Vec3::splat(scale)),
                colour: tint * Vec4::ONE.with_w(opacity),
            };
            InstanceBuffer::create(device, &[data])
        }.context("creating POI buffer")?;

        let edge_len = scale * 2.0;
        let max_diagonal = (edge_len.powi(2) * 2.0).sqrt();
        let bounds = Box3::from_origin_and_size(position, glamour::size3!(max_diagonal));

        Ok(ActivePoi {
            poi_idx,
            category_idx,
            filtered: false,
            bounds,
            buffer,
            position,
            #[cfg(todo)]
            tint,
            #[cfg(todo)]
            opacity,
            #[cfg(todo)]
            scale,
            icon: icon.clone(),
        })
    }

    pub fn update(pack: &mut ActivePack, poi_idx: usize) {
        let _ = pack;
        let _ = poi_idx;
    }

    pub fn draw(&self, device_context: &ID3D11DeviceContext) {
        if self.filtered {
            return;
        }

        self.icon.set(device_context, 0);
        self.buffer.set(device_context, 1);
        unsafe {
            device_context.Draw(4, 0);
        }
    }
}
