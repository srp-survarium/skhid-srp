# Lobby scenes

The project the lobby menu loads behind the character. 0.26g loads `projects/lobby_scene/day`
(`client_patch.md` section 3.42); choosing another one is separate work (exe + Flash). The scenes
below are content only: each is a project of its own next to the classic one, which stays as it is.

## Common to every lobby scene

- The character stands at the origin facing -z (yaw pi): the exe puts him there
  (lobby menu+694h, 0x21CCD2). Both donors' `gameplay/lobby_menu.options` agree:
  `local_player` at (0, 0, 0), `rotation_y` 180. So each scene keeps its donor's coordinates.
- The stock camera always looks at (0, height, 0) from distance 1.5..4.6 (the wheel); its eye is
  `point - distance * forward`, forward = (-sin yaw * cos pitch, sin pitch, cos yaw * cos pitch).
- The project is a client project only (`client_project`, no `.lua`, no server project). Its
  vegetation files are in `projects/lobby_scene/vegetation/<scene>/`: all lobbies share the level
  folder `projects/lobby_scene/`.
- What the lobby choice (client patch 81) takes from it, in each pack's project:
  - the classic lobby's camera `track` (copied: a project without one is refused) and the root
    `camera`;
  - `srp_lobby = { weapon = float3(...), weapon_view = <radians> }`, patch 45's weapon spot;
    `weapon_view = atan2(-dx, dz)` of the donor's own view direction there (the 0.26 camera's
    convention, forward = (-sin yaw, 0, cos yaw));
  - collision on every wall the camera's orbit (1.5..4.6 m) can cross: its distance is cut by a
    ray against the project's static collision. Where a wall's own collision is open (a window,
    a doorway) the pack adds an invisible box (0.26's unit box `editor/walker_collision`, scaled;
    the classic's static groups, cgroup 8, cmask 16).
- Sounds: the classic lobby's `sounds/background` and `sounds/lobby_room_tone` (the donors' sound
  environments have no 0.26 counterpart).
- Made by resource-porter (`vostok-port build-survarium-later-scene`, branch
  `feat/lobby-scenes-046-069`; its `docs/survarium-069d-to-026.md`, "Lobby scenes"), shipped as
  one content pack per scene, so either can ship or change alone.
- To see one before the choice exists: locally only, a test patch with the scene in place of the
  classic one (`vostok-port pack patch ... --replace
  projects/lobby_scene/day/client_project=<the scene's client_project>`). Never committed.
- Display spots: the stock weapon display (client patch 45) is BENCH (4.25, 1.30, 3.05) of the
  classic scene. Each scene's own spots are below; heights are of the surface under them, read
  from the meshes.

## lobby_scene/hangar_046 (pack lobby-hangar-046@5)

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
- Character: (0, 0, 0), rotation 180.
- Donor camera (`main_scene_alone`): look at (0, 1.3, 0), distance 1.5, pitch 14, yaw 0 (range 25),
  `look_at_yaw` -11.5, vertical fov 45; the character turned to 145.
- The stock camera frames him well at any distance (captures: stock and nearest). No wall within
  the orbit: the nearest is the right wall at 4.8 m (render meshes, rays from (0, 1.3, 0)), so no
  camera walls.
- Weapon spot (`srp_lobby`): `weapon = (-9.90, 1.30, 6.95)`, the donor's workshop view, floating
  over the floor (y 0) left behind the character; `weapon_view = 0.6545`, looking along
  (-0.609, 0, 0.793) towards the hangar wall 5 m behind it (the donor's: distance 1.8, yaw 37.5,
  pitch 0, fov 30, 0.69d's yaw convention being 0.26's, shown by the camp).
- Artefact spot: on the burlap-covered crate stack right behind the character, (3.84, 1.42, 3.91)
  (its top at y 1.414).
- Squad: donor (-0.705, 0, 0.562) rotation 200 and (0.814, 0, 0.595) rotation 162 (0.26: (1, 0, 0.5)
  and (-1, 0, 0.5)); shop player (-1.0, 0.04, 7.5) rotation 145.
- Not ported: particles birds, crows, light rays, underground smoke (0.26 has none of them; lamp
  sparks and leaflets use 0.26's own), the portal system.

## lobby_scene/camp_069 (pack lobby-camp-069@8)

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
    the room's probes (`probe_overrides` `order: first`: 0.26 draws a point's probes in the
    project's order, each over the ones before, so the room's still win in the room; inside, against
    @7, within 1/255 in a crisp capture), in a box over all the blocks, diffuse 2.0 at saturation
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
- The lobby's light flips between two states from run to run (same build: the left wall 50 or
  70 of 255, the dull one a bluish lift of the shadows). Not cloud shadows (`use_sky_shadows`
  false keeps it); not fixed.
- Character: (0, 0, 0), rotation 180.
- Donor camera (`main_scene_alone`): look at (0, 1.3, 0), distance 1.5, pitch -2, yaw 0 (range 25),
  `look_at_yaw` -11.5, vertical fov 45; the character turned to 145.
- **Camera walls.** The room (the house's collision) is x -2.44..1.56, z -2.07..5.53, y 0..2.81,
  but its collision is open at the back windows (z -2.07: the default view), the left windows and
  the right doorway. Without walls the stock eye left the room (pack 1: the wall not drawn from
  outside, the window frame over half the view). The pack's invisible walls: in front of the
  back windows' frames and curtains (inner face z -1.55), the left windows (x -2.10), the doorway
  (x 1.50), the front wall (z 5.53); 0.1 m thick, the room's full height. In a game (#778's
  build) the stock camera stays in the room, the eye about 1.5 m from him.
- Weapon spot (`srp_lobby`): `weapon = (-1.60, 1.27, 3.255)`, the donor's workshop view, 0.30 m
  over the work table (top y 0.97, the raincoat on it) left behind the character; the free part
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
