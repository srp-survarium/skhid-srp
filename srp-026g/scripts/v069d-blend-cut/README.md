# 0.69d's cut of an alpha-tested blend by its second layer (v021d-vertex-blend@4)

0.26g's alpha-tested blend programs (`alpha_test_consider_vertex_blend` on) test
`1 + W (base.a - 1)`: the second layer counts as opaque. 0.69d (`g_stage_default.ps`,
technique 1) cuts by the second layer's alpha where the material does not consider it
opaque (`consider_blended_albedo_opaque` false). Read from its bytecode and run through
an interpreter of the listings against both programs:

| 0.69d flags (technique 1) | permutation | weight W | alpha |
| --- | --- | --- | --- |
| no tiled mask, base opaque | `1001110100110001010030000` | the raw vertex alpha `w` | `B.a + w (1 - B.a)` |
| tiled mask, neither opaque | `1001110001110011010030000` | `sat(pow(w, (1 - m)^2 falloff) strength)` | `B.a + W (m - B.a)`, `m` the mask's red |
| base cut, second layer opaque (0.26g's own) | `1001111100110001010030000` | `w` | `1 + w (A.a - 1)` |

The lath under Rudnya's wallpaper (`stucco_wood_pbr`, 38.5 % transparent, black there)
drew opaque black on house_7's stairs.

resource-porter `weather_fog_021::vertex_blend::blend_cut` adds two guarded blocks to each
alpha-tested blend program v021d-vertex-blend@3 carries (40, the vertex-colour layouts):
before the blend-aware alpha the weight (raw, or the masked one with the decoded strength),
after it 0.69d's alpha, with temporaries r8 and r9. A material selects them by its blend
strength, apart from v021d's selectors (-21021, [-2^24, -2^23]) and every native strength:
-21069 for the linear form, `-(2^25 + strength * 2^17)` for the masked one. The converter
(resource-porter `survarium_026::later::blend_cut`) writes it from the donor's own flags.
`examples/vertex-blend-069d-cut-pack.rs` builds @4 from @3's own files.

`check_blend_cut.py` (`tools/check v069d-blend-cut`): @4's manifest operations are @3's,
every other file is @3's byte for byte, and each changed program keeps every @3
instruction in order with new instructions in two runs only, at the blend-aware alpha,
writing r8/r9 and, under a guard, the weight and the alpha, compared only with the
selectors.
