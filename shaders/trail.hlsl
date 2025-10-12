#ifndef DISCARD_ALPHA
#define DISCARD_ALPHA 0.01
#define DISCARD_Z 0.01
#endif
#ifndef INTENSITY_PARAM_2
#define INTENSITY_PARAM_2 1.3
#define INTENSITY_PARAM_1 0.2
#define INTENSITY_PARAM_0 0.2
#endif
#ifndef FEATHER_OFFSET
#define FEATHER_OFFSET float3(1.0, 1.0, 1.0)
#endif

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

PSOutput PSMain(VSOutput input)
{
    PSOutput output;
    float2 newtex = float2(input.tex.x, 1 - input.tex.y);
    float4 textureColour = input.color * shaderTexture.Sample(SampleType, newtex);
    if (textureColour.w < DISCARD_ALPHA || input.position.z < DISCARD_Z) { discard; }

    float3 displacement = input.distance.xyz;
    float distance_squared = dot(displacement, displacement);

    float distance_intensity = saturate(1.0 - distance_squared / (DistanceParam.y * DistanceParam.y));

    float2 viewport_size_1 = float2(ViewportParam.x, ViewportParam.y);
    float2 feather_scale = float2(DistanceParam.z, DistanceParam.w);
    float2 feather2 = saturate((float1(1.0).xx - abs(FEATHER_OFFSET.xy - input.position.xy * viewport_size_1)) * feather_scale);
    float feather = feather2.x * feather2.y;

    float intensity = INTENSITY_PARAM_2 * distance_intensity * distance_intensity + INTENSITY_PARAM_1 * distance_intensity + INTENSITY_PARAM_0;

    // fade out when close to the player
    float overlap_threshold = DistanceParam.x;
    float overlap = distance_squared / overlap_threshold;

    float alpha = textureColour.w * saturate(overlap) * saturate(intensity) * feather*feather;
    output.color = float4(textureColour.xyz, alpha);

    return output;
}
