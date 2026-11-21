# Original forward fog for vehicle glass

The School rain comparison found pale blue vehicle windows where the original
was dark. Restoring the original `use_soft_edges` switch did not correct this.
An input-free, read-only native draw trace matched all 4,424 uploaded vertex
bytes of the original UAZ ambulance glass and recorded its distortion, forward
base, probe and direct-lighting passes. Original diffuse, normal and transparency
pixels were preserved. The forward passes still sampled native Rayleigh LUTs,
while the existing original atmosphere adapter covered fullscreen, particles
and water. The observed native LUT is strongly blue; the original forward
shader uses the environment's authored Rayleigh color and original arithmetic.

`resource-porter/src/weather_fog_021/forward_glass.rs` transplants the pinned
original fog block into each exact matching native forward block. Native
lighting, reflection, transparency and bindings stay intact. The observed three
programs are mandatory; matching code also covers seven additional direct-light
permutations. Version 2 enumerates the complete published archive stack, including
the base-pass compatibility alias from patch14 that the live draw actually uses.
Its eleven resource paths contain ten distinct guarded programs. Different blocks and transparency continuations remain native
until reviewed. This is not a claim that every forward variant executes original
shader code.

Each branch uses one of the fifteen existing day/rain/evening scene markers.
The original RGB upload uses single-precision `powf(color, 2.2)`. The far-distance
schema mapping and observed original infinite height multiplier follow the
established day fullscreen/particle/water adapters. Original blocks use fresh
scratch registers; their native continuation's live value is restored explicitly.
Every unmarked scene executes the complete original native block. No global
fog setting or material tint is changed.

Regenerate with `examples/forward-fog-021-pack.rs PROFILES DONOR STOCK PATCHES_TSV PATCH_CACHE NEW_OUT`.
`forward-fog-profiles.json` is the exact profile input copied from the lighting
pack source. The generator checks both retail archive hashes, original/native
shader hashes and exact instruction windows before producing a source pack.
Every contributing published archive is hash-verified and recorded in
`source-archives.json`; the source manifest checksum records the inspected stack.
`check_forward_fog.py` independently verifies all fifteen guards/colors, each
transplanted original block, every native fallback instruction, unchanged shader
bindings/chunks and the final composed bytes when `--patch` is supplied. The
read-only `shader-digests` example independently indexes every published layer;
the checker requires every effective alias of the ten pinned native programs.

Version1 passed these checks and all ten programs disassemble. Its five-view
pilot improved glass brightness but retained a native base pass through the
published alias; runtime shader digests proved the omission. Version2 alias
closure passes the independent index check. Its composed native comparison is
**pending**. The first pilot reuses the
five kept School rain glass/control cameras; no original image is overwritten.
The existing day-port native-finish policy is preserved and does not by itself
waive visible differences. The separate roof sheen difference remains recorded.

## Particle base programs (version 3)

Fort's anomaly in rain drew pale blue squares: its additive particles
(`fx/level_5_anomaly/new/pasted_points`, `forward_default`) are fogged by the
forward base pass, and the native fog adds the sky LUT's colour over the whole
quad. 0.21's particle program fogs with a fifth of the environment's Rayleigh
colour. Version 3 adds a second case to the builder (`Kind::Particle`): the
original block of `forward_base.ps/______0________2__010__3...` goes into the
nine stock particle base programs that share one native fog block (blend 1 and
2, vertex classes 3 to 6; 36 stock paths and their 36 published aliases). Their
tails read only the fog colour (`r0.xzw`) and amount (`r0.y`), which the
original block's `r0.xyz` and `r1.y` are copied into. Other particle programs
have other register layouts and stay native. `forward-fog-inputs.json` pins the
nineteen programs; `check_forward_fog.py` tells the two base cases by how the
results reach the tail. Day and evening showed no squares before or after.
