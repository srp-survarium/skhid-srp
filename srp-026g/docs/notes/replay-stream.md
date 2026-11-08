# Replay sessions: a recording played in the running game

**Version scope:** 0.26g, on top of Queue and spectate (#1285, `queue-spectate.md`). Written
2026-11-07. **Status: server side built and tested with scripted clients; the lobby movie (the
request, the `watching` answer) is not built yet, the exe's viewer is track B's patch 130.** Path A of `research/replay-plan.md`: the server streams a match's recording as a
spectated match, so a replay plays in the running game (lobby -> replay -> lobby, no second
process, no `.sur` download).

The spike (branch `spike/replay-stream`) proved it on a real game on 2026-11-07: lobby -> replay
-> lobby, a seek resynced with 0 mismatches, the end returned the game to the lobby. It needs
#1285's patch 121 (no place of its own, the 0xEA naming 0xFF).

## Flow

1. The movie asks: `3D 00 <player> <match id>` with `player = 0xC000_0000 | place << 16 |
   seconds` (`lobby::ReplayWatchAsk`). `seconds` (u16) is the match time to start at, 0 for
   the first view; `place` the roster place the movie wants in view first, 0xFF for anyone.
   Or `player = 0xC100_0000` (`ReplayWatchAsk::MyDeaths`, the results screen's "Watch my
   deaths"): from the asker's first death (4 s before it), its own place in view; the server finds
   its place in the recording, its deaths in the match's events (the history, below): 4 s before
   the earliest its first death can have been (a death known within a window, the window's
   start); with no death kept, it starts where the asker's place is first in view.
2. The lobby answers at once with one `R1` row (`91`, as History's ask is answered,
   `replay_watch.md` section 2): `R1;preparing;<id 8 hex>;2`, or why not: `off` (no recordings),
   `old_game` (revision below 14), `busy` (the player is in a match or queued, has a session
   already, or `SRP_REPLAY_WATCH_MAX` sessions run).
3. The match server reads the match's newest `.srprec` from `SRP_RECORD_DIR` on a thread of its own
   (nice 19, one at a time; a recording already loaded for another session of that match is
   shared), and judges the asker before converting it: the players of the match and the
   moderators (`SRP_REPLAY_WATCH=match`, the default) or anyone (`all`); the recording's
   games of the asker's simulation epoch (the `incompatible` rule and refusals of
   `replay_watch.md`). The conversion is `sur_convert`'s, in memory: no `.sur` file, no token.
4. The movie polls as for History (`3B 00 <REPLAY_START> 01`): `R1;preparing;<id>;1` while it
   is read (2 while waiting), then `R1;watching;<id>;<start ms>`, or the refusal: `not_yours`,
   `incompatible` (with the game's message box saying why, once), `gone` (no recording),
   `failed` (unreadable, over `SRP_REPLAY_MAX_RECORDING_MB`, not read within 2 minutes),
   `busy`.
5. Read: the lobby sends the game to the match server as a spectator is sent: `85` "found",
   then `82` (match id = the recorded match, team 0, its session). The match server takes its
   0xD8 as the session's (a game with the late-join capability only) and plays it the
   recording through #1285's spectator machine: 0xE3, 0xE4 naming 0xFF with every place of the
   recording, the key frame, then the relays and roster changes at their recorded times.
6. It ends when the game leaves (0xE0, Esc), its link goes, or the recording ends (0xEA naming
   0xFF, then 7 s of silence; patch 121 takes the game back to the lobby). The session is over
   once the match server forgets the game.

**The start** (`Recording::start`): the first key frame after the asked place's first entry
(anyone's if it never entered or none is asked), so the camera has somebody to follow: a world
before any entry has nobody in it. A later time asked for starts from the key frame at or before
it, put as far past it as the inputs since allow (as a seek, below).

**The place to follow: the client picks.** Patch 121 follows the player it followed last, else
the first living one; the server has no message to make a spectator's game follow a place (0xE4's
local place would make the game drive that player). The server honours the place only through the
start (the first view after that place entered); the exe follows it (track B): the movie knows the
place it asked for. Following later (F5/F6, F2, a player list click) is the exe's own too: no
control message.

## Controls and notes (`crates/replay-control`)

One encoding for the server and the exe's arena (track B's patch 130), the crate
`replay-control`; both ways the payload starts `52`, times are the match clock (ms, the 0xE5
clock):

- **Game -> server**, a request: 0xE0 alone in its datagram (as an upload chunk; resent every 500
  ms until acked), 8 bytes `52 seq op 00 value:u32le`. Taken only when `seq` is newer (wrapping
  u8) than the last taken: a resend or a late one is not.
  - op 1 pause: value 1 paused, 0 playing;
  - op 2 speed: percent, 25, 50, 100, 200, 400, 800;
  - op 3 seek: the match time.
- **Server -> game**, notes in an 0xF6 (an 0xF6 `00` stays the upload request, never sent to a
  spectator):
  - the playback, `52 01 revision paused speed:u16 start:u32 end:u32` (start and end: the
    recording's first key frame and last record), sent once the game is live (after its first
    world), after each change and after each start again (a seek, a drift); the game takes one
    only when its revision is newer;
  - the events, `52 02 first:u16 total:u16` and up to 64 of `time:u32 kind:u8 actor:u8
    victim:u8` (kind 1 kill, 2 objective; 0xFF no place), as many notes as it takes, once live
    (an empty one for none).

**The events** are the match history's (`persist::match_events`, `docs/notes/match_history.md`),
read by the match server with the recording (`MatchHistory::events_of_replay` of its file stem,
the replay id; srp hands the match server its accounts): a kill (kind 1, killer and victim), a
death no kill was paired with (kind 1, no actor), a battery found or delivered (kind 2, its
player); captures, scores and the end are not shown. A match is in the history a moment after its
result; older ones only if backfilled. The session plays the newest recording of the match id.

What the server does:
- `pause` stops the clock (nothing more is sent but a 0xE6 ping each second, so the game's 5 s
  silence check never ends the link); the viewer's patch freezes the game's clock.
- `play` goes on from where it stopped, with no new world.
- `speed` scales the clock, so the relays and 0xE5 times; the patch sets the game's time scale.
- `seek t` starts the game again from the key frame K at or before t, its clock put at the
  furthest point <= t it can be: no more than 5 s past K (a tick byte reaches 6.35 s back) and no
  player with more than 26 inputs in between (`INPUT_RING_SAFE`; the game keeps 27). The inputs
  since K go with the world: the game plays K..clock at once on load.
- **Drift, the fallback**: three hash reports in a row whose world is 2 s or more off the
  playback (a game that did not freeze or scale its clock as told) start the game again from
  where the playback is.

Following a player (F5/F6, F2, a player list click) is the exe's own: no request.

## What a session never does

No seat, no match in the registry, no queue, no result, no rating, no reward: the lobby sends the
game with a ticket of the match server's address alone (`MatchHost::replay_ticket`). While a
session is not over (from the request to its end) its player is in none of its room's matches,
nor put into one late (`rooms::room_free`): this replaces PR #1496's 20-minute hold for the
in-client path. Its lobby link has the match's longer timeout while its game is on its way or
watching.

## Settings

| setting | |
|---|---|
| `SRP_RECORD_DIR` | the recordings; sessions are off without it |
| `SRP_REPLAY_WATCH` | `match` (default: the match's players and the moderators) or `all` |
| `SRP_REPLAY_MODERATORS` | account ids, comma-separated, that may watch any match |
| `SRP_REPLAY_WATCH_MAX` | sessions at once (default 4: the host has 1 vCPU and 512 MB; each costs about one player's traffic and its recording in memory, a few MB to a few tens) |
| `SRP_REPLAY_MAX_RECORDING_MB` | as for History (default 24) |
| `SRP_DEV_REPLAY_WATCH=<match hex>[,<seconds>[,<place>]]`, or `<match hex>,deaths` | dev aid: each game that signs in asks for that replay 10 s later, as the movie would |
| `SRP_DEV_REPLAY_CONTROL=<file>` | dev aid: lines `pause`, `play`, `speed 0.25|0.5|1|2|4|8`, `seek <seconds>` to every playback, read every 250 ms and removed |

## How to try it with a real game

The game needs #1285's patch 121. On the dev server, with recordings on and a match recorded
with the same exe (`SRP_RECORD_DIR`, `SRP_RECORD_MIN_PLAYERS=1` for a solo match):

```sh
# with the dev server's usual settings (SRP_GAME_DATA, SRP_DATA_DIR, ...):
SRP_RECORD_DIR=/tmp/srp-local/recordings SRP_REPLAY_WATCH=all \
SRP_DEV_REPLAY_WATCH=<match id hex> SRP_DEV_REPLAY_CONTROL=/tmp/srp-local/replay.control \
  ./target/release/srp
```

Sign in; 10 s later the game loads the replay. `echo "seek 60" > /tmp/srp-local/replay.control`
seeks. Esc or the recording's end returns to the lobby.

## Not known until a game runs it

1. Pause and speed with the viewer's patch: the game's clock frozen by `game::pause` and scaled by
   its time scale (track B); a game without it drifts and is started again (above), and at 2x or
   more its relays run past its clock (a relayed input more than 2.5 s ahead is moved back,
   PROVEN, `match.md` section 5).
2. **Seek** going back in time and minutes forward: LIKELY fine (a spectator out of step and a
   resynced player take the same path); 0 mismatches seen in the spike. UNKNOWN: the HUD's kill
   feed, scores and timers across a backward jump.
3. Bit 31 of an input's actions is cleared by the converter (`sur_convert.rs` `INPUT_FLAG`);
   whether that changes a world online is not known.
