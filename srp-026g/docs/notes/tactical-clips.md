# Tactical reload for the one-clip guns

The owner (2026-11-01): a tactical reload for the live guns that reload with one animation, "the
way the live vz.58 and the other 0.29 guns already do it": data and clips only, no exe change.
Packs `tactical-clips-029` (0.29a3's clips, preferred: the patch-19 donor, same rig) and
`tactical-clips-069` (0.69d0's, for guns 0.29a3 has none for); resource-porter
`port-tactical-reload` (`src/weapons_026/tactical.rs`, manifests `ports/tactical-reload-029a3.json`
and `ports/tactical-reload-069d.json`); `scripts/tactical-clips/make_pack.sh`.

## The rules

- **The reload** is 0.26g's two-clip one, state type 314 (`pistol_weapon_core_reload_state`), as
  the vz. 58's: with rounds left in the magazine the tactical clip plays, without them the empty
  one (get_weapon_lexeme 0x30FE90, slot `2 x (ammo_in_magazine == 0) + 4 x crouch`). Hands clips
  `[reload_tactic, reload] x 4`, item clips `[reload_tactic, reload]`.
- **The clips** are one donor build's pair, tactical and empty, so the two keep that build's
  lengths against each other. A donor clip at a path 0.26g has with other bytes is shipped at
  `<path>_029a3` / `<path>_069d0`; the live clip of that name stays for anything else.
- **The times:** the engine plays both clips at one speed, the tactical clip's length over
  `reload_time` (the ctor 0x30FC50). `reload_time` stays: the tactical reload lasts it, as the
  one reload did; the empty one lasts `reload_time` x (empty clip / tactical clip).
- **The sounds:** the donor's reload sound lists (its clips' sound events index them). Every
  converted gun's are the live ones already.
- **Which guns:** a live 303 gun whose donor reloads with two clips, and whose donor clips fit:
  the hands clips drive 0.26g's player rig (70 bones) bone for bone, the item clips the bones the
  gun's live reload drives, and at each clip's start and end every part of the gun sits in the
  hands where 0.26g has it at rest or in its own reload (5 mm, 0.02 rad; the porter checks it).
  Accepted by name in the manifest: the AKs' top round (`patron`, 1.3 cm), the Uzi's bolt
  (`zatvor`, 7 mm at the tactical clip's end).
- **Changes the simulation** (the reload's length): `simulation_changes.tsv`.

## 0.29a3 (pack tactical-clips-029@1)

0.29a3 reloads these with two clips (its configs: state 314, the same sound lists as ours):

| weapon | tactical (= reload_time) | empty, now | clips (frames) |
|---|---|---|---|
| AK-74M, AK-74N (+premium), AKS-74U (+premium), AKMN (+2 premiums), RPK | 3.167 | 4.37 | `ak/reload_tactic` 145, `ak/reload_029a3` 200 |
| Uzi (+2 premiums) | 2.3 | 3.35 | `uzi/reload_tactic` 55, `uzi/reload_029a3` 80 |
| AS Val, VSS (+premium) | 3.167 | 4.47 | `val/reload_tactic` 85, `val/reload` (hands, ours) / `val/item/reload_029a3` 120 |

Before, every reload lasted `reload_time`. The VSS's model has a `magazin2` the Val's clips do not
drive: as its live reload.

## 0.69d0, for the guns 0.29a3 has none for (pack tactical-clips-069@1)

0.69d0 reloads these with two clips (its 313, 0.26g's 314); its reload sound lists are ours:

| weapon | tactical (= reload_time) | empty, now | clips (frames) |
|---|---|---|---|
| AK-12 (+premium) | 2.8 | 3.71 | `ak_12/reload_tactic` 120, `reload_069d0` 159 |
| A-545 | 3.167 | 3.96 | `a_545/reload_tactic` 80, `reload_069d0` 100 |
| ASh-12 | 3.167 | 4.67 | `ash_12/reload_tactic` 122, `reload_069d0` 180 |
| 9A-91 | 3.167 | 4.47 | `a9_91/reload_tactic` 73, `a9_91/reload` 103 |
| VSK-94 (+premium) | 3.367 | 4.75 | the 9A-91's |
| PP-2000 | 2.8 | 3.43 | `pp2000/reload_tactic` 155, `reload_069d0` 190 |
| PP-19 Vityaz | 3.367 | 4.57 | `vityaz/reload_tactic` 140, `reload_069d0` 190 |
| PP-19 Vityaz premium | 3.167 | 4.30 | the Vityaz's |

0.69d0's item clips carry a `shoot` event channel (its fire clips' shot): only the fire state
listens to it (weapon_core_fire_state_base::initialize 0x30E9B3 subscribes, its finalize 0x33AA40
unsubscribes), so in a reload it is heard by nothing. Accepted by name: the Vityaz's fire selector
(`safety`, 10 degrees from our idle), the ASh-12's safety catch (25 degrees at the tactical clip's
end, as our own reload starts it). Reported: the hands' fingers and forearm roll bones apart from
our reload at the clips' ends (the Vityaz's forearm rolls); worth a look in the game.

## Not converted

- 0.69d0 moved the gun in the hands: its hands' `Weapon` bone carries the static placement our
  clips keep in the gun's `offset` bone (the same numbers across the gun root's half turn about y),
  so its reload would slide the gun at the start and end: the SR-2M, AEK-919K, Vepr (+premium)
  (13 cm), UMP-45 (14 cm), PPSh-41 (6 to 9 cm), SR-3M (+premium) (the Val's clips, 5 cm).
- One clip in 0.29a3 and 0.69d0 too: the PKP, RPD, VSSK (+camo), TOZ-122, Magnum (a revolver).

## Slower-reload report and lobby timings (2026-11-03)

Compared the stock resources.db with the cached release patches through revision 42 against
revision 43 (`cac1ca042`), reading the weapon configs and the actual `.b-spline` clips. All 25
variants converted from state 303 to 314 retain their exact `parameters/reload_time` float.
The tactical reload therefore retains the old single-clip duration; empty reloads are longer.
Examples, before weapon rolls or player reload-speed modifiers:

| Weapon | Before, either reload | Now, tactical | Now, empty |
|---|---:|---:|---:|
| AK-74M | 3.167 s | 3.167 s | 4.368 s |
| AK-12 | 2.800 s | 2.800 s | 3.710 s |
| Uzi | 2.300 s | 2.300 s | 3.345 s |
| AS Val | 3.167 s | 3.167 s | 4.471 s |
| ASh-12 | 3.167 s | 3.167 s | 4.673 s |

This is a data/disassembly result, not a timed playtest. It confirms an empty-reload slowdown
of 22.6–47.5% across these conversions; it does not reproduce a tactical-only slowdown. The
introduction kept the old time for tactical reloads instead of keeping the old empty time and
making tactical reloads faster. No timing/balance change is made by this UI correction.

Where the times live:

- The editable source is now `scripts/content-edit/weapon-stats.tsv`, column `reload`.
  The content build writes it to `gameplay/items/weapons/wpn_<weapon>.options`,
  `parameters/reload_time`, inside resources.db and its patches.
- That config's `states/reload/state_type` selects the reload implementation. State 303 has
  one hands clip; 314 has `[tactical, empty]` repeated four times in `user_animations`, and
  two item clips in `animations`. The named `.b-spline` files under
  `animations/single/weapons/` hold the clip time domains, in frames at 30/s.
- Verified in the original exe: constructor 0x30E000 passes the first hands resource
  (`resources+4`) to 0x30D750 for state 303; 0x30FC50 passes `resources+8` for 314 (after
  its two item resources). That helper computes `(end_frame - start_frame) / 30 / reload_time`.
  Both store this animation speed at state+138h. At 0x30FE90 the hands selection is
  `2*(ammo_in_magazine == 0) + 4*(posture == 1)`; both clips use that stored speed,
  multiplied by player modifier 13 at 0x30FF07. There is no independent tactical timer.
  Thus tactical = `reload_time`, empty = `reload_time * empty_frames / tactical_frames`;
  weapon rolls multiply both times, while player reload-speed bonuses divide both times.

The lobby previously displayed `reload_time` as simply “Reload”, including on two-clip weapons,
so it understated their empty reload. `weapon-details-ui-1.json` now transports property 162,
the actual empty/tactical hands-clip duration ratio, only for state 314. Its read-only `_inputs`
make build-content supply the composed animation files. Missing clips, invalid durations or
different durations between stances/views stop the build. There is no weapon-name table.
The Jackhammer and SVU-A reuse the same animation in both slots: they do not get a tactical
row. Equal duration alone does not suppress it: Kedr and Kiparis use distinct clips, both
lasting 2.5 s. The SKS and its two premiums already used state 314 before revision 43;
at revision 43 their tactical reload was 5 s and empty reload about 3.219 s (clip ratio
159/247), so the UI does not assume tactical is faster. The requested retune is below. Property 160 remains chambering time and 161 the kill-feed icon.

The card shows “Reload (empty)” and “Tactical reload” for those weapons. Comparing one with a
single-clip weapon reserves the tactical row on both cards, with `-` and no comparison colour
for a missing value. When neither weapon supports tactical reload, the row is omitted.
Weapon rolls, held Alt's kit and skills, and Ctrl's neutral colours apply to both times. The captions use all ten language
dictionaries. The UI portion is presentation-only; the balance follow-up below changes
simulation behavior. Neither needs an executable patch.

Validation: the original exe constructors, normalization helper and clip selector were read in
objdump; the archive audit found 25 unchanged base times and 70 valid two-clip variants (all
stance/view pairs agree). `test_reload_ratio.py` passes, covering nonzero time-domain starts,
unsupported states, missing clips and invalid/inconsistent durations. The lobby compiles with
Flex, and `WeaponDetailsHarness` reports `DETAILS RESULT pass`, including mixed comparisons in
both directions, ten languages, compact/expanded cards, weapon rolls, Alt effects and Ctrl.
The initial UI edit, before the explicit timing retune, was checked against build-content's composed inputs: 104 configs checked with
zero unexpected differences, property 162 added to 68 distinct-clip state-314 weapons and
omitted for the two identical-clip weapons and 33 other reload states. AK-74M transports
138 = 3.167 and 162 = 1.37931034.
No timed in-game playtest or release has been performed for this change.


## Other original game versions

The owner's follow-up compares the **new tactical reload with the old single reload**;
there was no tactical variant on those 25 converted weapons before revision 43. It is not
an old-tactical versus new-tactical comparison.

[tactical-reload-versions.tsv](tactical-reload-versions.tsv) records the actual config fields
for 17 relevant weapon variants across eight builds. 0.26g was read from the stock archive;
0.29a3 directly from its donor archive; 0.27d0, 0.28a2, 0.28d0, 0.31a2 and 0.46c2 from the
local weapon-survey's decoded config leaves; 0.69d0 directly from the tactical port's
`donor-reload.db`, `gameplay/game.options`. These are base config values, not measurements
with upgrades, skills or equipment. The port copies the clips but keeps 0.26g's timing values.

Earlier builds' `reload_time` field (seconds): the reload state and animation lengths still
matter, so these are not all empty-reload durations. The TSV includes the reload state ID;
there is no separate `tactical_reload_time` in these sampled early weapon configs.

| Weapon | 0.26g | 0.27d0 | 0.28a2 / d0 | 0.29a3 | 0.31a2 |
|---|---:|---:|---:|---:|---:|
| AK-74M / AK-74N / AKS-74U | 3.167 | 3 | 3 | 3 | 3 |
| AKM | 3.167 | 3 | 3 | 3 | 3 |
| RPK | 3.167 | 3.2 | 3.2 | 3.2 | 3.2 |
| Uzi | 2.3 | 2.3 | 2 | 2 | 2 |
| AS Val | 3.167 | 3 | 3 | 2.5 | 2.5 |
| VSS | 3.167 | 3.167 | 3.1 | 2.5 | 2.5 |
| AK-12 | 2.8 | 2.8 | 2.8 | 3 | 2.2 |
| A-545 | 3.167 | 3 | 3 | 3 | 3 |
| ASh-12 | 3.167 | 3.2 | 3.2 | 3.2 | 3.2 |
| 9A-91 | 3.167 | 3.167 | 3.167 | 3.167 | 3 |
| VSK-94 | 3.367 | 3.367 | 3.367 | 3.367 | 3.5 |
| PP-2000 | 2.8 | 2.8 | 3 | 3 | 3 |
| Vityaz | 3.367 | 3 | 3 | 3 | 3 |
| Vityaz premium | 3.167 | 3 | 3 | 3 | 3 |
| SKS | 5 | 5 | 5 | 5 | 5 |

By 0.46c2 and 0.69d0 the configs explicitly store both `tactical_reload_time` and `reload_time`.
The following cells are **tactical / empty**, in seconds. The “current” column is computed
from our actual mounted clips and 0.26g engine normalization, before modifiers.

| Weapon | Skhid before this change | 0.46c2 | 0.69d0 |
|---|---:|---:|---:|
| AK-74M / AK-74N / AKS-74U | 3.167 / 4.368 | 2.4 / 3.4 | 2.4 / 3.4 |
| AKM | 3.167 / 4.368 | 2.6 / 3.7 | 2.6 / 3.7 |
| RPK | 3.167 / 4.368 | 3 / 3.5 | 3 / 3.5 |
| Uzi | 2.3 / 3.345 | 2.6 / 3.5 | 2.6 / 3.5 |
| AS Val | 3.167 / 4.471 | 2.5 / 3.5 | 2.5 / 3.5 |
| VSS | 3.167 / 4.471 | 2 / 2.5 | 2.5 / 3.5 |
| AK-12 | 2.8 / 3.71 | 2.4 / 3.2 | 2.4 / 3.2 |
| A-545 | 3.167 / 3.959 | 2.2 / 3 | 2.6 / 3.6 |
| ASh-12 | 3.167 / 4.673 | 2 / 2.8 | 2.8 / 3.2 |
| 9A-91 | 3.167 / 4.469 | 2 / 2.4 | 2.4 / 3.2 |
| VSK-94 | 3.367 / 4.751 | 2.8 / 3.5 | 2.6 / 3.5 |
| PP-2000 | 2.8 / 3.432 | 2.1 / 3 | 2.1 / 3 |
| Vityaz | 3.367 / 4.57 | 2.5 / 3.5 | 2.5 / 3.5 |
| Vityaz premium | 3.167 / 4.298 | 2.5 / 3.5 | 2.5 / 3.5 |
| SKS | 5 / 3.219 | 3.2 / 3.6 | 5.7 / 3.7 |

Examples of the donor/current difference: our AK-12 tactical reload is 2.8 s versus 0.69d0's
configured 2.4 s; our Val and VSS are 3.167 s versus 2.5 s; our AK-74M is 3.167 s versus
2.4 s. Keeping the old 0.26g time explains these differences; it does not establish a
slowdown against the old 0.26g single reload, whose time is unchanged for these conversions.

## Explicit timing source and the requested 0.69 balance

The owner selected 0.69d0's tactical/empty pair for the weapons in the comparison above,
including the SKS, and retained the port's AK-12 pair (2.8 / 3.71 s). The table now authors
28 variants including premiums: the 25 converted weapons plus three SKS variants. Other
weapons retain their current timings. These are base times; upgrades and skills still apply.
The SKS's 5.7 / 3.7 pair deliberately follows the donor even though tactical takes longer.

`scripts/content-edit/weapon-stats.tsv` stores `reload` (native `parameters/reload_time`,
tactical for state 314) and `reload_empty` (`parameters/reload_empty_time`). The following
`reload-timings-1.json` manifest edit compiles independent pairs into stable, private clip
paths. It normalizes tactical hands to `30 * reload` frames and empty hands to
`30 * reload_empty` frames. Each corresponding item clip uses the hands' scale, preserving
its relative timing. Every bone curve's knot and every event-channel knot gets the same
scale; values, channel types, domains, names and bone bindings are preserved. This keeps
motion, sound triggers and animation completion together. Shared donor clips stay exact.

The native state still normalizes its first clip to `reload_time` and applies one playback
speed to both. With these cooked clips that produces exactly the authored pair (within
float precision), including the native modification and player reload-speed multipliers.
The UI's ratio is refreshed after cooking so property 138 and tactical row 162 describe
those same durations. The server's bot facts parse explicit pairs as a separate enum
variant; they replace both its old five-weapon constants and its guessed 1.25 empty ratio
where a pair exists. No executable changes are required. A `next` simulation row records
the timing and bot behavior change; the earlier presentation-only section describes the
UI portion before this balance follow-up.

Retune the table, not animation assets. The content build always starts with the donor
clips and refuses to retime a previously generated copy. Generated paths are stable across
future edits, so changing a duration replaces the same resources rather than leaving old
generated files behind.

Final validation (2026-11-03): 8 reload metadata/retiming tests and 35 content-editor tests
pass. Under `nix develop`, 7 bot-equipment tests and 242 match-server bot tests pass (17
existing tests ignored); Clippy for `survarium` and `match-server`, all targets, passes with
`-D warnings`. The unchanged lobby implementation passed its Flex build and Ruffle harness
(`DETAILS RESULT pass`), including all ten locales and missing-row comparisons.

`release/build-content 255` completed on main base `2480a68b3`. All 18,772 manifest files
pass type checks and match when read through the built patch over the players' archives. The local artifact has 650 changed/new files, 92,216,681
bytes, SHA-256 `74f2b9f5f8a6363ddaadd317cd6509185554871920d8bcaf39789904c8eae537`;
it also includes the manifest's other pending changes. 255 is a local test number, not a
release or a reserved revision. The resulting server game data passes the item-table tool's
startup validation. Reading the packaged configs and clips back verifies all 28 pairs and
both UI values within 1e-6 relative error; the largest retuned config has 513 entries (limit
565). This is built and checked, not played or released.

Local startup follow-up: combine the pending SCAR ammunition, medicine and reload changes in
one `next` row, as the replay parser requires. The 22 `session` replay tests pass, and the
PR server starts with its built game data. The isolated client and server use local test
revision 255 to match the test patch; the repository release revision stays unchanged.
