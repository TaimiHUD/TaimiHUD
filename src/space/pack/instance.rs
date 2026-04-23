use taimi_d3d::{
    buffer::math::Mat43,
    dx11::{
        buffer::{BufferOf, IndexBuffer, VertexBuffer},
        shader::{D3D11_INPUT_ELEMENT_DESC, InputLayout},
        prelude::*,
    },
};
use taimi_pack::attributes::BounceBehavior;
use crate::resources::Vertex;
use crate::space::DrawSpace;
use taimi_meta::spatial::IRRELEVANT_MIN;
use taimi_meta::spatial::IRRELEVANT_MAX;
use glamour::{Matrix4, Vector2, Vector3, Vector4, Vec3Swizzles};
use std::mem::{self, offset_of};
use std::fmt;

pub type EntityInstanceBuffer = BufferOf<EntityInstanceData>;
pub type PoiVertexBuffer = BufferOf<PoiVertex>;
pub type TrailVertexBuffer = BufferOf<TrailVertex>;

#[derive(Copy, Clone)]
#[repr(C, align(16))]
pub union EntityInstanceData {
    trail: TrailInstanceData,
    poi: PoiInstanceData,
}
impl EntityInstanceData {
    pub const INVALID: Self = Self {
        poi: PoiInstanceData::INVALID,
    };

    pub fn new_trail(trail: TrailInstanceData) -> Self {
        Self { trail }
    }
    pub fn new_poi(poi: PoiInstanceData) -> Self {
        Self { poi }
    }
    pub fn write_trail(&mut self, mut trail: TrailInstanceData) -> &mut TrailInstanceData {
        trail.marker.flags |= MarkerInstanceData::FLAG_IS_TRAIL;
        *self = Self::new_trail(trail);
        unsafe {
            self.trail_mut_unchecked()
        }
    }
    pub fn write_poi(&mut self, poi: PoiInstanceData) -> &mut PoiInstanceData {
        *self = Self::new_poi(poi);
        unsafe {
            self.poi_mut_unchecked()
        }
    }
    pub fn marker_data(&self) -> &MarkerInstanceData {
        unsafe {
            mem::transmute(self)
        }
    }
    pub fn marker_data_mut(&mut self) -> &mut MarkerInstanceData {
        unsafe {
            mem::transmute(self)
        }
    }
    pub unsafe fn poi_mut_unchecked(&mut self) -> &mut PoiInstanceData {
        mem::transmute(self)
    }
    pub unsafe fn trail_mut_unchecked(&mut self) -> &mut TrailInstanceData {
        mem::transmute(self)
    }
    pub fn is_trail(&self) -> bool {
        self.marker_data().flags & MarkerInstanceData::FLAG_IS_TRAIL != 0
    }

    pub fn alloc_populated(device: &Dx11Device, entities: &[Self]) -> anyhow::Result<EntityInstanceBuffer> {
        BufferOf::new_with_data(device, Ok(entities), ())
    }
}
impl fmt::Debug for EntityInstanceData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut f = f.debug_tuple("EntityInstanceData");
        match self.is_trail() {
            true => unsafe {
                f.field(&self.trail)
            },
            false => unsafe {
                f.field(&self.poi)
            },
        }.finish()
    }
}
impl Default for EntityInstanceData {
    #[inline]
    fn default() -> Self { Self::INVALID }
}

unsafe impl D3dBufferData for EntityInstanceData {}

