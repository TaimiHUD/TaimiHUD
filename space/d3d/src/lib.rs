pub mod blob;
pub mod buffer;
pub mod device;
#[cfg(feature = "dx11")]
pub mod dx11;
#[doc(hidden)]
pub mod macros;
pub mod shader;
pub mod state;

pub mod prelude {
    pub use {
        arcffi::cstr,
        crate::{
            blob::Blob,
            buffer::{
                dxgi::DxgiFormat,
                D3dContextBindableVertexBuffer as _,
                D3dBufferData,
            },
            state::{
                D3dState as _,
                D3dStateToken,
            },
            D3dContext,
            D3dContextBindable as _, D3dContextBindableSlot as _,
        },
        windows::{
            core::{BOOL, Interface as _, InterfaceRef},
            Win32::Graphics::{
                Direct3D::{
                    self as d3d,
                    ID3DBlob,
                },
                Dxgi::{
                    Common as dxgi,
                    IDXGISwapChain,
                },
            },
        },
    };
    #[allow(unused_imports)]
    pub(crate) use {
        anyhow::{anyhow, Context},
        crate::{
            impl_d3d,
            D3dInterfacePtr,
        },
        glamour::{
            Box2, Box3,
            Contains as _,
            Intersection as _,
            Point2, Point3,
            Scalar,
            Size2, Size3,
            Unit, Vector3,
        },
        glam::Vec4,
        std::{ops, mem, ptr, slice},
        windows::core::PCSTR,
    };
}

pub mod defaults {
    #[cfg(feature = "dx11")]
    pub use crate::dx11::{
        self as dx,
        Dx11Buffer as DxBuffer,
        Dx11Device as DxDevice,
        Dx11Context as DxContext,
        Dx11ContextRef as DxContextRef,
    };
}

#[cfg(feature = "windows")]
pub use windows::Win32::Graphics::{
    Direct3D as d3d,
    Dxgi::{
        self as dx,
        Common as dxgi,
    },
};

pub use self::{
    buffer::dxgi::DxgiFormat,
    device::{
        D3dContext, D3dDevice,
        D3dContextBindable, D3dContextBindableSlot,
    },
};

pub unsafe trait D3dInterfacePtr: Clone {
    #[cfg(feature = "windows")]
    type Interface: windows::core::Interface;

    #[cfg(feature = "windows")]
    fn as_d3d_param(&self) -> &Option<Self::Interface>;
    #[cfg(feature = "windows")]
    fn into_d3d_param(self) -> Option<Self::Interface>;
    #[cfg(feature = "windows")]
    fn from_d3d_param(param: &Self::Interface) -> &Self;
}
