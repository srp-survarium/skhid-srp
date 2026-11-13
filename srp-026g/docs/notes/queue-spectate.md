# Queue and spectate

The owner's of 2026-11-05: "you queue into a match and immediately join it as a spectator", "once
there is a free place, you join in on it", "no need to worry seeing other positions, this is a
dynamic game". Rooms: [rooms.md](rooms.md) (Queue, Late joins). Wire: `lobby.md` §2.16.3.

## The owner's rules

- **Its own button** (2026-11-05: "there should be a queue and spectate button in the room,
  otherwise it is a normal queue"): a full room's footer has "Queue and spectate" beside
  "Queue". Only that button spectates.
- **The plain Queue stays as it is** ("so that people could queue and check their inventory
  etc."): it keeps the player in the lobby (inventory, shop). Play's queuing, a squad's queue, a
  friend's "Queue for room" are plain queues too.
- **In that queue** either button reads the one "Leave queue", as for Queue.
- **Spectators are shown, quietly** ("would be nice to see spectators somewhere in the UI maybe,
  like in dirty bomb"; "shouldn't distract anyone though"):
  - the room's window and the list's panel: one muted line, "Spectators: A, B", that moves
    nothing else when someone starts or stops watching;
  - nothing pops up or animates: no chat line, no kill feed or hud notice, no sound;
  - never as players: no team row, no score, no results.
- **What a spectator sees:** the engine's own spectator view, every player, either team.
- **Detach** (2026-11-05: "I want the detach key yes"): F2 lets go of the followed player and
  flies freely (the game's free-fly camera); F6/F5 follow a player again.

## Picks (simplest; the owner may change them)

- **Behind the match by the Live delay, like every live watcher** (the owner's of 2026-11-09;
  `replay-stream.md`, "Live").
- **When:** at once. The button sends the game straight to the running match as a spectator,
  with no 15 s room view first. A room between matches (vote, wait): the usual queue, and the
  watch starts when its next match starts while the player still waits.
- **Not a match about to end:** no watch starts once the match is ending (its mode's rule or
  result); one watching stays.
- **No kit offer** before Queue and spectate (the plain Queue keeps its preset offer). A kit
  that cannot play is refused as for Queue.
- **A squad** that uses it watches as a pair and goes in together when two places are free (the
  queue's own rule).
- **A free place:** the queue's own rule takes the first who may come; a spectator may, though
  its game is in the match it watches. It is let go (one 0xEA naming 0xFF); its game, back in the lobby,
  goes into the match at once when it asks for its room (no 15 s), as a late joiner: side
  choice, until `LOAD_MARGIN`. Past it, it waits in the room for the next match, as any member.
- **The match ends:** a spectator watches nothing; it is still in the queue (in the lobby), and
  goes in if the end frees places, or watches the next match.
- **Leaving:** the game's own Leave match (0xE0), or its match link lost, leaves the queue (one
  exit). A plain Queue pressed for the same room keeps the place and stops the watch.
- **Not in the Tab list yet:** spectators are not roster places, so no game has their names, and
  the match link has no free message for them. It would take three new parts: a tagged system
  line of the lobby to the match's players, an exe hook in `process_incoming_text_message`
  (0x26B6F0, beside `#mstats`), and a muted line in hud.swf's `PlayerList` while Tab is held.
  Left for the owner to decide.
- **Out of step:** a spectator's hash reports are heard only to judge it: two in a row that the
  players' majority contradicts and it starts again from a fresh world, no more often than a
  player is resynced. They never count for the match (`match.md` section 37).
- **Spectators share the Live window's caps** (`SRP_LIVE_WATCH_PER_MATCH`, 4, and
  `SRP_LIVE_WATCH_MAX`, 8): each costs about one more player (its relays and worlds). A queue
  waiter past the cap stays in the queue without watching and starts when a place frees, the
  oldest account first.

## How

- **No promotion in place: a reconnect.** A spectator's game is an ordinary game whose match
  static info (0xE4) names no local place (0xFF). Which place is the game's own is fixed when its
  players are built (0x27BE5F marks it, the player copies it at construction 0x2D6A14,
  `on_match_ready` 0x27AD7B keeps it once in `network_client`+0x1F1C44); a place added later is
  built as somebody else's in every game. The stock `-spectator` key changes none of this (it
  skips the login, 0x21C0EC, and adds two console commands, 0x219CEB). So a spectator becomes a
  player by loading the match again: the late join's load, as anyone queued today.
- **The lobby** (`rooms.rs` `room_watchers`, one pass): who waits by that button while its
  room's match is played watches it; a watch nobody waits for ends. `play.rs` `send_to_watch`
  sends its game to the match as a seated player's is sent.
- **The seats** (`session::Matchmaking`): a session watches one match with no seat; a watching
  session gets no seat until it watches no more.
- **The match server** (`game/spectate.rs`, `match.md` §37): a spectator is no `Peer` and no
  place; it gets 0xE4 with 0xFF, a trusted world as a newcomer gets one, then every relay and
  roster change. It is sent nothing else (never 0xE9/0xEA of itself, never an answer to an input,
  0xEF..0xF5: its game would index its player table at 0xFF) and nothing of it is taken (inputs,
  kit choices, uploads; its hash reports judge it alone), so it changes nothing a match
  computes: no row in `simulation_changes.tsv`. It is let go by one 0xEA naming place 0xFF (a
  closed link ends no match in the game, `match.md` §24; patch 143 ends a spectator's on it),
  then silence.
- **The exe** (patch 143, `client_patch.md` §3.82), for a game with no place of its own only:
  after each world the camera keeps the player it follows (else the first living one), where
  the original looked the local place up and read past the player table; the game builds and
  sends no input for the player it follows (it would drive it in its own world); F6 / F5 (next
  and previous player) step through both teams; F2 (the game's own cam_2 binding, the replay's
  free view) lets go and flies with the game's free-fly camera, kept over a world's load and a
  player's leave until F6 / F5 (from the last one followed) or the next match; match chat
  lines and the followed player's
  death show no kit picker; an 0xEA naming 0xFF ends its match quietly (no results screen),
  back to the lobby.

## Watching something else from a room or its queue (the owner's of 2026-11-10)

"I want to be able to be in queue for joining the match and spectate other match at the same
time, and then once minimal players are found and room is created I should gracefully
automatically leave [the] game I'm spectating / replay I'm watching and be redirected to a
standard room menu." Asked when: "when the room reaches its minimum players and the map vote
begins"; for a room waiting for players, a room's Queue and Play's queue alike; "straight back,
but with nice non-disruptive ux/ui".

- **Watching is free meanwhile.** A member of a room, or one in a room's queue, may watch a live
  match (the Live window, a friend's Spectate) or a replay; a replay's viewer keeps its place in
  a queue as a live spectator does (`room_fit_of`).
- **The room calls back** (`rooms.rs` `room_recalls`, each tick): a room waiting for players once
  those who can play now and those watching who could would make its minimum, so the vote begins
  when they are back; a room in its vote; a room whose match takes a member now, or will in a
  moment (`room_takes_now`: `late_hold` says yes, or another is on its way in). A private room
  waiting is its owner's to start and calls nobody. One in a queue is called back as a member,
  once the queue has put it in the room.
- **How:** a live watcher is marked (`Matchmaking::recall_watcher`) and a replay's session too
  (`Watches::recall`: over at once while its game is still in the lobby); the match server tells
  the game and lets it go a few seconds later (below; one 0xEA naming 0xFF, as at a recording's
  end). The game comes back to the lobby by
  itself; its room takes it as any member (the vote, a late join).
- **Not begun then:** a watch asked while the asker's room would call back at once is answered
  `busy` (`room_keeps`).
- **In the game** (`WatchWindow.back`, `RoomsWindow.holdWatchReturn`): back from a watch in a
  room or a queue, the next room state decides: a room that called back says so once, with its
  state (the room code 18, `RoomCode::CalledBack`, kept until the game asks: a room that waited
  for players is still waiting at that moment, its vote begins with the one who is back), and it,
  or any room in its vote or its match, shows itself, with
  one line over it for 7 s, "Your room is ready" (`st_srp_rooms_called_back`; no press taken,
  nothing moved); a room still waiting, or no room, gives the Watch window back as it was.
- **Said first, in what is watched** (the owner's of 2026-11-10: "match found nice ui/ux
  indication with micro-countdown is still nice vs abruptly loading back into menu"): the match
  server tells the game once (`replay_control::ServerNote::Recall`, `52 05 <seconds>`) and lets it
  go `RECALL_SECONDS` (5) later: a replay's playback by its own clock (`Playback::recall_at`), a
  live spectator when the lobby's mark is seen (`Matchmaking::recall_watcher`, the match server's
  `Game::recall_spectator`, then `stop_watching`). The exe (patch 144, `on_server_note`) hands it
  to the hud (`root.srp_called_back_words`, the running language's `st_srp_rooms_called_back_in`,
  and `root.srp_called_back(seconds)`), which shows one line under the top of the screen, "Your
  room is ready. Returning in 5 s", counted down each second (`CalledBackNotice`); it takes no
  press. A session whose game is still in the lobby is over at once, with no line.
