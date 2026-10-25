# The sync core: what decides what to simulate when (map, state machines, staged rewrite)

The owner, 2026-10-25: "Can we start a worker to completely rewrite
synchronization internals in C with our calling library?"; then "we need to
properly design a state machine with state transitions and world captures
and packets", "it shouldn't be random boolean flags", and "if it makes
sense, we can even rewrite it in Rust with proper enums, no_std, instead of
C".

**Why.** Live play has constant rollbacks in fights. Patch 71's freeze
detector shows "ticks 6 (resimulated 4-5), rollbacks 1" on almost every
frame, at 2.5-4 ms a tick, so 40-70 ms frames. After a stall, one frame did
"rollbacks 45, ticks 87 (resimulated 84)".

**Scope.** The orchestration only: which world is loaded, which ticks are
stepped, when the history is saved, and which packets move a client between
its states. The per-tick simulation stays the game's own: `atomic_tick`
0x2D1E20 with its tick delegates 0x2D1D20, and the world's `serialize` and
`deserialize`. **The determinism contract:** the same inputs at the same
step boundaries give the same world, bit for bit.

Tags as in the other notes: **PROVEN** (read in the exe at the address
given, and where marked RUN, executed with Unicorn on the original bytes),
**LIKELY**, **GUESS**. Server facts are marked **main** (srp `main`
d4c7c598) or **#448** (only on the open PR `fix/precise-input-desync`).
Every diagram is 7-bit ASCII.

Contents:
1. The map: data, functions, callers, network entries, existing hooks
2. What a frame does today (RUN), and what the freeze detector counts
3. The state machines: the rules, the client, the server's view of a
   client, a place, a late-join admission
4. Error paths as transitions
5. Invariants
6. Rust `no_std` instead of C: evaluation
7. The staged plan
8. The server's migration to enums
9. Stage (a) as built
10. Reusable places (a design)

---------------------------------------------------------------------------

## 1. The map

### 1.1 `game_world_core` (0.26g; 0.21d names from its PDB, the 0.21d
layout at `vostok-versions/versions/0.21d/builds/2010` pdb-carcass
`headers/survarium/game_world_core.h`)

The core is the match core + 1F8h (`network_client` +1F1C40h is the match
core; the render move passes `[game_world+4D4Ch] + 1F8h`, 0x1F7C2F). Offsets
below are from the core.

| Offset | 0.21d name | What | Evidence |
|---|---|---|---|
| +0h | (the event list) | **change nodes**: `{+0 count, +4 head, +8 tail, +10h pool}`; a node is `game_event_history_item`, 1C0h bytes: per place 14h bytes from +0 (`+0` events mask: bit 0 entered, bit 1 left, others input kinds; `+4..` aware masks), `+194h[kind]` server-aware mask, `+1A4h[kind]` mask, `+1B4h` next, `+1B8h` prev, `+1BCh` time | insert_impl 0x2D1100, erase_impl 0x2D1340, process_events 0x2D2180, 0x27E870. PROVEN |
| +2FB8h | m_game_states_history | **the history ring**: `{+0 count, +4 head (oldest), +8 tail (newest)}`; items from a pool of 11 (`(max_ping 250 + 250)/50 + 1`; 128 with `-debug_hash_mismatch`, 0x2CBFDC) | tick_events 0x2D2318..0x2D2351, 0x2D3F90, 0x2D4C30. PROVEN |
| +2FCCh | m_players_inputs_history | per place `+ place*480h`: the input nodes (27 a place; node `+8` input, `+10h` own part, `+18h` network part, `+20h` time, `+24h` precise, `+25h`/`+26h` own/network part present) | match.md 5.2, 16. PROVEN |
| +E3E4h | (network input times) | per place `+ place*190h`: the times that came from the network (the duplicate test) | match.md 18. PROVEN |
| +12280h | m_active_clients_history | **masks list**: nodes `{+0 next, +4 prev, +8 mask, +0Ch time}`, tail at +12288h; the hash and every load take the mask at or before the item's time | hash 0x325150, deserialize 0x2D277D. PROVEN |
| +12548h | m_game_state_hash_history | hashes of items the ring recycles (server mode only: `+18FDDh`) | new_game_state_history_item 0x2D09C0. PROVEN |
| +12740h | m_game_statistic_events_history | **world events** `{+0 count, +4 head, +8 tail, +10h pool}`, node time +24h, next +1Ch; a load drops every event after the loaded time (0x2D3700); queued unless +18FE0h | push_world_event 0x2D0A60, 0x2D3700. PROVEN |
| +18840h..+188B0h | m_tickable_objects | seven tick-delegate lists, 10h each, run in order by atomic_tick | 0x2D1E3F..0x2D1E66. PROVEN |
| +18F58h | m_bullet_manager | the bullet manager | deserialize 0x2D2AD8. PROVEN |
| +18FB4h | m_event_horizon_interval_in_ms | **the event horizon**, 500 on a client | ctor 0x2D00C7. PROVEN |
| +18FB8h | | an input lead in ticks, used by change_input's "move it" | 0x1FAD4D, 0x2D38B7. LIKELY |
| +18FBCh | m_start_fixed_time_in_ms | **the grid origin**, 50 | ctor 0x2D00CD. PROVEN |
| +18FC0h | m_current_time_in_ms | **the world's time now** (any ms, not only the grid) | atomic_tick 0x2D1E84, deserialize 0x2D2944. PROVEN |
| +18FC4h | m_max_current_time_in_ms | **the newest time ever ticked to**; a tick to a time not past it is a re-simulation | atomic_tick 0x2D1E6D..0x2D1E8A. PROVEN |
| +18FC8h | m_valid_time_in_ms | **the rollback point**: the newest grid time whose item every known input agrees with; -1 none. Lowered by insert/erase, reset to -1 by tick_events when it steps from the newest item, and by clear | 0x2D1328, 0x2D13E1, 0x2D225A, 0x2D077C. PROVEN, RUN |
| +18FCCh / +18FD0h | m_min_unsnapped_input_time_in_ms | the oldest unsent off-grid own input, per input kind (player, profile) | snap 0x2D4E40 / 0x2D4F80. PROVEN |
| +18FD4h | m_last_resync_time_in_ms | **the last world loaded from the network** (0 after 0xE5); inputs and roster changes at or before it are refused | match.md 5.1, 18. PROVEN |
| +18FD8h | m_active_clients_mask | the places in play now | deserialize 0x2D27A4. PROVEN |
| +18FDCh | m_owner_id | the local place, 14h none | snap_inputs 0x2D2E16. PROVEN |
| +18FDDh | m_is_server | 0 on a client (0x2CBBC1) | PROVEN |
| +18FDEh | | `-reject_inputs` off | match.md 5.1. PROVEN |
| +18FDFh | m_is_first_time | **frozen until the first world**: move returns at once | move 0x2D2E4A, cleared 0x2D0FE7. PROVEN |
| +18FE0h | m_in_deserialize_now | set for a load (0x2D2785..0x2D2D5D): world events are not queued | PROVEN |
| +18FE4h | | the time rule's state | timelimit_rule_core 0x31816B. PROVEN |
| +18FE8h | | the world's game_effect_player | deserialize 0x2D2C4D. PROVEN |

**A history item** (`game_state_history_item`, C0C00h-odd bytes): the
serialized world, then `+C0BA8h` prev (older), `+C0BACh` next (newer),
`+C0BB0h` time, `+C0BB4h`/`+C0BB8h` the cached hash and its flag,
`+C0BB9h` "loaded from a snapshot" (no 0xDF for it). PROVEN.

### 1.2 The functions of the loop

