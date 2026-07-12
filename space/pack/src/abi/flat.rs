use {
    crate::{abi::MarkerInstanceData, legacy::Vertex, DrawSpace},
    core::mem::offset_of,
    glamour::{FloatScalar, Matrix4, Unit, Vec3Swizzles, Vector2, Vector3, Vector4},
    num_traits::AsPrimitive,
    taimi_d3d::dx11::{
        buffer::BufferOf,
        prelude::*,
        shader::{InputLayout, D3D11_INPUT_ELEMENT_DESC},
    },
};

pub type MapEntityInstanceBuffer = BufferOf<MapEntityInstanceData>;
pub type MapEntityInstanceData = Map2dInstanceData;
pub type Map2dVertexBuffer = BufferOf<Map2dVertex>;

#[derive(Debug, Copy, Clone, Default)]
#[repr(C, align(16))]
pub struct Map2dConstantDataV {
    pub render: Map2dRenderConstantDataV,
    pub trail: Map2dTrailConstantDataV,
    pub poi: Map2dPoiConstantDataV,
}
impl Map2dConstantDataV {
    pub const IDENTITY: Self = Self {
        render: Map2dRenderConstantDataV::IDENTITY,
        trail: Map2dTrailConstantDataV::IDENTITY,
        poi: Map2dPoiConstantDataV::IDENTITY,
    };
}
unsafe impl D3dBufferData for Map2dConstantDataV {}

#[derive(Debug, Copy, Clone, Default)]
#[repr(C, align(16))]
pub struct Map2dRenderConstantDataV {
    pub projection: Matrix4<f32>,
    pub _padding0: Vector2<f32>,
    /// TODO: Vec2?
    pub map_scale: f32,
    pub anim_timestamp: f32,
    // split into 2 structs in hlsl atm but irrelevant...
    pub _padding1: Vector2<f32>,
    pub anim_scale: f32,
    pub flags: u32,
    /// imgui units to normalized coords
    ///
    /// TODO: Affine2 / Mat34 / Mat23
    pub viewport_ortho: Matrix4<f32>,
}
impl Map2dRenderConstantDataV {
    pub const IDENTITY: Self = Self {
        projection: Matrix4::IDENTITY,
        _padding0: Vector2::ZERO,
        map_scale: 1.0f32,
        anim_timestamp: 0.0f32,
        _padding1: Vector2::ZERO,
        anim_scale: 0.0f32,
        flags: 0u32,
        viewport_ortho: Matrix4::IDENTITY,
    };
}
unsafe impl D3dBufferData for Map2dRenderConstantDataV {}

#[derive(Debug, Copy, Clone, Default)]
#[repr(C, align(16))]
pub struct Map2dTrailConstantDataV {
    pub tex_scale: f32,
    pub tex_offset: f32,
    pub scale_expand: f32,
    pub _padding0: f32,
}
impl Map2dTrailConstantDataV {
    pub const IDENTITY: Self = Self {
        tex_scale: 1.0f32,
        tex_offset: 0.0f32,
        scale_expand: 0.0f32,
        _padding0: 0.0f32,
    };
}
unsafe impl D3dBufferData for Map2dTrailConstantDataV {}

#[derive(Debug, Copy, Clone, Default)]
#[repr(C, align(16))]
pub struct Map2dPoiConstantDataV {
    pub _padding0: Vector3<f32>,
    pub scale: f32,
}
impl Map2dPoiConstantDataV {
    pub const IDENTITY: Self = Self {
        _padding0: Vector3::ZERO,
        scale: 1.0f32,
    };
}
unsafe impl D3dBufferData for Map2dPoiConstantDataV {}

#[derive(Debug, Copy, Clone, Default)]
#[repr(C, align(16))]
pub struct Map2dRenderConstantDataP {
    pub tint: Vector4<f32>,
}
impl Map2dRenderConstantDataP {
    pub const IDENTITY: Self = Self { tint: Vector4::ONE };
}
unsafe impl D3dBufferData for Map2dRenderConstantDataP {}

