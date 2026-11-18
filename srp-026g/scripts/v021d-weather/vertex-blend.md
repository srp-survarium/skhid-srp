# Original vertex-alpha blending

The Rudnya rain comparison at `in_03_near` shows the wall of
`level/01/background/back_house_6` substantially paler than the original.
The live draw has the original 317 vertices, 885 indices, vertex alpha and
both original diffuse texture payloads. Its albedo is already pale before
lighting. This is a material arithmetic difference, not an exposure adjustment.

The original unmasked shader interpolates with base weight `vertex_alpha`.
The observed native shader instead uses
`saturate(pow(abs(vertex_alpha), (1 - tiled_mask)^2 * falloff) * strength)`.
The imported material inherits native strength 4, tiled-mask scale 2 and
rotation 35 degrees. Preserving only common original material leaves did
not preserve the original rendering equation.

`v021d-vertex-blend@3` extends the verified correction to 500 native pixel
shader paths (125 distinct programs). Each layout has its own pinned native
blob, vertex-alpha input and final weight destination. The selector is the
integer material field `constant_vertex_blended_strength = -21021`, converted
exactly to float by the engine. It selects the original equation, not a tuned
strength. Removing that guarded assignment and its scratch register recovers
every native instruction; signatures, reflection, wrapper headers and resource
bindings remain identical. All 125 resulting programs disassemble successfully.
The four pilot shader outputs remain byte-identical to version 1.

The material audit classifies all 219 originally blended materials: 203 active
unmasked, 13 masked, and three with no active blend channels. This edit selects
all 216 active original materials. Fifteen shared native materials are
copied under `ported/v021d/original_blend/`, with exact replacements restricted to
ported model settings files. Twenty-eight materials need native technique 1
to activate the original second texture; two need their original normal-blend
flag restored to false. Other values, including reviewed native finishes,
remain unchanged. The three inactive materials retain their current settings.

Source: resource-porter `weather_fog_021/vertex_blend/linear_set.rs` and
`examples/vertex-blend-masked-021-pack.rs`. The native contracts are in
`linear-programs.tsv`; this repository keeps the check's independent path/hash
catalog as `vertex-blend-programs.tsv`. `make_vertex_blend.py` derives the edit
from `vertex-blend-inputs.json`. The registered `check_vertex_blend.py` checks
every composed shader path, independently derives its alpha input from native
instructions, verifies the material flags against the original archive, checks
all private model bindings and rejects changes to unrelated material leaves.

The original Rudnya rain wall pilot is accepted at its observed draw: the
original mesh, two texture payloads and live compatibility constant were
verified. The right-wall ROI changed from RGB 108.90/110.83/103.16 to
68.30/75.83/74.81, against the original 65.73/73.23/72.22. The neighboring control
wall's albedo remained effectively unchanged. Evidence is retained in
`private/v021d-five-weathers/visual/diagnostics/rudnya-wall-texture/`, including
`wall-linear-comparison.jpg` and `linear-wall-acceptance.json`.

This scoped rendered result does not accept the expanded material set or the
weather maps. The broader correction still requires composed checks and fresh
regional comparisons; other visual findings remain open.

## Original masked equation

For the 13 masked materials, the original shader computes `q = 1 - v`, then
`blend = pow(saturate((q*(1-a)+a) * pow(q,h) * (f+1)), f)`, where `v` is vertex
alpha, `a` base texture alpha, `f` the original factor and `h` original falloff.
The native base weight becomes `1 - blend`. The factor is encoded losslessly as
integer strength `-f * 2^23` in `[-2^24,-2^23]`; the engine's integer-to-float
conversion and shader power-of-two scale recover its exact f32 bits. The linear
marker and nonnegative native strengths are outside this interval. One private
wood-floor material also restores the original falloff 0.59 from native 10.

An alpha-only base sample uses the native base UV and sampler inside the uniform
material branch. It avoids reordering native samples: in 88 layouts, the native
base sample occurs after the blend weight. Both compatibility modes reuse one
additional scratch register. The entire native and linear instruction sequences
remain unchanged. An independent DXBC interpreter compared all 125 generated
layouts with the actual original shader block on a 257 by 257 grid for authored
pairs `(1,1)`, `(1.85,1)`, and `(1,0.59)`; the resulting weights match bit for bit.
All programs disassemble. This does not replace live rendered validation.

## Missing normal-off family

School/Rudnya/Fort/Mamayev load pilots exposed absent native variants with
technique 1 and normal blending off. `v021d-vertex-blend-normal-off@2` covers the
compatible original family: 482 absent paths, 192 pinned contracts and 42 distinct
original program blobs. This includes base-normal on/off, all original mode
aliases and seven vertex layouts across two quality prefixes. Every output is
an exact original unmasked/base-normal shader, without bytecode changes. The
builder refuses paths already present in stock or supplied mounted layers.

An original normal-off shader can read tangent Z while its native normal-blended
counterpart reads XYZ. Input compatibility therefore checks semantic name/index,
system value, component type, register and component inclusion. Output signatures
remain byte-identical. Every used original material constant has the same name,
offset and size in its native reference. Original shaders never sample blended
normals or tiled masks. The base-normal compiler branch at 0x1DFCE2 supplies
`t_normal` when needed; `s_base` is an existing native sampler. Unsupported
original alpha-discard, retired-feature and additional-constant combinations are
excluded, not guessed. All 168 version-1 payloads, including wrapper timestamps,
remain byte-identical; no previously composed shader changes in this expansion.

`check_normal_off.py` checks the composed original bytes, semantic contracts,
source-to-target key mapping and previous-payload identity. Runtime binding and
output comparison remain required. A separate original School ceiling capture
matched all 35 vertices/165 indices and the pinned original pixel shader; its
native control was stopped by the additional missing family before frames. The
expanded catalog includes that exact missing path. Evidence is retained in
`private/v021d-five-weathers/visual/diagnostics/school-normal-off/`.

The native class-1 shader reads its weight input from `v7.x`, which also belongs
to its UV input. This surprising layout is present in the original class-1
shader too; class 2 uses `v9.x`. The compatibility adapter preserves each actual
original/native layout rather than imposing the observed class-2 wall layout
on every program.

## Alpha-tested blended materials

Five active blended materials also enable the original alpha test. Both reviewed
original linear and masked programs copy base texture alpha into both operands
before their blend interpolation. Their cutoff is therefore `base_alpha -
alpha_ref`, independently of the vertex blend weight. The native template's
`alpha_test_consider_vertex_blend=true` instead makes the blended weight affect
the cutout. The source edit sets that selector to false on exactly those five
materials; it does not change their cutoff value, textures or blend equation.

`vertex-blend-inputs.json` pins the two reviewed original shader blobs in
`alpha_cutoff_proofs` and marks these rows `alpha_cutoff: base_texture`.
The generator checks their original alpha-test flag, and the composed checker
verifies both original blobs and the resulting selector on every alpha-tested
blend material. Previous unchanged-leaf snapshots were verified before excluding
this one additional changed field. The correction still requires the next
composed build and runtime comparison; candidate 7 predates it.
