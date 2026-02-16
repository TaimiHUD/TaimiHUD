#if !defined(SHADER_P) && !defined(SHADER_V)
#define SHADER_V 1
#define SHADER_P 1
#ifndef SHADER_SPACE
#define SHADER_SPACE 3
#endif
#else
#ifndef SHADER_P
#define SHADER_P 0
#endif
#ifndef SHADER_V
#define SHADER_V 0
#endif
#endif
#ifndef SHADER_SPACE
#define SHADER_SPACE 0
#endif

#define SHADER_POI_IS 1
#define SHADER_TRAIL_IS 2
#define SHADER_SPACE_POI ((SHADER_SPACE == SHADER_POI_IS) || (SHADER_SPACE == 3))
#define SHADER_SPACE_TRAIL ((SHADER_SPACE == SHADER_TRAIL_IS) || (SHADER_SPACE == 3))

#if SHADER_P

#ifndef DISCARD_ALPHA
#define DISCARD_ALPHA 0.01
#endif
#ifndef DISCARD_Z
#define DISCARD_Z 1.0
#endif
static const float DiscardZ = FEATHER_OFFSET.z * DISCARD_Z;

#if SHADER_SPACE
#ifndef INTENSITY_PARAM_2
#define INTENSITY_PARAM_2 1.3
#define INTENSITY_PARAM_1 0.2
#define INTENSITY_PARAM_0 0.2
#endif
#ifndef FEATHER_OFFSET
#define FEATHER_OFFSET float3(1.0, 1.0, 0.1)
#endif
#ifndef FEATHER_SIZE_Z
#define FEATHER_SIZE_Z 1.0
#define FEATHER_SCALE_Z 5.0
#endif
static const float3 FeatherOffset = float3(FEATHER_OFFSET.xy, 1.0 + FEATHER_OFFSET.z * FEATHER_SIZE_Z);
#endif

#endif

struct MarkerInput {
    float3 colour: MCOLOUR;
    float anim_scale: MANIM0;
    nointerpolation uint flags: MFLAG0;
    nointerpolation uint fade: MFLAG1;
};
#define FADE_RESOLUTION_NEAR 8.0f
#define FADE_RESOLUTION_FAR 4.0f
#define GET_FADE_START(f) (GET_PAIR0f(f) / FADE_RESOLUTION_NEAR)
//#define GET_FADE_RANGE(f, start) (GET_FADE_FAR(f) - start)
#define GET_FADE_RANGE(f, _start) (GET_PAIR1f(f) / FADE_RESOLUTION_FAR)

struct TrailInputV {
    float3 position: POSITION;
    float2 normal: NORMAL0;
    float2 tex: TEXCOORD0;
};
struct TrailInput {
    TrailInputV vertex;
    MarkerInput marker;
    //float2 _padding;
};

#if 0
struct PoiInputV {
    float3 position: POSITION;
    float2 tex: TEXCOORD0;
};
#endif
#define PoiInputV TrailInputV
// TODO: use InputV
// TODO: combine bounce+sizerange into a PoiInputParams struct so it can be one index in layout!
// TODO: combine anim+scale/etc into a struct so it can be one float4 in layout too
struct PoiInput {
    PoiInputV vertex;
    MarkerInput marker;
    nointerpolation uint size_range: PFLAG0;
    nointerpolation uint bounce: PFLAG1;
    column_major float4x4 model: PMODEL;
    float anim_offset: PDISP0;
    float map_scale: PDISP1;
    //float2 _padding;
};
#define BOUNCE_HEIGHT_RESOLUTION 16.0f
#define BOUNCE_HEIGHT_OFFSET 16384
#define GET_PAIR0i(b) ((b) & 0xffff)
#define GET_PAIR0f(b) float((b) & 0xffff)
#define GET_PAIR1f(b) float((b) >> 16)
#define GET_BOUNCE_DIST(b) (float(int(GET_PAIR0i(b)) - BOUNCE_HEIGHT_OFFSET) / BOUNCE_HEIGHT_RESOLUTION)
//#define GET_BOUNCE_ARG1 GET_PAIR1f
#if 0
#define BOUNCE_DURATION_RESOLUTION 16.0f
//#define GET_BOUNCE_DUR(b) (float(b /*& 0xffff0000*/) / BOUNCE_DURATION_RESOLUTION_F)
#define GET_BOUNCE_DUR(b) (float((b) >> 16) / BOUNCE_DURATION_RESOLUTION)
#endif

