# Mute chat: the lobby's player menu and the Tab list's (0.26g)

The owner's asks: 2026-10-28 (the lobby's player menu, tested locally: "mute should hide messages
from that player", "another one shows name: <muted>"); 2026-10-31 ("we had this work done before,
now we can extend it to the in-game HUD").

## The choice

- **Per player, kept by the server** with the account (the `ignored` table, its `mode` column:
  schema step 5). One list for the lobby and the match: both menus send the same request, the
  server applies it to every chat. No copy on the client.
- **Three choices**, in a "Mute chat" entry of a player's menu:
  - **Hide messages:** that player's lines never reach you.
  - **Show name: `<muted>`:** each line reaches you as `Name: <muted>`; the text never does.
  - **Unmute chat:** lines reach you as before.
- A new choice replaces the old one; there is never two for one player. Ignores made before
  (the friends window's) are Hide.
- **Every chat:** the match's (all and team), general, private, squad, clan, the room's, and the
  room's history sent on joining. Lines already shown stay.
- A muted player's friend requests and squad invites are dropped too (as an ignore's).
- Never your own name. At most `MAX_LISTED` mutes (the friends' limit); an unmute always goes.

## Where

- **The lobby:** right-click a player (rankings, a room's members) -> Mute chat -> the three
  choices (`flash/lobby_menu`, `survarium/friends/ChatMute.as`). The same menu offers Add/Remove
  friend, Send message and Invite to squad (a commander with room, a player not in the squad).
- **The match:** the Tab list's row menu (Ctrl for the cursor, right-click a row;
  tab-cursor-friends.md) has "Mute chat" after "Add to friends", for every other player's row, a
  friend's too and a bot's. A click opens the three choices in its place.
  - After a choice the menu shows one line until Tab is let go or a press outside it: "Chat
    muted" (Hide or `<muted>`), "Chat unmuted", or "Not sent" (the client's checks refused it).
  - Unmute is always offered, as in the lobby: the client knows only the hidden players (the
    native ignore list, `A3 08`), not those shown as `<muted>`.
- **Texts:** `st_srp_hud_mute_*`, `st_srp_hud_muted`, `st_srp_hud_unmute(d)`; both movies use the
  same keys.

## The wire

- **The request** is the game's own `comment_about_user` (`49 05`: name, kind, mode) with srp's kind
  254 and the mode 0 unmute, 1 hide, 2 `<muted>`. The lobby movie calls the native callback
  (0x28CBE0); exe patch 86 calls the same sender, `lobby_client::comment_about_user` (0x274470),
  with the place's bare name. Other kinds are reports and praise, refused as before.
- **The lines:** the server makes each recipient's line itself. A hidden sender's line is not
  sent; a `<muted>` one is sent with the body replaced, so the text is never on that link.
- **The client's list (`A3 08`)** holds the hidden players only: the game drops a line from a
  listed sender before reading it (0x26C010), which would drop the `<muted>` lines too.

## Checks: the server and the client

- **Server** (`set_chat_mute`): not your own name; a name that is an account (else the lobby's
  "Player not found", but nothing from a match: a bot's row); a known mode (else the message is
  refused); the list's limit.
- **Client, exe patch 86:** not your own name (ASCII case ignored), a place of the match, one of
  the three choices, the lobby connection up. No allowance: the server has none either.
- **Client, the hidden lines:** the native ignore list drops them on the client as well. A
  `<muted>` line needs no client check: its text never reaches the client.

## Not a match rule

Nothing a match computes changes; replays are not affected. No `simulation_changes.tsv` row.

## Tests

- Lobby server: `chat_mute_tests` (match and team, per recipient, switching, no original text in
  any packet, a mute from a match), `room_chat_tests` (live lines and history), persistence tests
  (both modes, the old ignores' migration).
- `client-patches/check_tab_cursor_026g.py` (Unicorn): the menu's answer, each choice sent with its
  mode, the refusals.
- `scripts/tab-row-menu/check.sh <hud.swf>` (Ruffle): the entry, its choices, the answers' lines,
  and the match chat showing `Name: <muted>` as text. `scripts/rooms-start/check.sh
  <lobby_menu.swf>`: the lobby menu's entries.
- **Not run in a game yet** for the Tab list's entry.
