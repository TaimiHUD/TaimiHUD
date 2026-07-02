#include "pathing.h"

#if SHADER_SPACE_TRAIL && SHADER_V
TrailOutputV trail_main_v(TrailInput input)
{
    TrailOutputV output;

    output.displacement = v_render.player_pos - input.vertex.position;

    float trail_is_wall = float(GET_MFLAG(input.marker.flags, MFLAG_WALL));
    float2 vnorm2 = lerp(
        float2(0.0, input.vertex.normal.y),
        float2(input.vertex.normal.y, 0.0),
        trail_is_wall
    );
    float3 norm = float3(input.vertex.normal.x, vnorm2.x, vnorm2.y) * v_trail.marker.scale;

    float4 pos_world = float4(input.vertex.position + norm, 1.0);

    float3 norm_tri = float3(0.0, 1.0 - trail_is_wall, trail_is_wall);
    float face_dir = dot(v_render.camera_dir, norm_tri);
    bool back_of_face = face_dir < 0.0;

#if GOGGLES2_REFLECTING
    float fade2 = pos_world.y;
#else
    float fade2 = 0.0;
#endif

    // TODO: if GET_MFLAG(input.marker.flags, MFLAG_WALL) adjust x+y or z-bias after transform to camera or something
    pos_world.y += (float)GET_SCALE_BIAS24U(input.marker.anim_scale) * pow(dot(pos_world.xyz - v_render.camera_pos, v_render.camera_dir), 2.0) * DepthBufScaleT;
    float4 pos = mul(v_render.view, pos_world);
#if 1
    float fade_near = GET_MFLAG(v_trail.marker.flags, SFLAG_DISTANCE_FADE) ? GET_FADE_START(input.marker.fade) : 9999.0;
    float fade_range = GET_FADE_RANGE(input.marker.fade, fade_near);
#if 0
    // camera.z is a fine estimate for long-distance fades, but not precise ranges...
    float fade_dist = pos.z - fade_near;
#else
    // using displacement (from char) rather than camera.z
    float fade_dist = length(output.displacement) - fade_near;
#endif
    float fade = saturate(1.0 - pow(fade_dist / fade_range, 3.0));
#endif
    output.position = mul(v_render.projection, pos);
#if GOGGLES2_SHADOWBOXING
    if (GET_MFLAG(v_trail.marker.flags, 0x4000)) {
        output.position.z = output.position.z + 0.0015;
    }
#endif

    float texoff = v_trail.tex_offset - v_render.anim_timestamp * GET_SCALE_ANIM(input.marker.anim_scale) * v_trail.marker.anim_scale;
    output.tex = float2(input.vertex.tex.x, 1.0 - MAD(input.vertex.tex.y, v_trail.tex_scale, texoff));

    float3 input_colour = input.marker.colour;
#if GOGGLES_OBSCURED && 0
    input_colour = saturate(input_colour * max(1.0, v_trail.marker.alpha));
#endif
    output.colour = float4(input_colour, GET_MFLAG_ALPHA(input.marker.flags) * v_trail.marker.alpha);
#if GOGGLES2_SHADOWBOXING && 0
    if (GET_MFLAG(v_trail.marker.flags, 0x4000)) {
        output.colour.x = 0.0;
        output.colour.y = 0.0;
        output.colour.z = 0.0;
    }
#endif
#if GOGGLES2_REFLECTING
    // to draw directly onto water surface
    output.position.y *= GET_MFLAG(v_trail.marker.flags, 0x4000) ? -1.0 : 1.0;
#endif

    // TODO: use clip/cull planes for anything we know here (tex alpha obviously missing)
    // float clip_fade = float(GET_MFLAG(input.marker.flags, MFLAG_OPAQUE));
    uint flags = input.marker.flags
        | (v_trail.marker.flags & MFLAG_OBSCURE_FADE);
    flags = flags ^ (uint(back_of_face) << MFLAG_FACE_CULL_FRONT_SHIFT);
    output.instance = uint2(
        flags
#if GOGGLES2_SHADOWBOXING || GOGGLES2_REFLECTING
            | (v_trail.marker.flags & 0x4000)
#endif
            | (v_trail.marker.flags & (0x40000 | 0x80000))
            | MFLAG_IS_TRAIL
        ,
        0
    );
    output.fade = float2(fade, fade2);

    return output;
}
#endif

