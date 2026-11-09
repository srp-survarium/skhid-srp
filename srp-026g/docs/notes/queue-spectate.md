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
  `replay-stream.md`, "Live"), and not of a match with a player who opted out of live watching.
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
- **No cap on spectators:** each costs about one more player (its relays and worlds); add one if
  the host struggles.

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
  closed link ends no match in the game, `match.md` §24; patch 121 ends a spectator's on it),
  then silence.
- **The exe** (patch 121, `client_patch.md` §3.78), for a game with no place of its own only:
  after each world the camera keeps the player it follows (else the first living one), where
  the original looked the local place up and read past the player table; the game builds and
  sends no input for the player it follows (it would drive it in its own world); F6 / F5 (next
  and previous player) step through both teams; F2 (the game's own cam_2 binding, the replay's
  free view) lets go and flies with the game's free-fly camera, kept over a world's load and a
  player's leave until F6 / F5 (from the last one followed) or the next match; match chat
  lines and the followed player's
  death show no kit picker; an 0xEA naming 0xFF ends its match quietly (no results screen),
  back to the lobby.
