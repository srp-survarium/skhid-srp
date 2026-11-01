# Rooms

The rules the code and the tests follow, the owner's of 2026-10-16 to 10-19. Code: the wire
`survarium::v026g::lobby::rooms`, the model `session::v026g::rooms`, the lobby
`lobby-server/src/v026g/rooms.rs`, the movie's window `flash/lobby_menu/src/survarium/rooms` (`RoomsWindow`
and its views `RoomsList`, `RoomsCreate`, `RoomsRoom`; `RoomsWire`, `RoomsText`, `RoomsLook`) and its
parts in the game's classes (`TopMenu`'s tab, `StatusPanel`'s Play, `LobbyMenu`'s Esc and results,
`BottomPanel`, `PlayerRanking`) ([lobby-source.md](lobby-source.md)). Contract: `lobby.md` §2.16.3.

- **Only way to PvP:** every PvP match is a room's. The old queue, its level search, the
  Matches tab and its JOIN, the skewed map draw and PvP server bots are gone. Play vs Bots
  and clan-team matches stay (clan matches: a uniform random map, no bots).
- **Public rooms (2026-10-28):** none at startup. Play uses two equipment brackets:
  up to level 10 (including the starter's internal level 0), and above 10 up to 15.
  It creates the first room of that bracket only when needed. Each public room is
  8 v 8, starts from 1 v 1, uses the standard non-experimental map pool, and keeps
  the existing 15 s vote plus 30 s wait.
  - Play fills an eligible room in its bracket first (nearest level, then fullest).
    When all are full it queues, choosing a shortest queue. Repeated Play keeps
    the same queue position.
  - Six ready Play users waiting across that bracket's queues open another room;
    up to 16 transfer together, oldest first. This repeats with demand: no fixed
    three-plus-one limit. The operator's existing `SRP_ROOMS_MAX` still applies;
    at that limit, or if creation fails, everyone keeps waiting.
  - Explicitly chosen queues do not count toward expansion and never move with it.
    Disconnected games, restarted games not yet told their queue, and unplayable
    loadouts do not count. A changed equipment bracket is routed again.
  - Players up to level 10 may explicitly join a level-15 room through Rooms.
    Play does not put them there automatically. The switch suggestion is only
    from a level-15 room to an available level-10 room the player can enter.
  - No member and no queue: the room disappears. An empty public room with
    waiters survives to admit them, including when everyone leaves after a match.
- **Making one:** players may create private rooms only, with a required 4-digit
  PIN and a chosen map/mode pool. No team-size, minimum, max-level or countdown
  controls. The server refuses a public CREATE request (code 16), including from
  older movies, without changing the player's current room or queue.
  - **Remembered** (the owner's of 2026-10-20: "when you set up your room with the specific
    levels, specific maps and game modes is for it to persist next time"): a room made by
    hand is its maker's next create window, kept with the account in srp.db (restarts,
    reinstalls, other PCs): team size, players to start, max level, time to start, the
    map/weather tiles and the modes. Not the PIN: typed each time (the owner may change
    that). A room Play made keeps nothing. Nothing kept: the defaults (8 v 8, from 1 v 1,
    any max level, 30 s, every mode, the pool without the experimental maps).
  - Fitted when asked: a map or mode no longer offered is dropped; one added later is in
    (an experimental one out); the max level never below the maker's gear; other values
    within today's ranges; nothing of the pool left: the default pool.
  - **Preloaded** (the owner's of 2026-10-25: the window was drawn first and the settings
    applied after; "slick and fast"): the movie asks for them as the lobby comes up (after
    the pool), at every lobby sign-in and whenever the gear level changes, and keeps the
    last answer; New room opens already filled and no answer redraws it. A room made from
    the window (Back pressed meanwhile too) updates that copy as the server keeps it. A
    sign-in to another account in the same game drops the copy at once. None yet (not
    answered so far): the window opens with the defaults and stays so until it is opened
    again.
  - No button puts the defaults back (the owner's of 2026-10-20: "no need for that").
  - **A PIN: no rewards** (the owner's of 2026-10-20: "let's remove rewards for private (pin)
    games"; players farmed silver in 1 v 1 PIN rooms): every match of a room with a PIN pays
    nobody in it anything, late joiners too. No experience, silver (the match's or a quest's),
    reputation, quest progress, daily bonus, drop (silver's dict 214 too), rating or balance
    rating; its results screen takes no drop card (refused and logged: "private match"). It
    takes nothing either (srp's choice: nothing paid, nothing charged): no wear, no ammo or
    consumables used, no autobuy or auto-repair after it. The results screen still opens with
    the match's numbers (winner, rows, scores) and nothing paid. Its map still counts as
    played (feeds nothing now). The lobby knows it when it makes the match (a PIN is the
    room's for its whole life) and tells the match server (`MatchSetup::private`); the result
    carries it to the rewards (`rewards::MatchResult::private`), which log once
    "Match …: private room, no rewards". Rooms without a PIN, clan matches and Play vs Bots
    pay as before. The site's room rules say so; no in-game text (it needs the lobby movie,
    parked).
- **Play:** follows the bracket and scaling rules above, with no old search screen.
  A room's running match is joined through the existing late-join flow. A player
  with a seat in a match goes back to it.
- **Members:** stay in their room from match to match until they press Leave, the owner
  kicks them (only in a room with a PIN; they may not come back), the room is disbanded,
  or their game is gone for good.
- **However they came in, members are alike** (the owner's of 2026-10-31: who came by Play
  stays in the room after the match, "like we did at the beginning", "then there is a single
  pass in code"): Play only picks, makes or queues for the room (above); after that a member
  by Play is a member as one that joined. When the room's match ends every member is back in
  the room for its next vote. This replaces the rule of 2026-10-20 (#254: who came by Play
  left after its match). A room with no members or queue is gone.
- **Lost (game crashed, killed, link silent; or started anew):** the place is kept for about 90 s (the link's
  30 s + the session's 60 s), the member shown grey (`away`); within it the room's own way in
  goes back to the same seat, state kept, as Play does (Play stays usable while their row says
  they play the match): see Late joins. After
  it the place is lost: they may join again as a new member. A game quit from its menu leaves
  the room at once.
  - **Started anew** (the owner's of 2026-10-20: "i was in that weird state where the game was
    not in a room, but i got thrown into a match regardless (twice)"): a new sign-in keeps the
    place, but the room seats it in no match (nor sends it "match found"), and shows it grey,
    until its new game is told the room: its movie asks where it is (at once, again every 2 s
    until answered, then every 5 s outside a room), or it presses Play or Join match. Then it
    plays as any member; one who comes while the match is played sees the room 15 s first. A
    seat it had in the room's match stays its own: it goes back to it after the 15 s, or at
    once by Join match or Play. A new game not told within 90 s of its sign-in loses the place.
- **Leave match (its own word, 0xE0; the owner's of 2026-10-19):** takes the player out of
  its room as Leave does (not kicked; it may join again). It is the only way to leave a match
  on purpose (the owner's of 2026-11-01: "someone can leave on purpose only through leave
  match button in menu", "and that should just kick the player out of the room"): its seat
  is given up at once, so the room's 15 s never take it back, whichever of its words, the
  lobby's or the match's, comes first. Any other way back to the lobby (a crash, a game
  started anew, a dropped link) keeps the member and its seat (Late joins). Joining again while that match is
  played puts it into the same match as a late join (15 s or "Join match") as a new place,
  fresh (new index, zero stats), never its old one; only an account's latest place in a match
  counts (results, rewards, ratings, quests, map plays), the place it left for nothing.
- **After a match (the owner's of 2026-10-20):** the results screen opens over the room's
  window: closing the results shows the room, not the lobby. A player in no room sees the
  lobby after them, as before. The movie has the room of its last state when the results
  come, and opens the window under them (`LobbyMenu.showRoomUnderResults`).
- **The results stay until the player closes them:** any change of the top menu's tab closes
  them (the game's rule, `LobbyMenu.closeVS`), so while they are up the rooms window opens,
  hides and draws under them without touching the tab (`RoomsWindow.show`), and the tab
  follows it as they close (`RoomsWindow.resultsClosed`). Live 2026-10-30: one let in from a
  room's queue as that room's match ended (and its results sent) had them closed by the
  window opening (`scripts/results-stay/check.sh`).
- **Before each match:** once the minimum of players who can play is in, a vote on 3 cards
  (map + weather + mode) for the room's time to start: 15 s and the owner's wait together,
  10 s more after a match (players come back from results first), always whole, a full
  room too. The vote is open until the match starts (the owner's of 2026-10-19: its time
  adds to the wait, nothing is fixed before the start); votes may change; the match starts
  on the card with most votes then, a tie or none drawn at random. Fewer members than the
  minimum stop it; a member changing kit stops nothing. A match that could not be made
  gives the room its wait again, the vote open, its votes kept.
- **The match starts at the room's minimum** (the owner's of 2026-10-20: "threshold is 2
  players though, not 4"; "just let people play, since rules are in rooms"; live, room 5209
  sat in "Awaiting other players" with 2 of 5): the game itself waits for 80 % of the
  match's places; the server starts it with who is in once that can't come (places given up
  by Leave match) or 30 s after the first player is in, if the room's minimum is in with
  one a side. A started countdown runs on whoever leaves. `match.md` §34.
  - The room never makes a match of fewer than its minimum: a member whose game can't
    play in it (an older game started anew) counts for nothing.
  - Fewer than the minimum, or nobody of a side, in or still able to come (everyone left
    too): the match is called off before its countdown, as one that could not be made
    (the vote and the wait again, whole); nothing of it counts (no results, pay, rating,
    quest, wear); its players see the results screen's spinner until they close it.
- **A private room is started by its owner** (the owner's of 2026-10-26: "in private lobbies
  it should be possible to start the game 1vs0"; then no "from N v N", no countdown, no team
  size). Private is a room with a PIN.
  - No team size and no players to start: it takes the most a match has, 20 (10 v 10, every
    map and mode). The create window hides Team size, Players to start, Max level and Time
    to start once a PIN is typed (their values kept for the next open room) and says
    "Private room: up to 20 players; you set the teams and start."
  - No countdown: it waits however many are in. Its owner (its maker, or the member who came
    first after it left) has Start beside Leave; the others read "Waiting for the creator to
    start." Start makes the match at once with whoever can play, 1 v 0 too, on the vote's
    winner then: the 3 cards are there and voted for as in any room, Start ends the vote.
  - **Its owner arranges the teams** (the owner's of 2026-10-26: "the owner should be able to
    arrange both teams (even 1 vs 10), there should be a button to move the player into any
    team"): the room shows its members in two columns, Team 1 and Team 2. Its owner has a
    button on each member, itself too (» on Team 1, « on Team 2), that moves it to the other
    team; any split, 1 v 0 or 1 v 10 to 10 v 10, a team 10 at most. Members choose nothing:
    a newcomer comes on the team with fewer (Team 1 of two alike), the owner moves it.
  - How it looks (the owner's of 2026-10-26, after the first try: "not very good with
    sizing"): the open room's grid of 4 columns of 5 rows, its left half Team 1 and its right
    half Team 2 (a rule between), each filled down then across, "TEAM 1  3 / 10" over it;
    a team's free places on one dim line ("7 open"), not a row each; the move and the kick
    small boxes inside their member's row, each in its own column (the owner's own row has
    the move only). The vote's cards are 20 lower for that line. Renders of it at 1920x1080
    and 1280x720: `scripts/rooms-start/shots.sh`.
  - Start plays the arrangement exactly: no sides drawn, no balance. A squad split by it
    plays as a squad on each team where two or more of it are.
  - The arrangement stays from match to match; a member who leaves is out of it.
  - A member who missed the Start (its game away or started anew, a late join that failed)
    goes in late on the team its owner put it on while that team has fewer than 10 in the
    match, else on the other; its team in the arrangement stays (the review of #527: a 1 v 3
    whose member came 15 s late had become 2 v 2). A newcomer who joins with the PIN during
    the match goes to the side with fewer players, as in any room, and is on that team in
    the arrangement after, while it has fewer than 10 there. No team ever has more than 10.
  - The server refuses (does nothing) a move of anyone but the owner, in an open room, while
    the room plays its match, onto a team of 10, or from an older list (`2 060 020 000`,
    `lobby.md`). The state tells the teams in its `A;<sides>` row.
  - The server refuses (does nothing) a Start of anyone but the owner, in an open room, while
    the room plays its match, or with a maintenance near (`2 060 000 041`, `lobby.md`).
  - After the match it waits for its owner's Start again: no vote or countdown of its own, so
    a lone member left idle plays no match after match.
  - The match server is told a minimum of 1 (`MatchSetup::room_minimum`): no side needs a
    player; called off only with nobody in it (`match.md` §34).
  - It pays nothing, as every PIN room's match ("A PIN: no rewards", above).
  - Its info line and banner say no "N v N" (the banner: members / 20); the list never did.
  - **No max level** (the owner's of 2026-10-26: "private room shouldn't have max level
    too"): whoever has the PIN joins with any equipment (its maker too), by Join, its queue or
    a friend's menu; no join offers a loadout for its gear (only one whose kit can't play,
    as anywhere); nobody is marked red or taken out of it for equipment; no "Switch room"
    out of it. The create window hides Max level with the other sliders (its value kept for
    the next open room); the room's info line, its list row and the facts beside the list
    show no max level (the server tells 0). On the server a private room's settings have no
    max level at all (`rooms::Door::Pin`).
  - Open rooms and Play keep their size, players to start, max level and countdown.
- **The chosen room beside the list** (the owner's of 2026-10-29: it flipped between two layouts):
  one layout through the vote. Before the vote, and while no card leads alone (the server then
  sends map 0), the map's render keeps its place empty and "Map chosen by vote" takes the map's
  line; its members are always in two columns.
- **The room's own view** (the owner's of 2026-10-29): a vote card's mode tag is as wide as its
  name; no hint line beside "Vote for the next map", and none in the bar while it votes (the
  owner's of 2026-10-30: there only a kit or gear warning); while its match is played, the bar says
  the map and mode it is played on (the state's `L` row) under the join hint.
- **Who plays:** a member whose kit can't play (no weapon, too little ammo) or whose
  equipment is over an open room's max level is marked red, counts for nothing, and is taken out of the
  room when the match starts without it (code 13 / 12); it may rejoin.
- **Sides:** drawn at the start, as even as can be (every split by the same count), squads
  together. A squad alone is no match (the owner's of 2026-10-31: a pair is never played
  against itself): an open room whose only members who can play are one squad waits, as
  with one player, until a third can play (`squads.md`).
- **Squads** (of two, the owner's of 2026-10-31): Play and Join take the mate along, a room
  with one place queues the pair, the late join keeps it on one side, and the room's lists
  mark each pair: `squads.md`.
- **Late joins:** a member who comes during the room's match (by Play, Join, its queue or a
  switch) sees the room first: who is in it and at what levels, to switch or make its own;
  it goes into the match 15 s later, or at once with "Join match". It goes in on the
  side with fewer players in the match; of two alike in that, on the side it played on in
  that match before (it left, or its game was gone for good); then on the weaker one (the
  owner's of 2026-10-19: players, then the previous team, then the balance; after a 4 v 8
  by strength), from its countdown on (the first seconds too) until 30 s (`LOAD_MARGIN`) are
  left of it; one who comes later waits in the same room and plays its next match.
  - **One with a seat in the match** (its game started anew, or back from the match it
    closed; the owner's of 2026-11-01: "player should just be able to rejoin through the
    room"): the same 15 s and Join match take it back to its own seat, as Play does, whatever
    the match takes now (none of the reasons below holds it). Live, rev40: such a member saw
    no Join match, only the late-join hint, and got in by Play alone.
- **A side's places in a match being played** (the owner's of 2026-10-31: "we never fully
  delete players"): the side the joiner goes to must stay within the room's size counting
  its players in the match **and** the places kept for players who are not in it now (a game
  that broke: its seat is kept and Play takes it back there, `Matchmaking::held_places`)
  while they are still members of the room; once the room lets them go (their game gone past
  its ~90 s, or Leave) their place is free. A place given up (left by its own word, a
  failed join) does not count. The side with fewer players full that way, the other one; both full: it waits
  ("The match is full."). The roster's 20 places, never used twice, are the match server's
  own limit ("player limit"). Until then a joiner could fill a side a player came back to,
  past the room's size.
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
  - the side it would go to full (the room's size): "The match is full." (4)
  - all 20 lifetime roster places used, even if players have left: "This match has reached its
    player limit. You'll join the next match." (7). The newcomer remains a room member and
    plays its next match; the public room's separate capacity is still 16 members.
  - a maintenance within 30 min: "Server maintenance soon: no new joins." (5)
  - the match ending (its mode's rule, its result): "The match is ending: you'll play the next one." (6)
  - its own kit or gear unfit: the red mark's text, as before a match (no seconds either).
- **In the match (Tab list), to do:** a lost player's row stays grey for the 90 s; one who
  left to the lobby, or is lost past the 90 s, has its row removed; coming back after the
  90 s is a new person (a new place, fresh state).
- **Switch to a lower max level:** a member of a level-15 public room, free to
  join and equipped at level 10 or below, is offered an available level-10 room
  (fullest first). No other better-room suggestions. One click joins it.
- **Queue:** a full room offers "Queue": from no room only, one queue at a time, PIN asked
  up front. The first who still may come is put in when a place frees (straight into a
  running match); its place shown in the window and the Rooms tab ("Queue 2"). Joining or
  making a room, Play, Leave queue or its game going take it out; a room gone takes its
  queue.
  - For the selected room whose queue the player is in, the footer's "Queue" becomes
    "Leave queue", enabled even if the kit cannot play. Cancellation restores the usual
    Join/Queue action. Above the list there is only the queue position and room name,
    with no second Leave queue button.
- **Maintenance:** within 30 min of one, no vote or match starts and no room is made; a room
  not playing is disbanded at once, one playing after its match (code 9 to its members and
  queue).
- **Browsing rooms from a room** (the owner's of 2026-10-22: "it should be possible to browse
  rooms while being in the rooms yourself ... you still stay in the room, but you can look at
  other rooms, if you exit rooms and enter them again you end up in your room, you cannot join
  room you are already in, but you can join other rooms"):
  - A member's window has two tabs at its top, in place of its title: My room (the room's
    view, as before) and All rooms (the list). In no room: the list, no tabs, as before.
  - All rooms is the list as anyone sees it; the member stays in its room meanwhile.
  - Its own room is in the list, its name in the Rooms tab's yellow; Join is off for it.
  - Join on another room: out of its own, as Leave, and into that one; the window shows My
    room again, the new one's. A full room: Join is off (a queue is from no room only).
  - A join refused (a wrong PIN, its gear over the max level, a full room, kicked) leaves it
    in its room, said in the list as outside a room (the state's `code`, `lobby.md` §2.16.3).
  - New room from it: made as from no room, out of the old one.
  - The window, closed and opened again, shows My room.
- **Kit presets** (the owner's of 2026-10-23): a join from the list or Play may first offer one of
  the player's kit presets, "Enter anyway" / "Exit" or "Switch and enter" (it makes that preset active) ([presets.md](presets.md)).
  A full room's Queue offers what its Join would, "Queue" in place of "Enter anyway"
  (the owner's of 2026-11-01).
  Play that would queue offers the other bracket's room with a place in the same way, with
  "Queue" for its first button (the owner's of 2026-10-30; presets.md, "Play that would queue").
- **Joining from the list:** the Join button, a double-click on a room, or Enter with one chosen
  (Up and Down choose the room above or below)
  (Queue for a full room; the PIN typed first for a private one).
- **Joining a friend's room** (the owner's of 2026-10-20: "it should also be possible to rmb
  on friend and join his room, if he is in a room"): a friend's right-click menu has "Join
  room" while that friend is a member of a room ("Queue for room" when it is full); not while
  the friend only waits in a room's queue, not for the player's own room. It joins as the
  list's Join: the Rooms window opens on the list with that room chosen; a PIN room focuses
  its PIN field (typed, then Join); a running match is the usual late join (the room first).
  Only friends see a friend's room (the lobby answers from the asker's own friend list).
- **Chat:** stays over the Rooms window.
- **Room chat** (the owner's of 2026-10-20: "each room has its own chat", "and you should be
  able to type in one", "there should be to the right of [General Chat] [Room Chat]"):
  - Tabs read [General Chat] [Room Chat]: the room's tab right after General, only while in
    a room ("Room Chat", uk "Чат кімнати", ru "Чат комнаты", de "Raum-Chat", pl "Czat
    pokoju"; 84 px at most in the tab's 108 px text field).
  - What a member types in it reaches its room's members, shown with its name as in the
    other channels; another room's members, its queue and players in no room never see it.
  - Members only: a room's queue is not in it; whoever is put in from the queue is.
  - Entering the room any way (make, join, Play, the queue, Switch room, a late join) puts a
    member in it; leaving, a kick, a switch, the room gone (disbanded, nobody left) take it
    out. A switch shows the new room's chat.
  - A newcomer's tab gets the room's last 20 lines (kept with the room; nothing outlives
    it).
  - "X joined the room" / "X left the room" lines, in blue, in the player's language.
  - In a match it gets nothing (the match chat is untouched); after it, the tab is given the
    last 20 lines again, what was said during the match among them.
  - Moderation is the other channels': the lobby's flood limit, an ignored player's lines
    not shown, the game's length limit and its censor.
  - Wire: `social.md` §9.8, `lobby.md` §2.16.3 (2 060 000 030).
- **In a room it shows** (the owner's of 2026-10-19: players did not know they were in one):
  the top bar's Rooms tab, label and icon, is yellow while the player is in a room; the
  Play button reads "Leave room" (in the case of the game's own Play label) in red and takes
  the player out of the room, except when its row says it plays the room's match while it
  is in the lobby (its game started anew): then it is Play, back to its seat. The switch to
  a lower max level ("Switch room") is the game's normal button, untinted (the owner's of
  2026-10-21: neither the whole button yellow nor its art tinted looked good).
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
- **The list's table:** each column as wide as its header and its texts need in every
  language (the longest map name, "Any", a vote's time), each header and cell given its
  column's width (none runs into the next); the name takes the rest and a long one is cut
  with "…"; the next map's weather in small type under its name. The side panel shows the
  map and weather on one line, the mode under them; a card's mode tag widens for a long mode
  name and its map name runs up to the votes. `scripts/rooms-start/check.sh` checks, in every
  language, the list, a room in each phase and hint, and the create window: no text cut by
  its field, no two on one line meeting (`ROOMS OVERLAP` lines), 5% of the width to spare.
- **The list's side panel: who is in the room** (the owner's of 2026-10-20: "down there is
  enough space for a list of players", "below state"; "it should also show a player level in
  room people list preview"; "make sure we properly handle long names"):
  - Below State: the chosen room's members, each with its equipment level (the room view's)
    in a column of its own, then its name. Grey while its game is gone (`away`), red with the
    red mark when its kit or gear cannot play, white for the player itself, gold in the room's
    match. While the room plays, the side of each player in the match (a bar of the side's
    colour; side 1 left, side 2 right; those not in the match where there is room).
  - Up to 20: one column while they fit at 18 a row; two otherwise (always two while it
    plays). Rows share the space (12 to 22 high, the font 12 to 14); the map's render is made
    up to 60 lower for a full room; nothing goes past the panel.
  - **Long names** (up to 63 bytes, Cyrillic and other wide letters): the list gets them whole;
    a name too wide for its place is cut by its width in the panel's own font and ends with
    "…" (the game's bold Calibri has it); the level is never covered. The room view's names
    the same way, its "(owner)" kept whole; the state's rows cut a name past 32 bytes and end
    it with "…". No hover with the full name (the movie's lists have no tooltips).
  - Anyone may ask, of any room, a room with a PIN too: names are no secret (a PIN keeps
    the door, not the names). Asked when a room is chosen, then every 3 s, whatever else the
    window polls (in a room or a queue too). In the room's window the movie asks for its own
    room's, for the whole names of a right-click there.
  - **Friends and clan:** not marked. The owner's: only "in exactly the same way it would
    have been done before rooms by original developers", and the original game marks no
    friend or clan member in any of its player lists (the results list: the player yellow
    0xF1E545, its squad green 0xB3EA97, the enemy squad's icon red; the squad list: the
    leader yellow; the friends and clan lists, the rankings and the lobby chat: no friend or
    clan colour; a clan shows only as its "[tag] " before a name in chat and in the friends
    list). Sources: the 0.26g movie's `PlayerListRenderer.updateData`,
    `SquadListRender`, `FriendListRender.updateText`, `ClanListRenderer`,
    `RankingItemRenderer`, `LobbyChat.get_styled_text`, `Colors`; `VictoryScreen.showContextMenu`
    hard-codes "not a friend".
- **A player's equipment from the rooms' lists** (the owner's of 2026-10-20: "after match in
  match overview you can rmb to see the players equipments", wanted in the rooms' lists too):
  a right-click on a member, in the list's side panel or in the room's window, opens the game's
  right-click menu with "Show player's information" and the game's own window with its kits,
  as the Rankings do (`player_gear.md`). The results screen itself opens that window on a
  double-click (its right-click menu has no such item). Any member, online or not. Only a
  member of a room now is answered by account (`GEAR_BY_ACCOUNT`): an account in no room
  gives nothing, so account ids cannot be walked for anyone's kits. A place of the room's
  window from an older list (a member came or went since) gives nothing. The window's title is
  the whole name (from the room's member rows), not the state's cut one.
- **Player actions in rooms and rankings:** the same menu also offers Add/Remove friend,
  the two chat mute entries ([chat-mute.md](chat-mute.md)), Send message, and Invite to squad
  when the local player leads a squad with space and the target is not already in it. Your own
  entry only offers information. The room UI harness checks the menu.
- **Deferred:**
  - **The list's counts** (the owner's of 2026-10-18: after rooms are solid): "Full · N in
    queue" on a full room, and totals in the list's header ("12 rooms · 87 players · 9
    waiting").
  - **A countdown in the last 10 s before a room's match** (the owner's of 2026-10-20, waiting
    for the owner's pick of a sound):
    - "for the last 10 seconds, can we have some count down timer", rising in pitch; "we need
      sounds from game, not something random". The candidate: 0.46's `ui_sounds/countdown_start`
      (its UI sound 20, 4.25 s of ticks; the owner: "we can most likely use that"). Mixes of
      game sounds were made to listen to (pips, rising pips or clicks, with or without
      `countdown_start` and a chime).
    - The owner: "if we don't have chime at the end, we should have it when match starts to
      load". The game plays the chime (UI sound 15, `match_selection_complete`) in
      `LobbyMenu.set_mm_sync_state`, from `lobby_menu::set_match_making_status` (0x2252D0) on
      0x85 status 1 place 0 ("match found"). The lobby sends that to a room's players too when
      their match is seated (`play.rs` `queue_status`, `waiting(0, 0, now)`), so it LIKELY
      still plays with rooms (the owner: "it used to play on search, not sure this happens
      still with rooms"): to check in the game.
    - How: the new sounds into `gameplay/ui_sounds` (ids 19 and up) by a content edit; the rooms
      window plays them by id (`sound_play`), driven from one end time each frame; a big 10...1
      over the lobby; stopped at once when the vote stops.
  - **Some of a match's chat in the Room Chat tab** (the owner's of 2026-10-21: "match messages
    should be preserved and shown in room chat (not all, but some)"): which lines (for example
    all-chat lines, not team or system lines) to decide with the owner.
