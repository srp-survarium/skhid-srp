# Lobby scenes

The owner (2026-10-29): "different ones should be for different factions"; "stalker2 one is for
bandits"; "the current one is for army"; "the room is for stalker". The owner (2026-10-30): by
faction, not by level, one lobby per faction: the Black Market the STK2 bunker (not the
blockpost), the Edge the hangar; the STK2 blockpost and bar ship nowhere. The 3D scene behind
the lobby menu follows the player's quest faction; there is no setting. Exe patch 81
(`client-patches/rust/patches/src/lobby_scene.rs`), patch 45's weapon spot
(`weapon_table_026g.asm`).

## The lobby of a faction

- One enum, `Lobby` in `lobby_scene.rs`, and one total match from the faction:

  | quest faction (factions_dict id) | lobby                                         |
  |----------------------------------|-----------------------------------------------|
  | Renaissance, the Army (3)        | `lobby_scene/day`, the classic (stock)        |
  | Black Market, the Bandits (2)    | `lobby_scene/stk2_bunker`, S.T.A.L.K.E.R. 2   |
  | Scavengers (1)                   | `lobby_scene/camp_069`, the 0.69d camp        |
  | Edge (4)                         | `lobby_scene/hangar_046`, the 0.46c2 hangar   |
  | none chosen yet (0), any other   | the Scavengers' camp                          |

  In code, not game data: the exe would have to read the factions' configs for a project name,
  more than the match.
- **The faction is the account's quest faction**, the last byte of `0x8E` (lc+1EA998h,
  `lobby.md` section 2.14). The server sends it at the sign-in and after every change of faction
  (`4B 00`: `8E`, then `83 4B`): the exe takes every one at `lobby_client.faction` (0x2717CF).
- **The next start shows it:** the faction is kept in user.cfg as the console variable
  `srp_lobby_faction` (`srp_lobby_faction 2`), written by the exe (user.cfg saved at once when it
  changed), never by the player. No line, an empty or unknown value: the Scavengers (the owner,
  2026-10-30: "no faction falls back to scavengers"; no account can switch to none). The
  faction is parsed so at the boundary (the 0x8E byte, the user.cfg value): the lobby's match is
  over the four factions alone.
- **Client only:** the server knows nothing of the lobby; a match is the same whatever lobby.

## When the faction is known

- **After the scene has loaded.** The lobby menu asks for its scene at the game's start, before
  the sign-in, and the network client is made in that query's own callback (on_render_scenes_ready
  0x220B20 calls game::create_network_client at 0x221C2F): no faction can come before the scene's
  results are taken. Holding the query until `8E` would hold the sign-in itself.
- So the first start, a new player's and one after a change of faction show the last start's
  lobby (the camp the first time); every later start shows the faction's from the first frame,
  with no swap.
- **A change of faction during a session shows after the next match** (below), as does the first
  `8E` of a session that differs from the saved one (no hint: nothing for the player to do).

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

## After a match

The owner (2026-10-29): "for lobbies the switch should happen after joining a faction and then
playing a game": after a change of faction, the new faction's lobby from the first frame back from
the next match. Built (PROVEN in the code; the swap run with Unicorn against the original's code on
the same objects; not run in a game yet):

- **One state machine** (`Scene` in `lobby_scene.rs`, one transition function `Scene::step`): not
  asked; starting (the start's query, as above); loaded, with the lobby shown, whether it is
  drawn (drawn, hidden, closed) and a swap's query in flight (the lobby it is for and what it asks).
  The events: the query built (`lobby_menu.scene_request`), its results (`lobby_menu.scenes_ready`),
  the lobby hidden (`lobby_menu::on_deactivate` 0x21D4A0: the active scene switched, a match;
  game::switch_to_lobby and the match's switch call the old scene's vtable+4), the lobby shown
  (`on_activate` 0x21D2E0), the game closing (`game::clear_resources` 0x1ED030, which deactivates
  the active scene too). The wanted lobby is the saved faction's at each event (`srp_lobby_faction`
  stays its one owner, written as before).
- **The lobby hidden, another faction saved than the lobby shown:** the lobby's query is asked
  again (query_resources 0x21E450 with the menu) with that lobby alone: its config first (as at a
  start), then its project; the other seven requests are that lobby's config (a binary config,
  no user data), so no second render scene, sound scene or movie is made. Back to the classic:
  its project at once. One such query at a time: a match entered while one is out asks nothing.
