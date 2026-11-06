# Lobby packs: the 0.46c2 hangar and the 0.69d camp

The two lobby projects resource-porter makes from the 0.46c2 and 0.69d lobbies, packs
lobby-hangar-046 and lobby-camp-069: their sources, the manifests' choices and each version. What
a lobby project must give, the lobby window and the upgrade view are the lobby selection's
(`lobby-scenes.md`, #778).

## Both packs

- Each is a client project of its own beside the classic `lobby_scene/day`, which stays as it
  is: `client_project` with no `.lua` and no server project; its vegetation files in
  `projects/lobby_scene/vegetation/<scene>/`.
- Made by resource-porter (`vostok-port build-survarium-later-scene`, branch
  `feat/lobby-scenes-046-069`; hangar @9 and camp @17 its `feat/lobby-reuse-native-textures`;
  camp @18 its `fix/room-lobby-ceiling`; its `docs/survarium-069d-to-026.md`, "Lobby scenes"),
  shipped as one content pack per scene, so either can ship or change alone.
- Sounds: the classic lobby's `sounds/background` and `sounds/lobby_room_tone` (the donors' sound
  environments have no 0.26 counterpart).
- To see one locally: a test patch with the scene in place of the classic one (`vostok-port pack
  patch ... --replace projects/lobby_scene/day/client_project=<the scene's client_project>`), or
  the lobby window of #778. Never committed.
- Textures from 0.26g (hangar @9, camp @16 and @17): a donor texture whose 0.26g namesake is the same
  image is not imported, and the materials name 0.26g's own file (resource-porter
  `reuse_native_textures`, its `native_same_image`, as Rudnya's: the same size, each 4x4 block's
  mean colour within 2 levels on average, no channel's mean moved by 1.5, 99 % of blocks within
  12 levels). Nothing is downscaled. No listed patch, pack or edit list replaces one of these
  stock files, so players have what was compared.

  | pack | reused | byte for byte | decode alike | re-encodes | MiB not carried | pack |
  | --- | ---: | ---: | ---: | ---: | ---: | --- |
  | lobby-hangar-046@9 | 72 of 131 | 13 | 59 | 0 | 123.8 | 214 -> 90 MiB |
  | lobby-camp-069@16 | 54 of 220 | 1 | 3 | 50 | 59.3 | 428 -> 368 MiB |

  The camp's re-encodes differ by 0.6 level per block on average (at most 1.5; 99 % of blocks
  within 8.2); side by side with 0.26g's, the worst (the vertex-blend masks, the gas cylinder,
  the level 02 tree atlases) show no visible change. Left imported: of the textures with a
  0.26g namesake, the hangar's 43 and the camp's 21 are other images (retouched gloss and
  specular maps, recoloured metal and moss, an edited corner) or another size.

## lobby_scene/hangar_046 (pack lobby-hangar-046@9)

- Source: 0.46c2, `projects/lobby_scene/day` of its 36 volumes. The owner asked for 0.40; 0.46c2 is
  the nearest build on disk with this lobby.
- Scene: an overgrown hangar (`level/lobby/hangar_lobby`, `hangar_wall`), flora of level/01 and
  level/03, props. 24 models imported (`ported/v046c2_lobby`), 10 are 0.26's own.
- Lights as 0.46c2 has them (0.26's units): 49 ambient lights, 11 lights, the donor's 4 probes
  with their own cubemaps, 3 wire sets. Environment: the donor's, every field 0.26g's executable
  reads (107 names, the same in both builds: sun elevation 55, azimuth 150, intensity 1.5; sky,
  clouds, mie and rayleigh fog, height-based ambient, probe multipliers, bloom, aberration,
  vignette, grain), with its colour grading table `color_grading/lobby_v2` (a 16x16x16 RGBA8
  volume laid out as 0.26's `l_03d`) imported as `ported/v046c2_lobby/color_grading/lobby_v2`.
  Only `blueshift` keeps the classic's: a test without it was alike, so it is left out as a field
  whose scale is not known to match.
- @3 (the owner: "the lighting in camp and hangar was different"): @2 drew the hangar teal-blue
  where the original is warm brown. Two causes: the classic lobby's environment, and the ambient
  light colours packed the wrong way round (0.26's `color` has red in the low byte: the donor's
  45 warm fills (1, 0.78, 0.67) drew blue). References: YouTube thumbnails of the 0.40/0.41
  lobby (warm brown hall, cool daylight from the windows and skylights, deep shadow under the
  trusses).
- @4: the manifest's `environment_overrides` raise the sun to 7 (the donor's 1.5) and the
  probes' diffuse multiplier to 2 (1.3), set by eye against the 0.40/0.41 thumbnails (0.26's eye
  adaptation evens the level, so the stronger sun mainly gives the bright patches through the
  roof grid and the windows). The sun shafts themselves were particles 0.26 lacks.
