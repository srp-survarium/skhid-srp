# The 0.26g renderer: what a light bake needs

What the 0.26g client draws and from which data, read so that an offline lighting bake (light maps,
or light baked into textures) can reproduce the game's own materials and lights. It is not about the
camouflage and decal bakes of [baked-textures.md](baked-textures.md), which bake G-buffer textures
for painted items.

Source: an abandoned real-time raytracing attempt (2026-10-27), which read the executable
(SHA-256 `ac33d993...3bf153`), ran parts of it under Unicorn, and ran the shipped shaders' original
bytecode on a GPU with D3D11 (no game was started). Its renderer was not kept; its findings are
here and in `tools/re-kit/v026g/` (functions, `abi.tsv`, `abi_inputs.tsv`). Tags as in the notes'
README: PROVEN (read at the address given, or the shipped shader executed), LIKELY, GUESS.

Shaders are in `resources.db` under `shaders/sm_4_0/<family>.<ps|vs>/...`
(`tools/re-kit/common/dbgrab`). A blob is a u32 count, that many 264-byte dependency records, then
the DXBC. The later reconstructed SDK sources differ from the shipped bytecode in several places
noted below; the bytecode is the reference.

## 1. The frame

`renderer::render` (0x13D010) stores the scene in the renderer context at +416Ch (0x13D210), the
scene view at +4170h (what the stages read as the view), and the renderer's float clock (+2E4h) at context+4140h (0x13D257), which is
the `scene_time` shader constant (registered at 0x1498A6..0x1498EB). PROVEN.

`renderer::execute_stages` (0x13BCA0, renderer in EDI) runs a 27-slot stage vector
(+150h/+154h) in slot order, each non-null stage through its vtable+4 (0x13BE15). The constructor
fills these slots (PROVEN; the "mode" is the constructor's literal argument, its meaning is read
from the code only where stated):

```
slot  stage (constructor)           assigned at
  7   lights, mode 0 (0x18EE40)     0x13A6CC
  9   shadow mask (0x1A1180)        0x13A706
 10   atmosphere, mode 0 (0x1A39D0) 0x13A741   scattering tables, sky dome (section 6)
 11   composition (0x1A7280)        0x13A77C   diffuse + specular accumulation -> generic0
 12   atmosphere, mode 1            0x13A7B8   another path (0x1A50C9), not read
 13   forward, mode 1 (0x1AAB30)    0x13A7F5
 14   screen-space reflections      0x13A830   also draws water (section 5)
      (0x1AC870)
 15   forward, mode 0               0x13A86C   opaque emission (section 3.3)
 16   lights, mode 1                0x13A8A8
 17   rain (0x1AD830)               0x13A8E3
 18   particles, mode 0 (0x14BA90)  0x13A91F
 20   particles, mode 1             0x13A9C3
 23   postprocess (0x1AFD40)        0x13AA7E
```

So emission is added after composition, the sky, SSR and water; a bake that lights the G-buffer
must not count emission twice. PROVEN (stage order, `stage_forward::execute` 0x1AB900 calls
`render_opaque_models` only for mode 0, 0x1ABBE9).

### 1.1 Render targets

`renderer_context::get_rt` (0x11C1D0) reads descriptor `index * A0h + 98h` of the target table at
context+119Ch. Names come from `rt_index_to_name` (0x162550, jump table 0x16272C). Formats from
`create_targets` (0x162DA0); HDR is 26 R11G11B10_FLOAT when the device feature level (+124h) is at
least 10_0 (0xA000), else 10 RGBA16_FLOAT (0x1631BE). PROVEN, and Unicorn-run.

```
index  name                          format
  10   $user$normal                  34 R16G16_FLOAT          G-buffer slot 0
  13   $user$albedo                  28 R8G8B8A8_UNORM        G-buffer slot 1
  15   $user$surface_parameters      28 R8G8B8A8_UNORM        G-buffer slot 2
  31   $user$accum_diffuse           HDR
  34   $user$accum_specular          HDR
   7   $user$accum_local_lights_specular  HDR
   8   $user$sun_shadow_and_scattering    49 R8G8_UNORM
  55   $user$generic0                HDR   composed scene colour
  56   $user$generic1                HDR   copy of generic0 for water/SSR
  71   $user$mie_scattering          10 RGBA16_FLOAT, 256x128
  72   $user$rayleigh_scattering     10 RGBA16_FLOAT, 256x128
```

`stage_gbuffer::execute` binds 10, 13, 15 to RTV slots 0/1/2 and clears slot 3 (0x177BEE).

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

## 5. Water

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

## 6. Sky and atmosphere

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

## 7. Not read

- G-buffer normal encoding (two R16 channels) beyond its ill-conditioning near +Z.
- Rain (the composition's rain branch, stage 17), decals, subsurface, translucency, SSAO/SSDO.
- Foliage and vegetation materials; terrain permutations other than the basic one.
- Most of the 886 G-buffer variants: detail-normal layers, UV scrolling, other discard rules.
- Ambient and environment-probe lighting (what fills the accumulation targets besides the sun
  and local lights), and how `debug_multipliers` is set in a normal frame (GUESS: 1, 1).