#[derive(Debug, Copy, Clone)]
#[repr(C, align(16))]
pub struct TrailInstanceData {
    pub marker: MarkerInstanceData,
    /// applied to pre-generated vertex data instead
    #[cfg(todo)]
    pub scale: f32,
    pub _padding0: Vector2<f32>,
}
impl TrailInstanceData {
    pub const INVALID: Self = Self {
        marker: MarkerInstanceData::INVALID,
        _padding0: Vector2::ZERO,
    };
}
impl Default for TrailInstanceData {
    #[inline]
    fn default() -> Self { Self::INVALID }
}
#[derive(Debug, Copy, Clone)]
#[repr(C, align(16))]
pub struct PoiInstanceData {
    pub marker: MarkerInstanceData,
    pub size_range: u32,
    pub bounce: u32,
    /// TODO: Mat43
    pub model: Matrix4<f32>,
    /// bounce start timestamp
    pub anim_offset: f32,
    /// TODO: Vec2?
    pub map_scale: f32,
    pub billboard_scale: f32,
    pub _padding0: f32,
}
impl PoiInstanceData {
    const SIZE: usize = MarkerInstanceData::SIZE + mem::size_of::<f32>() * (2 + 4 * 4 + 2 + 2);
    pub const INVALID: Self = {
        let invalid = Self {
            marker: MarkerInstanceData::INVALID,
            model: Matrix4::IDENTITY,
            size_range: 0,
            bounce: 0,
            anim_offset: 0.0,
            map_scale: 0.0,
            billboard_scale: 0.0,
            _padding0: 0.0,
        };
        match mem::size_of::<Self>() {
            PoiInstanceData::SIZE => invalid,
            _ => panic!("alignments bleh"),
        }
    };
    /// unit/s
    #[cfg(todo = "unused")]
    pub const BOUNCE_DURATION_RESOLUTION: u32 = 16;
    /// unit/m
    pub const BOUNCE_HEIGHT_RESOLUTION: u32 = 16;
    pub const BOUNCE_HEIGHT_OFFSET: f32 = 0x4000 as f32;
    pub fn set_size_range(&mut self, min: f32, max: f32) {
        self.size_range = pack_int_range(min, max);
    }
    pub fn clear_bounce(&mut self) {
        self.bounce = 1 << 16;
    }
    const ANIM_CYCLE_BOUNCE: f32 = core::f32::consts::PI * 2.0;
    const ANIM_CYCLE_RISE: f32 = 1.0;
    /// TODO: is duration for full cycle or to hit height?
    pub fn set_bounce_params(&mut self, height: f32, duration: f32) {
        let (h, d) = match duration {
            0.0 => (0.0, 0.0),
            _ => {
                let period = match self.marker.flags & MarkerInstanceData::FLAG_RISE {
                    0 => Self::ANIM_CYCLE_BOUNCE,
                    _ => Self::ANIM_CYCLE_RISE,
                };
                (
                    height * Self::BOUNCE_HEIGHT_RESOLUTION as f32,
                    period / duration,
                )
            },
        };
        self.marker.set_anim_scale(self.marker.anim_scale * d);
        #[cfg(todo)]
        let arg1 = (duration * Self::BOUNCE_DURATION_RESOLUTION as f32).min(1.0);
        let _arg1 = 0.0;
        self.bounce = pack_int_pair(h + Self::BOUNCE_HEIGHT_OFFSET, _arg1);
    }
    /// TODO: incorporate delay
    pub fn set_bounce(&mut self, height: f32, duration: f32, behaviour: BounceBehavior, wind_down: bool, delay: f32, start: Option<f32>) {
        let mut scale = match start {
            Some(..) => 1.0,
            None => 0.0,
        };
        match behaviour {
            BounceBehavior::Rise => {
                if wind_down {
                    scale = -2.0;
                }
                self.marker.flags |= MarkerInstanceData::FLAG_RISE
            },
            BounceBehavior::Bounce =>
                self.marker.flags &= !MarkerInstanceData::FLAG_RISE,
        }
        self.marker.set_anim_scale(scale);
        self.set_bounce_params(height, duration);
        self.anim_offset = start.unwrap_or(0.0);
    }
}
impl Default for PoiInstanceData {
    #[inline]
    fn default() -> Self { Self::INVALID }
}
#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct MarkerInstanceData {
    pub colour: Vector3,
    /// `animSpeed`
    ///
    /// TODO: u16? f16?
    pub anim_scale: f32,
    pub flags: u32,
    pub fade_range: u32,
}
impl MarkerInstanceData {
    const SIZE: usize = mem::size_of::<f32>() * (3 + 1 + 2);
    pub const INVALID: Self = {
        let empty = Self {
            colour: Vector3::ZERO,
            // since MIN>MAX when not negative, this probably does the opposite of fading!
            fade_range: (IRRELEVANT_MAX.abs() as u32) << 16 | IRRELEVANT_MIN.abs() as u32,
            flags: 0,
            anim_scale: 0.0,
        };
        match mem::size_of::<Self>() {
            MarkerInstanceData::SIZE => empty,
            _ => panic!("alignments bleh"),
        }
    };
    #[cfg(todo = "unused")]
    pub const IDENTITY: Self = Self {
        colour: Vector4::ONE,
    };

