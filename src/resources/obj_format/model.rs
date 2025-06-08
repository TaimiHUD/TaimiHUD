use {
    super::super::{Model, Vertex},
    glam::{Vec2, Vec3, Vec3Swizzles},
    tobj::Model as tobjModel,
};

pub struct ObjModel(pub tobjModel);
impl ObjModel {
    pub fn load(&self, xzy: bool) -> Model {
        let mesh = &self.0.mesh;
        let mut vertices = Vec::new();
        for index in mesh.indices.iter() {
            let start = *index as usize * 3;
            let end = *index as usize * 3 + 3;
            let start_2d = *index as usize * 2;
            let end_2d = *index as usize * 2 + 2;
            let vertex = &mesh
                .positions
                .get(start..end)
                .map(Vec3::from_slice)
                .map(|v| if xzy { v.xzy() } else { v })
                .unwrap_or_default();
            let colour = &mesh
                .vertex_color
                .get(start..end)
                .map(Vec3::from_slice)
                .unwrap_or(Vec3::new(1.0, 1.0, 1.0));
            let normal = &mesh
                .normals
                .get(start..end)
                .map(Vec3::from_slice)
                .unwrap_or_default();
            let texture = &mesh
                .texcoords
                .get(start_2d..end_2d)
                .map(Vec2::from_slice)
                .unwrap_or_default();
            vertices.push(Vertex {
                position: *vertex,
                colour: *colour,
                normal: *normal,
                texture: *texture,
            })
        }
        Model::from_vertices(vertices)
    }
}
