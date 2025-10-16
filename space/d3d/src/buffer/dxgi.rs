use crate::prelude::*;

pub use crate::dxgi::DXGI_FORMAT;

const DXGI_FORMAT_FORCE_UINT: DXGI_FORMAT = DXGI_FORMAT(-1i32);
impl_d3d! { impl enum for
    #[derive(Default)]
    #[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
    pub enum DxgiFormat: DXGI_FORMAT{u32} {
        #[default]
        Unknown(const UNKNOWN)
            = dxgi::DXGI_FORMAT_UNKNOWN,
        R32G32B32A32Typeless(const R32G32B32A32_TYPELESS)
            = dxgi::DXGI_FORMAT_R32G32B32A32_TYPELESS,
        R32G32B32A32Float(const R32G32B32A32_FLOAT)
            = dxgi::DXGI_FORMAT_R32G32B32A32_FLOAT,
        R32G32B32A32Uint(const R32G32B32A32_UINT)
            = dxgi::DXGI_FORMAT_R32G32B32A32_UINT,
        R32G32B32A32Sint(const R32G32B32A32_SINT)
            = dxgi::DXGI_FORMAT_R32G32B32A32_SINT,
        R32G32B32Typeless(const R32G32B32_TYPELESS)
            = dxgi::DXGI_FORMAT_R32G32B32_TYPELESS,
        R32G32B32Float(const R32G32B32_FLOAT)
            = dxgi::DXGI_FORMAT_R32G32B32_FLOAT,
        R32G32B32Uint(const R32G32B32_UINT)
            = dxgi::DXGI_FORMAT_R32G32B32_UINT,
        R32G32B32Sint(const R32G32B32_SINT)
            = dxgi::DXGI_FORMAT_R32G32B32_SINT,
        R16G16B16A16Typeless(const R16G16B16A16_TYPELESS)
            = dxgi::DXGI_FORMAT_R16G16B16A16_TYPELESS,
        R16G16B16A16Float(const R16G16B16A16_FLOAT)
            = dxgi::DXGI_FORMAT_R16G16B16A16_FLOAT,
        R16G16B16A16UNorm(const R16G16B16A16_UNORM)
            = dxgi::DXGI_FORMAT_R16G16B16A16_UNORM,
        R16G16B16A16Uint(const R16G16B16A16_UINT)
            = dxgi::DXGI_FORMAT_R16G16B16A16_UINT,
        R16G16B16A16Snorm(const R16G16B16A16_SNORM)
            = dxgi::DXGI_FORMAT_R16G16B16A16_SNORM,
        R16G16B16A16Sint(const R16G16B16A16_SINT)
            = dxgi::DXGI_FORMAT_R16G16B16A16_SINT,
        R32G32Typeless(const R32G32_TYPELESS)
            = dxgi::DXGI_FORMAT_R32G32_TYPELESS,
        R32G32Float(const R32G32_FLOAT)
            = dxgi::DXGI_FORMAT_R32G32_FLOAT,
        R32G32Uint(const R32G32_UINT)
            = dxgi::DXGI_FORMAT_R32G32_UINT,
        R32G32Sint(const R32G32_SINT)
            = dxgi::DXGI_FORMAT_R32G32_SINT,
        R32G8X24Typeless(const R32G8X24_TYPELESS)
            = dxgi::DXGI_FORMAT_R32G8X24_TYPELESS,
        D32FloatS8X24Uint(const D32_FLOAT_S8X24_UINT)
            = dxgi::DXGI_FORMAT_D32_FLOAT_S8X24_UINT,
        R32FloatX8X24Typeless(const R32_FLOAT_X8X24_TYPELESS)
            = dxgi::DXGI_FORMAT_R32_FLOAT_X8X24_TYPELESS,
        X32TypelessG8X24Uint(const X32_TYPELESS_G8X24_UINT)
            = dxgi::DXGI_FORMAT_X32_TYPELESS_G8X24_UINT,
        R10G10B10A2Typeless(const R10G10B10A2_TYPELESS)
            = dxgi::DXGI_FORMAT_R10G10B10A2_TYPELESS,
        R10G10B10A2UNorm(const R10G10B10A2_UNORM)
            = dxgi::DXGI_FORMAT_R10G10B10A2_UNORM,
        R10G10B10A2Uint(const R10G10B10A2_UINT)
            = dxgi::DXGI_FORMAT_R10G10B10A2_UINT,
        R11G11B10Float(const R11G11B10_FLOAT)
            = dxgi::DXGI_FORMAT_R11G11B10_FLOAT,
        R8G8B8A8Typeless(const R8G8B8A8_TYPELESS)
            = dxgi::DXGI_FORMAT_R8G8B8A8_TYPELESS,
        R8G8B8A8UNorm(const R8G8B8A8_UNORM)
            = dxgi::DXGI_FORMAT_R8G8B8A8_UNORM,
        R8G8B8A8UNormSrgb(const R8G8B8A8_UNORM_SRGB)
            = dxgi::DXGI_FORMAT_R8G8B8A8_UNORM_SRGB,
        R8G8B8A8Uint(const R8G8B8A8_UINT)
            = dxgi::DXGI_FORMAT_R8G8B8A8_UINT,
        R8G8B8A8Snorm(const R8G8B8A8_SNORM)
            = dxgi::DXGI_FORMAT_R8G8B8A8_SNORM,
        R8G8B8A8Sint(const R8G8B8A8_SINT)
            = dxgi::DXGI_FORMAT_R8G8B8A8_SINT,
        R16G16Typeless(const R16G16_TYPELESS)
            = dxgi::DXGI_FORMAT_R16G16_TYPELESS,
        R16G16Float(const R16G16_FLOAT)
            = dxgi::DXGI_FORMAT_R16G16_FLOAT,
        R16G16UNorm(const R16G16_UNORM)
            = dxgi::DXGI_FORMAT_R16G16_UNORM,
        R16G16Uint(const R16G16_UINT)
            = dxgi::DXGI_FORMAT_R16G16_UINT,
        R16G16Snorm(const R16G16_SNORM)
            = dxgi::DXGI_FORMAT_R16G16_SNORM,
        R16G16Sint(const R16G16_SINT)
            = dxgi::DXGI_FORMAT_R16G16_SINT,
        R32Typeless(const R32_TYPELESS)
            = dxgi::DXGI_FORMAT_R32_TYPELESS,
        D32Float(const D32_FLOAT)
            = dxgi::DXGI_FORMAT_D32_FLOAT,
        R32Float(const R32_FLOAT)
            = dxgi::DXGI_FORMAT_R32_FLOAT,
        R32Uint(const R32_UINT)
            = dxgi::DXGI_FORMAT_R32_UINT,
        R32Sint(const R32_SINT)
            = dxgi::DXGI_FORMAT_R32_SINT,
        R24G8Typeless(const R24G8_TYPELESS)
            = dxgi::DXGI_FORMAT_R24G8_TYPELESS,
        D24UNormS8Uint(const D24_UNORM_S8_UINT)
            = dxgi::DXGI_FORMAT_D24_UNORM_S8_UINT,
        R24UNormX8Typeless(const R24_UNORM_X8_TYPELESS)
            = dxgi::DXGI_FORMAT_R24_UNORM_X8_TYPELESS,
        X24TypelessG8Uint(const X24_TYPELESS_G8_UINT)
            = dxgi::DXGI_FORMAT_X24_TYPELESS_G8_UINT,
        R8G8Typeless(const R8G8_TYPELESS)
            = dxgi::DXGI_FORMAT_R8G8_TYPELESS,
        R8G8UNorm(const R8G8_UNORM)
            = dxgi::DXGI_FORMAT_R8G8_UNORM,
        R8G8Uint(const R8G8_UINT)
            = dxgi::DXGI_FORMAT_R8G8_UINT,
        R8G8Snorm(const R8G8_SNORM)
            = dxgi::DXGI_FORMAT_R8G8_SNORM,
        R8G8Sint(const R8G8_SINT)
            = dxgi::DXGI_FORMAT_R8G8_SINT,
        R16Typeless(const R16_TYPELESS)
            = dxgi::DXGI_FORMAT_R16_TYPELESS,
        R16Float(const R16_FLOAT)
            = dxgi::DXGI_FORMAT_R16_FLOAT,
        D16UNorm(const D16_UNORM)
            = dxgi::DXGI_FORMAT_D16_UNORM,
        R16UNorm(const R16_UNORM)
            = dxgi::DXGI_FORMAT_R16_UNORM,
        R16Uint(const R16_UINT)
            = dxgi::DXGI_FORMAT_R16_UINT,
        R16Snorm(const R16_SNORM)
            = dxgi::DXGI_FORMAT_R16_SNORM,
        R16Sint(const R16_SINT)
            = dxgi::DXGI_FORMAT_R16_SINT,
        R8Typeless(const R8_TYPELESS)
            = dxgi::DXGI_FORMAT_R8_TYPELESS,
        R8UNorm(const R8_UNORM)
            = dxgi::DXGI_FORMAT_R8_UNORM,
        R8Uint(const R8_UINT)
            = dxgi::DXGI_FORMAT_R8_UINT,
        R8Snorm(const R8_SNORM)
            = dxgi::DXGI_FORMAT_R8_SNORM,
        R8Sint(const R8_SINT)
            = dxgi::DXGI_FORMAT_R8_SINT,
        A8UNorm(const A8_UNORM)
            = dxgi::DXGI_FORMAT_A8_UNORM,
        R1UNorm(const R1_UNORM)
            = dxgi::DXGI_FORMAT_R1_UNORM,
        R9G9B9e5Sharedexp(const R9G9B9E5_SHAREDEXP)
            = dxgi::DXGI_FORMAT_R9G9B9E5_SHAREDEXP,
        R8G8B8G8UNorm(const R8G8_B8G8_UNORM)
            = dxgi::DXGI_FORMAT_R8G8_B8G8_UNORM,
        G8R8G8B8UNorm(const G8R8_G8B8_UNORM)
            = dxgi::DXGI_FORMAT_G8R8_G8B8_UNORM,
        Bc1Typeless(const BC1_TYPELESS)
            = dxgi::DXGI_FORMAT_BC1_TYPELESS,
        Bc1UNorm(const BC1_UNORM)
            = dxgi::DXGI_FORMAT_BC1_UNORM,
        Bc1UNormSrgb(const BC1_UNORM_SRGB)
            = dxgi::DXGI_FORMAT_BC1_UNORM_SRGB,
        Bc2Typeless(const BC2_TYPELESS)
            = dxgi::DXGI_FORMAT_BC2_TYPELESS,
        Bc2UNorm(const BC2_UNORM)
            = dxgi::DXGI_FORMAT_BC2_UNORM,
        Bc2UNormSrgb(const BC2_UNORM_SRGB)
            = dxgi::DXGI_FORMAT_BC2_UNORM_SRGB,
        Bc3Typeless(const BC3_TYPELESS)
            = dxgi::DXGI_FORMAT_BC3_TYPELESS,
        Bc3UNorm(const BC3_UNORM)
            = dxgi::DXGI_FORMAT_BC3_UNORM,
        Bc3UNormSrgb(const BC3_UNORM_SRGB)
            = dxgi::DXGI_FORMAT_BC3_UNORM_SRGB,
        Bc4Typeless(const BC4_TYPELESS)
            = dxgi::DXGI_FORMAT_BC4_TYPELESS,
        Bc4UNorm(const BC4_UNORM)
            = dxgi::DXGI_FORMAT_BC4_UNORM,
        Bc4Snorm(const BC4_SNORM)
            = dxgi::DXGI_FORMAT_BC4_SNORM,
        Bc5Typeless(const BC5_TYPELESS)
            = dxgi::DXGI_FORMAT_BC5_TYPELESS,
        Bc5UNorm(const BC5_UNORM)
            = dxgi::DXGI_FORMAT_BC5_UNORM,
        Bc5Snorm(const BC5_SNORM)
            = dxgi::DXGI_FORMAT_BC5_SNORM,
        B5G6R5UNorm(const B5G6R5_UNORM)
            = dxgi::DXGI_FORMAT_B5G6R5_UNORM,
        B5G5R5A1UNorm(const B5G5R5A1_UNORM)
            = dxgi::DXGI_FORMAT_B5G5R5A1_UNORM,
        B8G8R8A8UNorm(const B8G8R8A8_UNORM)
            = dxgi::DXGI_FORMAT_B8G8R8A8_UNORM,
        B8G8R8X8UNorm(const B8G8R8X8_UNORM)
            = dxgi::DXGI_FORMAT_B8G8R8X8_UNORM,
        R10G10B10XrBiasA2UNorm(const R10G10B10_XR_BIAS_A2_UNORM)
            = dxgi::DXGI_FORMAT_R10G10B10_XR_BIAS_A2_UNORM,
        B8G8R8A8Typeless(const B8G8R8A8_TYPELESS)
            = dxgi::DXGI_FORMAT_B8G8R8A8_TYPELESS,
        B8G8R8A8UNormSrgb(const B8G8R8A8_UNORM_SRGB)
            = dxgi::DXGI_FORMAT_B8G8R8A8_UNORM_SRGB,
        B8G8R8X8Typeless(const B8G8R8X8_TYPELESS)
            = dxgi::DXGI_FORMAT_B8G8R8X8_TYPELESS,
        B8G8R8X8UNormSrgb(const B8G8R8X8_UNORM_SRGB)
            = dxgi::DXGI_FORMAT_B8G8R8X8_UNORM_SRGB,
        Bc6hTypeless(const BC6H_TYPELESS)
            = dxgi::DXGI_FORMAT_BC6H_TYPELESS,
        Bc6hUf16(const BC6H_UF16)
            = dxgi::DXGI_FORMAT_BC6H_UF16,
        Bc6hSf16(const BC6H_SF16)
            = dxgi::DXGI_FORMAT_BC6H_SF16,
        Bc7Typeless(const BC7_TYPELESS)
            = dxgi::DXGI_FORMAT_BC7_TYPELESS,
        Bc7UNorm(const BC7_UNORM)
            = dxgi::DXGI_FORMAT_BC7_UNORM,
        Bc7UNormSrgb(const BC7_UNORM_SRGB)
            = dxgi::DXGI_FORMAT_BC7_UNORM_SRGB,
        Ayuv(const AYUV)
            = dxgi::DXGI_FORMAT_AYUV,
        Y410(const Y410)
            = dxgi::DXGI_FORMAT_Y410,
        Y416(const Y416)
            = dxgi::DXGI_FORMAT_Y416,
        Nv12(const NV12)
            = dxgi::DXGI_FORMAT_NV12,
        P010(const P010)
            = dxgi::DXGI_FORMAT_P010,
        P016(const P016)
            = dxgi::DXGI_FORMAT_P016,
        Opaque420(const OPAQUE_420)
            = dxgi::DXGI_FORMAT_420_OPAQUE,
        Yuy2(const YUY2)
            = dxgi::DXGI_FORMAT_YUY2,
        Y210(const Y210)
            = dxgi::DXGI_FORMAT_Y210,
        Y216(const Y216)
            = dxgi::DXGI_FORMAT_Y216,
        Nv11(const NV11)
            = dxgi::DXGI_FORMAT_NV11,
        Ai44(const AI44)
            = dxgi::DXGI_FORMAT_AI44,
        IA44(const IA44)
            = dxgi::DXGI_FORMAT_IA44,
        P8(const P8)
            = dxgi::DXGI_FORMAT_P8,
        A8P8(const A8P8)
            = dxgi::DXGI_FORMAT_A8P8,
        B4G4R4A4UNorm(const B4G4R4A4_UNORM)
            = dxgi::DXGI_FORMAT_B4G4R4A4_UNORM,
        P208(const P208)
            = dxgi::DXGI_FORMAT_P208,
        V208(const V208)
            = dxgi::DXGI_FORMAT_V208,
        V408(const V408)
            = dxgi::DXGI_FORMAT_V408,
        SamplerFeedbackMinMipOpaque(const SAMPLER_FEEDBACK_MIN_MIP_OPAQUE)
            = dxgi::DXGI_FORMAT_SAMPLER_FEEDBACK_MIN_MIP_OPAQUE,
        SamplerFeedbackMipRegionUsedOpaque(const SAMPLER_FEEDBACK_MIP_REGION_USED_OPAQUE)
            = dxgi::DXGI_FORMAT_SAMPLER_FEEDBACK_MIP_REGION_USED_OPAQUE,
        ForceUint(const FORCE_UINT)
            = DXGI_FORMAT_FORCE_UINT,
    },
}

#[doc(hidden)]
#[cfg(feature = "serde")]
pub mod serde_imp {
    pub mod format {
        use {
            serde::{
                Deserialize, Deserializer,
                Serialize, Serializer,
            },
            super::super::{DxgiFormat, DXGI_FORMAT},
        };

        pub fn serialize<S: Serializer>(class: &DXGI_FORMAT, serializer: S) -> Result<S::Ok, S::Error> {
            match DxgiFormat::try_from_d3d(*class) {
                Ok(class) => class.serialize(serializer),
                Err(..) => class.0.serialize(serializer),
            }
        }
        pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<DXGI_FORMAT, D::Error> {
            #[derive(Deserialize)]
            #[serde(untagged)]
            enum DxgiFormat {
                Enum(self::DxgiFormat),
                Raw(i32),
            }
            DxgiFormat::deserialize(deserializer).map(|c| match c {
                DxgiFormat::Enum(class) => class.to_d3d(),
                DxgiFormat::Raw(class) => DXGI_FORMAT(class),
            })
        }
    }
}
