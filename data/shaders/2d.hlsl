#include "map.h"

#if SHADER_V
MapOutput2dV map2d_main_v(MapInput2d input)
{
    MapOutput2dV output;

    float map_scale = lerp(
        v_render.map_scale,
        1.0f,
        GET_MFLAG(input.marker.flags, MFLAG_MAP2D_STATIC_SCALE)
    );
    float is_trail = GET_MFLAG(input.marker.flags, MFLAG_MAP2D_IS_TRAIL);
    float scale_vert = map_scale * lerp(v_poi.scale, 1.0, is_trail);
    float2 expand_dir = input.vertex.normal;
    expand_dir *= v_trail.scale_expand * scale_vert;
    // TODO? expand_dir *= input.vertex.trail_scale;
#if 0
    expand_dir *= v_marker.scale;
    scale_vert *= v_marker.scale;
#endif
    float2 pos2 = input.vertex.position * scale_vert + expand_dir;
    float3 pos32 = float3(pos2, input.marker.mid_height);

    float4 mpos = mul(input.marker.model, float4(pos32, 1.0));
#if 0
    mpos = mpos.xzyw;
#endif
    output.position = mul(v_render.projection, mpos);

#if 1
    float texoff = v_trail.tex_offset - v_render.anim_timestamp * GET_SCALE_ANIM(input.marker.anim_scale) * v_marker.anim_scale;
    output.tex = float2(input.vertex.tex.x, 1.0 - MAD(input.vertex.tex.y, v_trail.tex_scale, texoff));
#else
    output.tex = input.vertex.tex;
#endif

    output.colour = input.marker.colour;
    uint flags = input.marker.flags;
#if 0
    flags = flags | v_marker.flags;
#endif
    output.instance = flags;

    return output;
}
#endif

#if SHADER_P
MapOutput2dP map2d_main_p(MapOutput2dV vout)
{
    MapOutput2dP output;
    uint flags = vout.instance;
    float2 tex = vout.tex;

    // TODO? tex.y = 1 - tex.y;
    output.colour = vout.colour * shaderTexture.Sample(SampleType, tex) * p_render.tint;

    output.colour.w *= lerp(
        p_poi.marker.alpha,
        p_trail.marker.alpha,
        GET_MFLAG(flags, MFLAG_MAP2D_IS_TRAIL)
    );

    return output;
}
#endif
