# Mute chat: the lobby's player menu and the Tab list's (0.26g)

The owner's asks: 2026-10-28 (the lobby's player menu, tested locally: "mute should hide messages
from that player", "another one shows name: <muted>"); 2026-10-31 ("we had this work done before,
now we can extend it to the in-game HUD").

## The choice

- **Per player, kept by the server** with the account (the `ignored` table, its `mode` column:
  schema step 5). One list for the lobby and the match: the server pushes it to the client (`A3 08`,
  below), and both menus read that one list. The client keeps no list of its own.
- **Three states:** not muted; **hidden** (that player's lines never reach you); **shown as
  `<muted>`** (each line reaches you as `Name: <muted>`; the text never does).
- **The menu offers the two entries that change the state** (the owner, 2026-10-31: "2 options of
  two", no sub-menu): they sit in the player's menu itself, next to the friend entry.
  - not muted: "Hide chat", "Show chat as <muted>";
  - hidden: "Show chat as <muted>", "Unmute chat";
  - shown as `<muted>`: "Hide chat", "Unmute chat".
- A new choice replaces the old one; there is never two for one player. Ignores made before
  (the friends window's) are hidden.
- **Every chat:** the match's (all and team), general, private, squad, clan, the room's, and the
  room's history sent on joining. Lines already shown stay.
- A muted player's friend requests and squad invites are dropped too (as an ignore's).
- Never your own name. At most `MAX_LISTED` mutes (the friends' limit); an unmute always goes.

## Where

- **The lobby:** right-click a player (rankings, a room's members): the info, Add/Remove friend,
  the two mute entries, Send message, Invite to squad (a commander with room, a player not in the
  squad) (`flash/lobby_menu`, `survarium/friends/ChatMute.as`).
- **The match:** the Tab list's row menu (Ctrl for the cursor, right-click a row;
  tab-cursor-friends.md): "Add to friends" and the two mute entries; a friend's row the two mute
  entries alone; every other player's row, a bot's too.
  - After a choice the menu shows one line until Tab is let go or a press outside it: "Chat
    muted" (hidden or `<muted>`), "Chat unmuted", or "Not sent" (the client's checks refused it).
- The friends window's Ignored tab lists every muted player, both modes.
- **Texts:** `st_srp_hud_mute_hide` ("Hide chat"), `st_srp_hud_mute_mask` ("Show chat as
  <muted>"), `st_srp_hud_unmute` ("Unmute chat"), `st_srp_hud_muted`, `st_srp_hud_unmuted`,
  `st_srp_hud_mute_not_sent`; both movies use the same keys.

## The wire

- **The request** is the game's own `comment_about_user` (`49 05`: name, kind, mode) with srp's kind
  254 and the mode 0 unmute, 1 hide, 2 `<muted>`. The lobby movie calls the native callback
  (0x28CBE0); exe patch 86 calls the same sender, `lobby_client::comment_about_user` (0x274470),
  with the place's bare name. Other kinds are reports and praise, refused as before.
- **The lines:** the server makes each recipient's line itself. A hidden sender's line is not
  sent; a `<muted>` one is sent with the body replaced, so the text is never on that link.
- **The client's list (`A3 08`)**, pushed at sign-in and after each change, holds every muted
  player. A `<muted>` one's account id carries srp's mark, bit 31 (`lobby::SHOWN_MUTED`):
  - the game drops a line whose sender id is on the list before reading it (0x26C010); a marked
    id is no sender's, so the `<muted>` lines pass, and a hidden player's are still dropped;
  - the lobby movie gets the list as the game gives it (`set_ignored_list`, 0x28B910, the id a
    uint) and `ChatMute.modeOf` reads the mark; exe patch 86 reads the messaging client's ignore
    vector (+1Ch, the friend list's way) and answers the hud the state;
  - the friends window sends a row's id back as it has it (`4E 03`, `4E 05`): the server reads it
    without the mark (`lobby::listed_account`). Account ids are below bit 31.

## Checks: the server and the client

- **Server** (`set_chat_mute`): not your own name; a name that is an account (else the lobby's
  "Player not found", but nothing from a match: a bot's row); a known mode (else the message is
  refused); the list's limit.
- **Client, exe patch 86:** not your own name (ASCII case ignored), a place of the match, one of
  the three choices and not the player's state already, the lobby connection up. No allowance:
  the server has none either.
- **Client, the lines:** the game's own filter drops a hidden player's lines, and a `<muted>`
  player's under its real id (only the server's `<muted>` lines, sent so as not to be on the list,
  pass). A `<muted>` line's text never reaches the client.

## Not a match rule

Nothing a match computes changes; replays are not affected. No `simulation_changes.tsv` row.

## Tests

- Lobby server: `chat_mute_tests` (match and team, per recipient, switching, no original text in
  any packet, a mute from a match, the list and its marks after each change, the friends window's
  unignore of a marked id), `room_chat_tests` (live lines and history), persistence tests
  (both modes, the old ignores' migration).
- `client-patches/check_tab_cursor_026g.py` (Unicorn): the menu's answer with each state (the
  ignore list, a marked id), each choice sent with its mode, the refusals (an unchanged state too).
- `scripts/tab-row-menu/check.sh <hud.swf>` (Ruffle): a stranger's row (three entries), a friend's
  (two), each state's two entries, Unmute gone after Unmute, the answers' lines, and the match chat
  showing `Name: <muted>` as text. `scripts/rooms-start/check.sh <lobby_menu.swf>`: the lobby
  menu's entries in each state.
- **Not run in a game yet** for the Tab list's entry.
