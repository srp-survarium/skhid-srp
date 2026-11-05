# Medical skill tree

Source: `scripts/content-edit/medical-skills-1.json`, listed in `release/content.tsv`.
The third skill tab has 38 points: seven skills with five ranks and three unlocks.
Thresholds count points in earlier tiers of this branch. Tier 5 uses 25, within
the existing menu's unlock limit. Implemented and tested locally; not released. The owner played the near-ally
Larkspur crash correction. Final duration and restore corrections have automated
coverage, without a completed live multiplayer synchronization check.

| Tier | Required points | ID | Skill | Per rank / unlock | Maximum |
|---|---:|---:|---|---|---|
| 1 | 0 | 301 | Potent Painkiller | Existing protection +5%; duration +1 s | +25%; +5 s |
| 1 | 0 | 302 | Fast Bandaging | First-aid kit and bandage use time −2% | −10% |
| 2 | 5 | 311 | Metabolism | Regeneration starts 0.5 s sooner | 2.5 s sooner |
| 2 | 5 | 312 | Medical Training | First-aid kit and bandage healing +5% | +25% |
| 3 | 10 | 321 | Adrenalin | Movement +3% below 20 body health | +15% |
| 3 | 10 | 342 | Brothers in Arms | Incoming allied damage −10% | −50% |
| 4 | 20 | 331 | Supply Pocket | Secondary weapon replaced with supply pocket | One rank |
| 4 | 20 | 332 | Strong Bones | Arm and leg maximum health +10% | +50% |
| 4 | 20 | 322 | Additional Medicine | One extra medicine per medicine pocket | One rank |
| 5 | 25 | 341 | Medic | Larkspur active effect shared within 5 m | One rank |


Current equipment limits (owner clarification): **six quick slots total**, shared
between supply pockets and artifacts. Six supply pockets leave no artifact slot;
four supply pockets leave at most two artifact slots. Each supply pocket holds
**at most five items after all bonuses**, including Additional Medicine. Storage
stacks and ammunition quantities are not restricted to five.

The four-column layout, from tier 1 down to tier 5:

```text
| - x x - |
| x - - x |
| - x x - |
| - x x x |
| - x - - |
```

Tier 2 places Metabolism in column 1 and Medical Training in column 4.
Tier 3 has Adrenalin and Brothers in Arms; tier 4 has Supply Pocket, Strong
Bones and Additional Medicine; tier 5 has Medic alone in column 2.

Painkiller protection strengthens the existing effect relatively. The stock
coefficient 0.9 means 10% protection: at rank 5 it becomes 0.875, or 12.5%
protection, for 15 instead of 10 seconds. It does not add 25 percentage points.
The English Medic description ends exactly: `"They glow, you shine"`, including the quotes. All ten supported languages
and the original Russian table contain medical names, descriptions and a quoted
translation of this ending.

Supply Pocket excludes both Terminator (111) and the existing shooting Supply
Pocket (113); the exclusions are symmetric. Medicine capacity uses category 10,
the native medicine category, excluding whole-pocket items (the boosters).
Whole-pocket items remain one; the server applies the same cap to moves, preset
fills and account repair without deleting inventory stock. The menu uses committed skills, never an unsaved preview. Server reset
validation obtains available branches from the loaded tree instead of assuming
two tabs. The stock movie contains only two icon sets; `MedicalSkillIcon.as`
now displays ten original 0.69d survival PNGs for branch 3 within the existing
skill renderer. Their bytes are unchanged in `flash/lobby_menu/bitmaps/`;
`scripts/medical-skills/icons-069.tsv` records the donor paths, mapping and SHA-256.
The injector artwork represents Potent Painkiller; the original heal-ally
artwork represents Medic. The remaining eight use their corresponding 0.69 skills.

## Native implementation

`rust/patches/src/medical_skills.rs` contains safe rank parsing and calculations;
`vostok/medical.rs` owns the native object boundary. `medical_skills.toml` declares
patch 94's hooks; this is a code patch ID, not a release revision. The generated
engine library supplies every new call. Fields and sites are in their TSVs.

The unchanged 68-float modifier block transports the ten ranks as an exact
24-bit integer in unused `armored_trap` slot 43. Rank bit offsets, in table order,
are 0, 3, 6, 9, 12, 15, 16, 17, 20 and 21. Five-rank fields use three bits;
unlocks use one. The parser refuses fractional, nonfinite, out-of-range or
rank-6/7 values. Only the base profile value is decoded. Do not repurpose this
slot for gear or a trap feature. Healing uses native modifier 57; the supply
pocket uses native modifiers 15 and 64. Fast Bandaging is filtered to healing
medicines at the native use-time site, without altering global modifier 33.

