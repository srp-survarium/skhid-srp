# Lobby scenes

The owner (2026-10-29): "we will have a selection for them": players choose the 3D scene behind
the lobby menu. Exe patch 81 (`client-patches/rust/patches/src/lobby_scene.rs`), the main menu
movie (`flash/main_menu/src/SrpLobbyScene.as`), patch 45's weapon spot (`weapon_table_026g.asm`).

## The choice

- **Where:** Settings, Gameplay tab, the bottom panel's right half, on the Language list's row:
  "Lobby:" with the game's arrow selector (`< Classic >`, the video options' stepper; its arrows
  stop at the first and the last), the note under it; the Language choice has the left half (in
  Ruffle: `../images/lobby-scene-option.png`). The names are dictionary texts
  (`st_srp_lobby_<id>`, the label `st_srp_lobby_scene_option`, the note
  `st_srp_lobby_scene_restart`), which the exe answers translated. "STALKER 2", not
  "S.T.A.L.K.E.R. 2": the dotted one is wider than the selector's value field.
- **The lobbies**, in the selector's order (one enum, `Lobby` in `lobby_scene.rs`):

  | label             | id               | project                       |
  |-------------------|------------------|-------------------------------|
  | Classic           | `day`            | `lobby_scene/day` (stock)     |
  | STALKER 2         | `stk2_blockpost` | `lobby_scene/stk2_blockpost`  |
  | Hangar            | `hangar_046`     | `lobby_scene/hangar_046`      |
  | Camp              | `camp_069`       | `lobby_scene/camp_069`        |

- **Apply saves a pick**, as any option of the tab: a pick enables Apply and Cancel; Apply sends
  it to the exe, which writes user.cfg at once; Cancel, or leaving the tab or Settings without
  Apply, puts the saved lobby back.
- **Applies after restart:** the note "Applies after restart." shows while the saved lobby is not
  the one shown. No live swap (below).
- **Stored in user.cfg** as the console variable `srp_lobby_scene` (`srp_lobby_scene camp_069`),
  saved and read as every other setting. No line, an empty or unknown id: the classic lobby.
- **Client only:** the server never knows the choice; a match is the same whatever lobby.

## At start

- The lobby menu is made once, at the game's start (`game::create_lobby_menu`), and asks for its
  scene with its movies in one query (0x21E450); the exe asks for the chosen lobby there instead of
  `lobby_scene/day`.
- **A missing project would crash the game:** the project's cook reads its config without a
  check (`project_cooker_simple::create_game_objects`, a null config read at 0x22C11E; seen in a
  game, 2026-10-29). So another lobby than the classic is asked first as its config alone
  (`resources/projects/lobby_scene/<id>/client_project`, a binary config, no user data): a
  missing file fails only that result. When the results come, nothing has used them yet: the
  exe asks the whole query again, with the lobby's project when its config was there, else with
  the classic's. The classic is asked at once, as the stock game does.
- **A project without its camera track is the classic too:** on_render_scenes_ready plays the
  project's track (project+1E4h) without a check; such a project's results are let go and the
  query asked again with the classic.
