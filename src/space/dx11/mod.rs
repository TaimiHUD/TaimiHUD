pub mod backend;
pub mod depth_handler;
pub mod instance_buffer_data;
pub mod perspective_handler;
pub mod perspective_input_data;

#[cfg(todo)]
pub mod prelude {
    pub use taimi_d3d::dx11::prelude::*;
    // XXX: add common buffer/resource types
}

pub use {
    backend::RenderBackend, depth_handler::DepthHandler,
    instance_buffer_data::InstanceBufferData,
    perspective_handler::PerspectiveHandler, perspective_input_data::PerspectiveInputData,
};
