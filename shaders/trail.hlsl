struct VSInput
{
    float3 position: POSITION;
    float3 color: COLOR0;
    float2 tex: TEXCOORD0;
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
    float3 color: COLOR0;
    float2 tex: TEXCOORD0;
};

VSOutput VSMain(VSInput input)
{
    VSOutput output;

    float4 VertPos = float4(input.position, 1.0);
    float4 vpos = mul(View, VertPos);
    output.position = mul(Projection, vpos);

    output.tex = input.tex;
    output.color = input.color;

    return output;
}

struct PSOutput
{
    float4 color: SV_Target0;
};

PSOutput PSMain(VSOutput input)
{
    PSOutput output;
    float2 newtex = float2(input.tex.x, 1 - input.tex.y);
    float4 textureColour = shaderTexture.Sample(SampleType, newtex);
    output.color = float4(input.color * textureColour.xyz, textureColour.w);
    return output;
}
