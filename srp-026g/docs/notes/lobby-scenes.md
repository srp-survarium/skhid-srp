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
- **In a squad, the commander's lobby** (the owner, 2026-10-30: "when someone joins your squad,
  they go into your lobby, so it switches for them"): the lobby shown is
  `Squad::lobby(own faction)`, the commander's faction's in a squad, the player's own out of one.
  The commander's quest faction is srp's own last byte of `0xA1` (the squad info, sent again after
  every change of the squad, and after a commander's faction change: `squads_dirty`); the stock
  reader (read_squad_info 0x274720) stops before it, and a client that does not read it signs in
  and stays (seen 2026-10-30: a current exe against a server sending five bytes more). The exe
  reads it at `lobby_client.squad_read` (a squad's revision kept, 0x2748BC) and the squad id at
  `lobby_client.squad_id` (0x2747C3, both paths: 0 is no squad); parsed once into
  `Squad::{None, Led(Faction)}` (a squad without the byte: none). Joining: the joiner's lobby
  becomes the commander's, the commander's own stays. Leaving, kicked, the squad disbanded: back to
  one's own. A new commander: everyone left switches to theirs. The commander's faction change: the
  whole squad switches. A member's own change: saved for later, nothing seen while in the squad.
  `srp_lobby_faction` stays the player's own faction (a restart out of a squad opens it).
- **Client only:** the server knows nothing of the lobby; a match is the same whatever lobby (no
  simulation change: 0xA1 is a lobby message).

## When the faction is known

- **After the scene has loaded.** The lobby menu asks for its scene at the game's start, before
  the sign-in, and the network client is made in that query's own callback (on_render_scenes_ready
  0x220B20 calls game::create_network_client at 0x221C2F): no faction can come before the scene's
  results are taken. Holding the query until `8E` would hold the sign-in itself.
- So the first start, a new player's and one after a change of faction show the last start's
  lobby (the camp the first time); every later start shows the faction's from the first frame,
  with no swap.
- **A change during a session shows at once** (below), as does the first `8E` of a session that
  differs from the saved one, and a squad's `0xA1`.

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

## A change shown at once

The owner (2026-10-30): "the new lobby appears right when the player joins/switches to a faction";
it should look smooth (no hitch, no pop-in, no camera jump). Built (PROVEN in the code; run with
Unicorn against the original's code on the same objects; captured in a game, below):

- **One state machine** (`Scene` in `lobby_scene.rs`, one transition function `Scene::step`): not
  asked; starting (the start's query, as above); loaded, with the lobby shown, whether it is drawn
  (drawn, hidden, closed) and where a change is (`Change`: settled; a lobby missing; loading, the
  lobby it is for and what its query asks; fading to black with the project held; holding, frames
  drawn behind the black; appearing). The events: the query built (`lobby_menu.scene_request`),
  its results (`lobby_menu.scenes_ready`), a frame of the lobby drawn (`lobby_camera.view`, the
  lobby camera's tick, after patch 45's flight), the lobby hidden (`lobby_menu::on_deactivate`
  0x21D4A0, a match), shown (`on_activate` 0x21D2E0), the game closing (`game::clear_resources`
  0x1ED030). The wanted lobby is `Squad::lobby` of the saved faction and the squad at each event.
- **A frame with another lobby wanted than the one shown:** the lobby's query is asked again
  (query_resources 0x21E450 with the menu) with that lobby alone: its config first (as at a
  start), then its project; the other seven requests are that lobby's config (a binary config, no
  user data), so no second render scene, sound scene or movie is made. The old lobby is drawn,
  and turns with the mouse, while it loads. One change at a time; the last lobby wanted wins (a
  change while its query is out asks the new one; back to the one shown lets it go).
- **Its project in:** held (one reference of ours, `vostok::lobby`'s `HELD`, its one owner), and
  the scene fades to black: the lobby movie's `srp_scene_fade` (1, 250 ms), a black under every
  panel, over the 3D scene (the root's first child; the interface stays; get_mouse_over leaves it
  out, so the camera still turns). After the fade (`FADE_OUT`, 0.25 s of the lobby's frames) the
  project is put in the old one's place (`put_in`): the project halves of
  `lobby_menu::clear_resources` (0x21D080..0x21D100) and `on_render_scenes_ready` (0x220DA2..
  0x220EC3, 0x22193E..0x221975), in their order, each step the game's own function:
  - the old project out: `remove_static` 0x2855B0 (ESI the project); each static collision's
    remove (project+28h, +2Ch of them, 50h bytes; static_collision vtable 0xA029A8 +4: 0x2F8970,
    thiscall with the physics world, lobby menu+3104h); the track player's stop (0x2DE7F0, EDI
    menu+120h), +118h 0; +114h 0 and the menu's reference let go (count +108h, lock xadd; at 0
    `engine_release_object` 0xE0210, ECX project+38h, EAX the count);
  - the new project in: its reference taken, +114h; simple_game_project's vtable+0Ch (0x2F8720,
    thiscall); the stop and +118h 0 again; its track (+1E4h) made into an effect by the emitter's
    vtable+20h (`emit`: thiscall, out, 0.0, 1.0; ret 0Ch; out returned), played by 0x2DE090 (EDI
    menu+118h; the effect, 0, 1, 0, the listener menu+684h; ret 14h), the made effect let go (count
    +1Ch; at 0 its emitter's (+0Ch) vtable+1Ch, `destroy`); `insert_static` 0x285540 (EAX the
    project); each static collision's insert (vtable+0, 0x2F8850); our reference given back.
  The two emitter calls are the engine-function library's virtual rows (client_patch.md 0.5); nine
  emitter classes (vtables.tsv). The project's `camera` (0x242770) is not set again: every lobby's
  character is at the origin (below).
- **Behind the black:** with the swap the eye adaptation is reset (the engine's own: the byte
  0xAC914C, 1 in the image for the first frame; while set, stage_postprocess's
  compute_per_pixel_eye_adaptated_luminance 0x1B1400 takes the frame's own luminance as the last
  one's and clears it: the exposure is at once where it settles, no swing) and the camera is put
  at its ctor's pose (yaw, pitch, side 0, distance 1.96 m; not while patch 45's camera is at the
  bench or flying). `HOLD_FRAMES` (6) frames are drawn behind the black (the new scene's first
  frames), the adaptation is reset again and the scene fades in (2, 400 ms).
- **A lobby that is not there keeps the one shown, with no fade:** a config that is not there, or
  a project without its camera track, lets the results go; the lobby is not asked again until
  another one is wanted (`Change::Missing`). At the start the classic stands in (as above).
- **A match entered during a change:** a project held is put in at once (the lobby is not drawn)
  and the fade is gone; a query out is taken when it comes, put in at once while the lobby is
  hidden. The lobby on return is the wanted one, with no fade.
- **The game closing:** a held project is let go (our reference), nothing is put in; from then on
  nothing is asked and results are let go without the lobby menu being touched.
- **Costs and not known:** two lobby projects are in memory from the results to the swap (0.25 s);
  the project's cook runs while the old lobby is drawn (its disk reads beside the lobby's frames:
  GUESS, not measured); the eight requests of a change's query name one config (the resource
  system's handling of one name eight times in a query is read from the stock flow only, not
  seen). What the lobby's track effect plays (an ambient effect; GUESS). F11's hidden interface
  hides the black too (the root): a change while it is hidden is a cut, not a fade.

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

