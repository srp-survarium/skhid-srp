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
  - **Remembered** (the owner's of 2026-10-20: "when you set up your room with the specific
    levels, specific maps and game modes is for it to persist next time"): a room made by
    hand is its maker's next create window, kept with the account in srp.db (restarts,
    reinstalls, other PCs): team size, players to start, max level, time to start, the
    map/weather tiles and the modes. Not the PIN: typed each time (the owner may change
    that). A room Play made keeps nothing. Nothing kept: the defaults (8 v 8, from 1 v 1,
    any max level, 30 s, every mode, the pool without the experimental maps).
  - Fitted when the window opens: a map or mode no longer offered is dropped; one added
    later is in (an experimental one out); the max level never below the maker's gear now;
    other values within today's ranges; nothing of the pool left: the default pool.
  - No button puts the defaults back (the owner's of 2026-10-20: "no need for that").
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
- **After a match (the owner's of 2026-10-20):** the results screen opens over the room's
  window: closing the results shows the room, not the lobby. A player in no room sees the
  lobby after them, as before.
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
  side with fewer players in the match; of two alike in that, on the side it played on in
  that match before (it left, or its game was gone for good); then on the weaker one (the
  owner's of 2026-10-19: players, then the previous team, then the balance; after a 4 v 8
  by strength), from its countdown on (the first seconds too) until 30 s (`LOAD_MARGIN`) are
  left of it; one who comes later waits in the same room and plays its next match.
- **A late join that fails** (its game did not come, left, or did not catch up; the match
  server lets it go, `Matchmaking::join_failed`): the member waits in its room again, with
  its `J` row, and goes in 15 s later as a new place (live, 2026-10-19, room 2168: it stayed
  marked in the match and waited out all of it).
- **Why it cannot go in now** (the owner's of 2026-10-20: players see why): the window's
  banner shows one line instead of the 15 s (`J;-1;<why>`, `lobby.md` §2.16.3); once the match
  takes players again the seconds are back, and one whose 15 s are over goes in at once.
  - another joiner on its way in (one at a time): "Another player is joining the match; you go in after." (1)
  - a game of the match loading (at its start, as long as it takes; one that came again,
    30 s at most: `match.md` §33):
    "A player's game is loading; joining opens in a moment." (2)
  - 30 s or less left: "Under 30 s left: you'll play the next match." (3)
  - the side it would go to full (the room's size), or no place left in the match: "The match is full." (4)
  - a maintenance within 30 min: "Server maintenance soon: no new joins." (5)
  - the match ending (its mode's rule, its result): "The match is ending: you'll play the next one." (6)
  - its own kit or gear unfit: the red mark's text, as before a match (no seconds either).
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
- **In a room it shows** (the owner's of 2026-10-19: players did not know they were in one):
  the top bar's Rooms tab, label and icon, is yellow while the player is in a room; the
  Play button reads "Leave room" (in the case of the game's own Play label) in red and takes
  the player out of the room, except when its row says it plays the room's match while it
  is in the lobby (its game started anew): then it is Play, back to its seat. The switch to
  a lower max level ("Switch room") is yellow.
- **In a room's queue it shows the same way** (the owner's of 2026-10-20): the Rooms tab is
  yellow; Play reads "Leave queue" in red and takes the player out of the queue.
- **Window:** Rooms replaces Map in the top bar ("rooms" in every language; its own icon; locked with Clan and
  Ranking while the lobby loads); the window fills the Character + Inventory area, the
  inventory's header and close button; × and Esc hide it; opening another lobby window hides
  it. The bottom bar's Matches tab is hidden.
- **Sounds:** the game's click on every control; its alert (6) once a vote, 10 s before the
  match starts (the vote's last seconds; the vote runs until the start, 2026-10-19), also with
  the window hidden (not the match start's 15).
- **Site:** the launcher shows rooms and players in rooms; `/api/stats` has `rooms`,
  `in_rooms`, `queued` (in room queues).
- **Deferred:** the Rooms list's visibility (members of the selected room, "Full · N in
  queue", header totals; no friend or clan markers): after rooms are solid.
