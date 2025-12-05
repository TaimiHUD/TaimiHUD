#ifndef DISCARD_ALPHA
#define DISCARD_ALPHA 0.01
#endif
#ifndef DISCARD_Z
#define DISCARD_Z 1.0
#endif
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
#define dot3(l, r) (l.x * r.x + l.y * r.y + l.z * r.z)

struct VSInput
{
    float3 position: POSITION;
    float3 color: COLOR0;
    float3 normal: NORMAL;
    float2 tex: TEXCOORD0;
};

Texture2D shaderTexture : register(t0);
SamplerState SampleType : register(s0);

cbuffer ConstantBuffer : register(b0)
{
    column_major matrix View;
    column_major matrix Projection;
    column_major matrix Billboard;
    float4 PlayerPos;
    float4 Expand;
}

struct VSOutput
{
    float4 position: SV_Position;
    float4 color: COLOR0;
    float2 tex: TEXCOORD0;
    /*noperspective*/ float4 distance: POSITION1;
};

VSOutput VSMain(VSInput input)
{
    VSOutput output;

    float3 norm = input.normal * Expand.x;
    float4 pos = float4(input.position + norm, 1.0);
    //float4 pos = float4(input.position, 1.0);

    float3 displacement = PlayerPos.xyz - pos.xyz;
    output.distance = float4(displacement, 1.0);

    float4 vpos = mul(View, pos);
    output.position = mul(Projection, vpos);

    output.tex = float2(input.tex.x, input.tex.y * Expand.w + Expand.z);

    float alpha = PlayerPos.w;
    output.color = float4(input.color, alpha);

    return output;
}

cbuffer PConstantBuffer : register(b0)
{
    float4 DistanceParam;
    float4 ViewportParam;
}

struct PSOutput
{
    float4 color: SV_Target0;
};

static const float DiscardZ = FEATHER_OFFSET.z * DISCARD_Z;
static const float3 FeatherOffset = float3(FEATHER_OFFSET.xy, 1.0 + FEATHER_OFFSET.z * FEATHER_SIZE_Z);
PSOutput PSMain(VSOutput input)
{
    PSOutput output;
    float2 newtex = float2(input.tex.x, 1 - input.tex.y);
    float4 textureColour = input.color * shaderTexture.Sample(SampleType, newtex);
    //if (textureColour.w < DISCARD_ALPHA || input.position.z < DiscardZ) { discard; }

    float3 displacement = input.distance.xyz;
    float distance_squared = dot3(displacement, displacement);

    float distance_intensity = saturate(1.0 - distance_squared / (DistanceParam.y * DistanceParam.y));

    float2 viewport_size_1 = float2(ViewportParam.x, ViewportParam.y);
    float3 feather_scale = float3(DistanceParam.z, DistanceParam.w, FEATHER_SCALE_Z);
    float3 feather_offset = float3(
        abs(FeatherOffset.xy - input.position.xy * viewport_size_1),
        FeatherOffset.z - input.position.z * FEATHER_SIZE_Z /* / input.position.w */
    );
    float3 feather3 = saturate((float1(1.0).xxx - feather_offset) * feather_scale);
    float feather = feather3.x * feather3.y;

    float intensity = INTENSITY_PARAM_2 * distance_intensity * distance_intensity + INTENSITY_PARAM_1 * distance_intensity + INTENSITY_PARAM_0;

    // fade out when close to the player
    float overlap_threshold = DistanceParam.x;
    float overlap = distance_squared / overlap_threshold;

    float alpha = textureColour.w * saturate(overlap) * saturate(intensity) * feather*feather * feather3.z;
    output.color = float4(textureColour.xyz, alpha);

    return output;
}