    pub const FLAG_ALPHA_MASK: u32 = 0x00ff;
    /// whether it `canFade` near player etc
    pub const FLAG_OBSCURE_FADE: u32 = 0x0100;
    /// poi is a billboard
    pub const FLAG_BILLBOARD: u32 = 0x0200;
    /// poi unset `scaleOnMapWithZoom`
    pub const FLAG_MAP_STATIC_SCALE: u32 = 0x0400;
    /// poi [BounceBehavior::Rise]
    pub const FLAG_RISE: u32 = 0x0800;
    /// trail `isWall`
    pub const FLAG_WALL: u32 = 0x1000;
    /// avoid alpha blending if `occlude` is set
    pub const FLAG_OPAQUE: u32 = 0x2000;
    pub const FLAG_RESERVED_14: u32 = 0x4000;
    /// marker for safety
    pub const FLAG_IS_TRAIL: u32 = 0x8000;
    pub const FLAG_FACE_CULL: u32 = 0x0002_0000;
    /// cull front faces
    pub const FLAG_FACE_CULL_FRONT: u32 = 0x0001_0000;
    pub const FLAG_RESERVED_19: u32 = 0x0004_0000;
    pub const FLAG_RESERVED_20: u32 = 0x0008_0000;

    pub const FADE_RESOLUTION_NEAR: f32 = 8.0;
    pub const FADE_RESOLUTION_FAR: f32 = 4.0;
    pub fn set_fade_range(&mut self, near_start: f32, far_end: f32) {
        let start = match near_start {
            near_start if near_start < 0.0 =>
                IRRELEVANT_MAX.abs(),
            near_start => near_start * Self::FADE_RESOLUTION_NEAR,
        };
        // TODO: pick end based on distance/intensity settings if <=start as a semi-infinite mode?
        self.fade_range = match far_end {
            end => {
                // store range relative to start rather than absolute end
                let range = ((end - near_start) * Self::FADE_RESOLUTION_FAR).max(1.0);
                pack_int_pair(start, range)
            },
            #[cfg(todo)]
            end => {
                let end = (end * Self::FADE_RESOLUTION_FAR).max(start + 1.0);
                pack_int_pair(start, end)
            },
        };
    }

    pub fn alpha(&self) -> f32 {
        let alpha = (self.flags & Self::FLAG_ALPHA_MASK) as f32;
        alpha / 255.0
    }
    pub fn set_alpha(&mut self, alpha: f32) {
        let alpha = alpha * 255.0;
        self.flags = self.flags & !Self::FLAG_ALPHA_MASK | (alpha as u32).min(0xff);
    }

    pub fn set_anim_scale(&mut self, scale: f32) {
        let bias = self.anim_scale.abs().trunc();
        self.anim_scale = (scale / 64.0).clamp(-1.0 + f32::EPSILON, 1.0 - f32::EPSILON) + bias;
    }
    pub fn set_depth_bias(&mut self, bias: u32) {
        self.anim_scale = self.anim_scale.fract() + bias as f32;
    }
}
impl Default for MarkerInstanceData {
    #[inline]
    fn default() -> Self { Self::INVALID }
}

#[derive(Debug, Copy, Clone, Default)]
#[repr(C, align(16))]
pub struct ConstantDataV {
    pub render: RenderConstantDataV,
    pub trail: TrailConstantDataV,
    pub poi: PoiConstantDataV,
}
unsafe impl D3dBufferData for ConstantDataV {}
#[derive(Debug, Copy, Clone, Default)]
#[repr(C, align(16))]
pub struct ConstantDataP {
    pub render: RenderConstantDataP,
    pub trail: TrailConstantDataP,
    pub poi: PoiConstantDataP,
}
unsafe impl D3dBufferData for ConstantDataP {}

