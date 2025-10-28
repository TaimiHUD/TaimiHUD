pub mod backend;
pub mod depth_handler;
pub mod instance_buffer_data;
pub mod perspective_handler;

#[cfg(todo)]
pub mod prelude {
    pub use taimi_d3d::dx11::prelude::*;
    // XXX: add common buffer/resource types
}

pub use {
    self::{
        backend::RenderBackend,
        depth_handler::DepthHandler,
        instance_buffer_data::InstanceBufferData,
        perspective_handler::PerspectiveHandler,
    },
    taimi_d3d::device::SwapChain0 as SwapChain,
};
