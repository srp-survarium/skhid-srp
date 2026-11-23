// Patch 198's lens: the scope's picture (the second view's) drawn over the frame on the real optic's
// rear glass, after the frame's post-process, before the interface. Compiled by pip_lens.sh into
// pip_lens.rs.

cbuffer lens : register(b0)
{
    // the glass in the frame's view space: its centre
    float4 centre;
    // its two radii across its plane
    float4 across0;
    float4 across1;
    // the frame's projection, rows (row vectors)
    float4 projection0;
    float4 projection1;
    float4 projection2;
    float4 projection3;
    // the picture on the screen: its centre and radii in clip space
    float4 picture_disc;
    // x how sharp the rim's darkening is, y how dark the rim gets; zw the frame's size, pixels
    float4 rim;
};

Texture2D picture : register(t0);
SamplerState linear_clamp : register(s0);

struct v_out
{
    float4 position : SV_Position;
    // the point on the glass's plane, in glass radii (a little past the rim, for its soft edge)
    float2 local : TEXCOORD0;
};

static const float MARGIN = 1.06;

// a strip of four vertices: the glass's square in its plane, by SV_VertexID alone
v_out vs_main(uint id : SV_VertexID)
{
    float2 corner = float2((id & 1) ? 1.0 : -1.0, (id & 2) ? -1.0 : 1.0) * MARGIN;
    float3 p = centre.xyz + corner.x * across0.xyz + corner.y * across1.xyz;
    v_out o;
    o.position = p.x * projection0 + p.y * projection1 + p.z * projection2 + projection3;
    o.local = corner;
    return o;
}

// alpha-blended: the glass's edge (a circle in its plane, an ellipse on the screen) covers each
// pixel by its share inside it (no stair-steps)
float4 ps_main(v_out i) : SV_Target
{
    float r = length(i.local);
    float edge = max(fwidth(r), 1e-5);
    float cover = saturate((1.0 - r) / edge);
    clip(cover - 1.0 / 512.0);
    float2 ndc = i.position.xy / rim.zw * float2(2.0, -2.0) + float2(-1.0, 1.0);
    float2 offset = (ndc - picture_disc.xy) / picture_disc.zw;
    float2 uv = offset * float2(0.5, -0.5) + 0.5;
    float3 c = picture.Sample(linear_clamp, uv).rgb;
    float shade = lerp(rim.y, 1.0, saturate((1.0 - r) * rim.x));
    return float4(c * shade, cover);
}
