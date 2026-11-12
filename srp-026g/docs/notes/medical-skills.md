# Medical skill tree

Source: `scripts/content-edit/medical-skills-1.json`, listed in `release/content.tsv`.
The third skill tab has 32 points in a full build: thirteen skills, of which
the two tier-5 unlocks exclude each other, and so do tier 4's Unflinching and
Concentration. Thresholds count points in earlier tiers of this branch: 0, 3,
10, 15 and 20. The 2026-11-05 retune and the 2026-11-06 changes below
(Concentration among them) are implemented and tested locally; they have not
been played or released.

| Tier | Required points | ID | Skill | Per rank / unlock | Maximum |
|---|---:|---:|---|---|---|
| 1 | 0 | 301 | Potent Painkiller | Pain protection +1 point; duration +1.5 s | +3 points; +4.5 s |
| 1 | 0 | 302 | Fast Bandaging | First-aid kit and bandage use time -5% | -10% |
| 2 | 3 | 311 | Metabolism | Regeneration starts 0.5 s sooner | 2.5 s sooner |
| 2 | 3 | 313 | Take Cover | First-aid kit and bandage healing +10% while crouched | +20% |
| 2 | 3 | 312 | Medical Training | First-aid kit and bandage healing +8% | +24% |
| 3 | 10 | 342 | Brothers in Arms | Incoming allied damage -25% | -50% |
| 3 | 10 | 303 | Numb Legs | Sprint stamina -20% while a painkiller is active | -40% |
| 3 | 10 | 321 | Adrenalin | Movement +5% below 30 body health | +15% |
| 4 | 15 | 332 | Strong Bones | Arm and leg maximum health +10% | +50% |
| 4 | 15 | 323 | Unflinching | Bullets' slow -10% while a painkiller is active | -40% |
| 4 | 15 | 333 | Concentration | Sight sway -5% while a painkiller is active | -20% |
| 5 | 20 | 331 | Supply Pocket | Secondary weapon replaced with supply pocket | One rank |
| 5 | 20 | 322 | Additional Medicine | One extra medicine per medicine pocket | One rank |

Fast Bandaging's factor comes after the stock one (`medkit::set_active` 0x33D481, then the patch's hook) and only for an
item that heals: the painkiller's delay is never shortened. Before revision 50 a rank took 2 % (counted on recorded
matches: the largest delays left at rank 5 are 1,750 ms of a 2 s medkit); from 50, 5 %: no recorded delay exceeds it.

The four-column layout, from tier 1 down to tier 5 (`S` Supply Pocket and `M`
Additional Medicine, one or the other):

```text
| - x x - |    Potent Painkiller, Fast Bandaging
| x - x x |    Metabolism, Take Cover, Medical Training
| x x - x |    Brothers in Arms, Numb Legs, Adrenalin
| - x U C |    Strong Bones, Unflinching XOR Concentration
| - S M - |    Supply Pocket XOR Additional Medicine
```

Points held: tier 1 = 5, tiers 1-2 = 15, tiers 1-3 = 22, tiers 1-4 = 31 (with
Unflinching or Concentration); a full build 32.

## Concentration (owner decisions, 2026-11-06 and 2026-11-09)

- New skill 333, tier 4 column 4, 4 ranks; Strong Bones column 2, Unflinching
  column 3. Unflinching and Concentration exclude each other (antiskills both
  ways, as Supply Pocket and Additional Medicine).
- Since 2026-11-09 (the owner: "reduce sway by 5% per skill point when
  painkiller is active"): while a painkiller is active (the same test as Numb
  Legs and Unflinching), the sight's sway is 5% less a rank (-20% at rank 4).
  It replaces the 2026-11-06 hearing effect (environment -10% / opponents'
  sounds +10% a rank, the local player only), which is gone with its sound
  sites (commit 61205cf06 has them).
- The sway is the game's own: modifier 61 `breath_vibration` ("Sight sway",
  the gear's -20 % bonus 1210), and Concentration adds -0.05 a rank to it,
  as percentage points (skill-design rule): the gear's -20 % and rank 4
  together give -40 %, never below no sway.
