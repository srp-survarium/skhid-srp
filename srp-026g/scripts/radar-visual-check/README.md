# Radar v0.11 visual checks

The four views in `poses.json` reproduce the owner's screenshots from
2026-11-12: tires (01-03-56), the wall seam (01-05-31), distant smooth
ground (01-07-47), and the barrel seam (01-13-11). The 00-49-30 screenshot
is excluded. These are eye positions and normalized viewing directions;
the map is `v011e_level_03/day_2`, dictionary ID 208. The engine's project
ID 247 is a different identifier.

Use the existing resource-porter workflow in
`resource-porter-veg-v011/tools/chemical-plant/run_reference_011e.py` and
`tools/render-compare/`, together with its
`docs/investigation/original-011e-reference-preparation-20261014.md`.
`capture_views.py` delegates to that workflow's camera protocol, with the
same poses, 65-degree FOV, 1280x720 aspect and ten-second settling time
in both engines. It requires a running capture client and control file:

```sh
python3 srp-026g/scripts/radar-visual-check/capture_views.py \
  --porter-tree ../resource-porter-veg-v011 \
  --control /path/to/camera.txt --out /path/to/new-capture
```

The private comparison runs used a dummy Xorg display, a private Wine prefix,
and local test servers. They did not use the owner's display or live server.
The old 011e standalone server needed its stock loadout profiles retained,
but `SpawnPlayer` messages omitted: the reconstructed loadout otherwise
crashed retail 011e during weapon/inventory deserialization. The native
reference used the existing `private/rudnya-ref069/tool/previews026.py`
login/match workflow and the rev56 client/server. Its test bot needed
`game_data::init()` before login. These are capture-fixture adjustments,
not changes to the shipped game.

## Content sources

Build the two packs from the pinned published patch 22:

```sh
python3 srp-026g/scripts/radar-visual-check/build_geometry_pack.py \
  --porter private/release-tools/vostok-port \
  --patch22 ~/.cache/skhid-release/patches/resources.db.patch_22 \
  --out private/content-packs/radar-v011e-seams-1
python3 srp-026g/scripts/radar-visual-check/build_texture_pack.py \
  --porter private/release-tools/vostok-port \
  --patch22 ~/.cache/skhid-release/patches/resources.db.patch_22 \
  --out private/content-packs/radar-v011e-textures-1
```

The scripts refuse unexpected source hashes. Their output belongs in
`SKHID_PACKS_DIR`; the normal release content manifest lists each pack's
manifest SHA. The release builder assembles the final patch.

`geometry.json` changes three render meshes. Fourteen wall-side boundary
vertices move 10 cm into the wall in both render LODs. The high-detail
barrel-side boundary vertex moves to the neighboring terrain mesh's
position, matching the alignment already present in the low-detail mesh.
The builders check that bounds, indices, UVs and vertex bases stay unchanged.
Collision and scene placement are untouched.

`textures.json` identifies the shared vehicle atlas's diffuse, normal and
Fresnel DDSs. Their source dimensions are 4096². The native streaming-pool
initializer at preferred VA `0x0011b090` starts its square BC1/BC3 pools
at `2048 >> quality_offset`; unsupported dimension/format keys are skipped
by streaming registration. The baseline tire draw binds only 64² versions
of these atlas textures. The pack discards the top mip, updates the DDS
dimensions, mip count and Vostok payload-size header, and retains the
existing 2048² mip chain and sRGB trailer byte for byte. It does not
recompress or redraw the textures. The atlas is shared by other v011e
vehicle props, which receive the same compatibility correction.

The tire geometry is unchanged. Source and port tire positions match
exactly; maximum half-float UV rounding error is below 0.00031. The source
and imported atlas DDSs were byte-identical. Disabling shader LODs,
disabling texture streaming, and raising texture quality independently
left the smooth black bands and 64² atlas bindings unchanged.

Native static-mesh loading at `0x0016ec12` decodes both half-float UV
components before calculating the streaming factor at `0x001c83b0`;
this is not a float-width decoding failure. The latter function and its
five stack inputs/XMM0 result are recorded in re-kit.

`content-edit/radar-v011e-render-1.json` uses the existing detailed terrain
material for the simplified terrain meshes. Both LODs have identical UVs
at matching positions. Geometry LOD distances and index counts remain
unchanged. The previous distant material used a 256² diffuse atlas and
no normal map. This increases distant terrain shading work; it does not
increase the geometry detail or change match simulation.

## Evidence and validation

Private evidence is under `private/radar-visual-check/evidence/`, including
the original 011e and native baseline images and camera metadata. GPU
material captures distinguish the two surfaces of the closest tire model
by its world translation `[21.669, 0.459, 12.131]`: the UAZ wheel material
has tread, while `vehicles_s1` binds the coarse shared vehicle atlas.

The private candidate was built through `release/build-content` from the
rev56 tools, with a frozen manifest of published inputs plus these sources.
This avoids incorporating unrelated dirty Flash/content work in this
checkout. The combined candidate contains 15 files: three meshes, nine
model settings and three DDSs. Its private filename uses 57 solely because
the builder requires a numeric suffix; no release revision is reserved.
The initial quick build was followed by a successful `--full` validation
against the same frozen inputs. Both produce the identical patch SHA256
`73ec626fdad054899b7211c70be86c80c1dde72cb2cb427ff088c4ece84d17a0`.
No `REVISION`, revision table, live files or release state was changed.

The full build log is
`private/radar-visual-check/build/combined-validated.log`. It confirms
readback of the same 15 files, the players' overlay check, server data
generation and item-table generation. Its isolated source tree has no
existing item tables, so those tables are reported as new; none were
written into this checkout. `vostok-port pack check` passes both packs'
six input preconditions against patch 22; `pack verify` confirms all six
replacement files in the final candidate. Python compilation and scoped
`git diff --check` pass as well. The unrelated dirty sources in the
checkout's complete content manifest were not rebuilt.

Runtime inspection on 2026-11-12 confirms:

| View | Baseline | Fixed |
| --- | --- | --- |
| Tires | Shared atlas binds 64²; broad smooth bands | Diffuse, normal and Fresnel bind 2048²; tread restored |
| Wall | Bright line between wall and terrain | Line closed |
| Distant ground | Bright smooth patch behind the trees | Detailed terrain shading; glare removed |
| Barrels | Bright triangular sliver between terrain edges | Sliver closed |

`evidence/comparison.png` contains crops from original, port baseline and
fixed captures. `port-026g-fixed/` holds all four full images and camera
metadata; `tire-bindings.json` and `gpu/` preserve the actual material
bindings and world-transform buffers. `sha256.json` pins 110 evidence
artifacts. Selected original/baseline/fixed images have identical applied
eye/direction/FOV and 1280x720 dimensions, checked from hook metadata.
The original's second frame follows its ten-second first frame, with an
additional 400 ms settling interval. Overall lighting differs between
engine generations; the captures diagnose these defects, not general
lighting parity.

These are implemented content sources, build-validated and rendered in
private static capture sessions. They have not been playtested or released.
Distant terrain shading performance was not benchmarked.
