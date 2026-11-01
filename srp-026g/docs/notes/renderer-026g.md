# The 0.26g renderer

What the 0.26g client draws, in which order and from which data: the frame's passes, the lights
and their shadows, the ambient and probe light, emission, the post-process; enough for an offline
lighting bake (light maps, or light baked into textures) to reproduce the game's own materials and
lights, and for replacing the light shaders (soft shadows, section 13). It is not about the
camouflage and decal bakes of [baked-textures.md](baked-textures.md), which bake G-buffer textures
for painted items.

Sources: the executable (SHA-256 `ac33d993...3bf153`, names from `tools/re-kit/v026g/`); the
shipped shaders (fxc's disassembly of the DXBC, and recompiled from source, section 12); a real
frame captured with RenderDoc through DXVK (section 11); the 0.10b source
(`~/Projects/survarium/vostok`, branch v0.10b, engine and shader sources) for intent, always
checked against 0.26g; and an abandoned real-time raytracing attempt (2026-10-27), which ran parts
of the executable under Unicorn and the shipped shaders on a GPU with D3D11. Tags as in the notes'
README: PROVEN (read at the address or in the shader given, or the shipped shader executed),
OBSERVED (seen in the captured frame), LIKELY, GUESS.

Shaders are in `resources.db` under `shaders/sm_4_0/<family>.<ps|vs>/<permutation>`
(`tools/re-kit/common/dbgrab`). A blob is a u32 count, that many 264-byte dependency records, then
the DXBC. The 0.10b shader sources are an older generation than the 0.26g blobs (section 14); the
bytecode is the reference. Render options are the struct at `[0x2260498]` ("opt+X"); console names
from `options::register_console_commands` (0x3CD870), defaults from `options::set_default_values`
(0x3CD4D0). Blend states are the arguments of `state_descriptor::set_alpha_blend` (0x3E2EA0).

## 1. The frame

`renderer::render` (0x13D010) stores the scene in the renderer context at +416Ch (0x13D210), the
scene view at +4170h (what the stages read as the view), and the renderer's float clock (+2E4h) at
context+4140h (0x13D257), which is the `scene_time` shader constant (registered at
0x1498A6..0x1498EB). PROVEN.

`renderer::execute_stages` (0x13BCA0, renderer in EDI) runs a 27-slot stage vector (+150h/+154h)
in slot order, each non-null stage through its vtable+4 (0x13BE15). The constructor (0x139B40)
resizes it to 27 null slots (`cmp esi,1Bh`) and fills 23. Visibility, debug, view mode and screen
image are separate renderer members (+1E8h, +1E4h, +1E0h, +1DCh), not slots. Each stage tests its
own enable byte (opt+109h..+11Eh), all set to the same default (0x3CD54D..0x3CD5CB, LIKELY 1).
Classes, constructors and slots PROVEN; the names in the last column are the 0.21d PDB's
`enum_render_stage_type` with `clouds` removed (27 values, matching every class): LIKELY.

```
slot class (constructor, arg)               assigned  enable    0.21d name
  0  stage_shadow_direct (0x172330)         0x13A526  opt+109h  sun_shadows_accumulate
  1  stage_gbuffer (0x176BD0)               0x13A560  opt+10Ah  gbuffer
  2  stage_decals_accumulate (0x1784B0)     0x13A59B  opt+10Bh  decals_accumulate
  3  stage_accumulate_distortion (inline,   0x13A5E1  opt+10Ch  accumulate_distortion
     execute 0x17B4F0)
  4  stage_pre_rain (0x17B9D0)              0x13A61C  opt+10Dh  pre_rain_normal_modify
  5  stage_ambient_occlusion (0x17CEC0)     0x13A657  opt+10Eh  ambient_occlusion (SSAO, SSDO)
  6  stage_ambient_lighting (0x17F4F0)      0x13A691  opt+10Fh  ambient_lighting
  7  stage_lights, 0 (0x18EE40)             0x13A6CC  -         deferred_lighting
  8  null                                   -         -         light_propagation_volumes
  9  stage_shadow_mask (0x1A1180)           0x13A706  opt+111h  shadow_mask (the sun's)
 10  stage_atmosphere, 0 (0x1A39D0)         0x13A741  opt+115h  scattering tables + sky dome
 11  stage_composition (0x1A7280)           0x13A77C  opt+112h  composition (+ the sun)
 12  stage_atmosphere, 1                    0x13A7B8  opt+116h  atmosphere_on_geometry (fog)
 13  stage_forward, 1 (0x1AAB30)            0x13A7F5  opt+117h  forward_sky
 14  stage_screen_space_reflections         0x13A830  opt+113h  SSR, water
     (0x1AC870)
 15  stage_forward, 0                       0x13A86C  opt+114h  forward: opaque emission
 16  stage_lights, 1                        0x13A8A8  -         forward lighting
 17  stage_rain (0x1AD830)                  0x13A8E3  opt+118h  rain
 18  stage_particles, 0 (0x14BA90)          0x13A91F  opt+11Ah  particles_pre_distortion
 19  stage_apply_distortion (inline,        0x13A986  opt+11Ch  apply_distortion
     execute 0x1AE3B0)
 20  stage_particles, 1                     0x13A9C3  opt+11Ah  particles
 21  stage_volume_fog (0x1AE760)            0x13A9FE  opt+11Bh  volume_fog
 22  stage_foreground (inline,              0x13AA44  -         first-person models
     execute 0x1AF360)
 23  stage_postprocess (0x1AFD40)           0x13AA7E  opt+11Eh  post_process
 24..26  null                               -         -         debug_post_process, debug, shadow
```

Further runtime gates: SSAO/SSDO need `r_ambient_occlusion_quality` (opt+ECh) != 0 and
scene_view+284h (0x17D8BC); SSR needs opt+134h and `r_lighting_quality` (opt+D8h) >= 2
(`options::use_ssr` 0x3CD460); SSDO needs `r_ambient_occlusion_quality` = 3 and `r_ssdo`
(opt+106h) (`options::use_ssdo` 0x3CD440). PROVEN.

The data flow, one frame (light is linear HDR until the post-process):

```
gbuffer (1) -> normal, albedo, surface_parameters, position (linear depth), depth/stencil
  + decals (2) edit those three targets
  + sun shadow map (0) -> shadow_mask (9) -> sun_shadow_and_scattering
SSAO/SSDO (5) -> ssao_accumulator_full_x, ssdo_result
ambient (6):  accum_diffuse/specular  <- ambient volumes (x), probes (lerp), ambient lights (x / +),
                                         then SSAO (x, ambient diffuse only)
lights (7):   accum_diffuse/specular  += every visible point / spot light
composition (11): generic0 = albedo * (accum_diffuse + sun diffuse) + spec scale * (accum_specular
                  + sun specular)                           geometry pixels only (stencil)
atmosphere (10, 12), forward sky (13), SSR + water (14) -> generic0
forward (15): generic0 += emission                          ONE/ONE
rain (17), particles (18, 20), distortion (19), volume fog (21), first-person (22)
postprocess (23): luminance -> eye adaptation -> bloom -> tone curve, LUT, vignette -> AA
                  -> sharpen + gamma -> present
```

### 1.1 Render targets

`renderer_context::get_rt` (0x11C1D0) reads descriptor `index * A0h + 98h` of the target table at
context+119Ch. Names come from `rt_index_to_name` (0x162550, jump table 0x16272C). Formats from
`create_targets` (0x162DA0); HDR is 26 R11G11B10_FLOAT when the device feature level (+124h) is at
least 10_0 (0xA000), else 10 RGBA16_FLOAT (0x1631BE). PROVEN, and Unicorn-run.

```
index  name                          format
   9   $user$position                R16_FLOAT        linear view depth
  10   $user$normal                  34 R16G16_FLOAT          G-buffer slot 0
  11   $user$normal_copy             (decals read it)
  13   $user$albedo                  28 R8G8B8A8_UNORM        G-buffer slot 1
  15   $user$surface_parameters      28 R8G8B8A8_UNORM        G-buffer slot 2
  20   $user$ssao_accumulator        R8, half resolution
  21   $user$ssao_accumulator_full_x R8, full
  28   $user$ssdo_accumulator        RGBA8, half
  29   $user$ssdo_result             RGBA8, full (t_ssdo_result)
  31   $user$accum_diffuse           HDR
  32   $user$accum_ambient_lights    RGBA8 (ambient lights, cleared to 1)
  33   $user$accum_ambient_lights_specular_mask  R8
  34   $user$accum_specular          HDR
   7   $user$accum_local_lights_specular  HDR   (local lights' specular when SSR is on)
   8   $user$sun_shadow_and_scattering    49 R8G8_UNORM
  39   $user$bloom_combine; 40..51 bloom levels, RGBA16_FLOAT
  52   $user$present                 RGBA8
  55   $user$generic0                HDR   composed scene colour
  56   $user$generic1                HDR   copy of generic0 for water/SSR/emission
  71   $user$mie_scattering          10 RGBA16_FLOAT, 256x128
  72   $user$rayleigh_scattering     10 RGBA16_FLOAT, 256x128
```

`stage_gbuffer::execute` binds 10, 13, 15 to RTV slots 0/1/2 and clears slot 3 (0x177BEE). The
formats of 9, 20..33 and 39..52: 0x163460..0x16373D (PROVEN). The shadow maps are separate
(section 4.3).

## 2. Geometry

### 2.1 Scene membership

The scene's every model is a pointer vector at scene+9B358Ch/+9B3590h: `engine::world::add_model`
(0x118430) appends through 0x14FE60/0x1525E0, `remove_model` (0x118540, through
`scene::remove_model` 0x14FF60) erases, `renderer::render` walks it (0x13D363). The camera's draw
lists are separate; a bake wants this vector, not what is visible. The world functions take
pointers to handles whose first DWORD is the scene / model (ABI in re-kit). PROVEN.

The scene's lights are at scene+9B7670h (section 4).

A static model instance (vtable 0x9F2B24) keeps its world matrix at +14Ch (16 floats, row vectors),
its surface records at +298h (64h bytes each, count byte +294h) for **all** LODs. The original
resource's LOD table (+128h through instance+290h) has three counts and three u8 index lists;
LOD 0 is the most detailed. A record's +38h is the render surface, +3Ch the model owner, +44h its
category flags: bit 0 drawn for the camera, bit 1 cast sun shadows (camera `select_models` mask 1
at 0x1B790D, sun shadow mask 2 at 0x17515E). A static explicit-LOD query tests bit 0 only
(0x16F4D5). A skeleton instance (vtable 0x9F0F24) has its records at +62D4h, count +62D0h. PROVEN.

### 2.2 Vertex layouts

Static (`static_render_surface::load` 0x16EA30; declarations 0x9CBFD0 / 0x9CC060): stride 28, or
32 when surface+48h is 4. u16 indices; vertex count +10h, index count +14h; `res_geometry` at +4
(+4 vertex buffer, +8 index buffer, +0Ch stride; each buffer's ID3D11Buffer at its +8). PROVEN.

```
offset  element      format
   0    POSITION     R32G32B32_FLOAT
  12    NORMAL       R8G8B8A8_UNORM
  16    TANGENT      R8G8B8A8_UNORM
  20    BINORMAL     R8G8B8A8_UNORM
  24    TEXCOORD     R16G16_FLOAT
  28    COLOR        R8G8B8A8_UNORM   (32-byte layout only)
```

The basic `vertex_base` VS (3584 bytes, SHA-256 `9d6621e5...97ce1a`) reads each basis vector as
`.zyx * 2 - 1` (the bytes are z, y, x; the fourth byte unused), keeps tangent and binormal as
authored (no cross product, no per-vertex normalization: lengths survive interpolation), and
transforms them by the world-view matrix with its columns normalized. UV is passed unmodified.
PROVEN (bytecode run on a GPU against a CPU decoder, 135 basis components under rotation,
non-uniform scale, reflection and shear).

Skinned, four bones (`skeleton_mesh_gpu_skinning_4weights::load` 0x1CD400, declaration 0xA7A6D8,
stride 48, u16 indices): PROVEN.

```
offset  element       format
   0    POSITION      R32G32B32_FLOAT
  12    BLENDINDICES  R8G8B8A8_UINT
  16    BLENDWEIGHT   R32G32B32_FLOAT   fourth weight = 1 - x - y - z, not clamped
  28    NORMAL        R8G8B8A8_UNORM
  32    TANGENT       R8G8B8A8_UNORM
  36    BINORMAL      R8G8B8A8_UNORM
  40    TEXCOORD      R32G32_FLOAT
```

Its VS (`vertex_base` type 3, SHA-256 `c80f0404...9eb10c`, 80 matrix slots) normalizes each
blended T/B/N before the world-view transform, unlike the static VS. It writes T/B/N to output
registers 0..2 (the static VS uses 1..3), so the static G-buffer pixel shaders do not pair with it.
The palettes: `update_render_matrices` (0x15A590) keeps current, previous and alternate vectors at
instance +229Ch, +290h, +42A8h, each entry `transpose(inverse bind * pose)` (inverse bind at
original+140h); `set_constants` (0x15A520) uploads current and previous, or only the alternate.
A cached palette is the last one computed, not necessarily this frame's: `mt_draw` can skip the
bone update for a culled or item-less player (0x24754F..0x247592). PROVEN.

Terrain draws with the static `vertex_base` VS and layout; only its pixel shader differs
(section 3.4). LIKELY (one basic permutation checked).

## 3. Materials

### 3.1 Finding a surface's shaders and constants

`render_surface::get_material_effects` (0x158870, surface in EAX): the material instance at
surface+1Ch and the variant key surface+48h select a record (+21Ch array, +220h count, 94h bytes
each, key at +24h; an unknown key gives the first) through 0x16B070. With no instance, or the
default-material switch (byte 0x1F1D991) on, the key maps through 0x34D7D0 into the table at
0x1E1B7BC. The material effects' G-buffer effect is +2Ch; its techniques are a pointer vector at
effect +5624h/+5628h, a technique's passes at +8/+0Ch, a pass's VS +8, GS +0Ch, PS +10h;
`res_effect::apply_pass` always takes the first pass (0x3D724E). The G-buffer draw uses technique
0 (plus a quality bit, 0x1774B9); the depth prepass technique 9. PROVEN.

A pixel shader's constants are a table at PS+8: 18h-byte records (+0 class, float4 = 10h; +2
array count; +8 source pointer; +0Ch size; +10h the shared host, its name at host+20h, scalar
type at +30h). Read a material's value from the record's own source (+8), never from the host,
which holds whatever was drawn last. Its textures are bound by name: the hardware shader's
reflected descriptors at hardware+8F8h (54h bytes: name inline at +0Ch, bind point +4Ch) index the
material's texture list at PS+0Ch (pointers at +4); a `res_texture` has its ID3D11Resource at
+1ACh and its SRV at +1B4h. The shader family name is a pooled string at hardware+5D10h (text at
+10h). Unused reflected constants get no record (USED flag, 0x3D7A68). PROVEN.

### 3.2 G-buffer parameters (`gbuffer_pass.ps`)

Read in six shipped permutations (constant, textured, alpha-tested, roughness/Fresnel maps), and
for skinned materials in seven more run on a GPU. PROVEN for those permutations; the database
holds 5918 `gbuffer_pass.ps` blobs in 886 instruction variants.

| Input | Meaning |
|---|---|
| `solid_color_specular` (float4, offset 0) | linear multiplier of the base texture RGB; no extra 2.2 (the later source has one) |
| `specular_fresnel_roughness_parameters` (float4) | x, y: Fresnel offset and scale; z, w: roughness offset and scale |
| `t_roughness` | roughness = z + w * texture.**g** |
| `t_fresnel` | Fresnel = x + y * texture.**g** (green, not red) |
| `smoothness_multiplier` (offset 256) | written smoothness = (1 - roughness) * multiplier |
| `alpha_ref_parameter` (offset 248) | discard when base alpha is below it (alpha-tested variants) |
| `specular_color_parameter.x` (offset 64) | the specular scale, written to surface_parameters.w |
| `t_normal` | x = 2a - 1, y = 2g - 1, z = sqrt(max(1 - x^2 - y^2, 0)), normal (x, -y, z) normalized; only N is flipped on back faces |

Fresnel and roughness are separate parameters; there is no metallic. surface_parameters is
(Fresnel, smoothness, ?, specular scale). The `.z` channel was not read. GUESS: unused by these
variants.

**The specular scale.** The composition shader (`native-composition-small`, SHA-256
`d118f01f...a1f430`, rain off) is

```
diffuse = pow((abs(t_diffuse.rgb) + 0.055) / 1.055, 2.4)
out.rgb = diffuse * max(t_diffuse_accumulation.rgb, 0)  * debug_multipliers.x
        + t_parameters.w * max(t_specular_accumulation.rgb, 0) * debug_multipliers.y
out.a   = 0
```

with `t_diffuse` the albedo target and `t_parameters` the surface parameters (or their copy,
target 12 `$user$parameters_copy`; LIKELY), so the fourth channel scales all accumulated specular
light. An earlier reading called it scattering; that was wrong. Albedo is stored gamma-encoded and
decoded here. PROVEN (36 GPU cases of this permutation; the rain branch not checked).

**Wetness.** At draw time `stage_gbuffer::render_models` overrides the material's
`smoothness_multiplier` (stage host +40h) with scene_view+2DCh, or with 1.0 when render option
[0x2260498]+E8h is nonzero (0x177575..0x1775B7). The compiled value is not what is drawn. PROVEN for
the code; LIKELY that +2DCh is the weather's wetness.

The G-buffer compiler (`effect_gstage_default_materials::compile` 0x1DDD50) also binds
`uv_scrolling_parameters0/1` (0x1E10E1, 0x1E10F0): animated UVs exist; their use was not read.

### 3.3 Emission (`gbuffer_emissive_pass.ps`)

Drawn by forward mode 0 (slot 15) with **technique 6** of the G-buffer effect, only when the
material-effects byte +0 is set (0x1AB6C1..0x1AB6E8). Its blend is RGB ONE/ONE ADD, alpha ONE/ZERO
(set at 0x1E11D4, `set_alpha_blend` 0x3E2EA0), so it adds to the composed colour. PROVEN.

Three shipped programs, run on a GPU (60 cases): PROVEN.

- dark: writes 0;
- constant: `solid_emission_color.rgb` (the compiler multiplied it by the configured intensity,
  0x1E0D1E..0x1E0D71; no gamma);
- textured: that colour times `t_emission.rgb` at the vertex UV; texture alpha ignored.

Flicker: `emissive_flickering_parameters = (frequency, amplitude, enabled, unused)`, default
(0, 1, 0, 0) (0x1E10FF..0x1E1123) unless `use_emissive_flickering`. When enabled > 0.5 the
colour is multiplied by `(1 + 0.5 * sin(frequency * scene_time * 2 * pi)) * amplitude`, else by 1.
`scene_time` is context+4140h (section 1). For a static bake, take the factor's mean (amplitude)
or its value at a chosen time. Output alpha is 0.

### 3.4 Terrain (`terrain_gbuffer_pass.ps`)

One basic static permutation (SHA-256 `69e4aa23...1b93f2`), run on a GPU (48 draws, all three
targets): seven textures and two constants, nothing else.

- `texture_mask`; `texture_diffuse_0..3`; `texture_detail_0`, `texture_detail_1`;
  `constant_tiles` (float4, one tiling per layer), `constant_detail_tiles` (x, y used).
- A layer counts when its raw mask value is above 0.001; weights are the mask divided by the sum
  of all four channels.
- colour = 0.5 * weighted diffuse + 0.5 * detail0 + 0.00001 * detail1.
- surface_parameters = (0.04, (1 - colour.g) * 1.4, 0,
  2 - pow(1.5 * pow((abs(colour.g) + 0.055) / 1.055, 2.4), 1.9)), before the UNORM target clamps.

So terrain's smoothness and specular scale come per texel from its colour's green. PROVEN for
this permutation; normal-mapped and parallax terrain, and a zero mask sum, were not read.
Emission is separate from these seven layers: a terrain material's effect has the same technique-6
emission pass and material-effects flag as any other. LIKELY (read from the material graph, not
drawn).

### 3.5 G-buffer encoding

```
rt 10 normal              R16G16_FLOAT  view-space normal, spheremap encode
rt 13 albedo              RGBA8_UNORM   srgb_encode(colour) (3.2), a = 1
rt 15 surface_parameters  RGBA8_UNORM   x Fresnel, y smoothness, z translucency, w spec scale
rt 9  position            R16_FLOAT     linear view depth; P = depth * (eye ray)
```

Encode (gbuffer_pass.ps, e.g. ce740883, 3368 bytes): n normalized view space,
enc = n.xy / sqrt(8 - 8 n.z) + 0.5. Decode (probe, composition, ambient shaders):
e = enc*4 - 2, f = dot(e,e), n = (e * sqrt(1 - f/4), f/2 - 1) (exact inverse; singular at
n.z = +1). Position is rebuilt as t_position.x * (s_eye_ray_corner.xy * (1 - 2uv) / .z, 1) and
taken to world space with m_V2W. surface_parameters.z multiplies the probe back-face irradiance
(N reversed) and the composition translucency term, so it is a translucency / back-light
weight (writes 0 in the plain variants). PROVEN (bytecode; formats 0x1632C5..0x16356A).
Stencil: geometry pixels have a non-zero stencil (composition, apply_ambient_lights and decals
test != 0 / a mask); bit 40h is the scratch bit the light-volume masks invert. LIKELY (states
read, the G-buffer's stencil write value not read). Depth-buffer format: not read.

## 4. Lights

### 4.1 The sun

The current environment is inline at scene_view+118h: sun azimuth +40h and altitude +44h
(degrees), intensity +48h, linear RGB +4Ch. `environment_sun_direction` (0x34E020, environment in
EAX, output in ESI) returns the normalized direction the light travels:

```
d = (cos(az) * cos(alt), -sin(alt), sin(az) * cos(alt))      toward the sun = -d
```

`stage_lights::render_model_sun_lighting` (0x1918C0) uploads `sun_color` = RGB * intensity, no
gamma, no clamp (0x191FE3..0x192063), and places the light at `d * -1000` (0x19191F..0x1919DF).
PROVEN (instructions read; the angle conversion was Unicorn-run in the attempt). The later source
differs.

### 4.2 Point and spot lights

Every light of the scene, visible or not, is in the database at scene+9B7670h: (light pointer,
id) pairs of 8 bytes, sorted by id (`lights_db::add_light` 0x1BDE20). Authored lights
(`engine::world::add_light` 0x1189C0) and particle lights (0x14F200) both go there. A type-4
light (the sun) is not stored; its id goes to +2010h. PROVEN, Unicorn-run.

Light record (written by `fill_light` 0x1BD7F0 and `set_color_and_intensity` 0x1C9E40, read by
`render_light` 0x1949B0): PROVEN.

```
+204h  colour RGB, linear (packed bytes / 255, then pow 2.2)
+210h  intensity (apart from the colour)
+214h  position
+220h  umbra (full cone angle, radians)
+224h  direction
+23Ch  penumbra (full cone angle, radians)
+24Ch  spot falloff power
+25Ch  attenuation power
+260h  range
+264h  shadow transparency (from props +84h)
+2A9h  enabled (0: not drawn, 0x193504; +2A8h is a separate refresh flag)
+33Ch, +340h  diffuse / specular influence factors (unused by every shipped light shader)
+35Ch  flags: low nibble type (0 point, 1 spot); bit 5 casts shadows
+360h..+365h  point light faces +X, -X, +Y, -Y, +Z, -Z that may cast shadows
```

Radiance = colour * intensity. The shipped unshadowed point and spot shaders (point SHA-256
`b4264c8e...e73b827`, spot `d6bf8cd8...0097fc`):

```
attenuation = pow(max(1 - min(dist^2 / R^2, 1), 0.001), attenuation_power)
R = range                          (point)
R = range / sin(penumbra / 2)      (spot)
spot cone, c = cos of the angle to the axis:
  c >= cos(umbra / 2)                  -> 1
  c <  cos(penumbra / 2)               -> 0
  else pow((c - cos(penumbra/2)) / max(cos(umbra/2) - cos(penumbra/2), 0.0001), falloff)
```

The drawn volume is a sphere of radius range * 1.05 for a point light (0x1CB4BE) and a cone of
axial length range for a spot (`light::xform_calc` 0x1CAFB0). PROVEN (disassembly; the attempt's
renderer matched these curves numerically, not against the original pixel shader on a GPU).

Shadows: a light casts them when flags bit 5 is set and, for a point light, one of its face bytes
(`light::is_cast_shadows` 0x1C9DC0; a quality option can turn them off first: +271h, +26Ch,
[0x2260498]+9Ch). The shadowed shaders compute `saturate(visibility + shadow_transparency)`; the
filter is in [lobby-packs-046-069.md](lobby-packs-046-069.md), "What softens a shadow in 0.26g".
All 32 point/spot pixel shaders ignore `lighting_model`, `is_shadower` and the two influence
factors. PROVEN.

### 4.3 How the local lights are drawn

`stage_lights::execute` (0x193360), slot 7, PROVEN:

- Targets (0x193420..0x193484): RTV 0 `accum_diffuse` (31); RTV 1 `accum_local_lights_specular` (7)
  when `options::use_ssr` (0x3CD460) is on, else `accum_specular` (34). With SSR the local lights'
  specular is kept apart from the sun's and the probes'.
- The lights drawn are the scene view's visible-light vector (scene_view+14CDCh, filled by
  `stage_visibility`), in that vector's order. A light is skipped when it is disabled (+2A9h), or
  when the hi-Z occlusion option ([0x2260498]+12Dh) is on and the light is occluded (+270h) and
  not waiting for a refresh (+2A8h, cleared after the draw). Everything else goes to
  `render_light` (0x1949B0). There is no light count limit in this loop.
- Lights add up in the targets (one draw per light, or per face, blended ONE/ONE: OBSERVED in the
  capture, section 11).

**Volumes.** As in 0.10b (`stage_lights.cpp`, `render_light`): each light first marks its screen
area with a stencil-only draw of its volume (technique "mask local light"), then draws the
accumulator twice (techniques 0 and 1, the two stencil cases). Volume geometry: a sphere for an
unshadowed point (radius range * 1.05), a pyramid for a spot and for each face of a shadowed
point. LIKELY for 0.26g (the 0.10b code, the same geometry constructors 0x18EA20 / 0x18EB20 /
0x18ED30 in 0.26g); the draw sequence is OBSERVED in section 11.

**Point lights with shadows** (`render_shadowed_light`, 0x193630): six faces, each a 90 degree
pyramid. For a face whose byte (+360h..+365h) says it casts, the face's shadow map is rendered
(`render_to_hw_shadowmap` 0x190980) and the face is lit with the shadowed shader at once; a face
that does not cast is lit with the unshadowed shader. The face projection is
`create_perspective_projection(pi/2, 1, 0.025, range)` (0xEEE30; fov 0x9DA944 = 1.5708, near
0xA7A7A0 = 0.025): PROVEN. 0.26g adds a frustum test before the map is rendered (0x193AFA..0x193C3C;
a face whose test returns 2 skips its map): LIKELY a face outside the view.

**Spot lights**: one map, `create_perspective_projection(cone angle, 1, range/1000, range)` in
0.10b (`make_spot_light_shadowmap`). LIKELY unchanged.

**Shadow maps** (`stage_lights` constructor, 0x18EF86..0x18F0CE): five shared depth targets,
format 53 `R16_TYPELESS` (a D16 depth buffer), 1024, 512, 256, 128 and 64 texels square
(`$user$shadow_map_size_1024` .. `_64`), created whatever the options. One map per size, reused:
every light (and every point face) re-renders its map just before its lighting draw, so nothing is
kept between lights or frames (unless a light has `static_shadows`). `shadow_map_size_index`
picks the size. PROVEN.

**Samplers** (`register_samplers` 0x157730): `s_shmap_local` = filter 95h
`COMPARISON_MIN_MAG_MIP_LINEAR` (a bilinear 2x2 compare per tap), address CLAMP; `s_shmap`
(the sun's) = 94h `COMPARISON_MIN_MAG_LINEAR_MIP_POINT`. PROVEN for the filters (0x157DD1, 0x157CD3),
LIKELY for the address mode (EDI = 3 set at 0x15786D); the capture confirms (section 11).

### 4.4 The light shaders (`point_light_accumulator.ps`, `spot_light_accumulator.ps`)

16 permutations each, `____A_B___________________C`: **A** = the shadow quality
(`r_shadow_quality` 0..3), **B** = SSDO (`options::use_ssdo`: `r_ambient_occlusion_quality` 3 and `r_ssdo`), **C** = shadowed. A only matters when C = 1. PROVEN
(32 blobs disassembled, and recompiled from source, section 12).

Inputs: `$Globals` (cb0) `light_color`, `light_intensity`, `light_diffuse_influence_factor`,
`light_specular_influence_factor`, `lighting_model`, `view_to_light_matrix` (world -> light clip),
`shadow_z_bias`, `shadow_transparency`, `shadow_map_size`, `is_shadower`, then for a spot
`light_direction` and the cone cosines, then `s_eye_ray_corner`; `static_globals` (cb1, m_V2W and
screen_res used); `positional_lights_constants` (cb2: `light_position` (view space),
`light_attenuation_power`, `light_range`). Textures t0 `t_normal`, t1 `t_parameters`,
t2 `t_position` (linear view depth in .x), t3 `shadowmap_texture`, t4 `t_ssdo_result`. The two
influence factors, `lighting_model` and `is_shadower` are declared but unused. PROVEN.

Per pixel (PROVEN, from the bytecode; names are ours):

```
tc       = SV_Position.xy * screen_res.zw
P        = depth(tc) * eye_ray(tc)                         view space
N        = decode(t_normal.xy)                              Lambert azimuthal, z negated, clamped
f0, r    = t_parameters.x, t_parameters.y                   Fresnel at 0 deg, roughness
L, V, H  = normalize(light_position - P), normalize(-P), normalize(L + V)
att      = pow(max(1 - min(|light_position - P|^2 / light_range^2, 1), 0.001), light_attenuation_power)
diffuse  = saturate(NdotL * (1 - F(NdotL))) * att           F(x) = f0 + (1 - f0) (1 - x)^5
a        = 0.5 r^6 + 0.1 r;   D = min(a^2 / (pi ((NdotH a^2 - NdotH) NdotH + 1)^2), 1000)
k        = r / 2;  G = saturate(min(1 / (4 max(NdotL NdotV, 1e-6)), 1) * G1(NdotL) G1(NdotV))
           G1(x) = x / (x (1 - k) + k)
specular = NdotL * D * F(VdotH) * G * att
spot:      both * cone(c), c = dot(normalize(P - light_position), light_direction)  (section 4.2)
SSDO (B):  diffuse *= (0.5 + saturate(0.5 - 0.5 (dot(L, s.xyz) + s.w)))^6,  s = t_ssdo.rgba * (2,2,2,1) - (1,1,1,0)
shadow (C): see below; diffuse *= s, specular *= s  (spot: diffuse only where diffuse >= 0)
out0     = light_color * light_intensity * diffuse          accum_diffuse
out1     = light_color * light_intensity * max(specular, 0) accum (local lights) specular
```

So a light's diffuse is GGX-era: Lambert with a Schlick energy split, a GGX distribution with a
remapped roughness, Smith-Schlick visibility. The 0.10b shader (Beckmann, one packed
`sqrt(x/8)` target, `convert_to_linear_space` of the colour in the shader) is a different
generation: 0.26g writes linear light to two targets and the colour arrives linear from the CPU.
SSDO darkens a light's diffuse by the screen-space directional occlusion towards the light (it can
also brighten: the factor runs from 1/64 to about 11). PROVEN.

**The shadow lookup** (C = 1):

```
Pw     = m_V2W * (P + normalize(0.75 N + L) * 24 / shadow_map_size)     normal offset, view->world
p      = view_to_light_matrix * Pw;  p.xyz /= p.w
d      = p.z - shadow_z_bias
if 0 <= d <= 1:
  tc   = p.xy * (0.5, -0.5) + 0.5
  A 0,1: 4 taps at (+-1, +-1) / shadow_map_size       (each SampleCmpLevelZero is a bilinear 2x2)
  A 2,3: 16 taps, the 0.10b Poisson disc / shadow_map_size
  s    = saturate(mean + shadow_transparency)
else s = 1
```

The kernel is one texel wide whatever the light, its distance or its size; the edge's width on a
surface is one shadow-map texel there. Softness exists only as `shadow_map_size_index` (a coarser
texel) and as `shadow_transparency`. The normal offset is 24 / `shadow_map_size` metres
(9 cm for a 256 map, 2.3 cm for 1024): it detaches contact shadows a little. PROVEN.

## 5. Ambient and indirect light (`stage_ambient_lighting::execute` 0x1826F0)

Order in a frame (0x182766..0x1828BA), all after the G-buffer and SSAO, before slot 7 (lights):

```
1 render_skylight            only if stage+E4h != 0 (constructor sets 0, 0x17F58D)
2 ssr_mask (fill, rt 36)     only if use_ssr
3 render_ambient_volumes     multiply accum_diffuse
4 render_environment_probes  lerp into accum_diffuse + accum_specular
5 render_ambient_lights      multiply (intensity <= 1) / add (intensity > 1)
6 apply_ssao_and_height_based_ambient   multiply accum_diffuse
```

So everything the ambient stage writes is in accum_diffuse (rt 31) / accum_specular (rt 34)
before any point/spot light is added. SSAO and height-based ambient multiply only that ambient
diffuse; they do not touch accum_specular and do not touch later light accumulation. PROVEN.
Who clears accum_diffuse in a normal frame was not read (stage_lights::clear_surfaces 0x192A30
clears it only on its not-ready path, to pow(0.125, 0.5) grey).

LPV: slot 8 is null and the composition binds `$user$lpv_accumulation` only as a name
(0x1A9496); no LPV pass runs. Sky AO: `effect_sky_ambient_occlusion` (stage+5Ch) and
`effect_filter_fog_ambient_occlusion` (+DCh) are created but not used by the 0.26g execute.
PROVEN for the execute path; the shaders exist (`sky_ambient_occlusion.ps` 74837bae...c58951).

### 5.1 Skylight (0x182440)

Technique 1 of `effect_skylight` filled over accum_diffuse; `skylight.ps` (7ef4a1f4...423971)
writes the constant `skylight_parameters0` = (m, m, m, 0), m = stage+E0h (1.0 from the
constructor), alpha 1. It is a flat replacement value, not sky lighting; 0.10b computed a
six-direction sky colour here. Normally off (stage+E4h = 0; the writer was not found). PROVEN.

### 5.2 Ambient volumes (0x182220)

Boxes from scene_view+1BD18h; per volume the light-mask pass then `effect_ambient_volume`
(0x1CF650) techniques 0/1 on the unit box: blend RGB ZERO / SRC_COLOR (multiply), stencil as for
probes; `ambient_volume.ps` (8d79c087...d5b7c6) outputs the constant `ambient_volume_multiplier`.
So a volume multiplies accum_diffuse inside the box, hard-edged; drawn before the probes, it is
overwritten wherever a probe has alpha 1. PROVEN.

### 5.3 Environment probes (0x180980)

Data. `object_environment_probe::load` (0x28F3B0) reads `radius`, `diffuse_multiplier`,
`specular_multiplier`, `enabled`, `texture`, `texture_blend_to`, `geometry` (default 0),
`probe_side_width` (default 0.5, 0x927A44), `smart_attenuation` (default 0), `affect_diffuse`
(default 1), `face_average_color_0..5`. `insert` (0x28FFD0) builds the render properties: outer
transform = the object transform, inner transform = the same with each axis length reduced by
side_width (0x2903BA..0x290417), radius, inner radius = radius - side_width. The render probe
(`set_properties` 0x1C36F0 copies properties to probe+4) has: +234h outer transform, +274h inner
transform, +314h location, +320h radius, +324h radius - side_width, +328h/+32Ch diffuse/specular
multiplier, +33Ah smart_attenuation, +33Bh affect_diffuse, +33Ch geometry (0 sphere, 1 box),
+35Ch mip count, +36Ch..+378h cubemaps (specular, diffuse, and their blend_to pair). PROVEN.

Draw, per visible probe (list scene_view+16CF4h, already sorted, (c) below): the light mask
(`effect_light_mask`, back faces, depth LESS_EQUAL, stencil INVERT on depth fail) on the probe
volume, then techniques 0 and 1 (front faces depth-tested / back faces untested, each stencil
EQUAL on bit 40h, INVERT on pass) of one of 16 effects at stage+14h + 4*idx,
idx = 8*geometry + 4*smart_attenuation + 2*blending + affect_diffuse (0x180F38..0x181109), the
class `effect_environment_probe_lighting<geometry, smart_attenuation, blend, affect_diffuse>`
(RTTI). Volume: sphere = du_sphere (540 indices) scaled by probe+324h (radius - side_width) at
the location; box = unit cube with the outer transform +234h (0x180BD2..0x180CD4). Blend for all
16 (0x18A3D5): RGB SRC_ALPHA / INV_SRC_ALPHA, alpha ZERO/ZERO, write mask RGB, both RT0
accum_diffuse and RT1 accum_specular. Constants (0x18137B..0x181609): `light_range` =
0.98 * radius, `inner_radius` = (radius - side_width) - 0.05, `probe_location`, `num_mips`,
`color_parameters` = (diffuse_mult * scene_view+290h, specular_mult * scene_view+294h)
(`environment_probes_diffuse/specular_intensity_multiplier`), `outer_cube_side_sizes` /
`inner_cube_side_sizes` = row lengths of the outer / inner transform, `world_to_outer_probe`,
`world_to_inner_probe`, `probe_index`, `probes_blend_factor` (scene+126154h; blend_to used when
0 < f < 0.999, the blend_to textures alone when f >= 0.999). PROVEN.

Shader `environment_probe_lighting.ps`, 32 permutations; path digits: pos 3 lighting model (0 in
all 16 effects), pos 5 TDIFFUSE = affect_diffuse, pos 26-27 TALPHABLENDED_DIFFUSE = blend,
pos 31 LOD_INDEX = smart_attenuation, pos 41 ENV_PROBE_GEOMETRY_TYPE (macro bits from the
config setter around 0x3D936B; effect configs 0x18A310.. set exactly these bits). Read:

```
sphere alpha  (91f1723c...47c5df)  t = sat((|P - probe_location| - inner_radius)
                                           / (light_range - inner_radius))
                                   a = 1 - t*t*(3 - 2t)
box alpha     (bfb081af...06f6fa)  p = world_to_outer_probe * P   (unit cube, |p| <= 1)
                                   t_i = sat((outer_i*|p_i| - inner_i) / (outer_i - inner_i))
                                   a = prod_i (1 - t_i*t_i*(3 - 2 t_i))
box, smart_attenuation (e7f0b41b...9d27f1): the same falloff, combined with a term from the
                                   normal in probe space (a face-facing test against the
                                   outer/inner box), a^0.75 mix; not modelled further
diffuse, affect_diffuse=1 (aafc071e...021caa):
   rgb = 0.25*(RGBM(cube_diffuse(N)) + RGBM(cube_diffuse(-N)) * surface_parameters.z)
         * color_parameters.x * (1 - surface_parameters.x)
         RGBM(c) = (c*c).rgb / (c*c).a  (cubemap texels squared, then divided by alpha)
diffuse, affect_diffuse=0 (91f1723c): rgb = t_diffuse = $user$albedo (the gamma-encoded albedo
         itself, bound by the compiler 0x18A4AA), so the probe pulls accum_diffuse toward albedo
specular: R = reflect(V, N); mip = round((num_mips - 1) * sqrt(sat(N.V)) * smoothness?)
         rgb = 0.25*RGBM(cube(R, mip)) * (F0' * brdf.x^2 + brdf.y^2) * color_parameters.y * SSAO
         with F0' = surface_parameters.x - 0.04, brdf = t_probe_brdf (engine/env_brdf)
         at (N.V, smoothness)
out0.a = out1.a = a
```

N is the G-buffer normal (section 3.5) turned to world space with `m_V2W`. SSAO multiplies only the
probe specular inside this shader. PROVEN (bytecode; the specular mip expression read as
`sqrt(sat(dot)) * params.y * (num_mips-1)`, rounded).

**(a) What sphere probes need.** The code path is complete for geometry 0: sphere mask and volume,
sphere shaders, a sphere collision object of the full radius for culling (0x1C3A36..0x1C3AF7).
But the drawn sphere has radius `radius - probe_side_width` (side width defaults to 0.5), while
the shader fades from `radius - side_width - 0.05` to `0.98 * radius`: everything past the drawn
sphere is clipped, so a sphere probe is lit fully inside `radius - side_width - 0.05` and ends
within 5 cm of the mesh, effectively hard-edged; with radius <= side_width it draws nothing, and
for 0.02*radius > side_width + 0.05 (radius > ~27.5 m at the default) the fade range turns
negative. A sphere probe also needs `texture` (and for diffuse `affect_diffuse` and a diffuse
cubemap). PROVEN for the arithmetic; why the STK2 sphere probe "did nothing" was not reproduced
(GUESS: radius at or below the side width, or a box probe of the same or larger radius drawn
over it, (c)).

**(b) Box probe edges.** A box probe is NOT hard-edged: it fades with a smoothstep per axis from the
inner box (half-extent - probe_side_width) to the outer box face, product over the three axes;
the default band is 0.5 m. side_width 0 makes outer = inner (division by zero, hard edge).
PROVEN. The hard-edged boxes seen in the lobbies are ambient LIGHTS (5.4): their default
side_width is 0.1 m.

**(c) Overlap and order.** `stage_visibility::execute_stage` sorts the visible probes
(0x1B6B54/0x1B6B63, STLport introsort + final insertion sort) with `sort_by_size_predicate`
`a->+320h > b->+320h` (0x1BA0C7): largest authored `radius` first. +320h is the `radius`
property only; a box's volume comes from its transform, so for a box the radius is only this sort
key. Each probe then lerps over the result so far with its alpha (SRC_ALPHA / INV_SRC_ALPHA, for
diffuse and specular alike): the last drawn (smallest radius) replaces where its alpha is 1,
blends in its fade band, nothing adds. Ties: STLport sorts ranges of up to 16 elements
(`cmp eax,40h` = 16 pointers, 0x1B93A1) by insertion sort only, which is stable, so equal radii
keep the order of the visible list built by the culling query
(`get_results_and_prepare_bounds_env_probes` 0x1B8190); above 16 visible probes introsort makes
ties unstable. PROVEN for the sort and blend; that the culling order changes between runs is the
lobby observation (LIKELY: query results from the spatial tree / asynchronous occlusion).
Differs from 0.10b: 0.10b sorted inside the ambient stage; same predicate and same lerp blend.

### 5.4 Ambient lights (0x181EB0, per-list body 0x181660)

Properties (`object_ambient_light::load` 0x2907A0): `radius`, `enabled`, `color` (packed,
linearized with pow 2.2 at insert 0x2909C0), `intensity` (clamped >= 0), `attenuation_power`,
`dot_normal`, `affect_specular` (default 0), `geometry` (0 sphere, 1 box), `side_width`
(default 0.1, 0x921948), `smart_attenuation`. Render light: +44h location, +50h colour, +5Ch
intensity, +60h attenuation power, +64h radius, +68h side width, +6Ch geometry, +71h dot_normal,
+72h smart_attenuation, +73h affect_specular. PROVEN.

The visible list (scene_view+1CD24h) is split by intensity (0x1858E0): lights with intensity <= 1
form list A, > 1 list B. Effects: 24 at stage+64h, `effect_ambient_light<n, affect_specular,
geometry, mode>` with n = 0 plain, 1 dot_normal, 2 smart_attenuation (idx at 0x1819B1..0x181A12).

```
list A (mode 0): RT0 accum_ambient_lights (rt 32, RGBA8, cleared to 1),
                 RT1 accum_ambient_lights_specular_mask (rt 33, R8, cleared to 0)
                 blend SRC_ALPHA / INV_SRC_ALPHA (0x1859F5): lerp toward colour*intensity
                 then apply_ambient_lights (0x182081, fill, blend ZERO / SRC_COLOR = multiply,
                 stencil != 0):
                   accum_diffuse  *= A
                   accum_specular *= 1 + mask * (A - 1)   (mask = 1 only with affect_specular)
list B (mode 1): RT0 accum_diffuse only, blend SRC_ALPHA / ONE (0x187EBC): adds
                   colour * (intensity - 1) * alpha
```

Shapes (`ambient_light.ps`): sphere alpha = pow(1 - min(d^2 / (0.98 R)^2, 1), power)
(ea3c3173...0cd002); dot_normal multiplies by max(N.L, 0) (2f6e4518...349527); smart
attenuation mixes 0.5*(1 - wrapped N.L) (60e479ff...fc8852). Box (5441d195...551476):
p = view_to_local * P, per axis t_i = sat((min(|p_i|,1) - (1 - s_i)) / s_i) with
s_i = `local_side_width` = side_width / half-extent_i (0x181D07..0x181D3D), alpha =
1 - sat(1 - prod_i(1 - smoothstep(t_i))) = prod_i(...). So a box ambient light fades over
side_width metres inside each face: 0.1 m by default, which reads as a hard edge; raise
side_width for a soft edge. Power is not used by boxes. PROVEN.
An ambient light with intensity <= 1 therefore darkens / tints (multiplies) the ambient computed
so far (probes, volumes), one with intensity > 1 adds light; neither touches point/spot/sun light.

### 5.5 SSAO and height-based ambient applied (0x182530)

`effect_apply_ssao` technique (scene_view+298h != 0 selects HBA), fill over accum_diffuse only,
blend ZERO / SRC_COLOR (multiply). `apply_ssao_and_hba.ps` technique 0 (c6ca3565...929c88):
out = ssao_accumulator_full_x.r; technique 1 (d0e93b6b...865015): out = ssao *
lerp(hba_low_color, hba_high_color, pow(sat((worldY - p.x)/(p.y - p.x)), p.z)), with
hba_parameters = scene_view+29Ch.., low = +2ACh.., high = +2BCh... PROVEN.

### 5.6 SSAO and SSDO (`stage_ambient_occlusion::execute` 0x17D880)

Runs when the stage is enabled, opt+ECh `r_ambient_occlusion_quality` != 0 and scene_view+284h.
Otherwise `clear_surfaces` (0x17D530) clears ssao_accumulator_full_x (rt 21) with red = 1 (so
the SSAO multiplies are neutral), ssao_accumulator (rt 20) and ssdo_result (rt 29); the exact
ssdo_result clear colour (two clears at 0x17D78C and 0x17D846) was not resolved. Passes:

```
downsample depth+normals    effect +18h -> rt 1 frame_position_downsampled (RGBA16F, half)
                            renderer::downsample position -> rt 0 frame_depth_downsampled (R16F)
SSDO (use_ssdo 0x3CD440:    ssdo.ps (25c645d7...e0db4b) -> rt 28 ssdo_accumulator (RGBA8, half)
 quality == 3 && r_ssdo)
SSAO                        technique 1 ssao_hemisphere_stable_radius.ps (d929aea7...73b16c) when
                            quality >= 2, else ssao_hemisphere_optimized.ps (4028bc60...251397)
                            -> rt 20 ssao_accumulator (R8, half)
filter (r_ssao_use_filtering opt+12Eh; temporal with r_ssao_use_temporal_filtering opt+12Fh):
  temporal_mask -> rt 22
  ssao_filter_upsample_temporal.ps (ca308f72...eabf0c, SSDO variant):
      RT0 rt 21 ssao_accumulator_full_x (R8, full), RT1 rt 24 counts, RT2 rt 26 z,
      RT3 rt 29 ssdo_result (RGBA8, full)
  or ssao_filter_upsample.ps (e60b078c...b00fc6): RT0 rt 21, RT1 rt 29
copies for the next frame: 21->23, 24->25, 26->27, (SSDO) 29->30
```

PROVEN (targets 0x17DB99..0x17DD88, formats 0x163460..0x16373D). SSAO multiplies: the ambient
diffuse (5.5) and the probe specular (5.3); nothing else. `t_ssdo_result` (rt 29, full resolution
RGBA8, a direction in rgb (x2-1) and occlusion in a) is read by the composition sun term (6.1)
and by the point and spot light shaders (4.4). Only with `r_ambient_occlusion_quality` = 3 and
`r_ssdo`. Both options are user graphics settings (`st_options_ssao`, `st_options_ssdo` strings).

## 6. Composition and emission

### 6.1 Composition (`stage_composition::execute` 0x1A8170)

Effect = one of 8 at stage+10h + 4*(idx-4), idx = ssdo + 2*ssr + 4*sun + 4 (0x1A8548..0x1A85B0),
`effect_composition<sun, ssr, ssdo>`; sun = environment (scene_view+118h) byte +3Ch, ssr =
use_ssr, ssdo = use_ssdo. Target RT0 generic0 (rt 55), RT1 generic1 when SSR (0x1A870B), no
blend, stencil != 0 (geometry pixels only, the sky dome of slot 10 survives). Light constants:
`light_direction` = sun direction in view space, `light_color` = environment colour * intensity
(+4Ch, +48h) (0x1A81ED..0x1A82B7). PROVEN.

Shader `composition.ps`, digits: pos 5 = sun (TDIFFUSE bit), pos 6 = SSR (second output),
pos 13 = SSDO. The sun=0 variant is section 3.2's `d118f01f...a1f430`. The sun=1 variants
(e.g. 9ff86ad6...be337a, with SSDO 1f97890d...3f38a1) compute the deferred SUN here:

```
albedo = srgb_decode(t_diffuse)
if sun_shadow_and_scattering.x > 0:            (shadow from slot 9)
   sun_diff = light_color * shadow * (1 - F_schlick(N.L))  * ssdo_factor
   sun_spec = light_color * shadow * GGX-like(N, L, V, rough = params.y * roughness_multiplier)
if params.z > 0.001: translucency = params.z * light_color * (max(N.L,0)*shadow.y + 0.02)
                                    + an up-facing term (eye_direction.y)
out.rgb = albedo * debug.x * (sun_diff + translucency + max(accum_diffuse, 0))
        + params.w * debug.y * (sun_spec + max(accum_specular, 0))
(rain: albedo darkened by a rain-shadow-map term when use_rain > 0.5)
out.a = 0
```

ssdo_factor = (1.5 - 0.5*sat(dot(-L, ssdo.xyz*2-1)*0.5+0.5 ...))^6-style term from t_ssdo_result
(multiplies only the sun diffuse). PROVEN (bytecode); which sun variant a normal frame uses
depends on the environment flag (LIKELY on for outdoor maps). So the sun is lit here,
not in `stage_lights`.

### 6.2 Emission

Forward mode 0 (slot 15, `stage_forward::execute` 0x1AB900, enabled by opt+114h) first copies
generic0 into generic1 (0x1ABA64..0x1ABAAD), then binds RT0 = generic0 (rt 55) only
(0x1ABB54) with the scene depth buffer, and draws opaque models with G-buffer technique 6.
The emissive pass state (0x1E0C83..0x1E0C97): depth test on, LESS_EQUAL, no depth write;
blend RGB ONE / ONE ADD (0x1E11D4). generic0 at that point holds the composition (6.1), the sky
dome (slot 10), atmosphere-on-geometry (slot 12) and SSR/water (slot 14). Hence

```
final_pre_post = albedo_lin * (ambient + local lights + sun_diffuse) + params.w * specular
                 (then atmosphere-on-geometry lerp, slot 12)
               + emission                                   (slot 15, added)
```

and a surface with non-black albedo AND emission gets both its live lighting and the emission.
PROVEN. After emission: atmosphere-on-geometry has already run (slot 12, technique 5
`atmospheric_scattering_on_geometry_temporary` 39af78c0...fbd68a, lerp SRC_ALPHA /
INV_SRC_ALPHA toward the in-scattered colour, 0x1D29BA), so distance fog does NOT attenuate
emission; SSR ran before (slot 14), so emission is not in local reflections. What does act on
emission afterwards: rain (17), particles (18, 20), apply_distortion (19: re-samples generic0
through `t_distortion`, 9f9e44c1...07e472), volume fog (21: `volume_fog.ps` beb18918...70138e,
lerp SRC_ALPHA / INV_SRC_ALPHA toward the fog colour, 0x1D44EA, local fog volumes only), and
the post-process (section 10: bloom, eye adaptation, tone curve, LUT). PROVEN. Nothing multiplies emission
by albedo or by light.

## 7. Water

A material whose effect id is exactly `forward_default_simpe_water` gets material-effects byte +7
(0x136B7C..0x136B8C); only such surfaces take part in the SSR stage's local reflections
(`reflection_surface_rejected` 0x1ACB30 also needs an effect at +64h). The water compiler
(0x1E5FE0) uses `forward_water` for technique 0 and `forward_water_local_reflections` for
technique 1; **both have blending off** (0x1E63A9..0x1E645B), neither writes depth, technique 0 is
depth-tested and 1 is not. The later source blends. Before accumulating, the stage copies generic0
into generic1 (0x1AD534..0x1AD58F). The water PS (8040 bytes, SHA-256 `d3a91439...cd956e1`) reads
`t_frame_color` (t3, that copy), `t_foam` (t5), `t_diffuse_lighting` (t6), the local reflection
(t7), a cube-map probe and the atmosphere; its output replaces the pixel. PROVEN (code
Unicorn-run, the PS disassembled only). For a bake: water is drawn over the lit scene behind it,
not lit like the opaque materials.

## 8. Sky and atmosphere

**Scattering tables.** Targets 71 (Mie) and 72 (Rayleigh), 256x128 RGBA16_FLOAT, sampled with
`s_base`: linear, wrap. The atmosphere stage (`stage_atmosphere::execute` 0x1A4200) does nothing
when render option [0x2260498]+115h is off. In mode 0 (slot 10, before composition) it regenerates
the tables (technique 0, `fill_surfaces` 0x1A3690 into 72 then 71) when renderer bytes +18h/+19h
are set or the targets' generation (+2F88h) differs from scene_view+1DE38h, and stores the
generation (0x1A42E8..0x1A44EF); then, dirty or not, it draws the dome over generic0
(0x1A44F3..0x1A468B: technique 1; stage host +6Ch = toward-sun xyz, w zeroed when environment
byte +C8h is 0; host +74h = (environment+100h, environment+E4h, 0, 0)). Mode 1 (slot 12) jumps
to 0x1A50C9 at once; that path was not read. PROVEN, Unicorn-run (32 update paths, the constant
setup). The table contents were not generated or read.

**The dome.** `sky_dome_geometry` (constructor 0x1D1C60, draw 0x1D2150, drawn at 0x1A468B):
256 vertices (float4 position, float2 UV, stride 24), 1260 u16 indices, radius 10; the VS ignores
w and scales xyz by 100000. Two halves of 16 rings x 8 azimuth steps: polar i/15 * 100 degrees from
+Y (so it ends 10 degrees below the horizon), azimuth j/7 * pi in the first half and
2pi - j/7 * pi in the second, UV (i/16 + 1/32, j/8 + 1/16) in both. The halves mirror each other's
UVs, so the tables are not an equirectangular map: look up a direction by intersecting the dome
and interpolating its UVs. PROVEN (constructor Unicorn-run unchanged).

**The atmosphere PS**, as reproduced by the attempt and matched against the shipped VS/PS on a GPU
(two eyes, 40 directions, three suns, three phase powers): LIKELY.

```
c     = max(-dot(to_sun, view_dir), -0.9999)      view_dir from the eye to the dome
phase = 1 + c^2
mie_p = 0.0050041387 * phase / (1.990025 + 1.99 * c) ^ power
m     = Mie(uv);  r = Rayleigh(clamp(u, 0, 0.87), v)
rgb   = (0.75 * phase * abs(r)^1.4 + m + mie_p * sun_colour) * (1 - saturate(m.r))
out   = clamp(rgb, 0, 10)
```

Which frame values feed `power` and `sun_colour` was not pinned down.

**The forward sky.** The shipped `sky_sphere.ps` writes 0 to both outputs (it does not sample a
panorama, unlike the later source). `forward_sky.ps` averages `t_base` at LODs 2, 1 and 0, applies
the sky colour and fog, and multiplies the background (RGB blend ZERO/SRC_COLOR, alpha ONE/ZERO, no
depth, no culling). Its compiler (0x1DB810) raises `sky_color` RGB to 2.2 before `sky_multiplier`;
fog `sky_fog_power` / `sky_fog_up_limit` default to 1 and 0.25 unless both keys are given.
PROVEN (compiler Unicorn-run, 48 cases; shaders run on a GPU, 2160 draws). Clouds and the sun and
moon textures were not read.

## 9. Decals and the other passes

### 9.1 Decals (`stage_decals_accumulate::execute` 0x1789E0)

Copies the normal target (t 10 -> 11 `normal_copy`, 0x178AA5) then binds the three G-buffer
targets normal (10), albedo (13), surface_parameters (15) (0x178AEF..0x178B34) and draws each
decal box (`decal_instance::draw` 0x14DEB0) with `decal_default_material_effect` (0x1E3070,
shader `decal_base`, depth test LESS_EQUAL no write, stencil test against the geometry bits,
0x1E3288..0x1E3394). So decals modify the G-buffer before any lighting; they are lit like the
surface. PROVEN. Blend: `deferred_decal.ps` writes the albedo with the decal alpha in o1.w and
zeros elsewhere (e.g. 00adf4b1...d29172), consistent with per-target SRC_ALPHA blending from the
material `blend_mode` (`effect_material_base::compile_begin` 0x1CD0B0): LIKELY, the per-target
blend values were not read.

### 9.2 Other passes, one line each

- Forward sky (slot 13, forward mode 1, opt+117h): `forward_sky.ps` multiplies the background
  (section 8). Atmosphere slot 10: tables + dome (section 8); slot 12 in 6.2.
- SSR (slot 14): `ssr_*` shaders, also draws water (section 7); local reflections only for
  water materials.
- Rain (slot 17, opt+118h; pre_rain slot 4 modifies normals/wetness before lighting): `rain.ps`,
  `rain_sprite_drops.ps`, `wet_surface*.ps`. Not read further.
- Particles (slots 18 pre-distortion, 20 after): `forward_*` particle families,
  `offscreen_particle_lighting.ps` into `particle_lighting` (rt 57, RGBA16F, quarter res
  LIKELY); lit by probes/ambient via `forward_probe_lighting.ps`. Not read further.
- Distortion: slot 3 accumulates `$user$distortion` (R16G16F) / `distortion_mask` (R8) from
  `distortion_base`/`distortion_panner` materials; slot 19 `apply_distortion.ps` offsets the
  generic0 lookup by distortion * mask. PROVEN (shader).
- Volume fog (slot 21): exponential fog volumes, alpha = (1 - exp(-d * k)) * density, colour
  constant; lerp. PROVEN (shader tail).
- Foreground (slot 22): first-person models via `stage_forward::execute_foreground` and
  `stage_lights::execute_foreground` (vtable slot +8). LIKELY.

## 10. Post-process (`stage_postprocess::execute` 0x1B2270)

Enabled by opt+11Eh and scene_view+1DE3Ch. Order (PIX labels in the code):

```
1 accumulate_motion_vectors (0x1B1B50)  if r_use_motion_vectors_in_taa (opt+133h) and
                                        r_post_process_quality (opt+DCh) != 0
2 compute_luminance_parameters (0x1B1650):
   measure_per_pixel_luminance (0x1B1210): gather_log_luminance.ps (add3c268...b72a1e)
     L = dot(clamp(c,0,1e6), (0.2125, 0.7154, 0.0721)) + 0.001, then 2x2 reductions
     (gather_luminance / gather_exp_luminance: sum, min, max; final * 1/65536)
   compute_per_pixel_eye_adaptated_luminance (0x1B1400): eye_adaptation.ps (137f8461...837fa3)
     L = max(lerp(L_prev, L_cur, adaptation_factor), 0); dt = context+413Ch clamped to
     [0, 1/30]; second constant dt / scene_view+1F0h (LIKELY adaptation_factor = dt / T)
3 bloom ("L:bloom"): gather_bloom.ps (ff13d55d...1597e7) = 4-tap average *
   (bloom_parameters.x * bloom_parameters1.rgb, bloom_parameters.x) into bright_pixels_2x
   (no threshold in this shader); then downsample + blur at 4x..128x (bloom_* rt 40..51,
   RGBA16F); bloom_combine.ps (141eee47...814f08) = 0.5*b0 + 0.3*b1 + 0.2*b2 (variant 1:
   six levels and lens dirt) into bloom_combine (rt 39)
4 process_color_grading_texture (0x1B1F80): color_grading_lut_blending (LUT blend)
5 complex_blend: complex_post_process_blend.ps (744f4b59...7cc399, no DOF):
     c  = lerp(frame, bloom_combine, bloom_parameters.x) + light_scattering_result (god rays)
     L  = max(adapted, 0.001), Lb = max(adapted, 0.05)
     key = 1.03 - 0.1 / (2 + log10(L + 1))
     c *= pow(key / L, 0.2);  c = pow(c, 1.2)
     c  = 1 - exp(-c * 4.5 / (10 * min(Lb, 2) + 1))          (exponential curve)
     blueshift: l = dot(c, (0.3, 0.7, 0.2)); b = l * (0.8, 0.8, 1.4);
                c = b + sat(l * blueshift + 0.21) * (c - b)
     LUT: c = srgb_decode(LUT3D(srgb_encode(c) * 15/16 + 1/32))
     vignette if image_grain.w > 0.1: c *= min(pow(1 - |uv - 0.5|^2, w + 1), 1)
   DOF / bokeh variants (39254d25, a62ce2b4) when the scene uses them
6 AA: r_antialiasing_method (opt+F0h) selects FXAA (post_process_fxaa, "L:fxaa"), MLAA,
   SRAA or temporal AA (effects at stage+B8h..+E4h); r_fxaa_quality_* tune FXAA
7 sharpen ("L:sharpen", post_process_post_sharpen.ps f6e6e71d...8a59a0):
     c = 0.98*c + 0.02*(9c - 8 neighbours); then sRGB-style encode
     pow(c, 1/(2.4 * r_gamma_correction_factor)) * 1.055 - 0.055   (gamma here)
8 copy_rewrite -> present (rt 52, RGBA8); present.ps is a plain copy
```

PROVEN for the shaders quoted and the label order; which branches run (AA method, sharpen flag
at 0x1B2D20, motion blur `r_motion_blur_quality` opt+E4h, lens flares, radial blur,
`effect_aberration_and_sharpen<a,b,c>`) depends on the user options and was not traced branch by
branch. There is no separate gamma pass: the encode is in the sharpen / copy shaders.

## 11. The captured frame

Pending: the RenderDoc capture (one lobby frame, DXVK under Wine, `frame.rdc`) is taken; its
action list, targets and the PCSS before|after pairs are not yet folded in. OBSERVED claims
elsewhere in this note wait on it.

## 12. Compiling replacement shaders

**The compiler.** Every 0.26g blob's RDEF names its creator,
`Microsoft (R) HLSL Shader Compiler 9.29.952.3111`: the June 2010 DirectX SDK's fxc /
D3DCompiler_43, the same as 0.10b. The vostok repository's flake builds it from Microsoft's
`DXSDK_Jun10.exe` (`dxsdk-shader-compiler`: `fxc.exe` and the native `D3DCompiler_43.dll`); it runs
under Wine (keep the native DLL beside fxc.exe; Wine's builtin d3dcompiler is a different
compiler). The flags are `/Zpr /O3 /T ps_4_0 /E main`, as vostok's `docs/shaders.md` proved for
all 3,402 shipped 0.10b blobs. The game cannot compile HLSL; it only loads blobs. PROVEN.

**A blob** (`shaders/sm_4_0/<family>.<ps|vs|gs>/<permutation>` in resources.db) is a u32 count,
that many 264-byte records (a 260-byte include path, a u32 mtime), then the DXBC. The include list
of the 0.26g point light shader dates its sources to 2013-12 .. 2015-01 and names
`brdf.h`, `ssdo_common.h` and `shadow_quality.h`, which 0.10b does not have: the 0.10b sources
are not the 0.26g ones, so the 0.26g light shaders were rewritten from their disassembly. A
replacement blob keeps the stock blob's include records and swaps the DXBC; it ships as a file of
a resources patch (`release/build-patch`), like any content. PROVEN (the game loaded such a patch,
section 11).

**The round trip.** `point_light_accumulator.ps` and `spot_light_accumulator.ps`, reconstructed
from the 0.26g blobs (one file each, the permutation's three digits as `/D SHADOW_QUALITY`,
`/D USE_SSDO`, `/D SHADOWED`), compile with that fxc to the shipped DXBC:

```
                          point   spot
byte-identical            12/16   12/16   (container, checksum and all)
one mov placed elsewhere   4/16    4/16   (B = 1 and C = 1: `mov r4.w, 1.0` one slot later;
                                           the same instructions, declarations and RDEF)
```

So the constant layout (cbuffers, offsets, names), the resource and sampler slots, the outputs,
the G-buffer decode, the BRDF and the stock shadow filter of the reconstruction are the shipped
ones. PROVEN. The sources and the scripts are on the branch `research/renderer-026g`
(`srp-026g/shaders/`): `fxc.sh` (fxc under Wine in its own prefix),
`lights.sh roundtrip` (compile every permutation and compare with the shipped DXBC), `lights.sh
build` (compile the PCSS variants into db-ready blobs for `release/build-patch`).

vkd3d-shader 2.0 (`vkd3d-compiler`) reads DXBC and was used for nothing here: fxc's own
`/dumpbin` gives the reflection (cbuffer layouts, `[unused]` members, bindings) the
reconstruction needs, and only fxc reproduces the shipped bytes.

## 13. Soft shadows (PCSS) for the local lights

**Possible without the executable.** Every input a contact-hardening filter needs is already
bound to the shadowed light shaders, so a replacement of the 2 x 8 shadowed blobs is enough:

- the shadow map's depth: `shadowmap_texture` is the D16 map; `Load` reads raw depth with no
  sampler (the blocker search), `SampleCmpLevelZero` with `s_shmap_local` gives bilinear PCF;
- the light's projection: `view_to_light_matrix` is P * V of a D3D perspective projection
  (`create_perspective_projection`, section 4.3), so the shader reads it back:
  `P00 = |row0.xyz|` (cot of the half angle), `Q = dot(row2.xyz, row3.xyz) / |row3.xyz|^2`,
  `Qn = Q row3.w - row2.w`, and a map depth d is at `z = Qn / (Q - d)` metres from the light; the
  receiver's own distance is the projected `w`;
- `shadow_map_size`, `shadow_z_bias`, `shadow_transparency` as before.

What is missing is **a per-light size**: a light has no such field (its config keys, 0xA0D594..,
are `shadow_map_size_index`, `is_cast_shadow_*`, `range`, `attenuation`, `spot_*`, the two
influence factors, `use_with_lpv`, `static_shadows`, `min_screen_factor_for_shadow_casting`,
`is_light_animated`, `color_curve`, `light_animation_length`). The prototype uses one radius for
every light (`PCSS_LIGHT_RADIUS`, a compile-time constant). A per-light size needs a small exe
patch: register a constant host `light_source_radius` in the `stage_lights` constructor (as the
others, 0x18F562..), fill it in `render_shadowed_light` / `render_light` from a new light config
key read beside the others (0x34F99C..0x34FB38). The two influence factors are uploaded but read by
no shader; they are not a size and stay unused. PROVEN (the inputs), design (the patch).

**The filter** (`srp-026g/shaders/sm_4_0/pcss.h` on `research/renderer-026g`), per shadowed pixel:

```
1. blocker search   N_b point loads on a disc of radius
                    search = clamp(2 R P00 / z_r, 1.5 texels, 24 texels)
                    (the penumbra of a blocker at a fifth of the receiver's distance)
                    keep depths < receiver depth; none -> lit (no filter taps)
2. penumbra         z_b = Qn / (Q - mean blocker depth)
                    w   = R (z_r - z_b) / z_b * P00 / (2 z_r)    in uv, clamped 1 .. 24 texels
3. filter           N_f bilinear compare taps on a disc of radius w
                    s = saturate(mean + shadow_transparency)
receiver-plane bias: both depths minus radius * Qn/z_r^2 * tan(angle(N, L)) / (P00/(2 z_r)),
                    tan clamped to 5, so wide kernels do not shadow their own slope
disc: Vogel (golden angle), rotated per pixel by interleaved-gradient noise (fixed in screen
      space: a fine dither, no flicker; 0.26g has no temporal filter to hide noise)
```

`R` is the light's radius, `z_r` the receiver's distance from the light. The stock normal offset
(section 4.4) and the stock depth test (`0 <= d <= 1`) are kept.

**Cost by `r_shadow_quality`** (the permutation's A digit), shadow-map reads per pixel of a
shadowed light (a point light: per pixel of each face; each read is one instruction, a compare read
fetches 2x2 texels):

```
quality   stock          PCSS                         PCSS, no blocker found
   0      4 compare      4 compare (stock, unchanged)  4
   1      4 compare      8 loads + 8 compare  = 16     8
   2      16 compare     16 loads + 16 compare = 32    16
   3      16 compare     16 loads + 32 compare = 48    16
ALU slots (point, SSDO on): stock 146 / 181; PCSS 324 (q1), 468 (q2), 548 (q3)
```

**Limits.**
- Point-light faces are rendered with exactly 90 degrees and no guard band, so a kernel that
  crosses a face edge reads the clamped edge of its own face: a wide penumbra next to a cube
  seam can show a seam. Contact shadows (small kernels) are unaffected. A fix is an exe change
  (a slightly wider face frustum).
- D16 depth with near 0.025 m: at 3 m one depth step is about 5.5 mm, fine for the blocker mean.
- A 256 map (most lobby lights) has 2 cm texels at 2.5 m; a 24-texel cap is about 0.5 m of
  penumbra there, and the cap scales with the map: an author still picks the map size for the
  sharpest contact detail they want.

## 14. What changed since 0.10b

```
                    0.10b (source)                       0.26g (code and blobs)
stages              stage_sun, pre_lighting,             none of these; the sun is lit in the
                    resolve_lighting, translucency,      composition shader, SSAO/SSDO and
                    clouds, LPV stages                   ambient have their own stages, LPV slot
                                                         null
light BRDF          Beckmann, Schlick Fresnel            GGX with remapped roughness, Smith-
                                                         Schlick visibility, Lambert * (1 - F)
light targets       one packed sqrt(x/8) per channel     linear diffuse + specular (R11G11B10F),
                                                         colour linearized on the CPU
local light SSDO    -                                    diffuse * SSDO term (r_ssdo, AO quality 3)
shadow filter       16-tap Poisson, 1 texel              4 taps (quality 0, 1) or the same 16
                                                         taps (2, 3), 1 texel; normal offset
                                                         normalize(0.75 N + L) * 24 / size
shadow maps         D16 1024 / 512 / 256, only with      D16 1024 / 512 / 256 / 128 / 64, always
                    local light shadows on               created
probes              sorted in the ambient stage,         sorted by stage_visibility, same
                    radius, lerp                         predicate and lerp
skylight            six-direction sky colour             a flat value, normally off
emission            -                                    technique 6, ONE/ONE onto generic0
```

PROVEN for the 0.26g column (sections 1, 4, 5, 6); the 0.10b column is the source.

## 15. Not read

- Per-target decal blend values; the depth-stencil format and the G-buffer's stencil write value.
- The writer of the skylight switch (stage+E4h); particle lighting; rain internals.
- The AA, motion-blur, depth-of-field and lens branches of the post-process, branch by branch.
- Foliage and vegetation materials; terrain permutations other than the basic one; most of the 886
  G-buffer variants (detail-normal layers, UV scrolling, other discard rules).
- The order of ambient lights (no sort found on the render path).
- The sun's shadow map (`stage_shadow_direct`, cascades) and `stage_shadow_mask` beyond their
  output target.
