# STALKER FFA maps and Training Camp's crater arena

Experimental Pool and Military Warehouses have 24 authored FFA spawns each,
Training Camp 29. Training Camp keeps players inside the crater, mostly inside
its base's fence and on its raised islands, and gives its FFA spawns 500 ms of
protection. Pool and
Military Warehouses have in-game panorama pictures in room tiles and loading
screens. All original TDM spawn positions are preserved.

This integrates the previously unmerged `stalker-maps@6` pack, which supplies
six TDM ports. The other three are Repair Depot, Station and Depository. Only
the three requested maps receive FFA configurations. Team Deathmatch on all
six is not offered yet (`stalker-tdm-hold-1.json`; `stalker-maps.md`, "Free
for All only, for now"). The pack is immutable:
`pack.json` SHA-256 is
`ceb6ae661dda93e24f62d5c59a5cfba68cc9d4e3cd2e34d2eba1b52d99bd17cb`.
Its source and porter history are in `stalker-maps.md` and
`stalker-maps-handoff.md`.

| Map | Project | TDM / FFA dictionary IDs | Level / loading picture | Minimum horizontal separation |
| --- | --- | --- | --- | --- |
| Experimental Pool | `v010b_cs_mp_bath/day_4` | 210 / 42 | 15 / 31 | 15.52 m |
| Military Warehouses | `v010b_cs_mp_military_1/day_4` | 211 / 43 | 16 / 32 | 39.71 m |
| Training Camp | `v010b_cs_training_camp/day_4` | 212 / 44 | 17 / 33 | 13.42 m; 3D from island points (point 26 is 22.7 m above point 17) |

The remaining ports use levels 18–20 and pictures 34–36. Rudnya keeps level 14
and picture 30. The mode tables, movie classes, minimaps and ten localization
catalogs agree with these IDs. The Keeper pool includes every new project;
dictionary append-count guards include the six TDM registrations.

## Source edits and regeneration

`scripts/stalker-maps/ffa-spawns.tsv` contains positions and engine rotations.
`make_ffa_edits.py` generates `content-edit/ffa-stalker-1.json` over the source
pack and a composed dictionary that already contains its six TDM rows. The
following `ffa-stalker-radius-1.json` retunes all 77 FFA points to a 20 m enemy
radius. They have a 1 m ally radius, priority 1 and empty team collision zones.
Pool and Military Warehouses keep two seconds of spawn protection.

The FFA configs copy their final TDM configurations after table retunes, use
mode 8 and rules 93/91/94 without team rule 90, and set 30 kills and at most ten
players. This does not change main's spawn-selection algorithm, including its
five-candidate fallback when no point passes the clear test.

From `srp-026g`, using the dev shell:

```sh
cargo run --release --manifest-path scripts/stalker-maps/ffa-points/Cargo.toml -- \
  --db "$STOCK" --patch "$PATCH_12" ... --patch "$PATCH_57" \
  --project v010b_cs_mp_bath/day_4 \
  --project v010b_cs_mp_military_1/day_4 \
  --project v010b_cs_training_camp/day_4 --plots "$OUT/plots" > "$OUT/ffa-spawns.tsv"
python3 scripts/stalker-maps/make_ffa_edits.py --pack "$PACK" --dictionary "$DICTIONARY"
python3 scripts/stalker-maps/make_training_arena.py --native "$NATIVE_BORDER"
```

Review generated points before replacing the committed TSV. The planner loads
actual walker geometry and reachable standing floors, checks a 0.38 m radius /
1.8 m standing body, ground within 0.5 m, a six-metre walkable facing and a 2 m
margin from harmful zones. Stored rotations use Vostok's positive-yaw convention.
All 77 authored points passed these checks; the crater layout's first 24 were
selected from 1,173 safe candidates. Points are at least 12 m apart horizontally.
Training Camp then changes on Free for All's own project, `day_8`, which the
layers must give (every listed patch through 57): see below. Rerunning the
planner reproduces Pool's and Military Warehouses' rows and Training Camp's
unmoved ones byte for byte.

## Crater boundary and islands

`training-arena.tsv` is the reviewed crater outline. Training candidates and
their facing destinations stay at least 2 m inside it. This replaces the former
45 m distance-from-TDM rule, which admitted the surrounding forest.

The crater's centre has elevated walkways 15-30 m above its floor: three
towers, a central hub, catwalks between them, and five flat raised platforms,
the islands of `training-islands.tsv` (a box and floor height range each,
reviewed from the planner's standing floors). Training points 8 and 19 of the
first 24 are raised: point 8 `(30.435001, 21.321009, -23.845001)` on the
south-east island, point 19 `(38.685, 16.221554, 6.154999)` on the east tower's
ramp.

After its 24 (on the TDM project, as before), the planner loads `day_8` (no
ammo crates, the crater wall) and changes Training Camp there (the owner,
2026-11-13: "set more spawns on training camp on islands", "it is fine for
players to spawn close there", "remove half spawns on perimeter", "put them in
the center"):

1. The perimeter is every point outside the base's fence (`training-fence.tsv`,
   the octagon read off the walk grid) or within 5 m of the crater wall where
   the wall cuts the fence: 15 points. Going round the crater, every other one
   leaves, and the rest stay evenly spread. With 15 points, two neighbours
   stay; the planner picks the rotation whose longest gap between kept points
   is shortest. Points 2, 4, 15, 16, 18, 21 and 23 leave. Points 1, 3, 5, 6, 9,
   12, 13 and 20 stay, the longest stretch being 47 m (6 to 3).
2. The islands' standing floors are sampled on a 0.75 m lattice with the same
   checks. The planner adds the candidate farthest from every point so far,
   while one stays at least 12 m from all of them. Distance is in 3D when either
   point is on an island, so a crater-floor spawn under an island does not take
   it. This adds five points, IDs 24 to 28.
3. The leavers' IDs move to the centre: candidates inside the fence (crater
   floor, towers, islands), spread by the same farthest-point rule and spacing.
   This is done as a move: each ID gets a new position, and its row in
   `ffa-spawns.tsv` changes. `ffa-stalker-1.json` adds every point at its row's
   position, so IDs stay 0 to 28 with no gap. The removed places exist in
   `day_8` only as earlier versions of those rows. `day_4` is never edited.

| Island | Floor height | Standing area | Points before (rev 57) / after |
| --- | --- | --- | --- |
| west | 22-25 m | 122 m² | 0 / 2: 24 `(4.685, 24.885, -2.845)`, 28 `(-4.315, 22.158, 9.155)` |
| north-east upper | 22-23 m | 68 m² | 0 / 1: 25 `(31.435, 22.507, 33.905)` |
| north-east lower | 18-19 m | 54 m² | 0 / 1: 27 `(28.185, 17.926, 18.655)` |
| south-east | 20-22 m | 115 m² | 1 / 1: 8 |
| east | 21-22 m | 66 m² | 0 / 1: 26 `(50.435, 21.864, -12.595)` |

Removing the perimeter points freed no further island points. The islands'
blockers were the crater-floor spawns beneath them, and the 3D rule removes
those blocks. The south-east island stays at one point, because point 8 is on
it. Point 19 is on the east tower's ramp.

| Training Camp | Perimeter | Centre floor | Raised | Total |
| --- | --- | --- | --- | --- |
| Revision 57 | 15 | 7 | 2 | 24 |
| This change | 8 | 14 | 7 | 29 |

The moved points' new places are 2 `(27.2, -20.8)`, 4 `(42.2, -26.8)`,
15 `(-8.1, -8.8)`, 16 `(24.2, 24.2)`, 18 `(30.2, -5.8)`, 21 `(12.2, -26.8)` and
23 `(30.2, 36.2)`, all on the crater floor. Each is 13.4 m or more from every
other point.

The planner checks all 29 points on `day_8`: floor, body, harmful zones and
stored facing. This caught point 2's first candidate, planned on `day_4` on top
of an ammo crate that `day_8` no longer has.

A quick `build-content 58` changed one file, `day_8`. Read back from the built
patch:

- IDs 0 to 28 are all present.
- The 17 unmoved points are unchanged. The 7 moved ones changed only in position
  and rotation.
- 24 to 28 have 20 m and 500 ms.
- All other `day_8` data is the same.
- `day_4` is byte-identical to live.

The new layout has not been played.

`training-camp-arena-1.json` copies the stock native level-border model through
the content editor and replaces its primitives with 24 overlapping boxes along
the rim. Each spans y=-30..50, with 0.75 m overlap at each end. A walker-only
collision object references it (`cmask=4`, `cgroup=2`); bullets are unaffected.
All 78 existing collision objects remain unchanged. The boundary also contains
all 24 original TDM spawns. This is game content, not a server-side map table.

## Millisecond protection

The crater edit sets `invulnerability_time_ms = 500` on Training Camp's 29 FFA
points, with legacy seconds zero. Original TDM timing is unchanged.

The client hooks `respawn_point_core::load` (0x2F4580) at 0x2F4628, before the
native seconds-to-milliseconds conversion. Safe Rust parses a nonnegative
signed-range integer once, writes the existing +0x20 duration and takes the
generated exit to 0x2F4633. Missing or malformed optional values retain native
seconds behavior. This adds no serialized field. The site, field, generated
hooks, arena lock and re-kit ABI evidence accompany the change.

The content and client hook must ship together. A `next` simulation-change row
records the new points, boundary and timing. No feature revision is reserved.

## Validation and local playtest

`tools/check stalker-maps` checks generated edits against the source pack,
unchanged TDM data, native field types, dictionary preservation, island spawns
and the closed wall geometry, then tests and lints the planner. `build-content`
checks full manifest composition and actual dictionary registration; `ffa-native`
covers the native timing loader and existing FFA/late-join/world checks.

The tested local content was read back from the built archive: exact authored
positions/facings, all 72 IDs, 20 m radii, Training-only 500 ms and unchanged TDM
points. Both panorama JPEGs appeared byte-for-byte in the lobby movie. The
loaded walker mesh blocked 1,200 crossing rays at segment interiors and joins,
at five heights and in both directions; all 48 Training spawns were inside.

The owner played the corrected build in two 1280×720 windows on 2026-11-12 and
approved merge. The initial fullscreen run exhausted available GPU memory;
windowed mode displayed the lobby and match. This establishes a local playtest,
not exhaustive balance testing or a release. Integration onto current main
preserves its newer FFA fallback and lobby changes; its check results are
recorded in the PR.

The completed local Training Camp match is in the private world corpus at
`local/stalker-ffa-crater/rec026g_20261112-202431_2533279d.srprec` (45,082 bytes,
SHA-256 `1a81bf6c94e558b79fec02c98aacbf8bef4af601d7646115eeef2e8603c2de7c`).
The server logged the completed recording after both updated clients entered;
all 15 saved worlds passed the combined-tree server check. The native/host
comparison agreed on all 1,044 sampled worlds and their mutated copies on the
integration with main `4bebbb538`. Main's later executable-size precheck
(`6baf30627`) was integrated and the arena relinked; focused final native and
Rust results are recorded in the PR.

The combined-main quick content build passed (`tools/check build-content`,
key `59385548a82b`). Its archive SHA-256 is
`c430744415f0bcba6e5c6019b2d647e57703d0872ab8f92f30c9ecf6a1035f44`;
server-data SHA-256 is
`0f57d94b704dae55a3598c52cf150aed520e3bb7e718ce09bcd69a4d219c371b`.
Readback and the 1,200 walker-crossing checks also passed on this archive.
These are local test artifacts, not a release.

Two unrelated checks also fail on untouched main (`ffe92a0ba`): the localization
harness lacks Calibri's infinity glyph for the existing pocket-units string,
and HUD-options fails the existing `configs.start` registration assertion.
The same failures were reproduced in a clean detached baseline checkout;
no validation rule was loosened.

The full historical corpus is not green: 22,705 of 44,233 worlds passed and
21,528 were refused, mostly for older respawn counts plus invalid object states.
The additional full baseline comparison was stopped after resource contention;
these corpus failures remain a validation limit, not a claimed baseline pass.
The new Training Camp recording has no refused worlds.
