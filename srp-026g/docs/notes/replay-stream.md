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
   the opening (the end of the countdown); `place` the roster place the movie wants in view first, 0xFF for anyone.
   Or `player = 0xC100_0000` (`ReplayWatchAsk::MyDeaths`, the results screen's "Watch my
   deaths"): from the asker's first death (4 s before it), its own place in view; the server finds
   its place in the recording, its deaths in the match's events (the history, below): 4 s before
   the earliest its first death can have been (a death known within a window, the window's
   start); with no death kept, it starts where the asker's place is first in view.
2. The lobby answers at once with one `R1` row (`91`, `replay_watch.md` section 2): `R1;preparing;<id 8 hex>;2`, or why not: `off` (no recordings),
   `old_game` (revision below 14), `busy` (the player is in a match or queued, has a session
   already, or `SRP_REPLAY_WATCH_MAX` sessions run).
3. The match server reads the match's newest `.srprec` from `SRP_RECORD_DIR` on a thread of its own
   (nice 19, one at a time; a recording already loaded for another session of that match is
   shared), and judges the asker before converting it: anyone (`SRP_REPLAY_WATCH=all`, the
   default: the owner's decision, every player may watch any replay), or only the players of the
   match and the moderators (`match`); the recording's games of the asker's simulation epoch
   (the `incompatible` rule and refusals of `replay_watch.md`). The conversion is `sur_convert`'s, in memory: no `.sur` file, no token.
4. The movie polls (`3B 00 <REPLAY_START> 01`): `R1;preparing;<id>;1` while it
   is read (2 while waiting), then `R1;watching;<id>;<start ms>`, or the refusal: `not_yours`
   (`match` only), `incompatible` (with the game's message box saying why, once), `gone` (no
   recording), `failed` (unreadable, over `SRP_REPLAY_MAX_RECORDING_MB`, not read within 2
   minutes), `busy`.
5. Read: the lobby sends the game to the match server as a spectator is sent: `85` "found",
   then `82` (match id = the recorded match, team 0, its session). The match server takes its
   0xD8 as the session's (a game with the late-join capability only) and plays it the
   recording through #1285's spectator machine: 0xE3, 0xE4 naming 0xFF with every place of the
   recording, the key frame, then the relays and roster changes at their recorded times.
6. It ends when the game leaves (0xE0, Esc), its link goes, or the recording ends (0xEA naming
   0xFF; patch 121 takes the game back to the lobby). The session is over when its game acks that
   0xEA; the address stays silent the 7 s a game whose 0xEA was lost needs to end its link by
   itself (`SILENCE`), and a session seated meanwhile connects from it at once.

**The start** (`Recording::start`): the replay opens where the action is, at the end of the
match's countdown (`Recording::opening`: the start of the time rule's "in process" in the first key
frame that has it), put there as a seek puts a game (below); the playback note's `start`, the
viewer's bar, starts there too and no seek goes before it. A recording with no countdown's end (no
key frame in process: a bot's or a solo test's edge cases) opens at the first view, the first key
frame after anyone's first entry: a world before any entry has nobody in it. A place asked for
that entered after the opening starts at its own first view (the first key frame after its entry);
a later time asked for starts from the key frame at or before it, put as far past it as the inputs
since allow (as a seek, below).

**The place to follow: the asked one, then the client's.** The server tells the game the place
asked for (a replay's `place`, "my deaths"'s own place, a live watch's `place`: the same ask) once,
in a view note (`52 03 place`, `ServerNote::View`) as soon as the game watches (its first world is
in, so patch 121's camera has picked someone); patch 130 then follows that place, as a player list
click does. None asked (0xFF): no note, patch 121 follows the player it followed last, else the
first living one. A later start (a seek) sends none: the camera keeps whom it follows. The place
also moves a replay's start (the first view after that place entered). Following later (F5/F6, F2,
a player list click) is the exe's own: no control message.

## Controls and notes (`crates/replay-control`)

One encoding for the server and the exe's arena (track B's patch 130), the crate
`replay-control`; times are the match clock (ms, the 0xE5
clock):

- **Game -> server**, a request: 0xE0 (as an upload chunk; resent every 500 ms until acked), 14
  bytes `FF FF FF FF FF FF 52 00 seq op value:u32le`. The game's datagram may keep its multi flag
  (seen live, a 0xDF joined after it), so `udp-transport` finds the 0xE0's end by that head, as it
  does for the resource acks (`FF x6 0|1 00`); the first form, `52 seq op 00 value`, was parsed there
  as an empty 0xE0 followed by an id 0x52, and the datagram dropped. Taken only when `seq` is newer (wrapping
  u8) than the last taken: a resend or a late one is not.
  - op 1 pause: value 1 paused, 0 playing;
  - op 2 speed: percent, 25, 50, 100, 200, 400, 800;
  - op 3 seek: the match time (never before the opening, the end of the countdown: the playback's start).
  - op 4 leave (value 0): the viewer's Back; the server lets the game go as at the recording's end
    (the farewell, an 0xEA naming no place), and patch 121 ends its world in the match's message
    handling. Esc keeps the game's own leave (an empty 0xE0).
- **Server -> game**, notes in an 0xF6 (an 0xF6 `00` stays the upload request, never sent to a
  spectator):
  - the playback, `52 01 revision paused speed:u16 start:u32 end:u32` (start and end: the
    replay's opening and the recording's last record), sent once the game is live (after its first
    world), after each change and after each start again (a seek, a drift); the game takes one
    only when its revision is newer;
  - the events, `52 02 first:u16 total:u16` and up to 64 of `time:u32 kind:u8 actor:u8
    victim:u8` (kind 1 kill, 2 objective; 0xFF no place), as many notes as it takes, once live
    (an empty one for none);
  - the view, `52 03 place`: the place asked to be in view, once live (a live match's spectator
    gets it too, from the Live window's or a friend's ask), only when one was asked.
  - the kits, `52 04 place mask`: which of a place's kits a spectator's chooser may show (a mask below
    `1 << KIT_NUMBERS`). A replay's: each place's playable kits, once, after the events note. A live
    match's: the kits the place has played by the spectator's delayed clock, told when that mask changes.

**The events** are the match history's (`persist::match_events`, `docs/notes/match_history.md`),
read by the match server with the recording (`MatchHistory::events_of_replay` of its file stem,
the replay id; srp hands the match server its accounts): a kill (kind 1, killer and victim), a
death no kill was paired with (kind 1, no actor), a battery found or delivered (kind 2, its
player); captures, scores and the end are not shown. A match is in the history a moment after its
result; there is no backfill (the owner's decision): matches played before the history shipped
have no events, so their replays get an empty events note and "my deaths" starts where the asker's
place is first in view. The session plays the newest recording of the match id.

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

## Live: the matches being played (the Live window)

Track C's Live window (the "Matches now" tab revived), `lobby-server/src/v026g/live.rs`. Each row
is a `91` row at the request's start plus its index.

- **List**: `3B 00 <LIVE_START = 2 240 000 000> <count <= 20>` (polled every 5 s): `L0;<n>;<delay s>`
  (`SRP_LIVE_WATCH_DELAY_S`, so a card can say how far behind a watch is), then
  `L1;<match 8 hex>;<map id>;<mode>;<phase>;<players a>;<players b>;<score a|->;<score b|->;
  <played s>;<spectators>;<watch>`; `phase` 0 made, 1 countdown, 2 in play, 3 over; `watch` 1 may,
  0 full, 2 not watchable (fixed rosters, which take no spectator). A private room's match is not
  listed. The match id is the match server's (the replay's). After the `L1` rows,
  `F1;<match 8 hex>;<place>;<team 0|1>;<name>` for each friend of the asker present in a listed
  match (the Watch hub's pinned cards, the friends list's Spectate; `research/watch-hub-proposal.md`).
- **Roster**: `3D 00 <LIVE_ROSTER_ASK = 0xFFFF_FF06> <match>` chooses the match, then
  `3B 00 <LIVE_ROSTER_START = 2 241 000 000> <count <= 24>`: `L2;<match 8 hex>;<place>;<team>;
  <kills>;<deaths>;<name>` for each place in the match (kills and deaths of its newest trusted
  world).
- **Watch**: the replay session's ask (`0xC0000000 | place << 16 | seconds`) for a match still
  being played (countdown or in play) sends the game to it as #1285's spectators are sent, but
  `SRP_LIVE_WATCH_DELAY_S` (default 30, at most 58: a match keeps 60 s to start from) behind, so nobody calls a friend the positions: answered
  `R1;watching;<match>;0`, `busy` when full (`SRP_LIVE_WATCH_PER_MATCH`, default 4;
  `SRP_LIVE_WATCH_MAX`, default 8, of all matches) or not begun, `not_yours` when not watchable.
  A match no longer played gets its recording, as above.
- **Behind the match** (`game/spectate.rs`, `WatchKind::Live`): the match keeps its trusted worlds
  of the last 60 s (as its inputs); a delayed spectator starts from the newest at least 250 ms
  behind its clock (and fresh enough for the inputs since, 1.5 s) and gets the relays and roster
  changes from the match's lists as its clock reaches them, never as they are played. Queue and
  spectate is the same (`WatchKind::Queued { delay }`, the owner's of 2026-11-09: one rule for all
  live watching, so nobody calls a friend the positions). The delay of a watcher is at least 1 s
  (`LEAST_BEHIND`: nearer, an input on its way would be behind what it has been fed), so a delay
  of 0 is 1 s. A room's queue never ends a Live one. Not yet: a delayed spectator is let go at the match's end with the players, so the last
  delay of the match is not shown; a late joiner who entered within the delay has a place in its
  0xE4 before the world shows it (as a replay's late install).

### Who may be watched

"Allow others to watch my matches live" (the Settings window's Gameplay tab, under "Friends:", ticked by default)
is the account's, kept in `g26_extras.live_watch` (schema step 17: 1 allowed, the default; 2 hidden). Unticked, a
match with that player seated in it, one who left it included until it ends, is not listed, has no roster, is
answered `not_yours` to a watch ask and gives no friend's `F1` row, as a private room's match. Nothing names the
player. The list and the ask read it when they run, so a change applies from then on; spectators already watching
go on. The wire: `lobby.md` 2.16.6.

Not this setting's: finished replays. A room's "Queue and spectate" follows it: its watch of a
match with such a player does not start (`rooms::room_watchers`); one already there goes on.

## Opening a saved file (not built)

Save's `.srpreplay` (`replay_file.rs`) is for a later, offline path. It is to play through the same
machine as a kept recording: the file's `.sur` made into a [`Recording`] (`Recording::from_sur`) and
played by a match server on the player's own machine (the launcher starting it, the game connecting
to it as it connects to a replay session), so patch 130's viewer, its Moments and the kit view carry
over. The `.sur` inside is not meant for the native player (`-match_replay`, `replay_sur.md`): it
has no mode config (chunk 4), and there patch 130 stays out (`role()` is `None` when the client is
not `network_client`), so none of the viewer would. The header's `epoch` (the last simulation change
at or before the file's oldest game) is what that path's "recorded before a game update" is decided
by: the game can compare it with its own epoch, which the exe does not hold yet (`simulation_changes.tsv`
is the server's).

## Settings

| setting | |
|---|---|
| `SRP_RECORD_DIR` | the recordings; sessions are off without it |
| `SRP_REPLAY_WATCH` | `all` (default: anyone) or `match` (the match's players and the moderators); another value is `match` |
| `SRP_REPLAY_MODERATORS` | account ids, comma-separated, that may watch any match |
| `SRP_REPLAY_WATCH_MAX` | sessions at once (default 4: the host has 1 vCPU and 512 MB; each costs about one player's traffic and its recording in memory, a few MB to a few tens) |
| `SRP_REPLAY_MAX_RECORDING_MB` | a larger recording is not read (`failed`; default 24): reading one keeps it in memory with what is made of it |

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
