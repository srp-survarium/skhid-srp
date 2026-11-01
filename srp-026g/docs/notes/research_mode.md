# Research (mode 5, capture points)

The owner's rules: Research comes back as 0.27 shipped it, its rules unchanged (2026-10-19), but
for its points' rate, 1.25 x 0.27d's ("let's make it 1.25x faster", 2026-10-22);
"find previous work and (FULLY!) implement the research mode; this will not go into this release
though, and will go into the next one" and "we need to find everything associated with the
release too" (2026-10-22). Not in revision 28.

Code: exe patch 53 (`crates/research-rule` the rules, `client-patches/research_arena`
the game's side, `research_026g.asm` the hooks; README "53."); the hud movie
(`flash/hud`: `HUD.research_update`, `survarium.controls.ResearchUnits`,
`ResearchMinimapPoint`, `Minimap.setResearchUnits`); the content (`scripts/research-mode`,
pack `research-mode@3`); srp (`maps::mode::RESEARCH`, `match_msgs::rules::Research`,
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
- **Inside:** a living player whose body overlaps one of the unit's shapes (0.27d's physics
  trigger: boxes, cylinders; London's spheres): his capsule standing on his feet (the player's
  transform), 1.8 m tall standing and 1.1 m crouched (half its height read from his controller as
  the game's get_transform does), 0.38 m round (`default.player`: 0.76 m wide). A unit reaches no
  higher than its shapes: a player on a floor above a shape's top is outside (client_patch.md
  §3.50, "A player's transform is his feet"; the first build counted feet up to 0.9 m over it).
- **Points:** owning 1, 2 or 3 units gives 0.09375, 0.1875 or 0.4375 points a second (100 points
  in 17:47, 8:53 or 3:49). The owner (2026-10-22): "let's make it 1.25x faster", over 0.27d's
  0.075, 0.15 and 0.35 (22:13, 11:07, 4:46). Shown whole (rounded). The rule counts them in
  millionths of a point per 4 ms (375, 750, 1750: whole numbers), a 50 ms tick's share floored
  (one unit: 4687 for 4687.5, 100 points at 17:46.8).
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
- **Rewards: combat-adjusted cap** (PR #691, revised 2026-10-28):
  - captures and neutralisations share a payment allowance of **200 score + 40 per paid
    enemy kill + 20 per assist**. Above it, those two counters are reduced proportionally;
    their weights remain 30 per whole capture/neutralisation. Kill-invader rewards stay whole;
  - the repeat-kill cap runs first, so unpaid feeding kills cannot expand the allowance.
    `reward_caps::Payment` owns this order and the payment-only copy. Bot matches skip these event caps and have a tier-7 reward ceiling instead;
  - the earlier flat 200-point cap cut a contested 12/10 player's silver by 46% and a
    9/8 player's by 43% in match `7964b894`. Both now fit within the allowance. Conversely,
    21 captures and 20 neutralisations with zero kills/assists still pay at most 200 points;
  - score, quests and rating use the ordinary statistics; only the payment copy is reduced.
    Kills, help and the fixed payment are not cut by the objective rule;
  - these counters are evidence of repeated objectives, not proof of collusion. Objective-only
    legitimate play can still cross the allowance, while coordinated combat can raise it.
- **HUD:** the teams' points in Team Deathmatch's bars; the three units centred right under them, as in rev 30;
  the respawn countdown may overlap them (owner: less screen space) (beside the bars' indicator,
  following its position every frame, so the indicator's centring never counts them) (letter,
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
  Rudnya (v0.69) (218, pack `rudnya-069d`) takes level_01's day units and props as they are
  (`rudnya-069d.md`).
- `gameplay/game_modes/level_0N_<w>_5.options` (w day, evn, rain, prerain): 0.27d's parameters
  (`match_time` 900, `wait_time` 5, `countdown_time` 20, `wait_player_percent` 0.8,
  `respawn_time` 10, `resources_count` 100, `capture_time_in_s`, `resource_per_s` (x 1.25:
  0.09375, 0.1875, 0.4375),
  `restore_multiplier` 0.5), `rules [93, 91, 480, 94]` (480 = 1E0h: the exe's research rule
  cook), `ui.icon` 1 (0.27d's 3 is a frame 0.26g's lobby has not: its `SrvGameModeIcons` has
  three, 0 the protective device of modes 2 and 3, 1 a transmitter mast no mode used, 2 Team
  Deathmatch's skull; `LobbyGameLoad.setMode` shows frame icon + 1), `st_mode_05_name`, and the
  units as `capture_zones[] {cp_id, position,
  shapes[] {type 0 sphere / 1 box / 2 cylinder, position, axis_x, axis_y, axis_z, half}}` in
  world space, `capture_award` 100, `capture_reach` 0.38 (half the body's 0.76 m), `capture_half_height` 0.9 (half its 1.8 m standing, when the game gives none).
- `projects/level_0N/<w>_5/client_project` (w day, evening, rain, prerain): 0.26g's `<w>_4`
  with what 0.27d's `_5` changed against its own `_4`: the units' props (chargers, antennas,
  turbines, transmitters, diesel generators, monitors: render visuals and collision objects),
  their sounds, particles, lights and turning windmills, the cover 0.27d added or took away at the
  units, 0.27d's mode-5 spawns and resupply zones, each object in the form 0.26g's of its type
  have. Four models 0.26g lacks come byte for byte from 0.27d (same format; their materials
  are 0.26g's): the transmitter's cable corner, two box covers, level_01's tractor (0.27d moved
  it beside unit B for cover).
- **Values in 0.26g's types.** The game reads a config's vectors raw (a float3 value's data is a
  pointer to three floats: the respawn point's reader 0x2F4580 does `movq` off it), so a vector
  written as an indexed table puts an object somewhere near (x, 0, 0). Pack 1 copied the projects
  through JSON, which made every vector of every kept object (position, rotation, scale, pivot,
  colours, directions, `levelmap_min`/`max`, tangents, probe colours: about 9,800 on map 184) an
  indexed table: the first local test's level "did not load properly" and its spawns, props and
  player positions were wrong (2026-10-22). Pack 2 copies deeply (tuples stay float2/3/4), puts
  every added object, entry and spawn in 0.26g's form at every depth (`conform`: the keys 0.26g's
  projects have at that path, of the types they have there), and stops when a value of the made
  project has a type 0.26g's projects never have at its path, or a mode config one a stock 0.26g
  config does not (`check_types`, `check_config_types`). A colour ramp's `key0`, `key1`, ... count
  as one key: the game reads `key%d` from 0 while there is one (0x350080), and some 0.27d lights
  have four. Nothing else of the 0.27d objects (`skeleton_visual`, `wire_set`, lights, sounds,
  particles; nested tables included) is a key 0.26g lacks.
- **Environment probes** stay 0.26g's own (each map's `<w>_4` ones, none added): the game reads a
  probe's `texture` (0x28F48D, the `<w>_4` bake of its weather) and never a `textures` table (no
  such key in the exe), so a `_5` project needs no `_5` keys.
- **Minimap:** the hud places the units at (x, -z), as the exe's own objects (0x238681 negates z
  for `pos_y`), on each level's minimap frame (`Minimap._minimapProps`: level_02 110 m round
  (21.079, 105.098)). Every unit and spawn of the 25 maps is inside its frame (map 184: A at 0.37,
  0.40, B 0.60, 0.50, C 0.36, 0.61 of the frame).
- maps_dict: 145-149 (0.26g's own rows for `level_0{1,4,6,7,8}_day_5`, never with a file
  before); 184-203 for the rest (0.27d's 152-170 are srp's ported maps now).
- Texts in the six languages: 0.27d's `st_mode_05_name` ("Research"), `st_mode_05_description`,
  `st_capture_info`, `st_start_match_welcome_message_5`, `st_finish_match_mode_05_{winner,loser}
  _message`, `st_match_settings_checkbox_capture_points(_hint)`,
  `st_event_{capture_the_point,kill_invader}_description`; srp's `st_research_unit_{a,b,c}
  _{captured,lost}` and `st_finish_match_mode_05_draw_message` (`texts.tsv`).

## Making the pack

- `scripts/research-mode/make_pack.py --stock <folder> --donor <folder> --london <folder> --out
  <SKHID_PACKS_DIR>/research-mode-3`: the inputs at their game paths or as `dbgrab ... get` writes
  them, each checked against `inputs.sha256` (0.26g's 25 `<w>_4` projects and London's day_4
  config from the stock db and patch 14; 0.27d0's `_4`/`_5` projects, `_5` configs, six
  localizations and the four models; 0.31a2's London `day_4`/`day_5`), and `stock.list` (the
  stock db's and the patches' `dbgrab list`, for the models 0.26g has). It prints per map what it
  added, removed and left out, and the manifest row (`release/content.tsv`); the same inputs
  give the same pack.
- Checked: `vostok-port pack check` over the stock db and patches 12 to 27; `release/build-content
  29` with it (2026-10-22: one patch, every file the manifest makes as it makes it, the server
  data with 109 projects); `vostok-port pack probes`: no probe missing; `release/build-site` with
  that patch (the Maps tab with the units). Pack 2 (2026-10-22): its own type checks; `release/
  build-content 30` over revision 29 (one patch, every file as the manifest makes it, 109
  projects); map 184's project as players get it has 9,475 float3, 199 float4 and 354 float2
  values and no vector as an indexed table; every unit and spawn of the 25 maps on its minimap.

- **Units against the floors** (a report, not a gate): `mapdata-zones --db <resources.db> --patch
  <each offered patch> --patch <the new patch>` stands a player (patch 53's test) on every ground
  face under each unit's shapes and groups where he counts into floors. A unit taller than its
  floor is 0.27d's design (the owner, 2026-10-22: "it is definitely fine for capture bounding box to
  not just be floor"); a floor counts only where the capsule standing on it overlaps a shape, as
  in 0.27d. Pack 3 with patch 53's fixed test (2026-10-22), a floor above the unit's own inside
  its shapes:
  - level_08 (day, evening, pre-rain), unit C: faces at 37.50 m (64 samples, a small surface)
    inside its box (35.84 m its floor, top 39.18 m);
  - London (level_09 day), unit B: faces at 3.54-4.77 m (the level unit A stands on) inside B's
    sphere (radius 6.5 m, top 7.08 m), 0.31a2's units as they are.

  School (level_02), whose floor above A and C (20.25-20.5 m, the boxes' tops 20.04 m) took the
  units in the first build, has none: the first build took a player's transform for his capsule's
  centre (client_patch.md §3.50, "A player's transform is his feet").

## The exe (patch 53)

- **Rule:** `rn_rule` = game_match_rule_base (type 5) + the 0.26g rule slots (event 7: kills;
  8/9 enter/leave; 11 the world: its tick a delegate of the world's second list, after the time
  rule's; 15/16 the state; 17 the hud's team points; 18 completed). Cook class 0x1E0, registered
  by the game's constructor after its own cooks (the class is free in register_cook's table of
  0x206; refused, and the mode not loaded, if taken).
- **Tick (50 ms):** only while the time rule is in play (core+18FE4h = 3); players by roster
  place (entered, alive `+33Ch`, in the world `+33Fh`, team `+6B8h -> +45ECh`, feet `+43Ch`,
  capsule half height from the controller `[[+494h]]`); then the statistics' tail: `m_team_points` = the scores, `m_winner_team` = the
  leader.
- **Statistics:** `[core+18FA8h]` is the statistics handler (vtable 0xA0755C; a world's load
  calls its slot +34h, 0x2D2B0F); the shared statistics are at +8 in it: a place's record at
  +8 + 1F8h x place, u16 events[28][3] first (the handler's `recalculate_scores` 0x26DC30), the
  listener at +8 + 31A0h; the tail (+31D4h, +31E4h) is the handler's (tdm's 0x316257).
  - Revision 30 counted at +0Ch + 1F8h x place and told the listener at +31A0h: each award 4
    bytes on, and nobody told (that dword is the shared statistics' batteries pointer, 0 in
    Research). Kit 0's award went into kit 2's cell of the right event; kit 1's and kit 2's into
    the next event: a capture into 7 (a teammate cured), a neutralisation into 19 (a battery
    found), each 2.5 a one, so 250 points instead of 30, and a help's pay a one; a kill invader
    into 11 (participating, 0). The Tab score moved only at the next world loaded.
  - 7 and 19 are one byte on the wire: at every world loaded (a rollback, many a second) the
    cell is cut to the hundredths modulo 256, so the score fell by 640 when one passed 255 (the
    owner's "670, then 70"). The server read the same bytes: live match 0x31973a2 scored 993
    and 1023 and paid 124k silver.
  - Fixed in the next revision (`check_research_026g.py` runs the game's own
    `recalculate_scores` on the cell, and a kit 1 kill invader). No server-side workaround: the
    release raises the minimum patch revision, so revision 30's games are refused.
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
  writable), is the arena of every later patch (client_patch.md 0.3): blocks declared in
  `patch_026g.py`'s `ARENA` with their sources, linked by `link_026g.py`. Research's are
  `research.code` (research_026g.asm and research_arena) and `research.state`
  (the cook and UI cache; addresses in `arena_026g.lock.json`); its hooks take their targets from
  the link by symbol. Patch 58's kit keys have blocks of their own the same way,
  and no section (#225's dynamic anomalies had one too, taken out of revision 31).
- **HUD:** update_ui (every frame): `root.research_update(own team, points 0, points 1, goal,
  Z, then cp_id, x, -z, owner, taking, progress %, inside 0, inside 1, own player inside, then remaining capture/neutralisation ms per unit)`, sent
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
- Content: pack `research-mode@3` and the hud movie (content.tsv); the server data with it
  makes the 25 maps playable.
- srp: mode 5 in the maps, rooms, results and pay; a server that offers Research asks for the
  revision with patch 53.

## Tests

- `cargo test -p research-rule` (the rules, the wire, the water-fill, capture countdown), `check_research_026g.py`
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


## Research markers and aiming (2026-10-27)

Only Research is taken from the Rust port (`195bad01`, `feat/rust-port`): the pure
rules crate, the engine-facing arena crate, aligned entry stubs and the small runtime.
The rule layout, wire and simulation stay compatible with the previous implementation.
The C filename in the hash-mismatch field metadata is retained for compatibility.

The top HUD and projected world markers are separate. Neither shows “Capturing unit…”.
The local world marker grows from 24 to 34 px and has corner brackets; the HUD icon
grows by the same 34/24 factor. Both teams inside
means crossed swords. An enemy alone advancing a capture or neutralisation shows a white
`!`, on friendly-owned and neutral units alike; it clears when capture stops or finishes.
Friendly capture shows neither overlay. Swords take priority when both teams are inside.
Both overlays use `ResearchAlert`, with a two-second pulse measured by elapsed time
(not frame count), the swords' 54 x 53 image bounds, and the existing per-movie scale.
The approved warning source is `tools/actionscript/art/ResearchWarning.svg`; its 54 x 53
RGBA rendering is checked in as `ResearchWarning.png` in both movies. Repeated updates
keep the pulse phase; changing activity starts the new overlay fully visible.
The top HUD remains visible during aiming. World markers have alpha 0.55, fill alpha 0.12,
no opaque backing and sit below player icons in the movie's display list.

At 0x23B207, after the original update_player_icons call, Rust sends
`root.research_markers(team, count, [cp_id, projected, x, y, distance, owner, capturing,
progress, ms_left, local_inside] * count, [contested] * count, aiming, [advancing_team] * count)` on change and
every 60 frames. Projection uses the engine library's point_to_screen with the point
raised 2 m. Markers behind the camera or off screen are hidden; this is not wall occlusion.
The appended `advancing_team` is the sole occupying team (0/1, 2 for empty or contested),
not `capturing`: the latter remains the defender while its progress drains. The movie
combines this team with `ms_left > 0` to show an enemy warning. Older senders omit the
tail and show no warning rather than infer the wrong team; older movies ignore the tail.
The top HUD derives the same activity from its existing occupancy counts and timer.
No rule state, network message, capture timing, scoring or replay format changes.

Aiming follows the actual weapon state, not the scope attachment or mouse input:
game+4D58h is the input handler, camera director+84h must equal input+4, input+358h
must be first-person mode 0 or attached-spectator mode 4. Find the live viewed player
whose +F470h is that handler, then its held weapon (+128h, vtable A04174h); +5E8h
is the aimed byte. The observed writers are 2E1B5A and 2E1BFB. Iron sights therefore
hide markers too. Free cameras and dead players do not inherit another player's aim.

Visibility belongs to the world-marker parent Sprite: smoothstep to zero in 200 ms,
to one in 300 ms. Repeated packets do not restart the fade; reversals begin at the
current alpha. Only the activity overlay pulses; the capture icon stays steady. The overlay
animates a child, so it cannot reveal a hidden marker. Hidden/removed markers stop their
activity animation. The normal hide-interface key
still hides this parent.

Retail reference: 0.27d's CapturePointMarker used 24/34 px and alpha 195/255; its
crossed-swords art is reused. Its frame-based triangle pulse sped up at higher FPS.
The new elapsed-time pulse keeps the same two-second period at 30, 60 and 144 FPS.

Research's respawn and pre-match countdowns use 18 px text with a dark outline,
below the point icons and capture timers. If both messages arrive during a transition,
they occupy separate rows. The layout helper restores the original label geometry,
font and layout alignment when the HUD switches out of mode 5; other modes keep their
existing timers. The engine's countdown values and death-screen state are unchanged.

Research centres the score bars on the live stage midpoint, converted into the HUD's
coordinate space, before following them with its point icons. It does not depend on
the generic layout's initial viewport or the changing width of its labels; only its
horizontal layout is overridden, and switching modes restores the stock alignment.
Countdown spacing counts visible point graphics and timers only, with a 4 px gap:
hidden capture timers cannot leave an empty row after the capture ends.

The Research respawn text is a separate TextField under the score indicator. It
reads the engine's label value synchronously, inherits the indicator's visibility
and scale, and hides the original label only while Research owns the layout.
Changing the original label's TextField auto-size fought CLIK's own sizing and
timeline, producing a narrow flash on death. The separate field is unaffected by
late label validation or animation; leaving Research removes it and restores the
native label's visibility without changing that label's font or geometry.
