#define FEATHER_OFFSET float2(1.0, 0.945)

struct VSInput
{
    float3 position: POSITION;
    float2 tex: TEXCOORD0;
    column_major matrix Model: MODEL;
    float4 tint: COLOUR;
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

    float4 pos = float4(input.position, 1.0);
    // TODO? float3 pos = input.position + input.position * Expand.y;
    float4 bpos = mul(Billboard, pos);
    float4 mpos = mul(input.Model, bpos);

    float3 displacement = PlayerPos.xyz - mpos.xyz;
    output.distance = float4(displacement, 1.0);

    float4 mvpos = mul(View, mpos);
    output.position = mul(Projection, mvpos);

    output.tex = input.tex;

    float alpha = PlayerPos.w;
    output.color = float4(input.tint.xyz, input.tint.w * alpha);

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
    if (textureColour.w < 0.01) { discard; }

    float3 displacement = input.distance.xyz;
    float distance_squared = dot(displacement, displacement);
    float distance_intensity = saturate(1.0 - distance_squared / (DistanceParam.y * DistanceParam.y));
    float intensity = 1.3 * distance_intensity * distance_intensity + 0.2 * distance_intensity + 0.2;

    float2 viewport_size_1 = float2(ViewportParam.x, ViewportParam.y);
    float2 feather_scale = float2(DistanceParam.z, DistanceParam.w);
    float2 feather2 = saturate((float2(1.0, 1.0) - abs(FEATHER_OFFSET - input.position.xy * viewport_size_1)) * feather_scale);
    float feather = feather2.x * feather2.y;

    float alpha = textureColour.w * saturate(intensity) * feather*feather;
    output.color = float4(textureColour.xyz, alpha);

    return output;
}
