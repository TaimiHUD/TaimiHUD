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
    column_major matrix Billboard;
    float4 PlayerPos;
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

    float4 VertPos = float4(input.position, 1.0);

    float3 displacement = PlayerPos.xyz - VertPos.xyz;
    output.distance = float4(displacement, 1.0);

    float4 vpos = mul(View, VertPos);
    output.position = mul(Projection, vpos);

    output.tex = input.tex;

    float alpha = PlayerPos.w;
    output.color = float4(input.color, alpha);

    return output;
}

cbuffer PConstantBuffer : register(b0)
{
    float4 DistanceParam;
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

    // fade out when close to the player
    float overlap_threshold = DistanceParam.x;
    float overlap = distance_squared / overlap_threshold;

    float alpha = textureColour.w * saturate(overlap) * saturate(intensity);
    output.color = float4(textureColour.xyz, alpha);

    return output;
}
