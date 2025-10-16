use std::{ffi::CStr, fmt};

#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ShaderTarget {
    #[cfg(todo)]
    #[cfg_attr(feature = "serde", serde(rename = "vs_1_1"))]
    VertexShader_1_1,
    #[cfg(todo)]
    #[cfg_attr(feature = "serde", serde(rename = "ps_1_1"))]
    PixelShader_1_1,

    #[cfg_attr(feature = "serde", serde(rename = "vs_2_0", alias = "vs_2_x"))]
    VertexShader_2_0,
    #[cfg_attr(feature = "serde", serde(rename = "ps_2_0", alias = "ps_2_x"))]
    PixelShader_2_0,

    #[cfg_attr(feature = "serde", serde(rename = "vs_3_0"))]
    VertexShader_3_0,
    #[cfg_attr(feature = "serde", serde(rename = "ps_3_0"))]
    PixelShader_3_0,

    #[cfg_attr(feature = "serde", serde(rename = "vs_4_0"))]
    /// DX10
    VertexShader_4_0,
    #[cfg_attr(feature = "serde", serde(rename = "ps_4_0"))]
    /// DX10
    PixelShader_4_0,

    #[cfg_attr(feature = "serde", serde(rename = "vs_4_1"))]
    /// DX10.1
    VertexShader_4_1,
    #[cfg_attr(feature = "serde", serde(rename = "ps_4_1"))]
    /// DX10.1
    PixelShader_4_1,

    #[cfg_attr(feature = "serde", serde(rename = "vs_5_0", alias = "vertex", alias = "Vertex"))]
    /// DX11
    VertexShader_5_0,
    #[cfg_attr(feature = "serde", serde(rename = "ps_5_0", alias = "pixel", alias = "Pixel"))]
    /// DX11
    PixelShader_5_0,

    #[cfg_attr(feature = "serde", serde(rename = "vs_5_1"))]
    /// DX12
    VertexShader_5_1,
    #[cfg_attr(feature = "serde", serde(rename = "ps_5_1"))]
    /// DX12
    PixelShader_5_1,
}

impl ShaderTarget {
    pub const VERTEX: Self = Self::VertexShader_5_0;
    pub const PIXEL: Self = Self::PixelShader_5_0;

    pub fn as_str(&self) -> &'static str {
        let bytes = self.c_name().to_bytes();
        unsafe {
            str::from_utf8_unchecked(bytes)
        }
    }

    pub fn c_name(&self) -> &'static CStr {
        match self {
            Self::VertexShader_5_1 => c"vs_5_1",
            Self::VertexShader_5_0 => c"vs_5_0",
            Self::VertexShader_4_1 => c"vs_4_1",
            Self::VertexShader_4_0 => c"vs_4_0",
            Self::VertexShader_3_0 => c"vs_3_0",
            Self::VertexShader_2_0 => c"vs_2_0",

            Self::PixelShader_5_1 => c"ps_5_1",
            Self::PixelShader_5_0 => c"ps_5_0",
            Self::PixelShader_4_1 => c"ps_4_1",
            Self::PixelShader_4_0 => c"ps_4_0",
            Self::PixelShader_3_0 => c"ps_3_0",
            Self::PixelShader_2_0 => c"ps_2_0",
        }
    }

    pub fn kind(&self) -> ShaderKind {
        match self {
            ShaderTarget::PixelShader_5_0 | ShaderTarget::PixelShader_5_1
            | ShaderTarget::PixelShader_4_0 | ShaderTarget::PixelShader_4_1
            | ShaderTarget::PixelShader_3_0 | ShaderTarget::PixelShader_2_0
            =>
                ShaderKind::Pixel,
            ShaderTarget::VertexShader_5_0 | ShaderTarget::VertexShader_5_1
            | ShaderTarget::VertexShader_4_0 | ShaderTarget::VertexShader_4_1
            | ShaderTarget::VertexShader_3_0 | ShaderTarget::VertexShader_2_0
            =>
                ShaderKind::Vertex,
        }
    }
}

impl fmt::Display for ShaderTarget {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ShaderKind {
    Vertex,
    Pixel,
}

impl ShaderKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Vertex => "vertex",
            Self::Pixel => "pixel",
        }
    }
}

impl From<ShaderKind> for ShaderTarget {
    fn from(kind: ShaderKind) -> Self {
        match kind {
            ShaderKind::Pixel => Self::PIXEL,
            ShaderKind::Vertex => Self::VERTEX,
        }
    }
}

impl From<ShaderTarget> for ShaderKind {
    #[inline]
    fn from(target: ShaderTarget) -> Self {
        target.kind()
    }
}

impl fmt::Display for ShaderKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} shader", self.as_str())
    }
}
