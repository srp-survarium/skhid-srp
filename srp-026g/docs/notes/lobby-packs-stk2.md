# Lobby packs: the STK2 lobbies

The lobby projects resource-porter makes from S.T.A.L.K.E.R. 2's (2011-12) levels
(build-stk2-lobby-026): the bomb shelter (pack lobby-stk2-bunker,
ports/stk2-bunker-lobby-to-026g.json), the House, the Bar's upper floor (lobby-stk2-house,
ports/stk2-house-lobby-to-026g.json), and the Bar's shooting gallery below it (lobby-stk2-bar,
ports/stk2-bar-lobby-to-026g.json): their sources, the manifests' choices and each version.
The blockpost guard hut of STK2's own lobby (lobby-stk2, ports/stk2-lobby-to-026g.json) is
made and not shipped.
What a lobby project must give, the lobby window and the upgrade view are the lobby
selection's (lobby-scenes.md, #778).

## STK2 lighting (the bomb shelter; the House, the hut and the Bar as their own sections say)

- Kept from STK2: the project's own lights (point, sphere and box lights as points; spots
  and plane spots as spots with their cones and directions; the sun with its colour,
  intensity, shadow and influences), and its environment's post-process on the 0.26g
  environment object: fog colour and distances, sun colour, bloom, adaptation speed, no
  colour grading, no grain, aberration, vignette, god rays or lens flares (STK2's
  post-process has none). Every STK2 light at twice its intensity.
