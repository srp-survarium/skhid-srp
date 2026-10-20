# Dynamic anomalies (option D, README patch 42)

The owner (2026-10-20): "the anomalies in the release of london were dynamic, they appeared
with artifacts and disappeared when artifacts were picked up, and they used to spawn in many
lcoations"; "now there are just 7 static places". Start: "you should start worker on D". Not
for the current release.

Scope, plan and rules for bringing the release's timed anomalies to 0.26g. Code (milestone 1):
`client-patches/dynamic_anomalies_026g.asm`, check `client-patches/check_dynamic_anomalies_026g.py`.
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
  tick delegate at +0x194, fn +0x198, set by the constructor 0x31ADF0). Fields used here:
  `+0x124` artefacts_enabled, `+0x138` artefact pending, `+0x13C` its time, `+0x140/+0x144`
  containers, `+0x14C` energy_enabled, `+0x150` energy (f32), `+0x154` energy_maximum (f32),
  `+0x158` energy_decrease_speed, `+0x15C` energy_af_container_use, `+0x180/+0x184` states
  (sorted, highest threshold first), `+0x18C` current state, `+0x1AC` world, `+0x1B0/+0x1B4` its own
  artefacts (pointers, count; an artefact's `+0x280` is 0 while free), `+0x1B8` the core's
  LCG. The damage zones are the states' groups'. A state: `+0` enabled, `+8` energy_threshold (u32),
  `+0x10` active_time_sec, `+0x14` energy_on_exit, `+0x2C` its end time. PROVEN.
- **States already are a timer chain.** `select_state` 0x31BA90 keeps a state while its end
  time is set; else it takes the first enabled state with `energy >= threshold`.
  `anomaly_state::initialize` 0x31BC10 sets the end at now + `active_time_sec`·1000;
  `anomaly_state::execute` 0x31BCB0 sets `energy = energy_on_exit` when the end has passed.
  The stock fruit anomaly (level_02) uses it. PROVEN.
- **Pick-up.** `artefact_container_core::use_execute` 0x31C780: the artefact goes to the
  player, the container's cooldown starts, then the owner core's
  `on_artefact_container_use` 0x33BCD0 (vtable +0x28): if energy_enabled, energy +=
  `energy_af_container_use`, **clamped to `energy_maximum` when that is above 0**; then the
  artefact is pending again at now + respawn time. PROVEN. No stock map sets
  `energy_maximum` (all 0). PROVEN (data of levels 01 to 08 and London).
- **Artefacts.** `spawn_artefact` 0x31B6C0 runs when pending and its time has come: a chance
  draw, a free container (none holding one, cooldown over), one of the core's free artefacts,
  then pending = false when the cores' containers hold `max_artefacts_count`. PROVEN.
- **The world's tick.** `game_world_core::atomic_tick` 0x2D1E20 (EDI = now) runs the seven
  tick lists at +0x18840 (16 bytes each) in order through 0x2D1D20. PROVEN. A generic core's
  delegate sits in one of them (the list not traced; LIKELY 0).
- **Lockstep.** Every client simulates anomalies; a world snapshot (late joiners, resyncs,
  rollbacks, the 50 ms hash) is each object's `serialize`: a generic core writes pending +
  time, energy, state, its end time, groups, zones and its LCG (0x31AF20). So energy, state,
  state end and pending/time are already synced, rolled back and hashed. PROVEN (match.md §3,
  bots/anomalies.rs).

## 3. Approaches

- **(a) Port the 0.28 classes** (new class id in the factory, new rule class id, cooks,
  serialization, client particles; C on 0.26g's interfaces as for Research mode). Faithful,
  but 20+ KB of rewrite against a drifted layout, a new rule in the rules blob (the server's
  `rules.rs` decoder must learn it), new object state the bots' reader must skip. Weeks;
  risk high.
- **(b) Timed generic cores + a small scheduler (recommended).** Each London anomaly becomes
  a 0.26g `generic_anomaly_core` whose states are the 0.28 timeline, driven only by energy:
  | state | threshold | active_time | energy_on_exit | groups |
  |---|---|---|---|---|
  | charging | A = 3000 | 20 s | 2000 | the charging particle (a zone that does not hurt) |
  | damage | 2000 | 4 s | 1000 | the damage zone (+ detector) |
  | artefact | 1000 | 0 | — | the damage zone, the artefact-state particle |
  | after the pick-up | M = 50 | 60 s | 0 | none |
  | idle | 0 | 0 | — | none |
  `energy_enabled` 1, `energy_maximum` M (the marker: no stock core has one), every
  `energy_on_*` 0, `energy_decrease_speed` 0, `energy_initial` 0, one container, one kind of
  artefact, `max_artefacts_count` 1, `artefacts_first_spawn_time_sec` and
  `artefacts_respawn_time_sec` beyond any match (100000).
  - The pick-up needs **no exe change**: the stock clamp drops the energy to M → after-pick-up
    state (zones off) → 60 s → idle. PROVEN by the code of 0x33BCD0 and 0x31BCB0; to be seen
    in a game.
  - The exe adds **only the rule's choice** (README patch 42): once per world tick, at the
    release's times, pick one idle timed core and set its energy to its top threshold
    (charging starts) and its artefact pending at now + 24 s (the artefact lands as the
    artefact state begins).
  - Nothing new is serialized: the choice is a pure function of the world (energies,
    positions), the clock, the match start and the static seed. A rollback, a resync and a
    late joiner get it right with no new bytes; the hash, the server's snapshot decoder and
    the rules blob do not change.
  - Effort: about a week of exe work, a map rebuild (London project, a resources.db patch),
    a server bots follow-up. Risk: medium (the order of tick and scheduler; positions).
- **(c) Hybrid** (b's data with a ported 0.28 rule object for the choice): no gain over (b),
  and the rules-blob cost of (a).

## 4. Milestones of (b)

1. **The choice, alone** (this PR; README patch 42, not in the exe build yet): an assembly
   routine `dynamic_anomalies_026g.asm` that, given now, the tick's delta, the match start,
   the static seed and a table of `{core, x, y, z}`, decides whether a try falls in this tick
   and which idle timed core charges, and writes its energy and artefact time. Run with
   Unicorn on made-up cores. Effort: a day. Nothing changes in the game.
2. **The hook** (exe): from `game_world_core::atomic_tick` (or the generic cores' tick list),
   build the table from the world's tick lists (delegates whose function is 0x31B5B0, in
   list order = the same on every client), the containers' positions, the start time from
   the time rule (world+0x18824[0], +0x12C when +0x148 is 3), the static seed; call the
   routine. Unicorn on the game's own tick with made-up cores; inert on stock maps (no core
   has `energy_maximum`). Effort: 2 to 3 days. Needs: the delegates' list, where a
   container's position is, the static seed in the world (static_info.md: it reaches the
   world's +0x108).
3. **The data** (content): London's 27 places × 4 kinds as timed generic cores (section 3
   table), with their zones, containers and particles from 0.31a2; mochalka (758) mapped to
   a 0.26g artefact or left out; built as a resources.db patch with the 0.31a2 positions.
   `content_edit.py`/dbgrab check of the configs. Effort: 2 to 3 days.
4. **The server**: the London mapdata dumped again (bots' anomalies list); the bots' hazards
   treat a timed core's zones as on while its energy is at or above the damage threshold
   (from the checked worlds; the server never predicts the choice). Effort: 1 day.
5. **In a game** (only with the owner's go, on the shared GPU's turn): two clients, a late
   joiner and a resync on London; the hash stays equal; anomalies appear, an artefact lands,
   the pick-up switches the anomaly off. Release: an exe patch + a content patch + a server
   version + a `simulation_changes.tsv` row (old replays not watchable).

## 5. The rules (spec)

- Only maps whose cores carry `energy_maximum` > 0 (timed cores) have dynamic anomalies;
  every other map plays exactly as before.
- Tries: from match start + 30 s; try k at start + 30 s + 30·k s + an offset uniform in
  [-10, +10) s drawn from the static seed and k. A try happens in the tick that reaches its
  time (now - delta < T ≤ now). Before the match starts (waiting, countdown) none.
  The release drew each try from the last one (20 to 40 s apart); here each try has its own
  window (10 to 50 s apart, 30 s on average), so that nothing has to be kept.
- At a try: busy = energy > 0 (charging, damage, artefact, after the pick-up); active =
  energy > `energy_maximum` (charging, damage, artefact). 16 or more active: nothing.
  Candidates = idle timed cores (energy exactly 0) with one of their own artefacts free and
  no busy timed core closer than 15 m.
  One candidate, uniform by a draw of the same seed and k, in table order.
- The chosen core: energy = its top state's threshold; artefact pending at now + 24 s (its
  artefact state begins about 0.2 s later: each state ends on the first tick past its end).
- An artefact taken: the anomaly is gone at once (zones off) and cannot come back for 60 s.
- Late joiners, resyncs, rollbacks: nothing of the choice is kept outside the world.
- Bots: never predict the choice; read the energies from checked worlds.