#if SHADER_SPACE_TRAIL && SHADER_P
TrailOutputP trail_main_p(TrailInputP inp)
{
    SpaceOutputV vout = inp.space;
    uint flags = vout.instance.x;
    TrailOutputP output;
    float4 colour = vout.colour * shaderTexture.Sample(SampleType, vout.tex);
#if GOGGLES_OBSCURED
    colour.w += (0.2 + colour.x + colour.y + colour.z) * max(0.0, vout.colour.w - 1.0);
#endif
    float fade = vout.fade.x;
    colour.w = colour.w * fade;

#if 0
    bool face_front = inp.face_front;
#else
    // we don't have any sources of back-facing geometry yet...
    bool face_front = true;
#endif
    bool face_cull = GET_MFLAG(flags, MFLAG_FACE_CULL);
    bool face_cull_dir = GET_MFLAG(flags, MFLAG_FACE_CULL_FRONT) ^ face_front;
    float clip_face = float((!face_cull) | face_cull_dir) - 0.5f;
#if GOGGLES2_REFLECTING
    float clip_water_plane = vout.fade.y * (GET_MFLAG(flags, 0x4000) ? -1.0 : 1.0) + UNDERWATER_VISIBILITY;
#endif

    float clip_fade = float(GET_MFLAG(flags, MFLAG_OPAQUE));
    // XXX: or just enable depth clipping?
#if 0
    if ((colour.w + clip_fade) < DISCARD_ALPHA || vout.position.z < DiscardZ) { discard; }
#else
    clip(float3(
#if GOGGLES2_REFLECTING
        clip_water_plane,
#else
        clip_face,
#endif
        colour.w + clip_fade - DISCARD_ALPHA,
        vout.position.z - DiscardZ
    ));
#endif

#if GOGGLES2_REFLECTING
    float intensity = 1.0;
#else
    float distance_squared = dot(vout.displacement, vout.displacement);

    float distance_intensity = saturate(1.0 - distance_squared / (p_render.distance_fade * p_render.distance_fade));
    float intensity = INTENSITY_PARAM_2 * distance_intensity * distance_intensity + INTENSITY_PARAM_1 * distance_intensity + INTENSITY_PARAM_0;
#endif
#if GOGGLES2_REFLECTING
    float overlap = 1.0;
#else
    // fade out when close to the player
    float obscure_fade = float(GET_MFLAG(flags, MFLAG_OBSCURE_FADE));
    float player_feather = p_render.player_feather * lerp(
        0.2f,
        1.0f,
        float(GET_MFLAG(flags, MFLAG_IS_TRAIL))
    );
    float overlap = lerp(
        saturate(distance_squared / player_feather),
        1.0f,
        obscure_fade
    );
#endif

#if GOGGLES2_REFLECTING
    float3 feather3 = float3(1.0, 1.0, 1.0);
#else
    float2 viewport_size_1 = float2(p_render.edge_viewport21.x, p_render.edge_viewport21.y);
    float3 feather_scale = float3(p_render.edge_feather.x, p_render.edge_feather.y, FEATHER_SCALE_Z);
    float3 feather_offset = float3(
        abs(FeatherOffset.xy - vout.position.xy * viewport_size_1),
        MAD(vout.position.z, -FEATHER_SIZE_Z, FeatherOffset.z)
    );
    float3 feather3 = saturate((float1(1.0).xxx - feather_offset) * feather_scale);
#endif

    float feather = feather3.x * feather3.y;
    colour.w = colour.w * overlap * saturate(intensity) * feather*feather * feather3.z;
#if 0
    output.depth_push = colour.w >= 0.99 ? 0.0 : 1.0;
#else
    //output.depth_push = 0.0;
#endif
#if GOGGLES2_SHADOWBOXING
    if (GET_MFLAG(flags, 0x4000) && colour.w < 0.992) {
        discard;
    }
#endif
    float blend_factor, blend_const;
    if (GET_MFLAG(flags, MFLAG_IS_TRAIL)) {
        blend_factor = p_trail.marker.blend_factor;
        blend_const = p_trail.marker.blend_const;
    } else {
        blend_factor = p_poi.marker.blend_factor;
        blend_const = p_poi.marker.blend_const;
    }
    float alpha = colour.w * RECIP(MAD(colour.w, blend_factor, blend_const));
    output.colour = float4(
#if SHADER_P_PREMUL
        colour.xyz * alpha,
#else
        colour.xyz,
#endif
        alpha
    );
    return output;
}
#endif

