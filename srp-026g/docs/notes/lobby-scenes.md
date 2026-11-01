# Lobby scenes

The owner (2026-10-30): "a lobby selection screen" in place of a lobby by faction. A button in
the lobby opens a window of six cards (four until the owner's "let's add 2 more lobbies into the
game from stk2. One is house and another one is shooting range", 2026-11-01), a picture and a name
each (the owner, 2026-10-31: drawn as the rooms' map-vote cards); a click on one shows that lobby, behind a fade. In a squad every member is shown its commander's
pick ("when someone joins your squad, they go into your lobby, so it switches for them"). Exe
patch 81 (`client-patches/rust/patches/src/lobby_scene.rs`), the lobby movie's window
(`flash/lobby_menu/src/survarium/lobbypick/LobbyWindow.as`), srp's pick
(`crates/survarium/src/v026g/lobby/lobby_pick.rs`), patch 45's weapon spot
(`weapon_table_026g.asm`).

## The lobbies and the pick

- One enum, `Lobby` in `lobby_scene.rs`; its id is user.cfg's value, the card, srp's start and
  `0xA1`'s byte:

  | id | card (dictionary key)              | project                                  |
  |----|------------------------------------|------------------------------------------|
  | 1  | the Room (`st_srp_lobby_room`)     | `lobby_scene/camp_069`, the 0.69d camp; the default |
  | 2  | the Basement (`st_srp_lobby_basement`) | `lobby_scene/day`, the classic (stock) |
  | 3  | the Bunker (`st_srp_lobby_bunker`) | `lobby_scene/stk2_bunker`, S.T.A.L.K.E.R. 2 |
  | 4  | the Hangar (`st_srp_lobby_hangar`) | `lobby_scene/hangar_046`, the 0.46c2 hangar |
  | 5  | the House (`st_srp_lobby_house`)   | `lobby_scene/stk2_house`, the upper floor of S.T.A.L.K.E.R. 2's Bar |
  | 6  | the Shooting Range (`st_srp_lobby_range`) | `lobby_scene/stk2_bar`, S.T.A.L.K.E.R. 2's Bar shooting gallery |

- **The button** is the player panel's, top left, in the premium button's place (the owner,
  2026-10-31: "remove premium button and put lobby button there instead"): the original's
  premium button is taken out of the panel (`topPanelLeftContainer_56.placeLobbyButton`) and the
  lobby button (`survarium.lobbypick.LobbyButton`) takes its place and size. It has the money
  boxes' frame and nothing else of a money box: a button made in code (no timeline, so no state
  frame places a coin, an amount or a hint again; the owner's test of `SrvButtonTop` showed the
  coin on a press and a "silver 0" hint), its back the money boxes' own piece for each state
  (`btnBackUp`, `btnBackHover`, `btnBackDown`, `btnBackDisabled`) stretched to the box, no tooltip
  and no data. Its label is the top menu's look (Calibri bold 16, 0x9BADC1, upper case), centred
  in the box by its measured size (the amount's own field embeds digits only: "badly written and
  not centered"). The premium itself is as before (its state, the x2 mark, the offer after a
  match: `StatusPanel.premiumDiscount`, the seconds the button kept); only its button is gone.
  The window's title and the button are `st_srp_lobby_title` ("Lobby"); the texts are
  dictionary keys (`client-patches/localization`), every language's.
- **The pick is kept in user.cfg** as the console variable `srp_lobby` (`srp_lobby 3`), written by
  the exe (user.cfg saved at once when it changed), read once into `Lobby` (`Lobby::parse`): no
  line, an empty or unknown value is the Room. The six lobbies ship with the game (the owner,
  2026-10-30: "lobby files can't be missing"): there is no fallback for a missing one.
- **The window** (`LobbyWindow`, the lobby movie's ActionScript): the six cards in id order, three
  to a row (3 x 2; the count is the movie's `NAMES`), each a rooms map-vote card (`RoomsRoom`): 280 x 190, the lobby's render over the whole
  card (`RoomsLook.cover`, as `mapRender`), its name in white on a band of black at 0.62 along the
  bottom 58 px; the pick framed gold (3 px), the others 1 px. The pictures are the empty scenes
  (the owner, 2026-10-31: no character), from the stock camera, made by the diagnostics sweep's
  `empty` (`camera_sweep.rs`; never in a release). A click sends
  srp the pick (`request_players_range(2 095 000 000 + id, 1)`, srp's `3B` at a reserved start,
  `lobby.md` 2.16) and closes the window; Esc and the button close it too. What it shows the exe
  tells it (`root.srp_lobby_state(pick, shown)`), at its first frame and at each change.
  The pictures are the lobbies' stock views without anyone in them (camp @12, bunker @8,
  hangar @8, the classic, house @1, bar @4): 560 x 380, twice the card, JPEG at q90 as the
  game's map renders are (`flash/lobby_menu/bitmaps/SrpLobby*.jpg`, 393 KB the six), cut from
  2560 x 1440 captures of the diagnostics sweep's `empty`.
- **srp keeps each online account's pick** (in memory, `Player::lobby_pick`; none before one) and
  sends it back in `0xA1`: two bytes after the squad (after the id alone for none), the receiver's
  own pick and its commander's (0 none; no squad, none). A pick changed is `squads_dirty`: every
  client whose `0xA1` changed gets it again (the picker's own; its squad's when it commands one).
  The stock reader (read_squad_info 0x274720) stops before the bytes; a client that does not read
  them signs in and stays (seen 2026-10-30: a current exe against a server sending five bytes
  more on `0xA1`).
- **The exe reads them** at `lobby_client.squad_id` (0x2747C3, both paths; for no squad the bytes
  follow the id) and `lobby_client.squad_read` (0x2748BC, a squad's revision kept), checked
  against the message's size, into `Picks`: its own pick kept (saved); srp without one (the
  sign-in, or an older srp) is sent the saved one (the lobby client's ranking request 0x270070,
  as the movie's request_players_range sends it); the commander's pick is `Squad::Led(lobby)`, none
  `Squad::None`. The lobby wanted is `Squad::lobby(own)`: the commander's in a squad, one's own out
  of one. Joining: the joiner's lobby becomes the commander's; leaving, kicked, the squad disbanded:
  back to one's own; a new commander, or the commander's pick: everyone left follows; a member's
  own pick: saved, nothing seen while in the squad. `srp_lobby` stays the player's own pick.
- **Client only, beside srp's memory of the picks:** a match is the same whatever lobby (no
  simulation change: `3B` and `0xA1` are lobby messages).

## When the pick is known

- **The start shows the saved pick.** The lobby menu asks for its scene at the game's start,
  before the sign-in (the network client is made in that query's own callback, on_render_scenes_
  ready 0x220B20 at 0x221C2F), so the saved pick is the start's lobby; srp learns it at the
  sign-in's `0xA1`. A squad's commander's pick shows at once after (below).

## At start

- The lobby menu is made once, at the game's start (`game::create_lobby_menu`), and asks for its
  scene with its movies in one query (0x21E450); the exe asks for the saved pick's lobby there
  instead of `lobby_scene/day`.
- **A project that fails to load is a bug** (the six ship with the game; one without its camera
  track is logged in `srp_patches.log`, and the lobby menu takes its results as they are, as the
  stock game would). A missing project crashes the project's cook (a null config read at
  0x22C11E, `project_cooker_simple::create_game_objects`; seen in a game, 2026-10-29): the
  release's pack check is what keeps that from players.
- Nothing more than the stock game's query: the pick's project in the classic's place.

## A change shown at once

The owner (2026-10-30): the new lobby appears right when it is picked (or the squad changes), and
it looks smooth (no hitch, no pop-in, no camera jump). Built (PROVEN in the code; run with Unicorn
against the original's code on the same objects; captured in a game, below):

- **One state machine** (`Scene` in `lobby_scene.rs`, one transition function `Scene::step`): not
  asked; starting (the start's query, as above); loaded, with the lobby shown, whether it is drawn
  (drawn, hidden, closed) and where a change is (`Change`: settled; broken, a lobby whose project
  failed, not asked again until another is wanted; darkening; loading behind the black; holding,
  frames drawn behind the black; brightening). The events: the query
  built (`lobby_menu.scene_request`), its results (`lobby_menu.scenes_ready`), a frame of the
  lobby drawn (`lobby_camera.view`, the lobby camera's tick, after patch 45's flight), the lobby
  hidden (`lobby_menu::on_deactivate` 0x21D4A0, a match), shown (`on_activate` 0x21D2E0), the
  game closing (`game::clear_resources` 0x1ED030). The wanted lobby is `Squad::lobby` of the
  saved pick and the squad at each event (the lobby shown while a weapon's upgrade window has
  the camera, below): a pick changed, a squad changed, a load done or failed,
  a fade done, a match entered and the close all come as those.
- **A frame with another lobby wanted than the one shown: the black, then the load.** The scene
  fades to black (`FADE_OUT`, 0.25 s), the exe
  stepping it a frame each (`srp_scene_fade(per mille, loading)`, by at most a 30th of a second a
  frame: after a stall the fade still plays over its frames; the movie's own tween would have
  ended during the stall, seen 2026-10-30), a black under every panel, over the 3D scene (the
  root's first child; the interface stays; get_mouse_over leaves it out, so the camera still
  turns), the game's loading spinner on it. Whole, the lobby's query is asked again
  (query_resources 0x21E450 with the menu) with that lobby's project alone (the other seven
  requests its config, a binary config, no user data: no second render scene, sound scene or
  movie is made): its load and its cook's hitches happen behind the black.
- **Its project in** (in the results' callback, `ScenesReady::swap_in` / `put_in`): the project
  halves of `lobby_menu::clear_resources` (0x21D080..0x21D100) and `on_render_scenes_ready`
  (0x220DA2..0x220EC3, 0x22193E..0x221975), in their order, each step the game's own function:
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
    project); each static collision's insert (vtable+0, 0x2F8850).
  The two emitter calls are the engine-function library's virtual rows (client_patch.md 0.5); nine
  emitter classes (vtables.tsv). The project's `camera` (0x242770) is not set again: every lobby's
  character is at the origin (below).
- **Behind the black:** with the swap the eye adaptation is reset (the engine's own: the byte
  0xAC914C, 1 in the image for the first frame; while set, stage_postprocess's
  compute_per_pixel_eye_adaptated_luminance 0x1B1400 takes the frame's own luminance as the last
  one's and clears it: the exposure is at once where it settles, no swing; seen in the captures:
  the first frame after the black is within 2 % of the settled luminance) and, at the next frame,
  the camera at its ctor's pose (yaw, pitch, side 0, distance 1.96 m; not while patch 45's camera
  is at the bench or flying). `HOLD_FRAMES` (6) frames are drawn behind the black, the
  adaptation is reset again, the spinner goes and the scene fades in (`FADE_IN`, 0.4 s).
- **A project that fails behind the black** (no camera track: a bug): logged in
  `srp_patches.log`, nothing put in, the black stepped out over the lobby shown, and that lobby is
  not asked again until another one is wanted.
- **The last lobby wanted wins:** a change during the fade to black asks, once black, the lobby
  wanted then (back to the one shown: the black stepped out, nothing asked); while a project loads
  behind the black, the new one is asked behind the same black.
- **A weapon's upgrade window open** (patch 45's camera the table's: its table on, or its flight
  there or back): the lobby wanted is the one shown (`Scene::wanted`). Patch 45 reads the
  weapon's spot from the lobby shown's project when its table turns on, so a project put in under
  it would leave the weapon and the camera at the old lobby's spot in the new scene (a squad: the
  commander picks, or someone joins, while a mate has the window open). A pick made meanwhile
  plays once the window is closed and the camera home: a fade begun steps back out, a project that
  comes behind the black is let go, and the change starts again then, the new lobby's first frame
  at the camera's ctor's pose. (Not the other way, the spot read again after the swap: that would
  fly the camera from the old lobby's spot to the new one's in sight, and its height, distance and
  side angle are the camera setter's, which only patch 32's start of the window calls.)
