# 0.69 firing range

Owner request, 2026-11-15: port the actual 0.69 firing range and make it a map
selectable in rooms. Attachment UX is a separate proposal, not part of this port.

`port.json` is the reproducible source for `range-069@1`. It uses resource-porter
commit `9863002dd8a734c24c53bff587af6f3e5af4982e` (the reviewed 0.69 map converter),
the complete 0.69d split-volume set, and stock 0.26g. Both input fingerprints are
checked by the converter. `build.sh` builds the bundle and content pack, extracts
the original UI pictures and runs `ui.py` (Python with Pillow). A separate packer
argument names the current release content-pack tool.

Registration: map 250, level 21, loading picture 37; project
`v069d_shooting_range/day_4`, HUD key `shooting_range`. The scene keeps the donor's
nine respawn points (one team-0 player point, eight team-1 points), fixed target
models, collision and resupply area, using native 0.26 Team Deathmatch rules so
it works through the existing room flow. This does not port 0.69's private
practice launcher or its bot patrol controller.

The donor project is `projects/shooting_range/test/client_project`, with its
`enviorment/probes/global` cube and all supported local lighting volumes. The
native template is Vostok's day project: it contains the point-light template
the range needs. Imported resources have their own `v069d_shooting_range`
namespace. Geometry is not decimated. The first successful bundle has 112
render models and uses 11.6 MiB of static render geometry, including 0.4 MiB
of terrain.

The loading picture is the original 1920×798 PNG. The original minimap has
world half-extents 100×90 m centred at X=-38, Z=4. `ui.py` pads its vertical
world extent to 100 m for the existing square 0.26 minimap transform; it does
not stretch world positions. Source pictures are pinned by SHA-256.

The converter's `port-manifest.json` lists conversion limits. Disabled anomaly
and artefact demonstration objects stay disabled/absent. Later sound-environment
volumes, ambient object sounds, camera tracks and unsupported leaf/crow particle
effects are not carried by this converter. Its native sky/post-process and
converted materials/lighting require visual review against the donor.

Compose through `release/build-content`; never construct a resource patch from
the bundle directly. Native play and the visual sweep have not yet been done.

The new pack precedes the mode edits. Their `maps_dict` append checks therefore
expect one more existing row (`blackout-mode-1`, `ffa-1`, `ffa-ported-1`,
`v011e-vostok-modes-1`). Only those counts change; their IDs and operations stay
the same. `maps.tsv` supplies ordinary spawn protection and resupply settings;
`modes.tsv` supplies the native TDM timings and the new level/loading IDs.
The range also receives the ordinary hidden Keeper skill pool through
`artefact-keeper-1.json` (80 original client objects, then the pool).

Checks: `tools/check range-ui localization content-edit`. The range harness
loads both production movies in CPU Ruffle, opens room creation with map 250,
checks its illustrated choice and all ten translated names, and checks the
minimap at the authored world centre and original player spawn. The loading
picture is converted from palette PNG to RGB for the source movie compiler.

## Validation, 2026-11-15

- `range-ui`, `localization`, `content-edit`: passed.
- Full `release/build-content 61 --full`: passed, including resource readback,
  all 63,759 manifest files against the player stack, value types, lamp bindings,
  Keeper coverage and item tables. Local candidate only; no revision is reserved,
  no source revision changed, and nothing is released.
- Candidate: `artifacts/range-069/content-5/resources.db.patch_61`,
  1,299,904,286 bytes, SHA-256
  `ea40d529cb17fe72506a22cd47b350a68b607e171307e25f44ed84e6511fc640`.
  Server data SHA-256:
  `367d0c22c4c4558d821664c90e5a7092225597682d8f0cb4bbe0e188ee7e9db6`.
  The Winchester item-table difference (100 to 94) already exists in the merged
  scopes candidate; this branch changes no weapon stats.
- Existing collision checker, over stock, every published patch and the candidate:
  all nine donor positions have clear bodies and a reachable floor; two stand
  0.148 and 0.220 m above that floor. Positions are preserved. The check's separate
  FFA cover recommendation is not used to move the range's authored spawns.
- Map-data export/readback: nine spawns located, 328,562 walker and 289,960 bullet
  triangles; five sampled team-to-team paths found. Team-0 flood reaches 7/9
  spawn cells; team-1 seeds reach 9/9. This is the donor range, not a new balanced
  competitive layout or a port of the 0.69 bot controller.
- Isolated loopback server: starts with map 250 as its only enabled match map,
  loads the final game data, then stops cleanly. No native client was started.

Logs, collision reports and the prepared native preview are under
`artifacts/range-069/` in the main checkout. Native play and the donor/port visual
sweep remain pending: the owner's game was running when the candidate became
ready. Do not run the capture alongside it (`map-port/SKILL.md`).
