struct VSInput
{
    float3 position: POSITION;
};

struct VSOutput
{
    float4 position: SV_Position;
    float4 colour: COLOR0;
};

VSOutput VSMain(VSInput input)
{
    VSOutput output;

    output.position = float4(input.position.xzy, 1.0);
    output.colour = float4(1.0, 1.0, 1.0, 1.0);

    return output;
}
