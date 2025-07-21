struct VSInput
{
    float3 position: POSITION;
    float3 normal: NORMAL;
    float3 color: COLOR0;
    float2 tex: TEXCOORD0;
    column_major matrix Model: MODEL;
    float3 colour: COLOUR;
     //uint        instId  : SV_InstanceID;
};

Texture2D shaderTexture : register(t0);
SamplerState SampleType : register(s0);

cbuffer ConstantBuffer : register(b0)
{
  column_major matrix View;
  column_major matrix Projection;
}

struct VSOutput
{
    float4 position: SV_Position;
    float3 normal: NORMAL;
    float3 color: COLOR0;
    float3 colour: COLOUR;
    float2 tex: TEXCOORD0;
};

VSOutput VSMain(VSInput input)
{
    VSOutput output;

    float4 VertPos = float4(input.position, 1.0);
    float4 mpos = mul(input.Model, VertPos);
    float4 mvpos = mul(View, mpos);
    output.position = mul(Projection, mvpos);

    output.tex = input.tex;
    output.normal = input.normal;
    output.color = input.color;
    output.colour = input.colour.xyz;

    return output;
}


struct PSInput
{
    float4 position: SV_Position;
    float3 normal: NORMAL;
    float3 color: COLOR0;
    float3 colour: COLOUR;
    float2 tex: TEXCOORD0;
};

struct PSOutput
{
    float4 color: SV_Target0;
};

PSOutput PSMain(PSInput input)
{
    PSOutput output;
    float2 newtex = float2(input.tex.x, 1 - input.tex.y);
    float4 textureColour = shaderTexture.Sample(SampleType, newtex);
    output.color = float4(input.color * textureColour.xyz, textureColour.w);
    return output;
}