- What 0.26g lacks: STK2 lit its rooms with a flat ambient colour, light propagation
  volumes (its lamps' `use_with_lpv`) and a tonemap median exposure. With the lights alone
  the rooms were dark or flat and the character a silhouette from behind. The bomb
  shelter (@3, the owner's pick of a warm and a cold candidate, 2026-10-30) is balanced
  by eye in the game:
  - one box environment probe over the room, its cubemaps made from a warm-neutral
    floor-to-sky gradient (resource-porter `probe_cubes`). Only box probes (geometry 1,
    scale the half extents) light anything in 0.26g; a sphere one did nothing, even the
    classic lobby's own at four times its strength;
  - STK2's three wide fills (range 15, which lit the vaults flat and grey) shortened to
    6.5 m and warmed short of orange (1, 0.86, 0.7): a stronger warmth turns the plaster
    saturated orange;
  - a warm light under each street lamp; the one behind the character casts shadows and
    lights his back, so he reads from every side of the orbit;
  - the lamps glow: STK2 placed its unlit `lamp_street`, given its emissive variant (its
    mask lights only the shade's inside, which the lobby camera never sees, so the shade
    emits its texture at 0.6 of STK2's multiplier).
- 0.26g's ambient lights (resource-porter `objects` from `ambient_light(4)`, colour
  packed red in the low byte) light the room but not the character: only real lights and
  probes reach him.
- No counterpart in 0.26g (each pack.json lists them): STK2's ambient and skylight
  colours, tonemap median and thresholds, levels (frame lights and shadows), shadow
  transparency, bloom's maximum colour, depth of field. 0.26g's day sky stays behind the
  openings (STK2's skies are not ported).
- The geometry: one STK2 project's objects inside a box, placed with the engine's own
  transform (scale, rotation, translation), clipped to the box and merged by material
  into one model with their low collision (cmask 20, cgroup 10: the camera's ray stops
  at the walls). Materials on native 0.26g templates by STK2 base material and flags;
  textures at their sources' size, to 2048 px (the bunker's @5, the hut's @3, the Bar's @4;
  before, at most 512 px, props 256). Weapons on tables and STK2's static characters are
  left out.

## STK2 House, the Bar's upper floor (`lobby_scene/stk2_house`, pack lobby-stk2-house@1)

The owner (2026-11-01): "there is another house though", the one of the video "Map Showcase [5]
(House)" (youtube `_4iyMIQLaC8`, about 7:00): "there is a small kitchen, model in exoskeleton",
"tent on top". It is the upper floor of STK2's Bar building
(`architecture/research_institute/bar_shooting_gallery`, the Shooting Range's building), dressed
as `bar_shooting_gallery` dresses it. Manifest `ports/stk2-house-lobby-to-026g.json`.

- The match, frame by frame (the video's 6:50..7:50 against static renders of this floor from the
  same eye): the hall under a hanging camouflage net, high strip windows down both long walls,
  concrete pillars; long wooden trestle tables with benches, bottles and cups; through the opening
  at the hall's far end the kitchen corner, blue-green tiled, its gas cooker under the hood, a sink,
  the bar cabinet with bottles on top; a bench along the right wall; pallets and cement bags by the
  pillar at the stairs. The video's eye stands in the annex by the stairs, looking west. The tables
  are where `bar_shooting_gallery` (and its copy `dima_test`) put them; `stohe_light_test` puts
  them elsewhere and without benches.
- "Tent on top": the camouflage net (`props/bar/bar_camouflage`, its `shooting_gallery_camouflage`
  texture) hung under the ceiling over the whole hall, 2.6..3.9 m up. Kept; it fills the top of
  every view. Outside, on the roof landing beyond the annex's north door, an awning over the door
  (the video's 6:54) is not in the lobby's view (the door is closed, below).
- The figure in the exoskeleton: `dima_test` and `stohe_light_test` stand a `test_object/
  test_character` at (89.08, 0.221, -3.29) (STK2), by the left table, where the video shows it.
  Its model is in neither STK2 archive, so the video's build drew something not shipped.
  `bar_shooting_gallery` stands the STK2 Novice (`actors/.../neutral_01`, skinned) at
  (89.86, 0.22, -0.27) instead; static characters are left out. Not ported (the owner's call):
  STK2's static Duty exoskeleton (`character/human/dolg/dolg_exo_static`, an exported static
  model, 1.2 MB, its textures PSDs of up to 185 MB) could stand at the video's spot as a prop
  (a third slice of a project that places it, moved there).
- The candles: the video's table flames are in no project of the archives (no candle model or
  particle on these tables). STK2 put small warm point lights just above the twin tables and the
  right table (their light is kept, soft, no fixture).
- Spot: the open floor between the tables and the annex (STK2 (94.3, 0.221, -0.5)), facing
  east, so the default view looks west down the hall: the tables either side, the watering-can
  lamp over them, the kitchen corner at the far end under the net.
- Kept: the box x 77..101.5, y 0.05..9, z -6.5..6 (STK2 world), the upper floor, its roof and
  the annex by the stairs: 253 of 544 objects, merged in `level/stk2_house/main_v1`. 16 props
  have no model in the archive (barrels, buckets, cement bags, broken bottles). A second slice
  (`door_v1`) puts a copy of the annex's closed brown door (`props/door/door_exterior_brown_close`)
  in its open north doorway, which looked out onto the roof landing, its railing and nothing
  below.
- Unmapped faces: STK2 left some faces without a mapping (the kitchen partition's end, 19
  triangles, a strip of plaster stretched across it; door edges; a stair riser): mapped by
  projection at their surface's texel density (resource-porter's `Mesh::stk2`, as the bomb
  shelter's @9).
- Spots (data `srp-026g/scripts/lobby-scenes/stk2_house.tsv`):

      character          ( 0.000, 0.000,  0.000)
      camera, default    ( 0.000, 1.200, -1.960)  yaw 0
      weapon             ( 1.750, 1.194,  2.300)  weapon_view 0 (above the table behind the right)
      artefact           ( 0.950, 0.774,  2.250)  on the same table

- The upgrade view: the trestle table behind the character's right (top 0.77 m), the weapon
  0.42 m above it, seen along +z, the bottles and cups below it and the kitchen corner beyond.
- Light (as the bunker's @8):
  - one box probe over the hall (half extents 5.45, 1.85, 7.9), the bunker's warm-neutral
    gradient, ambient 0.45; warm ambient fills along the hall, in the kitchen corner and the
    annex; the character's fill and a warm bounce on his back;
  - STK2's lamps, each with its fixture: the watering-can lamp over the tables (a spot, warm,
    2.4, range 7) and the two halogen work lamps at the kitchen end (blue-white, now warm white
    (1, 0.9, 0.78));
  - STK2's small warm lights just above the twin tables and the right-hand table (no fixture:
    the video's candles' place), soft (2.2 and 1.8, range 3.2), and the bar cabinet's;
  - no light casts a shadow (STK2's cast them); STK2's sun is left out.
- The camera: boxes over the floor, under the net (2.35..2.65 m: its lowest within the wheel's
  reach), round the watering-can lamp, round the five tables with their benches near the
  character and the tub on its stool.
- The audit: the squad's three spots are clear, the floor at 0 under them; no view ray of the
  orbit meets nothing below the horizon (the default view's 1.4 % is the sky in the high
  strip windows). The eye is behind a surface in 8.0 % of the orbit, the stock floor rows; 23
  cells more at the wheel's far end (yaw 20..25 and 320, 4.6 m). The upgrade flight: as the
  Bar's and the hut's.
- In the game (2026-11-01, the full sweep): warm, no hard shadows; the net fills the top of the
  view, the kitchen corner and the bar cabinet glow at the far end, the tables either side. The
  strip windows show the 0.26g day sky, light blue; the hall's west end and the annex are the
  darker parts.
- The hut it replaces stays in resource-porter (`ports/stk2-lobby-to-026g.json`, @4), not shipped.

## STK2 blockpost hut (`lobby_scene/stk2_blockpost`, pack lobby-stk2@4, not shipped)

Made as the House and replaced by the Bar's upper floor the same day (the owner, 2026-11-01:
"there is another house though"). The owner (2026-11-01): "let's add 2 more lobbies into the game from stk2. One is house and
another one is shooting range". The house was taken for STK2's own lobby (`lobby_scene/project.xprj`,
2011-12): the a02 blockpost guard hut alone on terrain, with a campfire on its floor, a sentry
cabinet, a mattress and a stool. Manifest `ports/stk2-lobby-to-026g.json`. @1 and @2 (#768) were
made and never shipped.

- Spot: the hut's floor between the door and the campfire (STK2 (2.090, 1.107, 27.106)), facing
  the door. The default view looks across the room to the cabinet and the window wall.
- Kept: the whole STK2 project, five objects, merged in one model
  (`level/stk2_blockpost/main_v<version>`, `main_v4`, new with each version as the bunker's). Up to @2 these were
  five models of their own under `level/stk2_lobby`. STK2's terrain is not ported: a flat grass
  plane 1.107 m below the floor. The hut's walls are the camera's collision. 43 MB.
- Spots (@4, in its lobby frame; its spot data left with the hut):

      character          ( 0.000, 0.000,  0.000)
      camera, default    ( 0.000, 1.200, -1.960)  yaw 0
      weapon             ( 0.900, 1.600,  2.550)  weapon_view 0 (above the sentry cabinet's counter)
      artefact           (-0.468, 0.500,  2.310)  on the stool

- The squad (@2): the campfire stood where the squad's left member is, so it moved to
  (-1.15, 0, 1.65). Up to @2 the campfire composite's own light (it casts shadows) stayed at the
  old spot. @3 moves it with the logs (a slice's `moves` take lights too).
- The upgrade view: the weapon 0.41 m above the sentry cabinet's counter (top 1.19 m), seen
  along +z, with the cabinet's back panel behind it.
- Textures (@3): at their STK2 sources' full size, up to 2048 px. Up to @2 they were capped at
  1024: the walls' plaster, the concrete and its normal map, the concrete decal and the windows.
- Light (@3 and @4, as the bunker's @8):
  - one box probe over the room (half extents 2.7, 1.35, 2.7), with the bunker's warm-neutral
    gradient and ambient 0.45;
  - STK2's hut light (a white sphere light, 5.5 x 2, range 5, no fixture) is warm and softer:
    (1, 0.78, 0.55), intensity 2.4, range 7, falloff 1.5. @3 hung it under a pendant
    (`props/light/green_lamp_with_wire` with its glowing `props/light/lamp_on`). In the game the
    pendant's enamel shade, lit only from inside, showed as a black disc near the eye from behind
    and above: the hut is low, so the shade hung into the orbit. @4 puts the light in the glass of
    a caged ceiling plafond instead (`props/light/plafon_lamp` and its glowing
    `props/light/plafon_lamp_on`, drawn only, 0.52 m deep), on the crossing of two tie beams
    (-2.0, 2.58, 0.6). That is 0.8 m from STK2's spot and 1.0 m clear of the squad's left member
    (a first try right at STK2's spot hung 0.18 m over that member's head);
  - a soft warm fill on the window side (@4: (1.8, 2.1, -0.4), (1, 0.92, 0.82), 0.9, range 4).
    In the game the character was a black silhouette with the camera there, because the lamp,
    the campfire and the character fill all stand on the door and campfire side;
  - the campfire's light is warm (1, 0.82, 0.6);
  - no light casts a shadow, and the sun stays at 0.
- The view out (@3): through the open doorway the camera saw the grass plane end at 40 m and
  nothing past it. Now 0.26's own spruces (`flora/elka_green_01..05`) and bushes (`flora/bush_s_*`)
  stand round the hut at 6..25 m, drawn only, and the grass reaches 200 m. The sky shows above
  them and through the gaps at the eaves, as for any hut. The audit: of the orbit's view rays,
  1 % meet nothing, all of them the sky. Below the horizon it is 0.01 %, within 0.8 degrees of
  it, past 200 m.
- The camera (@2): a cage 0.15 m inside the hut (walls, floor and ceiling, the doorway closed)
  and boxes on the cabinet, the stool and the campfire.
- In the game (2026-11-01): the full squad, the orbit at 1.96 and 4.6 m, from above and below,
  the view out of the doorway and the upgrade view. The light is warm and as bright as the
  bunker's. No hard shadows. The squad is clear of the campfire. Through the doorway the spruces
  are dark against a dusk sky, beyond the porch railing. The window side, though no longer a
  silhouette, stays the hut's darker side.

## STK2 Bar shooting gallery (`lobby_scene/stk2_bar`, pack lobby-stk2-bar@5)

The shooting range: the lower level of STK2's `bar_shooting_gallery` (2011-12), with the
gallery's lanes, targets, camouflage nets, the bottle shelf at the lanes' end, its lamps, and the
hall before it. Manifest `ports/stk2-bar-lobby-to-026g.json`. The owner dropped it on 2026-10-30
(one lobby per faction) and brought it back on 2026-11-01. @1..@3 never shipped.

- Spot (@5): at the firing line. The owner (2026-11-01): "with shooting range, I wonder whether we
  can change their location, since now it looks like they are set there to be shot at". Up to @4
  the squad stood 4 m before the targets at the lanes' end (STK2 (97.0, -3.208, 0.6)), the
  camera looking at them with the targets behind. The lanes run 17 m from the firing line (a
  sandbag wall under a wooden counter, STK2 x 83.6..84.75) to the targets (x 100..101); behind
  the line is the shooters' plank floor (x 79.75..83.6, 0.1 m up), with a lounge (a chair and a
  television table) and the stairs up to the House. Three spots on it, each captured in the game
  from the stock camera with the squad:
  - R1, straight downrange (82.0, -3.109, 0.2), view along +x: the squad's backs to the
    sandbags and counter, the lanes and a target far behind them;
  - R2, from the lounge north-east (81.6, -3.109, -1.6), view yaw -0.9: the counter behind the
    squad, the lanes hidden behind its right member;
  - R3, by the stairs (82.0, -3.109, -1.0), view yaw -2.24 (south-east): the sandbag wall and
    counter run away on the left with the lanes and the targets off to that side, the squad
    beside the line, the stairs on the right. Picked: the targets are off to one side, not
    behind the squad.
  The character stands on the plank floor (y -3.109).
- Kept: the box x 77..102.5, y -3.6..0.3, z -7..6 (STK2 world): 286 of 544 objects, merged in
  `level/stk2_bar/main_v5` (`main_v4` up to @4). 16 props have no model in the archive (barrels, buckets, cement bags,
  broken bottles) and are left out, and so are the planks under the squad's right member (@2).
  The canister on the vent stack by the workbench sits on the floor (@3: the upgrade flight).
  141 MB.
- Spots (data `srp-026g/scripts/lobby-scenes/stk2_bar.tsv`):

      character          ( 0.000, 0.000,  0.000)
      camera, default    ( 0.000, 1.200, -1.960)  yaw 0
      weapon             (-3.284, 1.319,  0.284)  weapon_view 0.668 (above the firing counter)
      artefact           (-3.990, 0.909, -0.275)  on the firing counter

- The upgrade view (@5): the firing counter is the table, the weapon 0.41 m above its top
  (0.91 m), seen downrange with the lanes and targets beyond. From @2 to @4 the gallery had no
  table near the character, and 0.26's `props/furniture/workbench_02` stood in the back left
  corner for it; @5 leaves it out.
- Textures (@4): at their STK2 sources' full size, up to 2048 px. Up to @3 they were capped at
  512, and the props' at 256.
- Light (@4, as the bunker's @8):
  - no light casts a shadow. Up to @3 the gallery's spot, the hall's three ceiling lamps, the
    watering-can lamp and the halogen work lamps' spots all cast one;
  - no blue cast. The halogen work lamps were blue-white (0.62..0.71, 0.63..0.75, 0.91) and are
    warm white (1, 0.9, 0.78). STK2's sun (0.46, 0.53, 0.78) cast no shadow, so it lit the
    whole level blue; it is left out, as the bunker's is;
  - the hall's three ceiling lamps 12 m behind the character (range 19.6, which lit the hall
    flat) are warm (1, 0.86, 0.7) with range 14;
  - the room probe's ambient is 0.45 (0.2). The warm ambient fills along the hall and the
    gallery and the warm bounce on the character's back stay from @3.
  - The lamps are STK2's own composites, each with its fixture: the halogen work lamps, the
    ceiling lamps and the watering-can lamp. Nothing is added.
  - @5: the fills stay where they were in the gallery (moved with the frame); a dim warm fill at
    the foot of the stairs behind the squad, which showed black.
- The camera (@4): the hall's ceiling collision (3.07 m) has no margin, so at the wheel's far
  end with the eye above the point (pitch -25 degrees, 49 of 72 yaws) the eye stopped on it and
  saw into the level above. A box now sits 0.15..0.29 m under the ceiling over the hall, and
  another over the carton boxes and shelf by the right wall, which the eye got into at yaw
  50..75. The @2 and @3 boxes stay: the floor, the workbench, the bottle shelf.
- The camera (@5), re-made for the spot (the boxes are in the lobby frame, turned with it): the
  floor, under the ceiling (2.97 m over the planks), the sandbag wall and counter (to 0.2 m over
  its top: at 0.91 the eye passed just over it at pitch 0 and looked along the boards), the
  ammunition boxes by the stairs, the back wall and the south wall with the stairs' doorway (their
  collision has no margin), the television table and the chair.
- The audit (@4): no view ray of the orbit meets nothing (the gallery is closed). The eye is
  behind a surface in 8.2 % of the orbit, the stock floor rows. Views with over 10 % back faces
  are 8.1 %: the floor rows and two cells over a pallet on the floor at pitch +45..50.
- The audit (@5): the squad's three spots are clear, the floor at 0 under them; no view ray
  meets nothing; the eye is behind a surface in 8.1..8.5 % of the orbit, the stock floor rows and
  48 cells (0.4 %) at yaw 140..160 looking up, by the counter.
- In the game (2026-11-01, the same poses): warm, no blue, no hard shadows. The squad is lit from
  every side. The hall behind the character is dimmer, with a bright pool under its far lamp.
  From above (pitch -25) the eye stays under the ceiling. The upgrade view frames the workbench
  against the targets and the bottle shelf.
- In the game (@5, 2026-11-01, the full sweep, with the counter's box at its @5 candidate's 1.17 m):
  warm, no hard shadows; the squad by the firing line, the lit counter and sandbags on the left,
  the targets small at the lanes' end; the stairs behind the squad are the dark side. The upgrade
  view looks down the lanes over the counter.

## STK2 bomb shelter (`lobby_scene/stk2_bunker`, pack lobby-stk2-bunker@8)

The shelter of STK2's dressed level `scenario/a03_bomb_shelter`, not the bare
`bomb_shelter_test` project (its shell only, no props): the octagonal store room with
the gas-mask pile (`bomb_shelter_gasmask_garbage`), shelves, ammunition and gas-mask
crates, beds and lockers, and the vaults beside it.

- Spot: where STK2's designers stood a novice (STK2 (37.49, -4.37, -24.41); @2 0.25 m clear of the crates, -24.66), facing the
  room's west half. Chosen of four: the gas-mask room, the sleeping quarters, the store
  room, the crates by its door; the store room is the most dressed and the best lit.
- Kept: the whole shelter, x 24..63, y -5..0.5, z -36..6 (STK2 world): 337 of 348
  visuals, 10 lights. Up to @3 the box was x 28..48, z -31..-8, and its cut showed
  through the store room's doorways as flat blue-grey: the sky, past the vault, the
  corridors and the rooms the box left out (@4: no view ray of the orbit meets the sky). a03's sun (shadowed, it
  lights the level's outside) is left out: the sun object is there with intensity 0.
  28 MB.
- Spots (data `srp-026g/scripts/lobby-scenes/stk2_bunker.tsv`):

      character          ( 0.000, 0.000,  0.000)
      camera, default    ( 0.000, 1.200, -1.960)  yaw 0
      weapon             (-0.950, 1.350, -0.800)  weapon_view 0 (in the air, beside the character)
      artefact           ( 1.710, 0.610, -1.100)  on a gas-mask crate

- The upgrade view (@5): the shelter has no table (its crates, beds and shelves only), so
  the weapon of an upgrade window floats in the air in front of the character, beside
  him, and the camera looks at it along +z as the stock camera does: the character
  stands beside the weapon in the view, and the flight there is short and stays in the
  stock camera's own space. Up to @4 it was 0.40 m above the metal boxes by the south
  wall, where the game showed it hanging before a bed frame (the owner, 2026-10-31: the
  new lobbies had no craft table).
- Textures (@5, the owner, 2026-10-31: "in bunker some textures are low-quality and fuzzy", the
  switch box on the wall, the leaning plank): up to @4 the manifest scaled every STK2 texture to
  at most 512 px and the props' (`props/`) to 256: 133 of the 142 with a source were smaller
  than it (76 of 1024 px, 19 of 2048, 38 of 512 at 256). Their mips, DXT formats, sRGB and normal
  maps were right. @5 takes every texture at its source's size (to 2048 px); the pack is 159 MB
  (29 MB). The camp's and the hangar's packs keep their donors' textures up to 2048 px (the camp's
  ten 4096 px ones at 2048): not this.
- The cart (@5, the owner: "remove this untextured white thing with 4 wheels"): STK2's
  `props/garage/cart`, by the leaning plank, left out (the slice's `exclude`), and the camera box
  around it. Nothing stood on it.
- Lamps (@6, the owner, 2026-10-31: "in bunker the lightbulbs are missing completely", then "not
  just bulbs, plafonds too" and "the lighting too is a bit too sharp"): STK2's street lamps
  (`props/light/lamp_street_01..06`) are one surface each, a shade or a cage on its rod, with no
  bulb mesh: nothing was dropped in the port. They are left out, and 0.26's own fixtures (the
  same models 0.69d ships; Rudnya places no lamp) stand at their eight spots, drawn only (no
  collision for the camera's ray):
  - where STK2 hung an open shade: a pendant, `props/light/green_lamp_with_wire` (its wire and
    enamel shade, 0.64 m) with `props/light/lamp_on` (a socket and a glowing bulb, its emissive
    warm, (0.94, 0.72, 0.56) x 8.4) in its socket;
  - where it hung a cage: a caged ceiling plafond, `props/light/plafon_lamp` with its glowing
    glass `props/light/plafon_lamp_on`, on the ceiling.
- Light (@6): the store room's three lights at their fixtures (3 cm under the pendant's bulb, in
  the plafonds' glass), softer: intensity 2.4 (3.2), range 8.5 m (6), falloff power 1.5 (2);
  the room probe's ambient 0.3 (0.2). The warm colour kept.
- Shadows (@7, the owner: "in bunker shadows are a bit too strong"): the light under the store
  room's pendant bulb cast shadows, so the pendant's shade threw a hard pitch-black ellipse over
  the vault above it; the lights at the pendants (the store room's and STK2's under the corridor
  pendant) cast none now, and the light reaches the vault as a hanging lamp's does. The room
  probe's ambient 0.45 (0.3) lifts the dark parts; the warm colour kept.
- The merged model is `level/stk2_bunker/main_v<version>` (`main_v8`), a new folder with each
  version: its surfaces are numbered by material, and a resources.db patch only adds files, so a
  surface folder of an earlier version left in the same model failed all its materials (seen with
  @6's files over @5's: "material not loaded: nomaterial", the shelter untextured).
- No shadows (@8, the owner: a hard pitch-black band down the inner face of the big arch, "a
  shadow of something"; "decrease intensity of light a bit"): traced from each shadowing light
  through the band, the caster is the wooden shelf's post by the gas-mask crates, 1.7 m from STK2's
  store-room fill `point_light(5)`, which stands right beside the shelf. STK2 lit with it as a soft
  bounce; 0.26 drew the post's shadow hard and black. No light of the bunker casts a shadow now;
  the main lights are 18 % lower (the store room's lamps 1.97), the ambient and the bulbs' glow kept.
- The camera: boxes over the floor and around the props the eye got into (@2), and one
  across the front arch to the next vault (@3: at yaw 0, 4.6 m the eye went through it
  into the dark).
- Beyond the arches: the other rooms' lamps as warm as the store room's, and a dim warm
  light in the vault beyond the west arch and in the corridor beyond the east one (they
  showed black).
- Not ported: two decal materials without a diffuse (`decal_dirt_006`,
  `moss_012_alpha`): their surfaces are left out; the outdoor trees (speedtree) and the
  power line outside.
- Textures from 0.26g (checked 2026-10-31, not done): 13 of the 145 textures have a 0.26g
  namesake and 4 are the same image (`native_same_image`, as Rudnya's and the 0.46c2 and 0.69d
  lobbies'): `props/box/wood_big_nmap`, `props/covers/plywood_nmap`, `props/furniture/bed1` and
  `chair_nmap`, 3.4 MiB of 146; the rest are STK2's own images. Not worth a version or a
  reuse switch in build-stk2-lobby-026.

## Captures

The House @1, the Bar @5 and its three candidates (2026-11-01), as the hut and the Bar @4 the same day: the diagnostics sweep of this branch's exe, each project in the
Bunker's slot (`srp_lobby 3`, a local test patch over the live layers), no input but the sign-in.
The lobby window's pictures are the `empty` scene at 2560 x 1440, cut between the interface's
bars (y 180..1390) to the card's framing.

Headless, the debug exe of feat/lobby-select choosing `stk2_blockpost` with each
project in its place (a local test patch), no input but the sign-in. On the machine
that made them: `/mnt/hdd/Projects/lobby-stk2/survey/captures/` (`cand-*` the
candidates, `pick-*` the packs).
