# Dynamic anomalies (exe patch 42 and a resources.db patch)

The owner (2026-10-20): "the anomalies in the release of london were dynamic, they appeared
with artifacts and disappeared when artifacts were picked up, and they used to spawn in many
lcoations"; "now there are just 7 static places". Start: "you should start worker on D". Not
for the current release.

Then (2026-10-20): "we don't need first stage, … finish D properly and completely", as the
release did it. The rules below are what the code and the data follow. Code:
`client-patches/dynamic_anomalies_026g.asm` (README patch 42, `client_patch.md` §3.38),
checks `client-patches/check_dynamic_anomalies_026g.py` and the self-test's check 42; data:
`scripts/dynamic-anomalies/`; server: `match-server` `bots::anomalies`, `mapdata-dump --patch`.
PROVEN = read in the code (addresses given); LIKELY = inferred, not read to the end.

## 1. The release mechanism (0.28a2 to 0.46c2)

- **Where it lives.** New in 0.28a2 (0.27d0 has none of it; strings `timeout_anomaly_core`,
  `timeout_anomalies_rule_core`, `max_active_anomalies_count` in 0.28a2, 0.28d0, 0.31a2,
  0.46c2). PROVEN (strings). Read in the 0.28a2 exe, BCJ-decoded; 0.31a2's is LIKELY the
  same code (same strings, same data).