- @5 (the owner: @4 "too dark without direct light"): sun 3, probe diffuse 3.5. Mean luminance of
  the view 0.051 (@3 0.042, @4 0.063); the back wall, vines and floor out of the sun at or above
  @3 (compare script: `private/lobby-select/light/compare.py`).
- @7, the same light every run: its four box probes all had radius 9.0, the race of the camp's @8
  (below, @9); now 0.75 x their largest half-extent, and `environment_probe(2)`, which ties
  `environment_probe` (both 8.038), just below it. They meet only in a thin seam.
- @8 (the owner, 2026-10-31: "in hangar in the front we can have better quality tree brushes"):
  through the front gate's gaps (the -z wall, x -8.5..-2.8) the outside was the donor's
  background card `level/01/background/back_bush` (two triangles with a 1024 px tree atlas, scaled
  3x) and two of 0.26's `flora/bush_s_line_3` stretched 3 x 2 x 2: flat, blurry and aliased up
  close. Both left out (`exclude_models`); in their place 0.26's own bushes and trees
  (`native_scenery`, drawn only): `flora/bush_s_003`/`_005` just outside the gate, two fruit trees,
  three birches and a poplar further out on the ground's rise (z -8.8..-14.8), their crowns over
  the wall's top, none through it.
- @9: the 72 textures 0.46c2 kept from 0.26 drawn from 0.26g's own files ("Both packs").
- Character: (0, 0, 0), rotation 180.
- Donor camera (`main_scene_alone`): look at (0, 1.3, 0), distance 1.5, pitch 14, yaw 0 (range 25),
  `look_at_yaw` -11.5, vertical fov 45; the character turned to 145.
- The stock camera frames him well at any distance (captures: stock and nearest). No wall within
  the orbit: the nearest is the right wall at 4.8 m (render meshes, rays from (0, 1.3, 0)), so no
  camera walls.