- It is part of the match: every game of it computes the aimed weapon's sway
  in the simulation (weapon_core::tick), so it has its `simulation_changes.tsv`
  row.
- Name kept ("Concentration", retail 0.69's st_trained_ears_name); the
  description says "While a painkiller is active, reduction in sight sway:"
  with the stock "Sight sway" words of each language.
- Icon: retail 0.69d0 `skills/survival/15.png` (a brain), bytes unchanged,
  MedicalSkill069333 (`icons-069.tsv`).

## 2026-11-06 changes (owner decisions)

- Unflinching: 4 ranks of -10% (was 2 of -20%), the same -40% maximum; tier 4,
  now column 3. Its modifier-45 field is 3 bits at bit 4 (16.0 a rank, 0..4).
  Saved ranks above the old 2 cannot exist, so no new refund rule.
- Numb Legs moves from tier 1 to tier 3 (column 2). Tier 1 then holds 5 points,
  so tier 2 opens at 3 (was 5) and tier 1 still leaves a choice.
- A row unlocks the branch points it asks, whole: the stock window (and srp's
  walk after it) rounded the reached `branch_points` down to fives, which would
  leave a threshold of 3 locked. The lobby source (`CharWindow.as`) and
  `SkillsTree::unlocked_branch_points` take it whole; for the stock tree, whose
  thresholds are fives, nothing changes.
- The skills window's progress bar reaches a row's mark at that row's
  `branch_points` from the tree and moves evenly between two rows
  (`CharWindow.indicatorPoints`): the medical tab marks 3, 10, 15 and 20; a
  stock tab (0/5/10/15/20) draws its points as before.
- Tiers 3 and 4 shuffled: tier 3 Brothers in Arms column 1, Numb Legs 2,
  Adrenalin 4; tier 4 Strong Bones column 2, Unflinching 3.
- Saved skills: `SkillsTree::settle` refunds a saved Numb Legs only when rows 1
  and 2 no longer hold its 10 points (ids unchanged); a full build saved before
  stays as it is.
- Potent Painkiller is additive: +1 percentage point of pain protection a rank
  (the hit's coefficient minus 0.01 a rank, never below 0: 0.9 -> 0.89, 0.88,
  0.87, so 10% -> 13%) and +1.5 s, replacing the relative +8% a rank. The
  descriptions say it adds to the painkiller's protection; the lobby's Alt
  medicine card uses the same rule. Held Alt, the medicine cards also show Numb
  Legs, Unflinching and Concentration (a painkiller) and Take Cover (a medicine
  that heals) as lines of their own, "100 -> current %" (alt-weapon-stats.md).
- Icons: Numb Legs shows Potent Painkiller's `survival/1.png` (the same
  embedded bitmap, MedicalSkill069301; its own MedicalSkill069303 is gone). Take
  Cover shows retail 0.69d0 `skills/physical/15.png` (234 "Mobility", a crouched
  soldier; MedicalSkill069313, bytes unchanged), a try the owner judges in game;
  Medic keeps `survival/14.png`.

## 2026-11-05 retune (owner decisions)

- Numb Legs (new, 2 ranks): while a painkiller is active, sprinting costs 20%
  less stamina a rank. Sprint cost only; jumps and other costs are unchanged.
- Take Cover (new, 2 ranks): first-aid kits and bandages heal +10% a rank while
  the player is effectively crouched, checked at each heal tick. It multiplies
  with Medical Training. Effective crouch is the controller's, which stays set
  under a low ceiling, never the crouch key.
- Unflinching (new; 4 ranks since 2026-11-06, 2 before): while a painkiller is
  active, the movement slow of bullets (their stopping power) is 10% less a rank. It multiplies with Spring's
  factor, whose active already removes the slow.
- "A painkiller is active" is native set_active's flag on a painkiller in the
  player's inventory: the whole time its pain protection is linked, from the use
  (0x33D638) to its end (0x33DA3E).
