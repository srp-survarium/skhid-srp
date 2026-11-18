# Original particle transparency mipchains

The five ports reused nine native particle textures whose full-resolution image
matches 0.21 but whose regenerated lower alpha mip levels differ. These levels
control smoke, flame, sparks and water-drop opacity. This is the same issue as
the model alpha mip correction, applied separately to particles.

`v021d-particle-alpha@1` preserves the complete original DDS and options bytes in
`ported/v021d/original_particle_alpha/`. The source builder is resource-porter's
`examples/particle-alpha-021-pack.rs`, with pinned donor/stock archives and every
DDS/options hash in `src/particle_alpha_021_pins.json`. All nine options files are
already byte-identical between builds; no texture is regenerated or retuned.

`make_particle_alpha.py` generates the registered edit from
`particle-alpha-inputs.json`: 13 private material copies and 29 exact `Material`
leaf rebindings in 14 imported particle definitions. Shared native materials and
textures remain unchanged, as do older source material copies. Every material
property other than the listed alpha texture references, and every particle
property other than the listed material bindings, retains its complete snapshot.
`check_particle_alpha.py` checks original bytes, source generation, composed
bindings and those unchanged-property hashes through `tools/check v021d-weather`.

The audit covers 37 imported particle definitions reachable from all 20 weather
projects and 41 material pairs. All common original material leaves match. The
nine textures are smoke1, easy_flame1, ak_trail, School point_single/points/
points_small, Vostok smoke4, Fort pasted points, and environment water_drop.

## Runtime evidence and limits

A read-only native Vostok `artefact_container_2` draw matches the exact original
smoke1 top-mip bytes. Its actual offscreen particle PS digest is
`057b5c3e42c776c60cd29c36e4dbd373`; `t_base` and `t_transparency` bind seven mip
levels each. All 14 captured levels match native DDS bytes, and eight differ
from original bytes. This establishes the used mipchain difference without
launching the frozen Vostok original. Evidence is retained under
`private/v021d-five-weathers/visual/diagnostics/vostok-haze/particle-trace/`.

Six original particle materials reference normal DDS files missing in 0.21 but
present in 0.26. This alone does not justify a correction: the observed native
smoke PS and the corresponding original PS both omit `t_normal` entirely. Their
normal fields remain unchanged. Separate static and original School runtime
observations establish the original missing-texture fallback as a generated
16x16 linear RGBA8 gradient, one mip, pixels `(16*x,16*y,0,255)`. No substitute
normal texture is authored from that unrelated fallback observation.

The isolated pack, original-byte check and builder lint passed (44 pre-existing
library warnings, none added). Composed isolation and affected haze comparisons
remain required after the next combined build; this source correction does not
claim final visual acceptance or explain every smoke-phase difference.
