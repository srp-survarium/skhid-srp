# School and Tarakanovsky Fort v0.21

The day maps retain 0.21d geometry, local lights, probe volumes and their baked
cubemaps. The owner chose the 0.21 appearance on 2026-11-17, allowing the 0.26
sky, atmosphere and sun rendering. The environment edit therefore takes native
0.26 sky, distance fog, atmosphere and sun shading while retaining donor sun
angles and height ambient. Native lighting objects over the whole map washed
out some rooms and darkened others; they are no longer substituted.

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

- `v021d-school-fort-environment-1.json`: the atmosphere choice above, on modes
  2, 4 and 8; Blackout uses the battery project.
- `v021d-school-anomaly-1.json`: the stalactite material's native white diffuse
  multiplier, retaining the donor texture.
- `v021d-fort-torch-1.json`: a private torch particle for the Fort, its original
  5.047268 m light radius and shadow settings, with half light power in 0.26.
  The same comparison changed from mean luma 37.9–39.6 to 28.8, against 28.8 in
  the donor. The native shared torch is unchanged.
- `v021d-school-lighting-1.json`: the hallway ambient intensity 3.9 to 1.5 and
  the ruined-room pair 2 to 1/1.1 (the pair's native 0.26 intensities), retaining
  positions, radii and colours. The hallway comparison is 49.0 vs 45.7; the
  ruined room is 58.4 vs 59.0. Nearby earlier views are rechecked after a change.

`content_edit.py` preserves float2/3/4 dimensions in `set_vector`, checks old
values, and rejects a dimension mismatch. The torch's float2 curve keys and
material's float4 colour use this operation instead of replacing value types.

## Review artifacts

The comparison rig keeps the donor shots permanently and captures only the
port, one pose at a time. Captures use headless displays and disabled audio,
including an ALSA null device; the running clients are checked for audio streams.
The report shows the donor at left, port in the middle and amplified difference
at right. Automated flags remain visible beside inspection notes. Native sky,
sun shadows and atmosphere can produce intentional differences.

Local report paths (the owner's final review is still pending):

- `/home/sheep/Projects/survarium/v021d-rig-data/reports/step/school_day/index.html`
- `/home/sheep/Projects/survarium/v021d-rig-data/reports/step/fort_day/index.html`

These are local quick-build previews. They are not release artifacts and do
not set a release revision.
