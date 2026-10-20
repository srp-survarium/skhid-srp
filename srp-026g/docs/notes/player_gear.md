# A player's equipment from the Rankings (and the rooms' lists)

The rules the code follows. Code: `client-patches/player_gear_026g.py`,
`player_gear/SrpPlayerGear.as`, `player_gear/PatchPlayerGear.java`; the lobby's
`servers/lobby-server/src/v026g/gear.rs`; the wire `lobby::GEAR_BY_PLACE`. The owner's rules,
2026-10-20: "after match in match overview you can rmb to see the players equipments", "you
should be able to go to rankings and rmb to do the same", "for every single player".

- **What the results screen does (the game's own, unchanged):** a **double-click** on a row
  ("Double click to show information", `show_player_info_hint`) asks for that player's kits
  (`victory_screen_request_player_data` → `3D 00 <row> <match>`); the answer (`93`) opens the
  game's "Information about the player:" window (`player_info`, its kits and items, a tab a
  kit). Its right-click menu has Add to friends, Add to ignore, Report and Compliment, no
  equipment item (`VictoryScreen.showContextMenu`). srp answers the double-click for players of
  a match it keeps (`rewards.rs` `player_details`, the kits as the match left them).
- **Rankings:** a right-click on any row (Top 100 and My place, any page, the player's own row,
  the own place under the list) opens the game's right-click menu (as the results screen's) with
  one item, the game's own "Show player's information" (`show_player_info_button_hint`, every
  language). It opens the same window as the results screen, with that player's name.
- **Every player:** online or not, never signed in since a restart, with no match ever, the
  asker itself: the server answers from the account as it is now (srp.db, kept in memory).
  Kits as they are now, not as some match left them.
- **Which player:** the request names the row's place (`3D 00 <GEAR_BY_PLACE> <place>`, as the
  History's "Watch replay" names its match). The server keeps, for each player's session, its last
  8 ranking pages as sent (the first place, the account of each row): a place is the account the
  newest kept page with it showed there, however the ranking moved since, so a tab the movie
  keeps (Top 100 while "My place" was asked, or the other way) is what it showed. The own place
  under the list is the asker itself (kept as a page of one row when the page opens). A place no
  kept page has (rows of the sign-in's pushes): the ranking's now.
- **Which answer:** a `93` carries nothing of the player it is about, so the movie has one ask
  in flight at a time: an ask made meanwhile waits, the answer to the older one is then dropped
  and the newest goes; an ask not answered in 3 s (no such place) is given up. While the results
  screen shows, every `93` is its own (the screen is modal; an ask of ours from before it opened
  is given up).
- **Nothing to show** (no such place or account): no answer, no window (the game has no "nothing
  to show" message for `93`).
- **The rooms' lists** use the same menu and window (`SrpPlayerGear.menu`; `rooms.md`): a member of the Rooms list's side panel by its account (`GEAR_BY_ACCOUNT`; only a member of a room now is answered), a member of the asker's own room by its place and the state's serial (`GEAR_IN_ROOM`).
- **Older movies:** never ask. An older server does not answer (no window opens).
- **Release:** a lobby movie (resources.db patch) and the server; the server first.