#[derive(Debug, Copy, Clone, Default)]
#[repr(C)]
pub struct RenderConstantDataV {
    pub projection: Matrix4<f32>,
    /// TODO: Mat43
    #[cfg(todo)]
    pub view: Mat43,
    pub view: Matrix4<f32>,
    pub player_pos: Vector3,
    pub anim_timestamp: f32,
    /// XXX: HLSL could extract this from view matrix?
    ///
    /// cbuffer space isn't limited though, so better not to?
    pub camera_pos: Vector3,
    pub _padding0: f32,
    /// see `camera_pos` comment
    pub camera_dir: Vector3,
    /// used to restrict billboard sizes on-screen
    pub viewport_pixel_scale: f32,
    pub _padding2: Vector4,
}
unsafe impl D3dBufferData for RenderConstantDataV {}
#[derive(Debug, Copy, Clone, Default)]
#[repr(C)]
pub struct RenderConstantDataP {
    #[cfg(todo)]
    pub viewport: Vector2,
    pub edge_feather_viewport: Vector2,
    pub player_feather: f32,
    pub distance_fade: f32,
    pub edge_feather: [f32; 2],
    pub _padding0: Vector2,
}
unsafe impl D3dBufferData for RenderConstantDataP {}
#[derive(Debug, Copy, Clone, Default)]
#[repr(C)]
pub struct MarkerConstantDataP {
    pub blend_factors: Vector2<f32>,
    pub _padding0: Vector2<f32>,
}
impl MarkerConstantDataP {
    pub const INVALID: Self = Self {
        blend_factors: Self::ALPHA_FACTORS_NOP,
        _padding0: Vector2::ZERO,
    };
    /// [Self::alpha_factors(0.0)](Self::alpha_factors),
    /// nothing but plain alpha blending
    const ALPHA_FACTORS_NOP: Vector2<f32> = Vector2::new(0.0f32, 1.0f32);
    /// blend factors in order to achieve a target opacity, after partially applied
    ///
    /// example series of alpha-blended passes to achieve 80% effective opacity:
    /// 1. blend at 30%
    /// 2. blend at 80% using `alpha_factors(0.3 / 0.8)` to "fill in" the rest
    ///
    /// meant to be used like: `output.a *= 1/(output.a * factor.0 + factor.1)`,
    /// (alternatively `1/(factor.0 + factor.1/output.a)`?)
    pub fn alpha_factors(applied: f32) -> Vector2<f32> {
        Vector2::new(applied / (applied - 1.0), 1.0 / (1.0 - applied))
    }
    pub fn set_blend_factors(&mut self, applied: Option<f32>) {
        let applied = match applied {
            Some(amt) if amt.is_nan() => None,
            Some(amt) if amt >= 1.0 => None,
            Some(0.0) => None,
            a => a,
        };
        self.blend_factors = applied.map(Self::alpha_factors).unwrap_or(Self::ALPHA_FACTORS_NOP);
    }
    pub fn effective_alpha(&self, alpha: f32) -> f32 {
        let [blend_factor, blend_const] = self.blend_factors.to_array();
        alpha / (alpha * blend_factor + blend_const)
    }
    pub fn cumulative_alpha(&self, latest: f32, prior: f32) -> f32 {
        let latest = self.effective_alpha(latest);
        prior * (1.0 - latest) + latest
    }
}
#[derive(Debug, Copy, Clone, Default)]
#[repr(C)]
pub struct TrailConstantDataP {
    pub marker: MarkerConstantDataP,
}
#[derive(Debug, Copy, Clone, Default)]
#[repr(C)]
pub struct PoiConstantDataP {
    pub marker: MarkerConstantDataP,
}
#[derive(Debug, Copy, Clone, Default)]
#[repr(C)]
pub struct MarkerConstantDataV {
    pub scale: f32,
    pub alpha: f32,
    pub anim_scale: f32,
    pub flags: u32,
}
impl MarkerConstantDataV {
    pub const FLAG_OBSCURE_FADE: u32 = MarkerInstanceData::FLAG_OBSCURE_FADE;
    pub const FLAG_POI_LIMIT_SIZE: u32 = MarkerInstanceData::FLAG_BILLBOARD;
    pub const FLAG_DISTANCE_FADE: u32 = MarkerInstanceData::FLAG_WALL;
}
#[derive(Debug, Copy, Clone, Default)]
#[repr(C)]
pub struct TrailConstantDataV {
    pub marker: MarkerConstantDataV,
    pub tex_scale: f32,
    pub tex_offset: f32,
    pub _padding0: Vector2<f32>,
}
unsafe impl D3dBufferData for TrailConstantDataV {}
#[derive(Debug, Copy, Clone, Default)]
#[repr(C)]
pub struct PoiConstantDataV {
    /// TODO: Mat3 or Mat43, idk what hlsl likes
    pub billboard: Matrix4<f32>,
    pub marker: MarkerConstantDataV,
    /// TODO: Vec2?
    pub map_scale: f32,
    pub _padding0: Vector3<f32>,
}
unsafe impl D3dBufferData for PoiConstantDataV {}

