# The HUD group: crosshair, hit markers, player scores, quests, minimap

The owner (2026-10-30, for release 40): "allow hiding hit markers (UI change) and player scores
(and kills/deaths); it should only show names in alphabetic order". Exe patch 83
(`client-patches/rust/patches/src/hud_options.rs`), the hud movie (`flash/hud/src`: `HUD`,
`Crosshair`, `PlayerList`, `PlayerListRenderer`) and the main menu movie
(`flash/main_menu/src`: `SrpCheckGroup`, `SrpCheckOption`, `SrpLanguage`, `Settings`).

## The options

- **Where:** Settings, Gameplay, a group of its own, "HUD:", at the top of the right column
  (`../images/hide-options-settings.png`), in this order: the crosshair list (Dynamic crosshair,
  Static crosshair, Hidden crosshair), "Hide hit markers", "Hide player scores", "Hide quests",
  "Static minimap", the minimap position list. The tab's own look: the heading as "Friends:" is,
  plain checkboxes and the stock list, the same spacing.
- **The tab's groups** (the owner, 2026-10-31: "quest progress I think is part of the HUD too",
  "and map"; the crosshair a list in HUD; "Language:" under Friends): left "Friends:", then
  "Language:" (patch 79's list, its label a heading, its "Applies after restart." note under the
  list while it shows); right "HUD:". Each column's headings share its left edge, its controls
  the stock controls' edge (a list one pixel further, as the timeline has it); the spacing is the
  timeline's own, read once: the gap before a group, a heading's first control, the checkbox step,
  the minimap list's distance under its checkbox. Not shown: the stock "Crosshair:" and "Quest
  progress:" headings, the stock static crosshair checkbox (the list sets it) and the quest
  progress "Transparency" slider (the owner: not useful; its value stays as the exe has it, the
  menu never changes it). The old bottom panel (the hidden seeding and statistics options') is not
  drawn on this tab; the column divider is the timeline's and stops where that panel began.
- **The stock controls in HUD** keep their bindings (the exe binds them by name and its setters);
  only their places and labels change:
  - **Hide quests** is the stock "Show quest progress in match" (`_setter_static_show_quests`)
    inverted: checked is the stock value off. The menu shows the opposite of the exe's value and
    gives the exe the opposite of the box (`Settings.flip`: the bind of the values, a toggle given
    to the exe's setter, Apply). The stock default (shown) is Hide quests unchecked.
  - **Static minimap** is the stock "Fix minimap" (`_setter_static_minimap`), renamed: checked,
    the minimap does not turn with the view (north up; the hud's `set_rotable(false)`, the player
    arrow turns instead). Its value and behaviour are the stock ones.
  - **The minimap position list** (`_setter_static_minimap_position`: off, lower left, upper left)
    as it was. These two are the game's only map settings (no other tab has any).
- **The crosshair list:** Dynamic and Static are the stock static crosshair setting
  (`_setter_static_cross`: unchecked Dynamic, checked Static), so the stock values carry over; a
  pick of either sets it, which reaches the exe as the tab's own change. Hidden is patch 83's
  `srp_hide_crosshair`; picking it leaves the stock value, so Dynamic or Static comes back as it
  was when Hidden is dropped. The stock CLIK list, as the minimap's: it opens over the controls
  under it and a press outside closes it. (#862's transient rules are the lobby movie's; the
  Settings window is the main menu's, whose lists close this way already.)
- **Texts:** dictionary keys in all ten languages: the heading `st_srp_options_hud` ("HUD:";
  Russian, Ukrainian, Belarusian "Интерфейс боя:"/"Інтерфейс бою:"/"Інтэрфейс бою:", not
  "Interface", which the Interface level selector may take), `st_srp_hide_hit_markers_option`,
  `st_srp_hide_scores_option`, `st_srp_hide_quests_option`, `st_srp_static_minimap_option` (each
  language's word for "static" from its "Static crosshair"), `st_srp_crosshair_dynamic`,
  `st_srp_crosshair_static` (the stock "Static crosshair" texts), `st_srp_crosshair_hidden`. The
  exe gives the checkboxes' labels with their answers and the group's other texts with
  `root.srp_hud_texts`. The Finnish "Applies after restart." is shorter ("Vaatii
  uudelleenkäynnistyksen."): the old one took two lines.
- **Off by default:** off is the game as it was (Hidden off: the stock crosshair).
- **Apply saves:** a toggle or a pick is a pending change of the tab like any option there; Apply
  saves it, Cancel or leaving without Apply puts the saved state back.
- **At once:** the hud takes a change in its next frame, in a match too (no next-match wait).
- **Stored in user.cfg** (`Documents\survarium\user.cfg`): `srp_hide_hit_markers on|off`,
  `srp_hide_scores on|off`, `srp_hide_crosshair on|off` (saved bools, as the game's own options;
  the file is written at Apply). An older game ignores the lines.
- **Per player, on this computer:** nothing is sent; other players see what they chose.

## Hidden crosshair

- No crosshair is drawn in a match: the hud's whole crosshair is hidden, its hit marker with it
  (the marker is part of it), whatever "Hide hit markers" says. Back on Dynamic or Static, the
  marker follows its own option again.

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

- **Exe** (Rust, five lohooks of `hud_options.toml`, order 2, each after another patch's hook at
  the same site):
  - `configs.start` (game::on_configs_loaded's start, after patch 79): registers the three saved
    bools (cc_value<bool>::cc_value 0x1DDB0, then cc_bool's vtable 0x9E7C4C) before user.cfg runs;
  - `options.callback` (game_options::scaleform_callback's start, after patch 79): a call named
    `srp_hide_hit_markers`, `srp_hide_scores` or `srp_hide_crosshair` answers
    `root.srp_check_option(name, label, "1"|"0")`; with "1" or "0" it saves that first (user.cfg at
    once); `srp_hud_texts` answers `root.srp_hud_texts(heading, Hide quests, Dynamic, Static,
    Hidden, Static minimap)`;
  - `player_icons.update_start` (update_player_icons' start, every frame of a match, after patch
    65): `root.srp_hud_options(hide hit markers, hide scores, hide crosshair)` to the hud movie,
    once for each new hud movie or change;
  - `static_info.options_read`, `replay.options_read` (a match's or a replay's options read,
    after patch 65): the hud is told again, since a new match can make its hud movie at the last
    one's address (as patch 65 tells its own state again).
- **Settings code:** `vostok::settings` (the Settings' call, the save of user.cfg, `ConsoleBool`),
  shared with patch 79's Language choice.
- **Hud:** `HUD.srp_hud_options` sets `Crosshair.hitMarkersHidden`, the Tab list's
  `PlayerList.namesOnly` and the crosshair's visibility.
- **Main menu:** `SrpCheckOption` (one checkbox the exe keeps by its name), two in an
  `SrpCheckGroup` (the heading, the texts asked); `SrpCrosshairChoice` (the list over the stock
  checkbox and `srp_hide_crosshair`); `Settings.layoutGameplay` places them with `SrpLanguage` and
  the stock controls; `Settings.flip` inverts Hide quests.
- Nothing a match computes changes: no `simulation_changes.tsv` row.
- **Checks:** `../../client-patches/check_hud_options_026g.py` (Unicorn: the registration and
  user.cfg's lines, the Settings' calls, the hud told at each change), Rust tests,
  `../../scripts/hud-options/check.sh` (Ruffle: the two columns' order, edges, spacing, no
  overlaps, nothing below the window, every language's texts fitting, with the note shown and
  hidden; each pending change, Apply, Cancel; Hide quests' inversion through the stock setter and
  its default; the crosshair list's mapping, Hidden keeping the stock value, closing on a press
  outside; the hit marker shown off, hidden on, none with the crosshair hidden; the Tab list as
  today off, names only on, by clan and name). Not yet run in the game.
