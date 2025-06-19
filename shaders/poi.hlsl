struct VSInput
{
    float3 position: POSITION;
    float2 tex: TEXCOORD0;
};

Texture2D shaderTexture : register(t0);
SamplerState SampleType : register(s0);

cbuffer ConstantBuffer : register(b0)
{
    column_major matrix View;
    column_major matrix Projection;
}

cbuffer SpriteData : register(b1)
{
	column_major matrix Model;
	float4 tint;
}

struct VSOutput
{
    float4 position: SV_Position;
    float4 color: COLOR0;
    float2 tex: TEXCOORD0;
};

VSOutput VSMain(VSInput input)
{
    VSOutput output = (VSOutput)0;

    float4 VertPos = float4(input.position, 1.0);
    output.position = mul(Model, VertPos);
    output.position = mul(View, output.position);
    output.position = mul(Projection, output.position);

    output.tex = input.tex;
    output.color = tint;

    return output;
}

struct PSOutput
{
    float4 color: SV_Target0;
};

PSOutput PSMain(VSOutput input)
{
    PSOutput output = (PSOutput)0;
    float2 newtex = float2(input.tex.x, 1 - input.tex.y);
    float4 textureColour = shaderTexture.Sample(SampleType, newtex);
    output.color = input.color * textureColour;
    return output;
}