Painkiller duration is adjusted at start, tick, deserialize comparison and HUD
presentation. Native remaining-time fields remain the sole timer state. Limb
maximum health and regeneration delay are derived once after the constructor
binds the model's owner (site 0x2D706F). Native respawn reset restores that maximum;
loading current health does not multiply it again. Adrenalin reads normalized
body health; native damage and healing request movement selection again.

Medic runs at actual artefact activation (0x313333, ESI item, EDI simulation
time), after native enable/effect and before the activation timestamp/state
assignment. It obtains the source from item+250h / inventory+174h, reads its
simulation core, then visits active places in order. The visual effect helper is
also invoked by deserialize and must never perform this gameplay healing.
It excludes self, enemies, dead/inactive players and targets outside 5 m.
Each successful native lookup is an owned reference released on every exit.
For recipients it removes the Larkspur config's affects, fills body/head/pain
like patch 54, and adds a native effect instance with the recipient's calculator.
The source's normal activation and cooldown continue unchanged. Brothers in Arms
uses the same core lookup and team rule, after spawn invulnerability and before
native body-part damage processing. Environmental and self damage are unchanged.

## Effect ownership, capacity and duration

Each recipient receives an independent native effect from the source resource.
The original attempt reused mutable effect state and crashed during restore;
independent emission alone still exhausted the particle pool on activation.
The `artefact_lifebone` cook hook at 0x2EC81E requests at least 20 instances before
its asynchronous child cooks allocate their pools. Other resources are unchanged.
The retained crash dump has ten loaded Larkspur objects with distinct resources
and emitters, supporting a per-artefact pool budget. The owner confirmed the
corrected pool no longer crashes on activation near allied bots.

The source calculator freezes the resource's one-second animation until the
artefact ends. Shared effects now use a permanent stateless native calculator
and copy the activation duration into their existing length field. Their native
node stores the activation time. No player/item pointer is retained. The engine's
local visual rollback stream serializes the existing length/start and permanent
calculator pointer; no authoritative gameplay field is added. Ordinary clocks
continue unchanged. This local visual format is not an interchange format between
different executables.

The retained dump reports two 0x310-byte particle-instance allocations per loaded
resource. Raising each pool from 1 to 20 adds 29,792 bytes of records plus 304
bytes of free/active pointers: 30,096 bytes (29.4 KiB) per loaded Larkspur resource.
This excludes nested renderer allocations, allocator overhead and playing nodes.

## Validation and review

Automated checks cover all ten skills; not all ten have been played manually.

- `check_medical_skills_026g.py`: ranks 0–5; start/tick/restore durations;
  healing-only use time; native protection; limb health and repeated respawn;
  regeneration; movement threshold and damage/heal refresh; friendly fire;
  team/life/radius filtering; reference balance; independent effects; six shared
  slots and five-item capacity. Outcomes match across two independent heaps.
- Native particle regression: all 20 free-to-active claims at the original crash
  site, arbitrary-order returns and reuse. Rendering is substituted.
- Clock regression: 3/5/6.5-second expiry, timestamp wrap, native length and clock
  serialization, local pointer decoding/restored expiry, unchanged normal clocks.
- Review found that the original visual hook could heal on deserialize. The
  corrected activation hook passes regression checks that visual reconstruction
  never heals and absent visual resources do not suppress gameplay healing.
  Independent source/native-code review found no further blocking issues.
- Rust rank/effect tests and affected patch-crate Clippy pass under Nix/i686.
  Server reset/equipment/repair/preset and bot-loadout tests, plus affected-crate
  Clippy, passed for the medical feature before the final native-only correction.
- `survarium/tests/medical_skills.rs` parses the content-built tree, validating
  progression, layout, 38-point total, exclusions and native modifier transport.
- CPU Ruffle exercises the built lobby: medicine and whole-pocket capacities,
  drag limits, unsaved previews, reset, combined slot bounds and the ten original
  0.69 bitmap dimensions/alpha/bounds. Embedded bitmap pixels match the sources.
- Full manifest builds on base 2480a68b3 passed with 18,213 files. The final
  pre-review local patch changed only seven localization files. Latest-main
  integration and four additional locale translations passed the fresh build
  recorded below.
