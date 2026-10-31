# Hide hit markers, hide player scores

The owner (2026-10-30, for release 40): "allow hiding hit markers (UI change) and player scores
(and kills/deaths); it should only show names in alphabetic order". Exe patch 83
(`client-patches/rust/patches/src/hud_options.rs`), the hud movie (`flash/hud/src`: `HUD`,
`Crosshair`, `PlayerList`, `PlayerListRenderer`) and the main menu movie
(`flash/main_menu/src`: `SrpCheckGroup`, `SrpCheckOption`, `SrpLanguage`, `Settings`).

## The options

- **Where:** Settings, Gameplay, a group of its own, "HUD:", at the top of the right column
  (`../images/hide-options-settings.png`): "Hide hit markers", "Hide player scores", then the
  stock quest progress controls ("Show quest progress in match" and its "Transparency" slider) and
  the stock minimap controls ("Fix minimap" and the minimap position list). The tab's own look:
  the heading as "Friends:" is, plain checkboxes, the same spacing.
- **The tab's groups** (the owner, 2026-10-31: "quest progress I think is part of the HUD too",
  "and map"): left "Friends:", "Crosshair:" (now only "Static crosshair"), "Language:"; right
  "HUD:". The stock controls keep their own bindings (the exe binds them by name); only their
  places change, and the "Quest progress:" heading is not shown. Each column's headings share its
  left edge, its controls the stock controls' edge (a list one pixel further, as the timeline has
  it); the spacing is the timeline's own, read once: the gap before a group, a heading's first
  control, the checkbox step, the slider's distance from its checkbox (its bar is its row, the
  next control a step under it), the minimap list's under its checkbox. Language (patch 79's list)
  is a group under "Crosshair:", its label a heading, its "Applies after restart." note under the
  list while it shows. The old bottom panel (the hidden seeding and statistics options') is not
  drawn on this tab. One column for all of it did not fit the window; the column divider is the
  timeline's and stops where that panel began.
- **The map settings:** the game has two, both from the stock "Crosshair:" group of this tab:
  "Fix minimap" (`_setter_static_minimap`) and the minimap position list
  (`_setter_static_minimap_position`: off, lower left, upper left). No other tab has map settings.
- **Texts:** dictionary keys in all ten languages: the heading `st_srp_options_hud` ("HUD:";
  Russian, Ukrainian, Belarusian "Интерфейс боя:"/"Інтерфейс бою:"/"Інтэрфейс бою:", not
  "Interface", which the Interface level selector may take), the labels
  `st_srp_hide_hit_markers_option`, `st_srp_hide_scores_option`. The exe gives them with each
  checkbox's answer.
- **Off by default:** off is the game as it was.
- **Apply saves:** a toggle is a pending change of the tab like any option there; Apply saves it,
  Cancel or leaving without Apply puts the saved state back.
- **At once:** the hud takes a change in its next frame, in a match too (no next-match wait).
- **Stored in user.cfg** (`Documents\survarium\user.cfg`): `srp_hide_hit_markers on|off`,
  `srp_hide_scores on|off` (saved bools, as the game's own options; the file is written at Apply).
  An older game ignores the lines.
- **Per player, on this computer:** nothing is sent; other players see what they chose.

## Hide hit markers

- The crosshair's hit marker is not shown: the bars around the crosshair on a hit on an enemy,
  and the kill's mark (`root.crosshair_enemy_hit`, `Crosshair.setDamage`). A marker on screen when
  the option is turned on is cleared.
- Unchanged: the marks of where the player is hit from, the kill feed, every sound.
- The marker is the hud movie's: the exe only invokes `root.crosshair_enemy_hit` (at 0x238387 in
  draw_damage_collector 0x238230, and at 0x23D02D in the function 0x23CF20 that 0x2337D9 calls), so
  the movie drops it; the exe is not changed there.

## Hide player scores

- The Tab list shows the players' names only: no rank, kills or deaths columns, no column
  headers (`../images/hide-scores-tab-list.png`).
- Each team stays in its own list, as the board groups them; each list is sorted (the owner,
  2026-10-31) by clan tag, then by name, case-insensitive: the clans' players first (by tag),
  then the players without a clan, then the bots (the server's tag "BOT"). A row's name is
  "[TAG] name" (initialize_match), so the tag is read from it. Equal names go by the row's id.
  Nothing changes the order but a name: a player's kills or score do not move them.
- Kept: the team names, the player's own row highlighted, squad marks, a player gone offline in
  grey, the team scores at the top of the screen, the kill feed.
- **Results screen** (the lobby's end-of-match screen): left as it is. It comes after the match,
  and the player's pay is shown with its score there; the option is the match hud's.

## With the other interface settings

- **Separate checkboxes**, not part of the Interface level selector (Full / Minimal / Disabled,
  branch `feat/interface-level`, not merged): the selector decides which parts of the hud show;
  these change what the crosshair and the Tab list show when they do. Disabled hides both anyway.
- **F11 (hide interface):** unchanged; both options apply again when the interface shows.
- **Blackout:** no effect there, nothing to change: its interface is forced hidden (no crosshair,
  and Tab never shows the list). The saved choice is kept for the next match.
- **Replays:** the replay's hud follows the options as a match's does.

## How

- **Exe** (Rust, five lohooks of `hud_options.toml`, order 2, each after another patch's hook at the same
  site):
  - `configs.start` (game::on_configs_loaded's start, after patch 79): registers the two saved
    bools (cc_value<bool>::cc_value 0x1DDB0, then cc_bool's vtable 0x9E7C4C) before user.cfg runs;
  - `options.callback` (game_options::scaleform_callback's start, after patch 79): a call named
    `srp_hide_hit_markers` or `srp_hide_scores` answers `root.srp_check_option(name, label,
    "1"|"0", the group's heading)`; with "1" or "0" it saves that first (user.cfg at once);
  - `player_icons.update_start` (update_player_icons' start, every frame of a match, after patch
    65): `root.srp_hud_options(hide hit markers, hide scores)` to the hud movie, once for each new
    hud movie or change.
  - `static_info.options_read`, `replay.options_read` (a match's or a replay's options read,
    after patch 65): the hud is told again, since a new match can make its hud movie at the last
    one's address (as patch 65 tells its own state again).
- **Settings code:** `vostok::settings` (the Settings' call, the save of user.cfg, `ConsoleBool`),
  shared with patch 79's Language choice.
- **Hud:** `HUD.srp_hud_options` sets `Crosshair.hitMarkersHidden` and the Tab list's
  `PlayerList.namesOnly`.
- **Main menu:** `SrpCheckOption` (one checkbox the exe keeps by its name), two of them in an
  `SrpCheckGroup` (the heading and its checkboxes), placed with `SrpLanguage` and the stock groups
  by `Settings.layoutGameplay`.
- Nothing a match computes changes: no `simulation_changes.tsv` row.
- **Checks:** `../../client-patches/check_hud_options_026g.py` (Unicorn: the registration and
  user.cfg's lines, the Settings' calls, the hud told at each change), Rust tests,
  `../../scripts/hud-options/check.sh` (Ruffle: the checkboxes' place, pending change, Apply,
  Cancel, every language's labels inside the panel; the hit marker shown off, hidden on; the Tab
  list as today off, names only by name on). Not yet run in the game.
