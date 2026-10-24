# Hide the interface (0.26g)

A player's ask through the owner (2026-10-23): a key that hides the whole interface, in the lobby
too, for screenshots.

## The key

- **F11 hides the interface and shows it again.** It is the game's own action
  `toggle_replay_ui` (shown and hid only the replay viewer's controls before), so a replay keeps
  F11 as it was.
- **Options, Controls, "Hide interface"** (Скрыть интерфейс, Сховати інтерфейс, Oberfläche
  ausblenden, Ukryj interfejs): the last row of the game keys, F11 by default. Rebound there like
  any other key, saved and read back as the others (`bind toggle_replay_ui <key>` in `user.cfg`).
  The owner, 2026-10-24: "there is no in settings option to switch it".
- **Why F11:** the game has no screenshot key; F12 is Steam's, Print Screen Windows'. F11 did
  nothing in a match or the lobby. Free in every group otherwise: B, H, J, U, Y, Z, 9, 0, F4,
  F7-F10, F12, Caps Lock (L is the game's in a match).

## In a match

- **Hidden:** the whole hud (health, ammo, kill feed, chat, timers, the fps line), the crosshair,
  the minimap, the markers over heads and batteries. The 3D world, the weapon and the Esc menu
  stay.
- **F11 again** shows it.
- **Unseen, not held back:** the hud keeps running while hidden; kill-feed lines and messages of
  that time come and go unseen, and showing it shows only what is current.
- **Before the start:** F11 works during the countdown too, the kits with the rest; the start
  keeps what the player chose (the owner, 2026-10-24).
- **After a death:** the interface comes back by itself when the kits are shown, so the death
  screen's kits are never hidden.
- **The next match** starts with the interface shown.
- **Not taken:** the key also goes on to the game as before (nothing uses F11 there).

## In the lobby

- **Hidden:** the whole interface: panels, the top and bottom bars, windows, popups, messages,
  chat, tooltips, right-click menus (an open one is closed), the item being dragged, the loading
  screen; one coming up while hidden stays hidden. Only the character and the scene stay; drag and
  wheel still turn and zoom the camera.
- **Clicks** land on nothing hidden.
- **Shown again by:** F11, any key the lobby movie gets (letters, digits, arrows, Enter,
  Backspace...), Esc, a match found, its loading, the results.
- **The key that shows it does nothing else**, neither its press nor its release: Enter does not
  press Play nor answer a message hidden with the interface, R does not open the rooms, a letter
  does not start the search, C, I, S, F and O open no window.
- **Other F keys** (the movie never gets them) do nothing while hidden: F6 does not press Play.
- **Not while typing:** in a text box F11 is the box's, as before.
- **The cursor stays** (its own movie); move it aside for a picture.

## How

- Exe patch 62 (`hide_ui_026g.asm`; `client_patch.md` 3.55): in a match it calls the hud and
  player icons' movies' `hide_interface`; in the lobby the lobby movie's `toggle_interface`.
- The movies (lobby, hud, player icons): `InterfaceHider` hides every layer of the movie's stage
  and keeps it hidden; the movie goes on running.
- The controls row: patch 62 gives the options a 41st row (its table moved into the arena); its
  label is content edit `hide-interface-1.json`.
- No simulation change, no server change.

## Later: a match without UI

The owner's ask (2026-10-23), specced, not built: "the no-UI battery mode", Battery Hunt's rules
with no UI for anyone; the teams told apart by coloured bands on the characters.

### Rules

- **Rules:** mode 3's, unchanged (rule 89: 10 batteries, one every 60 +- 10 s from the chargers,
  900 s, respawn 10 s, the battery caps of `battery_credit.md`).
- **Hidden for everyone, the whole match:** crosshair, minimap, the Tab list's map, the battery
  bar, markers over heads and batteries (`player_icons.swf`), ammo, health, kill feed, points
  pop-ups, the container icon.
- **Kept:** the kit screen (after a death, before the start, keys 1-3), the respawn and start
  countdowns, the Tab list's rows, chat, the start and end messages, the Esc menu, the lobby's
  results screen.
- **F11** (patch 62) still hides what is left; it never brings a hidden part back.
- **Bots:** not offered (srp's bots play modes 2 to 4).

### Enforcing it

- **What the client gets** (PROVEN, `modes_maps.md` section 3): one u8 `map_id`; `maps_dict` names a
  config `gameplay/game_modes/<name>.options` holding `mode.mode`, the rules and the project.
- **Proposed: a flag in the config, not a mode id.** Copies of the 25 mode-3 configs (levels 1-8
  day, evening, rain; Cologne pre-rain; London day) with `mode.parameters.no_ui 1`, their own
  `ui.mode_name`, the same projects (`level_0N/<w>_3`), new `maps_dict` rows after Research's
  184-203 (u8: 204-228 leaves 27 ids).
- **The exe reads it once at load**, as patch 53 reads `mode.parameters` (the mode config's root
  at +Ch of the rule's options, `research_native_026g.c`).
- **Not show_ui(false):** `game_world::show_ui` (0x1F95F0) hides all of `hud.swf`, and the kits,
  the Tab list, chat and messages are in it (`HUD.as`: `_profilePanel`, `player_list`, chat).
  - `player_icons.swf` hidden all match (its `hide_interface`, as patch 62 hides it);
  - `hud.swf` stays and gets a new `root.set_bare(true)`, which hides the parts above and keeps
    them hidden whatever the exe calls later (`show_crosshair`, `update_objects`, ...);
  - patch 62's show path leaves `player_icons.swf` hidden in such a match.
- **Replays and late joiners** get it from the map id; not a simulation change (no
  `simulation_changes.tsv` row). A game without the patch would show the HUD: the server asks
  for its revision, as for Research.
- **Cost, mode id vs flag:**
  - a mode id (6): every `mode.mode` switch learns it: the exe (welcome 0x23B351, `set_game_type`
    0x234D66 and the battery bar, end messages), the lobby movie (`GameMode`, results, rooms),
    srp's 12 files that name a mode (static info weights, rules and snapshot decode, battery caps,
    rewards, balance, game, registry, bots): mode 3's behaviour copied into each;
  - a flag: the match is mode 3 to everything but the hud; srp reads `no_ui` into `MapMode`.

### Reading the game without markers

- **Chargers** (PROVEN, level_01 `day_3` project, `victory_item_spawn_points[].attached_c_objects`):
  `electric_drone` and `random_work` sounds (`sounds/single/devices/electric_device_charger/`), a
  `point_light`, spark particles, a lamp, a turning windmill. 4 stations, 3 in play
  (`spawn_points_count`). unknown: whether an idle station is quiet and dark.
- **A free battery:** no sound, light or particle (`gameplay/victory_items/default.options`:
  `item/quest/electrobox`, a collision box). Found by eye at the chargers or where a carrier died.
- **Containers:** `item/storage_device/box_atlant12` and wires at each base; no light, sound or
  particle (PROVEN on level_01 day_3; other maps not read). Batteries shown inside: unknown.
- **The carrier:** holds the box in both hands, weapon away (`hands_only` show/idle animations);
  5 % slower (`movement_speed_modifier` -0.05). Others see the box: the cue, nothing to add.
  LIKELY he sees it too (one model for both views, `weapon-camo.md`).
- **Score in the match:** `set_game_progress` still comes every frame (0x2364C0, `client_patch.md`
  3.50); bare, the movie shows the two counts in the Tab list's header. Option: a 2.5 s line on
  each change ("Blue 4 : 3 Red", `set_parameterized_message`).
- **Score at the end:** the lobby's results screen, as mode 3.
- **Spotting** (patch 38) shows nothing; its award still counts (LIKELY left).
- **Team kills** exist (event 4, `results.md`): the bands are the only friend-or-foe cue.

### Team bands, cheapest first

- **(a) A team camouflage by the armour bake** (`client_patch.md` 3.33, 3.41, 3.44: the profile
  visual's parts 0x204560, `camouflage_decal_cook_parameters`, the baker 0x12D1D0):
  - in such a match the exe sets each player's vest (and trousers) camouflage to its team's:
    the stock `textures/character/human/camo/black_market/blue_leather` and `red_leather`;
  - relative colours (own team blue, other red, as Research's HUD): each game bakes on its GPU;
  - reads at distance: well (the jacket and sleeves, as far as each vest's camo mask goes);
    players' own camos and decals are not shown in the mode;
  - only torso and legs bake: 43 `_aoac` textures in 0.26g (22 torso, 21 legs; no head, helmet
    or mask);
  - a decal band instead (layers P+0, P+270h): one-sided (the occlusion keeps the first surface),
    0.5 m at most, its place per vest: worse;
  - cost: one exe patch, no content; bakes on a match's first sight of each vest and team,
    cached as armour's (1 GiB). unknown: the player's team when the visual is built (LIKELY:
    `+6B8h -> +45ECh`, as patch 53 reads it). Every vest checked in a game.
- **(b) An armband or helmet band mesh:** none in any build. Searched (armband, arm_band, armlet,
  ribbon, band, bandana, sash, chevron, insignia, stripe, patch, povyaz): `vostok-port list` of
  0.1.1a-e, 0.20e, 0.21d, 0.23h, 0.26e, 0.26g; the path lists of 0.27d0, 0.28a2, 0.28d0, 0.31a2,
  0.46c2; both STK2 archives. Only `gameplay/items/drugs/bandages.options`. Needs a hand-made
  model and an eighth character part (or a team copy of each vest): not advised.
- **(c) An emissive or outline per team:** no outline pass or team tint known in 0.26g; a
  material per team needs shader and material-instance RE: unknown, the largest.

### Effort and first version

- **PR 1, content + srp + rooms:** 25 configs, `maps_dict` rows, the mode's name and description
  in six languages; `MapMode::no_ui`; the rooms' mode, name and colour: about 1-1.5 days.
- **PR 2, exe + hud movie:** the flag, `player_icons.swf` off, `set_bare`, patch 62's lock, the
  Tab header's score: about 2 days.
- **PR 3, exe:** the team camouflage (a): about 1-2 days, plus every vest seen in a game.
- **Total:** 3 PRs, 4-6 days, one combined test round (two clients, a 1 v 1 room).
- **First version:** PRs 1-3, (a) on vests only with the stock leathers, the score at Tab and the
  end only, the maps experimental (`SRP_ROOMS_EXTRA_MAPS`): a creator adds them by hand.

### Offering it

- **Rooms:** its own mode in the create window (beside Protective device, chargers, TDM,
  Research): srp gives a flagged map's pool entry a lobby-only mode code (6, `PoolEntry::mode`);
  the vote's cards read map, weather and that mode. Never sent to a match.
- **Pool:** 100 entries today (75 + Research's 25); +25 is 125 of `POOL_BITS` 160.
- **Default pool and Play:** only on the owner's word.
- **Open for the owner:** the mode's name; the use prompt, ammo and health shown or not; the
  score line on each change.
