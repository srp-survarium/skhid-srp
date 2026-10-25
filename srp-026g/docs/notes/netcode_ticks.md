# The per-tick protocol (0.69's netcode on 0.26g)

The spec of what PR "Netcode port phases 0, 2, 3, 4" builds: the plan's
phases 2 to 4 (`netcode_port_069.md`, draft PR #472; 0.69's own rules in
`netcode_069.md`, PR #468). Server: `match-server/src/v026g/game/ticks.rs`;
shared rules: `crates/sync-core` (`wire`, `land`, `confirm`, `clock`,
`time`); client: patch 79 (`client-patches/netcode_026g.asm`,
`rust/netcode`, `tick_sites_026g.py`; `client_patch.md` 3.61).

## Switch

- `SRP_NETCODE=v069|off`, default `off`. Off: every match is the stock
  protocol, byte for byte; a game that announces the new one plays the
  stock one. Kept for rollback.
- `SRP_NETCODE_TICK_MS=50|25|20`, default 50, only with `v069`. 25 and 20
  are trials (more simulation CPU in every game); never a default.

## Who plays it

- A patched game announces it in 0xD8: bit 1 of the capability byte
  (0x82, 0x83 with the late join's debug bit).
- With the switch on, a game that does not announce it is refused at its
  connect (and the log says so). Nobody is ever sent the protocol it did
  not announce.
- The server's 0xE4 to such a game ends with a trailer `[tick_ms, 1, 'S',
  'R', 'P', 'T']`: the game takes it off before anything reads 0xE4. No
  trailer: the stock protocol.

## The wire (`sync_core::wire`, version 1, little endian)

- C->S 0xE2, the game's update of a tick, every tick, empty or not:
  `[ver][u32 tick][n inputs][m kit choices] n x {back, rotation[4], actions
  u32} m x value`. An input's time is `tick - back`, inside the tick.
  Plain, reliable, resent every 50 ms (0xDE's info).
- S->C 0xF7, the server's update of a closed tick, one a tick to each
  game: `[ver][u32 tick][u32 entered][u32 left][arrival: 0 | 1 + i16]
  [n][m][k] n x {place, input} m x {place, value} k x {u32 update, 0
  confirmed | 1 moved + u32 to | 2 discarded}`. Unique (a large one
  splits), 500 ms to live, resent every 50 ms.
- The stock relays and their answers (0xE9..0xF5, 0xE7) are not sent on
  such a match; the game makes them itself from 0xF7.

## The server's rules

- An update lands at `max(first open tick, its tick)`: never in the past.
  Late, it is moved to the first open tick (outcome "moved to X") while it
  is still applicable for every game (`land::reach_ms`: 500 ms of history
  less the round trip, a tick and 50 ms of margin); later than that, or
  overtaken by a later update of the same game, it is discarded.
- A tick closes once the clock is past it and every playing place was
  heard for it, or 250 ms after it at the latest. A place not heard from
  for 250 ms holds nobody back.
- Each closed tick goes to every game: the others' inputs and kit
  choices, the entries and leaves of that tick, and that game's own
  outcomes. The server alone decides; every game applies the same.
- An input no game makes (a rotation half at 1024 or more, an unused
  action bit) discards its update; every game leaves such an input out.
- Duplicates (resends) are answered once.
- The server bots play the same ticks (a bot's inputs land like a game's).
- Resyncs, late joins and the replays of a closed tick work as before;
  the held answers of a game being resynced wait for its world.
- No 0xE7: each game steers its own clock.

## The game's rules (patch 79)

- The stock sender still picks what to send; the patch takes the own
  input at its new_packet and skips the stock packet. After each send pass
  one 0xE2 a tick since the last (a stall's empty ticks past one second
  are not sent; no input is lost).
- An outcome of its own update: confirmed nothing; moved: the stock move
  (0xEF/0xF0, 0xF4) of each input; discarded: the stock discard; an update
  older than the world it was resynced to: put back as a relay of its own.
- The others' changes go through the stock handlers, as the stock server
  sent them; leaves wait until the world is loaded.
- The clock: each 0xF7 says how early the game's updates came (the least
  `tick - match clock` since the last). The game keeps the latest 40; with
  10 or more, when the least is over 5 ms late (or over 55 ms early) it
  corrects by the difference with the game's own gradual correction, or
  at once past 250 ms, then waits for the correction to run.
- The tick: 0xE4's tick is stamped into the exe's 151 tick sites (46
  divisions, 48 imm8, 7 imm32, the hash interval, the interpolated view's
  1/T, the physics step T/3) with VirtualProtect; a 50 ms match writes
  nothing. A stock 0xE4 and a replay's start put 50 back. A site that does
  not hold what the last stamp wrote stops the stamp (log line).
- The journal (`-record_journal`) keeps the stock messages the game made,
  not 0xF7: it replays as a stock match (at 50 ms only).

## Recordings and replays

- `.srprec` headers carry `"netcode"` and `"tick_ms"`; the `.sur` made of
  a per-tick match reads the updates (and each game's own 0xE2 with its
  outcomes). A match of another tick than 50 makes no replay (the game's
  replay player is on 50 ms).

## Not in it

- Lag compensation, the state summary, a simulating server (plan pieces
  5, 7, 8).
- 25 and 20 ms are not tried in a game; patch 74's pacing constants stay
  50 ms.
