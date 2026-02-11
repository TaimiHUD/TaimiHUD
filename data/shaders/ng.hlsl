#include "pathing.h"

#if SHADER_SPACE_TRAIL && SHADER_V
TrailOutputV trail_main_v(TrailInput input)
{
    TrailOutputV output;

    output.displacement = v_render.player_pos - input.vertex.position;

    float trail_is_wall = GET_MFLAG(input.marker.flags, MFLAG_WALL);
    float2 vnorm2 = lerp(
        float2(0.0, input.vertex.normal.y),
        float2(input.vertex.normal.y, 0.0),
        float(GET_MFLAG(input.marker.flags, MFLAG_WALL))
    );
    float3 norm = float3(input.vertex.normal.x, vnorm2.x, vnorm2.y) * v_trail.marker.scale;

    float4 pos_world = float4(input.vertex.position + norm, 1.0);
    float4 pos = mul(v_render.view, pos_world);
#if 1
    float fade_near = GET_FADE_START(input.marker.fade);
    float fade_range = GET_FADE_RANGE(input.marker.fade, fade_near);
    float fade = 1.0 - saturate((pos.z - fade_near) / fade_range);
#endif
    output.position = mul(v_render.projection, pos);

    float texoff = v_trail.tex_offset - v_render.anim_timestamp * input.marker.anim_scale;
    output.tex = float2(input.vertex.tex.x, 1.0 - MAD(input.vertex.tex.y, v_trail.tex_scale, texoff));

    output.colour = float4(input.marker.colour, GET_MFLAG_ALPHA(input.marker.flags) * v_trail.marker.alpha);
    float obscure_fade = float(GET_MFLAG(input.marker.flags, MFLAG_OBSCURE_FADE));

    output.fade = float2(obscure_fade, fade);

    return output;
}
#endif

#if SHADER_SPACE_TRAIL && SHADER_P
TrailOutputP trail_main_p(TrailOutputV vout)
{
    TrailOutputP output;
    float4 colour = vout.colour * shaderTexture.Sample(SampleType, vout.tex);
    float fade = vout.fade.y;
    colour.w = colour.w * fade;

    // XXX: or just enable depth clipping?
#if 0
    if (colour.w < DISCARD_ALPHA || vout.position.z < DiscardZ) { discard; }
#else
    clip(float2(
        colour.w - DISCARD_ALPHA,
        vout.position.z - DiscardZ
    ));
#endif

#if 1
    float distance_squared = dot(vout.displacement, vout.displacement);

    float distance_intensity = saturate(1.0 - distance_squared / (p_render.distance_fade * p_render.distance_fade));
    float intensity = INTENSITY_PARAM_2 * distance_intensity * distance_intensity + INTENSITY_PARAM_1 * distance_intensity + INTENSITY_PARAM_0;
#else
    float intensity = 1.0;
#endif
#if 1
    // fade out when close to the player
    float obscure_fade = vout.fade.x;
    float overlap = lerp(
        saturate(distance_squared / p_render.player_feather),
        1.0f,
        obscure_fade
    );
#else
    float overlap = 1.0;
#endif

#if 1
    float2 viewport_size_1 = float2(p_render.viewport.x, p_render.viewport.y);
    float3 feather_scale = float3(p_render.edge_feather.x, p_render.edge_feather.y, FEATHER_SCALE_Z);
    float3 feather_offset = float3(
        abs(FeatherOffset.xy - vout.position.xy * viewport_size_1),
        MAD(vout.position.z, -FEATHER_SIZE_Z, FeatherOffset.z)
    );
    float3 feather3 = saturate((float1(1.0).xxx - feather_offset) * feather_scale);
#else
    float3 feather3 = float3(1.0, 1.0, 1.0);
#endif

    float feather = feather3.x * feather3.y;
    colour.w = colour.w * overlap * saturate(intensity) * feather*feather * feather3.z;
    output.colour = colour;

    return output;
}
#endif

