use {
    glam::{Vec2, Vec3},
    taimi_d3d::buffer::D3dBufferData,
};

#[derive(Debug, Copy, Clone, Default, PartialEq)]
#[repr(C)]
pub struct Vertex {
    pub position: Vec3,
    pub colour: Vec3,
    pub normal: Vec3,
    pub texture: Vec2,
}

unsafe impl D3dBufferData for Vertex {}