- Fewer ranks, the same or a near maximum: Potent Painkiller 3 x (+8%, +1.5 s; additive +1 point since 2026-11-06);
  Fast Bandaging 2 x -5%; Medical Training 3 x +8%; Adrenalin 3 x +5% below
  30 health (was 20); Brothers in Arms 2 x -25%. Metabolism and Strong Bones
  keep five ranks.
- Potent Painkiller shows both values per rank and in the final result.
- Medic (341) leaves the tree; it returns later in an anomalous tree. Its native
  code (activation sharing, the Larkspur 20-instance pool, the length clock) and
  its transport bit 20 stay as they are. It returned in the artefact tree:
  Cascade (542) sets bit 20 (artefact-skills.md), so a Larkspur's
  activation heals the teammates within 6 m (Cascade's reach) with its glow; since the owner's
  rev51 games every kind's activation (Spring, Lifebone, Onyx) plays its glow on them, a
  Lifebone's also heals (`share_active`). The sections below about Medic describe that code.
- Supply Pocket and Additional Medicine move to tier 5 and exclude each other,
  symmetrically; Supply Pocket keeps its exclusion of 111 and 113.
- Saved skills are settled when an account loads (`Account::repair`,
  `SkillsTree::settle`): ranks past a skill's levels, Medic, Additional Medicine
  beside Supply Pocket (Supply Pocket stays: it shapes the saved loadout) and
  skills whose row is no longer reached are taken back. Their points are free
  again (points to spend are the level minus the points held), the kits are sent
  again, and the five-items-a-pocket cap handles pockets that become overfilled.
- Icons: unchanged 0.69d0 survival PNGs chosen by the owner: Numb Legs
  `survival/9.png` (since 2026-11-06 `survival/1.png`, above), Take Cover `survival/14.png` (Medic's former icon; since 2026-11-06
  `physical/15.png`),
  Unflinching `survival/7.png` (`scripts/medical-skills/icons-069.tsv`).

Current equipment limits (owner clarification): **six quick slots total**, shared
between supply pockets and artifacts. Six supply pockets leave no artifact slot;
four supply pockets leave at most two artifact slots. Each supply pocket holds
**at most five items after all bonuses**, including Additional Medicine. Storage
stacks and ammunition quantities are not restricted to five.

Painkiller protection adds percentage points (since 2026-11-06). The stock
coefficient 0.9 means 10% protection: at rank 3 it becomes 0.87, or 13%
protection, for 14.5 instead of 10 seconds.
The English Medic description ended exactly: `"They glow, you shine"`; its texts
leave the edit list with the skill.

Supply Pocket excludes Terminator (111), the existing shooting Supply Pocket (113)
and Additional Medicine (322); the exclusions are symmetric. Medicine capacity uses category 10,
the native medicine category, excluding whole-pocket items (the boosters).
Whole-pocket items remain one; the server applies the same cap to moves, preset
fills and account repair without deleting inventory stock. The menu uses committed skills, never an unsaved preview. Server reset
validation obtains available branches from the loaded tree instead of assuming
two tabs. The stock movie contains only two icon sets; `SkillIcon069.as`
displays original 0.69d survival PNGs for branch 3 within the existing
skill renderer. Their bytes are unchanged in `flash/lobby_menu/bitmaps/`;
`scripts/medical-skills/icons-069.tsv` records the donor paths, mapping and SHA-256.

## Native implementation

`rust/patches/src/medical_skills.rs` contains safe rank parsing and calculations;
`vostok/medical.rs` owns the native object boundary. `medical_skills.toml` declares
patch 94's hooks; this is a code patch ID, not a release revision. The generated
engine library supplies every new call. Fields and sites are in their TSVs.

The unchanged 68-float modifier block transports the ranks as exact integers in
two unused slots, parsed once together (`Skills::parse`); either one invalid
leaves the player with no medical skill at all.

- Slot 43 `armored_trap`, 24 bits, the layout it shipped with: bit offsets 0
  (Potent Painkiller, 0..3), 3 (Fast Bandaging, 0..2), 6 (Metabolism, 0..5),
  9 (Medical Training, 0..3), 12 (Adrenalin, 0..3), 15 (Additional Medicine),
  16 (Supply Pocket), 17 (Strong Bones, 0..5), 20 (Medic, retained: set by the artefact tree's Cascade) and
  21 (Brothers in Arms, 0..2). Ranked fields keep three bits; unlocks use one.
- Slot 45 `can_defuse_traps`, 10 bits: Numb Legs at 0 (2 bits, 1.0 a rank,
  0..2), Take Cover at 2 (2 bits, 4.0, 0..2), Unflinching at 4 (3 bits, 16.0,
  0..4; two bits and 0..2 before 2026-11-06), Concentration at 7 (3 bits,
  128.0, 0..4). A full set is 74 with Unflinching, 522 with Concentration.
- The parser refuses fractional, nonfinite, negative or too-large values, any
  rank past its skill's levels, and Unflinching beside Concentration (the
  tree's exclusive pair: `Focus` holds one or neither). The lobby's
  `KitModifiers.as` parses the same. Only the base profile value is decoded.
- Do not repurpose either slot for gear or a trap feature.
- Slot 45 evidence (2026-11-05, the exact 0.26g exe): all 94 calls of the
  modifier getter 0x2D5AE0 and its rounding wrapper 0x2D5B80 resolve to an index
  and none is 45 (44 friendly_trap is read at 0x304D4C, 46 at 0x30500F, 48 at
  0x247054); apply_kit_parameter 0x2F7660 is called with 5, 14, 15, 16 and 17
  only; the name `can_defuse_traps_modifier` is referenced only by the two name
  tables (0x9284FC, 0x9C9C9C), not by code (25 and 64, read by name, have code
  references 0x224609 and 0x22462C); no kit-stride (704h) access reads its
  offset; neither the stock resources.db nor server-game-data.db nor any edit
  list contains `can_defuse_traps`. Only the weighted getter 0x2D5BC0 takes
  indices from configs, which never name it. The same scan finds no read of 43.

Healing uses native modifier 57; the supply pocket uses native modifiers 15 and
64. Fast Bandaging is filtered to healing medicines at the native use-time site,
without altering global modifier 33.

The combo skills' sites (sites_026g.tsv; fields_026g.tsv rows):

- Numb Legs: `medical.sprint`, 0x329AAD in player_stamina::sprint, XMM0 the sprint
  cost a second after modifiers 19 and 67, before the elapsed-time multiply. The
  stamina is embedded at base_player+6C0h (ctor 0x2D6E5C, tick 0x2D8FBC): field
  `player_stamina.player` at -6C0h.
- Take Cover: `medical.medicine_heal`, 0x33D948 in medkit::tick, XMM0 the amount of
  this influence (drug_amount times the elapsed time, after the minimum's comiss;
  the hook keeps the flags for the `ja`), EBP the holder. 0x33D93D was refused: a
  base relocation lies under it.
- Effective crouch: player+494h is the bt_character_controller wrapper (made at
  0x2D6EF1 by 0x3569D0); its +0 the Bullet controller; its +201h byte the
  effective crouch, set by the crouch states' request (0x337810) and cleared only
  when it can stand (+202h; 0x337830) or by a jump (0x2D793E); serialized at
  0x7FBA0; player_step 0x7BDA0 shapes the capsule from it (setup_crouch_state
  0x7F1A0 writes +203h). It is 0.69d's controller +251h.
- Unflinching: patch 54's stagger hook at 0x2DBD89 (artefacts_026g.asm) stays the
  one owner of that site. After Spring's getter it calls patch 94's generated
  entry `medical.stopping` (cdecl: the player, the sum in and out; every register
  kept), which multiplies the sum by the painkiller factor. No second hook.

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
It excludes self, enemies, dead/inactive players and targets outside Cascade's reach (6 m;
5 m before the owner's 2026-11-07 change).
Each successful native lookup is an owned reference released on every exit.
For recipients it removes the Larkspur config's affects, fills body/head/pain
like patch 54, and adds a native effect instance with the recipient's calculator.
The places it reaches are the copy's for its whole active (since 2026-11-11 patch 109 gives a
Spring's and an Onyx's active to them wherever they go, artefact-skills.md "Resonance or Cascade");
an activation without Medic's bit reaches none.
The source's normal activation and cooldown continue unchanged. Brothers in Arms
uses the same core lookup and team rule, after spawn invulnerability and before
native body-part damage processing. Environmental and self damage are unchanged.

Concentration (`on_sway` in medical_skills.rs, `vostok::medical::sway`; the
site `medical.sway`, sites_026g.tsv):

- breath_vibration_calculator::tick (0x31D6E0; the calculator at weapon+288h,
  EDI) runs from weapon_core::tick (0x2E0A40) only while the weapon is aimed
  (+5E8h), with the weapon's dispersion now as its first argument: the
  dispersion calculator's m_weapon_dispersion (+418h) plus
  m_character_dispersion (+404h), 0x2E0A23. At 0x31D722 it calls
  apply_modifier with index 61 (`breath_vibration`) on the holder's kit
  (calculator +3Ch, the player set by set_user 0x2E29F3): B x max(0, 1 + S).
  That value feeds the breath state's tick (vtable +1Ch), the amplitude it
  eases to (1 + params+18h x it, 0x31D898) and the phase's speed (params+1Ch
  x it, 0x31D7E9): the sight's sway. It is the only read of modifier 61 in
  the exe (the 93 apply_modifier calls' indices; 0x2D5B80's one call reads
  29, apply_modifiers 0x2D5BC0's two {4, 20} (0x2DBC1D) and {24, 56} (0x331715)).
- The site: 0x31D727, after that call, before `movss [esp+10h],xmm0`
  (0x31D72D); the hook replays `mov eax,[ebp+0Ch]` / `fild dword [ebp+0Ch]`.
  XMM0 becomes max(0, M - 0.05 x rank x B) = B x max(0, 1 + S - 0.05 x rank),
  with B the tick's argument ([ebp+8], never written) and the painkiller test
  of Numb Legs. A B or M not above 0 is left as it is.

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

Automated checks cover all thirteen skills; not all of them have been played manually.

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
  the thirteen skills' progression, layout, 32-point full build, exclusions and
  native modifier transport.
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
movie is `eb2ba8b7c606f5978095225a8784585d015d8d5df5edfc110a45c8501aeea0d9`;
its latest-main changes still need an owner gameplay check
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

## Validation of the 2026-11-05 retune

Implemented and tested locally; not played, not released.

- `check_medical_skills_026g.py` (Unicorn, built exe): Potent Painkiller 0..3 and
  Fast Bandaging 0..2 at start/tick/restore, use time and protection, ranks past
  the levels refused (no skill at all); Adrenalin at 0.3/0.29/0.19 health; Numb
  Legs, Take Cover and Unflinching for every modifier-45 rank combination 0..2
  plus rank 3 refused, painkiller active/inactive (an active bandage is no
  painkiller), effective crouch against the crouch key alone, and Spring
  active/inactive through the real stagger 0x2DBCD0; two kits, identical
  outcomes; the Medic checks unchanged. PASS.
- `check_artefacts_026g.py`: 85 stock and 84 patched checks as expected, the
  stagger cases among them.
- Patch-crate Rust tests (121) and i686 Clippy, no warnings.
- `survarium` skills tests (`SkillsTree::settle`), session account (with the
  refund test), rules, persist and simulation-list tests; session/survarium
  Clippy, no warnings.
- `release/build-content 49` (a local build; no number reserved): all 18823
  files the manifest makes are as it makes them; the built tree passes
  `survarium/tests/medical_skills.rs` (positions, thresholds, ranks, 30-point
  build, exclusions, modifiers 43/45/57).
- Ruffle harness on the built lobby movie: `MEDICALSKILLS RESULT pass`.
- `engine_026g.py`, `hooks_026g.py` and `link_026g.py` checks current.

## Validation of Concentration's sway (2026-11-09)

Implemented and tested locally; not played, not released. (The hearing
version's checks of 2026-11-06 went with it; commit 61205cf06 has them.)

- `check_medical_skills_026g.py` (Unicorn, built exe), `check_sway`: the
  native breath tick from 0x31D6F1 to 0x31D72D, the real apply_modifier of
  index 61 and the hook between, for the dispersion 0, 0.3 and 2, the kit's
  S61 0, -0.2, -0.9 and -1.5, Concentration 0..7 (5..7 refused) with and
  without Unflinching (refused beside it), painkiller on/off (an active
  bandage is no painkiller): B x max(0, 1 + S - 0.05 x rank), the argument
  unchanged; kits 0 and 2 the same. PASS; the file's other checks PASS.
- Patch-crate tests (218, the new sway test among them) and i686 Clippy, no
  warnings.
- `engine_026g.py`, `hooks_026g.py` current; `link_026g.py --write`, then
  current; `check_hooks_026g.py` 28 ok. The after_move and lobby-activate
  chains without Concentration's handlers: `check_lobby_scene`,
  `check_lamps`, `check_ragdoll`, `check_weapon_pickup` pass.
- `check_localization_026g.py` PASS; `release/build-content 54` (a quick
  local build; no number reserved) builds, skills_tree.options changed by the
  edit list.
- Ruffle on that build's lobby movie: `DETAILS RESULT pass` (the painkiller
  card's Sight sway line 100 -> 95 % and -> 80 %, green, in every language
  beside its value).
- Not played: how -5 % to -20 % of the sway feels at the sight.

## Validation of the 2026-11-06 changes

Implemented and tested locally; not played, not released.

- Patch-crate `cargo test medical` (3 tests: Unflinching 0..4 and 5..7
  refused, modifier 45 below 128, the additive painkiller 0.9/0.89/0.88/0.87)
  and i686 Clippy, no warnings.
- `check_medical_skills_026g.py` (Unicorn): Potent Painkiller's protection
  0.5 x (0.9 - 0.01 x rank); every modifier-45 combination of Numb Legs 0..2,
  Take Cover 0..2 and Unflinching 0..4 (-10% a rank) plus ranks 3/3/5..7
  refused; the rest unchanged. PASS.
- `survarium` skills tests (the whole-threshold walk, tier 2 at 3, settle:
  a full build saved before the move stays, Numb Legs refunded when rows 1-2
  no longer hold 10), session account tests (the saved row-1 Numb Legs build
  unchanged, kits not sent again) and the simulation-list test; Clippy on
  both crates, no warnings.
- `engine_026g.py`, `hooks_026g.py` current; `link_026g.py --write` (new
  lock), then current.
- `release/build-content 49` (a local build; no number reserved): all 18960
  files the manifest makes are as it makes them; patch
  `fbf094cd39e8d61a959da3d4d43bbc51f8615a9c053928bcd6d38c341a0211cf`, server
  data `ea4ca3e2f8b7d5e2fd2b95192fc4241a9b0ab15c77711c4f3fd563bdc3203fd9`.
  Its skills tree passes `survarium/tests/medical_skills.rs` (positions,
  thresholds 0/3/10/15/20, ranks, 32-point build, modifier 45 = 74).
- Ruffle on that lobby movie: `MEDICALSKILLS RESULT pass` (layout, tier 2
  refused at 2 and open at 3, tier 3 at 10, Unflinching stops at 4, the
  +3% / +4.5 s texts, Numb Legs on the painkiller bitmap, Take Cover's
  physical/15 pixels) and `DETAILS RESULT pass` (Alt card 10 -> 13%,
  Unflinching 5 refused, full combo 74 valid).
- The progress bar (lobby movie built from source, sha256
  `f87241fecf1007c8510fb07d022648a8b20bfd23b6e6f8c249c4d06d793d850c`):
  `MEDICALSKILLS RESULT pass` with the bar's value checked on the medical tab
  (below the row-2 mark at 2, the marks at 3/10/15/20, full with tier 5 taken)
  and on a stock 0/5/10/15/20 tab at every point 0..20 (min(points + 1, 21),
  as before, then full); the old drawing fails the medical marks. `DETAILS
  RESULT pass` on the same movie. Not seen in the game.
