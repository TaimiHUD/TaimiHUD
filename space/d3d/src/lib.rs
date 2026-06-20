//! DirectX 11 abstractions and rendering primitives for TaimiHUD.
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
    #[cfg(feature = "arcffi")]
    pub use arcffi::cstr;
    pub use {
        crate::{
            blob::Blob,
            buffer::{dxgi::DxgiFormat, D3dBufferData, D3dContextBindableVertexBuffer as _},
            state::{D3dState as _, D3dStateToken},
            D3dContext,
            D3dContextBindable as _,
            D3dContextBindableSlot as _,
        },
        windows::{
            core::{Interface as _, InterfaceRef, BOOL},
            Win32::Graphics::{
                Direct3D::{self as d3d, ID3DBlob},
                Dxgi::{Common as dxgi, IDXGISwapChain},
            },
        },
    };
    #[allow(unused_imports)]
    pub(crate) use {
        crate::{impl_d3d, D3dInterfacePtr},
        anyhow::{anyhow, Context},
        glam::Vec4,
        glamour::{
            Box2,
            Box3,
            Contains as _,
            Intersection as _,
            Point2,
            Point3,
            Rect,
            Scalar,
            Size2,
            Size3,
            Unit,
            Vector3,
        },
        std::{mem, ops, ptr, slice},
        windows::core::PCSTR,
    };
}

pub mod defaults {
    #[cfg(feature = "dx11")]
    pub use crate::dx11::{
        self as dx,
        Dx11Buffer as DxBuffer,
        Dx11Context as DxContext,
        Dx11ContextRef as DxContextRef,
        Dx11Device as DxDevice,
    };
}

#[cfg(feature = "windows")]
pub use windows::Win32::Graphics::{
    Direct3D as d3d,
    Dxgi::{self as dx, Common as dxgi},
};

pub use self::{
    buffer::dxgi::DxgiFormat,
    device::{D3dContext, D3dContextBindable, D3dContextBindableSlot, D3dDevice},
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
