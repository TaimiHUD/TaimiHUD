pub mod backend;
pub mod blending_handler;
pub mod depth_handler;
pub mod instance_buffer;
pub mod instance_buffer_data;
pub mod perspective_handler;
pub mod perspective_input_data;
pub mod vertex_buffer;

pub mod prelude {
    pub use {
        super::{
            ID3D11ResourceExt as _,
            D3d11ContextBindable, D3d11ContextBindableSlot,
            vertex_buffer::D3d11ContextBindableVertexBuffer,
        },
        windows::{
            core::{Interface as _, InterfaceRef},
            Win32::Graphics::Direct3D11::{
                self as d3d11,
                ID3D11Buffer, ID3D11Device, ID3D11DeviceContext,
            },
        },
    };
}

pub use {
    backend::RenderBackend, blending_handler::BlendingHandler, depth_handler::DepthHandler,
    instance_buffer::InstanceBuffer, instance_buffer_data::InstanceBufferData,
    perspective_handler::PerspectiveHandler, perspective_input_data::PerspectiveInputData,
    vertex_buffer::VertexBuffer,
};

use {
    crate::space::object::PrimitiveTopology,
    std::{array, mem},
    windows::{
        core::InterfaceRef,
        Win32::Graphics::Direct3D11::{
            self as d3d11,
            ID3D11Buffer, ID3D11DeviceContext,
        },
    },
};

pub trait D3d11ContextBindable {
    fn set(&self, device_context: &ID3D11DeviceContext);
}

pub trait D3d11ContextBindableSlot {
    fn set(&self, device_context: &ID3D11DeviceContext, slot: u32);
}

pub trait ID3D11ResourceExt {
    type Output: windows::core::imp::CanInto<d3d11::ID3D11Resource>;

    fn as_params(&self) -> &[Option<Self::Output>; 1] where Self: Sized;
}

impl ID3D11ResourceExt for ID3D11Buffer {
    type Output = Self;

    /// SAFETY: trust me
    fn as_params(&self) -> &[Option<Self::Output>; 1] {
        let single: &[Self; 1] = array::from_ref(self);
        unsafe {
            mem::transmute(single)
        }
    }
}

impl ID3D11ResourceExt for Option<ID3D11Buffer> {
    type Output = ID3D11Buffer;

    fn as_params(&self) -> &[Option<Self::Output>; 1] {
        array::from_ref(self)
    }
}

impl ID3D11ResourceExt for InterfaceRef<'_, ID3D11Buffer> {
    type Output = ID3D11Buffer;

    /// SAFETY: trust me
    fn as_params(&self) -> &[Option<Self::Output>; 1] {
        let single: &[Self; 1] = array::from_ref(self);
        unsafe {
            mem::transmute(single)
        }
    }
}

impl ID3D11ResourceExt for Option<InterfaceRef<'_, ID3D11Buffer>> {
    type Output = ID3D11Buffer;

    /// SAFETY: trust me
    fn as_params(&self) -> &[Option<Self::Output>; 1] {
        let single: &[Self; 1] = array::from_ref(self);
        unsafe {
            mem::transmute(single)
        }
    }
}
