# Match netcode (0.69d)

Part of **Skhid (Схід)**, a private server for Survarium 0.69d. The `srp` program is the Skhid server; the operator guide is [guide-server.md](../guide-server.md).

Status words: PROVEN = read in the 0.69d exe (address given); LIKELY = inferred; CHOICE = a server rule with no client
evidence; OPEN = unknown.

| Item | Result | Evidence |
|---|---|---|
| DF `n_late` entry | `u8 dt, u8 player`; the input time is `T - dt` with `dt` in **milliseconds** (one byte, so at most 255 ms old); no tick scaling | PROVEN 0x20032ED00 (the DF time minus the byte) |
| CD late entry | `u8 (CD.time - input.time)` in milliseconds, then the 11-byte input | PROVEN 0x20032F25B (the byte is subtracted from the input time at +0x10) |
| Input size | `packed_inputs = 0`: one input = 11 bytes `u16, u16, u56 big-endian` | PROVEN reader 0x2003276E0 |
| `tick_ms` | the `u8` at options + 0x1083, sent by the server in the match options (offset 4 of the unpacked blob) | PROVEN as a field; the value 50 is a CHOICE (0.26g used it; LIKELY the real one) |

What the code does with it:

* `servers/match-server/src/relay.rs`: the CD stamp is `time - dt`, the DF `dt` is the age in milliseconds, a late input
  older than `min(late_max_ms, 255)` is dropped.
* `crates/local-match/src/options.rs`: `PACKED_INPUTS = 0` as a named constant and `TICK_MS` as above; a unit test checks
  the header (send period, tick 50, `packed_inputs` 0).
* Tests: `servers/match-server/tests/relay.rs` and `tests/common/mod.rs` use milliseconds (a late input of 150 ms, one of
  250 ms that is dropped when it is too old; `dt` on the tick grid).

Open: the bit-packed input form (`packed_inputs = 1`, 0x2003E7DF0) is not implemented and not needed; the real server's
tick value is unknown (no real 0.69d match capture exists).

## Online matches (`online.md`)

The lobby handoff (`82`), the world capture procedure, the end-of-match reports and the lobby wiring are in `online.md`.
Facts from the world serializer: the statistics object (`W+0x180A38`) is saved into section G3 of every DE item (saver
0x2002F4070, loader 0x2002F4C40) and into every player block (0x2002F41C0); G3/G4 hold the rule modules (vector
`W+0x17D118`), G1/G2 the level-registered objects (list `W+0x17D780`, `u32` count first). The loader diffs player masks,
so an item with no players is valid input. The first world cannot be built server-side from static reading; it is
captured with the breakpoint at 0x20011A295 (`online.md` 2.2).

## Match items (PROVEN unless marked)

* DF reader 0x20032E790: `n_late` entries are `{u8 dt, u8 player}`, input time = T - dt in plain milliseconds
  (0x20032ED00); late inputs are applied before the normal ones with a "late" flag.
* `change_input` 0x2001B3290: a second copy for the same player and time is ignored; anything at or before the DE time is
  ignored; anything older than the 500 ms horizon is moved to about present + one tick ("will be moved locally", the desync
  case); a whole DF older than the horizon is dropped with a log line.
* Every CD must be confirmed with `a` = its time, else once more than 500 ms of later DFs arrived the sender discards its own
  input ("Discarding input..."). Our relay confirms every CD.
* The client sends one CD per tick and asserts `CD.time % tick_ms == 0` (fatal, 0x20032F1A7): the DE and every DF time must be
  on the tick grid. `World::validate` refuses an off-grid world.
* `send_period` is only the transport resend interval (twice it = the message lifetime); it does not set the CD / DF rate.
* A cached world with an empty player mask is accepted; the client sends no CD until its own player entered (normal, not a
  lost DE). The first DF must carry each player's enter bit and CB profile input, on the grid, after the DE time and within
  500 ms of the clients' time (our relay does). Entering sets the respawn time to the enter time; the spawn itself needs the
  respawn request (profile_input byte 1) or autorespawn (byte 2), which the client sends.
