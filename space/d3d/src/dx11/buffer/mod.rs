mod buffer;
mod constant;
mod texture2;
mod resource;
mod sampler;
mod vertex;
mod view;

pub use {
    crate::dx11::d3d11::{
        D3D11_BIND_FLAG,
        D3D11_CPU_ACCESS_FLAG,
        D3D11_RESOURCE_MISC_FLAG,
        D3D11_USAGE,
    },
    self::{
        buffer::{
            D3D11_BOX,
            D3D11_BUFFER_DESC, D3D11_SUBRESOURCE_DATA,
            ID3D11Buffer,
            Buffer, BufferOf,
        },
        constant::{
            ConstantBufferP,
            ConstantBufferV,
        },
        sampler::{
            D3D11_FILTER,
            D3D11_SAMPLER_DESC,
            D3D11_TEXTURE_ADDRESS_MODE,
            Filter,
            ID3D11SamplerState,
            SamplerState,
            TextureAddressMode,
        },
        resource::{
            D3D11_RESOURCE_DIMENSION,
            ID3D11Resource,
            Resource,
            ResourceDimension,
        },
        texture2::{
            D3D11_TEXTURE2D_DESC,
            ID3D11Texture2D,
            Texture2,
        },
        vertex::{D3d11ContextBindableVertexBuffer, VertexBuffer},
        view::{
            ID3D11ShaderResourceView, ID3D11View,
            D3D_SRV_DIMENSION,
            D3D11_SHADER_RESOURCE_VIEW_DESC, D3D11_SHADER_RESOURCE_VIEW_DESC_0,
            D3D11_TEX2D_SRV,
            ShaderResourceView, TextureView2, View,
        },
    },
};

use crate::{
    dx11::d3d11,
    impl_d3d,
};

impl AccessFlags {
    pub const READ_WRITE: Self = Self::from_bits_retain(Self::READ.bits() | Self::WRITE.bits());
}

impl BindFlags {
    pub const SHADER_RENDER: Self = Self::from_bits_retain(Self::SHADER.bits() | Self::RENDER.bits());
    pub const SHADER_UNORDERED: Self = Self::from_bits_retain(Self::SHADER.bits() | Self::UNORDERED.bits());
}

impl_d3d! { impl bitflags for
    pub struct BindFlags: D3D11_BIND_FLAG{u32} {
        const VERTEX = d3d11::D3D11_BIND_VERTEX_BUFFER.0;
        const INDEX = d3d11::D3D11_BIND_INDEX_BUFFER.0;
        const DEPTH = d3d11::D3D11_BIND_DEPTH_STENCIL.0;
        const RENDER = d3d11::D3D11_BIND_RENDER_TARGET.0;
        const SHADER = d3d11::D3D11_BIND_SHADER_RESOURCE.0;
        const CONSTANT = d3d11::D3D11_BIND_CONSTANT_BUFFER.0;
        const UNORDERED = d3d11::D3D11_BIND_UNORDERED_ACCESS.0;
    },
    pub struct AccessFlags: D3D11_CPU_ACCESS_FLAG{u32} {
        const WRITE = d3d11::D3D11_CPU_ACCESS_WRITE.0;
        const READ = d3d11::D3D11_CPU_ACCESS_READ.0;
    },
    pub struct BufferFlags: D3D11_RESOURCE_MISC_FLAG{u32} {
        const GENERATE_MIPS = d3d11::D3D11_RESOURCE_MISC_GENERATE_MIPS.0;
        const SHARED = d3d11::D3D11_RESOURCE_MISC_SHARED.0;
        const SHARED_NTHANDLE = d3d11::D3D11_RESOURCE_MISC_SHARED_NTHANDLE.0;
        const SHARED_KEYEDMUTEX = d3d11::D3D11_RESOURCE_MISC_SHARED_KEYEDMUTEX.0;
        const TEXTURECUBE = d3d11::D3D11_RESOURCE_MISC_TEXTURECUBE.0;
        const DRAWINDIRECT_ARGS = d3d11::D3D11_RESOURCE_MISC_DRAWINDIRECT_ARGS.0;
        const BUFFER_ALLOW_RAW_VIEWS = d3d11::D3D11_RESOURCE_MISC_BUFFER_ALLOW_RAW_VIEWS.0;
        const BUFFER_STRUCTURED = d3d11::D3D11_RESOURCE_MISC_BUFFER_STRUCTURED.0;
        const GDI_COMPATIBLE = d3d11::D3D11_RESOURCE_MISC_GDI_COMPATIBLE.0;
        const RESTRICTED_CONTENT = d3d11::D3D11_RESOURCE_MISC_RESTRICTED_CONTENT.0;
        const RESTRICT_SHARED_RESOURCE = d3d11::D3D11_RESOURCE_MISC_RESTRICT_SHARED_RESOURCE.0;
        const RESTRICT_SHARED_RESOURCE_DRIVER = d3d11::D3D11_RESOURCE_MISC_RESTRICT_SHARED_RESOURCE_DRIVER.0;
        const GUARDED = d3d11::D3D11_RESOURCE_MISC_GUARDED.0;
        const TILE_POOL = d3d11::D3D11_RESOURCE_MISC_TILE_POOL.0;
        const TILED = d3d11::D3D11_RESOURCE_MISC_TILED.0;
        const HW_PROTECTED = d3d11::D3D11_RESOURCE_MISC_HW_PROTECTED.0;
        const SHARED_DISPLAYABLE = d3d11::D3D11_RESOURCE_MISC_SHARED_DISPLAYABLE.0;
        const SHARED_EXCLUSIVE_WRITER = d3d11::D3D11_RESOURCE_MISC_SHARED_EXCLUSIVE_WRITER.0;
    },
}
impl_d3d! { impl enum for
    #[derive(Default)]
    pub enum Usage: D3D11_USAGE{u32} {
        #[default]
        const DEFAULT = d3d11::D3D11_USAGE_DEFAULT;
        const IMMUTABLE = d3d11::D3D11_USAGE_IMMUTABLE;
        const DYNAMIC = d3d11::D3D11_USAGE_DYNAMIC;
        const STAGING = d3d11::D3D11_USAGE_STAGING;
    },
}