fn pack_int_range(start: f32, end: f32) -> u32 {
    #[cfg(debug_assertions)]
    if !(start <= u16::MAX as f32) | !(end <= u16::MAX as f32) {
        log::info!("shader range {start}~{end} too big!");
    }

    const MAX: f32 = u16::MAX as f32;
    let end = match end {
        0.0..=MAX => end as u32,
        _ => 0xffff,
    };
    let start = (start as u32).min(end) as u16;
    start as u32 | end << 16
}
fn pack_int_pair(v0: f32, v1: f32) -> u32 {
    #[cfg(debug_assertions)]
    if v0 > u16::MAX as f32 || v1 > u16::MAX as f32 {
        log::info!("shader pair {v0},{v1} too big!");
    }
    let v0 = (v0 as u32) & 0xffff;
    let v1 = (v1 as u32) << 16;
    v0 | v1
}

#[derive(Debug, Copy, Clone, Default)]
#[repr(C, align(16))]
pub struct TrailVertex {
    pub position: Vector3<DrawSpace>,
    pub _padding0: f32,
    pub normal: Vector2<DrawSpace>,
    pub texture: Vector2<DrawSpace>,
}
/// doesn't use the normals but I also don't care enough
pub type PoiVertex = TrailVertex;

impl TrailVertex {
    pub const fn new(
        position: Vector3<DrawSpace>,
        texture: Vector2<DrawSpace>,
        normal: Vector2<DrawSpace>,
    ) -> Self {
        Self {
            position,
            normal,
            texture,
            _padding0: 0.0,
        }
    }
    pub const fn new_poi(
        position: Vector3<DrawSpace>,
        texture: Vector2<DrawSpace>,
    ) -> Self {
        Self::new(position, texture, Vector2::ZERO)
    }

    pub const INPUT_LAYOUT: [D3D11_INPUT_ELEMENT_DESC; 3] = [
        InputLayout::for_vertex(
            0,
            c"POSITION",
            0,
            dxgi::DXGI_FORMAT_R32G32B32_FLOAT,
            Some(offset_of!(TrailVertex, position)),
        ),
        InputLayout::for_vertex(
            0,
            c"NORMAL",
            0,
            dxgi::DXGI_FORMAT_R32G32_FLOAT,
            Some(offset_of!(TrailVertex, normal)),
        ),
        InputLayout::for_vertex(
            0,
            c"TEXCOORD",
            0,
            dxgi::DXGI_FORMAT_R32G32_FLOAT,
            Some(offset_of!(TrailVertex, texture)),
        ),
    ];

    pub const QUAD: [Self; 4] = [
        Self::new_poi(
            Vector3::new(-1.0, -1.0, 0.0),
            Vector2::new(1.0, 0.0),
        ),
        Self::new_poi(
            Vector3::new(1.0, -1.0, 0.0),
            Vector2::ZERO,
        ),
        Self::new_poi(
            Vector3::new(-1.0, 1.0, 0.0),
            Vector2::new(1.0, 1.0),
        ),
        Self::new_poi(
            Vector3::new(1.0, 1.0, 0.0),
            Vector2::new(0.0, 1.0),
        ),
    ];
    pub const POI_QUAD: [Self; 4] = [
        Self::new_poi(
            Vector3::new(-1.0, -1.0, 0.0),
            Vector2::ONE,
        ),
        Self::new_poi(
            Vector3::new(1.0, -1.0, 0.0),
            Vector2::Y,
        ),
        Self::new_poi(
            Vector3::new(-1.0, 1.0, 0.0),
            Vector2::X,
        ),
        Self::new_poi(
            Vector3::new(1.0, 1.0, 0.0),
            Vector2::ZERO,
        ),
    ];
    pub const POI_QUAD_TRANSPARENT: [Self; 4] = [
        Self {
            texture: Vector2::ONE,
            .. Self::POI_QUAD[0]
        },
        Self {
            texture: Vector2::new(0.9, 1.0),
            .. Self::POI_QUAD[1]
        },
        Self {
            texture: Vector2::new(1.0, 0.9),
            .. Self::POI_QUAD[2]
        },
        Self {
            texture: Vector2::new(0.9, 0.9),
            .. Self::POI_QUAD[3]
        },
    ];