- **Its project in, while the lobby is hidden** (`ScenesReady::swap`): the project halves of
  `lobby_menu::clear_resources` (0x21D080..0x21D100) and `on_render_scenes_ready` (0x220DA2..
  0x220EC3, 0x22193E..0x221975), in their order, each step the game's own function:
  - the old project out: `remove` 0x2855B0 (ESI the project); each static collision's remove
    (project+28h, +2Ch of them, 50h bytes; static_collision vtable 0xA029A8 +4: 0x2F8970, thiscall
    with the physics world, lobby menu+3104h); the track player's stop (0x2DE7F0, EDI menu+120h),
    +118h 0; +114h 0 and the menu's reference let go (count +108h, lock xadd; at 0
    `engine_release_object` 0xE0210, ECX project+38h, EAX the count);
  - the new project in: its reference taken, +114h; simple_game_project's vtable+0Ch (0x2F8720,
    thiscall); the stop and +118h 0 again; its track (+1E4h) made into an effect by the emitter's
    vtable+20h (`emit`: thiscall, out, 0.0, 1.0; ret 0Ch; out returned), played by 0x2DE090 (EDI
    menu+118h; the effect, 0, 1, 0, the listener menu+684h; ret 14h), the made effect let go (count
    +1Ch; at 0 its emitter's (+0Ch) vtable+1Ch, `destroy`); `insert` 0x285540 (EAX the project);
    each static collision's insert (vtable+0, 0x2F8850).
  The two emitter calls are the engine-function library's virtual rows (client_patch.md 0.5);
  nine emitter classes (vtables.tsv). The project's `camera` (0x242770) is not set again: every
  lobby's character is at the origin (below). clear_resources' byte +3110h (set at the exit) is not.
- **Never while the lobby is drawn:** results that come after the return (a short match, a slow
  disk) are let go untouched, and the next match asks again. A swap is never seen: the first frame
  back is the new lobby, or the old one when its project was late.
- **A missing pack is the classic:** a config that is not there, or a project without its camera
  track, makes the classic's project the one asked (none when the classic is shown: let go). The
  classic failing keeps the lobby shown. The next match asks the faction's config again (a few
  failed config reads, nothing else).
- **The game closing:** from game::clear_resources on, nothing is asked (its on_deactivate of the
  lobby asks nothing), and a swap's results that come after it are let go without the lobby menu
  being touched.
- **The faction changed again while a query is out:** results for the old wanted lobby are let go,
  and the new one is asked (none when it is the one shown).
- **Costs and not known:** two lobby projects are in memory for the moment of the swap; the
  project's cook runs while the match loads or runs (its disk reads beside the match's: GUESS, not
  measured); the eight requests of a swap's query name one config (the resource system's handling
  of one name eight times in a query is read from the stock flow only, not seen). What the lobby's
  track effect plays (an ambient effect; GUESS).

## Why not live

- The scene is loaded once per run, with the lobby's movies, and is never unloaded until exit
  (`lobby_menu::clear_resources` 0x21D080, from `game::clear_resources` only). A match does not
  unload it: going back to the lobby after a match is the same scene.
- The swap (above) runs only while the lobby is not drawn. A live swap (on a scene being drawn,
  the player watching) is not built: the owner asked for the switch after a match.

## What a lobby project must give (for the content packs)

A lobby is an ordinary `projects/lobby_scene/<id>/client_project` (and `.lua`), made as the
classic's. What the exe and our patches take from it:

- **The character stands at the origin, facing yaw pi** (looking along -z), as in the classic.
  The lobby's own character is made at (0, 0, 0) (0x21CCD2), the squad panels' at (1, 0, 0.5) and
  (-1, 0, 0.5), and the lobby camera orbits the point (0, height, 0) at 1.5..4.6 m (patch 45's
  notes, client_patch.md section 3.42). Place the scene around that spot: move and turn the
  whole scene, not the character. Keep 1.5 m of room around the point (0, 1.2, 0) but the floor,
  the floor at y 0 under all three, and nothing within 0.4 m of the squad's two spots (the
  exe's, fixed: the audit found the blockpost's campfire, the bar's planks and the bunker's
  leaf pile there, each moved or left out in its pack).
- **The lobby camera** (lobby_camera, read from the code: ctor 0x283BC0, tick 0x283EF0, ray
  0x2844D0; re-kit abi.tsv): the point (0, 1.2, 0); the yaw has no limit; the pitch is clamped to
  +-60 degrees every tick (0x2840C5); the wheel's distance target is 1.5..4.6 m. The current
  distance is cut by a collision ray from the point back to the eye (group 10h, mask 8: it meets
  static collision of cgroup 8 / cmask 16, the props' `collision` and camera boxes, not
  `walker_collision`), with no margin, then clamped to 1.5..4.6 again (0x28430E): **the eye
  passes anything nearer than 1.5 m**, and stops exactly on what the ray meets. So at the
  steepest upward pitches (above about 53 degrees, the floor 1.2 m below) every lobby, the
  classic too, shows the floor from below: the stock camera's (not fixed: its pitch limit is the
  owner's call). Fixed constants all; no per-lobby camera limits are needed.
- **Camera boxes keep the eye in the room.** Invisible boxes of the camera's collision alone
  (0.26's unit primitive `editor/walker_collision`, scaled; cgroup 8, cmask 16): a cage inside
  the walls, the floor and the ceiling, 0.15 m in (the eye stops short of them, never on them,
  where its near plane would see through), closing doorways and windows, and around the props
  the eye would get into (counters, crates, beds, barrels). The STK2 porter's `camera_boxes`,
  the 0.46c2/0.69d porter's `camera_walls`. Checked statically (below, "The audit").
- **One `track` object** (`game_object_type = "track"`), as the classic's: the lobby menu plays
  the project's track (project+1E4h) from the start. A project without one is refused (the
  classic is shown).
