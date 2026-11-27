# Retail 0.69d0 netcode: prediction, rollback and hit feedback, against 0.26g

The owner, 2026-10-25: "check how 0.69 worked for that prediction". Players see hit
markers and kill crosses that turn out wrong: a red cross for a kill that a late input
then undoes. This note says how the last retail build, 0.69d0, ran its match (lockstep or
not, history, inputs, the server's part), how it drove its hit and kill feedback, when
that changed after 0.26g, and what of it fits our 0.26g client, our server and the
sync-core plan (`sync_core.md` on `feat/sync-core`; `match.md`, `transport.md`).

**Sources.** The 0.69d0 x64 exe from `/mnt/hdd/survarium-offload/0.69d0/Survarium.zip`
(`game/binaries/x64/survarium.exe`, BCJ-decoded, image base 0x200000000), read with a
Ghidra 12 headless analysis and capstone; its `hud.swf` (FFDec); its
`gameplay/static_game_parameters.options` (resources.72.db). The x86 builds 0.27d0,
0.28a2, 0.28d0, 0.31a2 and 0.46c2 from the weapon survey (strings only). The 0.26g exe
(`identity.json`). 0.69 addresses are VAs with base 0x200000000; 0.26g ones are the
re-kit's.

Tags as in the other notes: **PROVEN** (read in the code at the address given),
**LIKELY**, **GUESS**. 0.69 has no PDB: its names come from its own assert and log
strings (`d:\survarium.public\sources\vostok\...`); a name in `code` with no such string
is ours. Every diagram is 7-bit ASCII.

Contents:
1. Short answers
2. 0.69's match model
3. Hit markers, damage numbers and kill markers
4. When it changed (0.26g to 0.69)
5. What to port, and how it fits the sync core
6. Address notes (0.69), and the 0.26g rows added to the re-kit
7. Open questions

---------------------------------------------------------------------------

## 1. Short answers

1. **0.69 kept lockstep with rollback**, the same `game_world_core` with a ring of world
   captures and re-simulation. Around it, the network layer was rebuilt
   (`network_client_syncronization.cpp`, new after 0.46c2):
   - **every client sends an update every tick**, empty or not
     (`client_game_update_message`), so the server knows when a client's inputs for a
     tick are complete;
   - **the server sends one game update a tick** (S->C 0xDF) with every place's input
     changes, entries, leaves, ping inputs and **confirmations of this client's own
     inputs** (`server_input_confirmation`). An update for tick U is the server's word that
     tick U is closed: the "inputs complete up to T" message exists in 0.69, built into
     the per-tick update;
   - **the server owns the input timeline**: an own input the server took late comes back
     confirmed at the tick it was applied, and the client moves it there; one that 500 ms
     of updates never confirmed is discarded ("Discarding input at time %d - update for
     %d still not received by server!");
   - **no input delay**: local inputs take effect at the local tick. Instead the client
     **steers its clock ahead** of the server from the confirmations, so its inputs arrive
     2 ms (`netsync_send_window_in_ms`) before the server closes their tick;
   - **ping is a simulation input** (`ping_input`): each client measures its latency and
     puts it in the world, and the simulation **lag-compensates hits**: bullets, grenade
     sensors and explosions are tested against the targets' hitboxes as they were
     `remote_player_delay_in_ms(shooter)` ago (a ring of hitbox copies, one physics
     collision group a tick);
   - the server sends a **per-tick state summary** (S->C 0xE0: a byte a place and global
     bytes); the client compares it with its own and **asks for a resync itself**, with a
     reason ('latency too large', 'engine error', 'network lag or packet loss', 'query
     update from server').
   History: 47 world captures on a client, 22 on the server (sized in bytes, so fixed);
   the client's event horizon is a constant 500 ms (`max_ping_time_in_ms` is gone); the
   tick length is a byte the server sends in the static match info (LIKELY 20 ms, see 2.2).
2. **Hit markers: at once, from the prediction**, in 0.26g and in 0.69 alike, and never
   taken back: the marker reads the local player's damage collector, which is world state
   (it rolls back with the world), but a marker on screen only fades on its timer.
   **Kill feedback: after confirmation in 0.69.** A kill is a world event created
   "not skip horizon", so the HUD takes it only once it is **sealed**: older than the
   newest ticked time minus the local player's confirmation delay (its latency rounded up
   to a tick, at most 500 ms, `0x2002F1110`). A kill a rollback removes before then is
   never shown. 0.69 also has a kill-cross gate (`kill_confirmation_timeout_in_ms`, 1000)
   on its crosshair, but no code writes its inputs, so 0.69d0 draws no red cross at all;
   kills reach the player through the kill feed and the kill camera (LIKELY). 0.26g has the
   same "sealed" test for its red kill cross, but with a delay field that is always 0, so
   **0.26g shows the red cross as soon as the kill is simulated**. Damage numbers: neither
   shows them in play.
3. **When:** 0.26g to 0.31a2 is the 0.26g design (deeper input history, kill cam).
   0.46c2 brings ping inputs, Huffman-coded inputs, "important" inputs, `remote_delay`
   and `allow_killer_claim`. After 0.46c2, 0.69d0 has the per-tick updates, the
   confirmations, client-side clock steering, client-requested resyncs, effect and decal
   rollback, and the kill-confirmation timeout. Section 4.
4. **Port, in this order:** (a) **sealed kill crosses**: 0.26g already passes a "sealed"
   bound to its kill-cross walker (`0x1FB6C0`), `max ticked - [game_world+560h]`, and that
   field is always 0; giving it the local player's confirmation delay is a tiny client
   patch with the same effect as 0.69 (small cost, low risk; the cross comes about one
   round trip later and is never wrong). (b) a **server "complete up to T"** message, to
   make the bound exact instead of estimated (medium: a server message, a client handler,
   a per-tick heartbeat from each client). (c) **clock steering from the server's view of
   input arrival** (medium; fewer late inputs, so fewer rollbacks). (d) **input delay**:
   0.69 did not use it; possible as an option (low-medium cost, feel risk). (e) **lag
   compensation** and the whole per-tick protocol: a simulation and protocol rewrite (high
   cost and risk; not now). Section 5.

---------------------------------------------------------------------------

## 2. 0.69's match model

### 2.1 Still lockstep with rollback

PROVEN, from the code:
- `game_world_core` keeps the 0.21d/0.26g names and machinery: `game_state_history_item`
  (an intrusive list; item size 0x106268, time at item+0x106238),
  `new_game_state_history_item` (string 0x200989B90), `insert_impl` 0x2002F1F20
  ("SKIPPING input %d.%03d (oldest is %d.%03d)"), `move(uint, uint)` 0x2002F6320,
  `deserialize(buffer_reader&, history_on_deserialization_enum, uint)` 0x2002F1360
  ("--- RESYNCHRONIZATION ---", "--- THE VERY FIRST SYNCHRONIZATION ---"),
  `change_input<client_update<T>>` for T = player, profile, ping and ai inputs, with the
  0.26g refusals ("input is SKIPPED(too old) ... -reject_inputs").
- Remote inputs arrive with times in the past of the local world (2.4) and are inserted
  there; the world is re-simulated from the capture before them.
- New: `game_world::rollback_effects` 0x2002356A0 and `process_new_decal_in_history`:
  effects, decals and tracers made by re-simulated ticks are reconciled on a load instead
  of being made again blindly (LIKELY on the details; HUD markers are not part of it).

### 2.2 History, horizon and tick

| What | 0.26g | 0.69d0 | Evidence (0.69) |
|---|---|---|---|
| Tick | 50 ms, fixed (`m_start_fixed_time_in_ms` 50) | a byte from the static match info, match settings +1083h; the grid origin is one tick | reader 0x200125061 (`u16 +1078h, u16 +107Ah, u8 +1083h, u8 +1080h, u32 +1074h ...`); ctor 0x2002EF530 stores it at core+180A94h. PROVEN |
| World captures | 11 on a client (`(max_ping 250 + 250)/50 + 1`), 1 on the server | **47 on a client, 22 on the server** (`0x3021118` / `0x16874F0` bytes of 0x106268) | the match cook 0x2002E5370: 0x2002E5E72, 0x2002E5EBF (server when the cook's byte +3Ch). PROVEN |
| Event horizon (client) | `max_ping_time_in_ms` + 250 = 500 | **500, a constant**, rounded down to the tick (`500 - 500 % tick`); 0 on the server and in a replay-like mode (core+180ADAh) | change_input 0x2001B338B, process_game_update_message 0x2001B001F, 0x2002F1110, 0x2001AED40. PROVEN |
| `max_ping_time_in_ms` | option, 250..1000 | **gone** (no string from 0.46c2 on) | strings. PROVEN |
| Server's "too new" | (none) | an input at or past now + 920 ms (0x398) is refused on the server | change_input 0x2001B315F. PROVEN |
| Max remote delay | (none) | u16 from the static match info (settings +107Ah), at core+180ACCh | 0x200125048, ctor. PROVEN |
| Hitbox history | (none) | `max remote delay / tick + 1` slots, at most 16 | 0x20039962A, 0x20040FDB1. PROVEN |

**The tick, LIKELY 20 ms.** The server's value is not in the client. 920 ms (the server's
"too new" window) is 46 x 20, and 47 client captures cover 920 ms at 20 ms a tick; 920 is
not a multiple of 50. A 20 ms tick and a 47-item ring (920 ms, against a 500 ms horizon)
fit together; a 50 ms tick would make the ring 2.3 s for no reason. GUESS for the exact
value, LIKELY that it was below 50.

### 2.3 What the client sends: an update every tick

```
 every frame: network_client per-frame pass 0x2001B11F0
   now_tick = floor(local clock, tick)
   if the local player is in play:
       send_game_updates(now_tick)                            0x2001B0F80
         for t = last_sent + tick .. now_tick:                (at most max-delay back)
             snap own inputs at t (per kind)                  0x2002FA710 snap_input_impl<player_input>,
                                                              0x2002FAA20, 0x2002FACF0, 0x2002FAFB0
             msg 0xCD = client_game_update_message(t)         0x20032ECE0 serialize
             send; core+180AC4h (last sent) = t
   if a resync is due (0x2001AED40): send_query_dynamic_match_info   0x2001AE5F0
```

- `client_game_update_message::serialize` 0x20032ECE0 (string
  `game_update_message.cpp`, "Invalid game update message generated! Update time is %d"
  when t is off the grid): `u32 tick`, then counts of player inputs, "empty" player
  inputs, profile inputs and ping inputs, then each player input as **a byte back from the
  tick** (its precise time, `tick - time`) and its payload, the profile inputs, and each
  ping input as a u16. PROVEN.
- **One message a tick, changes or not.** An update with no change still says "nothing
  changed up to t": the server learns that this client's inputs are complete up to t.
  0.26g sends inputs only on change (0xDC/0xDD/0xDE), so its server cannot know that.
  PROVEN for the loop; the id 0xCD is set at 0x2001B111D; that it is the
  "player input changed" row of the net-stats overlay is LIKELY (0.46c2's templates give
  player inputs C->S id 205 = 0xCD).
- Redundancy: none above the transport (each update is one message; resends are the
  transport's). LIKELY.

### 2.4 What the server sends: one game update a tick (S->C 0xDF)

The match dispatch `network_client::on_match_packet_received` 0x2001EDE00
(`network_client_handler.cpp`, "unknown message type : %d") takes only these ids:

| Id | State | Handler | What |
|---|---|---|---|
| 0xDC | 0 | inline | ports; answers with 0xCA. State 1 |
| 0xDD | 1 | 0x2001F3460 | static match info (tick, max delay, roster). State 2 |
| 0xDE | >= 3 | 0x2001AF7B0 `process_dynamic_match_info` | a world (`u32 time, u8 compressed`, then the world); loaded by 0x2002F1360 |
| 0xDF | >= 3 | 0x2001AFCB0 `process_game_update_message` | **the per-tick game update** |
| 0xE0 | 4 | inline, 0x2001ADEC0, 0x2001AE1B0 | the per-tick state summary (2.8) |
| 0xE2..0xE5 | | | BattlEye and the anti-cheat kick |

So **0.26g's per-message relays are gone**: entered (0xE9), left (0xEA), the input relays
(0xEB/0xEC), the inputs of a tick (0xED/0xEE), move and discard own input (0xEF..0xF2),
the kit relay and its move and discard (0xF3..0xF5) all became parts of 0xDF; set time
(0xE5), ping (0xE6) and clock offset (0xE7) have no message of their own (2.6). The
net-stats overlay (0x2001698D0) still lists the old names as statistics rows. PROVEN.

**0xDF layout** (parser 0x20032E4A0, PROVEN):

```
 u32  U                      the tick this update closes
 u8   (a byte kept with U)
 u32  entered mask           places entering at U (20 places)
 u32  left mask              places leaving at U
 u8   n player inputs        u8 n profile inputs   u8 n ping inputs   u8 n ai inputs
 u8   n confirmations        then n x { u32 own input tick, u32 server time }   server_input_confirmation
 n x  { u8 back, u8 place }  player input times: U - back
 u32  "empty input" mask     places whose input at U is the empty one
      player input payloads  (Huffman bit stream when the tree is in use)
 n x  { u8 place, payload }  profile inputs at U
 n x  { u8 place, u16 ping } ping inputs at U
 n x  { u8 place, u16, u16 } ai inputs at U
```

**How the client takes it** (`process_game_update_message` 0x2001AFCB0, PROVEN):

```
 0xDF(U) arrives
   |
   +-- seen U already (a ring of the last 257 update ticks, net+7A7DC8h)? -> drop
   |
   +-- close own ticks: 0x2002F6230 (snap at U), send_game_updates(U)        0x2001B0F80
   +-- remember U in the ring
   +-- latency sample, maybe a ping input (2.7)                              0x2001AEDF0
   +-- each confirmation {t, server_time}: clock steering (2.6)              0x2001AF210
   |
   +-- now - (500 rounded) > U ?  "large network lag or packet loss detected, will resync!"
   |
   +-- every tick t <= U whose update did arrive: own inputs in (t - tick, t] are final
   |                                                                       0x2001B2BC0
   +-- each own input still waiting (net+7A69A8h list, by tick t):
   |      confirmed in this update  -> if t != U: move it to U             0x2001B24D0
   |      not confirmed:
   |         first tick >= t whose update has not come = M
   |         M <= t + 500 -> move it to M (it can take effect no earlier)  0x2001B24D0
   |         M  > t + 500 -> "Discarding input at time t - update for M
   |                          still not received by server!"               0x2001B2880
   +-- rollback point (core+180AA4h) set from the oldest change (0x2001B093E, LIKELY)
   +-- entries (0x2002F25A0), leaves (0x2002F29E0; own place: close the match)
   +-- change_input for each player input (0x2001B3110), profile input
       (0x200138310), ping input, ai input (0x2001B41B0)
```

The server is the authority on **when** each input happens. Remote inputs reach a client
only inside the update that closes their tick. The client's own inputs are predicted at
the local tick and then **confirmed, moved or discarded** by the server's updates. 0.26g
had the same three outcomes as separate messages (0xEF move, 0xF1 discard, and silence
for "taken"); 0.69 made "taken" explicit and tied all three to the tick stream.

### 2.5 The server: it simulates

LIKELY: the 0.69 server ran the world. Its core is the same class with `m_is_server`
(core+180AD9h) and gets a **22-capture history** (0.26g's server got 1); change_input's
server branch refuses inputs "too new" (920 ms ahead, 0x2001B3164) and moves late ones;
the server answers a client's resync query with a world of its own (0xDE) and sends a
per-tick state summary (0xE0). The client exe has no server code beyond that branch, so
the rest is inference.

### 2.6 The clock: steered ahead, no input delay

- **No input delay** found: own inputs are snapped and sent at the local tick
  (`send_game_updates` sends t up to the local now), and change_input has no lead for
  local inputs. PROVEN for the send path; LIKELY that there is none elsewhere.
- **Steering, PROVEN.** Each `server_input_confirmation {t, server_time}` gives the
  margin `server_time - t + netsync_send_window_in_ms` (0x2001AF210; `send_window` at
  0x200AA2220, default **2**, range 0..100). The margins go into a 257-sample ring; the
  window's minimum (0x2001B1AC0) drives `floating_timer` corrections (0x20011A000: run
  faster or slower until the offset is made up; 0x20011A970: set outright on a big
  error). The client runs **ahead of the server** by its one-way latency plus the send
  window, so its input for tick t reaches the server just before the server closes t.
- `netsync_receive_window_in_ms` (value 0x20F43FCCC, default 0, 0..100) is added to the
  latency samples of 2.7. `netsync_system_timer_scale` (0x200AA2208, 1.0, 0.5..2) scales
  the timer. The three are console options (initialisers 0x200078F10, 0x200078F80,
  0x200078FF0).
- 0.26g: the server sets the clock (0xE5) and corrects it (0xE7, patch 15 makes the
  correction 6x faster). The 0.69 client corrects itself from evidence the server sends
  with every tick.
- `minimaze_input_lag` / `st_minimize_input_lag_option` sit with the engine's thread
  names (":logic", ":render"), not with the netcode: LIKELY a render/logic pipelining
  option (fewer frames queued), not network input delay.

### 2.7 Ping in the world, and lag-compensated hits

- **The measurement**, 0x2001AEDF0 (on every 0xDF): the time between the update's tick
  and its arrival goes into a 257-sample ring; a mean plus a spread (LIKELY: 0x2001B1A50,
  0x2001B19B0), capped at 500, gives the client's latency. When it changes it is put into
  the world as a **`ping_input`** for the next tick (`change_input<ping_input>`
  0x2001B50B0), sent to the server like any input. PROVEN.
- **`remote_player_delay_in_ms(place)`** 0x2002F1190 (its own assert string): a base per
  place from the roster (place record +5Ch) plus one tick, or a ping-derived value, or
  nothing, by a mode byte (+4Bh), clamped to `[tick, max remote delay]`. PROVEN for the
  arithmetic; that the ping-derived value is the place's `ping_input` is LIKELY.
- **Hitbox history.** `base_player::insert` (0x200308990, its string "inserting active
  player %d") makes each player's hit body 21 times, collision group
  `0x8000 << i | 0x40`. A **bullet** (0x2003994A0, via 0x200398020 from
  0x2003BCBE0, LIKELY the bullet manager's tick) ray-casts with the group of slot
  `((t - remote_player_delay(shooter)) / tick) % (max_delay / tick + 1)`; so do
  **explosions** (0x20040FD40, from `grenade_core_exploded_state` and
  `explosive_trap_core`) and the **sensor grenade** (0x2003EBB50). Hits are tested where
  the targets were `delay` ago, as the shooter saw them. PROVEN for the masks and the
  delay; "as the shooter saw them" is the evident intent (LIKELY).
- Because the delays are world state (ping inputs), every client and the server compute
  the same hits: lag compensation **inside the lockstep**, not a server override.
- The death camera (LIKELY) paces its phases by the watched player's delay
  (0x2001E5000, 0x2001E6370), and a game_world pass shows a view at
  `time - remote_player_delay(that player)` (0x200237200 -> 0x200236440). PROVEN for the
  calls; that this is the kill camera showing what the killer saw is LIKELY.

### 2.8 The per-tick state summary and the client's resync request

- **0xE0** (state 4): a tick index byte (unwrapped against the local time), then up to 20
  per-place bytes (with a presence mask) and global bytes (0x2001ADEC0). The client keeps
  its own summary of each tick (a 64-slot ring) and compares (0x2001AE1B0): each place
  that matches advances that place's "agreed up to" (net+7A693Ch, 20 entries); a mismatch
  marks a resync as due (net+7A698Ch). PROVEN for the comparison; that the bytes are
  per-place hashes is LIKELY (the resync log prints "global state %d, players mask").
- **The client asks** (0x2001B11F0 -> 0x2001AED40 -> `send_query_dynamic_match_info`
  0x2001AE5F0): "requesting resynchronization for time %d, global state %d, players mask
  ..., reasons: 'latency too large' 'engine error' 'network lag or packet loss' 'query
  update from server'". The server answers with 0xDE; "ignoring resync request: hash on
  time %d matches" when it is not needed. 0.26g: only the server decides a resync.

### 2.9 The whole loop

```
 client A (ahead by its latency + 2 ms)        server (closes tick U)          client B
   |                                              |                              |
   | 0xCD update(t) every tick ------------------>|                              |
   |   own input predicted at t, world goes on    | waits for every client's     |
   |                                              | update(U), or late: re-times |
   |                                              | the input to the tick it     |
   |                                              | arrived in                   |
   |<--------------- 0xDF update(U): all places' changes, A's confirmations ---->|
   | confirmed at t: done                         |                              | B: A's input at
   | confirmed at U != t: move own input to U     |                              | time <= U lands in
   | 500 ms of updates without it: discard        |                              | B's past: rollback,
   |                                              |                              | re-simulate
   |<--------------- 0xE0 summary(U) ---------------------------------------------|
   | compare, maybe ask for 0xDE                  |                              |
```

---------------------------------------------------------------------------

## 3. Hit markers, damage numbers and kill markers

### 3.1 0.69d0

- **Hit marker, predicted, PROVEN.** The local player's `inflicted_damage_collector`
  (player+1F340h: victim, time, damage, killed) is written by the hit handler in the
  simulation (0x200229B50: damage / max health x `hit_mark_damage_amount_koef`, capped
  at 1), cleared by a per-tick expiry (0x20022A3C0, after
  `crosshair_hit_detection.accumulation_time` = 0.3 s) and serialized with the player
  (0x200229800), so it rolls back with the world. The HUD's per-update pass 0x2001A5190
  shows a marker when the collector's victim or damage changes: level 1..3 by
  `crosshair_hit_detection.damage_breakpoint` [0, 0.15, 0.75], fade delay
  `enemy_hit_mark_timeout_in_ms` (200), via 0x2001AB0A0 -> hud.swf
  `HUDCrosshair.setDamage(level, delay, killed)`. Nothing removes a marker that a
  rollback undid; it fades (TweenLite, 1.3 s).
- **Kill cross, dead code, LIKELY.** 0x2001A5190 has a kill branch (red markers,
  `ColorTransform(0.85, 0, 0)`): type 3 within 200 ms of the kill, and type 2 once
  `floor(newest time, tick) - tick` has passed the kill time, within
  `kill_confirmation_timeout_in_ms` (1000): a kill shown **only once confirmed**. Its
  inputs (hud+75FCh, +7600h) are only ever reset, and the collector's killed flag is only
  zeroed, so 0.69d0 never shows it.
- **Kills, after confirmation, PROVEN.** A kill is a world event (type 0,
  `game_statistic_event_history_item`) made at 0x200304842 with `skip_horizon_time` 0
  (the allocator 0x2002F0C80 stores 0 at +3Ch). The game_world walker
  (0x2002380A1..0x200238125) hands an event to `process_statistic_event` 0x2001A9570 when
  it is **sealed** (`time <= newest ticked - 0x2002F1110(core)`) or flagged skip horizon.
  0x2002F1110 is the local player's confirmation delay: its own delay rounded up to a
  tick, at most 500 ms. So the kill feed line and the kill camera come one round trip or
  so after the kill, and a kill undone before then never shows. The damage event (type 3,
  0x200303FEA stores skip horizon 1) is taken at once.
- **Damage numbers:** hud.swf's `HitText` only behind a debug flag the exe never sets.
- hud.swf (0.69): `HUDCrosshair` `set damageIconDelay`, `set damageIconKilled`,
  `set damageIcon`; no remove or cancel call.

### 3.2 0.26g, for comparison

- **Hit marker:** `game_world_ui::draw_damage_collector` 0x238230, from
  `player::mt_draw` (0x247467): on a change of the collector it calls
  `root.crosshair_enemy_hit(fraction, 200, killed=false, debug=false)`. Predicted, never
  taken back. The same as 0.69.
- **Red kill cross:** game_world tick 0x1F7FF6 calls the walker 0x1FB6C0 with
  **EDX = core max ticked (+18FC4h) - [game_world+560h]** and the last shown time
  (game_world+564h). An event at or below EDX is sealed. For a kill (type 0, made with skip
  horizon 0 by 0x2D43B0) the walker calls 0x233770 only when sealed; 0x233770 checks
  `show_enemy_killed_marker` (0xA70EC6) and that the killer is the local player, then
  0x23CF20 -> `root.crosshair_enemy_hit(1.0, 200, killed=true)`, the red bar.
  **[game_world+560h] is zeroed by the UI constructor (0x2338CD, as ui+364h) and never
  written** (its only other reader is `root.set_game_stats` at 0x23D12F). So "sealed" is
  "simulated": the cross shows the frame the kill is first simulated, even when a late
  input will undo it. PROVEN.
- **Kill feed:** the other walker, 0x1FB660, passes "sealed" = `time <= max ticked -
  event horizon (500)` to `on_world_event` 0x23B5F0: the kill feed already waits 500 ms
  in 0.26g.
- **So the walker that would make the cross safe is already there, with its delay at 0.**
  0.69 put the same test on its kill handling, with the local player's delay in place of 0.

```
 time --->        kill simulated   late input arrives   sealed (0.69: now - own delay)
                       |                |                      |
 0.26g red cross:      X shown          rollback: kill gone    (cross still fading)
 0.69 kill feed:       .                rollback: kill gone    nothing to show
 0.69, kill stays:     .                                       X shown
```

---------------------------------------------------------------------------

## 4. When it changed (0.26g to 0.69)

From the strings of each build (0.46c2 has fewer log strings, so "absent there" may be a
stripped string). Columns: 026g 027d0 028a2 028d0 031a2 046c2 069d0.

```
max_ping_time_in_ms                      Y  Y  Y  Y  Y  -  -
input history depth (fixed_history<N>)   27 27 27 27 46 125 50
ping_input                               -  -  -  -  -  Y  Y
inputs_huffman_tree                      -  -  -  -  -  Y  Y
"important" input kinds                  -  -  -  -  -  Y  Y
remote_delay (move/discard refusals)     -  -  -  -  -  Y  -
"input is too new" (server refusal)      -  -  -  -  -  Y  Y
netsync_system_timer_scale               -  -  -  -  -  Y  Y
netsync_retransmission_time_in_ms        -  -  -  -  -  Y  -
root.allow_killer_claim                  -  -  -  -  -  Y  Y
server_input_confirmation                -  -  -  -  -  -  Y
process_game_update_message              -  -  -  -  -  -  Y
netsync_send/receive_window_in_ms        -  -  -  -  -  -  Y
"requesting resynchronization ..."       -  -  -  -  -  -  Y
"Discarding input at time ..."           -  -  -  -  -  -  Y
"client received input too late ..."     -  -  -  -  -  -  Y
move_input / discard_input (own)         Y  Y  Y  Y  Y  Y  -
"FULL RESYNC"                            Y  Y  Y  Y  Y  Y  -
hash_check_interval_in_ms                Y  Y  Y  Y  Y  Y  -
game_world::rollback_effects             -  -  -  -  -  -  Y
kill_confirmation_timeout_in_ms          -  -  -  -  -  -  Y
enemy_hit_mark_timeout_in_ms             Y  Y  Y  Y  Y  Y  Y
root.crosshair_enemy_hit (red cross)     Y  Y  Y  Y  Y  Y  -
root.show_kill_cam                       -  -  -  -  Y  Y  Y
root.show_resync / add_resync_ping       -  -  Y  Y  Y  -  -
```

- **0.27d0** renumbers every message id (the template ids shift by one) and adds the
  sync-gears HUD icon and `show_enemy_damaged_marker`.
- **0.28a2** shows resyncs to the player (`root.show_resync`, `add_resync_ping`).
- **0.31a2** adds the kill camera and an input history of 46.
- **0.46c2** is the first netcode change: ping as an input, compressed inputs,
  "important" inputs, a `remote_delay` in the move/discard refusals, server refusals of
  inputs too far ahead, the input history at 125, `max_ping_time_in_ms` gone. The hitbox
  slots of 2.7 probably start here (`remote_delay`), GUESS.
- **0.69d0** (after 0.46c2; 0.52 has no exe in the survey): the per-tick updates and
  confirmations, own move/discard folded into them, client clock steering,
  client-requested resyncs, effect and decal rollback, sealed kills and the (unused)
  kill-confirmation timeout; `root.crosshair_enemy_hit` gone (the crosshair moved to
  `HUDCrosshair`).

---------------------------------------------------------------------------

## 5. What to port, and how it fits the sync core

Ordered by gain for the complaint (wrong red crosses and markers) against cost.

### 5.1 (a) Sealed kill crosses: the 0.69 rule on the 0.26g walker

- **What.** Make the red kill cross wait until the kill is older than a confirmation
  bound. In 0.26g that is one number: `[game_world+560h]`, which the kill-cross walker
  already subtracts from the newest ticked time (0x1F7FE0). Stock is 0.
- **The bound.** 0.69's: the local player's own delay rounded up to a tick, at most
  500 ms. For 0.26g, `round_up(rtt, 50)` capped at 500, with rtt the round trip the
  client already measures (our ping display), is the same idea. A constant (150-250 ms)
  is the smallest first version. With the server message of 5.2 it becomes exact.
- **Why it works in 0.26g.** A world load drops every world event after the loaded time
  (0x2D3700), and the re-simulation makes them again. A kill undone by a late input is
  never made again, so it never reaches the bound; a kill that stands is shown once
  (the walker keeps the last shown time at game_world+564h).
- **Hooks.** A write of the bound into game_world+560h each frame (or a CALLEE at
  0x1F7FE0's `sub edx, [ebx+560h]`). Check first: `root.set_game_stats` reads the same
  field (0x23D12F) as a displayed stat, so the stats line would show the delay (decide:
  harmless, or move the bound to a field of ours and patch the `sub`).
- **The cost of it:** the cross comes `bound` ms after the shot that killed; the sound and
  the body fall stay predicted. Players see fewer crosses only when a kill was undone.
- **Cost / risk:** small (a few bytes, one patch, its test in Unicorn: a scripted kill
  then a late input that removes it); low risk. No simulation change, no server change.
- **Sync-core fit:** a read-only consumer of the Step machine: `Playing{.., confirmed:
  GameTime}`, where `confirmed` is the estimated bound now and the server's T later
  (5.2). The kill-cross bound is `confirmed`. In Rust: `sync_core::confirm::bound(rtt,
  tick) -> GameTime` and, later, `Confirmation::{Estimated{rtt}, Server{complete_to}}`.

### 5.2 (b) A server "inputs complete up to T" message

- **What 0.69 had:** each closed tick U came with every input of U, so the tick was
  closed by construction. Our server relays inputs as they come and has no per-tick
  closure: in 0.26g clients send inputs only on change, so the server cannot tell "no
  change" from "not arrived".
- **Smallest version for us:**
  1. each client tells the server how far its inputs are complete: a small C->S
     message every tick or every 50 ms, `{u32 own complete_to}` (its newest ticked time
     whose own inputs are all sent), or the same field added to an existing per-50 ms
     message. A client patch (the send pass `try_send_all` 0x27CFB0), a new server parse;
  2. the server keeps per place `heard_to` (that value, once every relay before it was
     sent) and sends every client `complete_to = min over other places in play of
     heard_to` when it rises (S->C, send-once, latest wins);
  3. the client stores it (a new field, read by 5.1's bound and by the sync core).
- **Uses:** the exact kill-cross bound (5.1); hit markers that wait for it (5.6); the
  sync core's rollback pacing (stage (a) of `sync_core.md`) could skip postponing past T,
  and a rollback below T is impossible by definition (a check, loud in tests).
- **Cost / risk:** medium. A protocol change both sides (one C->S, one S->C id; the
  client's id tables 0x279AE0 / 0x279BF0 have fixed ranges, so a spare id or a payload
  carried in an existing message); refuses old games by the one-supported-client rule.
  Risk low for the simulation (nothing simulated changes), medium for the protocol (a
  place that stops sending holds everyone's T: T must skip places not Live, and the
  bound falls back to the estimate when T is stale).
- **Sync-core fit:** server: a field of `Live` per peer (`heard_to`), and `Place` gets
  `complete_to`; client: an event `Complete(T)` taken in Playing only. `no_std`, no
  allocation: a `u32` per place.

### 5.3 (c) Clock steering from input arrival

- **What 0.69 did:** the server told each client, per input, when it arrived; the client
  kept its clock just early enough. Our server already measures how late a client's
  inputs arrive (the 0xEF "move" path is that case) and already corrects the clock (0xE7).
- **Port:** server-side: aim the 0xE7 correction at "inputs arrive N ms before the
  others need them" (the minimum margin over a window, as 0x2001B1AC0), not only at the
  clock offset. No client change.
- **Gain:** fewer own inputs moved, fewer late inputs at the others, so fewer rollbacks.
  **Cost / risk:** medium / medium (clock oscillation; the windowed minimum and a dead
  band are what 0.69 used). **Fit:** the server's `Clock::{Steady, Correcting}` machine
  (sync_core.md 3.5).

### 5.4 (d) Input delay

- 0.69 did **not** delay local inputs. It is possible in 0.26g: stamp local inputs
  (game_world 0x1EEDE0 -> change_input) at `now + d`. Remote inputs then arrive d ms
  "earlier" relative to their effect, so rollbacks shrink by d (and vanish for players
  whose latency is below d). Local movement and firing react d ms late.
- **Cost / risk:** low-medium cost (one CALLEE on the local change_input call, plus the
  0xDC/0xDD send times); the risk is feel, not correctness. Deterministic, no server
  change, but every game should use the same d (a match option from the server) or the
  rollbacks only move around. Offer only as an option, off by default.
- **Fit:** an input transform in the client's Playing state:
  `local_input(now) -> schedule(now + d)`.

### 5.5 (e) Lag compensation, per-tick updates, ping inputs: not now

- 0.69's hitbox slots need a simulation change on every client (N copies of every hit
  body, collision groups, the shooter's delay as world state), a ping input in the
  protocol and the server, and a full re-test of hit registration. High cost and risk. It
  also does not address the complaint: it changes *where* hits land, not *when* feedback
  is shown.
- The per-tick update protocol (2.3, 2.4) is the 0.69 design whole; for us it is the
  "whole orchestration ours" stage (e) of `sync_core.md` plus a server rewrite. 5.2 takes
  its one useful part (a closed tick) at a fraction of the cost.

### 5.6 Hit markers

- 0.69 kept them predicted. Two cheap options for 0.26g if the owner wants more:
  - keep them at once but **never red**: red only from the sealed kill (5.1); the
    hit marker's own `killed` argument is already false in 0.26g;
  - or delay them like kills: record a hit as a world event (as 0.69's damage event,
    type 3) and draw the marker from the sealed walker. A simulation-side change (a new
    world event), medium cost; the marker then lags by the bound, which players feel
    more than a late kill cross. Not recommended.

---------------------------------------------------------------------------

## 6. Address notes

### 6.1 0.69d0 (x64, base 0x200000000; not in the re-kit, which is 0.26g only)

| VA | Name | Tag |
|---|---|---|
| 0x2001EDE00 | `network_client::on_match_packet_received` (dispatch 0xDC..0xE5) | PROVEN (string 0x20096E8D0) |
| 0x2001AFCB0 | `network_client::process_game_update_message` (S->C 0xDF) | PROVEN (string 0x200966D70) |
| 0x20032E4A0 | game update message deserialize (the 0xDF layout) | PROVEN body, our name |
| 0x20032ECE0 | `client_game_update_message::serialize` | PROVEN (string) |
| 0x2001B0F80 | send own updates for each tick up to t (msg 0xCD) | PROVEN body, our name |
| 0x2001B11F0 | the per-frame sync pass (sends, resync check) | PROVEN body, our name |
| 0x2001AF7B0 | `network_client::process_dynamic_match_info` (0xDE) | PROVEN (string 0x200966EB0) |
| 0x2001AE5F0 | `network_client::send_query_dynamic_match_info` | PROVEN (string 0x200966BE0) |
| 0x2001AED40 | resync due? (time or 0) | PROVEN body, our name |
| 0x2001AE1B0 | compare the 0xE0 state summary with our own | PROVEN body, our name |
| 0x2001ADEC0 | parse the 0xE0 summary | PROVEN body |
| 0x2001AEDF0 | latency sample from an update; emits a ping input | PROVEN body, our name |
| 0x2001AF210 | a confirmation's margin -> clock steering | PROVEN body, our name |
| 0x2001B1AC0 | windowed minimum of the margins | PROVEN body |
| 0x20011A000 / 0x20011A970 | floating timer: gradual correction / set | PROVEN body |
| 0x2001B2BC0 / 0x2001B24D0 / 0x2001B2880 | own inputs of a tick: finalize / move / discard | PROVEN body, our names |
| 0x2001B3110 | `game_world_core::change_input<client_update<player_input>>` | PROVEN (string 0x200967020) |
| 0x200138310 | change_input<profile_input> | LIKELY |
| 0x2001B50B0 | change_input<ping_input> | LIKELY (called with the ping) |
| 0x2001B41B0 | change_input<ai_player_input> | LIKELY |
| 0x2002F6230 | snap own inputs at a tick (4 kinds) | PROVEN body |
| 0x2002FA710 | `game_world_core::snap_input_impl<player_input>` | PROVEN (string) |
| 0x2002F6320 | `game_world_core::move` | PROVEN (string 0x200989BD0) |
| 0x2002F1360 | `game_world_core::deserialize(buffer_reader&, ...)` | PROVEN (string) |
| 0x2002F1F20 | `game_world_core::insert_impl` | PROVEN (string) |
| 0x2002F1190 | `game_world_core::remote_player_delay_in_ms` | PROVEN (string 0x2009895A8) |
| 0x2002F1110 | own confirmation delay (min(own delay up to a tick, 500)) | PROVEN body, our name |
| 0x2002EF530 | `game_world_core` constructor | LIKELY |
| 0x2002EA8A0 | `pvp_match_core` constructor | LIKELY |
| 0x2002E5370 | the match cook: history sizes at 0x2002E5E72 / 0x2002E5EBF | PROVEN body |
| 0x200125000 | static match info: tick (+1083h), max remote delay (+107Ah) | PROVEN body |
| 0x200308990 | `base_player::insert` (21 hit bodies, group 0x8000<<i) | PROVEN (string) |
| 0x2003994A0 | bullet step: ray cast in the shooter's delayed slot | PROVEN body |
| 0x20040FD40 | explosion: query in the thrower's delayed slot | PROVEN body |
| 0x2003EBB50 | sensor grenade: the same | PROVEN body |
| 0x2002356A0 | `game_world::rollback_effects` | LIKELY |
| 0x2002380A1..0x200238125 | statistic events walker (sealed or skip horizon) | PROVEN |
| 0x2001A9570 | `game_world_ui::process_statistic_event` | LIKELY |
| 0x2001A5190 | HUD marker pass (hit marker; dead kill branch) | PROVEN body |
| 0x2001AB0A0 | set `damageIcon*` on the crosshair | PROVEN body |
| 0x2002F0C80 | new statistic event (skip horizon 0) | PROVEN body |
| 0x200304842 / 0x200303FE2 | kill event (type 0) / damage event (type 3, skip 1) | PROVEN |
| 0x200AA2220 / 0x20F43FCCC / 0x200AA2208 | netsync send window 2 / receive window 0 / timer scale 1.0 | PROVEN |
| 0x200AA2204 / 0x200AA21FC | `enemy_hit_mark_timeout_in_ms` 200 / `kill_confirmation_timeout_in_ms` 1000 | PROVEN |

Core fields (0.69 `game_world_core`, which is the match core + 230h): +180A94h grid
origin (one tick), +180AA0h current time, +180AA4h rollback point, +180AC4h last own
update sent, +180AC8h last resync, +180ACCh max remote delay, +180AD8h owner place,
+180AD9h is server, +180ADAh no-horizon mode, +17C2A0h places (B8h each: +4Bh delay
mode, +5Ch base delay). PROVEN from the uses above.

### 6.2 0.26g rows added to the re-kit (`tools/re-kit/v026g/inferred.tsv`)

| VA | Name |
|---|---|
| 0x001FB660 | `survarium::game_world::process_world_events` (sealed = max ticked - horizon; calls on_world_event) |
| 0x001FB6C0 | `survarium::game_world::process_kill_cross_events` (sealed = EDX; -1 = max ticked - horizon) |
| 0x00233770 | `survarium::game_world_ui::on_kill_cross_event` |
| 0x0023CF20 | `survarium::game_world_ui::crosshair_enemy_hit_killed` |

Data: game_world+560h the kill-cross delay (0 stock; = ui+364h, zeroed at 0x2338CD,
read by `root.set_game_stats` at 0x23D12F), game_world+564h the last kill-cross time,
0xA70EC6 `show_enemy_killed_marker`, 0xA74818 `enemy_hit_mark_timeout_in_ms` (200).

---------------------------------------------------------------------------

## 7. Open questions

1. The retail tick length and the max remote delay: server values, not in the client
   (2.2). A 0.69 replay or a capture would settle them.
2. Whether 0.69 drew remote players delayed (interpolated) to match its lag
   compensation, or predicted as 0.26g does. No render path reading
   `remote_player_delay_in_ms` was found besides the kill camera.
3. The exact meaning of the 0xE0 bytes (per-place hashes or something coarser).
4. 0.52 (no exe in the survey) would date the 0.69 protocol more closely than "after
   0.46c2".
