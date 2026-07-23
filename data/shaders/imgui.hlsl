#if SHADER_V
// though imgui can share the same shader as the map, probably worth separating
// since it needs way fewer features (and requires resource binds regardless)
MapOutput2dV imgui_main_v(MapInput2d input)
{
    MapOutput2dV output;
    float2 pos2 = input.vertex.position * scale_vert + expand_dir;
    float4 pos4 = float4(input.vertex.position, 0.0f, 1.0f);
#if 0 && TODO
    output.position = mul(v_ui.viewport_ortho, pos4);
#else
    output.position = mul(input.marker.model, pos4);
#endif
    output.tex = input.vertex.tex;
    output.colour = float4(1.0f, 1.0f, 1.0f, 1.0f);
    output.instance = 0;
    return output;
}
#endif