#[derive(Debug, Copy, Clone, Default)]
#[repr(C, align(16))]
pub struct Map2dMarkerConstantDataP {
    pub _padding0: Vector3<f32>,
    pub alpha: f32,
}
impl Map2dMarkerConstantDataP {
    pub const IDENTITY: Self = Self {
        _padding0: Vector3::ZERO,
        alpha: 1.0f32,
    };
}
unsafe impl D3dBufferData for Map2dMarkerConstantDataP {}

pub type Map2dTrailConstantDataP = Map2dMarkerConstantDataP;
pub type Map2dPoiConstantDataP = Map2dMarkerConstantDataP;

#[derive(Debug, Copy, Clone, Default)]
#[repr(C, align(16))]
pub struct Map2dConstantDataP {
    pub render: Map2dRenderConstantDataP,
    pub trail: Map2dTrailConstantDataP,
    pub poi: Map2dPoiConstantDataP,
}
impl Map2dConstantDataP {
    pub const IDENTITY: Self = Self {
        render: Map2dRenderConstantDataP::IDENTITY,
        trail: Map2dTrailConstantDataP::IDENTITY,
        poi: Map2dPoiConstantDataP::IDENTITY,
    };
}
unsafe impl D3dBufferData for Map2dConstantDataP {}

#[derive(Copy, Clone)]
#[repr(C, align(16))]
pub struct Map2dInstanceData {
    /// TODO: consider packing? shrug...
    pub model: Matrix4<f32>,
    pub colour: Vector4<f32>,
    pub anim_scale: f32,
    pub mid_height: f32,
    pub _padding0: f32,
    pub flags: u32,
    pub _padding1: Vector4<f32>,
    pub _padding2: Vector4<f32>,
}
impl Map2dInstanceData {
    pub const IDENTITY: Self = Self {
        model: Matrix4::IDENTITY,
        colour: Vector4::ONE,
        anim_scale: 0.0f32,
        mid_height: 0.0f32,
        flags: 0u32,
        _padding0: 0.0f32,
        _padding1: Vector4::ZERO,
        _padding2: Vector4::ZERO,
    };

    pub const FLAG_STATIC_SCALE: u32 = {
        let f = MarkerInstanceData::FLAG_MAP_STATIC_SCALE >> 8;
        match f & 0xff {
            0 => panic!(),
            _ => f,
        }
    };
    pub const FLAG_IS_TRAIL: u32 = {
        let f = MarkerInstanceData::FLAG_IS_TRAIL >> 8;
        match f & 0xff {
            0 => panic!(),
            _ => f,
        }
    };

    pub fn alloc_populated(
        device: &Dx11Device,
        entities: &[Self],
    ) -> anyhow::Result<MapEntityInstanceBuffer> {
        BufferOf::new_with_data(device, Ok(entities), ())
    }