struct SpaceOutputV {
    float4 position: SV_Position;
    float4 colour: COLOR0;
    float2 tex: TEXCOORD0;
    /*noperspective*/ float3 displacement: POSITION1;
    float2 fade: OFADE0;
    nointerpolation uint2 instance: OFLAGS0;
};
#define TrailOutputV SpaceOutputV
#define PoiOutputV SpaceOutputV
struct MapOutputV {
    SpaceOutputV space;
    float map_scale: POSITION2;
};

struct SpaceInputP {
    SpaceOutputV space;
    //bool face_front: SV_IsFrontFace;
};
#define TrailInputP SpaceInputP
#define PoiInputP SpaceInputP

struct SpaceOutputP {
    float4 colour: SV_Target0;
};
#define TrailOutputP SpaceOutputP
#define PoiOutputP SpaceOutputP

struct MarkerSharedV {
    float scale;
    float alpha;
    float anim_scale;
    uint flags;
};
#define SFLAG_DISTANCE_FADE MFLAG_WALL
struct PoiSharedV {
    column_major float4x4 billboard;
    MarkerSharedV marker;
    float map_scale;
    float _padding0;
    float _padding1;
    float _padding2;
};
struct TrailSharedV {
    MarkerSharedV marker;
    float tex_scale;
    float tex_offset;
    float _padding0;
    float _padding1;
};
struct RenderSharedP {
    float2 viewport;
    float player_feather;
    float distance_fade;
    float2 edge_feather;
};
struct RenderSharedV {
    column_major float4x4 projection;
    column_major float4x4 view;
    float3 player_pos;
    float anim_timestamp;
    float3 camera_pos;
    float _padding0;
    float3 camera_dir;
    float viewport_pixel_scale;
    float4 _padding1;
};

#if SHADER_SPACE
#if SHADER_V
cbuffer EntitySharedV : register(b0) {
    RenderSharedV v_render;
    TrailSharedV v_trail;
    PoiSharedV v_poi;
}
#endif
#if SHADER_P
cbuffer EntitySharedP : register(b0) {
    RenderSharedP p_render;
}
Texture2D shaderTexture : register(t0);
SamplerState SampleType : register(s0);
#endif
#endif

#define MFLAG_ALPHA_MASK 0x00ff
#define MFLAG_OBSCURE_FADE 0x0100
#define MFLAG_BILLBOARD 0x0200
#define MFLAG_MAP_STATIC_SCALE 0x0400
#define MFLAG_RISE 0x0800
#define MFLAG_WALL 0x1000
#define MFLAG_OPAQUE 0x2000
#define MFLAG_IS_TRAIL 0x8000
#define MFLAG_FACE_CULL_FRONT 0x10000
#define MFLAG_FACE_CULL_FRONT_SHIFT 16
#define MFLAG_FACE_CULL 0x20000
#define GET_MFLAG(flags, flag) bool(GET_MFLAG_BIT(flags, flag))
#define GET_MFLAG_BIT(flags, flag) ((flags) & (flag))
#define GET_MFLAG_ALPHA(flags) (float(GET_MFLAG_BIT(flags, MFLAG_ALPHA_MASK)) / 255.0f)

//#define MAD(v, m, a) mad(v, m, a)
#define MAD(v, m, a) ((v) * (m) + (a))
