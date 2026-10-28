# Match HUD options (0.26g)

Board T29 (a player's asks, scheduled by the owner 2026-10-23). The player's keys and options in
the match HUD.

## Kit keys on the kit screen (T29)

A player's ask (2026-10-21): "before respawn, add key shortcuts for change loadouts 1 2 3. i
sometimes missclick".

- **Keys 1, 2 and 3** (above the letters) pick kit 1, 2 and 3 while the kits are shown: after a
  death, and before the match starts.
- **A key is a click** on that kit's tab: the same kit choice, nothing new sent. A kit that
  cannot be played (greyed) or the one already chosen: nothing.
- **Only a press made on the kits, 300 ms or more after they appear,** picks (the owner,
  2026-10-25: 3 pressed mid-fight to heal must not pick kit 3 at the death; then "ignore kit keys
  for 300 ms after the kits screen appears"). A key held into the kits, pressed in play in the
  frame they came, or pressed in their first 300 ms picks nothing until it is let go and pressed
  again after that.
- **Before the start** the same: the keys work 300 ms after the kits are shown.
- **Not while typing** in the chat: the keys are the text's.
- **The keys keep their other uses:** the game gets them as before (a dead player's game sends no
  input); the numpad and rebinding do not pick kits.
- **How:** exe patch 58 hands the keys to the hud movie (`HUD.select_kit_key`).

## Hide chat in match (T29)

A player's ask (2026-10-21): "option to turn off in-game chat".

- **Options, Game, "Hide chat in match"** (the checkbox under Chat that read "Disable private
  messages at the time of battle", which did nothing): checked, other players' chat lines (all,
  team, squad, clan) are not shown in a match.
- **Still shown:** the player's own lines, server messages, everything in the lobby.
- **Off by default; remembered** in `user.cfg` (`g_private_messages_in_game on`).
- **Typing still works:** T opens the chat as before.
- **How:** exe patch 58 (the option's variable, the lines dropped), the main menu enables the
  checkbox, content edit `match-chat-off-1.json` (its label in en, de, pl, ru, uk).
