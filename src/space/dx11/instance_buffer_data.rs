use {
    glam::{Mat4, Vec4},
    taimi_d3d::buffer::D3dBufferData,
};

#[repr(C, align(16))]
#[derive(Debug, Copy, Clone)]
pub struct InstanceBufferData {
    pub world: Mat4,
    pub colour: Vec4,
}

impl InstanceBufferData {
    pub const IDENTITY: Self = Self {
        world: Mat4::IDENTITY,
        colour: Vec4::ONE,
    };
}

impl Default for InstanceBufferData {
    fn default() -> Self {
        Self::IDENTITY
    }
}

unsafe impl D3dBufferData for InstanceBufferData {}
