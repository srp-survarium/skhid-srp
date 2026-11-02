# 0.26g matches: sync, hashes, trusted worlds and resync

> **Version scope: client 0.26g only** (build 2777, no PDB). 0.26g matches
> are still lockstep, but every message, id and time format differs from
> 0.21d. The 0.21d page is MATCH-SYNC-021D.md; its history of fixes is the
> reason behind most numbers here. All addresses are 0.26g's.

> Sections marked *(server, may move)* describe
> `servers/match-server/src/v026g/` as it was read on 2026-10-12; check the
> code and `srp-026g/WORK.md` before relying on a number.

How a 0.26g match stays in step. Byte-level detail:
`srp-026g/docs/notes/match.md` (§1 messages, §2 handshake, §3
snapshot and hash, §4 time, §5 input rules, §13 the server, §14 hash
phases), `client_patch.md`, `snapshot_rules.md`, `transport.md`. Client
facts in short: CLIENT-FACTS-026G.md. The server: SRP-026G.md.

Tags are the notes': **PROVEN** (read in the exe at the address given),
**LIKELY**, **GUESS**. A client fact with no tag is PROVEN in its note.

## The model

- **Every client runs the whole simulation.** The server simulates nothing.
  It takes each client through the handshake, keeps the match clock, relays
  inputs, compares world hashes, gets worlds from patched clients, starts
  and resyncs clients from them, and ends the match.
- The client's server mode is hard-coded off (0x2CBBC1), and its world is
  **frozen until the first snapshot** (`game_world_core` +0x18FDF; `move`
  0x2D2E40 returns at once). match.md §2.1.
- **The stock client never sends its world.** `game_state_history_item::serialize`
  0x324BE0 has three callers and none builds a packet. So only a patched
  client can give the first world and every resync world. client_patch.md
  §2.
- A tick is 50 ms. The client's event horizon is `max_ping_time_in_ms` + 250
  = 500 ms; it keeps 11 history items.
- The roster is fixed by the static info (0xE4): always 20 profiles, N of
  them real. Players are put in with 0xE9 and taken out with 0xEA.

## Messages (`match.md` §1)

Client → server: 0xD8 connect {`u32` session, `u8` debug}, 0xD9 per port,
0xDA ready, **0xDB time reply** {3 × `u16`}, **0xDC input** {tick, 2
half-floats, `u32` actions}, 0xDD input with a precise time (+ `u8` ms),
0xDE kit choice {tick, value}, **0xDF hash** {tick, `u32` CRC}, 0xE0 leave
(empty; with a payload it is a snapshot chunk from a patched client), 0xE1
keep-alive every 1000 ms.

Server → client: 0xE3 ports, **0xE4 static info**, **0xE5 set time** {`u8`
generation, `u16` echo, `u32` match ms}, 0xE6 ping {`u16`, `u16`}, **0xE7
clock offset** {`i16` ms}, **0xE8 snapshot** {`u8` generation, `u8`
compressed, body}, 0xE9 / 0xEA player entered / left {tick, player}, **0xEB
/ 0xEC input relay**, 0xED / 0xEE several players' inputs of one tick, 0xEF
/ 0xF0 move your own input {from, to}, **0xF1 / 0xF2 discard** your own
input, 0xF3 kit choice relay, 0xF4 / 0xF5 move / discard a kit choice, 0xF6
(debug only in a stock client; the patch makes it the upload request).

Rules of the wire:
- **Times are one byte**: the tick number mod 256, read against the
  receiver's own clock (±127 ticks = ±6.35 s). Only 0xE5 carries a whole
  `u32`. A message more than about 6 s off is read as another tick, with no
  error. match.md §1.3.
- The two half-floats of an input must be relayed **byte for byte**.
- Nothing is ordered. Relays, roster changes and discards live 250 ms; 0xE8
  lives 1.75 s; 0xE5, 0xE6, 0xE7, 0xDB, 0xDF are sent once. transport.md §4.
- Client messages are plain: the server must drop repeats of D8, D9, DA, DE
  and E0 itself.

## Generations: how a sync is taken (`match.md` §2.2)

Every sync has a one-byte **generation**. The client keeps the last one it
loaded at `network_client` +0x1F1C68.

