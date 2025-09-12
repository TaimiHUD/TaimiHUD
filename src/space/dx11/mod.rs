pub mod backend;
pub mod blending_handler;
pub mod depth_handler;
pub mod instance_buffer;
pub mod instance_buffer_data;
pub mod perspective_handler;
pub mod perspective_input_data;
pub mod vertex_buffer;

pub use {
    backend::RenderBackend, blending_handler::BlendingHandler, depth_handler::DepthHandler,
    instance_buffer::InstanceBuffer, instance_buffer_data::InstanceBufferData,
    perspective_handler::PerspectiveHandler, perspective_input_data::PerspectiveInputData,
    vertex_buffer::VertexBuffer,
};

use windows::Win32::Graphics::Direct3D11::ID3D11Buffer;

/// SAFETY: trust me
pub(crate) unsafe fn cb_as_cb_list(cb: &ID3D11Buffer) -> &[Option<ID3D11Buffer>; 1] {
    std::mem::transmute(cb)
}
