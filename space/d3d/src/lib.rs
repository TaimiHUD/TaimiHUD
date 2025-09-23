pub mod blob;
#[cfg(feature = "dx11")]
pub mod dx11;
pub mod buffer;
pub mod shader;
pub mod state;

pub mod prelude {
    pub use {
        arcffi::cstr,
        crate::{
            blob::Blob,
            buffer::{
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
        crate::D3dInterfacePtr,
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

use windows::core::Interface;

pub trait D3dContext: Interface {
    type IDevice: D3dDevice;
}
pub trait D3dDevice: Interface {
    type IBuffer: Interface;
}

#[cfg(todo)]
impl<D3DC: D3dContext> D3dContext for &'_ D3DC {
    type IDevice = D3DC::IDevice;
}
#[cfg(todo)]
impl<D3DD: D3dDevice> D3dDevice for &'_ D3DD {
    type IBuffer = D3DD::IBuffer;
}

pub trait D3dContextBindable<D3DC> {
    fn set(&self, device_context: &D3DC);
}

pub trait D3dContextBindableSlot<D3DC> {
    fn set(&self, device_context: &D3DC, slot: u32);
}

impl<T: ?Sized, D3DC: D3dContext> D3dContextBindable<D3DC> for &'_ T where
    T: D3dContextBindable<D3DC>,
{
    fn set(&self, device_context: &D3DC) {
        D3dContextBindable::set(*self, device_context)
    }
}
impl<T: ?Sized, D3DC: D3dContext> D3dContextBindableSlot<D3DC> for &'_ T where
    T: D3dContextBindableSlot<D3DC>,
{
    fn set(&self, device_context: &D3DC, slot: u32) {
        D3dContextBindableSlot::set(*self, device_context, slot)
    }
}

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