    pub const INPUT_LAYOUT: [D3D11_INPUT_ELEMENT_DESC; 8] = [
        InputLayout::for_instance(
            1,
            c"MODEL",
            0,
            dxgi::DXGI_FORMAT_R32G32B32A32_FLOAT,
            Some(offset_of!(Self, model)),
        ),
        InputLayout::for_instance(1, c"MODEL", 1, dxgi::DXGI_FORMAT_R32G32B32A32_FLOAT, None),
        InputLayout::for_instance(1, c"MODEL", 2, dxgi::DXGI_FORMAT_R32G32B32A32_FLOAT, None),
        InputLayout::for_instance(1, c"MODEL", 3, dxgi::DXGI_FORMAT_R32G32B32A32_FLOAT, None),
        InputLayout::for_instance(
            1,
            c"MCOLOUR",
            0,
            dxgi::DXGI_FORMAT_R32G32B32A32_FLOAT,
            Some(offset_of!(Self, colour)),
        ),
        InputLayout::for_instance(
            1,
            c"MANIM",
            0,
            dxgi::DXGI_FORMAT_R32_FLOAT,
            Some(offset_of!(Self, anim_scale)),
        ),
        InputLayout::for_instance(
            1,
            c"MANIM",
            1,
            dxgi::DXGI_FORMAT_R32_FLOAT,
            Some(offset_of!(Self, mid_height)),
        ),
        InputLayout::for_instance(
            1,
            c"MFLAG",
            0,
            dxgi::DXGI_FORMAT_R32_UINT,
            Some(offset_of!(Self, flags)),
        ),
        #[cfg(todo)]
        InputLayout::for_instance(
            1,
            c"MFLAG",
            1,
            dxgi::DXGI_FORMAT_R32_UINT,
            Some(offset_of!(Self, fade_range)),
        ),
    ];
}
unsafe impl D3dBufferData for Map2dInstanceData {}
pub const INPUT_LAYOUT_MAP2D_INSTANCE: [D3D11_INPUT_ELEMENT_DESC; 12] = [
    Map2dVertex::INPUT_LAYOUT[0],       // POSITION0
    Map2dVertex::INPUT_LAYOUT[1],       // TEXCOORD0
    Map2dVertex::INPUT_LAYOUT[2],       // TEXCOORD1
    Map2dVertex::INPUT_LAYOUT[3],       // NORMAL0
    Map2dInstanceData::INPUT_LAYOUT[0], // MODEL0
    Map2dInstanceData::INPUT_LAYOUT[1], // MODEL1
    Map2dInstanceData::INPUT_LAYOUT[2], // MODEL2
    Map2dInstanceData::INPUT_LAYOUT[3], // MODEL3
    Map2dInstanceData::INPUT_LAYOUT[4], // MCOLOUR0
    Map2dInstanceData::INPUT_LAYOUT[5], // MANIM0 (anim_scale)
    Map2dInstanceData::INPUT_LAYOUT[6], // MANIM1 (mid_height)
    Map2dInstanceData::INPUT_LAYOUT[7], // MFLAG1 (mid_height)
];
#[derive(Debug, Copy, Clone, Default)]
#[repr(C, align(16))]
pub struct Map2dVertex {
    pub position: Vector2<DrawSpace>,
    /// TODO: precision or splitting isn't that necessary but idk
    pub texture_v: f32,
    /// TODO: may be messy if wrapping is desired?
    /// idk maybe SINT + scalar or just start encoding f16...
    pub texture_u: u8,
    pub _padding0: u8,
    pub normal: u16,
}
impl Map2dVertex {
    pub const INVALID: Self = Self {
        position: Vector2::ZERO,
        normal: 0u16,
        _padding0: 0,
        texture_v: 0.0f32,
        texture_u: 0,
    };
    #[inline]
    pub const fn new(position: Vector2<DrawSpace>, texture: Vector2<f32>) -> Self {
        Self {
            position,
            texture_u: (texture.x * 255.0f32) as u8,
            texture_v: texture.y,
            ..Self::INVALID
        }
    }
    #[cfg(todo)]
    pub fn texture(&self) -> Vector2<f32> {}
    #[cfg(todo)]
    pub fn normal(&self) -> Vector2<f32> {}
    pub fn set_texture(&mut self, uv: Vector2<f32>) {
        #[cfg(todo)]
        {
            self.texture = encode_snorm2(uv);
        }
        self.texture_u = (uv.x * 255.0f32) as u8;
        self.texture_v = uv.y;
    }
    pub fn set_normal(&mut self, normal: Vector2<f32>) {
        self.normal = encode_snorm2(normal);
    }

    pub fn alloc(device: &Dx11Device, vertices: &[Self]) -> anyhow::Result<Map2dVertexBuffer> {
        BufferOf::new_with_data(device, Ok(vertices), ())
    }
    pub const POI_QUAD: [Self; 4] = [
        Self::new(Vector2::new(-1.0, -1.0), Vector2::ONE),
        Self::new(Vector2::new(1.0, -1.0), Vector2::Y),
        Self::new(Vector2::new(-1.0, 1.0), Vector2::X),
        Self::new(Vector2::new(1.0, 1.0), Vector2::ZERO),
    ];

