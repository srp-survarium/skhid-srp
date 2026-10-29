# Lobby scenes

The owner (2026-10-29): "different ones should be for different factions"; "stalker2 one is for
bandits"; "the current one is for army"; "the room is for stalker". The 3D scene behind the lobby
menu follows the player's quest faction; there is no setting. Exe patch 81
(`client-patches/rust/patches/src/lobby_scene.rs`), patch 45's weapon spot
(`weapon_table_026g.asm`).

## The lobby of a faction

- One enum, `Lobby` in `lobby_scene.rs`, and one total match from the faction:

  | quest faction (factions_dict id) | lobby                                         |
  |----------------------------------|-----------------------------------------------|
  | Renaissance, the Army (3)        | `lobby_scene/day`, the classic (stock)        |
  | Black Market, the Bandits (2)    | `lobby_scene/stk2_blockpost`, S.T.A.L.K.E.R. 2 |
  | Scavengers (1)                   | `lobby_scene/camp_069`, the 0.69d camp        |
  | Edge (4)                         | `lobby_scene/hangar_046`, the 0.46c2 hangar   |
  | none chosen yet (0), any other   | the classic                                   |

  In code, not game data: the exe would have to read the factions' configs for a project name,
  more than the match.
- **The faction is the account's quest faction**, the last byte of `0x8E` (lc+1EA998h,
  `lobby.md` section 2.14). The server sends it at the sign-in and after every change of faction
  (`4B 00`: `8E`, then `83 4B`): the exe takes every one at `lobby_client.faction` (0x2717CF).
- **The next start shows it:** the faction is kept in user.cfg as the console variable
  `srp_lobby_faction` (`srp_lobby_faction 2`), written by the exe (user.cfg saved at once when it
  changed), never by the player. No line, an empty or unknown value: the classic.
- **Client only:** the server knows nothing of the lobby; a match is the same whatever lobby.

## When the faction is known

- **After the scene has loaded.** The lobby menu asks for its scene at the game's start, before
  the sign-in, and the network client is made in that query's own callback (on_render_scenes_ready
  0x220B20 calls game::create_network_client at 0x221C2F): no faction can come before the scene's
  results are taken. Holding the query until `8E` would hold the sign-in itself.
- So the first start, a new player's and one after a change of faction show the last start's
  lobby (the classic the first time); every later start shows the faction's from the first frame,
  with no swap.
- **A change of faction during a session applies at the next start**, as does the first `8E` of
  a session that differs from the saved one (no hint: nothing for the player to do).

## At start

- The lobby menu is made once, at the game's start (`game::create_lobby_menu`), and asks for its
  scene with its movies in one query (0x21E450); the exe asks for the saved faction's lobby there
  instead of `lobby_scene/day`.
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
- Never a black scene, never a crash.

## After a match (the owner's rule, not built yet)

The owner (2026-10-29): "for lobbies the switch should happen after joining a faction and then
playing a game": after a change of faction, the new faction's lobby from the first frame back from
the next match. Not in this PR yet; what it needs, read from the code (PROVEN unless tagged):

- **One state machine:** (the lobby shown, the lobby wanted) and three events: a faction (`8E`,
  the wanted lobby; user.cfg as now), the match entered (`lobby_menu::on_deactivate` 0x21D4A0:
  the lobby not drawn; when wanted is not shown, ask), back in the lobby (`on_activate`
  0x21D2E0). The scene stays loaded across a match (nothing unloads it); a swap while the match
  runs is not seen.
- **The new project:** the lobby's own query asked again (0x21E450, the probe first as at a
  start), its other seven requests made cheap (a small binary config each, no user data) so no
  second render scene, sound scene or movie is made; its results taken at
  `lobby_menu.scenes_ready` and on_render_scenes_ready not run.
- **The old project out** (`lobby_menu::clear_resources` 0x21D080's project half): `remove`
  0x2855B0 (ESI the project: its game objects' vtable+24h, its render visuals); each static
  collision (project+28h, +2Ch of them, 50h bytes, one class: vtable 0xA029A8, remove 0x2F8970,
  insert 0x2F8850, thiscall with the physics world, lobby menu+3104h); the game effect player
  (lobby menu+118h) stopped (0x2DE7F0, EDI player+8; +118h then 0); lobby menu+114h 0 and the
  project's reference let go (its count +108h; at 0 engine_release_object with ECX project+38h).
- **The new project in** (on_render_scenes_ready's project half): its reference taken, lobby menu
  +114h; its vtable+0Ch (simple_game_project 0xA0A064: 0x2F8720, thiscall); the game effect player
  restarted with the project's emitter (project+1E4h): the effect made by the emitter's
  vtable+20h (thiscall: out, 0.0, 1.0; ret 0Ch), played by 0x2DE090 (EDI the player; the effect,
  0, 1, 0, the menu's listener +684h; ret 14h), the made effect let go (its count +1Ch; at 0 its
  owner's (+0Ch) vtable+1Ch); `insert` 0x285540 (EAX the project); each static collision's insert.
  The project's `camera` (0x242770) is not needed: every lobby's character is at the origin.
- **Two true virtual calls:** the emitter is polymorphic (game_effect_emitter: composite,
  random permutation, hud, ... cooks), so the effect's making and letting go are vtable calls;
  the engine-function library has no virtual kind yet (`engine_026g.tsv`: a function is a VA).
  That kind (a slot offset, thiscall, its thunk `mov eax,[ecx] / call [eax+slot]`) comes first.
- **Not known:** what the lobby's emitter plays (an ambient game effect; GUESS), and how long a
  project's cook takes against a short match (a swap that comes after the return would show).

## Why not live

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

- Rust tests of the factions, the match and the transitions (`lobby_scene.rs`),
  `check_lobby_scene_026g.py` (Unicorn: the variable, `0x8E`'s faction kept and saved, the query's
  list, the probe, the fallbacks), selftest check 81, `check_weapon_table_026g.py` (the spot from
  the project's config, the turned bench).
- In a game (2026-10-29, local, headless, no input but the sign-in; the default build of this
  branch; packs lobby-hangar-046@2 and lobby-camp-069@2 in a test patch, no STK2 pack):
  `srp_lobby_faction 4` in user.cfg, the hangar from the first frame; `1`, the camp; `2` (no
  STK2 project), the probe failed and the classic came, no crash; at each sign-in the Black
  Market account's faction was saved (`srp_lobby_faction 2`). A debug build without a saved value:
  the classic; a test patch's `hangar_046`, that scene.
- An early finding of this PR, that user.cfg is never read under Wine, was wrong: it was this
  work's own headless prefix (load_cc_script got no resource for `user_data/user.cfg` there, the
  cause not found); in a local-clients `run.py` prefix the file is read and the saved faction
  applies. Debug builds of this work crashed on a user.cfg line of a `cc_string` (`r_resolution
  1280x720`, the one run.py writes): a debug-only hook at cc_string::execute read its argument
  off the wrong stack slot (never in the branch). check_lobby_scene_026g.py runs whole user.cfgs
  (run.py's LF file, the game's CRLF one, no final end of line) through the console's own load.
