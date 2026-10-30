# Hide hit markers, hide player scores

The owner (2026-10-30, for release 40): "allow hiding hit markers (UI change) and player scores
(and kills/deaths); it should only show names in alphabetic order". Exe patch 83
(`client-patches/rust/patches/src/hud_options.rs`), the hud movie (`flash/hud/src`: `HUD`,
`Crosshair`, `PlayerList`, `PlayerListRenderer`) and the main menu movie
(`flash/main_menu/src/SrpCheckOption.as`, `Settings`).

## The options

- **Where:** Settings, Gameplay, the bottom panel's right side, one under the other: "Hide hit
  markers", "Hide player scores" (`../images/hide-options-settings.png`). Two plain checkboxes,
  the game's own style. The labels are dictionary keys (`st_srp_hide_hit_markers_option`,
  `st_srp_hide_scores_option`), in all ten languages.
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
- Each team stays in its own list, as the board groups them; each list is sorted by name,
  case-insensitive (alice, Bob, carol, Dave). Nothing changes the order but a name: a player's
  kills or score do not move them.
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
    "1"|"0")`; with "1" or "0" it saves that first (user.cfg at once);
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
- **Main menu:** `SrpCheckOption` (one checkbox the exe keeps by its name), two of them in
  `Settings`.
- Nothing a match computes changes: no `simulation_changes.tsv` row.
- **Checks:** `../../client-patches/check_hud_options_026g.py` (Unicorn: the registration and
  user.cfg's lines, the Settings' calls, the hud told at each change), Rust tests,
  `../../scripts/hud-options/check.sh` (Ruffle: the checkboxes' place, pending change, Apply,
  Cancel, every language's labels inside the panel; the hit marker shown off, hidden on; the Tab
  list as today off, names only by name on). Not yet run in the game.
