use {
    crate::resources::Vertex,
    glam::{Vec3, Vec3Swizzles},
    serde::{Deserialize, Serialize},
    std::sync::LazyLock,
    taimi_d3d::dx11::{buffer::VertexBuffer, prelude::*},
};

// TODO: cut down on this
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ModelKind {
    #[default]
    Obj,
}

#[derive(Default, PartialEq, Clone)]
pub struct Model(pub(crate) Vec<Vertex>);

impl Model {
    pub fn from_vertices(vertices: Vec<Vertex>) -> Self {
        Self(vertices)
    }

    pub fn quad() -> &'static Self {
        static QUAD: LazyLock<Model> = LazyLock::new(|| Model::new_quad());
        &*QUAD
    }
    fn new_quad() -> Self {
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

        Self(vertices)
    }

    pub fn to_buffer(&self, device: &Dx11Device) -> anyhow::Result<VertexBuffer> {
        VertexBuffer::new_with_slice(device, &self.0, Default::default())
    }
}
