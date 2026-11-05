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

## Picks (simplest; the owner may change them)

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
  its game is in the match it watches. Its match link is closed; its game, back in the lobby,
  goes into the match at once when it asks for its room (no 15 s), as a late joiner: side
  choice, until `LOAD_MARGIN`. Past it, it waits in the room for the next match, as any member.
- **The match ends:** a spectator watches nothing; it is still in the queue (in the lobby), and
  goes in if the end frees places, or watches the next match.
- **Leaving:** the game's own Leave match (0xE0), or its match link lost, leaves the queue (one
  exit). A plain Queue pressed for the same room keeps the place and stops the watch.
- **Not in the Tab list:** spectators are not roster places, so no game has their names; a Tab
  line needs a way to send them (see the PR).

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
- **The match server** relays every input to a spectator and takes nothing from it (no input,
  hash vote or world), so it changes nothing a match computes: no row in
  `simulation_changes.tsv`.
- **The exe** keeps the spectator's camera on the player it follows after each world, where the
  original looked up the local place (none) and reset the view; next/previous player keys work
  as in the stock spectator mode.