- **A match entered during a change:** the black is gone at once; a project that comes while the
  lobby is hidden is put in at once; back in the lobby while it loads, the black and the spinner
  again. The lobby on return is the wanted one.
- **The game closing:** from then on nothing is asked and results are let go without the lobby
  menu being touched.
- **Where the time goes** (2026-10-30, headless on this machine, a diagnostics build's
  `srp_lobby_scene.txt`: each action with the lobby's frame count and the clock's milliseconds;
  three changes, the Basement to the Bunker, the Room to the Hangar, a squad joined and left):
  (with the config probe of then, 6..7 ms, gone since) the black took 8 frames; the project's load
  behind it 10..12 frames, 3.9..6.9 s, the frames going on at the lobby's rate meanwhile (its cook
  is the resource system's, off the game's own frame but for its results' callback); the swap
  (`put_in`, the game's thread, in the results' callback) 0..3 ms. The game drew the lobby at
  about 2.3 frames a second all along here (before a change as during it: a headless display
  without the GPU's own driver), so the "two 3 s stalls" of the first capture were that frame
  rate, a frame being 0.35..0.6 s, and the movie's 250 ms tween ending within one frame: no step
  of a change stalls the game's thread for long. Not measured on a real display.
- **Costs and not known:** the eight requests of a change's query name one config (the resource
  system's handling of one name eight times in a query is read from the stock flow only, not
  seen). What the lobby's track effect plays (an ambient effect; GUESS). F11's hidden interface
  hides the black too (the root): a change while it is hidden is a cut, not a fade.