    pub const INPUT_LAYOUT: [D3D11_INPUT_ELEMENT_DESC; 4] = [
        InputLayout::for_vertex(
            0,
            c"POSITION",
            0,
            dxgi::DXGI_FORMAT_R32G32_FLOAT,
            Some(offset_of!(Self, position)),
        ),
        #[cfg(todo)]
        InputLayout::for_vertex(
            0,
            c"PADDING",
            0,
            dxgi::DXGI_FORMAT_R32_FLOAT,
            Some(offset_of!(Self, _padding0)),
        ),
        InputLayout::for_vertex(
            0,
            c"TEXCOORD",
            0,
            dxgi::DXGI_FORMAT_R8_UNORM,
            Some(offset_of!(Self, texture_u)),
        ),
        InputLayout::for_vertex(
            0,
            c"TEXCOORD",
            1,
            dxgi::DXGI_FORMAT_R32_FLOAT,
            Some(offset_of!(Self, texture_v)),
        ),
        InputLayout::for_vertex(
            0,
            c"NORMAL",
            0,
            dxgi::DXGI_FORMAT_R8G8_SNORM,
            Some(offset_of!(Self, normal)),
        ),
    ];
}
impl From<Vertex> for Map2dVertex {
    fn from(v: Vertex) -> Self {
        let mut out = Self::new(v.position.xz().into(), v.texture.into());
        out.set_normal(v.normal.xz().into());
        out
    }
}
unsafe impl D3dBufferData for Map2dVertex {}

pub fn encode_snorm2<U>(v2: Vector2<U>) -> u16
where
    U: Unit,
    U::Scalar: FloatScalar + AsPrimitive<f32>,
{
    let Vector2 { x, y } = v2.as_::<f32>();
    // thank as for clamping these days...
    let x = (x * 128.0f32) as i8;
    let y = (y * 128.0f32) as i8;
    u16::from_ne_bytes([x as u8, y as u8])
}
pub fn encode_unorm4<U>(v4: Vector4<U>) -> u32
where
    U: Unit,
    U::Scalar: FloatScalar + AsPrimitive<f32>,
{
    let v4 = v4.as_::<f32>().to_raw() * 255.0f32;
    u32::from_ne_bytes([v4.x as u8, v4.y as u8, v4.z as u8, v4.w as u8])
}

/// layout compatible with default imgui vertex
#[derive(Debug, Copy, Clone, PartialEq)]
#[repr(C)]
pub struct ImDrawVert {
    pub pos: Vector2<f32>,
    pub uv: Vector2<f32>,
    pub col: u32,
}

