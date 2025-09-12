use glam::{Mat4, Vec4};

#[repr(C, align(16))]
pub struct InstanceBufferData {
    pub world: Mat4,
    pub colour: Vec4,
}