## What a lobby project must give (for the content packs)

A lobby is an ordinary `projects/lobby_scene/<id>/client_project` (and `.lua`), made as the
classic's. This file is the one spec of every lobby; each pack's own notes (its source, the
manifest's choices, each version) are the pack's: `lobby-packs-046-069.md` (the hangar and the
camp, #776) and `lobby-packs-stk2.md` (the bunker, #802). What the exe and our patches take from
it:

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
- **No table:** a table already in the scene first; else the weapon floats in the air in front
  of the character, beside him, seen along +z as the stock camera looks (the owner,
  2026-10-31: "we can use other tables, or just do that in the air"). The hangar got 0.26's
  own `props/furniture/workbench_02` (top 0.84 m) against its sandbags before that rule (the
  porters' `native_props`).
- **Each lobby's upgrade view** (captured in a game 2026-10-31, headless, the four lobbies, the
  flight there and back):

  | lobby | the weapon | `weapon` | `weapon_view` |
  |---|---|---|---|
  | the Room (`camp_069`, @10) | above the camp's own office table (the 0.69d weapon workshop's, its lights) | (-1.60, 1.37, 3.255) | 1.5708 |
  | the Basement (`day`) | above the workbench | (4.25, 1.30, 3.05) | -1.5708 |
  | the Bunker (`stk2_bunker`, @5) | in the air in front of the character, beside him (the shelter has no table) | (-0.95, 1.35, -0.80) | 0 |
  | the Hangar (`hangar_046`, @8) | above the workbench_02 against the sandbags (the hangar has no table of its own) | (2.289, 1.24, 3.925) | -2.3562 |
- **The artefact table and the bench bulb** of the classic (content edit `lobby-scene-2`, pack
  `lobby-bulb@1`) are the classic's decoration: a lobby that wants a lit table puts its own light
  (`point_light`) and props in its own project. No exe code places them.

## The camera flight to the weapon (patch 45, round 8)

The weapon's upgrade window flies the camera from where it is to the spot's view, and the close
flies it back: the eye on a straight line between the two eyes, raised by one arc over the whole
way where that line passes within 0.9 m of a character (the camp's spot is behind the character),
the yaw, the pitch, the distance, the side angle and the zoom with it, one smoothstep over 0.45 s;
back, to where the stock camera will be (the wheel's target cut by the camera's own ray there),
held a tick with no ray, then the stock camera from exactly there. The Rust patch
`weapon_flight` (one enum `Flight`, one `Flight::step`), at `lobby_camera.view`; patch 45's NASM
keeps the weapon at the bench and its requests, and casts no ray while the camera flies
(client_patch.md section 3.42). Until round 6 the flight glided back to the wheel's target with
no ray, out through a small room's wall, and the first stock tick snapped the eye back by up to
3.1 m (the camp, the bunker; 2.6 m in the classic, through the floor).

- **Round 8 (2026-10-31): no jump over the character.** The owner: "in room camera does a jump
  when going to a table, most likely because it hits a character". Round 7 raised the eye only
  near a character, by a bump of up to 0.8 m that rose and fell within about a tenth of a second
  at the flight's fastest (in the Room the line from the stock eye to the spot passes 0.2 m from
  the character; the classic's and the hangar's pass a squad panel's spot): the eye's vertical
  speed changed by 0.3 m a frame. Now the arc spans the whole flight, `height * 4 s (1 - s)`, its
  height the least that still takes the eye over every character the line passes (at most to
  2.5 m, under the rooms' ceilings): 0.03 m a frame at most. Not the ray (none is cast while it
  flies) nor the 1.5 m clamp (the flight sets the distance itself). Tests: every lobby's flight
  there and back, from twelve starts round the orbit, against the smoothstep's own acceleration.
- Where the line itself meets a prop: from the stock start no lobby's flight crosses one (the four
  spots, static ray casts against the drawn meshes); from a few orbit poses it grazes a prop the
  stock camera already stands behind (the bunker's crates, the camp's refrigerator).

## The audit (2026-10-30)

The owner: "other team members locations, camera panning and zooming, and camera movement for
weapon upgrade table". Static: every lobby's visual meshes (LOD0, each surface's two-sidedness)
and the static collision the camera's ray meets, ray-cast with embree; the orbit sampled at 72
yaws, 25 pitches and 7 distances; the squad's spots; the weapon's spot and the flight from 72
starts. The share of (yaw, pitch) whose eye is behind a visible surface at the wheel's 4.6 m (the
stock floor's 8 % in every lobby included), before and after the packs (the blockpost and the
bar were audited before the owner's pick of 2026-10-30, which left them out; the bar came back as
the Shooting Range on 2026-11-01, the blockpost ships nowhere: the House is the Bar's upper floor,
`stk2_house`, lobby-packs-stk2.md):

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

