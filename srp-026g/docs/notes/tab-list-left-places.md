# The Tab list shows the places in the match (patches 39 and 43)

The rules the code and the tests follow. Code: `client-patches/late_join_026g.asm`
(`tab_row`, `tab_shown`, `lists_hook`), `patch_026g.py` `TAB_LIST_*`, README patch 39,
`client_patch.md` section 3.36; the test: `check_tab_list_026g.py`. The owner's rules: 2026-10-18
(rooms: `rooms.md`, "In the match (Tab list)") and 2026-10-19 ("once they leave their previous
entry should be removed"). Patch 43 (own row gold): `patch_026g.py` `TAB_SELF`, README patch 43,
`client_patch.md` section 3.40, `check_tab_self_026g.py`; the owner 2026-10-20.

- **What the list is:** the in-match scoreboard (Tab, the hud movie's `root.list_set_players`).
  It has a row for every roster place that is *in the match* now, and for no other.
- **In the match** = between the server's 0xE9 (entered) and 0xEA (left) for that place:
  `kd_stats_rule`'s `online`, which every game of the match sets and clears at the same tick and
  which a world (late join, resync) carries. Nothing else decides it: no timer, no link state.
- **A row goes when:**
  - the player leaves by its own word (Leave match, 0xE0): the server's 0xEA, 300 ms ahead;
  - the player's link is silent past `SRP_MATCH_CLIENT_TIMEOUT_SECONDS` (10 s): the server
    gives the place up the same way (0xEA);
  - a server bot gives its place up (to a late joiner);
  - a newcomer's join fails after its place was prepared (it never entered).
- **A row comes (back) when:** the place enters (0xE9): at the start, as each player has loaded;
  a late joiner when it is let in; a lost player whose game comes back to the same place.
- **Coming back after leaving** is a new place (rooms): the old place's row is gone, the new
  one is its only row, with its fresh numbers. Two places of one name never both show.
- **Places not in yet:** a place still loading at the match's start, or a newcomer being
  prepared, has no row until it enters (the stock game listed them greyed).
- **On which clients:** every game of the match the same, the leaver's own excepted (it is out
  of the match); a late joiner's and a resynced game's list follow the world they loaded, so
  they never show a place that left before they came. The replay viewer follows the
  recorded 0xE9/0xEA the same way.
- **When it changes on screen:** while the list is shown, at the hud update after the change;
  a hidden list is made again the next time it is shown. Rows keep their order and sorting
  (the movie sorts by score).
- **What stays as it was:** the rows' numbers (kills, deaths, score) of the places listed; the
  replay viewer's player bar (`root.set_players_list`, every place); the enemies' markers; the
  server's results screen (every place, the left one with the numbers it had); what a match
  computes (no simulation change: games with and without the patch play together).
- **Grey rows:** none now. A lost player is listed normally until the server's timeout and then
  removed; it is not greyed for the rooms' ~90 s seat. That needs the server to tell the
  clients which places are away with a seat kept (a UI-only message and a client hook for it);
  not built (client_patch.md section 3.36, "Not done").
- **Your own row is gold** (owner, 2026-10-20: "when pressing tab in game, your own character
  should be gold", "just like when you kill or get killed in a kill list top right"): the
  kill list's own colour, the hud's `Colors.PLAYER` (0xF1E545), on the player's own row only, in
  every match and after every remake of the list (patch 43).