- **Two classes.** `timeout_anomaly_core` (one anomaly; client side `timeout_anomaly` adds the
  particles) and `timeout_anomalies_rule_core` (one rule per match, mode rule id 135 in 0.28,
  the mode config's `anomalies` table). PROVEN (RTTI, cooks, config keys).
- **One anomaly** (`timeout_anomaly_core`, 0.28a2 tick 0x5F41C0, serialize 0x5F3EE0). Fields:
  `m_state` (u8), `m_start_charging_time_ms`, `m_start_post_artefact_time_ms`, the three
  timeouts, its artefact container, damage zone, detector zone, and its own artefacts
  (`artefact_dict_id`). States, each switched on the exact tick (`now == start + timeouts`):
  - 0 idle: invisible, harmless, no artefact;
  - 1 charging, `charging_state_timeout` (20 s): the charging particle;
  - 2 damage, `damage_state_timeout` (4 s): the damage zone and detector zone switch on;
  - 3 artefact: an artefact is put into its container (0x5F4360, the first of its own
    artefacts not held by anyone); the zone stays on; it lasts until the artefact is taken;
  - 4 post-artefact: the container's taker calls 0x5F72C0: zones off, wait
    `m_post_artefact_state_timeout_ms` = the rule's `respawn_anomalies_timeout_s` (60 s),
    then back to 0.
  PROVEN.
- **The rule** (`timeout_anomalies_rule_core`, constructor 0x5F7B30, tick 0x5F7C00, choice
  0x5F7EC0). Values, all maps of 0.28a2 to 0.31a2 alike: `spawn_anomalies_delay_s` 30,
  `try_spawn_anomaly_interval_s` 30, `try_spawn_anomaly_variation_s` 10,
  `max_active_anomalies_count` 16, `block_new_anomalies_spawn_radius` 15,
  `respawn_anomalies_timeout_s` 60. PROVEN (code and data). Per tick:
  - nothing before match start + 30 s (start = the world's start time), nor before
    `m_next_try_spawn_time_ms`;
  - a try sets the next one at now + 30 s + uniform [-10, +10) s;
  - it counts the anomalies in states 1 to 3; at 16 or more nothing more;
  - the candidates: every anomaly of the map in state 0, one of its own artefacts free, and no
    anomaly in a state other than 0 (post-artefact included) closer than 15 m (container
    positions, 3D);
  - one candidate, uniform, gets `m_start_charging_time_ms = now`: it charges from this tick.
  - Random: the rule's own LCG `x = x·0x08088405 + 1`, index `(x·n) >> 32`, seeded from the
    constructor; `m_random.seed()` and the next try time are its serialized state.
  PROVEN.
- **"Many locations".** A place is four anomalies (lifebone 54, spring 74, onyx 76, mochalka
  758), one per artefact kind, at the same point: the 15 m rule lets one of the four live at a
  time. London (0.31a2, every weather) has 27 places = 108 timed anomalies, 20 under
  `mode_random_anomaly/`, 7 under `terrain/`; level_01 of 0.28d0 has 75 places. All 108 have
  charging 20 s, damage 4 s, container search 2 s, cooldown 0. PROVEN (data).
- **What a port of the classes takes** (option a below). The anomaly TU is about 14 KB in
  0.28a2 (0x5F3B00..0x5F7330 with its helpers), the rule about 3 KB, plus two cooks, the
  client particles class and the factory. It builds on things 0.26g lacks: a new
  `base_anomaly_core`, the 0.28 world-object layout (subobjects at +0x188..+0x19C against
  0.26g's +0x118), name-resolved links of another shape, the 0.28 factory and rule cook. LIKELY.

## 2. 0.26g, as far as the port needs it

- **Factory.** `project_cooker_simple::create_game_objects` 0x22C010 compares each
  `game_object_type`; `generic_anomaly_core` → class 0x7E (0x22D988). `timeout_anomaly_core`
  is not known. PROVEN.
- **`generic_anomaly_core`** (vtable 0xA169CC; load 0x33BD50; tick 0x31B5B0 run by its own
  tick delegate at +0x194, fn +0x198, set by the constructor 0x31ADF0). Fields:
  `+0x124` artefacts_enabled, `+0x12C` artefacts_respawn_time_sec (f32), `+0x138` artefact
  pending, `+0x13C` its time, `+0x140/+0x144` containers, `+0x14C` energy_enabled, `+0x150`
  energy (f32), `+0x154` energy_maximum (f32), `+0x158` energy_decrease_speed, `+0x15C`
  energy_af_container_use, `+0x180/+0x184` states (sorted, highest threshold first), `+0x18C`
  current state, `+0x1AC` world, `+0x1B0/+0x1B4` its own artefacts (pointers, count; an
  artefact's `+0x280` is 0 while free), `+0x1B8` the core's LCG. A state: `+0` enabled, `+8`
  energy_threshold (u32), `+0x10` active_time_sec, `+0x14` energy_on_exit, `+0x1C/+0x20`
  groups, `+0x28` its core, `+0x2C` its end time. PROVEN.
- **States are a timer chain.** `select_state` 0x31BA90 keeps a state while its end is set,
  else takes the first enabled state with `energy >= threshold`; `anomaly_state::initialize`
  0x31BC10 sets the end at now + `active_time_sec`·1000; `anomaly_state::execute` 0x31BCB0
  sets `energy = energy_on_exit` on the first tick past the end (`now > end`). The stock fruit
  anomaly (level_02) uses it. PROVEN.
- **Energy from players.** `on_player_action` 0x33BB50 adds `energy_on_*` × param for every
  action reported to the core and clamps to `energy_maximum` when that is above 0 (with 0
  added too). So a timed core keeps `energy_maximum` 0: a clamp would end an anomaly a
  player walks into. PROVEN.
- **Pick-up.** `artefact_container_core::use_execute` 0x31C780: the artefact goes to the
  player, the container's cooldown starts, then the owner core's `on_artefact_container_use`
  0x33BCD0 (vtable +0x28): energy += `energy_af_container_use` (clamped as above), then an
  artefact is due again at now + `artefact_respawn_delay` 0x31BA00. PROVEN.
- **Artefacts.** `spawn_artefact` 0x31B6C0 runs when pending and its time has come: a chance
  draw, a free container (none holding one, cooldown over), one of the core's free artefacts,
  then pending = false when the containers hold `max_artefacts_count`. PROVEN.
- **The world's tick.** `game_world_core::atomic_tick` 0x2D1E20 (`[esp+4]` the world, edi now,
  `[esp+8]` the tick's length) runs the seven tick lists at +0x18840 (16 bytes each) through
  0x2D1D20. The anomaly cores are list 6: `server_game_project::set_game_world_core` puts each
  core's delegate there in the project's order (0x318B26; first node world+0x188A4, next at
  +0xC), keyed by a counter. PROVEN.
- **The match.** The time rule is the rules vector's first (world+0x18824; vtable 0xA06E2C;
  state +0x148, 3 = in process from +0x12C). The static info is world+0x18FAC (0x2D00BB), its
  seed (the anomaly seed of every client, static_info.md) +0x577B0 (0x2D01D7). PROVEN.
- **A place.** A container's use box: container+0x38/+0x3C (its geometries, usable_object::load
  0x2F81F0) → collision_geometry+0x18 (the ghost, collision_geometry::load 0x2F8C30) → +0x18
  (the bullet object, bt_ghost_object 0x3578F0) → +0x40 (the world transform's origin,
  btCollisionObject 0x8E870, set by create_ghost_object 0x357AA0 through 0x70C20). PROVEN
  (offsets read in each function; the chain itself not run in a game).
- **Lockstep.** Every client simulates anomalies; a world snapshot (late joiners, resyncs,
  rollbacks, the 50 ms hash) is each object's `serialize`: a generic core writes pending +
  its time, energy, state, its end, groups, zones and its LCG (0x31AF20). PROVEN (match.md §3,
  bots/anomalies.rs).

## 3. The design (approach b)

- Considered: (a) port the 0.28 classes (20+ KB against a drifted layout, a new rule in the
  rules blob that the server's decoder would have to learn); (b) the release's timeline on
  stock generic cores (data) and only the rule in the exe; (c) a hybrid. Built: (b).
- **A timed anomaly** is a stock `generic_anomaly_core` with `energy_enabled` and
  `artefacts_respawn_time_sec` ≥ 100000 (its artefacts never respawn by themselves; no stock
  core has more than 80). Its states (highest first):

  | state | threshold | active_time | energy_on_exit | zones |
  |---|---|---|---|---|
  | charging | 3000 | charging_state_timeout (20 s) | 2000 | charge zone |
  | damage | 2000 | damage_state_timeout (4 s) | 1000 | damage zone + charge zone |
  | artefact | 1000 | 0 (until taken) | — | damage zone |
  | after the pick-up | energy_af_container_use (50) | respawn_anomalies_timeout_s (60 s) | 0 | none |
  | idle | 0 | 0 | — | none |

  `energy_initial` 0, `energy_maximum` 0, `energy_decrease_speed` 0, every `energy_on_*` 0,
  `energy_af_container_use` 50; artefacts [{its kind, 5}], one container,
  `max_artefacts_count` 1, `artefacts_first_spawn_time_sec` 100000, chance 1. Each zone group
  keep-all, not sequential. The charge zone hurts nobody (max_hit 0) and shows the release's
  charging particle (`pre_<kind>1`) at the artefact; the damage zone shows the release's
  damage / artefact-state particle (`pre_<kind>2`, `_indoor` indoors).
- **The exe** (README patch 42) adds two hooks, nothing else: the rule at the start of every
  world tick, and the pick-up of a timed anomaly's artefact (energy = its after-the-pick-up
  level, no artefact due). Everything else is the game's own code.
- **Nothing new is kept or serialized**: the rule's inputs are the world (energies, states,
  artefacts, places, the time rule), the clock and the static seed; its outputs are the
  chosen core's energy, pending and its time, all in the snapshot. A rollback, a resync and
  a late joiner choose alike; the snapshot's format, its hash and the server's decoders do
  not change.

## 4. The rules (spec)

- Only timed anomalies take part; every other core plays as before (stock pick-up included).
- **Tries**: none before the match is in process (waiting, countdown). The first at the
  match's start + 30 s; each next one 20 to 40 s (uniform) after the tick that made the last
  (the release: next = now + 30 s ± 10 s). A try happens in the first tick at or past its time.
- **At a try**: busy = energy > 0 (charging, damage, artefact, after the pick-up); active =
  energy above the after-the-pick-up level (charging, damage, artefact). 16 or more active:
  nothing. Candidates: idle timed anomalies (energy 0) with one of their own artefacts free,
  a place, and no busy timed anomaly closer than 15 m (3D, between the containers' use
  boxes). One candidate, uniform, in the project's order.
- **The chosen one**: charging from this tick (energy = its top state's threshold); its
  artefact due when its timed states above the pick-up level have run (charging + damage =
  24 s), as the release put it into the container at the end of damage.
- **The pick-up**: the anomaly is gone at once (its zones off with the state) and waits
  60 s (the after-the-pick-up state), then is idle and may be chosen again.
- **Late joiners, resyncs, rollbacks**: nothing of the rule is kept outside the world.
- **Bots** (server): never predict a choice; follow the states the checked worlds give.

## 5. What is not as the release (and why)

- **The states switch a tick or two late.** 0.26g's state end is whole seconds and ends on
  the first tick past it, and the next state is chosen on the tick after: damage begins
  20.1 s after the choice (the release: 20 s), the artefact state 24.2 s (24 s). The
  artefact itself lands at 24 s, as in the release. The game's own state code; not worth an
  exe change.
- **Two random streams.** The release drew the next try time and the choice from one LCG
  whose use depended on the world (a draw for the choice only when there were candidates);
  here the tries come from a stream of the seed alone and each try's choice from a draw of
  that try's, so that nothing has to be kept. Same distributions.
- **No mochalka.** The release's fourth kind (758, "Sponge", a thrown artefact, data type 13)
  does not exist in 0.26g (only 54, 74 and 76, no thrown kind); mapping it to onyx (the
  closest by role) would double the onyx anomaly at every place. Its anomalies are left out:
  each place has three kinds, one alive at a time, as before.
- **No anomaly detector zones.** 0.26g has no `anomaly_detector` effect.
- **Damage zones in 0.26g's form** (patch 14's: toxic/shock/radiation per kind, its sounds),
  the release's particles on them.

## 6. Maps

The release had timed anomalies on every map, not only London (every map config of 0.28a2,
0.28d0, 0.31a2 and 0.46c2 has the same `anomalies` table; none of 0.26g's maps has one). So
every map we ship from the release gets them (`scripts/dynamic-anomalies/build.py`, one
resources.db patch). Places per mode 2 / 3 / 4, three timed anomalies per place:

| map | from | places | the release removed (so here too) |
|---|---|---|---|
| level_01 Rudnya | 0.28d0 | 75 / 71 / 61 | the swamp anomaly (2 cores) |
| level_02 School | 0.28d0 | 86 / 84 / 85 | the fruit cloud and the stalactite (2 cores, 24 zones) |
| level_03 Vostok | 0.28d0 | 58 / 58 / 45 | the antenna's electric pairs and the rust anomalies (5 cores) |
| level_04 Chem plant | 0.28d0 | 53 / 53 / 39 | 2 cores |
| level_05 Fort | 0.28d0 | 67 / 63 / 65 | 2 cores |
| level_06 Mamayev | 0.28d0 | 43 / 39 / 43 | the acid and fire anomalies (3 cores) |
| level_07 Vector | 0.28d0 | 30 / 28 / 30 | the artefact spawner (1 core) |
| level_08 Cologne | 0.28d0 | 29 everywhere | none |
| level_09 London | 0.31a2 | 27 everywhere | patch 14's 7 always-on places |

- 75 projects (every weather of a mode takes the release's `day_<mode>`: its weathers have the
  same places), 3,870 places, 11,610 timed anomalies; v021d_/v011e_ maps are not from the
  release and keep their anomalies.
- The release's maps are 0.26g's (visuals and collision equal but for equivalent rotations
  and a few edits; on level_05 0.26g has 336 bush collisions more, two within 2 m of a place).
  LIKELY fine; to be seen in a game.
- Particles the game lacked (`pre_onix1`, `pre_onix2`, `pre_spring1`, `pre_spring2`,
  `pre_zhivokost1`) come from 0.31a2 as patch 14 converted its own four; every material
  they name is in the game.
- A project now holds up to 103,000 config values (level_02); the largest loaded so far is
  patch 14's London (43,799). The format has no total limit (a table's count is u16 only;
  the largest table, client_objects, stays far below): to be seen in a game.

## 7. The server

- Nothing in srp, the lobby or the match server reads anomalies, except the bots: the server
  data keeps a project's `server_objects` only, and nothing new is in a snapshot.
- The bots (`match-server` `bots::anomalies`, wired since before this) find each core in the
  checked worlds by its random value and run it on: a timed anomaly is idle until a world
  shows it charging (the clients' choice is never predicted), then its timers take it to
  damage and the artefact state; a world after the pick-up turns it off. Fixed for it: a
  state's end is taken on the first tick past it (as the client), and the tick after an
  `energy_on_exit` chooses the next state (the simulation used to skip it when idle).
- The bots' map files must be dumped again for every map: `mapdata-dump --db <stock> --patch
  <each live patch> --patch <this release's> --out <dir>` (the new `--patch` lays patches over
  the db as the game mounts them; `--project level_09/day_N` for London).
- More artefacts are found (every map, every 20 to 40 s): statistics event 5 (artefact) pays
  score and the find-artefacts quest. An owner's call, not a code change.
- The site's Maps tab draws one artefact marker per place (the kinds of a place's
  containers within 1.5 m together).

## 8. Release and test

- **Release**: exe patch 42 (`.srpa`, the default build), the content patch (the
  `build.py` output packed by `release/build-patch`), a server version (server data with
  the patch, `SRP_MIN_PATCH_REVISION` = the release: a game without patch 42 never charges a
  timed anomaly and would desync), the bots' map files dumped again, the `next` row of
  `simulation_changes.tsv` (old replays are not watchable after it).
- **Test** (only with the owner's go): two clients on London; a late joiner; a resync; an
  anomaly's whole life (charging → damage → artefact → pick-up → gone); another map. The
  kit: `private/local-clients/dynamic-anomalies-test/HOW-TO-TEST.md`.
