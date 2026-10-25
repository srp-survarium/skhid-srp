# Porting 0.69's netcode to 0.26g and srp: feasibility and a phased plan

The owner, 2026-10-25: 20 ms ticks would be "a big improvement to bring"; then "can we port
the whole 0.69 system?"; then, on porting it, "how, rewritten in Rust?". This note is the
feasibility study and the plan. **Nothing is built yet.** It answers, piece by piece, what
0.69's netcode would need in our exe and in srp, what it costs, what players get from it,
and in which order to do it, each phase shippable alone.

**Sources.** `netcode_069.md` (PR #468, the 0.69d0 study: 0.69 addresses are from there
unless marked new here); `sync_core.md` and the `sync-core` crate (PR #452, branch
`feat/sync-core`); `match.md`, `transport.md`, `static_info.md`, `replay_sur.md`; the
original 0.26g exe (`identity.json`), read again for this note with capstone and a Ghidra
12 analysis; the 0.69d0 x64 exe and its Ghidra project of #468; srp `main` 54945dcb; the
local branch `fix/kill-marker-confirmed` (patch 73, not pushed when this was written);
patch 71's frame traces (section 4).

Tags as in the other notes: **PROVEN** (read in the code at the address given),
**LIKELY**, **GUESS**, **MEASURED** (from a trace), **ESTIMATE** (a model of the
measurements). 0.26g addresses are the re-kit's VAs; 0.69 ones have base 0x200000000.
Every diagram is 7-bit ASCII.

Contents:
1. Short answers and the recommendation
2. The phases
3. The eight pieces side by side
4. What a tick costs today (patch 71 traces)
5. The pieces, one by one
6. The tick length: where 50 ms lives, what 20 ms changes, options (a) (b) (c)
7. Lag compensation in 0.26g
8. The code: shared crate, client arena, server; the state machines
9. Test plan
10. Risks
11. Re-kit rows added
12. Open questions

---------------------------------------------------------------------------

## 1. Short answers and the recommendation

1. **Yes, 0.69's netcode can be ported, as a re-implementation in Rust.** 0.69 kept the
   0.26g simulation (the same `game_world_core`, history ring, change_input, move, discard,
   insert_impl; `netcode_069.md` 2.1). What 0.69 rebuilt is the **network layer around it**:
   per-tick messages both ways, the server as the clock of every input, client-side clock
   steering, ping as world state and lag-compensated hits. All of that sits on entry points
   0.26g already has (change_input 0x1FAB50, move_input 0x27D2D0, discard_input 0x27D820,
   enter_match 0x2D1470, leave_match 0x2D18A0, floating_timer::set_offset 0x1EE9E0). So the
   client side is mostly **new message handling in `no_std` Rust that calls the game's own
   functions**, with thin asm hooks; the server side is srp's relay layer rewritten around
   a per-tick close. 0.69's x64 code is the spec, not a source: nothing is copied.
2. **The 20 ms tick is the least attractive piece in this engine.** MEASURED on the
   owner's live trace: a simulation step costs ~650 us, a world capture ~250 us, a load
   ~470 us, and the step cost is almost all fixed (per player, per bullet), not per ms.
   At 20 ms the same play needs ~2.5x the grid steps and captures and ~2x the re-simulated
   steps: **+40% to +65% simulation CPU** (ESTIMATE, section 4), on the logic thread that
   already makes the hitches the owner ranks first. The player-visible gains are small:
   ~15 ms less average delay of a remote player's non-precise input (movement, aim), and
   finer positions; shots are already exact to the ms (precise inputs), and desync
   detection is bound by the 500 ms horizon, not the tick. Finer history does **not** make
   rollbacks cheaper here: the restore point is on average 15 ms closer, but the same late
   input then re-simulates ~2x as many steps. Retail could afford 20 ms on an x64 engine
   two years newer; we would pay it on 2015 code.