- **`camera`** (the root table: `position`, `direction`, `distance`), as every project has.
- **The weapon's spot** for patch 45 (a weapon's upgrade window shows the weapon in the air
  above a table): a root table `srp_lobby` with
  - `weapon = float3( x, y, z )`: where the weapon's middle floats, in the clear above a table
    (a rifle is about 1 m long and turns there; 0.40 m above the top, as the classic's, and 0.6 m
    clear around it but the top);
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
- **No table:** the lobby gets one, 0.26's own `props/furniture/workbench_02` (top 0.84 m)
  against a wall or a prop in the frame (the porters' `native_props`): the bar's back left
  corner, the hangar's sandbags.
- **The artefact table and the bench bulb** of the classic (content edit `lobby-scene-2`, pack
  `lobby-bulb@1`) are the classic's decoration: a lobby that wants a lit table puts its own light
  (`point_light`) and props in its own project. No exe code places them.

## The camera flight to the weapon (patch 45, round 7)

The weapon's upgrade window flies the camera from where it is to the spot's view, and the close
flies it back: the eye on a straight line between the two eyes (over a character's head where the
line comes within 0.9 m of one: the camp's spot is behind the character), the yaw, the pitch, the distance,
the side angle and the zoom with it, one smoothstep over 0.45 s; back, to where the stock camera
will be (the wheel's target cut by the camera's own ray there), held a tick with no ray, then the
stock camera from exactly there. The Rust patch `weapon_flight` (one enum `Flight`, one
`Flight::step`), at `lobby_camera.view`; patch 45's NASM keeps the weapon at the bench and its
requests (client_patch.md section 3.42, round 7). Until round 6 the flight glided back to the
wheel's target with no ray, out through a small room's wall, and the first stock tick snapped
the eye back by up to 3.1 m (the camp, the bunker; 2.6 m in the classic, through the floor).

## The audit (2026-10-30)

The owner: "other team members locations, camera panning and zooming, and camera movement for
weapon upgrade table". Static: every lobby's visual meshes (LOD0, each surface's two-sidedness)
and the static collision the camera's ray meets, ray-cast with embree; the orbit sampled at 72
yaws, 25 pitches and 7 distances; the squad's spots; the weapon's spot and the flight from 72
starts. The share of (yaw, pitch) whose eye is behind a visible surface at the wheel's 4.6 m (the
stock floor's 8 % in every lobby included), before and after the packs (the blockpost and the
bar were audited before the owner's pick of 2026-10-30 and ship nowhere):

| lobby | before | after | what was wrong, what changed |
|---|---|---|---|
| classic `day` | 13.8 % | 13.8 % | the pipes overhead (no collision), the floor at the steepest pitch: stock, unchanged |
| `stk2_blockpost` (lobby-stk2@2) | 9.3 % | 8.0 % | the squad's left member in the campfire (moved to (-1.15, 0, 1.65)); the eye in the front wall, out through the doorway onto the porch, in the bunk: camera boxes; the weapon 0.40 m above the bunk |
| `stk2_bar` (lobby-stk2-bar@2) | 4.3 % | 8.2 % (the stock floor's rows; the eye no longer under the planks) | the characters 4 cm above the floor (the scene 0.038 m up); the right member in planks (left out); the weapon over nothing (a workbench in the back left corner); a floor box |
| `stk2_bunker` (lobby-stk2-bunker@2) | 15.7 % | 8.1 % | the left member and the own character's feet in the leaf pile (left out), crates 1.3 m from the point (the character 0.25 m over); the eye in the crates, barrels, metal boxes, cart: camera boxes; the weapon 0.16 m above a box (a clear top) |
| `hangar_046` (@6) | 8.0 % | 8.2 % | open sky (as the 0.46 hangar); the weapon 1.3 m above the dirt (a workbench against the sandbags); a ground box |
| `camp_069` (@7) | 18.6 % | 8.7 % | the eye in the corner counters, the bed pillows, the oxygen cylinder, at the ceiling: camera boxes; the weapon 0.29 m above the tarp (0.39) |

Game captures: the diagnostics build's `camera_sweep` (client_patch.md section 3.42, round 7).

## Checks

- Rust tests of the factions, the match and the transitions (`lobby_scene.rs`: the start's paths,
  a change shown after the next match, the probe then the project, the classic for a missing pack
  or a project without its track, late results let go, the query in flight at the next match, a
  faction changed while one is out, the close; every sequence of six events of seven kinds, under
  three loads: never a swap while drawn, never a query once closed),
  `check_lobby_scene_026g.py` (Unicorn: the variable, `0x8E`'s faction kept and saved, the query's
  list, the probe, the fallbacks; after a match: on_deactivate's query or none, the swap's lists,
  the swap's 14 calls and state against the original's clear_resources and on_render_scenes_ready
  halves on the same objects, late results, the close, a missing pack), selftest check 81 (the
  seven sites, the stock code the swap redoes), `check_engine_026g.py` (the virtual rows),
  `check_weapon_table_026g.py` (the spot from the project's config, the turned bench).
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