| VA | Name | ABI | What it does |
|---|---|---|---|
| 0x2D2E40 | `move` | EAX target ms, stack+4 core; `ret 4` | **The one entry.** See 1.3 |
| 0x2D21E0 | `tick_events` | EAX the item the world is at, stack core, first node, target; `ret 0Ch` | The step loop, 1.3 |
| 0x2D1E20 | `atomic_tick` | EDI the time to, stack core, dt; `ret 8` | One step: the seven delegate lists (0x2D1D20, sorted by id, slots before rules), then `+18FC0h = EDI`, `+18FC4h = max(+18FC4h, EDI)`. Also passes "on the grid" (`EDI % 50 == 0`) |
| 0x2D1D20 | `tick_delegates` | stack list, dt, time, on-grid; `ret 10h` | Sorts the list's delegates and calls each |
| 0x2D2180 | `process_events` | stack core, node; `ret 8` | For each place with bit 0 `on_player_entered_match`, bit 1 `on_player_left_match` (input bits are read by the players' own ticks from the input lists) |
| 0x2D2570 | `serialize(item&)` | stack core, item; `ret 8` | The world into an item; `item time = +18FC0h` |
| 0x2D2770 | `deserialize(item const&)` | stack core, item, a dword; `ret 0Ch` | The world from an item: the active mask, every player, the bullet manager (drops every tracer and adds them again), the rules, the objects, the physics, the effect players (every effect made again), `+18FC0h = item time`. Sets and clears +18FE0h |
| 0x2D09C0 | `new_game_state_history_item` | EAX core | An item from the pool; when the pool is full the oldest (head) is unlinked and reused (0x2D3F90 -> 0x2D4C30) |
| 0x2D04F0 | `serialize()` | ESI core | The first item: new item, serialize, link at the tail |
| 0x2D3C20 | (history find) | EAX the ring, ECX time | The newest item with time <= ECX, else 0. PROVEN (new label) |
| 0x27E870 | (first node after) | EAX core, ECX time | The first change node with time > ECX, else 0. PROVEN (new label) |
| 0x2D3700 | (drop world events after) | EDX core, stack time; `ret 4` | Every world event with time > the given time back to its pool. PROVEN (new label) |
| 0x2D2E10 | `snap_inputs` | ESI core | When there is a local place: 0x2D4E40 / 0x2D4F80 put the oldest unsent off-grid own input of each kind on the grid (through erase_impl and insert_impl) |
| 0x2D1100 | `insert_impl` | EAX time, stack core, place, kind, aware, server-aware; `ret 14h` | **Refuses** a time with no item older than it ("too old", log line 0x257; returns 0). Else the node at that time (new, 0x2D3D70, linked by 0x2D4640, or the one there), its bits, and **`+18FC8h = min(+18FC8h, grid point strictly before the time)`** |
| 0x2D1340 | `erase_impl` | EAX time, ECX kind, stack core, place, flag; `ret 0Ch` | Clears the bits of the first node at or after the time; `+18FC8h = min(.., newest item older than the node)`; frees an empty node |
| 0x2D1070 | `has_event_horizon_item` | EAX core | An item at or before max - horizon exists and was not loaded from a snapshot |
| 0x2D0B30 | `event_horizon_history_item` | EAX core | That item (the one 0xDF hashes and 0xF6 uploads) |
| 0x2D10D0 | (event horizon hash) | ESI out, stack core; `ret 4` | {time, CRC} of that item |
| 0x325150 | `game_state_history_item::hash` | EAX item, stack masks | CRC-32 of the serialized item without its time |
| 0x2D0550 | (`clear`) | | Every history item freed (time >= arg), world events freed, `+18FC0h = +18FC4h = 50`, `+18FC8h = -1`, `+18FD4h = 0`; with flag 1 the unsent off-grid own input removed. 0xE5 calls it with (0, 1), an 0xE8 load with (0, 0) |
| 0x2D0B70 | `deserialize(buffer_reader&)` | | The network world: clear (0x2D0550), a fresh item read (0x324D80, `+C0BB9h = 1`), loaded with 0x2D2770, `+18FC4h` and `+18FD4h` = its time; the first one logs "THE VERY FIRST SYNCHRONIZATION" and clears +18FDFh (0x2D0FE7), later ones "FULL RESYNC" |

**insert_impl's refusal and discard's.** An input whose time has no item
older than it is refused by the history itself (insert_impl, "too old");
`discard_input` 0x27D820 and `move_input` 0x27D2D0 refuse the same way with
"cannot find game state". Both are the ring's depth: 11 items, 500 ms
behind the newest.

### 1.3 `move` and `tick_events`, as the code runs them

Both read and RUN (Unicorn, the original bytes, `check_sync_core_026g.py`):

```
move(core, T):                                   0x2D2E40
  if frozen (+18FDF): return
  if cur == T and valid >= T: return             nothing to do
  snap_inputs(core)                              may lower valid
  if cur <= valid or valid >= T:                 -- forward (no rollback due)
      item = newest item with time <= T          0x2D3367 (the ring's tail, usually)
      if none: item = head (log), or return if the ring is empty
      if item.time != cur: deserialize(item)     <-- a world load, EVERY render frame
      if item.time < T: tick_events(item, first node after item.time, T)
  else:                                          -- rollback: an input landed before cur
      item = newest item with time <= valid      0x2D3C20
      if none and the ring is empty: log, return
      if none: log "cannot find", valid = head.time, item = head
      deserialize(item)
      drop world events after item.time          0x2D3700
      tick_events(item, first node after item.time, T)
  (debug hash log only with -debug_hash_mismatch)

tick_events(item, node, T):                      0x2D21E0
  loop:
    if item.next == none: valid = -1             0x2D225A, checked EVERY step:
                                                 reaching the newest item ends the rollback
    grid = next 50 ms point after cur
    to   = min(grid, node.time, T)
    if node and node.time == to:
        process_events(node); node = node.next   (before the step that ends at its time)
    if to != cur: atomic_tick(to - cur)          EDI = to
    if to == grid:
        item = item.next, or a new item linked at the tail (oldest reused when full)
        serialize(item)                          the grid point saved
  until to == T
```

Two things follow that the earlier notes did not have:
- **Every render frame loads a world.** game_world::tick moves to the game
  clock, which is off the 50 ms grid. The world is then at an off-grid
  time, and the next frame's move finds the newest item (a grid point)
  `!= cur` and loads it, then steps from it again. The partial step is
  for the eyes only; the grid items are only ever stepped from a grid item.
  So one deserialize a frame is the stock design, not a rollback (RUN: 59
  of 60 frames at 17 ms load once; the one that ended on a grid point does
  not).
- **The rollback point is reset only when the step loop reaches the newest
  item** (0x2D225A is inside the loop, re-checked every step). A rollback
  steps through the old items, overwriting them in place, reaches the tail,
  and from there `valid = -1` (RUN).

### 1.4 Who calls `move`

| Site | Caller | Target |
|---|---|---|
| 0x1F7C37 | `game_world::tick` 0x1F77E0 | **the render move**: the game clock (`[ebp+0Ch]`), every logic tick, when the match core has a history (`+31B4h`) and a world time (`+191B4h != -1`) |
| 0x1F76F5 | 0x1F76B0 (game_world::tick, the interpolated view 0x1F7A9F / 0x1F7ADF) | the grid points either side of (clock - 200 ms), for a spectator or replay camera |
| 0x27D08D | `network_client::on_idle` 0x27D010 | the event horizon, on the grid: `start + ((now - 500 - start)/50)*50`, only when `valid + 500 < now` (an input older than the horizon is pending) or `max + 500 < now` (the world has not been moved for 500 ms) |
| 0x1EDF6A | 0x1EDEF0 (the replay player) | a replay's time |

`on_idle` then runs `try_send_all` 0x27CFB0: own inputs (a node with
exactly an item's time is sent, 0x27E6D0), 0xDF of the event-horizon item
(0x27CD7A), 0xE1, 0xDB.

### 1.5 What the network layer calls

`network_client::on_match_packet_received` 0x27A300, `jmp [eax*4+27A738h]`
for 0xE3..0xF6 (PROVEN, the table read): the connection state is
`network_client +1F1C64h` (0 connecting, 1 ports known, 2 static info in).

| Id | Taken in | Handler | Into the core |
|---|---|---|---|
| 0xE3 ports | state 0 | 0x27B830, state := 1 | - |
| 0xE4 static info | state 1 | process_static_match_info 0x27B880, state := 2 | the level loads; 0xDA when done (on_match_ready 0x27AA20) |
| 0xE5 set time | state 2 | process_set_time_request 0x27B570 | newer generation only: clock set, 0xDB, clear 0x2D0550(0,1), +191B8h (the match core's last set time), send timers zeroed |
| 0xE6 ping | state 2 | 0x27B770 | 0xDB on the next pass |
| 0xE7 clock offset | state 2 | 0x27B7F0 | floating_timer::set_offset 0x1EE9E0 |
| 0xE8 world | state 2 | process_dynamic_match_info 0x27BFF0 | equal or newer generation: stores it, `deserialize(reader)` 0x2D0B70 (mode 1: the input lists survive) |
| 0xE9 entered | state 2 | 0x27C5F0 | enter_match 0x2D1470 -> insert_impl (kind 0) |
| 0xEA left | state 2 | 0x27C690 | leave_match 0x2D18A0 -> insert_impl (kind 1); own place: close the match |
| 0xEB / 0xEC input relay | state 2 | process_player_input_change 0x27C280 | change_input 0x1FAB50 -> insert_impl |
| 0xED / 0xEE inputs of a tick | state 2 | 0x27C340 | change_input 0x1FAB50 per place |
| 0xEF / 0xF0 move own input | state 2 | 0x27C7B0 | move_input 0x27D2D0 -> erase_impl, change_input |
| 0xF1 / 0xF2 discard own input | state 2 | 0x27C8D0 | discard_input 0x27D820 -> erase_impl |
| 0xF3 kit relay | state 2 | 0x27C980 | change_input<profile> 0x1FB730 |
| 0xF4 / 0xF5 move / discard kit | state 2 | 0x27CA30 / 0x27CB50 | move_input<profile> 0x27DCD0 / discard_input<profile> 0x27E200 |
| 0xF6 | state 2 | stock: debug on_hash_mismatch; patched (patch 4): the upload of the event-horizon item | - |

Local inputs: `game_world` 0x1EEDE0 -> change_input (from network 0); kit
choice: `select_profile` 0x1F8070 -> change_input<profile>.

### 1.6 What our patches already hook in this region

| Patch | Site | Kind | What |
|---|---|---|---|
| 3 | 0x27CC53 | byte | hash interval 100 -> 50 ms |
| 4 (snapshot upload) | 0x27AEA1, 0x27A505 | JUMP | the first world before 0xDA (the initial item `[core+2FC0h]`); the event-horizon item on 0xF6 |
| 11 (late join) | 0x2D3880 bind_slot (CALLEE), 0x2D3906 history_capacity (JUMP: twenty input histories), 0x27A23D connect_capability, 0x27A378 prepare_hook (the 0xE4 case), 0x27B517 disconnect_hook, 0x219E60 client_destroyed_hook, 0x1F2013, 0x2CBF9B, the counters in `late_join_runtime_026g.c` (`bind_slot_reserved`) | | the roster grows during a match |
| 13 | 0x279610, 0x27A2E0, 0x3237B4 | bytes | twenty places in the mask bytes; the scoreboard |
| 15 | 0xA799C8 (the value), 0x1E9848 | data | the clock takes a correction 6x faster |
| 16 | the scoreboard counters (0x3239F0 / 0x323A00 callers) | | a newcomer's score not counted again at every rollback |
| 19 | 0x2D1050, 0x2D1055 (CALLEE in deserialize 0x2D0B70, frozen block) | | a first world's inputs and kit choices in time order |
| 26 | 0x1EE3A3 | JUMP | the replay's seek |
| 38 | 0x2D8A0F | JUMP | spotting (in the simulation) |
| 62 | (none here) | | F11 hides the UI; listed by the ask, it touches nothing of this region |
| 71 (#446, not on main) | entries of 0x2D2E40 move, 0x2D1E20 atomic_tick, 0x2D2770 deserialize, 0x322830 emit_bullet, ... | ENTRY (first bytes) | the freeze detector's counters; **a new hook must not take these entries** |
| 72 | 0x7E6E6, 0x7E720 (step_down) | in place | fall gravity per step dt |

So: **nothing of ours is inside move or tick_events today**, and patch
71's entry hooks sit on move, atomic_tick and deserialize. A new layer
hooks the *callers* of move (CALLEE sites), not its entry.

---------------------------------------------------------------------------

## 2. What a frame does today, and what the freeze detector counts

RUN on the stock bytes with a model world (`check_sync_core_026g.py`:
atomic_tick, serialize, deserialize and process_events are stand-ins that
keep a hash chain; move, tick_events, insert_impl, the ring and the node
list are the game's):

```
frame  T     loads   steps                      note
 1    84      [50]   50->84                     render: load the grid item, step to the clock
 2   101      [50]   50->100 (save) 100->101    a grid point crossed: one full step, saved
 3   118     [100]   100->118
30   577     [300]   300->347 ev 347->350 ... 550->577   an input at 347 came 230 ms late:
                                                 6 of the 7 steps are re-simulations
31   594     [550]   550->594                   back to one load, one step
```

- **"rollbacks 1" on almost every frame is the render load**, not a
  network rollback (patch 71 counts every 0x2D2770 call). The rollbacks of
  the owner's complaint are the **resimulated** ticks: "ticks 6
  (resimulated 4-5)" is one load and 4-5 steps of an input that came
  200-250 ms late, in a 40-70 ms frame.
- **"rollbacks 45, ticks 87 (resimulated 84)"**: the logic thread ran
  game_world::tick many times inside one drawn frame after a stall
  (patch 71 counts per drawn frame), each moving the world and each
  rolling back to the input that the network pass before it brought.
- Each re-simulated step is a whole atomic_tick (2.5-4 ms live); each load
  rebuilds the effect players and every tracer (PR #441's rows for
  0x2DDDF0 and 0x321C50).

---------------------------------------------------------------------------

## 3. The state machines

### 3.1 The rules (the owner's)

- **One enum per machine**, a sum type whose variants carry exactly the
  data valid in that state. No free-standing flags or Options that mean a
  state.
- **One transition function** per machine: `(state, event) -> (state,
  actions)`, total over events. An event a state does not take is logged
  and ignored, or rejected, explicitly, never half-applied.
- Short-lived states are sub-machines of the state they live in (a
  rollback is a sub-state of Playing; a divergence holds its evidence).
- Machines that are truly independent (the snapshot upload of a client and
  its sync) are separate machines, not flags of one another.
- The same rule for the client layer, in the language it is written in.

### 3.2 The client: what the exe keeps today

The client's sync state is spread over these fields; together they make
the states below.

| Field | Values | Set / cleared |
|---|---|---|
| `network_client +1F1C64h` | 0 connecting, 1 ports known, 2 static info in | 0xE3, 0xE4 handlers |
| the match core | none until the level loaded | on_match_ready 0x27AA20, 0xDA |
| `network_client +1F1C68h` generation | 0xFF at 0xDA; the 0xE8's on a load | 0x27AE9A, 0x27C006..0x27C040 |
| core +18FDFh frozen | 1 from the ctor to the first load | 0x2D0130, 0x2D0FE7 |
| core +18FD4h last resync | 0 after 0xE5, the world's time after 0xE8 | 0x2D0550, 0x2D0FC5 |
| the ring | empty after 0xE5 until the 0xE8 load | 0x2D0550 |
| core +18FC8h rollback point | -1 or a grid time | 1.2 |
| core +18FC4h newest time ticked | | atomic_tick, a load |
| match core +191B8h | the last 0xE5 time | 0x27B737 |
| `+1F1C60h` the time rule's state | 1 waiting, 2 countdown, 3 play | client_timelimit_rule::tick |

Combinations that mean nothing, and can happen:
- **frozen and a non-empty ring after a first 0xE5 whose 0xE8 never came**:
  the world is frozen, so move does nothing, but inputs and roster changes
  are taken onto an empty history with last resync 0 (match.md 18). Only
  a newer 0xE5 + 0xE8 gets out of it.
- **not frozen, the ring empty, last resync 0**: a game resynced while it
  plays, between its 0xE5 and its 0xE8. move goes on "through its
  empty-history branches" (0x2D2EE8, 0x2D3388: it logs and returns), so the
  world stands still while the clock runs; the ring is empty, so
  `has_event_horizon_item` is false and no 0xDF goes; own inputs are made
  on the new clock. An 0xF1/0xEF in that window finds no item and is
  refused ("cannot find game state"): the input stays in that game alone
  (server: C4 below).
- **a rollback point older than the ring**: "cannot find" (log), the head
  is loaded instead, and the inputs between are played wrong for good.

### 3.3 The client machine (proposed, one enum)

```
                 0xE3                 0xE4                      level loaded
 Connecting ---------> PortsKnown ---------> Loading{place} ------------------+
                                                                              | send 0xDA
                                                                              v
                                   +------------------------------- Ready{gen=FF}
                                   |   0xE5(g newer)                 (frozen, no clock)
                                   v
                       Timed{gen g, clock, first=true}
                       (frozen; inputs queue on an
                        empty ring; 0xE5(g'>g) restarts)
                                   |
                                   | 0xE8(g' >= g) loaded: "THE VERY FIRST SYNCHRONIZATION"
                                   v
 +---------------------> Playing{gen, loaded_from S, step: Step} <-----------------+
 |                         |    |                                                   |
 |    0xE8(g' >= gen)      |    | 0xE5(g' newer)                                    |
 |    (a world alone,      |    v                                                   |
 |     no 0xE5: the        |  Resyncing{gen g', clock, since}                       |
 |     world-only resync)  |  (the world runs on, the ring empty,                   |
 +-------------------------+   no 0xDF; 0xF1/0xEF refused)                          |
                           |    | 0xE8(g' >= g') loaded: "FULL RESYNC"              |
                           |    +---------------------------------------------------+
                           |
                           | 0xEA(own place) / link lost
                           v
                        Ended{why}
```

`Step`, the rollback sub-machine of Playing (the state of +18FC8h and the
history, made explicit):

```
            insert/erase lands before the world's time
  Current ----------------------------------------------> Behind{to}
     ^                                                      |
     |  move: load the item at <= to, step to T;            |
     |  the step loop reaches the newest item              |
     +------------------------------------------------------+
     ^                                                      |
     |                     stage (a): a render move within |
     |                     50 ms of the last rollback and  |
     |                     before to + 300 ms              v
     +-------- forced (T >= to + 300, or a step  --- Postponed{to, last}
               past 50 ms of game time)            (the world steps on from the
                                                    newest item; `to` is kept)
```

| Variant | Carries | Captures in it | Timers |
|---|---|---|---|
| Connecting | the server's address | none | the transport's |
| PortsKnown | the ports | none | |
| Loading | the place, the options | none (the level loads) | |
| Ready | generation 0xFF | the initial item `[core+2FC0h]` (patch 4 uploads it before 0xDA) | |
| Timed{first} | generation, clock | none (the ring was cleared) | the 0xE5 clock; the server's 1.75 s / entry_state |
| Playing | generation, S = last resync, Step | the ring: 11 items on the grid, 500 ms; the event-horizon item (hashed by 0xDF every 50 ms, uploaded on 0xF6); the masks list | event horizon 500 ms; 0xDF every 50 ms; 0xE1 every 1 s |
| Resyncing | generation, clock, since | none (the ring is empty) | |
| Ended | why | | |

Packets per state (client side; "--" not taken):

| Packet | Connecting | PortsKnown | Loading | Ready | Timed | Playing | Resyncing | Ended |
|---|---|---|---|---|---|---|---|---|
| 0xE3 | -> PortsKnown | -- | -- | -- | -- | -- | -- | -- |
| 0xE4 | -- | -> Loading | -- | -- | -- | -- | -- | -- |
| 0xE5 newer | -- | -- | crash (no core) | -> Timed | -> Timed (restart) | -> Resyncing | -> Resyncing (restart) | -- |
| 0xE6 / 0xE7 | -- | -- | taken | taken | taken | taken | taken | -- |
| 0xE8 >= gen | -- | -- | -- | taken onto the old clock (LIKELY; the server never does it) | -> Playing | -> Playing (world-only resync) | -> Playing | -- |
| 0xE9 / 0xEA | -- | -- | -- | queued (no history: all taken) | queued | insert; refused at or before S, or 500 ms behind (entry) | queued, all taken | -- |
| 0xEB..0xEE, 0xF3 | -- | -- | -- | queued | queued | insert; refused at or before S; moved if 500 ms behind | queued | -- |
| 0xEF/F0, 0xF1/F2, 0xF4/F5 | -- | -- | -- | refused | refused | taken while an item older than the time is in the ring, else "cannot find game state" | refused ("cannot find") | -- |
| 0xF6 (patched) | -- | -- | -- | -- | -- | upload the event-horizon item if it is not from a snapshot and 500 ms old | -- | -- |

Sent: 0xD8 (Connecting), 0xD9 (PortsKnown), 0xDA (Loading -> Ready), 0xDB
(answers 0xE5 at once, 0xE6 on the next pass), 0xDC/0xDD/0xDE own inputs
(Timed, Playing, Resyncing: every 50 ms while there are changes), 0xDF
(Playing, when the event-horizon item exists), 0xE0 chunks (patch 4: Ready,
Playing), 0xE0 empty (leave), 0xE1 (every 1 s).

The upload is its own machine (patch 4): `Idle | Sending{item_time,
chunks, acked}`; it runs in Ready and Playing and changes nothing of the
sync.

### 3.4 The server's view of a client: today

`Stage` (game.rs 410-437, main): `Connecting`, `Loading`,
`WaitingSnapshot{since, entering}`, `Timing{time, entering, last, sent}`,
`Syncing{serial, time, entering}`, `Live`, `Gone`. Beside it, about forty
fields of `Peer` (game.rs 595-802) that are states in all but name. The
inventory (main unless #448; G = game.rs, LJ = late_join.rs, ST = start.rs):

| Field | Meaning | Becomes |
|---|---|---|
| `stage` | the phase | the enum's tag |
| `generation` | the sync generation | in `Timing`, `Syncing`, `Live` |
| `set_times`, `time_confirmed`, `time_answered`, `clock_set`, `clock_given` | 0xE5 copies and whether one was answered | `Timing{copies, answered: Option<Answer>}`; `Syncing.clock: Clock::{Answered, Assumed}` |
| `early_entries`, `late_roster_redos` | entries ahead of a first load | `Syncing{kind: First{early_entries, roster_redos}}` |
| `resync_from` | a resync runs (set at its first 0xE5, cleared only by finish_sync) | `kind: Resync{since, ...}` of WaitingSnapshot/Timing/Syncing |
| `replayed`, `sync_pending`, `sync_relays`, `gate_since`, `gate_checked`, `replay_redos` | the replay and the world's gate | `Resync{replay: Replay::{NotSent, Sent{at, for}, Gated{...}}}` |
| `resync_generations`, `world_waits`, `world_wait_since`, `slow_loads`, `answer_redos` | redo counters | one `ResyncBudget{generations, waits, slow_loads, answer_redos}` in `Resync`, checked by every redo (C1) |
| `held_answers`, `sent_answers` | answers held during a resync | `Resync{held_answers}`; `Live{answers_in_flight}` |
| `resync_floor` | the least snapshot time | `Resync{floor}`, made from the `Live` evidence that caused it (C8) |
| `entered_early` | a player back in | `kind: First{entry: Back{at}}` |
| `held`, `held_until` | first-world relays held | `kind: First{held}` |
| `reload_due`, `reloads` | a TDM reload | `Live{reload: Option<Due>}` -> a transition to WaitingSnapshot{Reload} |
| `wrong_world_asked`, `wrong_worlds` | a diagnostic upload | `Live{agreement: Diverged{asked_world: Some(..)}}` with a deadline (C5) |
| `asked`, `asked_clock`, `ask_at`, `unanswered_since`, `upload_offset` | upload requests | the upload machine of the place (per peer, beside the sync) |
| `diverged` (`Divergence{first,last,since}`), `judge_from`, `restores_unagreed`, `resync_deferred_since` | the hash verdicts | `Live{agreement: Agreement}` (below) |
| `lags`, `lag_ms`, `correction`, `round_trip(s)`, `correcting_until`, `correction_ends`, `clock_offset_pending` | the clock | `Clock::{Steady, Correcting{offset, until, acked}}` inside Live |
| `missed`, `discarded`, `in_flight` | transport verdicts | `Live{misses}` (tie evidence) |
| `load_reports`, `sync_sent_at`, `synced_at` | load proof | `Syncing{world: WorldInFlight{sent_at, reports}}` |
| `farewell` | the end resent | `Gone{farewell: Some(..)}` |
| `patched`, `resources_loaded`, `installed_roster`, `preparing`, `loading_since`, `left_behind` | capability and late join | capability stays a field (not a state); the rest `Loading{..}` and the admission machine |
| `kept_own` (#448) | an own input the game kept alone | `Live{agreement: Doomed{OwnInputKept{at}}}`: a resync due now (C2) |
| `loaded_from` (#448) | the last start world's time | `Live{loaded_from}`, set when the load is proven, not when sent (C6) |

Contradictory or unclear combinations today (from the code; "live" where a
log shows it):
- **C1, uncapped generations** (main): a Syncing world that is lost
  (`is_lost`) goes to `redo_sync` and a new generation with no cap;
  `RESYNC_GENERATIONS_MAX` (8) only stops replay redos; `resync_from`,
  `replay_redos`, `resync_generations`, `slow_loads` reset only in
  finish_sync; after 20 s every pass is "given up" and starts from a world
  up to 5 s old, past the 27-input ring. **Live:** place 1 of 0x27e8fbc0,
  15:20:03 to 15:21:05 UTC, 27 generations in 60 s ("no fresh snapshot ...
  in 20 s", match.md 35 on #448). About 2.2 s a generation fits it.
- **C2** (#448): `kept_own` waits for the resync interval (5/15 s) and the
  correction defer (20 s); a healed divergence clears `diverged` but not
  `kept_own`.
- **C3**: `keeps_own` ignores a peer that is not Live; drop orders sent
  before a resync's first 0xE5 set no floor.
- **C4**: a player back in (Syncing, `entered_early`) has its inputs taken
  but its answers not held; a 0xF1/0xEF reaches a game with no history and
  is refused ("cannot find game state"). Live 2026-10-15: 9 drops at the
  edge of its history (match.md 30).
- **C5**: `wrong_world_asked` has no expiry.
- **C6** (#448): `loaded_from` is set when the world is sent.
- **C7**: kit answers 0xF4/0xF5 are untracked with a 450 ms life, though
  the client's table has them reliable.
- **C8**: `resync_floor` only rises.
- **C9**: a joiner's second start runs the resync path on a peer whose
  inputs are still ignored.
- **C10**: a failed or rejected admission leaves `waiting_entry` true for
  good: a reconnecting game of that place is a ghost.
- **C11**: `time_confirmed` without `time_answered` (5 unanswered 0xE5).
- **C12**: a stand-in's leave within 300 ms of an entry may be dropped.
- **C13** (#448): the kit re-relay reads the stand-in events too.

### 3.5 The server's view of a client: proposed

```rust
enum ClientSync {
    Connecting { since: Instant },
    Loading { since: Instant },                       // 0xE4 sent, waits 0xDA
    AwaitingWorld { why: Start, since: Instant },     // WaitingSnapshot
    Timing { why: Start, generation: Generation, clock: u32,
             copies: Copies /* <= MAX_SET_TIMES */, answered: Option<Answer> },
    Syncing { why: Start, generation: Generation, world: WorldInFlight /* S, serial,
              sent_at, reports, lifetime */, clock: Clock },
    Live { generation: Generation, loaded_from: u32, agreement: Agreement,
           clock: ClockCorrection, answers: AnswersInFlight, reload: Option<ReloadDue> },
    Gone { why: Why, farewell: Option<Farewell> },
}
enum Start {
    First { entry: Entry /* New | Back{at} | Joiner{admission} */, held: HeldRelays,
            early_entries: Vec<u32>, roster_redos: u8 },
    Resync { since: Instant, floor: u32, cause: Cause, replay: Replay,
             held_answers: Vec<Answer>, budget: ResyncBudget },
    Reload { since: Instant },
}
enum Agreement {
    Agrees,
    Unjudged { until: u32 },                          // installing, just loaded
    Diverged { first: u32, last: u32, since: Instant, reports: Reports,
               asked_world: Option<Deadline> },
    Doomed { cause: Cause /* OwnInputKept{at} | RelayPastHistory{at} | Loaded wrong */ },
}
```

Transitions (trigger -> guard -> actions -> state). Current behaviour
unless marked **proposed**:

| From | Trigger | Guard | Actions | To |
|---|---|---|---|---|
| (none) | 0xD8 | place free or the same account | 0xE3 | Connecting |
| Connecting | 0xD9 | | 0xE4 | Loading |
| Loading | 0xDA | | generation := FF; a player back: its entry to the others first | AwaitingWorld{First} |
| AwaitingWorld | a usable snapshot | fresh (700 ms, 1500 after 3 s, 5 s after 20 s), on the grid, past the floor and the place's last roster change | generation+1, 0xE5 | Timing |
| Timing | 0xDB of this generation | | clock answered | Syncing (0xE5 again, the replay, 0xE8) |
| Timing | 5 copies unanswered | | clock assumed | Syncing |
| Syncing{First} | the world acked or proven by a hash | join ready; no roster change past its horizon | relays flushed, entry at grid_up(clock+300) | Live |
| Syncing{Resync} | the world acked or proven | | held answers sent if within 450 ms, else floor and redo | Live / AwaitingWorld |
| Syncing | the world lost (1.75 s / entry_state) | **proposed:** `budget.generations < 8` | redo | AwaitingWorld (same `Start`) |
| Syncing | budget spent | **proposed** | disconnect (0xEA own) with the reason logged | Gone |
| Live | a hash report | tick > loaded_from | Agreement updated | Live |
| Live | Diverged lasts 1 s (match) or 3 s (wall) | the resync interval; no correction running | ask its world | AwaitingWorld{Resync{floor}} |
| Live | Diverged, then its newest report agrees with a strict majority | #448 | "agrees again; not resynced" | Live{Agrees} |
| Live | an own input kept alone (drop order lost or past its history) | #448 sets `kept_own`; **proposed** Doomed at once | resync now, floor = the input's time | AwaitingWorld{Resync} |
| any | 0xE0 empty / timeout / replaced | | 0xEA to the others | Gone |
| any | the match ends | | own 0xEA, resent until acked | Gone{farewell} |

Packets per state (server side):

| Packet in | Connecting | Loading | AwaitingWorld | Timing | Syncing | Live | Gone |
|---|---|---|---|---|---|---|---|
| 0xD9 | -> Loading | ignored | ignored | ignored | ignored | ignored | ignored |
| 0xDA | ignored | -> AwaitingWorld | ignored | ignored | ignored | ignored | ignored |
| 0xDB | | | ping | answers 0xE5 | ping | ping, clock | |
| 0xDC/DD/DE | | | Resync: relayed | Resync: relayed | Resync or Back: relayed | placed: relay / move / drop | |
| 0xDF | | | ignored | ignored | load proof | judged | |
| 0xE0 chunk | | upload (initial) | upload | upload | upload | upload | |
| 0xE0 empty | Gone | Gone | Gone | Gone | Gone | Gone | |
| 0xE1 | alive | alive | alive | alive | alive | alive | |

| Packet out | Sent in |
|---|---|
| 0xE3 | -> Connecting |
| 0xE4 | Connecting (on 0xD9); growing roster: Prepare |
| 0xE5 | Timing (every 150 ms, 5 at most), Syncing (once more before 0xE8) |
| 0xE6 | Live, every 1 s |
| 0xE7 | Live (correction), after a resync's load |
| 0xE8 | Syncing |
| 0xE9 / 0xEA | Live; Resync with the replay sent (kept); First with the 0xE5 answered (entries) |
| 0xEB / 0xEC / 0xF3 | Live; First (held, flushed every 50 ms); Resync (kept, gated) |
| 0xEF/F0, 0xF1/F2, 0xF4/F5 | Live (450 ms); held in Resync |
| 0xF6 | Live (asked), before a resync (its wrong world) |

### 3.6 A place (proposed, one enum)

Today a place's state is derived from `Game.events`, `stand_ins`,
`given_up`, `admissions[i]`, `load_waits[i]`, `kit_changes[i]` and #448's
`kit_choices[i]`.

```
            roster: match start / late-join admission entered
  Empty -------------------------------------------------> InPlay{entered_at, kit}
    |                                                        |      ^
    | no game by the start                                   | 0xEA |  0xE9 (back), the kit
    v                                                        v      |  re-relayed for the entry
  StandIn{entered_at, left_at: Option} --- its player ---> Away{left_at, last_kit}
                                           comes                 |
                                                                 | 0xE0 empty (gave up)
                                                                 v
                                                              GivenUp
  Entering{admission}: Preparing -> CatchingUp{after} -> Entered  (-> InPlay)
                       any -> Failed -> Rejected (-> Empty, the ghost of C10 ends)
```

With reusable places (section 10) GivenUp is not the end: it becomes
`Vacant` once no game can roll back across the leave, and a Vacant place
takes a new occupant through the same admission (`Vacant -> Entering ->
InPlay`), the occupant counter carried by every variant.

`Away{last_kit}` carries the kit the place last chose: a snapshot holds a
place's next kit (+0Dh) only while it is in play (restore 0x2D2A24..0x2D2A37),
so a game that started while the place was away has the lowest kit there.
The entry back re-relays `last_kit` for the entry tick (#448's fix, made a
field of the state instead of a separate vector).

### 3.7 Per-message coverage

Client -> server:

| Id | States that take it (server) |
|---|---|
| 0xD8 | (attach) |
| 0xD9 | Connecting |
| 0xDA | Loading |
| 0xDB | Timing (answer), every other state (ping) |
| 0xDC / 0xDD / 0xDE | Live; AwaitingWorld/Timing/Syncing with Start::Resync; Syncing with Start::First{Back} |
| 0xDF | Syncing (load proof), Live (judged) |
| 0xE0 | every state but Gone |
| 0xE1 | every state but Gone |

Server -> client:

| Id | States that take it (client) |
|---|---|
| 0xE3 | Connecting |
| 0xE4 | PortsKnown |
| 0xE5 | Ready, Timed, Playing, Resyncing (newer generation) |
| 0xE6, 0xE7 | Loading.. Resyncing |
| 0xE8 | Timed, Playing, Resyncing (equal or newer generation) |
| 0xE9, 0xEA | Ready, Timed, Playing, Resyncing (0xEA own: Ended) |
| 0xEB..0xEE, 0xF3 | Ready, Timed, Playing, Resyncing |
| 0xEF..0xF2, 0xF4, 0xF5 | Playing (refused elsewhere) |
| 0xF6 | Playing (patched) |

---------------------------------------------------------------------------

## 4. Error paths as transitions

| Path | Trigger -> what happens | Today | Proposed |
|---|---|---|---|
| A discard past the history | client Playing: 0xF1 for T, no item older than T -> "cannot find game state", the input stays (stock). Server: the drop order counts as delivered | stock; main counts a missed tick only | #448: `kept_own` -> resync from a world past T; **proposed:** Live{Doomed} at once, no interval; stage (d) makes the ring 1000 ms so the order is taken |
| A resync loop | server Syncing: world lost -> redo, new generation, no cap; after 20 s every world is up to 5 s old (past the 27-input ring) | main (C1); live 27 generations in 60 s | `ResyncBudget` checked by every redo; past it: disconnect, or a reload from a fresh upload of the majority; never a world older than the input ring |
| A joiner's transient disagreement | server Live: a late joiner disagrees 50-650 ms, then agrees | main resyncs after 1 s; #448 `agrees_again` | Agreement::Diverged carries the reports; a strict-majority agreement after it closes it (as #448), and the client side is looked for (state a snapshot does not carry) |
| A returning player's kit | place Away -> InPlay; a game that loaded while the place was empty spawns it in kit 0 | main: wrong for those games | #448 re-relays the last kit; proposed: `Away{last_kit}` |
| A hitch -> late inputs -> drop | client stalls; its inputs reach the server late; L >= ~410 ms -> 0xF1, which comes past its history -> refused -> its world alone keeps the input | main; live 2026-10-15 58 of 215 desyncs | #448 resync; proposed: stage (d) (1000 ms ring) makes the order take; the server's reach (`ANSWER_REACH_MS`, `CLIENT_HISTORY_MS`) follows the ring |
| The bake stall -> resync | client: a camouflage/decal bake blocks the logic thread (patch 70's subject); every relay of that time comes late, every own input goes late; patch 71 then shows "rollbacks 45" | stock | stage (a)'s coalescing: one rollback per 50 ms of game time during the catch-up; patch 70 removes the stall itself |

---------------------------------------------------------------------------

## 5. Invariants

1. **Determinism.** A grid item is stepped only from a grid item, through
   every change node between them, splitting at each node's time; the same
   inputs and the same split points give the same bytes. Nothing of the new
   layer may change a split point unless the stage says it is a simulation
   change (stage (c)).
2. **No stale item reaches the outside.** An item is hashed (0xDF) and
   uploaded (0xF6) once it is the event-horizon item (500 ms behind the
   newest time ticked). Every item after the rollback point is stale until
   the rollback steps through it; so a rollback must happen before
   `newest - 500 >= point + 50`. Stage (a) forces one at `point + 300`.
3. **The history covers the server's reach.** An input, a drop or a move
   the server can still send must find an item older than its time: the
   ring (500 ms today) >= the server's reach (`late_relay_reach` <= 425 ms,
   `DROP_ORDER_LIFE` 450 ms, `ANSWER_REACH_MS` 450 ms). Deepening the ring
   (stage (d)) is what lets the server reach further; never the other way.
4. **When a world may be captured or loaded.** Captured: at a grid point by
   the step loop, and on 0xF6 the event-horizon item only (never one
   loaded from a snapshot). Loaded: by move (a grid item), by 0xE8 (Timed,
   Playing, Resyncing). A load drops the world events after it and rebuilds
   the effects and tracers.
5. **The tick order.** In a step the delegates run in id order: places'
   slots first, rules last (0x2D1D20, ids handed out slots first); a change
   node at a step's end time is processed before that step. The late
   join's key ranges (patch 11's `bind_slot`) keep newcomers' slots before
   the rules.
6. **What a snapshot must carry.** Everything a later step reads: the
   places in play with their next kit (+0Dh), the rules, the objects, the
   physics, the effects. **Not carried:** an empty place's next kit; a
   battery count no world holds (match.md 28). The server makes up for each
   (#448's kit re-relay).
7. **Generations.** 0xE5 is taken only for a strictly newer generation,
   0xE8 for an equal or newer one; every resync is the next generation.

---------------------------------------------------------------------------

## 6. Rust `no_std` instead of C: evaluation

**Verdict: feasible; the plan is Rust.** Stage (a) is built in Rust.

What was tried (2026-10-25, the flake's own toolchain):
- `nix develop ./srp-026g` has rustc 1.98.0-nightly with `rust-src`. A
  `#![no_std]` staticlib built for `i686-unknown-linux-gnu` with
  `-C relocation-model=static -C target-cpu=pentium4`, `panic=abort`,
  `lto=fat`, one codegen unit, is one ELF32 object of our crate.
- **Not with cargo's `-Zbuild-std`:** that resolves the whole standard
  library's workspace (`library/Cargo.lock`: `proc_macro` needs
  `rustc-literal-escaper` from crates.io, among others), so it builds only
  with the network or a warm cargo cache (it seemed offline the first time
  because the cache here was warm; with an empty `CARGO_HOME` and
  `--offline` it fails, even for `-Zbuild-std=core`). The link therefore
  calls **rustc alone**: `core` from the toolchain's own rust-src
  (`library/core/src/lib.rs`, `-Zforce-unstable-if-unmarked`, about 10 s,
  once a toolchain, kept in `srp-026g/target-arena/sysroot-<key>`), an
  empty `compiler_builtins` (rustc links every `no_std` crate against one;
  the arena takes none of its code), then the crates the arena crate names
  by path as rlibs and the arena crate as a fat-LTO staticlib, all with
  `--sysroot` at that folder. No cargo, no crates.io, no network: checked
  with `CARGO_HOME` pointing nowhere.
- That object has only R_386_32 and R_386_PC32 relocations, its sections are
  `.text.*`, `.rodata.*`, `.bss.*` (which `link_026g.py`'s patterns already
  take) and `.eh_frame` (dropped). An `extern "thiscall"` call to a fixed
  address came out as `mov ecx, p; push arg; call 0x2D2E40`. An enum with
  data came out as plain stores. No GOT, no TLS, no probes for small frames.

How it fits:
- **The linker** takes a third kind of source, a crate (`Cargo.toml`):
  `link_026g.py` builds it with the fixed flags above (paths remapped, no
  debug info), takes the crate's own object out of the archive, and checks
  it like any other (relocations, sections). A symbol it lacks (memcpy, a
  64-bit division) is an undefined symbol, which the link refuses loudly.
  The rustc version goes into the lock's `built_with`; the lock pins the
  bytes, as for gcc.
- **Calling the game:** through the engine library's cdecl thunks, as C
  does. `engine_026g.py` writes a fourth file, `engine_026g.rs`
  (`extern "C" { pub fn engine_...(...); }`), from the same rows: one source
  of truth for C, NASM and Rust. A row's ABI stays checked by
  `check_engine_026g.py`.
- **The shared crate:** `srp-026g/crates/sync-core`, `#![no_std]`, no
  allocation: the enums and the transition functions. The match server
  (std) can use it as an ordinary dependency, and `cargo test -p sync-core`
  runs on the host. The arena crate
  (`client-patches/sync_core_arena`) is the thin client side: the hook's
  Rust entry, the world's fields, the engine calls, the panic handler (a
  `ud2`: a loud crash at a known address, never a wrong world).
- **Testing:** host tests of the transition functions; Unicorn for the arena
  build: the differential check runs the stock path and the Rust path in the
  same emulated game on scripted timelines and compares every grid item
  that reaches the event horizon (what 0xDF hashes and 0xF6 uploads).

Risks:
- Nightly only (`#![feature(compiler_builtins)]` in the stand-in, `core`
  built from source): the workspace already needs nightly.
- **Stack alignment:** the game's frames are aligned to 4 bytes; rustc's
  i686 code assumes 16 at a call (the C patches build with
  `-mincoming-stack-boundary=2` for the same reason). Every entry from the
  game into Rust goes through a NASM stub that aligns the stack
  (`push ebp / mov ebp,esp / and esp,-16 / ... / leave`).
- A compiler update changes the bytes: the lock shows it, as with gcc.
- Panics must not drag in formatting: the crate avoids indexing that can
  panic; the panic handler takes no `Location`.
- SSE: `target-cpu=pentium4` is SSE2, what the C patches assume (`-msse2`).
  The orchestration has no floating point.
- Size: stage (a) is under 200h bytes of code.

---------------------------------------------------------------------------

## 7. The staged plan

Each stage ships alone and gives the stock orchestration's worlds and hashes
for the same inputs, unless it says it is a simulation change. Each stage
names the states and transitions of 3.3 it takes over.

### (a) Rollback pacing (built: section 9)

- **Machine:** Playing's `Step`: a new transition `Behind{to} ->
  Postponed{to, last}` for a render move, and `Postponed -> Current` when
  forced.
- **Rule:** a rollback is due when the world's time is past the rollback
  point and the target is past it too (stock's own test). A **shallow** one
  (the point is the grid point at or before the world's time: the item every
  render move loads anyway) is stock and does not count. For the render
  move only (`game_world::tick`, target past the world's time, world not
  frozen) a deeper one is **postponed** while the last one was less than
  50 ms of game time ago **and** the target is less than 300 ms past the
  rollback point. Postponed means: the world steps on from its newest item,
  the rollback point is kept (hidden from the stock move for that call, then
  restored as the minimum of the two). Everything else is stock.
- **Hook:** CALLEE at 0x1F7C37 (`call 0x2D2E40` in game_world::tick) ->
  `sc_render_move` (EAX target, stack core, `ret 4`, as move). Not move's
  entry: patch 71 takes that.
- **Interface:** `sync_core::pacing::render_move(Pacing, World, GameTime)
  -> (Pacing, Plan::{Stock, Postpone{keep}})`, `World::{Frozen{core},
  Running{core, now, rollback: Option<GameTime>}}` parsed once by the
  arena's glue (section 9); the arena calls `engine_world_core_move`.
- **Unicorn:** `check_sync_core_026g.py`: the built exe, the game's own
  move, tick_events, insert_impl and the ring; a model world (atomic_tick,
  serialize, deserialize, process_events as a hash chain); scripted
  timelines (late inputs, bursts, a stall and catch-up, precise inputs at
  one ms, inputs too old for the ring); the same timeline through the stock
  call and through the hook; every grid item compared when it reaches the
  event horizon, and the final ring after a last move.
- **Gain (the model, re-simulated steps):** the table in section 9. In
  short: -57% at 100 fps, -51% at 60, -25% at 30 with inputs 120-260 ms
  late; -13% with inputs spread to 420 ms (many are past the deadline at
  once); the worst single frame does not change (a forced rollback steps
  all).
- **Risks:** a misprediction is shown up to 50 ms longer. Consumers of
  "first play" (a step past +18FC4h: damage and equipment sounds
  0x23EEC4, 0x23F444, 0x2A6488; the time rule's status 0x267AD2) then act on
  the predicted world a little more often, as they do for every late input
  today. Nothing a match computes changes.

### (b) Cheaper loads

- **Machine:** the load action of `Behind -> Current` and of every render
  frame (2): one world load a frame is the stock design.
- **Idea:** a load from move (a render rewind or a rollback, never 0xE8)
  keeps the effect presenters and the tracers whose serialized state did
  not change, instead of making every effect again (0x2DDDF0 at 0x2D2C54)
  and every tracer again (0x321C50 via 0x322920).
- **First:** measure (patch 71 times the loads too). The effect player is
  in the world (its section is hashed), so only the presenters may be kept,
  never the effects; proving which state is visual-only is the work.
- **Hooks:** CALLEE at 0x2D2C54 and in bullet_manager::deserialize.
- **Unicorn:** a load with and without, the world bytes after it identical;
  the presenters' calls counted.
- **Gain:** every frame at any frame rate, on top of (a). **Risk:** medium
  (a presenter kept for an effect that changed).

### (c) One precise split a tick

- **A simulation change**, a revision of its own, every game of the match
  on it: off-grid inputs inside one 50 ms step split it once (at the
  earliest), not once per input. Coordinate with patch 72 (its gravity
  uses the step's dt).
- **Machine:** the step boundaries of tick_events.
- **First:** what the weapons read of the precise ms (fire timing).

### (d) A deeper ring

- **Machine:** Playing's captures (the ring) and the error path "a discard
  past the history".
- **Change:** 21 items (1000 ms) instead of 11, the event horizon (hashes,
  uploads) staying 500 ms: the pool's count (computed near 0x2CBFDC, where
  `-debug_hash_mismatch` makes it 128; to be read), not
  `max_ping_time_in_ms` (that moves the horizon). About 8 MB more.
- **Then the server:** `CLIENT_HISTORY_MS`, `ANSWER_REACH_MS`,
  `late_relay_reach` up to about 950 ms for games of that revision; drops
  that are refused today are taken.
- **Not a simulation change;** a protocol reach change (the server must
  know the revision). **Risk:** deeper rollbacks cost more when they happen.

### (e) The whole orchestration ours

- **Machine:** Playing whole (`Step`, the ring, the node list), then Timed
  and Resyncing.
- **Change:** move, tick_events, the ring and the node list in Rust,
  calling the stock atomic_tick, serialize and deserialize; the network
  handlers' calls (change_input, discard, move, enter, leave) into ours.
- **Unicorn:** the differential check of (a) over every path (first world,
  resync, refusals, the ring full), then the full game in a test match.
- **Gain:** the state is ours to read (resync decisions on the client, a
  catch-up budget per frame); **risk:** the largest; only after (a), (b),
  (d) have run live.

### Order

(a) now. Then the server's enum refactor (8). Then (d) (with the server's
reach), (b) after a measurement, (e). (c) only when the owner wants a
simulation change.

---------------------------------------------------------------------------

## 8. The server's migration to enums

1. **A refactor PR, no behaviour change.** `ClientSync`, `Start`,
   `Agreement`, `Place` (3.5, 3.6) replace `Stage` and the fields of 3.4;
   each transition is a function `(state, event) -> (state, actions)`;
   an event a state does not take is logged once. Shared types
   (`Generation`, wire ticks, the grid) move to `sync-core`.
   **Proof:** every existing test unchanged; and a replay harness that
   feeds a recorded match (`SRP_RECORD_DIR`, `.srprec`: every datagram in
   and out per place) to `Game::receive` / `Game::poll` with the recorded
   times, old and new code side by side, comparing the outgoing messages
   per place and time (message level: the recorded acks name the old
   server's sequence numbers). Five live recordings of 2026-10-25 are kept
   privately (never committed): 0x7b29ddf0, 0x3fb35f05, 0x1374fe21 (clean,
   no out-of-sync), 0x27e8fbc0 (the 60 s resync loop, a player's return)
   and 0x400a6fc1, with the server's log of the first four.
2. **Then one behaviour change a PR:** C1 (a resync budget, checked by
   every redo; past it a disconnect or a fresh start), C2 (Doomed at once),
   C4/C3 (answers held for a player back in, floors from every drop
   order), C5 (a deadline for the wrong world), C10 (a rejected admission
   back to Empty), C7 (kit answers tracked).

---------------------------------------------------------------------------

## 9. Stage (a) as built (client patch 74)

**Files.** `crates/sync-core` (`#![no_std]`; `pacing.rs`: `Step`,
`Pacing`, `Plan`, `render_move`, its tests), `client-patches/
sync_core_arena` (the arena's Rust crate: `sync_render_move`, the panic
handler), `client-patches/sync_core_026g.asm` (`sc_render_move`, the
hook's entry), `client-patches/engine_026g.rs` (generated: the library for
Rust) and the library's new row `engine_world_core_move`,
`client-patches/check_sync_core_026g.py`, `selftest_026g.py` check 74,
README patch 74, client_patch.md 3.59.

**The hook.** CALLEE at 0x1F7C37: game_world::tick's `call 0x2D2E40` ->
`sc_render_move` (EAX the time, the core at [esp+4], `ret 4`, EBX ESI EDI
EBP kept; it aligns the stack to 16 for the call, the game's frame being
aligned to 4) -> `sync_render_move(core, time)` (cdecl) -> the rule -> the
game's move through `engine_world_core_move`.

**The machine** (3.3, `Step`): the types make the illegal states
unrepresentable and the raw fields are parsed once. `GameTime(u32)` (its
wrap-safe `since`, `grid_floor`, `earliest`) and `CoreId(u32)`; the world
is `World::Frozen{core} | World::Running{core, now: GameTime, rollback:
Option<GameTime>}`, made by the arena glue alone from +18FC0h, +18FC8h
(-1: none) and +18FDFh, which also encodes the one value it writes back.
`Pacing::{Free, Paced{core: CoreId, at: GameTime}}` is the state between
render moves; `render_move(Pacing, World, target: GameTime) -> (Pacing,
Plan::{Stock, Postpone{keep: GameTime}})` is the transition, total over its
inputs; `Step::of(now, rollback, target)` is Behind exactly when `rollback
< now && rollback < target`:

| World and step | Guard | Plan | Next pacing |
|---|---|---|---|
| Frozen | | Stock (move does nothing) | unchanged |
| Running, Current | | Stock | unchanged |
| Running, Behind, shallow (to >= the grid point at or before now) | | Stock | unchanged |
| Running, Behind | target <= now | Stock | Paced{core, target} |
| Running, Behind | Paced{same core, at}, target since at < 50, target since to < 300 | Postpone{to} | unchanged |
| Running, Behind | otherwise | Stock | Paced{core, target} |

After a postponed move the glue writes back `restored_point(keep, the
point the move left)`: the earliest of the two, `keep` when the move left
none.

**The differential check** (`check_sync_core_026g.py`, Unicorn, the
original exe and the build): one emulated game a run, the game's own move,
tick_events, insert_impl, erase_impl, the ring and the node list; a model
world. Each timeline runs on the original's move, the build's move and the
hook, with on_idle's horizon move where its test asks. Every frame's
event-horizon item (time and world), the ring at the end, the world's time
and rollback point, and every input's acceptance are the same in all three
on all nine timelines; no horizon item changes after it was the horizon
item. The hook is entered with the stack at each residue of 16 in turn and
must enter `sync_render_move` aligned (esp + 4 a multiple of 16) every time. The work done (stock -> paced):

| Timeline | World loads | Steps | Re-simulated steps | Frames with a rollback |
|---|---|---|---|---|
| inputs on time, 100 fps | 319 -> 319 | 1284 -> 1284 | 649 -> 649 (0%) | 236 -> 236 |
| 8 players, 120-260 ms late, 100 fps | 559 -> 488 | 5678 -> 2796 | 5079 -> 2197 (-57%) | 395 -> 103 |
| 8 players, 120-260 ms late, 60 fps | 377 -> 368 | 4204 -> 2310 | 3721 -> 1827 (-51%) | 269 -> 90 |
| 8 players, 120-260 ms late, 30 fps | 228 -> 228 | 2877 -> 2237 | 2532 -> 1892 (-25%) | 144 -> 89 |
| 12 players, 60-420 ms late, 100 fps | 562 -> 532 | 12526 -> 10986 | 11927 -> 10387 (-13%) | 440 -> 297 |
| bursts of 30 at once | 361 -> 319 | 2841 -> 1425 | 2442 -> 1026 (-58%) | 229 -> 58 |
| a 700 ms stall, caught up in 16 ms logic ticks | 408 -> 363 | 4051 -> 2077 | 3598 -> 1624 (-55%) | 261 -> 75 |
| precise inputs, two in a tick | 372 -> 347 | 8295 -> 5363 | 7896 -> 4964 (-37%) | 266 -> 123 |
| late roster changes, refused inputs, discards | 456 -> 424 | 3924 -> 2762 | 3425 -> 2263 (-34%) | 274 -> 121 |

The timelines are made up (every place changes its input every 50 ms, or
at random off-grid times for the precise one); the live gain is to be read
from patch 71's per-frame trace.

**Not covered (stage (a)):** the local player's own off-grid input snapping
(snap_inputs with a local place: its input lists are not modelled; it only
lowers the point, which the postponement keeps as the least); a real
world's load and step costs (the model counts them).

---------------------------------------------------------------------------

## 10. Reusable places (a design; the stage after the pacing and the server's enums)

The owner, 2026-10-25: "I thought if you leave, others can join in your
place ... can't the new Rust code be designed around that ... so people can
rejoin."

### 10.1 Why places are used up today

A match's places are append-only. A late joiner, or a player who comes back
as a new seat, always takes the next index; a place given up (0xE0 empty)
is never taken again; the room left is 20 less the seats ever used. So a
match runs out after enough leaves and joins (the "7 v 4" of a long match),
and a full 10 v 10 can never take anyone back. The reasons are in the
client:

| What a place has | Where | Bound | Today's rule |
|---|---|---|---|
| the slot record, 30h bytes | core +18458h (begin), +1845Ch (end), +18460h (capacity); `+0/+4/+8` the three kits' players, `+0Ch` the kit in use, `+0Dh` the next kit, `+10h` itself, `+14h` its delegate (the slot callback), `+20h` -1, `+24h` FFh, `+2Ch` the place | late join install (`lj_native_install`, late_join_install_026g.c) | **appended**: install refuses unless `end == begin + 30h * place` |
| the profile | match options `+ place * 45FCh`; the roster count at options `+577B5h`; the squad at `+5782Ch + place` | install decodes the wire profile into it | the count only grows |
| callback ids | `(place + 1) << 16` through core +18FB0h | bind_slot_reserved (late_join_026g.asm) | a range per place: reusable as is |
| weapon ids | core +18B4Ch + place * 24 (six a place: three kits, two weapons) | bind_slot_reserved | a range per place: reusable as is |
| the slot's grenades and traps | nodes of the saved-objects list (core +18914h first, +18918h last; next +8, prev +4) | bind_slot_reserved puts a slot's nodes **in front**, newest slot first | the layout of every history item's game-objects bytes; `shift_saved_objects` prepends zero bytes to every item when a slot is bound |
| tick delegates | the seven lists at core +18840h (the slot's delegate 0x321790 sets +0Dh from the newest kit choice each step) | 0x321820 (player_slot::initialize) | never removed |
| animations | the match core +19238h table | install (`bind->animations`) | appended in roster and kit order |
| statistics | twenty records (late join keeps twenty whatever the roster); the live count at statistics +8 +31E0h | install sets the count to place + 1 | in the world: snapshot, hash |
| the input lists | core +2FCCh + place * 480h, core +E3E4h + place * 190h | the core's constructor (twenty, patch 11) | survive everything (match.md 17) |
| in a snapshot | the active mask's places only: their player blocks, their `+0Dh` (0x2D2A24..0x2D2A37) | | an empty place's next kit is not carried (#448) |

### 10.2 The place's lifecycle with reuse

```
                 roster (start / admission entered)              0xEA, not given up
  Empty -------------------------------------------> InPlay -------------------------> Away{left_at, last_kit}
    ^                                                  ^  ^                               |   |
    |                                                  |  +---- 0xE9 back (same player) --+   |
    |                                                  |                                      | 0xE0 empty (gave up), or
    |                                    Entering{admission, occupant n+1}                    | away past the reuse wait
    |                                                  ^                                      v
    |                                                  |  admission (Prepare / installed)  GivenUp{left_at}
    |                                                  |                                      |
    |                                                  +---- Vacant{since, occupant n} <-------+
    |                                                         (inactive in every world a game     ring + lead past left_at:
    +---------- (never: an index is not unbound)               could load; reusable)                no world of the ring has it
```

**Occupant** is a counter per place (0 for the first player); every
variant past Empty carries it, and the client keeps it too. A place is
**Vacant** once it has been out of play for longer than any game can roll
back across: `left_at + ring (1000 ms with stage (d), 550 today) + the
roster lead (300 ms)` before the server's clock, so no history item of any
game has it active, and no relay, drop or roster change for the old
occupant can still arrive.

### 10.3 What every client must do, alike, at the same tick

The rule that makes it deterministic: **the recycling is two halves, one
outside the simulation and one inside it**, as the late join already is
(install outside, entry inside).

1. **Uninstall (outside the simulation, at the server's Prepare for a
   Vacant place):** only for a place that no history item has active (the
   client checks: the masks list +12280h has the place's bit clear in
   every node at or after the oldest item's time; else it answers "busy").
   - the old occupant's three kit players: their tick-list nodes unlinked
     from the seven delegate lists, their weapons and inventory objects
     destroyed through the game's own destructors (the player's
     destructor is already hooked: `player_destroyed_hook` 0x1F2013);
   - the slot's grenade and trap nodes taken out of the saved-objects list,
     and **their bytes taken out of every history item's game-objects
     buffer** (item +BDF30h: +4 the bytes, +8 the room, +0Ch the size): the
     inverse of `shift_saved_objects`. A node of an untouched grenade or trap
     saves one zero byte; a place inactive across the ring has none touched
     (to be proved per object kind: grenade_core::serialize 0x303DA0,
     booby_trap_core::serialize 0x3052E0);
   - its callback ids (`(place + 1) << 16`) and weapon ids released; its
     animations left (they are presentation; ids keep growing: to check
     that nothing a world holds names an animation id);
   - the slot record's kit players cleared; the profile freed (references
     counted, as install takes them).
2. **Install (outside, as today) into the same index:** install takes a
   place below the end (the `end == begin + 30h * place` test becomes "the
   end, or a Vacant place"); the new profile is decoded over the old;
   bind_slot_reserved binds into the same ranges.
3. **The canonical layout.** "The newest slot first" stops being a
   function of the index once indexes are reused, so the saved-objects
   list's order becomes **by place, descending** (place 19's nodes first,
   then 18, ... then the map's objects), whatever the order of binding:
   bind_slot_reserved inserts a slot's nodes before the first node of a
   lower place (not at the front), and the history items get the zero
   bytes at the same offset (not at the front). A game that starts from a
   snapshot binds its slots in index order and gets the same layout, by
   construction. (This changes nothing for today's matches: places are
   bound in ascending order, each in front, which is that same order.)
4. **Reset (inside the simulation, at the entry tick T):** the new
   occupant's 0xE9 carries "a new occupant" (the place byte's bit 7, under
   a revision of its own; every game of the match runs it). In the step to
   T, before the slot delegates (the change node is processed before the
   step, 1.3), process_events resets the place's world state: its
   statistics record (score, kills, deaths: what the scoreboard and the
   results read), `+0Ch` and `+0Dh` to the new occupant's first kit (the
   kit choice at T, as #448 does for a return), the rules' per-place
   state (respawn timers, the battery carrier, Research's units: a list to
   read per rule), the team (from the new profile). Because it is a world
   change at a tick, a rollback across T undoes and redoes it like any
   entry.
5. **The hud:** the scoreboard row, the kill feed names and the tab list
   read the profile; patch 39's row refresh and patch 16's counters must
   see the occupant change (an entry of a place whose row exists).

### 10.4 What the server and the snapshot must carry

- The server's place machine (3.6) with `Vacant` and the occupant counter;
  admission into a Vacant place is the late-join admission (Prepare to every
  game, installed from all, a fresh world for the newcomer, its catch-up
  checked, entry at a tick all are told) with "replace" in the Prepare.
- A game that starts from a snapshot after the reuse binds the new profile
  at load (0xE4 carries the current occupants' profiles, as it does for the
  roster today); a snapshot taken before the entry tick must not start a
  game that will see the entry (as today: `usable_snapshot` needs worlds
  after the place's last roster change).
- **Empty places' state:** a snapshot carries `+0Dh` and the player blocks
  of active places only. For a Vacant place that is right (nothing of it may
  be read); for an Away place the server keeps `last_kit` (#448's re-relay)
  until the place is GivenUp.
- The game-objects section of a snapshot is in the canonical layout (10.3,
  3), so a joiner's own binding reads it.

### 10.5 What patch 11 already does, and what is new

| Part | Today (patch 11) | Reuse needs |
|---|---|---|
| Prepare / installed / failed / busy, outside the simulation | yes | the same, with "replace" |
| profile decode, kit players, animations, physics | yes (append) | into a Vacant index |
| id ranges per place (callbacks, weapons) | yes | reused as they are |
| twenty input histories and statistics records | yes | the same |
| saved-objects nodes and the history items' bytes | in front, zero bytes prepended (`shift_saved_objects`) | canonical layout, insertion at an offset, **removal** of the old occupant's |
| destruction of a place's objects | only when the match ends (`player_destroyed_hook`) | at uninstall, while the match runs |
| world reset of a place at a tick | none (a new index has a zero record) | the new-occupant entry |
| server: a place's state | derived from events, `given_up` | the Place enum with Vacant and the occupant |

### 10.6 Cost and risks

- **Cost:** the largest stage after (e). RE: each object kind of a slot and
  its serialized bytes in the history; the delegate lists' unlinking; each
  rule's per-place state; the scoreboard's and the hud's per-place caches.
  Code: uninstall in C or Rust next to install, the layout change in
  bind_slot_reserved, the new-occupant event, the server's Vacant state and
  admission. Tests: Unicorn on install/uninstall/reinstall against a fresh
  binding of the same roster (the saved-objects list and every history
  item's bytes equal), the differential check of 9 with a reuse in the
  timeline, server tests with a place reused many times, and then a live
  test with several games.
- **Risks:** a desync the moment a place is reused if one game's layout or
  reset differs (every game of the match is affected; the hash catches it
  within a second, a resync puts it right); a crash from a pointer to the
  old occupant left in the hud, the kill feed or a rule; memory of the old
  profile. A revision of its own (the 0xE9 flag), so the server must offer
  reuse only to games of that revision.
- **Order:** after stage (a) and the server's enum refactor (the Place
  machine is where Vacant lives); before or with (d) (the reuse wait is the
  ring's length).
