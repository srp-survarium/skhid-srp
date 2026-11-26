# The desync capture: both sides' worlds at the split, and what differs

`servers/match-server/src/v026g/game/desync_capture.rs` (the capture), `world_diff.rs` (the diff),
`src/bin/srp-world-diff.rs` (the same diff offline). Since 2026-11-26.

The owner's rule: the moment games disagree, the server asks one game from each side for that tick's
world, so every live desync comes with the first field that differs.

## Why

Live, 2026-11-25, match 0x421b579c: the games split 7 against 7 at tick 270700. No side was a
majority, so nothing was asked for (the earlier split capture, `capture_split`, needed a majority),
and the first worlds kept from both halves were of 279650, 9 s later, when only consequences
differed. The worlds games upload on their normal cadence (`checked`, `rejected`, `unchecked`) and at
a resync (`wrong`) are seconds past a split.

## What a game can give

- The server learns of a split from the games' hash reports (0xDF, `match.md` §1.1): each game
  reports the CRC of its event-horizon history item, once per tick (50 ms, client patch §3.6), as its
  event horizon passes it.
- The request for a world is 0xF6 with one payload byte (`ServerMessage::UploadRequest`,
  `client_patch.md` §3.4). The game's handler (`snapshot_upload_026g.asm`, `cave_request`) **ignores
  the payload**: it uploads `game_world_core::event_horizon_history_item` (0x2D0B30) of the moment it
  handles the request. That function (read 2026-11-26) returns the newest history item whose time is
  at most now less the horizon (`[core+18FC4h] - [core+18FB4h]`), walking back from the newest
  (`[core+2FC0h]`) by +0xC0BA8. There is no message that names a tick.
- The history holds (max_ping + 250) / 50 + 1 items (11; 128 with `-debug_hash_mismatch`, 0x2CBFDC):
  the horizon's span. The horizon item is the oldest a game keeps, so the tick a report names has
  left the game by the time the report reaches the server, and a request could not name it anyway.
- So the server cannot get the world of the exact first disagreeing tick. It gets the nearest it
  can: it asks as soon as it sees the split, and times the two requests so that both games upload
  the same tick, the first both can still give (`aim_tick`, as the hash-starved requests are timed).
  That tick is about the slower game's round trip past the split, plus up to a tick.

### The exact tick would need an exe patch (proposed, not made)

A small client patch could answer "your world at tick T" from a ring of the worlds it hashed:
`game_state_history_item::hash` (0x325150) already serializes the horizon item into its scratch
buffer for the CRC; a hook after the serialize (0x3251D7) would copy those bytes (about 20 KB, at
most 0xF000) with their item time into a ring in the `.srpx` arena (16 slots, 0.8 s, a few hundred
KB at real sizes); 0xF6 with payload `01 <tick>` would upload the slot of that tick through the
existing chunk sender, `00` keeping today's meaning (an older game ignores the payload and uploads
its horizon world, which the server tells by the item time). Not implemented: it changes the exe and
waits for the owner's word.

## The capture

- **When.** At each hash report the server keeps (`Game::hash`), the reports of its tick are looked
  at (`watch_desync`). Two CRCs at a tick start a split (`Episode`), unless the games are
  installing a new place (their worlds differ by it until each has it). The split ends at a tick
  every game in the simulation reported the same for; until then nothing more is asked for.
- **Whom.** The reports of the tick, by CRC: the two largest sides, and of each the game with the
  shortest round trip that is patched, live, no test bot, not entering, and has no request of its
  own on the way or waiting (`ask_at`, `asked`). With no such game on a second side yet, the next
  report tries again, for 500 ms (`ASK_REACH`); then the log says the split was missed.
- **Timed for one tick.** Each game's upload offset (where its event horizon is against the match
  clock, a way on): from its earlier uploads where it gave any (`upload_offset`), else its round trip
  less the least lag of its last 20 reports (`ReportLags`). The requests go when each game, if its
  offset is that, handles them in the middle of the same tick (`aim_tick`); with no such tick within
  a second, both go at once.
- **How often.** Once a split, 4 a match, 20 s apart.
- **It only looks.** Both requests are `Wanted::Look(WrongWorld::Capture)`: the worlds that come are
  checked as every upload is (a refused one is dumped `unloadable`, as before) and then only saved:
  no vote, no hash report, no candidate, no world anyone starts from. No game that another request
  waits on is asked. The resync of a game out of step is judged and done exactly as without it
  (`check_hashes`: 2.5 s settle, 1 s confirm), well after the capture's upload (under a second). The
  only change elsewhere: the majority game's world that `capture_split` asked for was a vote; the
  capture's is not (the periodic requests make the votes, as they did).
- **A request close to another.** The capture prefers the best-linked games, which the periodic
  requests for a world to start others from prefer too. A game handles requests in order; one
  asked for a vote while its capture upload is on its way gives the capture's world as the vote's
  (the server keeps one request a game, `asked`): a world of the present, checked as any, so no
  resync waits; the capture is then written with the other side's world alone. As the earlier
  split capture and the world of a game out of step (`ask_wrong_world`).
