pub use crate::dx11::d3d11::{
    ID3D11Device,
    ID3D11Device1,
    ID3D11Device2,
    ID3D11Device3,
    ID3D11Device4,
    ID3D11Device5,
};
use crate::{
    device::D3dDevice,
    dx11::{buffer::ID3D11Buffer, context::DeviceContext0, prelude::*},
};

impl D3dDevice for ID3D11Device {
    type IBuffer = ID3D11Buffer;
}

impl_d3d! {
    unsafe impl D3dInterfacePtr for ID3D11Device;

    @[transparent(D3dInterfacePtr <= ID3D11Device)]
    pub struct Device0.device
        @deref(ID3D11Device)
    ;
}

impl Device0 {
    pub fn get_immediate_context(&self) -> anyhow::Result<DeviceContext0> {
        unsafe { self.device.GetImmediateContext() }
            .context("ID3D11Device::GetImmediateContext")
            .map(Into::into)
    }
}

impl_d3d! {
    @[transparent(D3dInterfacePtr <= ID3D11Device1)]
    pub struct Device1 {
        pub device0: Device0,
    }
    @deref(Device0);
}

// TODO: more versions?
