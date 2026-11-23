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
    // the depth the frame's first-person models were drawn with: x, y the projection's [2][2] and [3][2]
    // (their own depth range), z how far toward the eye the glass is tested (m): what of the optic is in
    // front of its rear glass (a rim, an eyecup's lip) covers the picture; w 1 the frame's depth is
    // bound, 2 shown instead (red the frame's, green the glass's), 0 none
    float4 depth;
    // the exit pupil: xy the clear field's centre in glass radii (it follows the eye off the optic's
    // axis), z its soft edge: the rest of the glass in its shadow
    float4 pupil;
};

Texture2D picture : register(t0);
Texture2D<float> frame_depth : register(t1);
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
    o.position.z = (depth.x + depth.y / max(p.z - depth.z, 1e-4)) * o.position.w;
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
    if (depth.w > 0.5)
    {
        float scene = frame_depth.Load(int3(i.position.xy, 0));
        if (depth.w > 1.5)
            return float4(saturate((1.0 - scene) * 20.0), saturate((1.0 - i.position.z) * 20.0), 0.0, 1.0);
        clip(scene - i.position.z);
    }
    float2 ndc = i.position.xy / rim.zw * float2(2.0, -2.0) + float2(-1.0, 1.0);
    float2 offset = (ndc - picture_disc.xy) / picture_disc.zw;
    float2 uv = offset * float2(0.5, -0.5) + 0.5;
    float3 c = picture.Sample(linear_clamp, uv).rgb;
    float shade = lerp(rim.y, 1.0, saturate((1.0 - r) * rim.x));
    shade *= 1.0 - smoothstep(1.0, 1.0 + pupil.z, length(i.local - pupil.xy));
    return float4(c * shade, cover);
}