3. **What players would feel most, in order:** (a) no wrong kill crosses or hit markers
   (patch 73, built locally, plus an exact "complete up to T" bound to make them prompt);
   (b) fewer own-input moves and drops, so fewer self-rollbacks and resyncs, from steering
   each client's clock to the server's input deadline; (c) rollback depth bounded by each
   player's own round trip instead of the sum of two players' latencies, from 0.69's
   per-tick update (a high-ping player stops shaking everyone else's world);
   (d) lag-compensated hits (a hit tested against where the target was, not undone by the
   target's late input). 20 ms comes last, if at all.
4. **Recommendation:** port 0.69's protocol in five phases (section 2), each behind a server
   switch where the client keeps both paths for one release cycle of testing, then the old
   path removed (the one-supported-client rule). **Make the tick length a server value
   (option (b)) but ship it at 50 ms**, and try 25/20 only after the CPU work of the sync
   core (patch 74 pacing, stage (b) cheaper loads) has landed and the trace says the logic
   thread has room on a weak PC. Take 0.69's cheaper wins (option (c)) first: they are
   phases 1 to 3.
5. **The 0.69 server simulated the world** (LIKELY to near-PROVEN: its core has a server
   branch in shared code, a 22-capture history, and the 0xE0 summary is a CRC per place of
   the server's own world). srp does not, and **none of phases 1 to 5 needs it**: 0.69's
   protocol works with a server that only orders inputs, because every client still
   simulates the same world. A simulating srp would mean running the game itself on the
   server (section 5.8); not planned.

---------------------------------------------------------------------------

## 2. The phases

Each phase ships alone, changes no world bytes unless it says so, and has a server switch
where the client can carry both paths. "Client Rust" means `no_std` Rust in the patch arena
(the sync-core toolchain of #452), "server Rust" srp, "shared" the `sync-core` crate used by
both, "asm" a NASM stub or an in-place byte change.

```
 phase 0  groundwork          #452 merged; server enums (sync_core.md 8); one Grid type
    |                         (tick, origin) instead of 7 TICK_MS constants; no change
    v
 phase 1  cheap wins          1a sealed kill/hit feedback (patch 73)
    |                         1b server clock steering to the input deadline (srp only)
    |                         1c "complete up to T": a client heartbeat + a server bound
    v
 phase 2  per-tick protocol   client update every tick (0.69 0xCD), server update every tick
    |                         with all inputs + own-input confirmations (0.69 0xDF);
    |                         replaces relays, move, discard, entered, left
    v
 phase 3  client clock        the client steers its own clock from the confirmations
    |                         (netsync_send_window); latency measured per update
    v
 phase 4  tick from server    the tick a static-info value; shipped at 50; 25/20 tried under
    |                         a switch when the CPU budget allows
    v
 phase 5  lag compensation    a root-transform ring per player, hits tested at t - d;
                              d a match constant first, then a ping input per player
 (optional, after 2)          per-place state summary and client-asked resync (0.69 0xE0)
 (not planned)                a simulating server
```

| Phase | What players get | Client Rust | Server Rust | Shared | asm | World bytes | Switch | Size | Risk |
|---|---|---|---|---|---|---|---|---|---|
| 0 | nothing (groundwork) | - | the enum refactor; `Grid` everywhere | `Grid`, `Tick`, `TickLen` | - | no | - | M (server) | low |
| 1a | no wrong red cross; hit markers only for kept hits | - (patch 73 is asm today; may move to Rust with 1c) | - | - | patch 73 | no | client only | S (built) | low |
| 1b | fewer own inputs moved/dropped, fewer resyncs after a lag | - | 0xE7 aimed at the input deadline | `clock::Steering` | - | no | `SRP_CLOCK_STEER` | S-M | medium (oscillation) |
| 1c | kill cross ~RTT after the kill instead of 500 ms | heartbeat send, bound read | `heard_to`, `complete_to` | `wire::{Heartbeat, Complete}` | 2 hooks, id tables | no | `SRP_COMPLETE_TO` | M | low-medium |
| 2 | rollbacks bounded by own RTT; no moved-input divergence; exact seal | update send + handler (confirm/move/discard) | the tick close, per-client updates, replay of updates after a load | `wire::{ClientUpdate, ServerUpdate}`, `confirm::OwnInputs` | dispatcher range, send hook | no | `SRP_TICK_UPDATES` | L | medium-high (protocol) |
| 3 | inputs arrive just in time: fewer late inputs anywhere | steering from confirmations | stop 0xE7 under the switch | `clock::Steering` (same as 1b) | - | no | with phase 2 | M | medium |
| 4 | (at 20 ms) ~15 ms less remote-input delay; finer positions | read the tick from 0xE4, re-stamp the 102 sites (6.1) | `Grid` per match, reach windows, bots' step | `TickLen` | the site table, the 0xE4 trailer hook | yes at another tick | per match | M-L | high (CPU, desync on a missed site) |
| 5 | hits not undone by the target's late input | ring writer, swap around queries | delay in static info; ping input later | `lagcomp::Ring` | 3-4 hooks | **yes** | revision + match option | M-L | high (simulation) |

---------------------------------------------------------------------------

## 3. The eight pieces side by side

| # | Piece (0.69) | 0.26g today | Needs a world change | Needs the server | Depends on |
|---|---|---|---|---|---|
| 1 | Client update every tick, empty or not (C->S 0xCD) | inputs only on change (0xDC/0xDD/0xDE), resent 500 ms | no | parse, `heard_to` | - |
| 2 | Server update every tick: all places' inputs, entries, leaves, own-input confirmations (S->C 0xDF) | a relay per input (0xEB/0xEC), move/discard own (0xEF..0xF2, 0xF4/0xF5), kit relay 0xF3, entered/left 0xE9/0xEA | no | the tick close, per-client updates | 1 (to close early; without it only by deadline) |
| 3 | Client clock steering (`netsync_send_window_in_ms` 2) | the server sets (0xE5) and corrects (0xE7) the clock | no | none (stops 0xE7) | 2 (server time per own input) |
| 4 | Tick length from the static info (LIKELY 20 ms) | 50 ms, compiled in | yes (a different simulation) | `Grid` per match | 0 |
| 5 | Lag compensation: `ping_input`, hit bodies per past tick, hits at `t - delay(shooter)` | none; hits test the targets' current pose | **yes** | delay in static info (ping input later) | 3 for the ping |
| 6 | Sealed kill feedback (kill events taken only when sealed) | the walker's bound is 0 (cross at once); patch 73 makes it the 500 ms horizon | no | none (exact with 1c/2) | - |
| 7 | Per-tick state summary (S->C 0xE0, a CRC byte per place) and client-asked resync | the server judges 0xDF hashes and resyncs | no | a summary from the clients' reports | 2 |
| 8 | The server simulates | srp trusts the majority of witness uploads | - | a game per match | - |

---------------------------------------------------------------------------

## 4. What a tick costs today (patch 71 traces)

MEASURED from patch 71's `srp_frames.csv`:
- **live**: the owner's game, revision 33, 481.7 s in play at 100 fps (49,212 frames);
- **local**: two local clients, revision 34, ~30 s of play each, no shots (the local
  test of 2026-10-25).

| Per second of play (live) | Value |
|---|---|
| simulation (move: loads, steps, captures) | 245 ms, 24.5% of one core |
| logic thread total | 358 ms |
| steps (atomic_tick) | 239.7, of them re-simulated 121.5 |
| world loads (deserialize) | 99.3 (one a frame: the render move, `sync_core.md` 1.3) |
| frames with a re-simulation | 60.7 |

| Per call (live) | Cost |
|---|---|
| one step (least squares over 48,187 frames: steps, captures, calls) | ~650 us (fit 644) |
| one capture (serialize at a grid point) | ~250 us |
| one world load | ~470 us (median 452) |
| simulation per frame | p50 2.27 ms, p90 3.87, p99 6.71, max 37.3 |

Re-simulated steps per frame: 0: 18,959 frames, 1: 10,375, 2: 12,613, 3: 3,677, 4: 1,532,
5: 647, 6: 231, 7: 98, 8: 34, 9 and more: 21.

The local trace (two players, no shots) has ~100-120 us a step and ~80 us a load: **the
step cost grows with players and bullets, not with the step's length.** In the live
trace a frame's one partial step (any length up to 50 ms) costs about what a full 50 ms
grid step does (median 848 us against ~660 us for the grid step plus its capture's share):
the fixed part dominates. Only the physics (fixed 1/60 s substeps, section 6.1) is
proportional to dt.

**A model** (ESTIMATE): the same frames, the same late inputs (the same game time to
re-simulate per rollback, the restore gap tick/2), the same rollbacks a second (move runs
once a frame), a step's cost fixed:

| Tick | Steps/s (re-simulated) | Captures/s | Simulation ms/s | Per frame at 100 fps |
|---|---|---|---|---|
| 50 (today; the model gives 239 for a measured 245) | 242 (122) | 142 | 239 | 2.39 ms |
| 25 | 353 (213) | 253 | 339 | 3.39 ms |
| 20 | 408 (258) | 308 | 389 | 3.89 ms |

- With 30% of a step proportional to dt (an upper bound for the physics share), 20 ms
  costs +43% instead of +63%.
- With patch 74's pacing (-50% re-simulated steps in its model, `sync_core.md` 9), 20 ms
  costs ~273 ms/s: **today's cost +14%**. Pacing first, then a shorter tick, is the only
  order that does not make hitches worse.
- A weak PC (2.5-3x slower per step) spends ~60-75% of a core on the simulation at 50 ms
  in a fight; at 20 ms without pacing it would need more than a whole core, and a logic
  thread that cannot keep up falls behind, which makes more late inputs and more
  re-simulation (the "rollbacks 45, ticks 87" spiral of `sync_core.md` 2).

**Memory.** A history item is 0xC0BCC bytes (789,452). 11 items (500 ms at 50) are 8.7
MB; 26 (500 ms at 20) 20.5 MB; 51 (1000 ms at 20, the deeper ring of sync-core stage (d))
40 MB; 0.69's 47 items of 0x106268 bytes were 50 MB. The exe is large-address-aware
(PROVEN, PE characteristics 0x122), so this fits the address space; the pool's allocator is
in 6.1.

---------------------------------------------------------------------------

## 5. The pieces, one by one

### 5.1 Piece 1: a client update every tick

- **0.69** (`netcode_069.md` 2.3): every frame, for each tick from the last sent to the
  local now, the client snaps its own inputs of that tick and sends one
  `client_game_update_message` (0xCD): `u32 tick`, counts, each player input as a byte back
  from the tick (its precise time) and its payload, kit (profile) inputs, ping inputs. An
  empty update says "nothing changed up to t". Resends are the transport's.
- **Exe:** a send pass each frame (a CALLEE in `try_send_all` 0x27CFB0, or at on_idle's call
  of it) that walks the local place's input list (core +2FCCh + place*480h) for the ticks
  since the last update. The own inputs are already in the lists (local change_input) and
  snap_inputs 0x2D2E10 already puts off-grid ones on the grid; the update only reads them.
  A new C->S id: the ten C->S ids 0xD8..0xE1 are all used and fixed-length
  (`transport.md` 4); the range bytes (getter+6/+7, written at 0x2792D4 and 0x27A15B) and
  the info table 0x279AE0 must grow by one variable-length id (0xE2), or the update rides
  the 0xE0 carrier with a kind byte as the snapshot chunks do (`client_patch.md` 3.2).
  The id route is cleaner; the table is ours to relocate into the arena.
- **srp:** parse; per place `heard_to` (the newest tick whose update arrived, with every one
  before it).
- **Port:** re-implemented. The layout and the rule ("one a tick, from last sent to now, at
  most max delay back") are 0.69's; the code is ours.
- **Depends on:** nothing. **Size:** client ~200 lines Rust + a hook; server ~150 lines.
- **Risk:** no world change. Traffic: 20 updates/s at 50 ms (50 at 20 ms), ~8-30 bytes
  each with the 3-byte transport header: 0.6-1.5 KB/s up per client. The transport's u8
  sequence numbers have a half range of 128 messages: 6.4 s at 20/s, 2.6 s at 50/s, enough
  for its 500 ms resends.
- **Alone, players get:** nothing they see. It is the base of 1c and 2.
- **Parts:** client Rust (walk, encode, send through the game's packet sender), shared
  (`wire::ClientUpdate`), asm (the hook, the id range bytes), server Rust (parse).

**Phase 1c, "complete up to T", is piece 1's smallest form** with a server answer, before
the whole of phase 2 (`netcode_069.md` 5.2):
- the client sends `{u32 t}` each tick: its own inputs up to t are all sent (the newest
  ticked time whose own inputs went out);
- the server keeps `heard_to` per place (t, once every input of that place up to t has
  arrived) and sends each client `complete_to = min(heard_to over the other Live places,
  the server's own finality clock - 250, the earliest relay to this client not yet acked by
  the transport, minus one)`, when it rises (send-once, the newest wins). **The ack term is
  what makes it exact under an unordered transport:** a relay still in flight cannot be
  overtaken by a T that covers it, because the server knows from the acks which relays the
  client holds;
- the client stores T (a field the kill walker of patch 73 reads: section 5.6) and, in
  tests, checks that no insert ever lands at or before T.
- A place that stops sending holds T for everyone: T skips places not Live, and the bound
  falls back to the 500 ms horizon when T is stale. Ids: one C->S, one S->C (fixed
  lengths 4 and 4), the same table work as above.

### 5.2 Piece 2: a server update every tick, with own-input confirmations

- **0.69** (`netcode_069.md` 2.4, 2.9): the server closes tick U and sends each client one
  update: `u32 U`, entered and left masks, each place's changed player inputs at U (a byte
  back each: the precise time), the "empty input" mask, kit, ping and AI inputs, and
  `server_input_confirmation {u32 own tick, u32 server time}` for this client's inputs. The
  client: drops a U it has seen (a ring of 257); closes its own ticks up to U; for each own
  input waiting: confirmed at t: final; confirmed at U != t: **moved to U**
  (0x2001B24D0); not confirmed and the first tick without an update is more than 500 ms
  after t: **discarded** (0x2001B2880); then entries, leaves and change_input for every
  remote input. **Remote inputs reach a client only inside the update that closes their
  tick**; the server is the clock of every input.
- **Why it matters (the timing).** Today a relay goes out the moment the server gets the
  input: client A's input at t reaches B at t + lat(A) + lat(B), and B rolls back by the sum
  of both one-way latencies (LIKELY: srp steers every clock to its own, so A and B are at
  about the same time). With 0.69's model A runs ahead by lat(A) (phase 3), its input
  for t reaches the server just before the server closes t, the server sends the update at
  t, and B, ahead by lat(B), gets it at its local t + 2 lat(B): **B's rollback depth is its
  own round trip, whatever A's ping.** The average depth is about the same; the spread is
  fairer: a 200 ms player no longer shakes every other player's world by 200 ms, only his
  own. And because the server decides where a late input lands, every client moves it to
  the same tick by construction (today's move/discard messages, made explicit).
- **Exe:** a new S->C id (variable length; the range [0xE3, 0xF6] at getter+4/+5, 0x2792CC
  and 0x27A14F, grows to 0xF7; the dispatcher's jump table 0x27A738 (20 entries) and the
  length and info tables 0x2799E0 / 0x279BF0 move to the arena with one more entry). The
  handler is Rust: it calls the game's own `change_input` 0x1FAB50 (from network 1),
  `enter_match` 0x2D1470, `leave_match` 0x2D18A0, `move_input` 0x27D2D0 / 0x27DCD0,
  `discard_input` 0x27D820 / 0x27E200, through engine thunks. The game's insert_impl then
  lowers the rollback point as today: **the simulation and the history are untouched.**
  The own-input bookkeeping (pending, confirmed, moved, discarded) is ours, beside the
  game's lists.
- **srp:** the largest change of the port. Today srp keeps one flat list of relays
  (`Game.inputs`), decides each input when it arrives (`place`, the 250 ms horizon), and has
  no per-tick close (survey of game.rs, section 6.2). Needed:
  - a per-match tick loop: tick U closes when every live place's `heard_to >= U` (piece 1)
    or at a deadline (`U + deadline`, the server's clock); a place that is not Live does
    not hold it;
  - per place a timeline `BTreeMap<Tick, Input>` of the open ticks; an input that comes
    after its tick closed is re-timed to the open tick (today's `place` move rule, now one
    rule for everyone);
  - one update per client per tick: all places' changes at U plus that client's
    confirmations; a ring of the last ~2 s of closed updates;
  - the late join and the resync: after a world at S, the server sends the closed updates
    after S (today: the replay of relays, `send_replay`, `roster_since`); this is simpler,
    since the updates are the whole timeline;
  - the recorder keeps working (datagrams); the .sur converter learns the update.
- **Port:** the layout (parser 0x20032E4A0) and the processing rules (0x2001AFCB0) are the
  spec; re-implemented on both sides.
- **Depends on:** piece 1 to close a tick before its deadline (without it every tick waits
  for the deadline: a fixed input delay for remote players).
- **Size:** client Rust ~600-900 lines + the dispatcher change; server: a new module of
  ~1500-2500 lines replacing the relay paths of game.rs (relay, relay_to_others,
  answer_owner, send_drop_order, roster_change, the held relays, the replay), and its tests.
- **Risk:** medium-high for the protocol (every input path changes on both sides),
  low for desyncs if the server is right (every client applies the same input at the same
  tick, as today), but the late join and resync ordering must be redone and proven. Traffic
  down: 20 (or 50) updates/s per client, 18 bytes plus the inputs: ~1-3 KB/s per client.
- **Alone, players get:** fairer rollbacks (above), no "moved input" divergence, an exact
  seal for 1a, and an end to the C1-C4 classes of `sync_core.md` 3.4 that come from
  answers and relays racing a resync.
- **Parts:** client Rust (handler, own-input machine), shared (`wire::ServerUpdate`,
  `confirm::OwnInputs` with the move/discard rule, so server and client agree on it by
  construction), asm (id range, tables, dispatcher entry stub), server Rust (tick close,
  timelines, updates).

### 5.3 Piece 3: client clock steering

- **0.69** (`netcode_069.md` 2.6): each confirmation gives a margin `server_time - t +
  send_window` (2 ms); a 257-sample ring, its windowed minimum (0x2001B1AC0) drives the
  floating timer: a gradual correction (0x20011A000) or a set on a big error
  (0x20011A970). The client runs ahead of the server by its one-way latency plus 2 ms.
- **0.26g today:** the server sets the clock (0xE5) and corrects it (0xE7 ->
  `floating_timer::set_offset` 0x1EE9E0; patch 15 makes it take a correction 6x faster).
  srp aims the clock at its own estimate of the client's lag (`sample_lag`,
  `correct_clock`, game.rs), not at the input deadline.
- **Two versions:**
  - **1b, server only:** srp already sees how late each input arrives relative to where it
    must land (`place`'s move rule is that case). Aim 0xE7 at "inputs arrive N ms before
    their tick closes" using a windowed minimum of those margins, a dead band, and the
    existing 0xE7. No client change.
  - **3, client:** the 0.69 rule in Rust from phase 2's confirmations, calling the game's
    set_offset; srp stops sending 0xE7 under the switch.
- **Depends on:** 1b nothing; 3 on piece 2. **Size:** S-M each. **Risk:** oscillation (a
  clock that jumps makes rollbacks of its own); 0.69's windowed minimum and gradual
  correction are the known cure.
- **Alone, players get:** fewer late own inputs (moved or dropped by the server today, each
  a self-rollback or, past the history, a resync: the hitch -> late input -> resync chain of
  `sync_core.md` 4).
- **Parts:** shared `clock::Steering` (margins ring, windowed minimum, `Steady |
  Correcting{offset}`), used by srp in 1b and by the client in 3; client Rust glue to
  set_offset; server Rust.

### 5.4 Piece 4: the tick length

Section 6.

### 5.5 Piece 5: lag compensation

Section 7.

### 5.6 Piece 6: sealed kill feedback

- **0.69:** a kill event is taken by the HUD only when sealed: older than the newest ticked
  time minus the local player's confirmation delay (its latency rounded up to a tick, at
  most 500 ms). Hit markers stay predicted.
- **0.26g:** the kill-cross walker 0x1FB6C0 gets `max ticked - [game_world+560h]`, a field
  nothing writes (0): the cross shows when the kill is first simulated (`netcode_069.md`
  3.2).
- **Built (patch 73, local branch `fix/kill-marker-confirmed`):** the walker gets the event
  horizon (500 ms) as the kill feed's walker does (0x1F7FDA); the per-frame draw of the
  predicted hit collector is gone (0x247467); each hit's world event carries the shooter's
  sum, and the kill walker's handler draws confirmed hits too. So both markers show only
  what every game agrees on, 500 ms late.
- **What the port adds:** an exact bound instead of 500 ms. With 1c (the server's
  `complete_to` T) or phase 2 (the closed U), an event at or before T is final on every
  client: the cross and the marker can come ~RTT + a tick after the kill instead of 500 ms.
  The walker's argument becomes `min(max ticked - 500, T)` read from the field 1c writes.
- **Parts:** asm today (patch 73); the bound read from a field the Rust of 1c/2 writes.

### 5.7 Piece 7: the state summary and a client-asked resync

- **0.69** (new here, PROVEN): 0xE0 carries 3 presence-mask bytes, one byte per present
  place, then 3 global bytes; each place byte is the low byte of a CRC-16/ARC (table
  0x20F43DFB0) over that place's serialized state in the capture (producer 0x2001AE340 ->
  0x2002FC2B0). The client compares with its own (0x2001AE1B0) and asks for a world itself.
- **srp has no world** to make a summary from; it has the clients' 0xDF hashes (the CRC of
  the whole event-horizon item) and judges them by majority. The useful part of 0.69's
  summary for us is the **per-place** split: a client could add a CRC byte per place to its
  hash report, and the server would know which place's state diverged (today: only that
  the world did). A client-asked resync saves the server's judging delay (1 s) at most.
- **Depends on:** nothing strictly; best after phase 2. **Size:** S-M. **Gain:** diagnosis
  (which player's state goes wrong first) more than play. **Not in the main line.**

### 5.8 Piece 8: a simulating server

- **0.69** (LIKELY to near-PROVEN, new here): no server code in the client exe (no
  "dedicated", no server exe name, no `game_server` source path), but `m_is_server`
  (core+180AD9h) has 34 uses in shared `game_world_core` code: change_input's refusal of
  inputs too new and re-timing of late ones (player 0x2001B3110, profile 0x200138310, AI
  0x2001B41B0, ping 0x2001B50B0), enter/leave's correction of a time too old, the horizon
  helpers 0x2002F10D0 / 0x2002F1110, `synchronize_impl` 0x2001B7450 (LIKELY the update
  builder shared with the server); a 22-capture server history; the 0xE0 summary is a CRC
  of a world. So the retail server ran the same simulation.
- **For srp** that would mean running the game's simulation on the server host: either
  the 0.26g exe itself, headless under Wine, one per match, in its (never run) server mode
  (`pvp_match_core` with the server flag: horizon 250, one history item), or a rewrite of
  the simulation in Rust (not feasible: it is the whole game, Bullet included, bit-exact).
  The first is a project of its own (a renderer-free run, resources on the server, CPU per
  match, Wine on the live host) whose gain is authority (no trusted uploads, a summary to
  compare against), not feel. **Not planned;** phases 1-5 do not need it.

---------------------------------------------------------------------------

## 6. The tick length

### 6.1 Where 50 ms lives in 0.26g

Read again for this note (PROVEN unless tagged; Ghidra decompiles and every immediate 50,
`0x51EB851F`, 0.05f, 1/50 and 1/60 in the simulation, network and UI code accounted for).

**The arithmetic.** Every division by 50 in these paths is unsigned
`mov eax,51EB851Fh ; mul r/m32 ; ... ; shr reg,4`. A division by 20 is `0xCCCCCCCD` with
the same `shr 4` (exact for every u32): **an imm32 swap**; by 25 is `51EB851Fh` with
`shr 3`: the shift byte too. The other forms are `imul r,r,32h`, `add r,32h`, `lea
[..-32h]` (imm8 / disp8), five `mov r,32h ; div r` and two origin stores (imm32). No
signed division by 50.

| Site (VA) | What | Class | At 20 ms |
|---|---|---|---|
| 0x2D00CD (imm 0x2D00D3); 0x2D0765 (imm 0x2D0766) | the grid origin 50: the core's constructor (+18FBCh) and `clear` 0x2D0550 (origin, now and max) | GRID | the origin a multiple of the tick (note) |
| 0x2D1E21, 0x2D1E2B | atomic_tick's "on the grid": `t - (t/50)*50 == 0`, absolute | GRID | 2 |
| 0x2D2272, 0x2D2282 | tick_events' next boundary `origin + ((cur-origin)/50+1)*50` | GRID | 2 |
| 0x2D1303, 0x2D130F | insert_impl: rollback point = the grid point strictly before t | GRID | 2 |
| 0x2D15F6, 0x2D1603, 0x2D1A26, 0x2D1A33 | enter/leave_match's "CORRECTED" time (next grid after max) | GRID | 4 |
| 0x2D4EEE, 0x2D4EFB, 0x2D502E, 0x2D503B | snap_input (player 0x2D4E40, kit 0x2D4F80) | GRID | 4 |
| 0x1FAD53, 0x1FAD69, 0x1FAD7F, 0x1FAD89, 0x1FAFDB, 0x1FAFE7, 0x1FAFF3, 0x1FB003, 0x1FB2D5, 0x1FB3B3 | change_input (player) 0x1FAB50: the lead in ticks (+18FB8h x 50), valid + one tick, `t % 50` kept on a move, the "5 x horizon" rule, the precise test, the unsnapped own input | GRID | 12 |
| 0x1FB932, 0x1FB946, 0x1FB95C, 0x1FB966, 0x1FBBB8, 0x1FBBC4, 0x1FBBD0, 0x1FBBE0, 0x1FBE3D | the same in change_input (kit) 0x1FB730 | GRID | 9 |
| 0x1FDCC9..0x1FDCE9, 0x1FDCF9..0x1FDD1B | `fixed_time_before` 0x1FDCC0, `fixed_time_at_or_after` 0x1FDCF0 (origin-relative grid helpers) | GRID | 8 |
| 0x27EAAE, 0x27EC4E | own-input send: an off-grid node not flagged precise is not sent | GRID | 2 |
| 0x27D075, 0x27D081 | on_idle's horizon move `origin + ((now-500-origin)/50)*50` | RING | 2 |
| 0x27CC21, 0x27CC2E | `send_match_messages` 0x27CBF0: now floored to 50 for the 0xDF / 0xE1 timers | CADENCE | 2 |
| 0x27CC4F (imm 0x27CC53) | the hash interval, 100 (patch 3: 50); sent when `last + interval <= floor(now)` | CADENCE | 20 (or 40, 60, 100); 50 would fire at uneven 40/60 ms |
| 9 inline readers in the 0xEB..0xF5 handlers (0x27C39A/C3CA, 0x27C620/C65A, 0x27C6C5/C6FB, 0x27C818/C864, 0x27C852/C88E, 0x27C910/C945, 0x27CA98/CAE4, 0x27CAD2/CB0E, 0x27CB83/CBC6); 0x2F696D/0x2F699A (`read_message_time` 0x2F6940); readers 0x318400, 0x318540, 0x318690; writers 0x3184A0, 0x3185A0, 0x3185F0, 0x318620 | the 1-byte wire tick `(cur + sdiff8(b, cur & FF)) * 50` against core +18FC0h (`cyclic_difference` 0x20FF90); the precise byte `t % 50` | WIRE | 32; the reach falls from +-6.4 s to +-2.56 s |
| 0x2CC007 (multiplier 0x2CC012) | the ring: `(max_ping 250 + 250)/50 + 1` = 11 items of 0xC0BCC; 128 with `-debug_hash_mismatch`, 1 on a server (`pvp_match_core_cook::on_resources_loaded` 0x2CBC60) | RING | 1 (26 items) |
| 0x2D5699, 0x2D56A4; 0x2D00C7 | the horizon 250 + 250 = 500 ms | RING | nothing (ms) |
| 0x1EDEFC/F08, 0x1EE198/1A2, 0x1EE268/278; 0x218C4D/C58 | the replay player 0x1EDEF0 (`(t/50+1)*50`, precise = `t % 50 != 0`); `replay_network_client::tick` 0x218BF0 | GRID | 8 |
| 0x1F7A57/A63, 0x1F7ABA, 0x1F7AD6; 0x1F7B0A (disp 0x1F7B0C) | the interpolated view (`interpolated_view_at` 0x1F76B0): grid points either side of the clock, the fraction `x 0.02f` (1/50, 0xA2DB34) | GRID | 4 + the disp pointed at a 0.05f (0x9CC878); 0xA2DB34 itself is also a 0.02 m hand offset (0x309F8F): never edit it |
| 0x316B8B, 0x316B9A; 0x3169D8 (imm 0x3169DF) | the respawn deadline snapped to the grid (on_event 0x316AA0; the tick 0x316820 respawns when `deadline == time`); no spawn point: retry `+ 50` | GRID, TICKCOUNT | 3 |
| 0x30A153, 0x30A167 | a grenade's throw time snapped to the grid (`throw_grenade` 0x30A120) | GRID | 2 |
| 0x31BD91, 0x31BD9B | an on-grid flag for `damage_zone_core::tick` 0x319A70, which ignores it (LIKELY) | GRID | 2 (harmless) |
| 0x75610, 0x7562C; 0x7B48D (disp 0x7B491); 0x7E1C5 (disp 0x7E1C9) | the physics' 1/60 (0xA2DBB4, shared with weapon_core_cook: never edit it); the character controller's nominal step 0.05 (+2C704h, patch 72 reads it); step_down's grounded probe 0.05 m | PHYS | optional (below) |

**Totals:** a fixed 20 ms is **102 operand swaps, ~240 bytes** (46 magics, 48 imm8, 7 imm32,
1 disp32), **no instruction rewritten**, provided the grid origin becomes a multiple of
the tick. **Note on the origin:** atomic_tick, the wire encoding, the interpolated view,
the replay and the `t % 50` tests use the absolute grid; tick_events and the helpers the
origin-relative one; they agree today only because the origin 50 is a multiple of 50.
With 20 ms and origin 50, the grid would be 50, 70, 90 and the wire and on-grid tests
break; origin = the tick (20, 25 or 50) keeps every site a byte swap.

**Not tick-related (checked):** the /1000 sites (log formatting), /400 at 0x2D41D7
(`fixed_history` items), 1/60 at 0x30B336 (weapon-cook frames), the 50 ms damage-sound
throttle 0x23FBF0, the countdown digits 0x267F7B, and the other 50.0/20.0/0.05 constants
(0x245DF9, 0x23DD9B, 0x332A99, 0x237550, 0x249BA1, 0x849FE). move_input 0x27D2D0 and
discard_input 0x27D820 have no 50 (ms). The 0x20FF90 callers in the transport are
sequence numbers.

**The physics at 20 ms.** `tick_physics_world` 0x2D1D00 calls `bullet_physics_world::tick`
0x75550 with the step's own dt (not a smoothed frame delta as in the 0.10b source). It
calls stepSimulation twice: `(0, 0, 1/60)` (which zeroes the accumulated local time: the
remainder is thrown away every call) and `(dt x 0.001, 3, 1/60)`; updateActions then runs
once with the whole dt (the character controller). So a 50 ms step is 3 rigid-body
substeps (50 ms of physics), a 20 ms step **1 substep (16.7 ms): rigid bodies (grenades,
batteries, bodies) would run at 5/6 speed**. A split tick loses time in both cases. The fix
is to point 0x75612 / 0x7562E at a 0.02f (one exact substep a tick). The controller skips
a step only when `dt == 0.05` and the body has not moved, and scales its move by
`dt / 0.05`; stock step_down takes gravity x 0.05 per call (2.5x gravity at 20 ms), which
patch 72's `min(dt, nominal)` already fixes: **patch 72 must stay**. Whether the grounded
probe (0x7E1C5) and the animation root motion are per 0.05 s is a GUESS to check.

**What counts ticks, and what runs on grid ticks only.** Rules, artefacts, anomalies,
medkits, oxygen, stamina, breath, the player's tick and animations, weapons, the effect
player, grenades and the spawner use dt or ms (the timelimit 0x318100, generic anomaly
0x31B5B0, damage zone 0x319A70, artefacts 0x313200 / 0x311890 / 0x312170 / 0x3127E0,
medkit 0x33D790, oxygen 0x33E5D0, stamina 0x329790, breath 0x31D040 / 0x31D0A0,
base_player::tick 0x2DAA60, tick_animations 0x2D8EE0, weapon_core::tick 0x2E09E0, effect
player 0x2D1CE0 -> 0x2DD8A0, grenade_core::tick 0x303A60, spawner 0x3148A0,
assign_physics_transform 0x2D93A0). Counted in ticks: change_input's lead, the respawn
retry, the respawn and throw times on the grid, the controller's nominal 0.05.
**`bullet_manager::tick` 0x321D10 runs on grid steps only** (`cmp [ebp+10h],0`): bullets are
evaluated from their start time in ms, so they stay correct, but at 20 ms 2.5x as often
(more ray tests a second, and slightly different hits: LIKELY). `booby_trap_core::tick`
0x304DB0 arms at a ms deadline but its countdown takes only the on-grid step's dt (a stock
quirk). No AI or spotting reader of the on-grid flag was found.

**Guards, UI, sound:** the "first play" tests (0x23EEC4 `+18FC4h < [obj+324h]`, 0x23F430,
0x2A6488, 0x267AD2) and the event walkers 0x1FB660 / 0x1FB6C0 (`+18FC4h - +18FB4h`) are ms:
nothing assumes 50.

**The snapshot and the hash:** world times are u32 ms (item +C0BB0h, the respawn deadlines
at +138h per place, timers); no tick reader or writer is called from the deserializers
(0x2D0B70, 0x2D2770, 0x324D80): 1-byte ticks exist only in messages (LIKELY, from the
xrefs). A world from a 50 ms match does not replay the same at 20 ms.

**The history pool:** not the heap but the match cook's fixed arena (cook 0x9261E38: +24h
the buffer 0x2261E38, +28h the size **0x7000000, 112 MiB**, written at 0x2CBBAD /
0x2CBBB7); items x 0xC0BCC + 0x3AE40 above it asserts "not enough memory for creating new
match" (0x2CC029). 26 items (20.5 MB) and 47 (37.1 MB) fit, as the 128 debug items (98.8
MB) do.

**.text is not writable at run time** (section flags 0x60000020; the one VirtualProtect
import, 0x870200, is called only by the CRT's `_resetstkoflw` 0x420858; none of our
patches calls it).

### 6.2 Where srp assumes 50 ms

From a read of srp `main` (paths under `srp-026g/`; G = `servers/match-server/src/v026g/
game.rs`, MM = `crates/survarium/src/v026g/match_msgs.rs`, B = `servers/match-server/src/
v026g/bots/`):

| Group | Where | What a 20 ms (or per-match) tick needs |
|---|---|---|
| the tick itself | MM:42 `TICK_MS = 50`; also `v026g/sur.rs:56`, `bots/anomalies.rs:47`, `bots/kin.rs:41` (`TICK_S = 0.05`), the test client `scripted.rs`, sync-core `lib.rs:18` (and the 0.21d server's own, left alone) | one `Grid`/`TickLen` from sync-core; per match: a field of `Game`, threaded through MM's API |
| wire tick | MM:185 `tick_byte`, MM:194 `tick_time` (sdiff8, +-127 ticks), MM:210-228 `WireTime`, MM:348 the 0xDD ms check (< TICK_MS) | the tick passed in; the reach shrinks from +-6.35 s to **+-2.54 s** and the byte wraps every 5.12 s |
| windows past the reach | G:279 `TICK_REACH_MS 5000`, G:522 `WAITING_REACH_MS 5500`, G:307 `INPUT_MAX_AGE_MS 5000`, G:274 `SIMULATION_STEP_MS 1000` (assumes the 12.8 s wrap), G:131 `TOO_NEW_MS 2500`, `mod.rs:361-371` entry_state clamp | re-derived as functions of the tick, all under 2.54 s; or new messages carry u32 times (phase 2 does) |
| grid | G:405 `grid_up`, G:3623/3717 snapshot on the grid, G:5527 the move rule, G:5556/5573 upload aiming, G:375 `AIM_MS = TICK_MS/2 - 3` (7 ms at 20, under the loop's 5 ms receive wait), `game/start.rs:109,210` | `Grid` methods |
| tick counts | G:205 `INPUT_RING 27` / G:226 `_SAFE 26` (27 inputs: 1.35 s at one a tick at 50, 540 ms at 20, below `RESYNC_LOAD_BY_MS 1200`), G:379 `MOVE_STEPS 4`, G:367 `HASH_TICKS_KEPT 64`, G:318 `MISSED_KEPT 64`, bots `LEAD_TICKS 2` (encode.rs:17, drive.rs:70) | each re-expressed in ms; the 27-input ring is the client's (`fixed_history<27>`): a rule of its own at 20 ms |
| literals | G:4642 `+ 500`, G:4644 `game - 50`, G:5975-5976 `.max(50)`, `WAITING_RELAY_FLUSH 50ms`, `RESYNC_WORLD_RESEND_MARGIN_MS 50`, `SRP_RELAY_PACK_MS .min(50)` (mod.rs:322), `CLOCK_TOLERANCE_MS 10` | named constants of the grid |
| client history | `RELAY_USEFUL_MS 500`, `ANSWER_REACH_MS 450`, `DROP_ORDER_LIFE 450`, `late_relay_reach` | valid as long as the client keeps 500 ms (26 items at 20 ms) |
| hashes | G:1516 `hash()` keyed by the decoded tick; `check_hash_phases` "every second tick" | 2.5x the entries; the cadence a multiple of the tick |
| bots | `bots/encode.rs` one input a grid tick, 2 ticks ahead; `kin.rs`/`motor.rs`/`body.rs`/`recoil.rs` mirror the game's step (`TICK_S`, ~50 sites with the anomalies) | lead in ms; the physics mirror follows the real step; 2.5x decisions a second |
| recordings | `.srprec` (record.rs): raw datagrams (1-byte ticks), the JSON header has no tick; the converter (sur_convert.rs:1121,1155) decodes with 50 | a `tick_ms` key in the header (missing = 50) |
| replays | `.sur` stores ms, but the game's reader is on the 50 ms grid (`(t/50+1)*50`, key frames only on the grid, `replay_sur.md` 66-72); chunk 0 has `"0.26g0", u32 8` | old replays refused, not converted: the existing revision gate (`session/src/v026g/replays.rs`, `SIMULATION_CHANGES`, simulation_changes.tsv) takes one row |
| clock | 0xE5/0xE6/0xE7/0xDB carry ms | nothing (tolerance tighter) |
| results, rules | absolute ms (`match_msgs/snapshot.rs`, rules.rs) | nothing |

Size (ESTIMATE): a fixed 20 ms server is ~40 constants and literals plus tests (~2 days); a
per-match tick ~75-90 sites in ~20 files (G, MM, bots, sur_convert, late join, start).

### 6.3 Our own patches

| Patch | 50 ms assumption | At 20 ms |
|---|---|---|
| 3 hash interval 50 | "one tick" (every tick fires when interval <= tick) | one byte <= the smallest tick; 0xDF 50/s per client, the server's hash map 2.5x |
| 4 snapshot upload | none (u32 ms, the game's horizon item) | nothing |
| 11 late join | none in the client (the "20" are places) | nothing; server entry ticks via `Grid` |
| 15 clock factor | none | nothing |
| 18, 19, 20 | ms, not ticks | nothing |
| 31 kit change before start | none (the rule's tick time) | tests step by 50 |
| 38 spotting | none (cooldown 0) | test constant |
| 53 Research | `gain = dt * rate / 4`: dt from the game, floored per tick | exact at 20 (20 % 4 = 0): 100 points 17:46.67 instead of 17:46.8 |
| 58 kit keys | none (the game's select_profile stamps) | nothing |
| 71 freeze detector (local tool) | "split" when `time % 50 != 0`, the displaced bytes are atomic_tick's own `/50` | constant, or read the tick |
| 72 fall gravity `min(dt, tick)` | reads the controller's tick field (+2C704h), filled from the game's 0.05 (.rdata 0x9CC878, load 0x7B48D) | nothing in the patch; the game's 0.05 is a site of 6.1 |
| 73 confirmed markers (local) | reads the game's horizon field +18FB4h | nothing |
| 74 sync-core pacing | `TICK_MS 50`, `MIN_GAP_MS 50`, `DEADLINE_MS 300`, the compile-time `DEADLINE + 2*TICK <= HORIZON` | constants (fixed), or the tick a field of `World::Running` and the assert a parse-time check |

### 6.4 What 20 ms would change for players, honestly

- **Rollbacks are not cheaper.** The restore point (the grid point before the late input)
  is on average tick/2 behind the input: 25 ms at 50, 10 ms at 20. That saves 15 ms of game
  time per rollback, but each ms of re-simulated game time costs 2.5x the steps, and a step's
  cost is fixed (section 4). A rollback of an input 100 ms late is ~3 steps at 50 and ~6 at
  20. And there are 2.5x the captures a second (each ~250 us).
- **Per-tick overhead:** ~650 us a step and ~250 us a capture on the owner's PC in a live
  fight; a grid step every 20 ms adds 30 steps and 30 captures a second at least
  (~27 ms/s, 2.7% of a core) before any rollback, and ~+60% with them (section 4).
- **Memory:** +11.8 MB for 26 items (500 ms), +31 MB for 51 (1000 ms).
- **Input-to-effect latency:** the local effect is at once at any tick (prediction). A
  remote player sees a non-precise own input (movement, turning) on the next grid point
  (PROVEN: snap_input 0x2D4E40, run by every move, re-inserts an own input at t at the
  smallest grid point >= t; an off-grid node that is not precise is not sent otherwise,
  0x27EAAE): on average 25 ms later at 50, 10 ms at 20: **~15 ms less**,
  less misprediction of remote players. Precise inputs (bit 5 of the actions, LIKELY fire;
  `match.md` 5.2) carry their exact ms already: **shots are not quantised** today.
- **Hit registration:** shots exact either way; the targets' positions in the world move in
  smaller steps (a 5 m/s run: 25 cm a tick at 50, 10 cm at 20). What decides whether a
  shot that looked right hits is the target's latency (misprediction), which lag
  compensation addresses, not the tick.
- **Desync detection:** the hash is of the event-horizon item, 500 ms behind, judged by the
  server after `divergence_confirm_ms` 1000: the tick changes neither.
- **CPU and memory on a weak PC:** section 4: +40-65% simulation CPU without pacing,
  +14% with it; +12 MB. Bullets are stepped on grid steps only (6.1), so in a fight their
  ray tests also run 2.5x as often.
- **Feel changes that are not latency:** rigid bodies at 5/6 speed unless the physics step
  is re-pointed (6.1); grenade throws and respawns land on a finer grid; Research's points
  round differently (patch 53, exact at 20).

### 6.5 The options

| | (a) fixed 20 ms everywhere | (b) server-chosen (20, 25, 50), as 0.69 | (c) keep 50, take 0.69's cheaper wins |
|---|---|---|---|
| Exe | the 102 swaps of 6.1 in the file, origin 20, the 1/60 pointer for the physics; one revision | the same 102 sites from a table generated at build time, **re-stamped when the match's static info arrives** (/50 = `51EB851F` `shr 4`, /25 = `51EB851F` `shr 3`, /20 = `CCCCCCCD` `shr 4`: the same instruction shapes, so every site stays a byte swap; the origin set to the tick). .text is read-only at run time, so the re-stamp calls VirtualProtect (already imported, IAT 0x870200) around the writes; the other way, every site rewritten to read a variable, is ~100 code changes and is not worth it. The tick carried by 0xE4 (a trailer read by a hook at the 0xE4 case, patch 11's site 0x27A378) | none for the tick (phases 1-3) |
| srp | constants (~2 days) | `Grid` per match (~20 files) | phases 1-3 |
| Patch size | M | M-L | - |
| Risks | every client must agree: a missed site is a desync on every match; old replays and recordings refused (one simulation_changes row; a `tick_ms` header key); CPU (section 4); no way back without a release | the same, plus the site table must be complete; but **a bad tick is rolled back by a server setting**, and 25 can be tried before 20 | none new |
| Test | Unicorn: the step loop, wire reader/writer, ring size, horizon, replay player at 20; local 2-client match; PvB 10v10 with patch 71 traces against 50 | the same at 50, 25 and 20 from one build; a test that every site of the table is the only /50 left in the simulation and network code | the phases' own |
| Sync core fit | `TICK_MS` constant changed | `Grid{origin, tick}` parsed from the static info into `World::Running`; the compile-time assert becomes a parse check | - |

**Recommended:** (c) now, (b) built as phase 4 and shipped at 50 ms, a 25 then 20 ms trial
behind the switch when the trace allows. Not (a): it is (b)'s work without the way back.

---------------------------------------------------------------------------

## 7. Lag compensation in 0.26g

### 7.1 How 0.26g tests a bullet (PROVEN, new here)

- `bullet_manager::tick` 0x321D10 (a core tick delegate, registered at 0x2D0090) ->
  `tick_bullets` 0x3226F0 -> `bullet::tick` 0x32C180 -> `check_collision` 0x32D4E0 ->
  the physics world's ray test, vtable +48h = `bullet_physics_world::ray_test` 0x75E20
  (`ret 20h`; result, from, dir, length, group 0x10, mask 0x48, the bullet as requester,
  1), called at 0x32D5E8..0x32D625. The piercing paths (`process_front_face_piercing`
  0x32E4F0, `process_back_face_piercing` 0x32E740) loop back into the same ray test. It is
  the only bullet query. The world's vtable 0x9E2EBC: +2Ch add, +38h remove, +3Ch move,
  +48h ray_test, +4Ch ray_query, +50h object_query, +54h recover, +60h
  get_all_objects_in_radius.
- **A player's hit body** is one `bt_animated_rigid_body`, a btCompoundShape with one child
  per hit target, mass 0 (`new_animated_bt_hit_model` 0x71600, at `[[player+498h]+12Ch]`),
  in the world with group 0x40, mask 0x11 (`activate_physics` 0x2D9930, at 0x2D995D, and
  in deserialize at 0x2DBAE8). `assign_physics_transform` 0x2D93A0 moves its root once a
  tick (world move 0x75AA0 to player+40Ch, at 0x2D94A9), which updates the broadphase AABB.
- **The bones are posed lazily, at query time.** The ray callback's `needsCollision`
  0x75BD0 calls, for a proxy of group 0x40 (0x75BEE), a boost::function at
  `[object+0F8h]+10h` bound by the base_player constructor (0x2D6F77) to
  `base_player::recompute_damage_collision_bones` 0x2DA5B0: 0 for a team mate of the
  requester, 2 when the pose stamp +344h equals the player's time +324h, else it poses
  the bones (`animated_object::update` 0x3B480) and returns 1, which makes ray_test run
  the ray again on the fresh bones (0x75F4C..0x75F8D).
- **Derived, not serialized:** deserialize 0x2DB490 sets +324h to the time and lowers
  +344h below it (0x2DB49F..0x2DB4CF), forcing a pose on the next query;
  `select_animations` sets +344h = -1 (0x2D978D).
- **Explosions:** 0x339630 (from the trap's explode 0x3056D0 and the grenade's 0x30A4A0)
  takes `get_all_objects_in_radius` 0x760C0 (group 0x10, mask 0x40: hit bodies only), poses
  each player, and rays from the centre to each bone for occlusion (0x3393F0, mask 0x8,
  static geometry). **Sensors** (`collision_sensor` 0x3076A0..0x308220) are ghost-object
  overlaps; the trap sensor (0x201 / 0x420) overlaps the character controller (0x824 /
  0x3), not the hit body. 0.26g has no sensor grenade.
- Sizing: `base_character_full.skinned_model/hit_targets` has 19 hit targets on 16 bones
  (10 capsules, 7 boxes, a sphere, a cylinder). A full pose is 19 x 64 bytes + the root:
  1280 bytes; a root alone 64 (16 as position and yaw).

### 7.2 The collision groups: no room for 0.69's ring

- **16-bit filters (PROVEN):** `needsCollision` reads group and mask as words at proxy+4
  and +6 (0x75BD8, 0x75BE2); the callback constructor 0x75B40 stores them as words;
  `get_collision_group` returns a u16 (as the 0.10b source's `u16 filter_group`).
- **In use:** hit bodies 0x40/0x11; character controller 0x824/0x3 (0x7B4B7); battery
  0xA18/0x418 (0x301D00); grenade 0x818/0x8 (0x303C7B); 0x20/0x20 and 0x800/0x800
  (0x265EFC, 0x26628C); rays: bullets 0x10/0x48, explosion 0x10/0x40, occlusion 0x10/0x8,
  usable 0x114/0x8A, spot 0xFFFF/0x80A, 0x30/0x8 (0x248B58), 0x1/0x20 (0x26D549),
  0x404/0x202 (0x3064D2), recover 0x414/0x20A (0x3066E5), radius 0x18/0x800 (0x2470B0);
  data: usable boxes 0x80/0x100 and 0x880/0x100, trap sensor 0x201/0x420, level objects
  0x80/0x100 and 0x1/0x20 (`collision_geometry::load` 0x2F8C30).
- **Free: 0x1000, 0x2000, 0x4000, 0x8000** (LIKELY; static level meshes added through
  registers not exhausted). Four bits, not 16 or 21.
- **0.69 (PROVEN, a correction to `netcode_069.md` 2.7):** its filters are 32-bit
  (`needsCollision` 0x2005E7D70 reads dwords). `base_player::insert` loops 21 times, but
  `0x8000 << i` shifts out for i = 17..20 and every query takes the slot modulo
  `min(max_delay/tick + 1, 16)`: only 16 slots are used. And the 21 bodies are **coarse
  bounding bodies holding only the player's root at a past tick**, not 21 copies of the hit
  model: the slot writer 0x200305CA0 (from the per-tick update 0x2003021D0) moves body
  `(T / tick) % N` to the root at T; a ray that touches a slot's body makes the callback
  0x200306260 compute `T = t_cross - remote_player_delay(shooter)`, re-pose the one hit
  model at T by interpolating two world captures
  (`base_player::recompute_damage_collision_bones_for_time` 0x200307160) and test the ray
  against it with `rayTestSingle` 0x20067AC70; the bounding body itself never reports.
  The ring is derived (rebuilt from the captures after a load), not serialized.
  Explosions (0x20040FD40) overlap with the slot bit and re-pose the same way; the
  occlusion is not compensated.

### 7.3 The mechanism for 0.26g

| Option | What | Verdict |
|---|---|---|
| (a) a Bullet world per past slot | new worlds in patch code | too heavy |
| (b) our own ray test against stored poses | ray vs capsule/box/sphere/cylinder, a fake `closest_ray_result` for the body part and material | the most code |
| (c) **move each target to its past root around the query, then back** | the game's world move (vtable +3Ch, updates the AABB) before the ray test, a bitwise restore after; the lazy bone pose and the team filter of 0x75BD0 unchanged | **recommended** |
| (d) a shadow body per player in a free bit (0x1000) | posed on demand; the bullet mask 0x48 -> 0x1008; 0x75BEE's `test cl,40h` widened | more code than (c) |

0.69's "pose in the callback" alone does not work here: without proxies at the past
positions, a past position outside the current AABB never reaches `needsCollision`.

**The smallest deterministic version:**
- **Ring:** once a tick, after the move at 0x2D94A9, store the root (player+40Ch) into
  `ring[place][(t / tick) % N]` (t = player+324h), with the spawn tick so a slot from before
  a respawn is never used. **Serialized** with the player (N x 16 bytes appended to
  `base_player::serialize` 0x2DAF60 / deserialize 0x2DB490), so a resynced or late-joining
  game holds the same ring as the others; the server's snapshot walker skips the longer
  player block (u16-length-prefixed; LIKELY no change).
- **Swap:** around each bullet ray test (hook at 0x32D5E8, 5 bytes; the piercing loops pass
  through it again) and each explosion (the entry of 0x339630, or its calls at 0x305728 and
  0x30A4ED): for every other live player, save the body transform, move it to the slot at
  `t - d`, query, restore bitwise. The bones stay the current pose relative to the root:
  posture changes (crouch, stand) inside d are not compensated.
- **The delay d:** world state, as 0.69's `ping_input`. Smallest: one constant a match
  (e.g. 100 ms = 2 ticks), from the static info, behind a revision. Per player: a new input
  kind carrying the latency (0.69's ping input), sent like the kit input; needs phase 3's
  measurement, a new C->S/S->C payload and a server rule (clamped to the max delay).
- **Size:** ~0.5-0.7 KB of code (Rust with asm entries) plus 2-8 KB of state; the full-pose
  variant (19 child transforms per swap, 154 KB+ for 20 players x 6 slots) is too big to
  serialize.
- **Risks (determinism):** the ring serialized or not (not: a resynced game differs for
  d ms); slots from before a spawn; the bitwise restore; the pose stamp +344h must never be
  left on a past pose (the root-only variant never poses past bones). The broadphase tree
  changes order with the moves: harmless, every game makes the same moves.
- **The design risk:** 0.26g draws remote players at the predicted present (LIKELY). A hit
  tested against the target's root d ago is offset by v x d from the model the shooter sees
  (0.5 m at 5 m/s with 100 ms). So lag compensation buys **hit stability** (a hit tested
  against a past, confirmed pose is not undone by the target's late input, when d is at
  least the input latency), and costs aim alignment unless remote players are drawn d late
  too, which is open even for 0.69 (`netcode_069.md` 7.2). This is why it is the last phase.

---------------------------------------------------------------------------

## 8. The code: shared crate, client arena, server

### 8.1 Where each part lives

```
 srp-026g/crates/sync-core        no_std, no alloc; host tests; used by srp and the arena
   time.rs      GameTime, Tick, TickLen (20 | 25 | 50), Grid { origin, tick }
   pacing.rs    (patch 74)
   wire/        ClientUpdate, ServerUpdate, Heartbeat, Complete: encode into &mut [u8],
                decode from &[u8] into fixed arrays (20 places); one layout for both sides
   confirm.rs   OwnInputs: Pending{t} -> Confirmed{t} | Moved{from, to} | Discarded{t};
                the 500 ms discard rule; Closed{up_to: Tick}
   clock.rs     Steering: margins ring, windowed minimum, Steady | Correcting{offset}
   lagcomp.rs   Ring { slots: [Root; N], spawn: Tick }, slot_for(t, d)
 srp-026g/client-patches/netcode_arena    the arena crate: hook entries, the game's fields
                parsed once (as sync_core_arena does), engine calls (change_input,
                enter/leave, move/discard input, set_offset, the packet sender), panic = ud2
 srp-026g/client-patches/netcode_026g.asm  entry stubs (stack aligned to 16), the id range
                bytes, the relocated id tables and dispatcher entries
 srp (match server)  the tick close, timelines, per-client updates, steering (1b), the
                recorder header's tick
```

Constraints of the arena (from the sync-core link, PR #452): rustc alone, offline;
`core` only; no u64 division or `memcmp`/`memmove` (undefined symbols are refused: the
crate must avoid them or the arena must provide them); no allocator (fixed arrays, or the
game's `engine_malloc`); `.srpx` has ~870 KiB under its 1 MiB guard, and state is file
bytes. A netcode crate of tens of KB fits.

**What must stay asm:** the hook entries (stack alignment, the game's register ABIs), the
dispatcher's range bytes and tables, the CALLEE sites (try_send_all's send, the bullet ray
test, the explosion pass, the ring writer after 0x2D94A9), and, for phase 4, the
re-stamped constant sites.

### 8.2 The state machines (`sync_core.md` 3)

Client, inside `Playing` (3.3), each a sub-machine, no flags:
- `Inputs: OwnInputs` (phase 2): per own input `Pending{t}` until an update `U >= t`
  confirms it (`Confirmed`), moves it (`Moved{to}`), or the 500 ms rule discards it.
- `Closed{up_to: Tick}` (1c's `complete_to` T, then phase 2's U): the seal. An insert below
  it is an error (loud in tests): no rollback can reach a closed tick.
- `Clock: Steering` (phase 3): `Steady | Correcting{offset, until}`.
- `Grid` (phase 4): parsed at `Loading` from the static info; constant for the match.

Server, inside `ClientSync::Live` (3.5) and the match:
- per peer `heard_to: Tick` (piece 1) and `Steering` (1b, 3);
- per place a `Timeline` of open ticks;
- per match `TickClose: Open{U, waiting: PlaceMask, deadline} -> Closed{U}`, one
  transition per tick; an update out to each Live peer on `Closed`;
- the resync and the late join: `Syncing` sends the closed updates after the world's time
  instead of the relay replay.

Today's `Stage` and the forty `Peer` fields (`sync_core.md` 3.4) should move to the enums
first (phase 0): phase 2 lands in `Live` and `Syncing`, and doing it in the flat fields
would add the corner cases the owner asked to avoid.

---------------------------------------------------------------------------

## 9. Test plan

Each phase:
1. **Host tests** of the shared crate (`cargo test -p sync-core`): encode/decode round
   trips against byte vectors written from 0.69's layouts; the confirm machine's
   transitions over scripted update streams (confirmed, moved, discarded at exactly 500 ms,
   duplicate U, a gap); steering on synthetic margins (no oscillation, bounded error).
2. **Unicorn** (`check_*_026g.py`, the original exe and the build): the hooks entered with
   the stack at every residue of 16; the handler driving the game's own change_input,
   move/discard, enter/leave on a model world (sync-core's differential harness: the same
   timeline through the stock relays and through the updates gives the same event-horizon
   items); phase 4: the step loop, the wire reader and writer, the ring size and the horizon
   at 50, 25 and 20; phase 5: a scripted shot at a moving target with and without the swap,
   the world bytes after the query identical to before it.
3. **Server tests** (srp): the tick close with places joining, leaving, stalling; late
   inputs re-timed; the late join and resync from updates; recordings converted.
4. **A local 2-client run** (`scripts/local-clients/run.py`, on the owner's go): a match
   with shots and a forced stall (the stall test of `sync_core.md` 9), patch 71 traces
   compared with the previous build, no resync.
5. **Player vs Bots 10v10**: the bots' encoder on the new path, 10 minutes, the trace and
   the server's divergence log; for phase 4 the same at 50 and 20 ms.
6. Old replays and recordings: refused with the reason (replay gate), never misread.

---------------------------------------------------------------------------

## 10. Risks

- **Desyncs:** every client must agree. Phases 1-3 change no world byte: a bug there
  delays or misplaces inputs (a resync), never a different simulation of the same inputs.
  Phase 4 at another tick and phase 5 are simulation changes: one missed site or one game
  without the ring serialized is a divergence on every match; the hash catches it within a
  second and a resync repairs it, loudly.
- **Old replays and recordings:** `.sur` is in ms but the reader is grid-bound: a different
  tick refuses old replays (one row of `simulation_changes.tsv`); `.srprec` gets a
  `tick_ms` header key (missing = 50); phase 2's messages need the converter.
- **Weak PCs:** the tick (section 4) is the danger; phases 1-3 cost nothing measurable
  (a message a tick); phase 5 costs N moves per query per player (cheap next to a step).
- **Protocol:** phase 2 replaces every relay path at once; behind a switch, the old paths
  stay in the server until the new one has run live, then go (one supported client).
- **32-bit memory:** a deeper or finer ring costs 12-31 MB of a large-address-aware
  process, inside the match cook's fixed 112 MiB arena (6.1): it fits.
- **The re-stamp of phase 4** writes code at run time: a site missing from the table is a
  50 ms division in a 20 ms game (a desync on every match). The table is generated from the
  original exe and checked by a self-test that no other `51EB851Fh` / `32h` of the listed
  functions remains; hooks that displace a site's bytes into their own stubs (patch 71's
  entry on atomic_tick takes 0x2D1E21's `mov eax,51EB851Fh`; it is a local tool) must
  re-stamp their copy too.
- **Scope:** the owner's rule of few corner cases: each phase is one machine change; the
  switch is one setting, removed when the phase is proven.

---------------------------------------------------------------------------

## 11. Re-kit rows added

`tools/re-kit/v026g/inferred.tsv` (34 rows), `abi.tsv` (8), `abi_inputs.tsv` (17). Names
marked "ours" come from what the code does (no string names them).

| VA | Name |
|---|---|
| 0x0003B480 | `vostok::collision::animated_object::update` |
| 0x00075B40 | `vostok::physics::closest_ray_result_callback::closest_ray_result_callback` |
| 0x001EDEF0 | `survarium::game::play_replay_to` (ours) |
| 0x001F76B0 | `survarium::game_world::interpolated_view_at` (ours) |
| 0x001FDCC0 / 0x001FDCF0 | `game_world_core::fixed_time_before` / `fixed_time_at_or_after` (ABI rows: EAX time, ECX origin) |
| 0x0020FF90 | `vostok::network_core::cyclic_difference<unsigned char>` |
| 0x00218BF0 | `survarium::replay_network_client::tick` |
| 0x00248160 / 0x00248720 | `survarium::booby_trap::explode` / `survarium::grenade::explode` (ours) |
| 0x0027CBF0 | `survarium::network_client::send_match_messages` (the body of try_send_all) |
| 0x002CBC60 | `survarium::pvp_match_core_cook::on_resources_loaded` |
| 0x002D1CE0 / 0x002D1D00 | `game_world_core::tick_game_effect_player` / `tick_physics_world` (ABI rows) |
| 0x002D4E40 / 0x002D4F80 | `game_world_core::snap_input<player_input>` / `<profile_input>` |
| 0x002DA9A0 | `survarium::base_player::tick_physics_animation` (ours) |
| 0x002F6940 | `survarium::network_client::read_message_time` (ours) |
| 0x00303B60 / 0x00303C10 | `survarium::grenade_core::explode` (ours) / `insert` |
| 0x00304DB0 / 0x003056D0 | `survarium::booby_trap_core::tick` / `explode` (ours) |
| 0x0030A120 / 0x0030A4A0 | `survarium::grenade_set_core::throw_grenade` / `grenade_core::apply_explosion` (ours) |
| 0x00318400 / 0x003184A0 / 0x00318540 / 0x003185A0 | `survarium::deserialize_player_input` / `serialize_player_input` / `deserialize_profile_input` / `serialize_profile_input` |
| 0x003185F0 / 0x00318620 / 0x00318690 | `survarium::serialize_local_fixed_time` / `serialize_local_time` / `deserialize_local_time` (ABI rows) |
| 0x0031BD90 | `survarium::anomaly_state::tick_zones` (ours) |
| 0x003393F0 / 0x00339630 | `survarium::bullet_manager::explosion_occlusion` / `process_explosion` (ours) |
| 0x00075550 | ABI rows of `vostok::physics::bullet_physics_world::tick` (ECX the world; dt in ms, time in ms; `ret 8`) |

`game_world_core::clear` 0x2D0550 is left to #452, which adds it. Data and fields named in
this note (not rows): core +18FBCh the grid origin, +18FB8h the input lead in ticks; the
match cook's arena (0x9261E38 +24h/+28h, 112 MiB); the controller's nominal step
+2C704h (0.05 from 0x9CC878); the physics' 1/60 at 0xA2DBB4 and 0.02 at 0xA2DB34 (both
shared: never edit); the player's +114h team, +324h time, +344h pose stamp, +40Ch root,
+498h animated object (+12Ch the hit body); the hash interval static 0xA2B393C.

---------------------------------------------------------------------------

## 12. Open questions

1. The retail tick (LIKELY 20 ms) and max remote delay are server values; a 0.69 replay or
   capture would settle them (`netcode_069.md` 7.1).
2. What share of a step is proportional to dt (the physics substeps): a trace of a local
   match at 25 ms (phase 4's switch) would measure it.
3. Whether 0.69 drew remote players delayed to match its lag compensation (open in #468);
   it decides whether phase 5 should come with a delayed draw of remote players.
4. The meaning of 0.69's 0x4000 bit in the bullet's group and mask (0x4010 / slot | 0x4008).
5. Whether the four free collision bits of 0.26g are really free on every level (meshes
   added through registers were not exhausted).
