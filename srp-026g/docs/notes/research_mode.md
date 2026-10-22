# Research (mode 5, capture points)

The owner's rules: Research comes back as 0.27 shipped it, its rules unchanged (2026-10-19);
"find previous work and (FULLY!) implement the research mode; this will not go into this release
though, and will go into the next one" and "we need to find everything associated with the
release too" (2026-10-22). Not in revision 28.

Code: exe patch 53 (`client-patches/research_rule_026g.c` the rules, `research_native_026g.c`
the game's side, `research_026g.asm` the hooks; README "53."); the hud movie
(`flash/hud`: `HUD.research_update`, `survarium.controls.ResearchUnits`,
`ResearchMinimapPoint`, `Minimap.setResearchUnits`); the content (`scripts/research-mode`,
pack `research-mode@1`); srp (`maps::mode::RESEARCH`, `match_msgs::rules::Research`,
rewards); the lobby (`GameMode.RESEARCH`, the rooms' and matches' mode names).

## Rules (0.27d's)

- **Goal:** three research units a map (A, B, C: `cp_id` 0, 1, 2). A team earns research
  points while it owns units; the first to 100 wins, else more points at 15 minutes; equal
  points: a draw.
- **Taking a unit:** players of one team inside it and none of the other.
  - A whole capture takes 20, 18, 15, 14.9, 14.8, 14.7, 14.6, 14.5 s for 1 to 8 players (more
    count as 8).
  - Both teams inside: nothing moves.
  - Another team's progress, or its ownership, is drained first at the same speed; at zero the
    unit is neutral and the new team's capture starts (with what was left of that tick).
  - Nobody inside and nobody owns it: the progress falls back at half speed.
  - An owned unit stays owned with nobody inside; its owner inside fills its drained progress
    back up.
- **Inside:** a living player whose capsule (radius 0.4 m, 0.9 m above and below its centre)
  overlaps one of the unit's shapes (0.27d's trigger: boxes, cylinders; London's spheres).
- **Points:** owning 1, 2 or 3 units gives 0.075, 0.15 or 0.35 points a second (100 points in
  22:13, 11:07 or 4:46). Shown whole (rounded).
- **End:** 3 s after a team reaches 100, or at the time limit (900 s; the time rule as in every
  mode: waiting 80 %, countdown 20 s). Respawn 10 s, at the mode's own spawns.
- **Score events** (0.27d `gameplay/awards/events.options`):
  - a capture or a neutralisation (taking a unit from the other team) shares 100 award points
    among the team's players inside, "water-filled" by the part of the capture each was there
    for (0.27d 0x31E920); a full capture is worth score 30 (0.27d's `capture_the_point`), counted
    in the statistics' events 6 (capture) and 18 (neutralisation), in hundredths;
  - killing an enemy inside a unit his team does not own ("kill invader") is score 10, event 10.
  srp sends these weights (30, 30, 10) in a Research match's static info; the activity pay
  follows the score as for every event.
- **HUD:** the teams' points in Team Deathmatch's bars; the three units under them (letter,
  owner's colour: own team blue, other red, nobody grey; a ring for the capture of the team
  taking it); "Capturing unit..." while one's own player is inside a unit his team is taking;
  the units on the minimap and the Tab map; messages: the start's "Find and capture research
  units!", "Unit A captured" / "Unit A lost" for one's own team, the end's victory, defeat or
  draw.
- **Late joiners, rollbacks, replays:** the rule's whole state is in the world (21 + 96 bytes a
  unit); a joiner, a resync or a replay computes the same.
- **Bots (Player vs Bots):** not offered in Research (srp's bots play modes 2 to 4).

## Data

- 25 maps: levels 1 to 8 in day, evening, rain (Cologne: day, evening, pre-rain), as 0.27d
  shipped them, and London's day (0.31a2's Research layout; skhid's London has only its day).
- `gameplay/game_modes/level_0N_<w>_5.options` (w day, evn, rain, prerain): 0.27d's parameters
  (`match_time` 900, `wait_time` 5, `countdown_time` 20, `wait_player_percent` 0.8,
  `respawn_time` 10, `resources_count` 100, `capture_time_in_s`, `resource_per_s`,
  `restore_multiplier` 0.5), `rules [93, 91, 480, 94]` (480 = 1E0h: the exe's research rule
  cook), `ui.icon` 3, `st_mode_05_name`, and the units as `capture_zones[] {cp_id, position,
  shapes[] {type 0 sphere / 1 box / 2 cylinder, position, axis_x, axis_y, axis_z, half}}` in
  world space, `capture_award` 100, `capture_reach` 0.4, `capture_half_height` 0.9.
- `projects/level_0N/<w>_5/client_project` (w day, evening, rain, prerain): 0.26g's `<w>_4`
  with what 0.27d's `_5` changed against its own `_4`: the units' props (chargers, antennas,
  turbines, transmitters, diesel generators, monitors: render visuals and collision objects),
  their sounds, particles, lights and turning windmills, the cover 0.27d added or took away at the
  units, 0.27d's mode-5 spawns and resupply zones, each object in the form 0.26g's of its type
  have. Four models 0.26g lacks come byte for byte from 0.27d (same format; their materials
  are 0.26g's): the transmitter's cable corner, two box covers, level_01's tractor (0.27d moved
  it beside unit B for cover).
- maps_dict: 145-149 (0.26g's own rows for `level_0{1,4,6,7,8}_day_5`, never with a file
  before); 184-203 for the rest (0.27d's 152-170 are srp's ported maps now).
- Texts in the six languages: 0.27d's `st_mode_05_name` ("Research"), `st_mode_05_description`,
  `st_capture_info`, `st_start_match_welcome_message_5`, `st_finish_match_mode_05_{winner,loser}
  _message`, `st_match_settings_checkbox_capture_points(_hint)`,
  `st_event_{capture_the_point,kill_invader}_description`; srp's `st_research_unit_{a,b,c}
  _{captured,lost}` and `st_finish_match_mode_05_draw_message` (`texts.tsv`).

## Making the pack

- `scripts/research-mode/make_pack.py --stock <folder> --donor <folder> --london <folder> --out
  <SKHID_PACKS_DIR>/research-mode-1`: the inputs at their game paths or as `dbgrab ... get` writes
  them, each checked against `inputs.sha256` (0.26g's 25 `<w>_4` projects and London's day_4
  config from the stock db and patch 14; 0.27d0's `_4`/`_5` projects, `_5` configs, six
  localizations and the four models; 0.31a2's London `day_4`/`day_5`), and `stock.list` (the
  stock db's and the patches' `dbgrab list`, for the models 0.26g has). It prints per map what it
  added, removed and left out, and the manifest row (`release/content.tsv`); the same inputs
  give the same pack.
- Checked: `vostok-port pack check` over the stock db and patches 12 to 27; `release/build-content
  29` with it (2026-10-22: one patch, every file the manifest makes as it makes it, the server
  data with 109 projects); `vostok-port pack probes`: no probe missing; `release/build-site` with
  that patch (the Maps tab with the units).

## The exe (patch 53)

- **Rule:** `rn_rule` = game_match_rule_base (type 5) + the 0.26g rule slots (event 7: kills;
  8/9 enter/leave; 11 the world: its tick a delegate of the world's second list, after the time
  rule's; 15/16 the state; 17 the hud's team points; 18 completed). Cook class 0x1E0, registered
  by the game's constructor after its own cooks (the class is free in register_cook's table of
  0x206; refused, and the mode not loaded, if taken).
- **Tick (50 ms):** only while the time rule is in play (core+18FE4h = 3); players by roster
  place (entered, alive `+33Ch`, in the world `+33Fh`, team `+6B8h -> +45ECh`, position
  `+43Ch`); then the statistics' tail: `m_team_points` = the scores, `m_winner_team` = the
  leader.
- **Wire (rules blob, between the respawn rule and the kd rule; little-endian):**

  | at | size | what |
  |---|---|---|
  | 0 | u32 x2 | team points, millionths (0..100 000 000) |
  | 8 | u16 x2 | team scores shown (0..100) |
  | 12 | u32 | the end: match ms, FFFFFFFFh none |
  | 16 | u32 | roster places entered and not left |
  | 20 | u8 | units Z (3) |
  | 21 + 96z | u32 | progress, 0..1 000 000 000 |
  | +4 | u8 x4 | team taking it, owner (2 nobody), players inside of team 0, of team 1 |
  | +8 | u32 x2 | those players by roster place |
  | +16 | u32 x20 | each place's mark: its share of the capture at entry, 1e9 = all |

- **Section:** the exe's section table has room for twelve entries (its headers end at 400h,
  where the first section's bytes begin; moving every section's bytes to grow it would change
  every file offset the tools and the released records name). Eleven are taken (the seven of the
  game, `.srp`, `.srpd`, `.srpc`, `.srpw`). The twelfth, `.srpx` (0xA42D000, code, data and
  writable), holds the code of every later patch that needs pages of its own, each at an offset
  of its own: Research at +0 (code to +3000h, its state page +3000h), the dynamic anomalies
  (patch 42, #225) at +4000h (`DYNAMIC_ANOMALIES_OFFSET`). #225 then adds no section: it
  assembles `dynamic_anomalies_026g.asm` for 0xA431000 and puts its bytes into `.srpx` at
  +4000h (int3 between), its two hooks aiming there.
- **HUD:** update_ui (every frame): `root.research_update(own team, points 0, points 1, goal,
  Z, then cp_id, x, -z, owner, taking, progress %, inside 0, inside 1, own player inside)`, sent
  at a change and every 60 frames; the messages through the hud's parameterised message.

## Inventory of the builds (2026-10-22)

| build | Research | maps | what differs |
|---|---|---|---|
| 0.26g (ours) | none: `maps_dict` 145-149 name `_day_5` configs that do not exist; the hud has an older `set_capture_progress` no code calls | | |
| 0.27d0 (build 2500) | added (0.27a) | 24 (8 x 3) | the source: rules, configs, projects, texts |
| 0.28a2, 0.28d0 | same configs | 24 | capture 20, kill invader 5 (0.28a: "reduced"); map hints `st_map_hint_5_*`; the anomaly rework in every project |
| 0.31a2 | same rules | 27: + London | renumbered rules, `score` for `resources_count`, capture times 1-3 players only, achievements (`win_ctp`, `capture_points_ctp`) |
| 0.46c2 | `ctp` configs | 63 (variants v1-v4) | mode 2 configs named `ctp`, sounds `ui_sounds/capture_mode/*`, daily quests |
| 0.69a0 | | | an unextracted installer only (/mnt/toshiba): not read |

Ported: 0.27d's rules, configs, 24 maps, props, texts; London's day from 0.31a2 (its units are
spheres); the hud and minimap as code of our own (0.27d's widgets are graphics of its movie,
which the source tree does not carry); the score events as 0.27d weighed them.

Left out, and why:
- **0.28a's lower awards** (20, 5): the owner chose 0.27d's rules.
- **0.46c2's `ctp` variants and sounds:** a later rework (other unit places per variant, new
  ui sounds); the owner asked for Research as it was released.
- **The spawn's invulnerability zone** (`effect_zone`, 3 s): 0.26g's spawns give 6 s already.
- **level_03's reworked anomaly in its mode-5 project:** 0.27d's anomaly format; the map keeps
  0.26g's (and the geometries its artefacts name).
- **Badges, achievements, daily quests of the mode:** 0.26g has none of these systems.
- **Map hints** (`st_map_hint_5_*`, 0.28a): the loading screen of 0.26g shows the description.
- **The lobby's "Research" match filter checkbox** (0.27d `checkBoxCapture`): there is no
  search queue any more; the rooms' create window lists the modes, Research among them.
- **0.27d's environment probes:** the projects keep 0.26g's own (the same places).

## Release impact

- Exe: patch 53. Simulation change for mode 5 only. Its code is in a new section, `.srpx`, the
  section of the later patches' code (below).
  Simulation change: a match of mode 5 needs it; modes 2-4 do not change.
- Content: pack `research-mode@1` and the hud movie (content.tsv); the server data with it
  makes the 25 maps playable.
- srp: mode 5 in the maps, rooms, results and pay; a server that offers Research asks for the
  revision with patch 53.

## Tests

- `test_research_rule_026g.c` (the rules, the wire, the water-fill), `check_research_026g.py`
  (the built exe's hooks and slots under Unicorn on made-up worlds), `selftest_026g.py`.
- srp: `match_msgs::rules` (decode), maps (mode 5 playable), rewards (weights, no battery caps).
- In the game: see the PR's test plan.

## How 0.27d does it (0.27d0 VAs, x86-BCJ-decoded exe)

- `capture_the_point_rule_core` (0x208 bytes): ctor 0x31E5C0; events 0x31E920 (kill invader:
  victim inside a zone his team does not own; type 2: water-fill of 100 by marks); tick
  0x31EEC0; serialize 0x31F160 (16 bytes). `capturable_zone_core`: tick 0x324070 (the capture
  above), enter 0x324350 (mark = progress or 1 - progress), serialize 0x323E90 (96 bytes).
- Its hud: `root.show_capture_info`, `set_capture_info` (`st_capture_info`),
  `set_capture_data`, `update_capture_progress`; minimap objects `capture_point` with `cp_id`.
- Source: `/mnt/hdd/Projects/Survarium Builds/v0.27d0-build2500` (Inno + `.sup`, unpacked into
  `/mnt/hdd/Projects/weapon-survey-20261016/work/builds/v027d0`); the exe is x86-BCJ filtered.
