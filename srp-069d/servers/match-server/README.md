# match-server (0.69d)

Online match relay. Evidence tags in the code: PROVEN = read in the client code, LIKELY = shape/analogy, UNVERIFIED =
no evidence, CHOICE = our own value where the client gives none. Background: `docs/notes/online.md`,
`docs/notes/netcode.md`.

## What it does

The relay has no game simulation of its own. Per player: `C9 -> DC (ports) -> DD (options + level checksum) -> CB
(ready) -> DE (start world) -> DF every tick`, CD relayed to all players, CE / timeout = leave, end by the mirrored
time limit / all left / lobby request.

With a world host attached (`world_host` in the config, crate `world-host-proto`) the host runs the game's own world
code on the match's DF stream: it builds the start world, sends the per-tick hashes the relay passes on as E0, answers
resync requests (CC) with a DE of the running world, reports the end, winner and per-player counters, drives the bots
and gives a dropped player's client the running world when it rejoins. Without a host, or with a host that failed, the
match runs as a plain relay: a stored start world, no E0, CCs logged and not answered.

| Module | Content |
|---|---|
| `table` | match message table: S->C DC..E5, C->S C9..D4 (class, length rule, lifetime, resend), PROVEN |
| `mtransport` | `MatchTransport`: udp-transport's header codec and seq window with that table (see below) |
| `match_msgs` | payload codecs C9 CA CB CC CD D4, DC DD DE DF E0 E2 E4, `profile_input`, `player_input`; PROVEN/LIKELY in doc comments |
| `world` | `.srpde` world cache format + `collision_checksum` helper |
| `relay` | one match: handshake, DE, DF loop, leave, rejoin, end, resync rules, host bots (sans IO) |
| `hub` | lobby API + datagram router (sans IO); `server` = UDP socket thread around it |
| `host` | world host link per match (`query_projects` asks the daemon for its projects): Create / Df / Close out, Ready / Tick / De / Bots / Stats / Outcome / Final / Diverged / Fault in, catch-up restarts; socket IO on its own threads |
| `clock` | maps the monotonic clock onto the wall clock's rate (a VM whose monotonic clock runs slow would slow the clients' worlds) |

### Lobby API (`Hub` / `MatchServer`)
- `new_match_id()`: a fresh match id (= the world's seed) for the `match_options` blob; every match has its own, so
  several matches of one map run at once. `can_create(map, mode, project)`: whether a start world can be had.
- `create_match(MatchSpec{map, mode, options, players[{account_id, name, team, session?, bot}], project, ...})` ->
  `MatchTicket{match_id, host, port, players[{slot, account_id, order_id}]}`. The lobby sends each client
  `0x82 {host, port, match_id, side, order_id}`; the client answers C9 `{order_id, session}`. `order_id` is a random
  unique per-player token. `options` is the raw `match_options` blob (layout: local-match `options.rs`) with the
  players in roster order = slot order; the server compresses it into DD.
- `set_winner(match_id, team)`: a winner the lobby learned elsewhere. `end_match(match_id)`: forced end (final DF with
  every player's own leave bit). `end_match_after_result(match_id)`: end after the result went out (closing DFs with
  code 2: the client keeps its world and shows the result screen).
- `take_ended()`: `MatchEnd{match_id, map, mode, reason, winner, duration, players, stats, ...}`;
  `take_outcomes()`: the world host's end of a match while it still runs; `take_left()`, `take_dropped()`,
  `take_rejoined()`, `allow_rejoin()`, `leave_dropped()`: leaves and rejoins. `MatchServer::start(.., on_end)` calls
  the end callback on the driver thread.

### World cache file (`.srpde`, one per (map, mode, project))
Used when no world host builds the start world. The exact header table is in `src/world.rs`; `srpde-pack`
(`src/bin/srpde_pack.rs`) writes it from a captured item (procedure in `docs/notes/online.md` 2.2).
`WorldStore::load_dir` loads `*.srpde`.

### Why `mtransport` is a copy
`udp_transport::Transport` / `wire::parse` use the lobby `class_of(direction, id)` everywhere, so every match id is an
"invalid message type" there. `mtransport.rs` reuses the header codec and the seq window and has its own message layer:
fixed-length ids (no length prefix in a multi datagram; E4 length by content), per-id lifetime and resend (DE
1900/100 ms, DF 2 x cfg / cfg, E0 40 ms), `send_tracked` + `delivery(key)` (DD only after DC was taken), a send window
and a 100 ms keepalive. A shared transport would need a table parameter in `udp-transport`.

## Open risks
1. **DF details**: header, counts and the `dt` unit (milliseconds, 0x20032ED00) are PROVEN; the order "late inputs
   before the input-mask inputs", the `code` values other than 0xFF and 2, and the 3-byte mask byte order are
   UNVERIFIED. `packed_inputs` (bit-packed DF) is NOT supported: the options must have it 0.
2. **Without a world host desyncs are silent**: no E0, so a lost DF, an input that reaches a client > 500 ms late or
   float differences between PCs diverge the clients without any signal. Mitigation: DF resent for 2 x send_period,
   late inputs > `late_max_ms` (400) dropped, DE re-sent when no CD follows.
3. **Late join** (CB after the start) is refused: it needs a DE of the running world. Rejoin of a dropped player works
   only with a world host.
4. **DE lifetime is 1900 ms** in the client's table: a DE that is not fully acked in that time is dropped for good. The
   DE is re-sent after 3 s without a CD (3 times); a DE may have at most 255 parts.
5. **DD level checksum**: must equal the client's CRC-32 of the collision world (fatal assertion "Checksum mismatch").
   With a world host it comes from the host's `Ready`; a stored world carries the value from the client log.
6. **Handshake order**: DD is sent only after DC was acked (or CA seen) because the client takes DD only in state 1.
   If neither ever comes the player stalls.
7. `CF` table length 10 vs. one f32 written: `Fixed(10)` assumed; only matters in multi datagrams.

## Tests
`cargo test -p match-server`: unit tests (table, transport on the match table with byte examples, every codec, world
file, clock), `tests/relay.rs` (hub + scripted Client-role transports on a virtual clock: handshake, DD/DE content, DF
stream, relayed inputs, confirmations, late input + duplicate CD + CC, leave, lobby end + winner, ready timeout,
refused session / late joiner, silent client, PvE ends), `tests/host.rs` (the host link against a fake world host on
a socket pair: start world, E0, resync, bots, outcomes, restarts, rejoin), `tests/udp.rs` (real loopback sockets) and
`tests/host_e2e.rs` (ignored by default: a recorded match against a running world host daemon).
