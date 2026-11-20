# Original painted-alpha surfaces

The original Vostok evening `ground_02` view showed gravel where the port showed
grass. Render ray intersections and the live draw identified
`level/03/terrain_all::trash_dirt__vc` (2,230 vertices, 8,934 indices). Its mesh and
all nine diffuse mip levels were byte-identical to the original. The native
pixel shader ignored the original painted vertex-alpha test, although its
vertex shader still supplied the painted alpha.

The original visible cutoff is:

```
mask = pow(v, light_border * 7)
     + a * pow(v, dark_border * .7) + .2
cutoff = mask * (1 + multiply * (saturate(3*a) - 1)) - .4
discard if cutoff < 0
```

Here `v` is painted vertex alpha and `a` is base texture alpha. The adapters keep
the original instruction order, including the separate multiplications and
fused multiply/adds. The two original scalar vertex inputs are v7.x (class 1)
and v9.x (class 2). The other five original static classes supply zero painted
alpha, as their original programs do.

The original depth pass is different: it discards when ordinary base texture
alpha is below `alpha_ref`. All 14 original alpha-discard depth programs use
that rule. Therefore the selector encodes both the visible profile and the
original shadow threshold:

| Selector | Dark | Light | Multiply | Shadow threshold | Materials |
|---|---:|---:|---|---:|---:|
| -21210 | .1 | .5 | false | .5 | 1 |
| -21211 | .5 | 1 | false | .25 | 1 |
| -21212 | 1 | 1 | false | .5 | 13 |
| -21213 | 1 | 1 | true | .25 | 6 |
| -21215 | 1 | 1 | false | .25 | 7 |

The diagnostic -21214 profile belonged to an alpha-disabled original material
and is deliberately excluded. Five such materials remain unchanged.

`vertex-alpha-inputs.json` records all 33 original material identities, donor
leaf hashes, the 28 exact changes, and hashes of every unaffected effective
leaf. Every edited target is already private to the 0.21 maps. All original
blends are inactive: 26 have the original master switch disabled; the other
two enable no channels. Native compilation ignores that legacy master switch
and uses `vertex_blend_technique` instead. The edit selects ordinary native
shading, removes template-only tiled-mask and channel activation, and enables
the original cutoff. Original caster flags, active base normal/detail inputs,
textures and unrelated material values remain unchanged.

The source builder is resource-porter's `examples/vertex-alpha-021-pack.rs`,
with `weather_fog_021/vertex_alpha/{geometry,depth,profiles}.rs`. It reads pinned
donor and stock resources directly. The pack contains 1,364 visible paths
(259 programs) and 78 depth paths (21 programs). Visible coverage includes all
854 ordinary native alpha paths for the seven original static classes and 510
aliases with identical complete native blobs. Other material feature groups
are unrestricted. Native constants, texture bindings, output signatures and
default arithmetic remain unchanged; only the original additional input
component is made readable where required.

`make_vertex_alpha.py` generates the content edit. `check_vertex_alpha.py` is
registered in `tools/checks/v021d_weather.sh` and checks every composed alias,
the source hashes, native instruction inverses, donor cutoff execution, exact
material selectors and unaffected leaves. Its scalar executor needs only the
standard Python 3.13 library. Earlier visibility and alpha-mip checks undo only
these exact validated later deltas when comparing their prior snapshots.

Independent full-grid validation in
`private/v021d-five-weathers/visual/diagnostics/vostok-ground/` checked all 259
visible programs against the seven original layouts on 257×257 inputs for each
of the five profiles. Every cutoff matched bit-for-bit; all programs
disassembled. The 21 depth programs passed 4,101 alpha samples and 276 selector
values, including boundaries and unmarked values. The initial one-material
capture restored the original gravel and its painted edge: static-region MAE
fell from 17.94 to 2.07. This causal capture predates the full family.

Full-family composed validation and runtime comparisons remain required.
The local proof artifacts are not a substitute for the final regional sweep,
nor for a capture of an affected shadow caster. No release is implied by this
development pack version.
