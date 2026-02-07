struct VSInput
{
    float3 position: POSITION;
    float2 tex: TEXCOORD0;
    float3 vcolour: COLOR0;
    float3 normal: NORMAL;
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
    float4 Expand;
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

    //float expand_dir = normalize(input.normal);
    float3 expand_dir = input.normal;
    float isTrail = dot(expand_dir, expand_dir); // 1.0 for trails, 0.0 for POIs
    //float isPoi = step(isTrail, 0.5);
    float isPoi = 1.0 - isTrail;

    float3 norm = expand_dir * Expand.x;
    float scalePos = isPoi * Expand.y;
    //float scalePos = 0.0;
    float3 pos = input.position + norm + input.position * scalePos;

    float3 poipos = mul(Model, float4(pos, 1.0)).xyz;
    float4 mpos = mul(input.Model, float4(pos * isTrail + isPoi * poipos, 1.0));
    float4 mvpos = mul(World, mpos);
    output.position = mul(View, mvpos.xzyw);

    float scaleTex = Expand.w - 1.0;
    output.tex = float2(input.tex.x, input.tex.y + isTrail * (input.tex.y * scaleTex + Expand.z));

    output.colour = float4(input.colour.xyz * input.vcolour, input.colour.w);

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