- The classic itself failing is the stock game's failure (as without the patch).
- Another lobby costs one more query at the start (its config, the lobby's movies with it).
- Never a black scene, never a crash; the Settings then show the note (the saved lobby is not
  the one shown).

## Why at the next start, not live

- The scene is loaded once per run, with the lobby's movies, and is never unloaded until exit
  (`lobby_menu::clear_resources` 0x21D080, from `game::clear_resources` only). A match does not
  unload it: going back to the lobby after a match is the same scene.
- A live swap would redo by hand the project half of `clear_resources` and of
  `on_render_scenes_ready` (0x220B20) on a scene being drawn: the project's remove (0x2855B0) and
  insert (0x285540), its static collision objects (project+28h, 50h bytes each, into the physics
  world lobby menu+3104h), the camera track player at lobby menu+118h (restarted from
  project+1E4h with the menu's +684h listener), the camera config, the refcounts. That is the
  fragile engine state the owner asked to avoid; the owner may ask for it as its own change.

## What a lobby project must give (for the content packs)

A lobby is an ordinary `projects/lobby_scene/<id>/client_project` (and `.lua`), made as the
classic's. What the exe and our patches take from it:

- **The character stands at the origin, facing yaw pi** (looking along -z), as in the classic.
  The lobby's own character is made at (0, 0, 0) (0x21CCD2), the squad panels' at (1, 0, 0.5) and
  (-1, 0, 0.5), and the lobby camera orbits the point (0, height, 0) at 1.5..4.6 m (patch 45's
  notes, client_patch.md section 3.42). Place the scene around that spot: move and turn the
  whole scene, not the character. Keep 1.5 m of room around the origin and the floor at y 0.
- **Walls the camera stays inside have collision objects.** The lobby camera's distance (the wheel,
  1.5..4.6 m) is cut by a collision ray from the point it looks at back to the eye (0x2844D0, the
  project's static collisions): a wall without a collision object lets the eye through it. No
  per-lobby distance in data: a small room (camp_069's back wall at z -2.2) gives its walls
  collision objects, as the classic's (that the ray meets the project's static collisions is read
  from the code, not seen in a game).
- **One `track` object** (`game_object_type = "track"`), as the classic's: the lobby menu plays
  the project's track (project+1E4h) from the start. A project without one is refused (the
  classic is shown).
- **`camera`** (the root table: `position`, `direction`, `distance`), as every project has.
- **The weapon's spot** for patch 45 (a weapon's upgrade window shows the weapon in the air
  above a table): a root table `srp_lobby` with
  - `weapon = float3( x, y, z )`: where the weapon's middle floats, in the clear above a table
    (a rifle is about 1 m long and turns there; keep 0.6 m clear around it);
  - `weapon_view = <radians>`: the yaw the camera looks at it from, in the lobby camera's
    convention (forward = (-sin yaw, 0, cos yaw)): -1.5708 looks along +x, 0 along +z, 1.5708
    along -x, 3.1416 along -z. The weapon lies across the view, its length left to right. The
    camera stands 1.2 m from the spot and a little above it (pitch -0.25), with no collision
    ray: keep that line of sight clear.

  From a direction the camera should look along (dx, dz) on the floor: `weapon_view = atan2(-dx, dz)`.

  The classic's, by content edit `lobby-scenes-1.json`:
  `srp_lobby = { weapon = float3( 4.25, 1.30, 3.05 ), weapon_view = -1.5708 }` (above the
  workbench, between its drill press and its vice). A project without `srp_lobby/weapon` (or
  without `weapon_view`): the upgrade window keeps the stock camera and the weapon stays in the
  character's hands.
- **The artefact table and the bench bulb** of the classic (content edit `lobby-scene-2`, pack
  `lobby-bulb@1`) are the classic's decoration: a lobby that wants a lit table puts its own light
  (`point_light`) and props in its own project. No exe code places them.

## Checks

- Rust tests of the enum and the transition (`lobby_scene.rs`), `check_lobby_scene_026g.py`
  (Unicorn: the variable, the query's list, the probe, the fallbacks, the Settings' call),
  selftest check 81, `check_weapon_table_026g.py` (the spot from the project's config, the turned
  bench), the movie in Ruffle (`scripts/lobby-scene/check.sh`).
- In a game (2026-10-29, local, headless, a debug build that set the choice; no input but the
  sign-in): no choice, the classic; `camp_069` (not in the db), the probe failed and the classic
  came, no crash; `hangar_046` from a test patch (the classic's project without its walls), that
  scene in the lobby.
- **Not seen:** the choice read back from user.cfg in a game. In the local Wine 11.8 prefixes the
  game's query of `user_data/user.cfg` gives no resource (load_cc_script gets null, also with the
  certificate-only build), so no line of user.cfg is applied at a start there, this one and patch
  79's `localization` included; the choice is saved (the line `srp_lobby_scene <id>`) and the
  console variable takes a line (Unicorn). Windows not checked.

