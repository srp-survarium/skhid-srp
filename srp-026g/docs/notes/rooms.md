# Rooms

The rules the code and the tests follow, the owner's of 2026-10-16 to 10-19. Code: the wire
`survarium::v026g::lobby::rooms`, the model `session::v026g::rooms`, the lobby
`lobby-server/src/v026g/rooms.rs`, the movie `client-patches/rooms/SrpRooms.as`
(`rooms_026g.py`). Contract: `lobby.md` §2.16.3.

- **Only way to PvP:** every PvP match is a room's. The old queue, its level search, the
  Matches tab and its JOIN, the skewed map draw and PvP server bots are gone. Play vs Bots
  and clan-team matches stay (clan matches: a uniform random map, no bots).
- **Making one:** team size 6 v 6 to 10 v 10; players to start 1 v 1 to 3 v 3; max level
  (equipment cap; 0 = any; never below the maker's own gear); the time to start, 30 s to
  1 min 15 s (the 15 s vote and a wait of 15 s to 1 min; shown as the whole); an optional 4-digit PIN; a pool of maps (with modes: Protective device,
  Protective device: chargers, Team Deathmatch; experimental maps addable). Named after its
  maker; never changed once made. No level is chosen: the room's level is its members'
  match level.
- **Play:** goes straight into a room, no search shown: of the open rooms that let the player
  in, the one nearest its level however far (too few online to leave anyone out), then the
  fullest; no squad, friend or clan preference; none: a new room for it (8 v 8, any max level, 30 s, the whole pool, from
  1 v 1). Only a room made by hand has a max level. A running match is
  joined at once. A player with a seat in a match goes back to it.
- **Members:** stay in their room from match to match until they press Leave, the owner
  kicks them (only in a room with a PIN; they may not come back), the room is disbanded,
  or their game is gone for good.
- **Lost (game crashed, killed, link silent; or started anew):** the place is kept for about 90 s (the link's
  30 s + the session's 60 s), the member shown grey (`away`); Play within it goes back to the
  same seat, state kept (Play stays usable while their row says they play the match). After
  it the place is lost: they may join again as a new member. A game quit from its menu leaves
  the room at once.
- **Leave match (its own word, 0xE0; the owner's of 2026-10-19):** takes the player out of
  its room as Leave does (not kicked; it may join again). Joining again while that match is
  played puts it into the same match as a late join (15 s or "Join match") as a new place,
  fresh (new index, zero stats), never its old one; only an account's latest place in a match
  counts (results, rewards, ratings, quests, map plays), the place it left for nothing.
- **Before each match:** once the minimum of players who can play is in, a vote on 3 cards
  (map + weather + mode) for the room's time to start: 15 s and the owner's wait together,
  10 s more after a match (players come back from results first), always whole, a full
  room too. The vote is open until the match starts (the owner's of 2026-10-19: its time
  adds to the wait, nothing is fixed before the start); votes may change; the match starts
  on the card with most votes then, a tie or none drawn at random. Fewer members than the
  minimum stop it; a member changing kit stops nothing. A match that could not be made
  gives the room its wait again, the vote open, its votes kept.
- **Who plays:** a member whose kit can't play (no weapon, too little ammo) or whose
  equipment is over the max level is marked red, counts for nothing, and is taken out of the
  room when the match starts without it (code 13 / 12); it may rejoin.
- **Sides:** drawn at the start, as even as can be (every split by the same count), squads
  together; a room of one squad plays it on both sides.
- **Late joins:** a member who comes during the room's match (by Play, Join, its queue or a
  switch) sees the room first: who is in it and at what levels, to switch or make its own;
  it goes into the match 15 s later, or at once with "Join match". It goes in on the
  weaker side, from its countdown on (the first seconds too) until 30 s (`LOAD_MARGIN`) are
  left of it; one who comes later waits in the same room and plays its next match.
- **In the match (Tab list), to do:** a lost player's row stays grey for the 90 s; one who
  left to the lobby, or is lost past the 90 s, has its row removed; coming back after the
  90 s is a new person (a new place, fresh state).
- **Switch to a lower max level:** a member not playing a match is offered, beside Leave,
  "Switch room", with what it is beside it ("A place free: room for gear up to level N", whose, how full): the open room (no PIN, a place free, not kicked from) of the lowest max
  level that takes its equipment and is lower than its own room's (none set counts as the
  highest); then the fullest. One click joins it, out of the room it was in.
- **Queue:** a full room offers "Queue": from no room only, one queue at a time, PIN asked
  up front. The first who still may come is put in when a place frees (straight into a
  running match); its place shown in the window and the Rooms tab ("Queue 2"). Joining or
  making a room, Play, Leave queue or its game going take it out; a room gone takes its
  queue.
- **Maintenance:** within 30 min of one, no vote or match starts and no room is made; a room
  not playing is disbanded at once, one playing after its match (code 9 to its members and
  queue).
- **Joining from the list:** the Join button, a double-click on a room, or Enter with one chosen
  (Up and Down choose the room above or below)
  (Queue for a full room; the PIN typed first for a private one).
- **Chat:** stays over the Rooms window.
- **Window:** Rooms replaces Map in the top bar ("rooms" in every language; its own icon; locked with Clan and
  Ranking while the lobby loads); the window fills the Character + Inventory area, the
  inventory's header and close button; × and Esc hide it; opening another lobby window hides
  it. The bottom bar's Matches tab is hidden.
- **Sounds:** the game's click on every control; its alert (6) once a vote, 10 s before the
  match starts (the vote's last seconds; the vote runs until the start, 2026-10-19), also with
  the window hidden (not the match start's 15).
- **Site:** the launcher shows rooms and players in rooms; `/api/stats` has `rooms`,
  `in_rooms`, `queued` (in room queues).
- **Fake rooms (local tests only, 2026-10-20):** `SRP_FAKE_ROOMS` makes rooms of
  fake members at start, so one game window tests a full room and its queue;
  `room.sh add|remove` (or `SRP_FAKE_ROOMS_LEAVE_SECONDS`) puts a fake in or
  takes one out. A fake fills a place but is no player: never in a vote or a
  match, never saved (`SRP-026G.md` "Fake rooms").
- **Deferred:** the Rooms list's visibility (members of the selected room, "Full · N in
  queue", header totals; no friend or clan markers): after rooms are solid.