- World round-trip checks passed for eight corpus worlds on fresh A/B and resync
  fixtures; three snapshots from the crashed match passed the earlier ownership
  correction. These run patched codec hooks, not a whole rendered native world.

The last owner-requested local launch was 2026-11-03 19:09:30 UTC with the
retained level-100 CamoBob profile and PvB enabled. Logs confirmed lobby sign-in
and DXVK 2.7.1/Vulkan on the RTX 3060 Ti. That executable was
`1cfb1b387724f64ad6251671f498a7000d0db94d84da13b0ea1712d47cec4c83`, before the
activation-only review correction. Source REVISION remains 44; local revision
45 builds do not reserve a release number. Final glow duration, live two-client
synchronization and release are not claimed by the automated evidence.

Final integration with main b4f6ab938: full manifest build passed for 18,773 files,
with 652 new/changed files (including main's reload and Blackout content). Patch
SHA-256 `fc11460688e72f06faa5ea8f4a12b1858e7ebc467353b2e374eff3f348d5de91`;
server data `a0fa6a2379213c3fb32180373ee262d8662520ca28cb822a6439347f73825125`.
The generated skill tree integration test and compiled lobby Ruffle harness pass.
After the rebase, 38 session rules, 10 account repair, one simulation-list and 15
bot-loadout tests pass, as does session/survarium Clippy. The reviewed executable
passes all eight world round trips on fresh A/B and resync fixtures.

The broader `selftest_026g.py` passed arena, engine ABI, hook/call-shape and
cert-only structural checks before stopping at its Windows-only certificate
check: Linux Python has no `ctypes.WinDLL`. It did not complete its remaining
variants; this is not reported as a full self-test pass. The new lobby source
movie is `eb2ba8b7c606f5978095225a8784585d015d8d5df5edfc110a45c8501aeea0d9`,
not `RESULT_SHA256`; its latest-main changes still need an owner gameplay check
before publishing. No release has been made.


## Revision 45 follow-up (2026-11-03)

The owner reported Medic failing between two human allies in live match
`0x2d034412`. Read-only logs/recording confirm both on team 0 and revision 45;
the account read confirms skill 341 selected. Match logs contain resyncs; their
cause has not been established from those logs.

The direct failure was an activation-path bypass. The existing `fire_kind`
detour at 0x313300 activated each ready artefact in arena code, copying native
enable/effect/timestamp/state instructions. The review moved Medic to 0x313333
inside native action, so the normal pressed path never reached it. The fix
calls the existing original-action bridge from each ready artefact's activation.
This preserves multi-artefact activation and Onyx timing, reaches Medic once per
actual activation, and keeps deserialize/visual reconstruction free of healing.

The real-entry regression starts at the detoured 0x313300, runs native Lifebone
activation and checks an ally's health. The exact revision-45 EXE leaves it at
0.1 instead of 1.0. This test also covers multiple Larkspurs, repeated presses,
key-up, and pressing an active artefact while another of its kind is ready.
Production recording E4 profiles contain Medic bit 20: packed modifier 43 is
1,792,557 initially and 12,311,405 later, in all three kits. The account's later
skill list alone was not evidence of the earlier profile, so both were checked.

A separate correctness fix removes the recipient requirement for player+33Fh
(`inserted`). Native remove clears it at 0x2DA79D and insert sets it at 0x2DA869.
It is not serialized life state: the codec writes/reads +33Ch, +33Dh and +33Eh,
not +33Fh. The retained old crash dump has a living active player temporarily
removed during deserialize; that does not prove an ordinary activation saw it
clear. Medic now uses the active roster, serialized alive state, team and
position; fixtures check identical choices regardless of temporary insertion.

These tests establish the bypass and the unnecessary flag dependency; they do
not prove that every live resync was caused by Medic. A fresh two-human match
remains the final played confirmation. The change is not released.

Validation of this follow-up: native real-entry regression, all three kits and
independent heaps, 20-effect particle pool/clock checks; existing artefact
regressions (85 stock, 84 patched); affected Rust tests/Clippy; session booster,
pocket, preset and repair tests; CPU Ruffle medical UI and HUD notice checks.
Eight snapshots from the reported match round-trip through the patched codec
hooks with two fresh heaps and a resync heap. This is not a full native world
simulation. The global localization audit has the same pre-existing medical
catalog/manifest findings on released source and this branch; all ten language
catalogs still have zero missing, extra, empty, placeholder, markup or UTF-8
errors. The complete content manifest build is checked separately.