* A repeated enter is a no-op (0x2002F2C55). The relay repeats a player's enter bit in every DF until its first CD and
  re-sends its profile with every DE retry, so a lost first DF does no harm.
* Optional (LIKELY safe): the relay rounds CD late stamps down to the grid; passing `cd.time - dt` through is exact either way.

## Size limits

* Parts: the part count is a u8 (0x2007568EF): a message has at most 255 parts. Our parts are 440 bytes, so a DE payload is
  at most 112 200 bytes; a world whose DE needs more is refused when the world file is loaded (`world.rs`
  `DE_MAX_PAYLOAD`), not at match start. `lzma2::compress` writes stored chunks only (no compression), so a big
  world must be stored as real raw LZMA2 (`body_kind 1`, dictionary <= 0x44800, the client's decoder) by an offline encoder.
  OPEN: real 0.69d world sizes (online.md 2.2).
* DF: the client copies `min(len, 0x1000)` bytes into its DF job but parses `len` (0x2005BB7B0..0x2005BB7CC): every DF payload
  stays under 0x1000 bytes; surplus ping / profile entries and inputs wait for the next DF (a deferred input of the tick goes
  out as a late input; `MatchStats::df_deferred`).
* Receive buffer 1200 bytes per packet (LIKELY: one datagram per packet; the receive call was not traced): our datagrams stay
  <= 508 bytes. Bigger DE parts (~1100 bytes) only after a run confirms it.
* Options record: +0x2D8 = finished matches. Below `damage_protector` 3 (`default.player`) the client takes
  `max(0, (3 - n) / 3) x 0.15` off toxic (hit type 4), radiation (5) and shock (7) damage (0x2003EA53D) for EVERY player, no
  booster needed. We send the account's real count (`profiles.matches`, schema v23, counted for every finished match
  that `stats::record_match` records; local bot matches count nothing, CHOICE). +0x1082 = PvE difficulty AND an unchecked
  index into the 3 anomaly-maze damage profiles (0x20030EC81): PvE options must send 0..2; PvP sends 0. +0x1081 = equipment
  level (a script variable). +0x2DC: no reader found. Options rank table: the in-match scoreboard never draws the rank (only
  a tie-break); the league-top crown byte is drawn.
* E0 / E2 / E4 are not needed for a match without a world host (E4 = anti-cheat kick).
* Hash checks (always on with a live world host): the relay sends E0 `{tick_lo, mask 0, g0, g1, g2}` from the host's hash
  of item t taken after DF t + tick (the client's own hash time), from the enter + 500 ms; a CD's inputs, profile, ping and
  confirmation stay in one DF (whole bundles move); with any host attached a late entry keeps dt < 2 ticks.
* Resync (always on with a live world host): a client's CC (tick rebuilt from `tick_lo`) waits until the host's newest
  final tick (last DF - 300 ms) lies above it, then one `WantDe`; the host's DE (item T with full 1 + the per-player
  animation-history tail; PROVEN in test matches to load in the stock client and keep its world equal to the host's) goes
  to that client only, after the relay's checks (T on the grid, after the first DE, not after the last DF, above the CC
  tick, item time at +0, <= 0x44800 decoded, <= 255 parts, presence mask at +0x11 against the roster). Afterwards no E0
  and no CC at or below max(T, the newest DF when it went out) for that client. Breaker: at most 3 host DEs per client per
  60 s, the next CC switches that client's resync off; a CC with reason 1 (the DE did not land) after a DE switches it off
  at once. The CC reason is ONE value, not bit flags (CC sender 0x2001AE98F..0x2001AE9EB): 4 = mismatch with every DF and
  equal masks, 2 = beyond the contiguous DFs, 8 = while the masks differ, always with a mask-0 E0, 1 = forced; 2 / 4 / 8
  are all real mismatches; `global` 0xFF = g0 or g1 differ. 3 failed DEs (refused, lost after 2 s, failing the checks)
  switch it off for the match. Never while the link is not live. A host refusal (`Fault "WantDe <id>: ..."`) is not a
  link failure; a restarted host child replays the log without the WantDe frames of the earlier child.