| Fact | Where |
|---|---|
| Just before 0xDA (ready) the client sets its generation to 0xFF, so the first sync is generation 0 | 0x27AE9A |
| **0xE5 is taken only when its generation is strictly newer** (ahead by 1..127, mod 256). It sets the game time, answers 0xDB, clears the history, and zeroes the send timers. It does **not** store the generation | 0x27B570 (0x27B59E..0x27B5D1) |
| **0xE8 is taken when its generation is equal or newer; it then stores it.** An older one is skipped whole | 0x27BFF0 (0x27C006..0x27C040) |
| The first 0xE8 logs "THE VERY FIRST SYNCHRONIZATION" and unfreezes the world; later ones log "FULL RESYNC" | 0x2D0E4C, 0x2D0F37 |
| 0xE5 must come after 0xDA: it dereferences the match core | match.md §2.2 step 5 |

So **every resync must use the next generation**: 0xE5(g+1), then 0xE8(g+1).
A late 0xE8 of an older resync is dropped by the client. Because 0xE5 does
not store the generation, the client takes every 0xE5 of a generation until
it has loaded that generation's 0xE8.

The danger: 0xE5 is sent once. A lost 0xE5(g) followed by a delivered
0xE8(g) loads the world onto the old clock (LIKELY, not seen live).

*(server, may move)* The order the server uses (match.md §13,
`game.rs`):
1. 0xE5(g) **alone**, again every 150 ms (`SET_TIME_EVERY`), until the
   client's 0xDB answers it (matched by its echo).
2. Then 0xE5(g) once more with 0xE8(g) behind it. A world never arrives
   without its time.
3. After 5 unanswered 0xE5 (`MAX_SET_TIMES`) the snapshot goes anyway.
4. A 0xE8 not acked within its 1.75 s counts as lost: the sync starts again
   as generation g+1. The first world of a game that has none yet has 4 s
   (`entry_state`): that game stands still, and what a thin line lets
   through of the world adds up in it (match.md §27).
5. The world goes compressed (LZMA2) where that is smaller, its parts a few
   at a time; and to a game that waits for its first world the others'
   inputs go every 50 ms, many in a datagram (match.md §27).

Between the first 0xE5 and the 0xE8 a client being resynced **goes on**
on its new clock: seen live (match.md §30), it sends new own inputs every
50 ms and 0xDF reports for the whole load, never for a tick at or before
its newest own input. Its history is cleared; whether it takes
0xF1/0xF2/0xEF/0xF0 then is not settled. Its input lists are rings of 27
per player: a world loaded more than about 1.2 s after its snapshot plays
the earliest inputs wrong. For a client resynced while it plays the server
therefore sends, after the 0xDB, the others' inputs and the roster changes
since the snapshot, and the world (0xE5 again and 0xE8) only once all of
that is acked, its parts again every round trip + 50 ms (120 ms at least);
a world not loaded 1.2 s after its snapshot goes again from a fresh one
(three times at most); the answers to the client's own inputs wait for the
load. match.md §30.

## Snapshots

**The body** (0xE8 after its two bytes; `game_state_history_item` 0x324BE0 /
0x324D80; match.md §3, snapshot_rules.md §1):

```
u32 time_ms
u32 active_clients_mask
per set bit: one player block (no length)
u16 len + bullet manager
u16 len + statistics
u16 len + game objects
u16 len + physics world        (new in 0.26g)
u16 len + game rules
u16 len + game effect player   (new in 0.26g)
```

- The player blocks have no length, so the sections are found **from the
  end**, by chaining the `u16` prefixes.
- A world is at most 0x8000 bytes: the client's own `hash()` serializes into
  a 0x8000-byte buffer. The server sends 0xE8 with `compressed` = 0.
- What the server reads for results is small: the **11-byte statistics
  tail** (two team points, the winner byte, the active mask) and the
  timelimit rule's 13 bytes, found by their signature (bytes 4..8 equal the
  item's time). snapshot_rules.md §2.2, §3.3.
- The order of the rules inside the rules section is LIKELY, not proven;
  the reader does not depend on it.
- The input lists survive a resync (0xE8 loads with mode 1), and an own
  input node at or before the snapshot's time is never played again:
  PROVEN. match.md §17, §30.

**Uploads** (the patch; client_patch.md §3.2, §3.4):