- **No shared tick, no capture.** Stock clients on opposite hash phases never report the same tick
  (`check_hash_phases`): no split is seen by the reports, and nothing is asked.
- **Written** when both worlds came, or 4 s after the request with what came (a full packet pool,
  a refused world or a game gone sends none), or when the match closes.

## Where the captures are

`<SRP_SNAPSHOT_DIR>/captures/match026g_<match id>_<split tick>_c<n>_r<run>/` (live:
`/srv/srp/data/snapshots/captures/` in the srp26 container's data volume), one folder a capture:

- `match026g_<id>_<tick>_p<place>_capture_r<run>.bin`: each side's world (a body, as every dump:
  `srp-worlds`, `srp-world-check --worlds`, the `snapdiff` example read it);
- `capture.json`: the summary and the diff.

Captures over `SRP_CAPTURE_MAX_MB` (default 200, about a thousand) together: the oldest folders go
after each capture. The rest of `SRP_SNAPSHOT_DIR` (the dumps of every kind) has no cap.

The match log has one line a capture (warning; this one from the test of a 2-against-2 split,
`a_split_with_no_majority_has_a_world_of_each_side_saved_at_one_tick`):

```text
Match 0x11a72501: desync capture 1 at 12150 ms (seen 12660 ms): groups f3a69efb:[0, 1] 1fdcccb0:[2, 3];
worlds player 1 at 12200 ms, player 3 at 12200 ms, 50 ms after the split; first difference: bullets at +0,
header.current_time_ms 0 vs 7; <SRP_SNAPSHOT_DIR>/captures/match026g_11a72501_12150_c1_r1795693343
```

(one line in the log; broken here). Of the recorded live pair below, the line would end `first
difference: place 6 state at +426, rest.linear_horizontal_speed 2.0735257 (0x4004b4a5) vs 1.9975309
(0x3fffaf18)`.

`NOT the same tick` after the worlds' times says the timing missed (each game's own tick is in
`capture.json`); `no diff (a world did not come)` that one side sent nothing.

## Reading `capture.json`

| field | |
|---|---|
| `split_tick` | The first tick the games are known to disagree on: the first of the run of disagreeing ticks that holds `first_seen_tick` (a report of an earlier tick can come later) |
| `first_seen_tick`, `seen_at_ms` | The tick whose reports showed the split, and the match clock then |
| `groups` | Each side: the CRC its games reported for `first_seen_tick`, and its places, the largest first |
| `worlds` | Each game asked: its place, what it reported, its round trip, the tick its request was timed for (`aimed_tick`), the tick it gave (`tick`), the world's CRC and file |
| `after_split_ms`, `same_tick` | How far past `split_tick` the worlds are (the later one), and whether both are of one tick |
| `first_difference` | The first difference with a field named, in one line |
| `diff.differences` | Every part of the body that differs, in the body's order |

Each difference:

- `part`: `mask` (a place in one world only), `place N state`, `place N animation`, or a section
  (`bullets`, `statistics`, `objects`, `physics`, `rules`, `effects`); `body` when one world does not
  walk;
- `at` (in each body), `in_part`: its first differing byte; `lengths`: the part's length in each;
- `field`: the first object of the part and the field of it that differ, as srp-validate's typed
  views read them (the world format's one definition, `packet-validation.md`), read with the
  match's own roster as the server checks its uploads (`world_validation::with_contexts`):
  - a place's state: `head`, `effect N`, `hit effect N`, `active objects`, `stamina`,
    `inventory header`, `item N` (its slot's owner and body: a weapon's state machine, a medkit...)
    and `rest` (the body, the damage model, the controller, the recent hits, the carry);
  - the sections: `bullet N` and `header`; `place N`, `battery N`, `tail`, `weapons` of the
    statistics; `object N` of the game objects (the map's list); `rules`; `effects`;
  - `path`: the field's names in the view (the game's own, `m_` dropped), indices for list entries;
    `values`: each world's value, an f32 as its value and bits. Offsets (`at`) are left out: they
    move with any length before them.
  - `items (the kits unknown)`: equal to the inventory header, and the kits were not known (only
    `srp-world-diff` without a recording).

What the views do not read is given by its byte only: an animation's bit stream, the physics
section. A place's state and its animation usually differ together; the state's field is the one to
read.

## Offline

```text
srp-world-diff [--data <server-game-data.db> --recording <rec.srprec>] <a.bin> <b.bin>
```

The diff of any two dumps of one tick (a capture's, a `split` and its `checked` of before, two
uploads `srp-world-check --worlds` wrote), as JSON, its first difference on stderr. With the
match's recording and the release's server game data its rosters read with, every part is read as
the server does; without, each place to its inventory header and the sections that need no match.

The test `two_recorded_worlds_of_a_split_name_the_field_the_games_first_differ_in` (`world_diff.rs`)
is match 0x421b579c's split capture of tick 221600 (`tests/data/desync_221600_{a,b}.bin`): place 6's
`rest.linear_horizontal_speed` (`m_linear_horizontal_speed`).
