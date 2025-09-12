struct VSInput
{
    float3 position: POSITION;
    float2 tex: TEXCOORD0;
    column_major matrix Model: MODEL;
    float4 colour: COLOUR;
};

Texture2D shaderTexture : register(t0);
SamplerState SampleType : register(s0);

cbuffer ConstantBuffer : register(b0)
{
    column_major matrix Model;
    column_major matrix World;
    column_major matrix View;
}

struct VSOutput
{
    float4 position: SV_Position;
    float4 colour: COLOR0;
    float2 tex: TEXCOORD0;
};

VSOutput VSMain(VSInput input)
{
    VSOutput output;

    float4 VertPos = float4(input.position, 1.0);
    float4 mpos = mul(input.Model, mul(Model, VertPos));
    float4 mvpos = mul(World, mpos);
    output.position = mul(View, mvpos.xzyw);

    output.tex = float2(1.0 - input.tex.x, input.tex.y);

    output.colour = input.colour;

    return output;
}

cbuffer PConstantBuffer : register(b0)
{
    float4 Tint;
}

struct PSOutput
{
    float4 colour: SV_Target0;
};

PSOutput PSMain(VSOutput input)
{
    PSOutput output;
    float2 newtex = float2(input.tex.x, 1 - input.tex.y);
    float4 textureColour = shaderTexture.Sample(SampleType, newtex);

    output.colour = input.colour * textureColour * Tint;

    return output;
}
