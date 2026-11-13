# STALKER FFA maps and Training Camp's crater arena

Experimental Pool, Military Warehouses and Training Camp each have 24 authored
FFA spawns. Training Camp keeps players inside the crater, including spawns on
two raised islands, and gives its FFA spawns 500 ms of protection. Pool and
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
| Training Camp | `v010b_cs_training_camp/day_4` | 212 / 44 | 17 / 33 | 16.16 m |

The remaining ports use levels 18–20 and pictures 34–36. Rudnya keeps level 14
and picture 30. The mode tables, movie classes, minimaps and ten localization
catalogs agree with these IDs. The Keeper pool includes every new project;
dictionary append-count guards include the six TDM registrations.

## Source edits and regeneration

`scripts/stalker-maps/ffa-spawns.tsv` contains positions and engine rotations.
`make_ffa_edits.py` generates `content-edit/ffa-stalker-1.json` over the source
pack and a composed dictionary that already contains its six TDM rows. The
following `ffa-stalker-radius-1.json` retunes all 72 FFA points to a 20 m enemy
radius. They have a 1 m ally radius, priority 1 and empty team collision zones.
Pool and Military Warehouses keep two seconds of spawn protection.

The FFA configs copy their final TDM configurations after table retunes, use
mode 8 and rules 93/91/94 without team rule 90, and set 30 kills and at most ten
players. This does not change main's spawn-selection algorithm, including its
five-candidate fallback when no point passes the clear test.

From `srp-026g`, using the dev shell:

```sh
cargo run --release --manifest-path scripts/stalker-maps/ffa-points/Cargo.toml -- \
  --db "$STOCK" --patch "$VALIDATED_STALKER_CONTENT" \
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
All 72 authored points passed these checks; the new crater layout was selected
from 1,173 safe candidates.

## Crater boundary and islands

`training-arena.tsv` is the reviewed crater outline. Training candidates and
their facing destinations stay at least 2 m inside it. This replaces the former
45 m distance-from-TDM rule, which admitted the surrounding forest.

Training points 8 and 19 are on the raised islands:

- Point 8: `(30.435001, 21.321009, -23.845001)`.
- Point 19: `(38.685, 16.221554, 6.154999)`.

`training-camp-arena-1.json` copies the stock native level-border model through
the content editor and replaces its primitives with 24 overlapping boxes along
the rim. Each spans y=-30..50, with 0.75 m overlap at each end. A walker-only
collision object references it (`cmask=4`, `cgroup=2`); bullets are unaffected.
All 78 existing collision objects remain unchanged. The boundary also contains
all 24 original TDM spawns. This is game content, not a server-side map table.

## Millisecond protection

The crater edit sets `invulnerability_time_ms = 500` on Training Camp's 24 FFA
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