    pub fn alloc(device: &Dx11Device, vertices: &[Self]) -> anyhow::Result<TrailVertexBuffer> {
        BufferOf::new_with_data(device, Ok(vertices), ())
    }
}
impl From<Vertex> for TrailVertex {
    fn from(v: Vertex) -> Self {
        let normal = match v.normal {
            norm if (norm.y - v.position.y).abs() > 0.001f32 =>
                norm.xz(),
            norm => norm.xz(),
        };
        Self::new(
            v.position.into(),
            v.texture.into(),
            normal.into(),
        )
    }
}
unsafe impl D3dBufferData for TrailVertex {}
pub const INPUT_LAYOUT_MARKER: [D3D11_INPUT_ELEMENT_DESC; 4] = [
    InputLayout::for_instance(1, c"MCOLOUR", 0, dxgi::DXGI_FORMAT_R32G32B32_FLOAT, Some(offset_of!(MarkerInstanceData, colour))),
    InputLayout::for_instance(1, c"MANIM", 0, dxgi::DXGI_FORMAT_R32_FLOAT, Some(offset_of!(MarkerInstanceData, anim_scale))),
    InputLayout::for_instance(1, c"MFLAG", 0, dxgi::DXGI_FORMAT_R32_UINT, Some(offset_of!(MarkerInstanceData, flags))),
    InputLayout::for_instance(1, c"MFLAG", 1, dxgi::DXGI_FORMAT_R32_UINT, Some(offset_of!(MarkerInstanceData, fade_range))),
];
pub const INPUT_LAYOUT_TRAIL_INSTANCE: [D3D11_INPUT_ELEMENT_DESC; 7] = [
    TrailVertex::INPUT_LAYOUT[0], // POSITION0
    TrailVertex::INPUT_LAYOUT[1], // NORMAL0
    TrailVertex::INPUT_LAYOUT[2], // TEXCOORD0
    INPUT_LAYOUT_MARKER[0], // MCOLOUR0
    INPUT_LAYOUT_MARKER[1], // MANIM0 (anim_scale)
    INPUT_LAYOUT_MARKER[2], // MFLAG0 (flags)
    INPUT_LAYOUT_MARKER[3], // MFLAG1 (fade)
    #[cfg(todo = "unused")]
    InputLayout::for_instance(1, c"TPADDING", 0, dxgi::DXGI_FORMAT_R32G32_FLOAT, Some(offset_of!(TrailInstanceData, x))),
];
pub const INPUT_LAYOUT_POI_INSTANCE: [D3D11_INPUT_ELEMENT_DESC; 16] = [
    TrailVertex::INPUT_LAYOUT[0], // POSITION0
    // TODO: remove! offset will deal with it
    TrailVertex::INPUT_LAYOUT[1], // NORMAL0
    TrailVertex::INPUT_LAYOUT[2], // TEXCOORD0
    INPUT_LAYOUT_MARKER[0], // MCOLOUR0
    INPUT_LAYOUT_MARKER[1], // MANIM0 (anim_scale)
    INPUT_LAYOUT_MARKER[2], // MFLAG0 (flags)
    INPUT_LAYOUT_MARKER[3], // MFLAG1 (fade)
    // size_range, bounce
    InputLayout::for_instance(1, c"PFLAG", 0, dxgi::DXGI_FORMAT_R32_UINT, Some(offset_of!(PoiInstanceData, size_range))),
    InputLayout::for_instance(1, c"PFLAG", 1, dxgi::DXGI_FORMAT_R32_UINT, Some(offset_of!(PoiInstanceData, bounce))),
    InputLayout::for_instance(1, c"PMODEL", 0, dxgi::DXGI_FORMAT_R32G32B32A32_FLOAT, Some(offset_of!(PoiInstanceData, model))),
    InputLayout::for_instance(1, c"PMODEL", 1, dxgi::DXGI_FORMAT_R32G32B32A32_FLOAT, None),
    InputLayout::for_instance(1, c"PMODEL", 2, dxgi::DXGI_FORMAT_R32G32B32A32_FLOAT, None),
    // TODO: change to 3x3
    InputLayout::for_instance(1, c"PMODEL", 3, dxgi::DXGI_FORMAT_R32G32B32A32_FLOAT, None),
    InputLayout::for_instance(1, c"PDISP", 0, dxgi::DXGI_FORMAT_R32_FLOAT, Some(offset_of!(PoiInstanceData, anim_offset))),
    InputLayout::for_instance(1, c"PDISP", 1, dxgi::DXGI_FORMAT_R32_FLOAT, Some(offset_of!(PoiInstanceData, map_scale))),
    InputLayout::for_instance(1, c"PDISP", 2, dxgi::DXGI_FORMAT_R32_FLOAT, Some(offset_of!(PoiInstanceData, billboard_scale))),
    #[cfg(todo = "unused")]
    InputLayout::for_instance(1, c"PPADDING", 0, dxgi::DXGI_FORMAT_R32G32_FLOAT, Some(offset_of!(PoiInstanceData, x))),
];