| Fact | Value |
|---|---|
| Carrier | client → server **0xE0 with a payload**, always alone in its datagram. An empty 0xE0 is a real leave |
| Chunk | `u32 item_time, u16 offset, u16 total, data[n]`, n = min(480, total − offset); datagram 492 bytes |
| Burst | at most 69 chunks (0x8000 bytes); each resent every 500 ms until acked; any order |
| Server checks | `offset % 480 == 0`, `offset < total ≤ 0x8000`, exact length; drop duplicates by (item time, offset); reassemble per item time |
| First world | sent unasked in `on_match_ready`, before 0xDA: the initial item `[core+0x2FC0]` |
| Later worlds | on request: server → client **0xF6 with one payload byte `00`** (an empty 0xF6 fails the length check and drops the whole datagram). The client uploads its **event-horizon item** |
| Pool guard | the client's packets come from a pool of 256; an empty pool crashes it. The patch skips an upload unless `chunks + in use + 16 ≤ capacity` |

A request is **silently not answered** when:
- the client is not in state 2 or has no match core;
- it has no event-horizon item, or that item **came from a snapshot**, or it
  is younger than the horizon. So right after an 0xE8 the client has nothing
  to upload until its simulation is 500 ms past the snapshot's time;
- the pool guard skips it.

Server rules that follow: one request per client at a time; wait for the
upload to finish or about 1 s before asking again; keep acking during the
burst. Send 0xF6 only to patched clients.

The body the server gets is exactly what the client hashes, so
`0xE8 = generation, 0, item_time, body`, and the body's CRC is what 0xDF
reports for that item.

## Hashes and their phases (`match.md` §2.3, §14)

- The hash is **boost CRC-32** of the serialized item **without** its
  leading `u32` time (`hash()` 0x325150; table built at 0x1C210).
- The client never compares hashes. It only reports: 0xDF {tick, CRC}.
- The tick reported is the **event-horizon item's** (the newest item with
  time ≤ now − horizon), not the current tick.
- No 0xDF is sent for an item that was loaded from a snapshot (item
  +0xC0BB9).

**The stock client sends its world hash every second tick, on its own
phase.** Read from `network_client::try_send_all` 0x27CBF0:

| Fact | Where |
|---|---|
| The gate: `last_sent + interval <= now` on the 50 ms grid, then `last_sent = now`. No `% 100`, no counter | **0x27CD52**..0x27CD74 |
| `interval` is the static 0xA2B393C, filled once from the option `-hash_check_interval_in_ms` (object 0xAA19F8), default 100 | 0x27CC4F |
| `last_sent` (`network_client` +0x1F1C50) is written by the constructor, by the gate, and by **the 0xE5 handler, which zeroes it** (0x27B74F). The 0xE8 handler does not touch it | match.md §14 |
| So the phase is **drawn again at every 0xE5** the client takes: first sync, late join, every resync, every repeat of 0xE5 | PROVEN (logic) |
| With frames under 50 ms the phase then stays; it flips when a pass misses its grid step | LIKELY |

What that does: two stock clients are on the same phase (every report
meets) or on **opposite phases** (no tick ever has two hash reports), about
even odds, until one hitches or is resynced. With 3 or more clients at least
two always share a phase.

**The fix in the full patch: one byte.** `mov dword [esp+18h],64h` at
0x27CC4F becomes `…,32h`: the byte at **0x27CC53, `64` → `32`** (file offset
0x26C053). The default interval is then 50 ms, every client reports every
tick, and there is no phase. PROVEN for the bytes and for what reads them
(one writer of the static at 0x27CC60, two readers, both the gate; no
relocation on the immediate); **the effect is not tried live**.
client_patch.md §3.6.

For an exe without it: `-hash_check_interval_in_ms=50` on the command line
(LIKELY syntax, not tried). The key still wins over the patched default.

## Trusted worlds *(server, may move)* (`match.md` §13)

A world may only be used when clients vouch for it.

- Hash reports are keyed by their resolved tick and counted only from
  clients in the simulation. CRCs of different ticks cannot be compared, so
  the rule is **same tick**.
- A tick with one report is not judged. `majority` wants more than half of
  the reports to agree; one report is enough only when one client is in the
  simulation, and with two both must agree.
- **An uploaded world is trusted when two clients named its tick with its
  CRC** (one, when a single client is in the match). An upload is itself a
  report for its tick. `MatchResult::trusted_by` says how many vouched.
- The **first** world starts the match and must be one where nothing has
  happened yet (`SRP_CHECK_INITIAL_WORLD`). Its time is taken from the
  upload (GUESS that it is on the 50 ms grid; if not it is rounded up).
