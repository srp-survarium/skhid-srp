# School and Tarakanovsky Fort v0.21

The day maps retain 0.21d geometry, local lights, probe volumes and their baked
cubemaps. The candidate preserves original authored colours and intensities
(School sun 3, Fort sun 2) and adapts original shader instructions to native
bindings. Shared compatibility rules replace the earlier room overrides and
environment calibrations. School's omitted low height-ambient colour now uses
the original black default rather than a value inherited from the native map.

| Map | Protective device | Team Deathmatch | Blackout | Free for All |
| --- | --- | --- | --- | --- |
| School, map number 25 | 174 | 176 | 9 | 111 |
| Fort, map number 26 | 177 | 178 | 10 | 112 |

`release/content.tsv` pins `maps-021d-school-fort@4`, built with resource-porter
`tools/content/school_fort_021d.sh`. The porter binds donor cubemaps to every
mode copy; otherwise a native template's `day_4` cubemap can survive the donor
`day` binding. It also treats a particle's capitalized `Material` as a material
reference: School's shared native effect material must not follow a same-named
particle alias into a nonexistent material.

The source edit lists, applied after the mode copies are made, are:

- `v021d-school-fort-environment-1.json`: shared shader markers and cloud/default conversion, on modes
  2, 4 and 8; Blackout uses the battery project.
- `v021d-school-anomaly-1.json`: the stalactite material's native white diffuse
  multiplier, retaining the donor texture.
- `v021d-fort-torch-1.json`: a private torch particle for the Fort, its original
  5.047268 m light radius and shadow settings, with original intensity and
  colour curves preserved. The earlier half-power adjustment is removed.
  The native shared torch is unchanged.

The room-specific `v021d-school-lighting-1.json` overrides were removed in
favor of the shared engine compatibility rule: every ambient light in these
projects uses the original multiplicative semantics even above intensity 1.
The hallway retains authored intensity 3.9 and the ruined-room pair retains
2/2. See [the scope inventory](three-maps-lighting-scope.md) for the engine
evidence, remaining asset-specific exceptions and validation status.

`content_edit.py` preserves float2/3/4 dimensions in `set_vector`, checks old
values, and rejects a dimension mismatch. The torch's radius curve keys and
material's float4 colour use this operation instead of replacing value types.

## Review artifacts

The comparison rig keeps the donor shots permanently and captures only the
port, one pose at a time. Captures use headless displays and disabled audio,
including an ALSA null device; the running clients are checked for audio streams.
The report shows the donor at left, port in the middle and amplified difference
at right. Automated flags remain visible beside inspection notes. The shader refinement's sweep is pending; earlier screenshots are historical.
The current School `in_19_near` comparison retains a sunlit far wall in 0.21
and shade in 0.26 (luma 45.4 versus 31.0). This is an explicit native-shadow
limitation for final review. The owner requests a close match with stock
values and minimal renderer changes, not pixel identity; shadow generation
remains native. Current hallway and ruined-room comparisons are close with
their original light intensities, without local corrections.

Local report paths (the owner's final review is still pending):

- `/home/sheep/Projects/survarium/v021d-rig-data/reports/step/school_day/index.html`
- `/home/sheep/Projects/survarium/v021d-rig-data/reports/step/fort_day/index.html`

These are local quick-build previews. They are not release artifacts and do
not set a release revision.