#[derive(Copy, Clone)]
#[repr(C, align(16))]
pub struct ImMap2dInstanceData {
    /// could use this but also cbuffer is there anyway so...
    pub model32: [u32; 4],
    #[cfg(todo = "unnecessary")]
    pub zero16: u16,
    #[cfg(todo = "unnecessary")]
    pub _padding0: [u16; 7],
}
impl ImMap2dInstanceData {
    pub const IDENTITY: Self = Self {
        model32: [
            u32::from_be(0xff000000u32),
            u32::from_be(0x00ff0000u32),
            u32::from_be(0x0000ff00u32),
            u32::from_be(0x000000ffu32),
        ],
        #[cfg(todo)]
        zero16: 0u16,
        #[cfg(todo)]
        _padding0: [0u16; 7],
    };
    /// if model32 is always the identity matrix, it already has some zeros in it...
    pub const ZERO16_OFFSET: usize = match () {
        #[cfg(todo = "unnecessary")]
        _ => offset_of!(Self, zero16),
        #[cfg(target_endian = "big")]
        _ => offset_of!(Self, model32),
        //#[cfg(target_endian = "little")]
        _ => offset_of!(Self, model32) + size_of::<u32>() * 2,
    };
    pub const ZERO8_OFFSET: usize = match () {
        #[cfg(todo = "unnecessary")]
        _ => Self::ZERO16_OFFSET,
        #[cfg(todo = "unnecessary")]
        _ => offset_of!(Self, zero8),
        _ => offset_of!(Self, model32) + size_of::<u32>(),
    };
    /// step=0 will never step (indicates one entry for all instances)
    pub const INSTANCE_STEP: u32 = match () {
        #[cfg(todo = "unnecessary")]
        _ => u32::MAX,
        #[cfg(todo = "unnecessary")]
        _ => 1u32,
        _ => 0u32,
    };
    #[deprecated = "INPUT_LAYOUT_MARKER_COMPAT"]
    pub const INPUT_LAYOUT: [D3D11_INPUT_ELEMENT_DESC; 11] = Self::INPUT_LAYOUT_MARKER_COMPAT;
    pub const INPUT_LAYOUT_MARKER_COMPAT: [D3D11_INPUT_ELEMENT_DESC; 11] = [
        InputLayout::for_vertex(
            0,
            c"POSITION",
            0,
            dxgi::DXGI_FORMAT_R32G32_FLOAT,
            Some(offset_of!(ImDrawVert, pos)),
        ),
        InputLayout::for_vertex(
            0,
            c"TEXCOORD",
            0,
            dxgi::DXGI_FORMAT_R32G32_FLOAT,
            Some(offset_of!(ImDrawVert, uv)),
        ),
        InputLayout::for_vertex(
            0,
            c"MCOLOUR",
            0,
            dxgi::DXGI_FORMAT_R8G8B8A8_UNORM,
            Some(offset_of!(ImDrawVert, col)),
        ),
        InputLayout::for_instance_step(
            1,
            c"NORMAL",
            0,
            dxgi::DXGI_FORMAT_R8G8_UNORM,
            Some(ImMap2dInstanceData::ZERO16_OFFSET),
            ImMap2dInstanceData::INSTANCE_STEP,
        ),
        InputLayout::for_instance_step(
            1,
            c"MANIM",
            0,
            dxgi::DXGI_FORMAT_R8_UNORM,
            Some(ImMap2dInstanceData::ZERO16_OFFSET),
            ImMap2dInstanceData::INSTANCE_STEP,
        ),
        InputLayout::for_instance_step(
            1,
            c"MANIM",
            1,
            dxgi::DXGI_FORMAT_R8_UNORM,
            #[cfg(todo = "unnecessary")]
            Some(ImMap2dInstanceData::ZERO16_OFFSET + size_of::<u8>()),
            Some(ImMap2dInstanceData::ZERO16_OFFSET),
            ImMap2dInstanceData::INSTANCE_STEP,
        ),
        InputLayout::for_instance_step(
            1,
            c"MFLAG",
            0,
            dxgi::DXGI_FORMAT_R8_UINT,
            Some(ImMap2dInstanceData::ZERO8_OFFSET),
            ImMap2dInstanceData::INSTANCE_STEP,
        ),
        InputLayout::for_instance_step(
            1,
            c"MODEL",
            0,
            dxgi::DXGI_FORMAT_R8G8B8A8_UNORM,
            Some(offset_of!(ImMap2dInstanceData, model32)),
            ImMap2dInstanceData::INSTANCE_STEP,
        ),
        InputLayout::for_instance_step(
            1,
            c"MODEL",
            1,
            dxgi::DXGI_FORMAT_R8G8B8A8_UNORM,
            Some(offset_of!(ImMap2dInstanceData, model32) + size_of::<u32>()),
            ImMap2dInstanceData::INSTANCE_STEP,
        ),
        InputLayout::for_instance_step(
            1,
            c"MODEL",
            2,
            dxgi::DXGI_FORMAT_R8G8B8A8_UNORM,
            Some(offset_of!(ImMap2dInstanceData, model32) + size_of::<u32>() * 2),
            ImMap2dInstanceData::INSTANCE_STEP,
        ),
        InputLayout::for_instance_step(
            1,
            c"MODEL",
            3,
            dxgi::DXGI_FORMAT_R8G8B8A8_UNORM,
            Some(offset_of!(ImMap2dInstanceData, model32) + size_of::<u32>() * 3),
            ImMap2dInstanceData::INSTANCE_STEP,
        ),
        #[cfg(todo = "unnecessary")]
        InputLayout::for_instance_step(
            1,
            c"POSITION",
            1,
            dxgi::DXGI_FORMAT_R8G8_UNORM,
            Some(ImMap2dInstanceData::ZERO16_OFFSET),
            ImMap2dInstanceData::INSTANCE_STEP,
        ),
    ];
    /// 3 floats in is expected to always be 0 when the ortho projection is a mat4
    /// without rotation/shear/etc
    /// (row or column major; `x_axis.z == z_axis.x == 0.0f32`)
    ///
    /// `x_axis.y` and `y_axis.z` are also options but shrug
    pub const ORTHO_ZERO32_OFFSET: usize = offset_of!(ImOrthoData, x_axis) + size_of::<f32>() * 2;
    pub const INPUT_LAYOUT_ORTHO_IB: [D3D11_INPUT_ELEMENT_DESC; 11] = [
        Self::INPUT_LAYOUT_MARKER_COMPAT[0],
        Self::INPUT_LAYOUT_MARKER_COMPAT[1],
        Self::INPUT_LAYOUT_MARKER_COMPAT[2],
        InputLayout::for_instance_step(
            1,
            c"NORMAL",
            0,
            dxgi::DXGI_FORMAT_R8G8_UNORM,
            Some(Self::ORTHO_ZERO32_OFFSET),
            Self::INSTANCE_STEP,
        ),
        InputLayout::for_instance_step(
            1,
            c"MANIM",
            0,
            dxgi::DXGI_FORMAT_R8_UNORM,
            Some(Self::ORTHO_ZERO32_OFFSET),
            Self::INSTANCE_STEP,
        ),
        InputLayout::for_instance_step(
            1,
            c"MANIM",
            1,
            dxgi::DXGI_FORMAT_R8_UNORM,
            #[cfg(todo = "unnecessary")]
            Some(Self::ORTHO_ZERO32_OFFSET + size_of::<u8>()),
            Some(Self::ORTHO_ZERO32_OFFSET),
            Self::INSTANCE_STEP,
        ),
        InputLayout::for_instance_step(
            1,
            c"MFLAG",
            0,
            dxgi::DXGI_FORMAT_R8_UINT,
            Some(Self::ORTHO_ZERO32_OFFSET),
            Self::INSTANCE_STEP,
        ),
        InputLayout::for_instance_step(
            1,
            c"MODEL",
            0,
            dxgi::DXGI_FORMAT_R32G32B32A32_FLOAT,
            Some(offset_of!(ImOrthoData, x_axis)),
            Self::INSTANCE_STEP,
        ),
        InputLayout::for_instance_step(
            1,
            c"MODEL",
            1,
            dxgi::DXGI_FORMAT_R32G32B32A32_FLOAT,
            Some(offset_of!(ImOrthoData, y_axis)),
            ImMap2dInstanceData::INSTANCE_STEP,
        ),
        InputLayout::for_instance_step(
            1,
            c"MODEL",
            2,
            dxgi::DXGI_FORMAT_R32G32B32A32_FLOAT,
            Some(offset_of!(ImOrthoData, z_axis)),
            ImMap2dInstanceData::INSTANCE_STEP,
        ),
        InputLayout::for_instance_step(
            1,
            c"MODEL",
            3,
            dxgi::DXGI_FORMAT_R32G32B32A32_FLOAT,
            Some(offset_of!(ImOrthoData, w_axis)),
            ImMap2dInstanceData::INSTANCE_STEP,
        ),
    ];
}
unsafe impl D3dBufferData for ImMap2dInstanceData {}
/// TODO: affine2d etc
pub type ImOrthoData = glam::Mat4;
