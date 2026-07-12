struct MapInput2dV {
    float2 position: POSITION;
    // TODO: is float3 position: POSITION; possible and switch out inputlayout to source POSITION0=R8G8(slot0), POSITION1=B8X8(slot1)?
    float2 normal: NORMAL0;
    float2 tex: TEXCOORD0;
    //float2 _padding;
};
struct MapMarkerInput {
    // TODO: affine? 4x4 seems wasteful for 2d...
    column_major float4x4 model: MODEL;
    float4 colour: MCOLOUR;
    float anim_scale: MANIM0;
    float mid_height: MANIM1;
    // TODO? float trail_scale: MANIM2;
    nointerpolation uint flags: MFLAG0;
    // TODO? nointerpolation uint fade: MFLAG1;
    //float _padding;
};
struct MapInput2d {
    MapInput2dV vertex;
    MapMarkerInput marker;
    //float2 _padding;
};
struct MapOutput2dV {
    float4 position: SV_Position;
    float4 colour: COLOR0;
    float2 tex: TEXCOORD0;
    nointerpolation uint instance: OFLAGS0;
    // TODO? float map_scale: POSITION1;
};
struct MapOutput2dP {
    float4 colour: SV_Target0;
};
struct MapMarkerSharedV {
    // TODO? float scale;
    float2 _padding0;
    float anim_scale;
    uint flags;
};
struct MapTrailSharedV {
    float tex_scale;
    float tex_offset;
    float scale_expand;
    float _padding0;
};
struct MapPoiSharedV {
    float3 _padding0;
    float scale;
};
struct MapMarkerSharedP {
    float3 _padding;
    float alpha;
};
struct MapTrailSharedP {
    MapMarkerSharedP marker;
};
struct MapPoiSharedP {
    MapMarkerSharedP marker;
};

struct MapRenderSharedV {
    column_major float4x4 projection;
    float2 _padding;
    float map_scale;
    float anim_timestamp;
    // TODO: map offset+compasssize/scale?
};
struct MapUiSharedV {
    column_major float4x4 viewport_ortho;
};
struct MapRenderSharedP {
    float4 tint;
};

#if SHADER_MAP
#if SHADER_V
cbuffer EntitySharedV : register(b0) {
    MapRenderSharedV v_render;
    MapMarkerSharedV v_marker;
    MapUiSharedV v_ui;
    MapTrailSharedV v_trail;
    MapPoiSharedV v_poi;
}
#endif
#if SHADER_P
cbuffer EntitySharedP : register(b0) {
    MapRenderSharedP p_render;
    MapTrailSharedP p_trail;
    MapPoiSharedP p_poi;
}
Texture2D shaderTexture : register(t0);
SamplerState SampleType : register(s0);
#endif
#endif

#define MFLAG_MAP2D_STATIC_SCALE 0x04
//#define MFLAG_MAP2D_BILLBOARD 0x02
#define MFLAG_MAP2D_IS_TRAIL 0x80
#ifndef GET_MFLAG
#define GET_MFLAG(flags, flag) bool(GET_MFLAG_BIT(flags, flag))
#define GET_MFLAG_BIT(flags, flag) ((flags) & (flag))
#define GET_SCALE_ANIM(scale) ((scale % 1.0f) * 64.0f)
#define MAD(v, m, a) ((v) * (m) + (a))
#endif
