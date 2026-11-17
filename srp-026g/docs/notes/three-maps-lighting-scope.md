# Scope of the five 0.21 day ports' appearance changes

The owner extended the shared pipeline to the already ported Vostok and
Mamayev day maps on 2026-11-17. Their earlier packs substituted native lighting
objects; the new source edits restore donor lights/probes and their mode
cubemap bindings. Existing fixture bindings are reassociated with those lights
using the shared breakable-lamp rule. No geometry or gameplay objects are
replaced. All day-mode aliases are covered; unrelated maps keep native behavior.

The candidate preserves original authored colours and intensities and adapts
original shader instructions to the newer engine. The appearance target is a
close match with stock 0.21 values and as few renderer changes as practical,
not pixel identity. On 2026-11-17 the owner approved the @3 Rudnya hall,
northern corridor and church comparisons as the desired standard. Keep this
rendering scope unless another view exposes a noticeable compatibility defect;
do not tune away small residual differences. Final five-map and native-map
isolation review remains pending. Room-specific ambient
overrides, global artistic calibration and the half-power Fort torch change
are removed. The original fog/tone pilot reduced Rudnya tractor-hall image
error from 9.25 to 3.09 (luma 20.2 versus reference 20.5), but still had the
previous global calibration. The complete original-value candidate requires
its own visual sweep; these pilot numbers are not final acceptance.

| Scope | Change | Source / limitation |
| --- | --- | --- |
| Engine rule, every ambient light in these five day-map projects | Original linear colour × intensity in the multiplicative pass, including intensity above 1 | `ambient_021_026g.c` / `.asm`; exact project-name and owning-world guards, no coordinates/light names. Other maps and lobby scenes keep native behavior. |
| Engine rule, every particle light in these five day-map render scenes | Original authored intensity, without the newer engine's ×3 gain | Same source; actual owning render scene must equal the active game world's scene, with the same exact project guard. No particle names, positions or curve edits. Fort torch and Mamayev dugout comparisons pass; broader review pending. |
| Shared shaders, these five day environments | Original geometry fog, tone curve, sky/scattering, clouds, quality-1 AO, ray falloff, direct-sun block, supported particle/water fog, original point/spot lighting | `three-maps-021-lighting@4`, resource-porter `examples/lighting-021-pack.rs`; 88 resources, fingerprinted original/native inputs and native fallback. Full coverage and remaining native portions are listed below. |
| Environment conversion, every day-mode alias | Original authored colours/intensities; original defaults where omitted; cloud rotation convention, neutral extra cloud-sun multiplier, shader marker | Generated from donor and canonical packs by `tools/content/three_maps_021_environment.py`. Rudnya omitted height ambient becomes black/white; School omitted low becomes black; Fort authored values stay. No artistic gain/tint. |
| Porter rule, every imported probe | Bind donor weather cubemaps to every mode alias | Preserves donor volumes and textures. |
| Porter rule, imported materials/particles | Native schema conversion, resource namespacing and dependency binding | Compatibility conversion; not a claim of identical shading. |
| Fort torch resource only | Private native-schema torch, donor radius/shadows and original intensity/colour curves | `v021d-fort-torch-1.json`; resource-specific schema/radius correction, no half-power adjustment. |
| School anomaly material only | Native white diffuse multiplier with donor texture | `v021d-school-anomaly-1.json`; asset-specific conversion. |
| Specific vehicle materials | UAZ/Kamaz wheel gloss/Fresnel overrides; van/ambulance Fresnel texture correction | Porter manifests and guarded shared-resource fix; asset-specific. |
| Specific cabinet materials | Fresnel 0.05 | Porter `finish_exceptions`: three School/Fort cabinet materials, Rudnya electrical cabinet 02; retained asset exceptions. |
| Existing Vostok/Mamayev metal-column material | Gloss 0.055, Fresnel 0.04 | Earlier `metal_column_05_destr_red` finish exception; retained asset-specific conversion, not a new lighting adjustment. |

### Shader coverage and remaining native behavior

