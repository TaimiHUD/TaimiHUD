use crate::{
    dx::{self, DXGI_RESOURCE_PRIORITY},
    dx11::{
        prelude::*,
        buffer::{
            Buffer, Texture2,
            ID3D11Buffer, ID3D11Texture2D,
        },
    },
};

pub use crate::dx11::d3d11::{
    ID3D11Resource,
    D3D11_RESOURCE_DIMENSION,
};

impl_d3d! {
    unsafe impl Dx11Child for ID3D11Resource;

    @[transparent(Dx11Child <= ID3D11Resource)]
    pub struct Resource.resource;
}

impl Resource {
    pub const PRIO_MIN: DXGI_RESOURCE_PRIORITY = dx::DXGI_RESOURCE_PRIORITY_MINIMUM;
    pub const PRIO_LOW: DXGI_RESOURCE_PRIORITY = dx::DXGI_RESOURCE_PRIORITY_LOW;
    pub const PRIO_NORMAL: DXGI_RESOURCE_PRIORITY = dx::DXGI_RESOURCE_PRIORITY_NORMAL;
    pub const PRIO_HIGH: DXGI_RESOURCE_PRIORITY = dx::DXGI_RESOURCE_PRIORITY_HIGH;
    pub const PRIO_MAX: DXGI_RESOURCE_PRIORITY = dx::DXGI_RESOURCE_PRIORITY_MAXIMUM;

    pub fn get_type_d3d(&self) -> D3D11_RESOURCE_DIMENSION {
        unsafe {
            self.as_d3d().GetType()
        }
    }

    pub fn get_type(&self) -> ResourceDimension {
        ResourceDimension::try_from_d3d(self.get_type_d3d())
            .unwrap_or(ResourceDimension::Unknown)
    }

    pub fn as_buffer(&self) -> Option<&Buffer> {
        match self.get_type_d3d() {
            ResourceDimension::BUFFER => Some(unsafe {
                Buffer::from_d3d_ref(mem::transmute::<&ID3D11Resource, &ID3D11Buffer>(self.as_d3d()))
            }),
            _ => None,
        }
    }

    pub fn try_into_buffer(self) -> Result<Buffer, Self> {
        match self.get_type_d3d() {
            ResourceDimension::BUFFER => Ok(unsafe {
                Buffer::from_d3d(mem::transmute::<ID3D11Resource, ID3D11Buffer>(self.into_d3d()))
            }),
            _ => Err(self),
        }
    }

    pub fn as_texture2(&self) -> Option<&Texture2> {
        match self.get_type_d3d() {
            ResourceDimension::TEXTURE2D => Some(unsafe {
                Texture2::from_d3d_ref(mem::transmute::<&ID3D11Resource, &ID3D11Texture2D>(self.as_d3d()))
            }),
            _ => None,
        }
    }

    pub fn try_into_texture2(self) -> Result<Texture2, Self> {
        match self.get_type_d3d() {
            ResourceDimension::TEXTURE2D => Ok(unsafe {
                Texture2::from_d3d(mem::transmute::<ID3D11Resource, ID3D11Texture2D>(self.into_d3d()))
            }),
            _ => Err(self),
        }
    }

    pub fn get_eviction_prio(&self) -> DXGI_RESOURCE_PRIORITY {
        DXGI_RESOURCE_PRIORITY(unsafe {
            self.as_d3d().GetEvictionPriority()
        })
    }

    pub fn set_eviction_prio(&self, priority: DXGI_RESOURCE_PRIORITY) {
        unsafe {
            self.as_d3d().SetEvictionPriority(priority.0)
        }
    }
}

impl_d3d! { impl enum for
    #[derive(Default)]
    pub enum ResourceDimension: D3D11_RESOURCE_DIMENSION{i32} {
        #[default]
        Unknown(const UNKNOWN) = d3d11::D3D11_RESOURCE_DIMENSION_UNKNOWN,
        Buffer(const BUFFER) = d3d11::D3D11_RESOURCE_DIMENSION_BUFFER,
        Texture1(const TEXTURE1D) = d3d11::D3D11_RESOURCE_DIMENSION_TEXTURE1D,
        Texture2(const TEXTURE2D) = d3d11::D3D11_RESOURCE_DIMENSION_TEXTURE2D,
        Texture3(const TEXTURE3D) = d3d11::D3D11_RESOURCE_DIMENSION_TEXTURE3D,
    },
}
