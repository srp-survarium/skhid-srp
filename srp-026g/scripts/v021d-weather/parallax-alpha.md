# Ordinary parallax alpha closure

Candidate 9 Rudnya requested the absent native shader
`_3___1001_10001_00____2_____0__0_00000____00000____000__________0__`
after the original inactive blend flags were correctly translated to ordinary
native selectors. `ported/v021d/terrain_tiled/dirt_vc` uses parallax without a
normal map. Enabling a normal map to obtain an existing shader would change its
original settings and sample an unbound resource.

`v021d-vertex-alpha-parallax@1` supplies all 28 corresponding missing paths: seven
native vertex classes, two quality levels and two proven native aliases. Source
pins and paths are in `parallax-alpha-programs.json`; the resource-porter builder
is `examples/vertex-alpha-parallax-021-pack.rs`, with implementation in
`src/weather_fog_021/vertex_alpha/parallax.rs`.

For high quality, the shipped dirt program supplies the native height sampling,
UV displacement, colour, roughness and output encoding. Its dirt-mask-only cutoff
is replaced by base alpha minus the material threshold. Classes 1/2 lose 14 dead
mask instructions and the unused mask texture binding; the checker verifies the
removed temporaries are overwritten before use by the native finish. The other
five classes replace the shipped unconditional zero-mask discard. Reflection
marks the ordinary threshold as used and disables only the two removed mask
parameters. Height and roughness bindings retain their native slots. Low quality
reuses the existing ordinary alpha program; the shipped low-quality dirt programs
with/without parallax are byte-identical.

Both templates then receive the same original painted-alpha guard checked for
`v021d-vertex-alpha@1`. This preserves the seven original vertex-input contracts,
five authored profiles and ordinary unmarked cutoff. The separate pack adds only
absent paths, leaving the 1442 already-checked visible/depth replacements unchanged.

`check_parallax_alpha.py` reconstructs the ordinary templates from pinned stock,
checks exact native aliases and low-quality equivalence, then uses the independent
painted-alpha instruction inverse and scalar original-shader evaluator. All 28
paths and 14 distinct programs pass the private proof; all 14 DXBC programs decode.
Clippy reports 44 existing warnings and no new warning. The composed registered
check, Rudnya load and regional visual comparisons remain required; these static
proofs do not establish final map parity.
