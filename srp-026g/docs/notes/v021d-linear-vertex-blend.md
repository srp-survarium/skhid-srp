# Original 0.21 unmasked vertex blending

The imported moss/rock boundary was sharper than the original because the
native template added a tiled vertex mask and strength 2. The compatibility
pack changes only those new-engine fields: black mask and strength 1 restore
original linear diffuse/normal vertex weighting with authored falloff 1.
Native shader code, donor-authored fields, surface artwork, geometry and lighting
remain unchanged. Specular/roughness are outside this correction.

The generic rule selects the shared `material_instances/ported/v021d/` namespace
used by Rudnya, Vostok, Mamayev, School and Fort, including modes/weather sharing
those materials. There are no map, object, position or colour tuning lists.
Release-64 inputs select 171 materials. Four original unmasked candidates remain
unchanged because their native permutation has no tiled mask:

- `tiled/metal/metal_column_05_destr_red.material`
- `tiled/stucco/stucco_bricks_vc.material`
- `tiled/trash/trash_dirt_moss_crack_vc.material`
- `tiled/wall/paper_01_palki_vc.material`

The generated mask is
`textures/ported/v021d/compat/linear_vertex_blend_mask.{dds,options}`. This pack
adds no executable patch and changes no map IDs, game rules, world format,
server tables or native materials. It remains a follow-up for review, unmerged
and unreleased; revision 64 is unchanged.

## Source and reproduction

Porter source: `srp-survarium/resource-porter` commit
`4b7f1d6` on `followup/v021d-linear-vertex-blend`, based on
`port/v021d-three-lighting` at `1ea3667`. The shared `vertex_blend_021` classifier
and application are used both by future 0.21 conversion and by
`examples/linear-vertex-021-pack.rs`. Eligible imports cannot be collapsed to
native aliases; generated texture bindings are applied after dependency scan.

Pack `v021d-linear-vertex-blend@1` has manifest SHA-256
`0d14127df5e33f14cddba2c1b786042649e164b3686b1c27a56dffd358cd5db8`.
The exact argv (all stock/published layers and accepted release64 input) is in
`/home/sheep/Projects/survarium/skhid-srp/private/v021d-moss-smoothness/source-regeneration.json`.
The local reproduction commands are:

```sh
# Run from the porter source checkout, with the archived input list:
nix develop --command cargo run --release --example linear-vertex-021-pack -- \
  "$donor_021_db" "$new_pack_directory" "$stock_026_db" "${patches[@]}"

# Composition stays on the normal manifest pipeline, in the isolated SRP checkout:
SKHID_RELEASE_ENV=/home/sheep/Projects/survarium/skhid-srp/private/v021d-moss-smoothness/source-build.env \
  release/build-content 64 --out /tmp/v021d-linear-review-content
```

The latter is a local quick-build name, not a release revision reservation.
The captured candidate was built through this pipeline from an earlier
manifest serialization; the shared-rule generator reproduces all 173 operations
and both emitted file payloads exactly. Only descriptions/serialization changed.
Captured patch SHA-256:
`821ec212526962c340e5d61cfcab96a40c1eca8c5a1b43497d183fa554362556`.

## Review evidence and limits

Private gallery:
`/home/sheep/Projects/survarium/skhid-srp/private/v021d-moss-smoothness/comparisons.html`.
Three columns are original permanent reference, accepted-before (content
byte-identical to release64), and candidate at the same owner settings/pose.
The four full-resolution sheets under `review/` are `battery_4.png`,
`generic_anomaly_core_2.png`, `damage_zone_1.png`, and `particle_4.png`.

The first three show smoother original moss transitions. The fourth retains
moving smoke, atmosphere and foliage differences; its full-frame brightness is
farther from the reference. No light tuning or wider map visual sweep is claimed.
All four candidate captures were headless and silent; their clients are stopped.
A texture-quality-only control and original alpha-mip control did not fix these
edges; those diagnostics are not included in the feature source.

Validation: 16 vertex-filtered porter tests pass, including four focused new
regressions (blend identity for all 256 byte values, field isolation/idempotence,
unsupported contracts, native-alias exclusion). The example builds. A layered
byte comparison of all 9,324 overlay-union files finds exactly the 171 selected
material changes and two generated masks; all remaining effective files share
identical layers. No original authored leaf changed. Detailed reports are
`composed-isolation.json`, `linear-output-verification.json`, and
`source-regeneration.json` in the private review directory.

Strict Clippy stops on 44 existing diagnostics outside changed code; no unrelated
fixes were added. `tools/check --list` broadly selects 14 checks through
`content.tsv` and re-kit TSV globs. Per the requested focused scope, executable,
world-corpus, map-ID, ambient, localization and unrelated server suites were not
repeated, nor was identical content recomposed merely for pack metadata changes.
The native parameter-binding observation is recorded in
`tools/re-kit/v026g/abi_inputs.tsv`; full shader paths/math and binding addresses
are documented in the porter source note.
