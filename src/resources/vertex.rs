use glam::{Vec2, Vec3};

#[derive(Copy, Clone, PartialEq)]
#[repr(C)]
pub struct Vertex {
    pub position: Vec3,
    pub colour: Vec3,
    pub normal: Vec3,
    pub texture: Vec2,
}