| Pass | Adapted original code | Remaining native behavior |
| --- | --- | --- |
| Geometry fog | 1 program; authored linear Rayleigh RGB | Native engine bindings |
| Tone curve | 4 supported permutations | Other permutations |
| Sky/scattering | 2 programs; original equations and authored gain | Native scheduling/bindings |
| Clouds | 8 lit/unlit programs | Native bindings with coordinate/default conversion |
| AO | 1 quality-1 generator, normalized for native application | Other qualities and filter/application |
| God rays | 4 radial falloff programs | Mask, projection and application |
| Sun composition | 4 direct diffuse/specular blocks | Foliage block, other permutations, shadow generation |
| Particle fog | 24 supported batched variants | Particle lighting and other permutations |
| Water fog | 8 supported variants | Surface lighting and other permutations |
| Point/spot lights | All 32 native permutations use original diffuse/specular and shadow-sampling programs | Native shadow-map generation and screen-position input |

Probe shading is still native. Ambient sphere
programs already match the original; the native box-edge saturation remains.
These are explicit remaining differences, not hidden room corrections.

The removed local ambient workarounds were:

| Map / object | Previous override |
| --- | --- |
| School `ambient_light(10)` | Intensity 3.9 → 1.5 |
| School `ambient_light(14)` | Intensity 2 → 1 |
| School `ambient_light(15)` | Intensity 2 → 1.1 |
| Rudnya tractor-station `ambient_light` | Intensity 2.7 → 1.1 initially; warmer trial used 1.3 and RGB 135/171/255 instead of 216/229/255 |
| Rudnya northern `ambient_light(58)` | Intensity 1 → 1.5 initially; subsequent local tint trials were stopped in favor of the engine correction |

These objects retain their original authored values in the new content
candidate. Previous screenshots and their notes are historical comparisons;
they are not validation of the engine candidate.

## Evidence for the ambient compatibility rule

Original `object_ambient_light::insert` (0x1A7008) and native 0x290930 both
linearize packed RGB with exponent 2.2. Original
`stage_ambient_lighting::render_ambient_lights` (0x10EFAA) iterates one list
and uploads RGB × intensity at 0x10F1B6–0x10F1E7. Native 0x1858E0 splits at
intensity 1; 0x181660 subtracts 1 for the additive list. Thus an authored
intensity above 1 no longer has its original meaning.

All six original spherical ambient pixel programs are byte-identical to
the native multiplicative variants. Each of the six box variants differs
by one instruction's saturation flag (`mad` → `mad_sat`); that native box
edge clamp remains. The compatibility hook changes only outgoing render
parameters: linear RGB becomes RGB × intensity, intensity becomes 1.
Authored objects are not mutated, and repeated insertion recomputes from
the original object rather than accumulating a correction.

The hook uses the object's scene at +108h and its game at scene+9Ch, requiring
the scene to be the game's embedded world (+18h). It reads the project's
buffer string at game+55E0h, assigned before project loading for live and
replay games. A lobby therefore cannot inherit compatibility from the
last match's stale map name. The hook has no persistent state to reset.

## Shader isolation and original-program verification

The built @4 pack has 88 replacements. An independent DXBC check resolves
only the explicit five marker guards and recovers every native executable
instruction in all 88 programs. Only extra scratch declarations and the
static-constant-buffer declaration/size needed by those guards differ.
A second check disassembles the 32 original point/spot programs and every
marked branch: all original arithmetic is identical, with only screen-UV
reconstruction and named resource-binding slots adapted. The final source
reproduces all 89 pack files (including pack.json) byte-for-byte.

These CPU checks prove instruction preservation, not visual equivalence.
The final five-map in-game sweep is pending. An independent check preserves
all 88 shaders' instructions for the three @3 profiles. Archive comparison
found 8,893 identical files; only those 88 shaders and Vostok day projects
changed, with original probe textures and Mamayev day projects added. This
allows the completed @3 comparisons to be retained with explicit provenance.
The expanded ambient selector passed 264 cases; the particle rule passed
1,152 cases including actual render-scene isolation and owner teardown.
Clippy in the normal Nix development
environment introduces no warnings; the library retains 44 existing warnings.
