use {
    crate::exports::runtime::textures::TextureSlot,
    taimi_d3d::{dx11::prelude::*, D3dContextBindableSlot},
};

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

impl D3dContextBindableSlot<Dx11Context> for TextureSlot {
    fn set(&self, context: &Dx11Context, slot: u32) {
        if let Some(view) = self.resource_view() {
            view.set(context, slot)
        }
    }
}
