use {
    crate::{
        resources::Vertex,
        space::dx11::VertexBuffer,
    },
    anyhow::anyhow,
    glam::{Vec3, Vec3Swizzles},
    serde::{Deserialize, Serialize},
    windows::Win32::Graphics::Direct3D11::{
        ID3D11Buffer, ID3D11Device, D3D11_BIND_VERTEX_BUFFER, D3D11_BUFFER_DESC,
        D3D11_SUBRESOURCE_DATA, D3D11_USAGE_DEFAULT,
    },
};

// TODO: cut down on this
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ModelKind {
    #[default]
    Obj,
}

#[derive(Default, PartialEq, Clone)]
pub struct Model(Vec<Vertex>);

impl Model {
    pub fn from_vertices(vertices: Vec<Vertex>) -> Self {
        Self(vertices)
    }

    pub fn quad() -> anyhow::Result<Self> {
        let mut vertices = Vec::new();
        let height = 1.0;
        let width = 1.0;
        let vertex_coordinates = [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, height, 0.0),
            Vec3::new(width, height, 0.0),
            Vec3::new(width, height, 0.0),
            Vec3::new(width, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 0.0),
        ];
        let colour = Vec3::new(1.0, 1.0, 1.0);
        let mut normal = Vec3::new(0.0, 0.0, 0.0);
        for i in 0..vertex_coordinates.len() {
            let current = vertex_coordinates[i];
            let next_idx = (i + 1) % vertex_coordinates.len();
            let next = vertex_coordinates[next_idx];
            normal += Vec3::new(
                (current.y - next.y) * (current.z + next.z),
                (current.z - next.z) * (current.x + next.x),
                (current.x - next.x) * (current.y + next.y),
            );
            vertices.push(Vertex {
                position: current - Vec3::new(width / 2.0, height / 2.0, 0.0),
                normal,
                texture: current.xy(),
                colour,
            });
        }

        Ok(Self(vertices))
    }

    pub fn to_buffer(&self, device: &ID3D11Device) -> anyhow::Result<VertexBuffer> {
        let vertex_data_array: &[Vertex] = self.0.as_slice();

        let stride: u32 = size_of::<Vertex>() as u32;
        let offset: u32 = 0;
        let count: u32 = vertex_data_array.len() as u32;

        log::info!("Setting up vertex buffer");
        let mut vertex_buffer_ptr: Option<ID3D11Buffer> = None;
        let subresource_data = D3D11_SUBRESOURCE_DATA {
            pSysMem: vertex_data_array.as_ptr() as *const _,
            SysMemPitch: 0,
            SysMemSlicePitch: 0,
        };
        let vertex_buffer_desc = D3D11_BUFFER_DESC {
            ByteWidth: size_of_val(vertex_data_array) as u32,
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_VERTEX_BUFFER.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
            StructureByteStride: 0,
        };
        log::info!("Creating vertex buffer with {} elements", count);
        let buffer = unsafe {
            device.CreateBuffer(
                &vertex_buffer_desc,
                Some(&subresource_data),
                Some(&mut vertex_buffer_ptr),
            )
        }
        .map_err(anyhow::Error::from)
        .and_then(|()| vertex_buffer_ptr.ok_or_else(|| anyhow!("no vertex buffer")))?;

        let vertex_buffer = VertexBuffer {
            buffer,
            stride,
            offset,
            count,
        };
        Ok(vertex_buffer)
    }
}
