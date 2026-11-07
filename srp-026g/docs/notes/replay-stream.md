# Replay sessions: a recording played in the running game

**Version scope:** 0.26g, on top of Queue and spectate (#1285, `queue-spectate.md`). Written
2026-11-07. **Status: server side built and tested with scripted clients; the lobby movie (the
request, the `watching` answer) and the exe (the controls, following the asked place) are not
built yet.** Path A of `research/replay-plan.md`: the server streams a match's recording as a
spectated match, so a replay plays in the running game (lobby -> replay -> lobby, no second
process, no `.sur` download).

The spike (branch `spike/replay-stream`) proved it on a real game on 2026-11-07: lobby -> replay
-> lobby, a seek resynced with 0 mismatches, the end returned the game to the lobby. It needs
#1285's patch 103 (no place of its own, the 0xEA naming 0xFF).

## Flow

1. The movie asks: `3D 00 <player> <match id>` with `player = 0xC000_0000 | place << 16 |
   seconds` (`lobby::ReplayWatchAsk`). `seconds` (u16) is the match time to start at, 0 for
   the first view; `place` the roster place the movie wants in view first, 0xFF for anyone.
2. The lobby answers at once with one `R1` row (`91`, as History's ask is answered,
   `replay_watch.md` section 2): `R1;preparing;<id 8 hex>;2`, or why not: `off` (no recordings),
   `old_game` (revision below 14), `busy` (the player is in a match or queued, has a session
   already, or `SRP_REPLAY_WATCH_MAX` sessions run).
3. The match server reads the match's `.srprec` from `SRP_RECORD_DIR` on a thread of its own
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
   0xFF, then 7 s of silence; patch 103 takes the game back to the lobby). The session is over
   once the match server forgets the game.

**The start** (`Recording::start`): the first key frame after the asked place's first entry
(anyone's if it never entered or none is asked), so the camera has somebody to follow: a world
before any entry has nobody in it. A later time asked for starts from the key frame at or before
it, put as far past it as the inputs since allow (as a seek, below).

**The place to follow: the client picks.** Patch 103 follows the player it followed last, else
the first living one; the server has no message to make a spectator's game follow a place (0xE4's
local place would make the game drive that player). The server honours the place only through the
start (the first view after that place entered); the exe follows it (track B): the movie knows the
place it asked for. Following later (F5/F6, F2, a player list click) is the exe's own too: no
control message.

## Controls: the game's 0xE0

A replay session's game sends a control as 0xE0 alone in its datagram (as a patched game sends
an upload chunk), 8 bytes:

| bytes | |
|---|---|
| 0..2 | `FF 52` (`ff 'R'`): no upload chunk (more than 8 bytes) or install ack (`ff ff`, 20 bytes) starts so |
| 2 | sequence number, the game's own, wrapping; one equal to the last taken is a resend, ignored |
| 3 | op: 1 pause, 2 play, 3 speed, 4 seek |
| 4..8 | u32 LE argument: speed in percent (50, 100, 200, 400; others ignored), seek in match ms; 0 otherwise |

- `pause` stops the clock (nothing more is sent).
- `speed` scales the clock (and so the relays).
- `seek t` starts the game again from the key frame K at or before t, its clock put at the
  furthest point <= t it can be: no more than 5 s past K (a tick byte reaches 6.35 s back) and no
  player with more than 26 inputs in between (`INPUT_RING_SAFE`; the game keeps 27). The inputs
  since K go with the world: the game plays K..clock at once on load.
- `play` after a pause is a seek to where it stopped (the game's clock ran on meanwhile).

The exe still has to freeze and scale its own clock for pause and speed (below): the server's
side only stops or scales what it sends.

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
| `SRP_DEV_REPLAY_WATCH=<match hex>[,<seconds>[,<place>]]` | dev aid: each game that signs in asks for that replay 10 s later, as the movie would |
| `SRP_DEV_REPLAY_CONTROL=<file>` | dev aid: lines `pause`, `play`, `speed 0.5|1|2|4`, `seek <seconds>` to every playback, read every 250 ms and removed |

## How to try it with a real game

The game needs #1285's patch 103. On the dev server, with recordings on and a match recorded
with the same exe (`SRP_RECORD_DIR`, `SRP_RECORD_MIN_PLAYERS=1` for a solo match):

```sh
# with the dev server's usual settings (SRP_GAME_DATA, SRP_DATA_DIR, ...):
SRP_RECORD_DIR=/tmp/srp-local/recordings SRP_REPLAY_WATCH=all \
SRP_DEV_REPLAY_WATCH=<match id hex> SRP_DEV_REPLAY_CONTROL=/tmp/srp-local/replay.control \
  ./target/release/srp
```

Sign in; 10 s later the game loads the replay. `echo "seek 60" > /tmp/srp-local/replay.control`
seeks. Esc or the recording's end returns to the lobby.

## Not known until a game runs it (predictions)

1. **Pause.** The game's clock is not ours: 0xE5 sets it (`game::set_time_in_ms` 0x1EE4A0) and it
   runs on; `move` 0x2D2E40 plays 50 ms ticks up to it (`client_patch.md` "What depends on the
   factor"). So while paused the world goes on with no new inputs, each player on its last input
   (LIKELY: its actions mask held, so a runner keeps running), and the camera follows that drift.
   Hence `play` = resync. A real freeze needs an exe patch: `game::pause` 0x1ED5C0
   (inferred-high) on the control, or the timer's speed (below) at 0.
2. **Speed.** Not with a stock game. Its clock runs at 1x; only 0xE7 steers it, at
   1 +- `time_floating_factor` (0.3 from revision 8, at most 0.5; `client_patch.md` `set_offset`
   0x1EE9E0), so 0.7x..1.3x, never 0.5x/2x/4x. At 2x the relays run ahead of the game's clock by
   1 s a second: past 2.5 s a relayed input is moved back (PROVEN, `match.md` section 5), so the
   world goes wrong after about 2.5 s. What would work: an exe patch setting the game timer's speed
   (`floating_timer` +0x20, `client_patch.md` 0x147110) along with the control it sends.
3. **Seek** going back in time and minutes forward: LIKELY fine (a spectator out of step and a
   resynced player take the same path); 0 mismatches seen in the spike. UNKNOWN: the HUD's kill
   feed, scores and timers across a backward jump.
4. Bit 31 of an input's actions is cleared by the converter (`sur_convert.rs` `INPUT_FLAG`);
   whether that changes a world online is not known.
