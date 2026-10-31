# A cursor on the Tab list, and "Add to friends" from its rows (0.26g)

The owner's ask for release 40 (2026-10-30): "allow pressing Ctrl while in Tab, which gives a cursor.
This cursor can be used to right-click players in the menu to mute them (next feature), add to
friends, etc.; for now just adding to friends."

## The cursor

- **Ctrl pressed while the Tab list is held shows a mouse cursor.** It is a press, not a hold: the
  cursor stays after Ctrl is let go, for as long as Tab is held. Either Ctrl key works.
- **While it is shown:** the mouse moves the cursor and nothing else. The view does not turn, and
  no mouse button fires, aims or acts. The movement keys still move the player. This is the same
  cursor as the kit screen's (the game's own `show_cursor`), and it starts at the screen's centre.
- **Tab let go hides it**, closes an open menu, and gives the mouse back to the view. A button held
  through it does not fire on its release (patch 31's warmup keys).
- **Ctrl is crouch by default.** A Ctrl pressed with Tab held is the cursor's key only. That Ctrl
  does not crouch, and its held and release events go to no one until it is let go, even after
  Tab. A Ctrl already held before Tab (a crouch) stays the player's, and so does every Ctrl
  pressed without Tab. Crouch toggle (C) works as before.
- **A lost release** (the chat or another window took Ctrl's up): the next Ctrl pressed without
  the list is the player's again, so a crouch is never eaten. A new match starts with no cursor
  state of the match before.
- **Dead or before the start:** the kits' cursor is already shown, so right-click on a row works
  there without Ctrl. Tab let go leaves that cursor to the kits.
- **Blackout:** Tab shows no list there, so Ctrl crouches and no cursor shows.
- **Typing in the chat:** the chat takes every key, Ctrl included, as before.
- **The interface hidden (F11):** the list is hidden with the rest of the hud, but Tab and Ctrl
  still show the cursor. There is nothing to click, and Tab let go hides it again.

## The menu

- **A right-click on a player's row** opens a small menu at the cursor. A left click on a row does
  nothing new.
- **Rows:** everyone in the match, allies and enemies alike, except:
  - your own row: no menu;
  - a player already on your friend list: no "Add to friends" entry (the two mute entries alone).
  A bot's row is offered: the match's place records carry no account, and the server's bots'
  clan tag "BOT" is one a players' clan may also take, so the client has no reliable bot mark.
  From a match the lobby says nothing for a name that is no account.
- **Entries:** "Add to friends" (`st_srp_hud_add_friend`) and the two chat mute entries that change
  the player's state ([chat-mute.md](chat-mute.md)). The menu's entries are a list the exe's answer
  fills (`root.srp_player_menu(place, add_friend, mute, muted)`).
- **Closes** on Tab let go, on a press outside it, and at the next match. No timer closes it.
- **The hide-scores option** (names only, alphabetical) keeps the rows and their places, so the
  menu works there too. The worker who builds it must keep the row renderer and the lists'
  click events.

## Add to friends

- **The request is the lobby's own friend request** (`4D` type 0 with the player's name, what the
  lobby's menus send). The lobby link is up during a match, and the request goes through the
  lobby's normal friend rules (social.md 1.5, `Core::important`). The other player answers it in the
  lobby, now or at their next sign-in.
- **After the click** the menu shows one line in its place, until Tab is let go or a press
  outside it:
  - "Friend request sent" (`st_srp_hud_friend_sent`) when the exe sent it;
  - "Friend request not sent" (`st_srp_hud_friend_not_sent`) when its own checks refused it.
- **Checks, the same on the client (exe patch 86) and the lobby:**
  - never your own name;
  - never a player already on your friend list;
  - at most 5 requests at once, then one more every 10 s (`FRIEND_REQUEST_BURST`,
    `FRIEND_REQUEST_EVERY`). The lobby drops one over this limit; the client does not send it.
    The limit also applies to requests sent from the lobby.
  - the lobby connection up (client only, as the lobby's own senders check it).
  - The lobby's existing rules still apply: one waiting request per pair, nothing to a player who
    ignores you.

## How

- **Exe patch 86** (`client-patches/rust/patches/src/tab_cursor.rs`, `tab_cursor.toml`). It has
  two lohooks:
  - `tab_cursor.key`, at the game world's `on_keyboard_action` 0x25CD00, which shows and hides the
    cursor and takes Ctrl's events;
  - `hud.callback`, at the hud's `scaleform_callback` 0x233D30, which answers the movie's
    `srp_player_menu(place)` with `root.srp_player_menu(place, offer)`, and its
    `srp_add_friend(place)` with `root.srp_friend_request(place, sent)`.
- **The row's place** (its `id`) is what the movie sends. The exe reads that place's bare name from
  the match's places, so a clan tag shown in the row is never sent.
- **The hud movie:** `survarium/controls/PlayerRowMenu.as` (a class of its own), set up in
  `HUD.as`. The movie's texts are dictionary keys; the hud is now given its own srp texts,
  the `st_srp_hud_*` keys only (`localization_026g.FLASH_MOVIES`), at every match start.
- **Not a match rule:** nothing a match computes changes, and replays are not affected.

## Checks

- `client-patches/check_tab_cursor_026g.py` (Unicorn), on the original exe: the keys, Blackout,
  and the hud's two calls.
- `cargo test` in `client-patches/rust`: the cursor's transitions and the allowance.
- `scripts/tab-row-menu/check.sh <hud.swf>` (Ruffle): the menu, the exe's answers, and a real click
  on the entry.
- Lobby server tests: `friend_requests_come_five_at_once_then_one_every_ten_seconds`,
  `a_friend_request_for_nobody_from_a_match_is_dropped_in_silence`.
- **Not run in a game yet**, in particular:
  - whether Scaleform gives the hud a right-click (the lobby movie gets them);
  - how a friend request that reaches a player during their own match is shown.
