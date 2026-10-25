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
- **Making one:** team size 6 v 6 to 10 v 10; players to start 1 v 1 to 3 v 3; max level
  (equipment cap; 0 = any; never below the maker's own gear); the time to start, 30 s to
  1 min 15 s (the 15 s vote and a wait of 15 s to 1 min; shown as the whole); an optional 4-digit PIN; a pool of maps (with modes: Protective device,
  Protective device: chargers, Team Deathmatch, Research once its maps are in the data;
  experimental maps addable). Named after its
  maker; never changed once made. No level is chosen: the room's level is its members'
  match level.
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
- **Play:** goes straight into a room, no search shown: of the open rooms that let the player
  in, the one nearest its level however far (too few online to leave anyone out), then the
  fullest; no squad, friend or clan preference; none: a new room for it (8 v 8, any max level, 30 s, the whole pool, from
  1 v 1). Only a room made by hand has a max level. A running match is
  joined at once. A player with a seat in a match goes back to it.
- **Members:** stay in their room from match to match until they press Leave, the owner
  kicks them (only in a room with a PIN; they may not come back), the room is disbanded,
  or their game is gone for good. Except who came by Play:
- **Who came by Play leaves after its match** (the owner's of 2026-10-20: "people who pressed
  Play, rather than entering a room, leave it after the match ends; then next time they'd have
  a chance to land at their level; whoever explicitly entered a room stays in it"; "this is a
  good idea to get people to not have to click the yellow button"):
  - By Play: Play put it into an open room, or made a new room for it (its maker too: it
    pressed Play, it made no room). Chosen: a room made by hand (its maker), Join from the
    list (PIN too), its queue, "Switch room".
  - When the room's match ends, each member by Play who played it leaves the room, as by
    Leave (not kicked, no penalty; it may join again): it sees its results, then the lobby,
    and its next Play takes the room nearest its level again. Chosen members stay.
  - One by Play that did not play the match (it came too late to go in) stays, plays the next
    one and leaves after it: Play is for a match, not for a trip back to the lobby.
  - Joining the room (back after it left, or from inside it) makes one by Play a chosen
    member. A match called off before its countdown was not played: nobody leaves.
  - A room nobody is left in is gone, as always.
- **Lost (game crashed, killed, link silent; or started anew):** the place is kept for about 90 s (the link's
  30 s + the session's 60 s), the member shown grey (`away`); Play within it goes back to the
  same seat, state kept (Play stays usable while their row says they play the match). After
  it the place is lost: they may join again as a new member. A game quit from its menu leaves
  the room at once.
  - **Lost in a match** (two minutes of silence there: hung loading, crashed): the session
    waits for the match to end, the room does not (live, 2026-10-25: a game hung loading kept
    its room "full" all match, its side one short). The place is kept the session's 60 s after
    the link went, longer only while the match server has the player in the match (playing,
    or on its way in); then it is lost as above.
  - **Started anew** (the owner's of 2026-10-20: "i was in that weird state where the game was
    not in a room, but i got thrown into a match regardless (twice)"): a new sign-in keeps the
    place, but the room seats it in no match (nor sends it "match found"), and shows it grey,
    until its new game is told the room: its movie asks where it is (at once, again every 2 s
    until answered, then every 5 s outside a room), or it presses Play or Join match. Then it
    plays as any member; one who comes while the match is played sees the room 15 s first. A
    seat it had in the room's match stays its own: Play goes back to it. A new game not told
    within 90 s of its sign-in loses the place.
- **Leave match (its own word, 0xE0; the owner's of 2026-10-19):** takes the player out of
  its room as Leave does (not kicked; it may join again). Joining again while that match is
  played puts it into the same match as a late join (15 s or "Join match") as a new place,
  fresh (new index, zero stats), never its old one; only an account's latest place in a match
  counts (results, rewards, ratings, quests, map plays), the place it left for nothing.
- **After a match (the owner's of 2026-10-20):** the results screen opens over the room's
  window: closing the results shows the room, not the lobby. A player in no room sees the
  lobby after them, as before; so does one by Play that left with the match's end. The
  movie still has the room of its last state when the results come, and opens the window
  under them (`LobbyMenu.showRoomUnderResults`); `RoomsWindow.matchOver` asks the state at
  once. An answer of no room while the results are up leaves the window under them as it
  is (the results stay open until the player closes them: hiding the window would change
  the top menu's tab, which closes them; the owner's of 2026-10-21, "it shouldn't have
  open-reopen flicker"); it is hidden as they close (`RoomsWindow.resultsClosed`), so
  the lobby shows.
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
  - **Its seat in the match** is kept for the whole match (live, 2026-10-25: riad5 left room
    4799 after his link broke and could join no room; Play alone took him back). Leave works
    as always. Joining the room whose match keeps the seat from the room list takes it back
    to that seat and makes it a member again, even past the room's size (the room then lets
    nobody else in until it is under its size; a match seats at most its size a side). Play
    and Join match take it back to the seat without re-adding it to a room it left. Any other
    room refuses it with code 5: "You're still in a match: press Play to return to it, or
    wait for it to end."
  - **Three places at most** (`PLACES_PER_ACCOUNT`; live, 2026-10-25: a game that could not
    load went in every 15 s for minutes, each try a place of the match's 20 for good and the
    other joiners held out while it ran, until the match was full to everyone): an account
    with 3 places in the match's roster (its first, failed late joins, a leave of its own
    word and a come-back) is not put into it again; it plays the room's next match, with no
    `J` row.
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
  "Switch room", with why beside it ("Fairer match: opponents' gear up to level N"; then whose, how
  full; the owner's of 2026-10-20: why to switch was not clear): the open room (no PIN, a place free, not kicked from) of the lowest max
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