#if SHADER_SPACE_POI && SHADER_V
PoiOutputV poi_main_v(PoiInput input)
{
    PoiOutputV output;

    float is_billboard = float(GET_MFLAG(input.marker.flags, MFLAG_BILLBOARD));
    float3 vertex = input.vertex.position;
#if 1
#define POI_MIDPOINT 1
    // TODO: viewport in cb_v!
    float2 viewport = float2(3840.0, 2160.0);
    float aspect = viewport.y / viewport.x;
#if 0
    //float vpscale = sqrt(dot(viewport, viewport)) * 2.0;
#else
    // bhud just treats it as a max height rather than size?
    float vpscale = viewport.y * 2.0;
#endif

    float size_min = GET_PAIR0f(input.size_range) / vpscale;
    float size_max = GET_PAIR1f(input.size_range) / vpscale;
    float3 midpoint = mul(input.model, float4(0.0, 0.0, 0.0, 1.0)).xyz;
#if 0
    float4 viewed = mul(v_render.view, float4(midpoint, 1.0));
    float cam_dist = viewed.z;
    //float size_range = (size_max - size_min) * 0.5;
    float4 sz4 = mul(v_render.projection, viewed);
    float4 sz40 = mul(v_render.projection, float4(viewed.xy + float2(1.0, 1.0), viewed.z, 1.0));
    float2 sz = (sz40.xy / sz40.w) - (sz4.xy / sz4.w);
#else
    // TODO: cross with camera plane instead of this
    float cam_dist = distance(midpoint, v_render.camera_pos);
    // TODO: just like, use vfov angle to calculate projected height???
    float4 sz4 = mul(v_render.projection, float4(1.0, 1.0, cam_dist, 1.0));
    float2 sz = sz4.xy / sz4.w;
#endif
    float size_screen_screen = sqrt(dot(sz, sz));
    float limit_max = saturate(size_max / size_screen_screen);
    float limit_min = saturate(size_screen_screen / size_min);
    vertex.xy = vertex.xy * lerp(
        1.0,
        clamp(1.0, (1.0 / limit_min), limit_max),
        is_billboard
    );
#if TODO
#endif
#endif
    float3 pos_origin = vertex * v_poi.marker.scale;
    pos_origin = lerp(
        pos_origin,
        mul(v_poi.billboard, float4(pos_origin, 1.0)).xyz,
        is_billboard
    );
    float4 pos = mul(input.model, float4(pos_origin, 1.0));
#ifdef POI_MIDPOINT
    output.displacement = v_render.player_pos - midpoint;
#else
    output.displacement = v_render.player_pos - pos.xyz;
#endif

#if 1
    float bounce_height = GET_BOUNCE_DIST(input.bounce);
    float bounce_anim = (v_render.anim_timestamp - input.anim_offset) * input.marker.anim_scale;
    float bounce_y = lerp(
        saturate(bounce_anim),
        sin(bounce_anim),
        float(!GET_MFLAG(input.marker.flags, MFLAG_RISE))
    );
    pos.y = pos.y + bounce_y;
#endif

    pos = mul(v_render.view, pos);
#if 1
    // TODO: consider using displacement (from char) rather than camera z?
    float fade_near = GET_FADE_START(input.marker.fade);
    float fade_range = GET_FADE_RANGE(input.marker.fade, fade_near);
    float fade = 1.0 - saturate((pos.z - fade_near) / fade_range);
#endif
    pos = mul(v_render.projection, pos);
    output.position = pos;

    output.tex = input.vertex.tex;

    // TODO: map anti-scale
    // TODO: max/min size
    // TODO: can-fade
    // TODO: fadenear/fadefar
    output.colour = float4(input.marker.colour, GET_MFLAG_ALPHA(input.marker.flags) * v_poi.marker.alpha);
    // TODO: apply+preprocess player_feather here?
    float obscure_fade = float(GET_MFLAG(input.marker.flags, MFLAG_OBSCURE_FADE));
    output.fade = float2(obscure_fade, fade);

    return output;
}
#endif

#if SHADER_SPACE_POI && SHADER_P
PoiOutputP poi_main_p(PoiOutputV vout)
{
    PoiOutputP output;
    float4 colour = vout.colour * shaderTexture.Sample(SampleType, vout.tex);
    if (colour.w < DISCARD_ALPHA || vout.position.z < DiscardZ) { discard; }

    float distance_squared = dot(vout.displacement, vout.displacement);

    float distance_intensity = saturate(1.0 - distance_squared / (p_render.distance_fade * p_render.distance_fade));

    float2 viewport_size_1 = float2(p_render.viewport.x, p_render.viewport.y);
    float3 feather_scale = float3(p_render.edge_feather.x, p_render.edge_feather.y, FEATHER_SCALE_Z);
    float3 feather_offset = float3(
        abs(FeatherOffset.xy - vout.position.xy * viewport_size_1),
        MAD(vout.position.z, -FEATHER_SIZE_Z, FeatherOffset.z)
    );
    float3 feather3 = saturate((float1(1.0).xxx - feather_offset) * feather_scale);
    float feather = feather3.x * feather3.y;

    float intensity = INTENSITY_PARAM_2 * distance_intensity * distance_intensity + INTENSITY_PARAM_1 * distance_intensity + INTENSITY_PARAM_0;

#if 0
    float overlap = distance_squared / p_render.player_feather;
#endif

    float alpha = colour.w * saturate(intensity) * feather*feather * feather3.z;
    output.colour = float4(colour.xyz, alpha);

    return output;
}
#endif
