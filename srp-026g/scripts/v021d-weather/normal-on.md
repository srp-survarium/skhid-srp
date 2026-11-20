# Missing native normal-blend permutations

The original material conversion activates combinations absent from the 0.26g
shader archive. Candidate7 exposed four such combinations across School,
Rudnya, Fort and Mamayev. `v021d-vertex-blend-normal-on@1` supplies the complete
bounded closure of the corresponding existing native layouts:704 absent paths,
44 pinned source programs and160 distinct resulting DXBC programs.

The source is resource-porter `weather_fog_021/vertex_blend/normal_on.rs`, its
`normal-on-programs.json`, and `examples/vertex-blend-normal-on-021-pack.rs`.
The builder reads the owner's stock/original archives, applies the existing
original-linear/masked adapter, and emits AddFile operations only. Every path
must be absent from stock and the supplied mounted patch stack. No native
program, signature, shared material or map setting is replaced by this pack.

A missing mask-off permutation uses its exact native mask-on neighbor. Six
coordinate instructions and the tiled-mask sample are replaced by a base-alpha
sample with the native UV/sampler contract. The unused tiled-mask declaration
and reflection binding are removed; the other texture slots stay unchanged.
This retains native unmarked base-alpha weighting, while the existing explicit
original-material marker selects the donor blend equation. In particular,
School's native scene-time offset remains400; copying the corresponding raw
original program would incorrectly read396.

A missing alpha-enabled permutation adds the native cutoff after the blend
adapter: either baseAlpha-alpha_ref, or
1+weight*(baseAlpha-1)-alpha_ref for native consider-blend keys. The original
alpha-tested blend materials require the base-only key: both original linear
and masked shaders copy base alpha into both interpolation inputs. The separate
vertex-blend source edit explicitly disables the inherited native
alpha_test_consider_vertex_blend control for the five affected materials.

`check_normal_on.py` independently recovers every original native instruction
outside those bounded changes, checks the complete reflection delta and exact
composed payload, proves removed coordinate components are dead and that no
mask value escapes the original weight override, and checks absence across
stock/published archives. All704 aliases passed that inverse audit and all160
programs disassembled. Native loading and visual comparison remain required;
these static results alone do not accept the maps.
