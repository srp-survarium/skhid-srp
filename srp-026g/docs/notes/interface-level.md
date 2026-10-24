# The interface level (0.26g)

The owner (2026-10-24): "change our UI to be full/minimal/disabled selector through < full >";
"minimal will be still playable, but will remove unnecessary UI"; "since we own the code we can
update many features in UI based on that".

## The setting

- **Options, Game, "Interface"** (Интерфейс, Інтерфейс, Oberfläche, Interfejs): the game's arrow
  selector `< Full >`, in the bottom panel of the Game tab, right of the seeding checkbox.
- **Three values:** Full (Полный), Minimal (Минимальный), Disabled (Отключён). Full by default.
- **Saved** in `user.cfg` (`g_interface_level 0|1|2`), as the other options; a wrong value reads
  as Full. The console sets it too.
- **At once:** Accept in a match changes the interface in the next frame; no restart.
- **In a match only.** The lobby is not changed by it.

## The levels

- **Full:** everything, as before.
- **Disabled:** what F11 hides (`hide-ui-key.md`): the whole hud, the crosshair, the minimap, the
  markers and names over heads, Research's markers. The screen effects stay (the pain's red edges,
  the blood, the grenade's flash).
- **Minimal (a placeholder: the owner moves elements in the tables below):**
  - kept: the crosshair and its hit mark, health, armour, the body, ammo and the weapon, grenades
    and the quick slots, the hit and damage-direction marks, the mode's score line and time,
    Research's units and timers, the battery carried, the match's messages, the chat (its own
    option hides it), the kits, the Tab list, the teammates' markers, the objects' markers,
    Research's markers (letter and capture ring);
  - removed: the kill feed, the minimap, the quests' progress and tasks, the players' names over
    heads, the text under Research's markers (time left, metres).
  - 0.26g's hud has no reward, points or achievement pop-ups in a match (the exe never calls the
    movie's `add_points` or `set_context_message`), so there are none to remove.

## F11

- **F11 hides the interface and shows it again at the chosen level.** With Disabled chosen, F11
  changes nothing seen. (A placeholder too.)
- A new match starts at the chosen level (F11's hiding is lifted, as before).

## A mode that forces a level

- **One entry point:** `interface_force(setting, least)` in the exe (`interface_026g.c`): the
  movies get the larger of the player's choice and `least`. A Blackout mode calls
  `interface_force(0, 2)` at its start and `interface_force(0, 0)` at its end.

## The tables (where to move an element)

- Each movie has one: `srp-026g/flash/hud/src/survarium/constants/InterfaceLevel.as` (the hud's
  elements) and `srp-026g/flash/player_icons/src/survarium/constants/InterfaceLevel.as` (names,
  markers, Research's markers). A row is `element: [shown at Full, shown at Minimal]`.

| movie | element | Full | Minimal |
| --- | --- | --- | --- |
| hud | `crosshair` (crosshair, hit mark) | yes | yes |
| hud | `playerInfo` (health, armour, body, ammo, weapon) | yes | yes |
| hud | `quick_slots` (grenades, quick slots) | yes | yes |
| hud | `progress_indicator` (score line, time) | yes | yes |
| hud | `researchUnits` (Research's units, timers) | yes | yes |
| hud | `container_icon` (the battery carried) | yes | yes |
| hud | `chat` | yes | yes |
| hud | `combatLog` (kill feed) | yes | no |
| hud | `minimap` | yes | no |
| hud | `questList` (quests' progress) | yes | no |
| hud | `questTaskList` (quests' tasks) | yes | no |
| player icons | `players` (teammates' markers) | yes | yes |
| player icons | `names` (names over heads) | yes | no |
| player icons | `markers` (objects' markers) | yes | yes |
| player icons | `research` (unit letter and ring) | yes | yes |
| player icons | `researchText` (time left, metres) | yes | no |

## How it works

- **Exe patch 64** (`interface_026g.asm` and `.c`; `client_patch.md` 3.57): a table of settings,
  each a saved console variable (the game's cc_u32, as `minimap_position`), a row of the gameplay
  options (the game's options_item_int, row 16 + i) and the movies' method. After the hud's
  `update_ui`, every frame of a match, each movie (the hud's, the player icons') gets
  `root.set_interface_level(value)` when its value changed or the movie is new.
- **The main menu:** `SelectorRow` places the game's arrow selector for each row the timeline
  does not have; `Settings` binds it as the timeline's own (label, values, the variable).
- **The hud:** `set_interface_level` hides the root at Disabled; each element of the table is
  shown when both the game and the level show it (`showElement`, where the hud showed it before).
  F11's `hide_interface` and the level share `showInterface`.
- **The player icons:** three layers (Research's units, players, objects), the names
  (`HUDMarker.nameShown`), Research's texts (`ResearchMarker.textShown`).
- **Labels:** content edit `interface-level-1.json` (six localization tables).
- No simulation change, no server change.

## Adding the next interface setting

A new choice (the kill feed's length, the minimap's size, names over heads on or off):

1. A row of `interface_settings` in `interface_026g.c`: its variable's name (saved in
   `user.cfg`), its label key, its values' keys, and the movies' method that gets the value
   (`root.set_<name>`). The gameplay tab's count follows the table (patch_026g.py reads it from
   the link); `python link_026g.py --write`.
2. A row of `SelectorRow.ROWS` in the main menu (the control's name, its place in the Game tab), in
   the same order.
3. Its labels in a content edit (as `interface-level-1.json`) and a row of `release/content.tsv`.
4. The method in the movie that draws the element (`HUD`, the player icons' `MainTimeline`), and
   its own handling there.
5. `check_interface_026g.py` and `selftest_026g.py` check 64 cover the table's rows; add the new
   one's cases.

A new element of an existing level (an element Minimal hides or keeps) is a row of the movie's
`InterfaceLevel.SHOWN`, and where the hud shows it, `showElement` instead of `visible`.