- Later worlds are taken only when asked for, for a tick near the match
  clock (at most 5 s behind, 250 ms ahead).
- Results are read only from a trusted world whose numbers are possible
  (0.21d's plausibility rules). The mode's end (all batteries home; in Team
  Deathmatch no tickets and nobody alive) is taken from such a world, once
  play has begun; the match is then over `END_DELAY_MS` (3000) later.
- A world the reader cannot split is still used to start and resync
  clients, is saved as kind `unreadable`, and gives no results.

**Test bots (`SRP_BOT_ACCOUNTS`, testing only, off by default).** A place
can be a scripted client's (`srp-bot`), which simulates nothing and so sends
no hash and no world. The server is told which accounts are bots; it does
not let the bot pretend. A bot's place is a player (roster, 0xE9, relays)
and never a witness: its reports and uploads are dropped, it is no voter,
no source of worlds and is never resynced for hashes, and it is started
only once a real client simulates. The rules above then count the real
clients only. Two or more real clients: unchanged. **One real client and
bots: one voucher**, the rule of "a single client in the match": acceptable
for a test alone (the only world there is is that client's), and the reason
the setting must never be on for a public server (that client could claim
any plausible result). A match left to its bots for 45 s is over. Details:
`match.md` §19, SRP-026G.md.

**With stock 100 ms clients on opposite phases** no tick has two hash
reports. Worlds are then compared only at an upload's tick, and only when
that tick happens to be one of the other client's: about one upload in two.
The others have one voucher and are not trusted. A client out of step is
found late, and the end by the mode's rule and the results come late, or not
at all with bad luck. The server logs "… on opposite hash phases …" once two
or more clients report and no tick got two reports for 5 s
(`HASH_STARVED_AFTER`), then every 30 s (`HASH_STARVED_LOG`). Only 0xDF
reports count for that, not uploads. With the full patch this state should
not arise. Idea not built: while starved, ask both clients for an upload at
once.

## Resync *(server, may move)*

- A client whose reports disagree with the others' is resynced, each time
  with the next generation. The disagreement must last 1 s of match time
  (`divergence_confirm_ms`), or 3 s on the wall clock; at most one resync of
  a client per 15 s (`resync_interval`), or per 5 s once its last resync
  was agreed with past the load (`resync_after_agreed`,
  `SRP_RESYNC_AFTER_AGREED_SECONDS`), but 15 s again for a client resynced 4
  times within a minute (`RESYNCS_PER_MINUTE`). A resynced client wrong from its
  first reports after the world (within 1.5 s of it going, `LOAD_WRONG_MS`)
  is resynced again at once, twice in a row at most (`QUICK_REDOS`); a
  report that agrees inside that time does not stop it. Its wrong world is
  asked for just ahead of the resync and saved as kind `wrong`, 2 a player
  and 8 a match at most.
- A player back in the match (in the others' worlds since its entry) is put
  at the match clock by its first world, as a resynced client is, and its
  inputs are taken while that world is on its way.
- Who is wrong (`judge`): the majority's CRC is right. With no majority, a
  client known to be out of step is wrong; else the side with the better
  links is right, if it clearly outweighs the rest (`TRUST_MARGIN`); else
  the client that started from a snapshot last is the likely odd one; else
  nobody is named.
- **Out of step by the transport:** a relay, discard or roster change that
  was never acked in its 250 ms marks the client as having missed that tick.
  A lost 0xE9 / 0xEA is also sent once more. An own input the client was
  told to drop (0xF1 / 0xF2) counts as well. Either names the client in a
  tie only for a tick less than 1 s before the disputed one, or before the
  start of the disagreement it is in (`MISSED_RECENT_MS`): an older miss
  changed nothing.
- A resynced client's unjudged reports are dropped; reports count only from
  clients in the simulation (both fixed after tests resynced a healthy
  client).
- A resync or a newcomer starts only from a **fresh** snapshot: ≤ 700 ms old
  (`RESYNC_FRESH_MS`); a newcomer after 3 s of waiting ≤ 1500 ms
  (`FRESH_SNAPSHOT_MS`); after 20 s the newest there is, if the one-byte
  ticks still reach it.
- **A client resynced while it plays** (match.md §30): its own inputs are
  relayed all the way, as any client's; its clock is put at the match clock
  as the 0xE5 arrives and past its newest own input; after its 0xDB it gets
  the replay, and its world goes only once that is acked and from a
  snapshot at most 1000 ms old (`RESYNC_WORLD_MAX_AGE_MS`), and again from a
  fresh one if not loaded 1200 ms after its snapshot (`RESYNC_LOAD_BY_MS`,
  three times at most), or as soon as a player has more inputs after the
  snapshot than the game keeps (27, `INPUT_RING`; 26 for a world still on
  its way). An unchecked upload is started from only when another client
  reported its hash too. An answer to an own input (drop or move) waits from
  its first 0xE5 to its load; after it, it goes if the input is still in the
  game's history, else the sync is done again from a snapshot past it.
- **A newcomer wrong from its first report** (match.md §31): after 1 s of
  reports against the majority its world is asked for and saved (kind
  `joiner`), and it is started once more, once, with 30 s more to catch up.
- A world whose parts were not acked counts as loaded once the client
  reports, for a tick past the snapshot, the majority's hash (its acks name
  only its newest few seqs). A resync's 0xE5 never puts the game's clock
  back: a game whose clock went back made its inputs of those ticks once
  more, with other contents. A relay acked so late that the game moved it
  keeps that client's next resync on a snapshot past it. A tie of two
  reports never ends a standing disagreement. 0xE7 goes again until
  acked. All match.md §30.
- **A game's first world** gets the others' entries (0xE9) since its
  snapshot with it, once its 0xE5 is answered, and those made while it is
  on its way at once: once loaded, a game refuses an entry an event
  horizon (500 ms) behind its present, and every copy after
  (`enter_match` 0x2D15C2). Leaves (0xEA) still go after the load (0xEA
  before it crashed the client). If a roster change would still reach it
  past its horizon, its start is done again from a fresh world, twice at
  most (`LATE_ROSTER_REDOS`). match.md §30.
- **A late-join group** (match.md §32) enters together; a member whose game
  is not connected for 20 s, or whose time runs out, is left out, and the
  rest enter without it if the sides stay within one player (or no further
  apart than before).
- **Never from a world more than 5 s old while others play**
  (`TICK_REACH_MS`): the client's clock would be more than 127 ticks from
  the others' and every relay would be read at a wrong tick. (0.21d, with
  whole times, took any.)
- A player coming back enters for the others first (0xE9); its upload is
  asked for once the clock is 600 ms past the entry.
- The match end (0xEA with the client's own index) is sent again while it
  stays unacked.

## Clocks (`match.md` §4)

- 0.26g clients send no clock: 0xE1 is empty.
- 0xE5 → 0xDB {`u16` gap, `u16` echo, `u16` client ms}; 0xE6 {`u16` a,
  `u16` b} → on the client's next send pass 0xDB {b, a + hold time, client
  ms}. Bytes PROVEN. That `now16 − second field` is the round trip, and that
  the third field is the client's game clock (low 16 bits): LIKELY.
- 0xE7 {`i16`} goes to `floating_timer::set_offset`, as 0.21d's 0x55.
- *(server, may move)* Each live client gets a 0xE6 every second
  (`PING_EVERY`). The lag estimate is the smallest of the last 4 samples; a
  sample more than 5 s off is not used; the correction assumes the client
  moves at 5 % speed (LIKELY, same handler as 0.21d).
  `SRP_CLOCK_CORRECTION=off` stops 0xE7 if the clock field turns out to be
  something else.

## Late inputs (`match.md` §5)

What the client does (PROVEN):
- A relayed input at or before the last resync time is rejected.
- A relayed input that arrives **after the client's horizon is no longer
  dropped: the client moves it** to a tick of its own choosing (log "input
  is moved"), unless it runs with `-reject_inputs`. The tick differs between
  clients, so the worlds still diverge. The server must never relay an input
  that can arrive past a client's horizon.
- A relayed input too far ahead (more than 5 × horizon) is moved back.
- A local input can still be merged into the player's previous own node and
  pull it to the new time (0.21d's "pulled back", unchanged).
- `discard_input` 0x27D820 refuses a tick at or before the last resync, and
  one with no history item before it. `move_input` 0x27D2D0 is new (0xEF).

*(server, may move)* A tick is final 250 ms after it (`HORIZON_MS`).
`SRP_LATE_INPUTS=relay` (default since 2026-10-15): a late input is relayed
at its own tick while every other game in the simulation can still take it
into its history: its lateness under 425 ms less the largest usual round
trip of those games (`late_relay_reach`); the owner gets no answer (it keeps
the input, as all do). Past that, as `discard`. Why: a drop order to a slow
line came past the owner's own history, and the owner kept the input nobody
else had (live, 2026-10-15: 58 of 215 desyncs). A world is still right: a
game uploads its tick 500 ms back, as its hash, and the input reached it
before that. `SRP_LATE_INPUTS=discard` (what 0.21d does): the owner gets 0xF1 /
0xF2, nothing is relayed. `SRP_LATE_INPUTS=move`: the input goes to the
first open tick, the owner gets 0xEF / 0xF0, the others the relay. `move` is
a GUESS at the original server and was never tried on a client.

## Timing *(server, may move)*

| Constant (`v026g/game.rs`, `mod.rs`) | Value | Meaning |
|---|---|---|
| `HORIZON_MS` | 250 | A tick is final this long after it |
| `SYNC_LEAD_MS` | 350 | A client that enters while others play starts its clock this far past its snapshot |
| `RESYNC_ONE_WAY_MAX_MS` | 250 | A resynced client's clock is put this far past the match clock at most (half its round trip) |
| `RESYNC_WORLD_MAX_AGE_MS` | 1000 | A resynced client's world goes only from a snapshot this fresh |
| `REPLAY_ACK_WAIT` | 3 s | A resync's replay not acked in this is done again |
| `RESYNC_LOAD_BY_MS` / `SLOW_LOADS_REDONE` | 1200 / 3 | A resync world not acked this long after its snapshot (plus the ack's way) goes again from a fresh one, this many times a resync |
| `INPUT_RING` / `INPUT_RING_SAFE` | 27 / 26 | A resync world loaded with more inputs of one player after its snapshot (or on its way with this many) goes again from a fresh one, within `SLOW_LOADS_REDONE` |
| `LOAD_WRONG_MS` / `QUICK_REDOS` | 1500 / 2 | A resynced client wrong this soon after its world went is resynced again without `resync_interval`, this many times in a row |
| `MISSED_RECENT_MS` | 1000 | A missed relay or dropped own input names a client in a tie only this close before the disputed tick (or the start of its disagreement) |
| `RESYNC_CLOCK_MARGIN_MS` | 100 | A resynced client's clock goes this far past its own (when measured) and past its newest input seen |
| `RESYNC_WORLD_RESEND_MIN` | 120 ms | A resync world's parts go again every round trip + 50 ms, this at least (0xE8's own pace 500 ms) |
| `JOINER_ABSENT` (`late_join.rs`) | 20 s | A late-join group member not connected this long is left out |
| `ANSWER_REACH_MS` | 450 | An answer held while a client loaded goes after the load for an input this far behind its clock at most |
| `SET_TIME_EVERY` / `MAX_SET_TIMES` | 150 ms / 5 | 0xE5 repeats until 0xDB; then the snapshot goes anyway |
| `RESYNC_FRESH_MS` / `RESYNC_FRESH_WAIT` | 700 / 3 s | How fresh a start snapshot must be |
| `FRESH_SNAPSHOT_MS` | 1500 | After the wait |
| `TICK_REACH_MS` | 5000 | Never start from an older world while others play |
| `RESYNC_REQUEST_GAP` | 1 s | Ask for an upload again this often |
| `REJOIN_UPLOAD_AFTER_MS` | 600 | After a returning player's entry |
| `divergence_confirm_ms` / `_max` | 1000 / 3 s | Disagreement must last this long |
| `resync_interval` | 15 s | At most one resync of a client |
| `resync_after_agreed` / `RESYNCS_PER_MINUTE` | 5 s / 4 | The least time between resyncs once the last one was agreed with; 15 s again after 4 within a minute |
| `hash_settle_ms` | 2500 | Hash reports are judged once this old |
| `refresh` | 5 s | How often a running match asks for a fresh world |
| `CANDIDATE_WAIT` | 3 s | An upload waits this long for the reports that check it |
| `HASH_STARVED_AFTER` / `HASH_STARVED_LOG` | 5 s / 30 s | The "opposite hash phases" warning |
| `PING_EVERY` | 1 s | 0xE6 to each live client |
| `client_timeout` / `loading_timeout` | 10 s / 60 s | A silent client is gone |
| `entry_state` | 4 s | A game's first world is kept on its way this long; a later one 1.75 s (the client's table) |
| `WAITING_RELAY_FLUSH` / `WAITING_RELAY_RESEND` | 50 ms / 150 ms | The others' inputs for a game that waits for its first world: how often they go, and go again unacked |
| `bulk_burst` / `bulk_pace` (`udp-transport`) | 4 / 3 ms | Parts of a split message sent at once, and the time for one more |
| `END_DELAY_MS` | 3000 | The mode rule reports its end this long after |

## Start and end of a match (`modes_maps.md` §10, §11)

- The client's `timelimit_rule_core` (0x318100) starts the 20 s countdown
  when at least one player has entered and
  `popcount(entered) ≥ floor(N × 0.8)`, N = the roster size the server sent.
  The server controls that only through N and its 0xE9 / 0xEA. Size the
  roster to the players really present.
- After 5 minutes of waiting the rule reports the match completed without a
  start. Play lasts 900 s. None of this can be changed from the server.
- There is no "match finished" message. *(server, may move)* The server
  keeps its own copy of the timelimit rule, stepped by the 0xE9 / 0xEA it
  sends and set right from each trusted world; the mode's end comes from a
  trusted world; each client then gets 0xEA with its own index.

## The client patch (`client_patch.md`, `patch_026g.py`)

`srp-026g/client-patches/patch_026g.py <survarium.exe> <out.exe>
[--cave=section|hash-mismatch] [--cert-only] [--bin=<file>]`, or `<exe>
--check`. The source must be the original exe (sha256 checked); every site
is checked against its original bytes, so another build or an already
patched file is refused. The output is another file; the script never
touches the source.

What it writes:

| Patch | Site | Why |
|---|---|---|
| Certificate chain check | 0x12A10 `83 EC 2C` → `31 C0 C3` | Every stock client stops at start: its code-signing certificate ran out on 2015-06-06. In every build |
| Single-instance mutex | 0x124F9 `74 09` → `90 90` | Several clients on one PC |
| Hash interval 100 → 50 ms | 0x27CC53 `64` → `32` | No hash phases (above) |
| Battery rule (modes 2, 3) | 0x313A62 `0F 85 D3 00 00 00` and 0x313B34 `C6 81 60 01 00 00 00` → `90`s | The rule's completion is not in the world a rollback loads, so the replay of the tick with a team's last battery could not put it in, and no container could be used after it (`match.md` §26). Patch revision 3; built and self-tested, its handler run outside the game, and in the players' download since 2026-10-13 12:22 UTC; not watched in a match to a team's last battery yet |
| First-sync hook | 0x27AEA1, 5-byte jump in `on_match_ready` | Upload the initial world before 0xDA |
| Request hook | 0x27A505, 5-byte jump in the 0xF6 case | Upload the event-horizon world when the server asks. It replaces the debug test, so the debug branch can no longer be reached |
| The upload code | 420 bytes, from `snapshot_upload_026g.asm` | Serializes the item as `hash()` does and sends it as 0xE0 chunks; clears "may be joined" on each chunk packet; the pool guard |
| Relocations | entries over rewritten bytes become type 0 (one, at 0x27A507, in the section build) | A rebase cannot corrupt the code |
| PE checksum | recomputed (full builds) | |

Where the upload code goes:
- **`--cave=section` (default, the installed build):** a new 8th section
  `.srp` at VA 0xA425000 (RVA 0xA415000, 0x200 bytes of file). The
  Authenticode blob moves 0x200 bytes towards the end and the security
  directory follows it: the client reads that blob at every start
  (`CryptQueryObject`), but does not compare the file's hash with it.
  Nothing of the original code is lost.
- **`--cave=hash-mismatch` (fallback):** over the dead
  `game_world_core::on_hash_mismatch` 0x325A60. No header change. Use it if
  a client with one more section misbehaves.
- **`--cert-only`:** only the certificate patch; what step 0 installed (the
  file now kept as `survarium_srp.cert-only.bak.exe`).

`selftest_026g.py` builds all three into `srp-026g/target-a4/scratch/` and
checks the PE structure, that every byte outside the intended ranges equals
the original, the relocations, the disassembly of the cave and hooks, the
checksum, the certificate read, the hash interval site and the battery
rule's two sites. All pass. **Not tested by it: the code itself running**
(the battery rule's handler alone was run outside the game,
`client_patch.md` §3.9).

The 0.21d battery patch is not ported: 0.26g serializes the next-battery
time (LIKELY fixed; confirm in a live mode 3 match).

## Settings the match server reads (`SRP_*`)

From `servers/match-server/src/v026g/mod.rs`, read once at start:

| Setting | Default | Does |
|---|---|---|
| `SRP_LATE_INPUTS` | `relay` | `relay`, `discard` or `move` (above) |
| `SRP_CLOCK_CORRECTION` | `on` | `off`: no 0xE7 |
| `SRP_CHECK_INITIAL_WORLD` | `on` | `off`: take a first world that already has statistics in it (for when the reader, not the world, is wrong) |
| `SRP_SNAPSHOT_DIR` | `logs/snapshots` | Where uploaded worlds are saved; `off` or empty for none |
| `SRP_COMPRESS_STATE` | `on` | `off`: 0xE8 goes as it is, not as LZMA2 (match.md §27) |
| `SRP_ENTRY_STATE_SECONDS` | 4 | A game's first world is kept on its way this long (2 to 5) before its sync starts over; less where its snapshot is old, so that the ticks of the inputs relayed meanwhile stay within 127 of the clock the game waits at |
| `SRP_RELAY_PACK_MS` | 15 | Relays and drop orders wait this long (at most 50, 0 = off) for others to share their datagram (transport.md §11.1) |
| `SRP_BOT_ACCOUNTS` | not set | **Testing only.** The accounts that are test bots (read by `srp`, marked by the lobby on a match's places): above, "Test bots" |
| `SRP_PACKET_LOG=<dir>` | off | Every datagram in and out, one file a connection (`crates/udp-transport`). **Set it for every live test** |
| `SRP_BIND_HOST`, `SRP_PUBLIC_HOST`, `SRP_MATCH_PORT` (1236), `SRP_MATCH_BIND`, `SRP_MATCH_PUBLIC_HOST` | 127.0.0.1 | The address (`crates/vostok/src/config.rs`) |

Tests only: `SRP_TEST_LOG`. The queue and the map choice
(`SRP_MATCH_GATHER_SECONDS`, `SRP_MATCH_FILL_PLAYERS`,
`SRP_MATCH_QUIET_SECONDS`, `SRP_MATCH_MODES`, `SRP_MATCH_MAPS`) belong to
the lobby: SRP-026G.md.

## What the first live match must show

In order (the checklist is in the notes' `README.md`, "What to check in the
first live session", steps 5–7):

1. The client starts at all with an 8th section. If not: the hash-mismatch
   build.
2. "player N … connected", "has loaded" in `logs/match.log`.
3. "gave the initial world": the upload code runs, and a 30–69 datagram
   burst arrives whole. Two clients' initial worlds should be identical.
4. "starts from the snapshot", "is in sync": 0xE5 / 0xDB / 0xE8 and the
   generation rule work, and stored 0xE8 loads.
5. No "on opposite hash phases": the 50 ms patch works.
6. "no agreed hash for player N's snapshot … not used" should be rare.
7. The snapshot reader splits a real world (else kind `unreadable`), and
   the timelimit rule is where the reader looks.
8. Clock corrections stay small (else `SRP_CLOCK_CORRECTION=off`).
9. "the mode's rule has its end", then the results.

Still open and only a live match can tell: whether the original server moved
or discarded late inputs; what bit 5 and bit 31 of the action mask are; the
client's flush rate in a match; whether a real client tolerates repeated
plain reliables (E3, F3..F5).

## Evidence tools

- `SRP_PACKET_LOG`: the packet capture (Wireshark is not installed).
- `SRP_SNAPSHOT_DIR`: saved worlds, with their kind (`unreadable` among
  them; `wrong`: a game's own world as it was found out of step).
- `client-patches/check_world_roundtrip_026g.py <survarium.exe> [dumps]`:
  for each saved world, a late joiner and a resynced game load it and save it at
  once under Unicorn, through every patch hook inside the world's load and save.
  Each hook must write back the bytes it loaded; a byte written from the game's
  own objects is a resync that never converges (`client_patch.md`, "World round
  trip").
- `servers/srp/tests/v026g_end_to_end.rs`: the real servers with two
  scripted clients through a whole match. Its clients report every tick's
  hash and skip the countdown, so it proves the servers agree with each
  other and with the notes, not with the client.
- `crates/survarium/src/v026g/match_msgs/client_model`: a strict reader of
  what the server sends, with the state and generation rules.
