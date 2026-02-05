pub use crate::dx11::d3d11::{
    ID3D11DeviceContext,
    ID3D11DeviceContext1,
    ID3D11DeviceContext2,
    ID3D11DeviceContext3,
    ID3D11DeviceContext4,
};
use crate::{
    device::{D3dContext, D3dContextBindable},
    dx11::{device::ID3D11Device, prelude::*},
    state::{D3dStateSnapshot, PrimitiveTopology},
};

impl D3dContext for ID3D11DeviceContext {
    type IDevice = ID3D11Device;
}

impl_d3d! {
    unsafe impl Dx11Child for ID3D11DeviceContext;

    @[transparent(D3dInterfacePtr <= ID3D11DeviceContext)]
    pub struct DeviceContext0.context
        @deref(ID3D11DeviceContext)
    ;
}

impl_d3d! {
    @[transparent(D3dInterfacePtr <= ID3D11DeviceContext1)]
    pub struct DeviceContext1 {
        pub context0: DeviceContext0,
    }
    @into()
    @deref(DeviceContext0);
}

impl_d3d! {
    @[transparent(D3dInterfacePtr <= ID3D11DeviceContext2)]
    pub struct DeviceContext2 {
        pub context1: DeviceContext1,
    }
    @into()
    @deref(DeviceContext1);
}

impl_d3d! {
    @[transparent(D3dInterfacePtr <= ID3D11DeviceContext3)]
    pub struct DeviceContext3 {
        pub context2: DeviceContext2,
    }
    @into()
    @deref(DeviceContext2);
}

impl_d3d! {
    @[transparent(D3dInterfacePtr <= ID3D11DeviceContext4)]
    pub struct DeviceContext4 {
        pub context3: DeviceContext3,
    }
    @into()
    @deref(DeviceContext3);
}

impl D3dContextBindable<Dx11Context> for PrimitiveTopology {
    fn set(&self, device_context: &Dx11Context) {
        unsafe { device_context.IASetPrimitiveTopology(self.d3d()) }
    }
}
impl D3dStateSnapshot<Dx11Context> for PrimitiveTopology {
    fn empty_state(_: &Dx11Device) -> anyhow::Result<Self> {
        Ok(Self::Undefined)
    }
    fn snapshot_state(device_context: &Dx11Context) -> Self {
        Self::from_d3d(unsafe { device_context.IAGetPrimitiveTopology() })
    }
}
