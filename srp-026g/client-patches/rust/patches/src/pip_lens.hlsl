// Patch 198's lens: the scope's picture (the second view's) drawn as a disc over the frame,
// after its post-process, before the interface. Compiled by pip_lens.sh into pip_lens.rs.

cbuffer lens : register(b0)
{
    // xy the disc's centre, zw its radii, in the frame's clip space
    float4 disc;
    // x how sharp the rim's darkening is, y how dark the rim gets
    float4 rim;
};

Texture2D picture : register(t0);
SamplerState linear_clamp : register(s0);

struct v_out
{
    float4 position : SV_Position;
    float2 offset : TEXCOORD0;
};

// a strip of four vertices: the disc's square, by SV_VertexID alone
v_out vs_main(uint id : SV_VertexID)
{
    float2 corner = float2((id & 1) ? 1.0 : -1.0, (id & 2) ? -1.0 : 1.0);
    v_out o;
    o.offset = corner;
    o.position = float4(disc.xy + corner * disc.zw, 0.0, 1.0);
    return o;
}

// alpha-blended: the edge covers each pixel by its share inside the circle (no stair-steps)
float4 ps_main(v_out i) : SV_Target
{
    float r = length(i.offset);
    float edge = max(fwidth(r), 1e-5);
    float cover = saturate((1.0 - r) / edge);
    clip(cover - 1.0 / 512.0);
    float2 uv = i.offset * float2(0.5, -0.5) + 0.5;
    float3 c = picture.Sample(linear_clamp, uv).rgb;
    float shade = lerp(rim.y, 1.0, saturate((1.0 - r) * rim.x));
    return float4(c * shade, cover);
}
