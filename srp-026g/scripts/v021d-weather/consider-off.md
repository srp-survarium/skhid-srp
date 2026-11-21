# Base-alpha cutoff for alpha-tested blend programs

An original alpha-tested blend material cuts out by its base texture alone, so
`v021d-vertex-blend-1.json` turns `alpha_test_consider_vertex_blend` off on its port.
Stock ships that selector for most feature sets; where only the consider-on program
of a key exists, the game stops at load on the absent path. Candidate 10 hit it on
Rudnya (all three weathers) with the masked stucco materials:
`gbuffer_pass.ps/_3___1001_00101_00____2_____0__1_01000____10000____000__________0__`.
0.21 has no program to adapt for these keys: it only shipped their masked form, which
the port draws through the blend adapter's masked branch.

`v021d-vertex-blend-consider-off@1` adds every such absent sibling: 12 paths. Each is
the `v021d-vertex-blend@3` program of the same key (native program plus the original
blend adapter) with one changed instruction: the blend-aware alpha
`1 + w * (a - 1)` becomes the base alpha `a`, before the unchanged `alpha_ref`
comparison. The builder is resource-porter's
`examples/vertex-blend-consider-off-021-pack.rs`
(`weather_fog_021/vertex_blend/consider_off.rs`); it derives the set from the blend
catalog and skips whatever stock, a published patch or another listed pack has.

`check_consider_off.py` (in `tools/check v021d-weather`) requires each path absent from
stock, composed unchanged, and exactly that one instruction away from its source.
