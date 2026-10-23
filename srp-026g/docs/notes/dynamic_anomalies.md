# Dynamic anomalies (exe patch 42 and the content pack `dynamic-anomalies`)

The owner (2026-10-20): "the anomalies in the release of london were dynamic, they appeared
with artifacts and disappeared when artifacts were picked up, and they used to spawn in many
lcoations"; "now there are just 7 static places". Start: "you should start worker on D". Not
for the current release.

Then (2026-10-20): "we don't need first stage, … finish D properly and completely", as the
release did it. 2026-10-22: "we already have a lot of work done for them, so let's finish
it", for the release after revision 28. Then (2026-10-22): "dynamic anomalies should work
differently, they should only work for london and keln, other maps keep static". The rules
below are what the code and the data follow. Code: `client-patches/dynamic_anomalies_026g.asm`
(README patch 42, `client_patch.md` §3.38), checks `client-patches/check_dynamic_anomalies_026g.py`
and the self-test's check 42; data: `scripts/dynamic-anomalies/build.py --pack`, the `pack
dynamic-anomalies@3` row of `release/content.tsv`; server: `match-server` `bots::anomalies`,
`mapdata-dump --patch`. PROVEN = read in the code (addresses given); LIKELY = inferred, not
read to the end.

## Spec

- **Where (the owner, 2026-10-22):** London (level_09) and Cologne (level_08) only, every mode
  and weather they have. Every other map keeps its static anomalies as they are: level_01 to
  level_07, v021d_/v011e_ and Research's (mode 5, #297). Their projects are not in the pack,
  and no core of theirs is timed (stock cores' artefacts come back within 80 s), so patch 42
  does nothing there. A place = three timed anomalies (lifebone, spring, onyx) at one point;
  at most one of them lives at a time.
- **An anomaly's life:** idle (nothing) -> charging 20 s (the charging particle, harmless) ->
  damage 4 s (the damage zone) -> artefact (one artefact in its container, the zone on, until
  someone takes it) -> after the pick-up: gone at once, 60 s rest -> idle.
- **Damage (the owner, 2026-10-23: "with proper damage values").** Each kind's damage zone
  hurts as the release's did: 2 s to death, unarmoured, at full intensity. The release
  measured is 0.28d0, the first with the mechanism and Cologne's source. Its zones are 0.5
  per part for every kind, but its body routes a part's hit at 0.1 to 0.15 into one health
  pool. 0.26g routes each part at 1 into its own pools, pain included (`formula.txt`
  section 8), so the same 0.5 killed 8 to 9 times faster here (pack @2). The values
  (`build.py` `MAX_HIT`, pack @3):

  | kind (artefact) | type | parts hit | max_hit a part | its fatal pool, a second | death | 0.28d0 | 0.26g's static zones of the type |
  |---|---|---|---|---|---|---|---|
  | lifebone (54) | toxic | face | 0.5 | infection 0.5 | 2.0 s | 0.5 x face, 2.0 s | 0.05 to 0.25 x face, 4 to 20 s; v021d_level_06 1 x 3 parts, 0.33 s |
  | spring (74) | shock | back, front, brain, both hips, feet, arms (9) | 0.0625 (was 0.5) | pain 0.5 (body 0.125) | 2.0 s (was 0.25 s) | 0.5 x the 9, 2.0 s | level_03's pairs 0.6 x 2 feet a hit, one a second; v021d_level_03 0.9 x 6 parts every 2 s: 0.7 to 1.0 s |
  | onyx (76) | radiation | the same 9 | 0.0556 (was 0.5) | infection 0.5 | 2.0 s (was 0.22 s) | 0.5 x the 9, 2.0 s | 0.023 to 0.08 x 4 or 5 parts, 2.5 to 11 s |

  The same for every kind and place:
  - continuous, `interval_in_msec` 1000, `max_armor_piercing` 0;
  - the release's hit curve: full intensity to 0.9 of the way out, 0.1 at the edge;
  - the release's zone shape (geometry mesh type 2): scale 2.5 x 1.43 x 2.5 indoors, 4.0 x
    1.96 x 4.0 outdoors;
  - armour lowers each part's hit as for any hit of its type.

  The timings are the release's: charging 20 s (the charge zone: max_hit 0, harmless),
  damage 4 s, then the zone stays on while the artefact waits.
- **Sponge (the owner, 2026-10-23): Sponge is not in the game yet; anomalies hold only
  0.26g's artefacts.** A timed core's artefacts are one row of its own kind: 54 lifebone, 74
  spring or 76 onyx, the only artefacts of 0.26g's `items_dict`. `build.py` refuses any other
  kind. The release's fourth kind, mochalka (758, the Sponge; 0.28's Sponge port is parked as
  #311), is left out with its anomalies (section 5).
- **Which one charges:** first try at the match's start + 30 s (not in the warmup or the
  countdown), then a try every 20 to 40 s. A try picks one idle anomaly at random that has a
  free artefact and no busy anomaly within 15 m. None while 16 are active.
- **Removed:** London's 7 always-on patch_14 places (the release removed them). Cologne had
  none. The other maps' static anomalies stay (the school's fruit cloud, Vostok's electric
  pairs, the swamp, ...).
- **Rewards:** an artefact taken is the game's statistics event 5, paid as today (score, the
  find-artefacts quest, the explorer award). Nothing new on the server.
- **Late joiners, resyncs, rollbacks, replays:** nothing outside the world; the same choice
  everywhere. The snapshot, its hash and the server's decoders are unchanged. Replays recorded
  before the release are not watchable after it (`simulation_changes.tsv`).
- **Bots** (testing only): keep off a timed anomaly's damage zone while a checked world shows
  it charging, hurting or holding its artefact; never predict the next choice; take no
  artefacts.
- **Release:** exe patch 42 (every full build; its code the block `anomalies.code` of the
  `.srpx` arena, linked by `link_026g.py`) and the content pack in one release; the exe
  requires that release's content patch (patch 49), and the server refuses older exes.
- **Not as the release:** no mochalka (0.26g has no such artefact); state switches a tick or
  two late; no anomaly-detector zones (section 5).

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
- **The match.** The time rule is the rules vector's first (world+0x18824; state +0x148, 3 =
  in process from +0x12C). In a game it is the client class `timelimit_rule` (vtable
  0xA06E8C): the game's constructor makes `timelimit_rule_cook` (0x1E9AEA, vtable 0x9FD944),
  whose create (0x206670) builds it with 0x267A40, which stores 0xA06E8C at 0x267A61 after the
  core's constructor 0x317DA0 stored `timelimit_rule_core`'s 0xA06E2C. The rule knows the time
  rule by the slot both share, serialize 0x317F30 at +0x3C (no other place of the exe holds
  that address). The first build compared the vtable with 0xA06E2C only, which a game never
  has: no timed anomaly would ever have charged (found by #225's review, 2026-10-23; fixed). The static info is world+0x18FAC (0x2D00BB), its
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

The release had timed anomalies on every map (every map config of 0.28a2, 0.28d0, 0.31a2 and
0.46c2 has the same `anomalies` table; none of 0.26g's maps has one). The owner (2026-10-22)
wants them on London and Cologne only, so the pack (`scripts/dynamic-anomalies/build.py`) has
those two maps' projects and nothing of the others. Places per mode 2 / 3 / 4, three timed
anomalies per place:

| map | from | places | removed (as the release) |
|---|---|---|---|
| level_08 Cologne | 0.28d0 | 29 everywhere | none |
| level_09 London | 0.31a2 | 27 everywhere | patch 14's 7 always-on places |

- 12 projects (Cologne: day, evening and prerain of modes 2 to 4; London: day of modes 2 to 4;
  every weather of a mode takes the release's `day_<mode>`, whose weathers have the same
  places), 342 places, 1,026 timed anomalies, 5 particles: 17 pack operations.
- Static, as today: level_01 to level_07 (the swamp, the school's fruit cloud and stalactite,
  Vostok's electric pairs and the rest), v021d_/v011e_, Research's mode-5 maps.
- The release's Cologne is 0.26g's (visuals and collision equal but for equivalent rotations
  and a few edits). LIKELY fine; to be seen in a game.
- Particles the game lacked (`pre_onix1`, `pre_onix2`, `pre_spring1`, `pre_spring2`,
  `pre_zhivokost1`) come from 0.31a2 as patch 14 converted its own four; every material
  they name is in the game.
- A project now holds up to 65,902 config values (London day_3; Cologne up to 47,500). That
  is no limit: the live v011e_level_03 day_2 (patch 28) loads with 112,899. The format has no
  total limit (a table's count is u16 only).
- **What a match must hold** (#225's review, 2026-10-23): the saved world goes through three
  fixed buffers: 0x2000 for the objects' state, 0x2000 for the second stream, 0x8000 for the
  whole world. Cologne's 87 timed cores need about 3.2 to 3.5 KB of them, against about
  5.4 KB free in the fullest live 20-player world. To see in a full 20-player Cologne match.

## 7. The server

- Nothing in srp, the lobby or the match server reads anomalies, except the bots: the server
  data keeps a project's `server_objects` only, and nothing new is in a snapshot.
- The bots (`match-server` `bots::anomalies`, wired since before this) find each core in the
  checked worlds by its random value and run it on: a timed anomaly is idle until a world
  shows it charging (the clients' choice is never predicted), then its timers take it to
  damage and the artefact state; a world after the pick-up turns it off. Fixed for it: a
  state's end is taken on the first tick past it (as the client), and the tick after an
  `energy_on_exit` chooses the next state (the simulation used to skip it when idle).
- The bots' map files of London and Cologne must be dumped again (the other maps' do not
  change): `mapdata-dump --db <stock> --patch <each live patch> --patch <this release's>
  --project level_08/<variant> --project level_09/day_N --out <dir>` (`--patch` lays patches
  over the db as the game mounts them).
- More artefacts are found on London and Cologne (every 20 to 40 s): statistics event 5
  (artefact) pays score, the find-artefacts quest and the explorer award (`session`
  `rewards.rs`, event::ARTEFACT), as today. An owner's call, not a code change.
- The site's Maps tab draws one artefact marker per place (the kinds of a place's
  containers within 1.5 m together).

## 8. Release and test

- **Release** (one `release.sh` run with the content):
  - exe patch 42 (every full build): the arena's block `anomalies.code` in `.srpx`
    (`link_026g.py`, client_patch.md 0.3), the two hooks by symbol;
  - the content: the pack `dynamic-anomalies@3` (row in `release/content.tsv`). Its folder
    is made once into `$SKHID_PACKS_DIR/dynamic-anomalies-3` by `build.py --pack
    dynamic-anomalies@3` with `--live` = the stock db and the base rows' patches (12, 14, 17,
    19 to 24), as `scripts/dynamic-anomalies/README.md` says; `release/build-content` then
    composes it into the release's `resources.db.patch_<n>`;
  - the release's exe requires that patch (patch 49) and `deploy` sets
    `SRP_MIN_PATCH_REVISION`: a game without patch 42 never charges a timed anomaly and would
    desync;
  - the bots' map files of London and Cologne dumped again (testing only, section 7);
  - the `next` row of `simulation_changes.tsv` (old replays are not watchable after it).
- **Test** (only with the owner's go; the kit:
  `private/local-clients/dynamic-anomalies-test/HOW-TO-TEST.md`):
  - London, two clients: an anomaly's whole life (charging -> damage -> artefact -> pick-up
    -> gone); a late joiner; no resync;
  - Cologne: the same timed anomalies;
  - a static map (level_02 School): its anomalies as today (the fruit cloud there), nothing
    timed.