- Rust tests of the picks, the squad and the transitions (`lobby_scene.rs`: the start, a change
  faded, loaded behind the black, swapped and faded in, a stall's frame not skipping the fade, a
  broken project logged, faded back and not asked again, the last lobby wanted winning, a match
  entered at every frame of a change, the close; every sequence of six events of eight kinds,
  under two loads: a swap only behind the black or while not drawn, one query at a time, no
  black left once settled), `check_lobby_scene_026g.py` (Unicorn; client_patch.md 3.59), selftest check 81 (the
  nine sites, the stock code the swap redoes, the squad reader, the eye adaptation's byte, the
  lobby movie's holder, the 3B sender), `check_engine_026g.py` (the virtual rows),
  `check_weapon_table_026g.py` (the spot from the project's config, the turned bench),
  `scripts/lobby-fade/check.sh` (Ruffle: the black under a panel, the spinner, the window and its
  cards; `window.png`), lobby-server's `a_squad_info_carries_the_lobby_picks` (a pick sent back
  alone out of a squad, each member's own and the commander's in one, the commander's change to
  every member, a member's to its own byte only), `lobby_pick.rs` (the starts and ids).
- In a game (2026-10-30, local, headless, no input but the sign-in; a diagnostics build of this
  branch with this branch's srp; the live layers, this branch's patch 40 and the packs camp @8,
  bunker @4 and hangar @6): the lobby window opened (the sweep's `window`), its cards and the
  pick marked; a pick (`pick 3`, `pick 4`) and a squad (`led 3`, then `alone`) each shown at once:
  the black stepped in, the spinner, the new lobby, the black stepped out, the camera at its
  stock pose, the exposure within 2 % of its settled value once the black was gone; srp got the
  saved pick at the sign-in. (Before the pick, 2026-10-29: the faction's saved lobby from the
  first frame.)
- An early finding of this PR, that user.cfg is never read under Wine, was wrong: it was this
  work's own headless prefix (load_cc_script got no resource for `user_data/user.cfg` there, the
  cause not found); in a local-clients `run.py` prefix the file is read and the saved faction
  applies. Debug builds of this work crashed on a user.cfg line of a `cc_string` (`r_resolution
  1280x720`, the one run.py writes): a debug-only hook at cc_string::execute read its argument
  off the wrong stack slot (never in the branch). check_lobby_scene_026g.py runs whole user.cfgs
  (run.py's LF file, the game's CRLF one, no final end of line) through the console's own load.