#if SHADER_SPACE_POI && SHADER_V
PoiOutputV poi_main_v(PoiInput input)
{
    PoiOutputV output;

    uint is_billboard_bit = GET_MFLAG_BIT(input.marker.flags, MFLAG_BILLBOARD);
    float3 vertex = input.vertex.position;

    float3 midpoint;
#if 1
    //bool is_size_limited = is_billboard &* GET_MFLAG(v_poi.marker.flags, MFLAG_BILLBOARD);
    bool is_size_limited = bool(is_billboard_bit & v_poi.marker.flags);
    if (is_size_limited) {
    // bhud just treats it as a max height rather than size?
    float vpscale = v_render.viewport_pixel_scale;
    //float vpscale = sqrt(dot(viewport, viewport));

    float size_min = GET_PAIR0f(input.size_range) * vpscale;
    float size_max = GET_PAIR1f(input.size_range) * vpscale;
    midpoint = mul(input.model, float4(0.0, 0.0, 0.0, 1.0)).xyz;
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
    float4 sz4 = mul(v_render.projection, float4(input.billboard_scale, input.billboard_scale, cam_dist, 1.0));
    float2 sz = sz4.xy / sz4.w;
#endif
    //float size_screen_screen = RECIP(sqrt(dot(sz, sz)));
    float size_screen_screen = RECIP(sz.y);
    float limit_min = size_min * size_screen_screen;
    float limit_max = size_max * size_screen_screen;
    vertex.xy = vertex.xy * clamp(1.0, limit_min, limit_max);
    }
#endif
    bool is_billboard = bool(is_billboard_bit);

    float3 pos_origin = vertex * v_poi.marker.scale;
    pos_origin = lerp(
        pos_origin,
        mul(v_poi.billboard, float4(pos_origin, 1.0)).xyz,
        float(is_billboard)
    );
    float4 pos = mul(input.model, float4(pos_origin, 1.0));
    // TODO: prefer real midpoint if available idk
    midpoint = pos.xyz;

    output.displacement = v_render.player_pos - midpoint;

    bool back_of_face = false;
    if (!is_billboard) {
        float3 norm_origin = float3(0.0, 0.0, 1.0);
        // ignore translation, we just want it rotated...
        float3 norm = mul(input.model, float4(norm_origin, 0.0)).xyz;
        float face_dir = dot(midpoint - v_render.camera_pos, norm);
        back_of_face = face_dir < 0.0;
    }

#if 1
    float bounce_height = GET_BOUNCE_DIST(input.bounce);
    float bounce_anim = (v_render.anim_timestamp - input.anim_offset) * GET_SCALE_ANIM(input.marker.anim_scale) * v_poi.marker.anim_scale;
    float bounce_y = lerp(
        min(bounce_anim, 1.0),
        0.5f - cos(bounce_anim) * 0.5,
        float(!GET_MFLAG(input.marker.flags, MFLAG_RISE))
    ) * bounce_height;
    pos.y = pos.y + bounce_y;
#endif
#if GOGGLES2_REFLECTING
    float fade2 = pos.y;
#else
    const float fade2 = 0.0;
#endif

    pos = mul(v_render.view, pos);
#if 1
    float fade_near = GET_MFLAG(v_poi.marker.flags, SFLAG_DISTANCE_FADE) ? GET_FADE_START(input.marker.fade) : 9999.0;
    float fade_range = GET_FADE_RANGE(input.marker.fade, fade_near);
#if 0
    // camera.z is a fine estimate for long-distance fades, but not precise ranges...
    float fade_dist = pos.z - fade_near;
#else
    // using displacement (from char) rather than camera.z
    float fade_dist = length(output.displacement) - fade_near;
#endif
    float fade = saturate(1.0 - pow(fade_dist / fade_range, 3.0));
#else
    float fade = 1.0;
#endif
    pos = mul(v_render.projection, pos);
    output.position = pos;
#if GOGGLES2_SHADOWBOXING
    if (GET_MFLAG(v_poi.marker.flags, 0x4000)) {
        output.position.z = output.position.z + 0.0015;
    }
#else
    output.position.z += (float)GET_SCALE_BIAS24(input.marker.anim_scale) * DepthBufScaleT;
#endif
#if GOGGLES2_REFLECTING
    // to draw directly onto water surface
    output.position.y *= GET_MFLAG(v_poi.marker.flags, 0x4000) ? -1.0 : 1.0;
#endif

    output.tex = input.vertex.tex;

    // TODO: map anti-scale
    // TODO: max/min size
    // TODO: can-fade
    // TODO: fadenear/fadefar
    float3 input_colour = input.marker.colour;
#if GOGGLES_OBSCURED && 0
    input_colour = saturate(input_colour * max(1.0, v_poi.marker.alpha));
#endif
    output.colour = float4(input_colour, GET_MFLAG_ALPHA(input.marker.flags) * v_poi.marker.alpha);
    // TODO: apply+preprocess player_feather here?

    uint flags = input.marker.flags
        | (v_poi.marker.flags & MFLAG_OBSCURE_FADE);
    flags = flags ^ (uint(back_of_face) << MFLAG_FACE_CULL_FRONT_SHIFT);
    output.instance = uint2(
        flags
#if GOGGLES2_SHADOWBOXING || GOGGLES2_REFLECTING
            | (v_poi.marker.flags & 0x4000)
#endif
            | (v_poi.marker.flags & (0x40000 | 0x80000))
        ,
        0
    );
    output.fade = float2(fade, fade2);

    return output;
}
#endif

#if SHADER_SPACE_POI && SHADER_P
PoiOutputP poi_main_p(PoiOutputV vout)
{
    PoiOutputP output;
    float4 colour = vout.colour * shaderTexture.Sample(SampleType, vout.tex);

    float clip_fade = vout.fade.z;
    clip(float2(
        colour.w - DISCARD_ALPHA + clip_fade,
        vout.position.z - DiscardZ
    ));

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
#if 0
    output.depth_push = alpha >= 0.99 ? 0.0 : 1.0;
#endif
#if GOGGLES2_SHADOWBOXING
    if (GET_MFLAG(flags, 0x4000) && alpha < 0.992) {
        discard;
    }
#endif
    output.colour = float4(colour.xyz, alpha);

    return output;
}
#endif