- Rust tests of the factions, the squad, the match and the transitions (`lobby_scene.rs`: the
  start's paths, a start on a missing lobby not asked again, a change loaded behind the old lobby
  then faded, swapped and faded in, a missing pack or a project without its track kept with no
  fade, the last lobby wanted winning, a match entered at every frame of a change, the close;
  every sequence of six events of eight kinds, under three loads: a swap only behind the black or
  while not drawn, a query only while drawn, one project held at most, no black left once
  settled), `check_lobby_scene_026g.py` (Unicorn: the variable, `0x8E`'s faction kept and saved,
  the query's list, the probe, the fallbacks; the lobby camera's tick through its chain: the
  change's query lists, the held project, the fade's invokes, the swap's 14 calls and state
  against the original's clear_resources and on_render_scenes_ready halves on the same objects,
  the eye adaptation's byte, the camera's pose; `0xA1`'s squad and srp byte; a missing pack; a
  match entered and the close behind the black; late results), selftest check 81 (the ten sites,
  the stock code the swap redoes, the squad reader, the eye adaptation's byte, the lobby movie's
  holder), `check_engine_026g.py` (the virtual rows), `check_weapon_table_026g.py` (the spot from
  the project's config, the turned bench), `scripts/lobby-fade/check.sh` (Ruffle: the movie's
  fade under a panel, to black, from black and gone, gone at once), lobby-server's
  `a_squad_info_carries_its_commanders_quest_faction` (the byte, re-sent on the commander's
  change, not on a member's).
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