- Weapon spot (`srp_lobby`), since @6: `weapon = (2.289, 1.24, 3.925)`, 0.40 m above 0.26's own
  `props/furniture/workbench_02` (`native_props`, top 0.84 m) against the sandbags behind the
  character's right (the hangar has no table of its own); `weapon_view = -2.3562`, looking at the
  bench's front. Up to @5 the donor's workshop view, `(-9.90, 1.30, 6.95)`, `0.6545`, floating 1.3 m
  over the floor (the donor's: distance 1.8, yaw 37.5, pitch 0, fov 30). Captured in a game
  2026-10-31 with the upgrade window open (#778's lobby-scenes.md).
- Artefact spot: on the burlap-covered crate stack right behind the character, (3.84, 1.42, 3.91)
  (its top at y 1.414).
- Squad: donor (-0.705, 0, 0.562) rotation 200 and (0.814, 0, 0.595) rotation 162 (0.26: (1, 0, 0.5)
  and (-1, 0, 0.5)); shop player (-1.0, 0.04, 7.5) rotation 145.
- Not ported: particles birds, crows, light rays, underground smoke (0.26 has none of them; lamp
  sparks and leaflets use 0.26's own), the portal system.

## lobby_scene/camp_069 (pack lobby-camp-069@18)

- Source: 0.69d, `projects/lobby_scene/day` of volume 72 (the last copy), without its New Year
  snow: the 8 instances of `level/new_year_event/` and `generic/modular/terrain/snow/` (volume
  32's scene). Not the event versions of volumes 26 and 70.
- Scene: a room of a house (`building/private_house`) with the work table under the raincoat tent
  (`level/lobby/raincoat_tent`), the fridge (`props/kitchen/refrigerator_lobby`), the background
  (`level/02/bg_shop`, `level/04/bg_pipe_05`, houses, trees). 101 models imported
  (`ported/v069d_lobby`), 9 are 0.26's own.
- Lights (@3), by the manifest's `photometric_lights`, set by eye against the references (0.69d
  draws through a fixed exposure, EV 9.5, and a filmic curve that 0.26 lacks):
  - the 18 adding ambient lights: 0.004 per lux (illuminance x multiplier x intensity), at most 2;
  - the 65 multiplying ones (0.69d `type` 1, they multiply the light under them): the 4 fills
    (multiplier above 1: `floor_light_1/2`, `light/ambient_light` x50 at the camera, x3) as
    adding ones, 0.005 per unit above 1 (the x50 one is a sphere of 11.6 m about the camera, bluish:
    stronger, it turned the window side blue); the 61 painted contact shadows (multiplier below 1: under
    the boxes, the carpet, the fridge, the nightstand, the lamp) as negative ambient lights,
    -0.25 per unit below 1 (0.26 subtracts them: shown in a game, the corners and the floor
    under the props darken);
  - 13 lights: 0.015 per lumen (the 200-lumen key on the character 3, the lamp bulbs 0.15).
- Probes (@3): the donor's 6 volumes (boxes) with their own cubemaps, re-baked for 0.26 by the
  porter (`probe_bake.rs`): 0.69d stores RGBM (colour over its largest channel, radiance rgb x
  a^2) and its irradiance as 9 SH coefficients; 0.26 stores texels at or below 1 as they are and
  brighter ones over their largest channel (alpha its reciprocal), with a 16x16 `_diffuse` cubemap
  of irradiance over pi (0.26's and 0.46c2's own check out so, to 3 %). The `_diffuse` is made
  from the SH (real basis with the Condon-Shortley phase over the world axes: on 4 of the 6
  probes the L1 band points where the cubemap is brightest), the cubemap scaled to the SH's mean;
  diffuse multiplier = the probe's 0.69d multiplier x 0.9. @2 drew every volume with the classic
  lobby's warm cubemap x2.6 (the flat warm orange).
- @4 keeps only the room probe and the room-fill one (`exclude_probes`): the three small window-side
  and camera-side boxes and the 127 m outdoor box carry blue sky irradiance, and 0.26 blends them
  over the room probe (0.69d lets the room's win), so in @3 the window side, the floor and the
  left wall went blue.
- Environment: the classic lobby's sky, fog and post-process, the donor's sun (elevation 50,
  azimuth 25: through the left window onto the papers and the right wall), at intensity 5 and
  warm (1, 0.86, 0.68) by `environment_overrides` (@4: the key light of the references), no colour grading
  (0.69d white-balances at 6500 K and has no table; the classic's `l_03d` made it warmer).
- @5 (the owner: @4 "too dark without direct light"): the probe boxes' `probe_side_width` is the
  donor's 0.1 m. The template's integer 2 had been kept, and 2 m of fade at each side of a 4 m
  room left its walls outside the room probe, which is why stronger probes changed nothing.
  The room probe is x0.45 with its baked irradiance at saturation 0.4 and directionality 0.5 (the
  SH bands above the constant one halved: the sides facing away from the window lit as bounce),
  the fills neutral-warm (1, 0.93, 0.84) at 0.02, the sun 3; only the 127 m outdoor box is left
  out (with it the room probe loses). Luminance against @3: left wall 0.031 (0.030), doorway 0.028
  (0.017), far wall 0.051 (0.028), ceiling 0.137 (0.157), the sun patch on the floor 0.21 (0.12).
- @6, the chandelier (the owner: dark where the 0.53 preview's glows). The donor's lamp
  (`props/electric/lustra_patina`, 1.3x at (-0.42, 2.79, 2.10)) has one opaque dielectric material
  for its frame and its shades, no emission; the two bulbs inside are 0.26's own
  `props/light/lamp_on` (emissive, hidden by the shades). 0.69d lit the glass from the bulbs; 0.26
  does not, so the shades drew dark (opaque, as the donor's: no alpha test or blend on either
  side). Its lights were all there but weak or wrong: two point lights (10 lm: 0.15 at 2 m), two
  adding ambient lights (20 lux: 0.12) and three multiplying ones at 0.01 over the ceiling (its
  painted shadow), which 0.26 subtracts. Now (`glowing_surfaces`, `light_overrides`): the shades
  (`lustra_patina_plafon_s`) take a copy of the material that emits its albedo, warm, x2.2; the
  bulbs 0.5 at 2 m; the ambient lights 0.4; the painted ceiling shadow left out. The ceiling around
  the lamp: luminance 0.27 (@5 0.14), the rest as @5.
- @6, emission: 0.69d's `use_emission` with a `t_emission` texture (in its g_stage or forward
  stage) is 0.26's g_stage emissive now. In this lobby only the laptop screen
  (`props/electric/notebook_modern_on`) had one; it had drawn dark. The hangar's donor materials
  have no emission (it stays @5).
- References: YouTube thumbnails of 0.65b (a bright room, daylight from the left window, cream
  walls) and of the 0.53 preview (darker, the sun patch on the floor before the desk); @3 sits
  between them; @4 is warm and sunlit like both, still darker on the left wall and ceiling than
  0.65b (0.26's eye adaptation evens the level; no 0.69d exposure or filmic curve).
- Window panes (`water/glass_water`) use 0.26's `tiled/window/glass`.
- @8, the view out of the windows (the owner: "flat building and plain blue sky", then "they are
  untextured"). The stock camera orbits all the way round, so the windows show what 0.69d's fixed
  camera hid: the room is a floor 22.7 m over the donor's own background, level 02's apartment
  blocks (`bg_house_1/2`, `bg_shop`: 15 to 220 m) on a terrain patch, billboard trees. Their meshes
  and textures are the donor's, whole; they drew as flat grey-blue slabs:
  - the faces out of the sun had no light (the outdoor box left out since @4) and the classic's
    interior fog (rayleigh from 1 m to 185 m) made them flat. The outdoor box is back, drawn before
    the room's probes (so the room's still win in the room; inside, against @7, within 1/255 in a
    crisp capture), in a box over all the blocks (`probe_overrides`), diffuse 2.0 at saturation
    0.15; the fog level 02 day's (rayleigh 15..300 m, mie 40..1200 m);
  - the far blocks (one at 88 m through the back window stayed a slab) are lit in 0.69d by its
    outdoor ambient light boxes, which the room's photometric scale had left at 0.15: 1.5
    (`light_overrides`). 0.26 does not apply the outdoor probe that far out.
  - The yard (the owner's pick of three captures: faithful, a courtyard, overgrown): an overgrown
    yard under the windows of 0.26's own level 02 trees and bushes (poplars, birches, willows,
    fruit and dead trees, the trees scaled 1.1-2.1 so their crowns reach the room's floor), 0.26's
    level 04 tree belts past the blocks, level 02's background terrain and tree rings (as the
    school map has them) for the horizon: 110 render visuals of 0.26 models, no collision
    (`native_scenery`). The pack grows by 0.5 MB (the outdoor probe's cubemap, the project).
  - Captures: `lobby-audit/rotation/camp_069-8-*` (the full orbit at 1.96 and 4.6, pitch +-30 at the
    window yaws; before is @7).
- @9, the same light every run. @8's light flipped between two states (same build: the yaw 0 left
  wall 50 or 70 of 255, the dull one a bluish lift of the shadows), about half the runs each.
  0.26g draws the visible probes largest `radius` first, each over the ones before
  (stage_visibility's sort, `tools/re-kit/v026g` 0x1B9390), and probes of equal radius in its probe
  tree's order, which changes from run to run. All six had the template's 15.3, so the outdoor box
  (outdoor sky irradiance) drew over the room's probes in about half the runs. A box probe's
  radius is only that sort key in 0.26g (its volume is its scale; 0.10b's engine set it to 0.75 x
  the largest half-extent itself, 0.26g takes the project's): the porter now gives each box that
  radius, so the 220 m outdoor box draws first and the room's, smaller, over it. The same race
  is what turned the window side blue in @3. Not the atmosphere: its two recalculations at load
  (scene_changed, then parameters_changed) take the scene view's final parameters, and their
  timing did not follow the state.
- @10 (the owner, 2026-10-31: "I think in room the door got lost"): the right wall's doorway
  (door_01's frame, x 1.65, z 2.1..3.4) was a black hole. Nothing was dropped: both donor doors
  are in the project (`props/door/wood_door_01` at (1.76, 0, 2.1), its leaf swung open behind the
  wall, and the double door `props/door/wood_door_02` beyond the doorway). The donor placed the
  double door at rotation (-pi, -1.453, -pi), facing out of the room; 0.69d's fixed camera never
  showed it, and 0.26's orbit sees its one-sided leaves from behind, unlit. The manifest's
  `instance_rotations` turns it to (0, 1.689, 0): its panelled leaf fills the doorway, lit
  (captured in a game, before and after). Drawing the leaves two-sided did not help (their backs
  are unlit).
- @12 (the owner, 2026-10-31: "the lighting from the sink is strange"): a dark wedge ran from the
  washbasin (`props/kitchen/washstend`, (-1.13, 0.75, 5.32), on the back wall) up the wall and on
  across the ceiling. It is the donor's painted contact shadows at the basin,
  `environment_and_lights/shadows/ambient_light(1)`, `(5)`, `(6)`, `(7)`: negative ambient-light
  boxes, (1) and (5) tilted 25 degrees about z, (1) 2.9 m tall. 0.69d fades their edges (its own
  shots show a clean wall there); 0.26 draws them as hard-edged boxes. The manifest's light
  overrides leave the four out. Found by game captures, one cause ruled out at a time: the fill
  light under the basin (@11 turned its shadow off: no change, undone in @12), every other
  shadowing light, the sun.
- @13 (the owner, 2026-10-31: shadows "not diffused at all, always so crisp"; the classic lobby's
  are right): the floor-level fills (`character_lights/point_light(3)`, `(10)`) cast no shadow, as
  the classic's floor fills cast none; the window light `point_light(5)` keeps its shadow on a
  256-texel map (`shadow_map_size_index` 2, as the classic's spot lights; it was 512). What softens
  a shadow in 0.26g is in the next section.

- @15 (the owner, 2026-10-31: "what is this shadow plop in a room?", a dark square on the
  ceiling by the back wall, behind the chandelier): the chandelier's two positive ambient boxes
  (`room/lustra/ambient_light`, `(1)`, 0.4) end at z 3.4..3.7, and 0.26 draws an ambient box with a
  hard edge: bright inside, unlit beyond it to the back wall (z 5.5). They, the two negative
  chandelier boxes in the same zone (`shadows/ambient_light(8)`, `(9)`) and the faint ceiling
  boxes there (`light/celling/ambient_light`, `(2)`..`(6)`) are left out; the chandelier's two point
  lights (no shadow, falloff power 1) reach the back wall instead, 0.8 and 4.2 m (0.5 and 2 m): a
  fill with no edge. No box edge is left on the ceiling between the chandelier and the back wall
  (checked against the project). @14 had turned the window light's shadow off as the cause: wrong
  (the square grew); undone, its shadow on a 256-texel map as in @13. Not yet seen in a game.

- @16: the 54 textures 0.69d kept from 0.26 drawn from 0.26g's own files ("Both packs").
- @17 (the owner, 2026-10-31, on @15: "now it is very dark where the character looks"; @12's
  lighting "was good, the only problem was that single patch"): the light overrides are @12's
  again: @13's floor-fill shadows and 256-texel window map, @14's window change and @15's ceiling
  changes are all undone (what @13 says of shadow softness stands). Two changes only:
  - the two negative ambient boxes over the chandelier, `shadows/ambient_light(8)` and `(9)`
    (2 x 1.1 m footprints, y 2.37..4.37, cut by the ceiling; 0.26 draws them hard-edged), left
    out. The chandelier's own ambient lights are spheres with a soft edge (geometry 0, radius
    1.025 m, side width 0.5), not boxes, and stay. A dark patch on the ceiling remains; the owner
    keeps it ("it is fine, we make release like this");
  - the doors' hinge pins left out (the owner: "the door incorrectly shows hinges", "we can just
    remove hinges"): resource-porter's new `removed_parts` drops a model surface's triangles near
    given points; the 15 cm pins of `props/door/wood_door_02` (4) and `wood_door_01` (2), LOD0 and
    LOD1 (128, 64, 64 and 32 triangles: the pins alone; LOD2 has none). The door keeps @10's turn.
- @18 (the owner, 2026-11-01: "we have this invisible collision making light strange in room right
  now", the invisible ambient-light boxes of the ceiling hunt): @17's ceiling patch. 0.26 draws an
  ambient box with a hard edge, and after @17 two sets of boxes still cut the ceiling (y 2.81).
  Both are left out; every other light is @17's:
  - `light/ambient_light(6)`, -0.225: x -2.64..2.09, y 1.44..3.01, z -2.41..-1.62, a dark strip
    along the whole back-window wall with its straight edge on the ceiling 0.45 m from the wall.
    Its pair under it, `light/ambient_light(5)` (-0.2475, y -0.19..1.37), goes too, or its top
    would be a hard line across the window wall;
  - `light/celling/ambient_light(1)`..`(6)`: faint fills (0.003..0.012, against the room fills'
    0.98) laid on the ceiling as 0.48 m slabs, with edges and gaps across it. The ceiling's faint
    sphere `light/celling/ambient_light` and the chandelier's spheres stay.
  - The back-window camera wall moves 0.15 m into the room (inner face z -1.40). Before, the eye
    stopped on the wall's face, about 0.07 m from (5) and (6). The eye straight toward the windows
    stays at z -1.50 (the camera's 1.5 m clamp).

  The camera walls are collision only (`collision_objects`, cgroup 8 / cmask 16), as the STK2
  `camera_boxes`. They are Bullet shapes for the camera's ray, and 0.10b's renderer never reads
  physics, so they draw, shadow and light nothing. The built project differs from @17's by the
  eight lights and that wall alone. Not yet seen in a game.

- Character: (0, 0, 0), rotation 180.
- Donor camera (`main_scene_alone`): look at (0, 1.3, 0), distance 1.5, pitch -2, yaw 0 (range 25),
  `look_at_yaw` -11.5, vertical fov 45; the character turned to 145.
- **Camera walls.** The room (the house's collision) is x -2.44..1.56, z -2.07..5.53, y 0..2.81,
  but its collision is open at the back windows (z -2.07: the default view), the left windows and
  the right doorway. Without walls the stock eye left the room (pack 1: the wall not drawn from
  outside, the window frame over half the view). The pack's invisible walls: in front of the
  back windows' frames and curtains (inner face z -1.55; -1.40 since @18), the left windows (x -2.10),
  the doorway (x 1.50), the front wall (z 5.53); 0.1 m thick, the room's full height. In a game
  (#778's build) the stock camera stays in the room, the eye about 1.5 m from him.
- Weapon spot (`srp_lobby`): `weapon = (-1.60, 1.37, 3.255)` (since @7; 1.27 before), the donor's
  workshop view (its `weapon_workshop` lights), 0.40 m over the work table (top y 0.97, the
  raincoat on it) left behind the character; the free part
  of the table is z 2.7..3.6 (a mug at z 2.6, a notebook from z 3.6). `weapon_view = 1.5708`,
  looking along -x at the table: the eye 1.2 m from it is in the room at x -0.4; from the other
  side it would be outside the left wall (x -2.44). The donor's: distance 1.6, yaw 90, pitch
  -20, fov 30.
- Artefact spot: on the same table's front half, (-1.25, 0.98, 2.95).
- Squad: donor (-0.705, 0, 0.562) rotation 200 and (0.814, 0, 0.595) rotation 172; shop player
  (-1.69, 0.05, 1.602) rotation 190.
- Not ported: particles crows, light rays, dust, the stove fire (0.26 has none; the fly uses
  0.26's), the shadow proxies (`shadow_proxy_geom`: 0.26 has none), the fridge's sound, 0.69d
  metalness and detail maps, exposure, white balance and tone curve.

## What softens a shadow in 0.26g

Read 2026-10-31 in the stock point-light shader (`shaders/sm_4_0/point_light_accumulator.ps`,
its sixteen compiled variants disassembled), the light's config reader (0x34F99C..0x34FB38) and
the 0.10b engine source (`stage_lights.cpp`, `point_light_accumulator.ps`, `spot_shadow.h`):

- **No per-light softness, size or blur field.** A shadowed light samples its shadow map with a
  fixed kernel about one texel wide: 4 taps at `r_shadow_quality` 0 and 1, a 16-tap Poisson disc
  at 2 and 3 (the shader variant's first flag; user.cfg's `r_shadow_quality`, 1 in the test
  prefixes). The edge's width is the map's texel: `shadow_map_size_index` 0 = 1024, 1 = 512,
  2 = 256 texels (0.10b's `index_to_shadow_size`), so a larger index draws a wider, softer edge.
- **`shadow_transparency`** is added to the shadow term: `saturate(shadow + shadow_transparency)`,
  the light left in the shadow (0: black). The exe reads it as a float or an int (0x34FB06); the
  stock lights all carry the int 0.
- **The classic lobby** (`day`): its five shadowing lights are two spots and three points; the
  spots and one point use 256-texel maps, the floor-level fills cast no shadow, and its shadowing
  points fall off steeply (`attenuation_power` 2.9..7.7), so a shadow fades away from its light.
- **Global:** `r_shadow_quality` 2 or 3 (the game's shadow quality option) gives every light the
  16-tap filter: smoother edges, not wider; 16 shadow-map reads a pixel instead of 4 for each
  shadowed light. Not changed.
- **The hangar** against the classic: one shadowing light, `character_lights/point_light(3)`, a
  floor-level fill (0.8, 10 m, 512 texels): a floor fill casting a shadow, which the classic's do
  not. Left as it is (the owner is not sure of the hangar; its shadow is faint).
