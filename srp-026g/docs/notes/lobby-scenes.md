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

## lobby_scene/hangar_046 (pack lobby-hangar-046@2)

- Source: 0.46c2, `projects/lobby_scene/day` of its 36 volumes. The owner asked for 0.40; 0.46c2 is
  the nearest build on disk with this lobby.
- Scene: an overgrown hangar (`level/lobby/hangar_lobby`, `hangar_wall`), flora of level/01 and
  level/03, props. 24 models imported (`ported/v046c2_lobby`), 10 are 0.26's own.
- Lights as 0.46c2 has them (0.26's units): 49 ambient lights, 11 lights, the donor's 4 probes
  with their own cubemaps, 3 wire sets. Environment: the classic lobby's sky and post-process,
  the donor's sun (elevation 55, azimuth 150).
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
  sparks and leaflets use 0.26's own), the portal system, the donor's colour grading.

## lobby_scene/camp_069 (pack lobby-camp-069@2)

- Source: 0.69d, `projects/lobby_scene/day` of volume 72 (the last copy), without its New Year
  snow: the 8 instances of `level/new_year_event/` and `generic/modular/terrain/snow/` (volume
  32's scene). Not the event versions of volumes 26 and 70.
- Scene: a room of a house (`building/private_house`) with the work table under the raincoat tent
  (`level/lobby/raincoat_tent`), the fridge (`props/kitchen/refrigerator_lobby`), the background
  (`level/02/bg_shop`, `level/04/bg_pipe_05`, houses, trees). 101 models imported
  (`ported/v069d_lobby`), 9 are 0.26's own.
- Lights: the 18 adding ambient lights (lux x 0.5, at most 3), 13 lights (lumen / 30, 0.2..3); the
  61 multiplying ones (0.69d type 1, painted contact shadows) are dropped. Its 6 probe volumes
  use the classic lobby's cubemap: 0.69d probes are encoded for its exposure and carry their
  irradiance as SH coefficients, with no `_diffuse` cubemap. Window panes (`water/glass_water`)
  use 0.26's `tiled/window/glass`.
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
  metalness and detail maps, exposure and colour grading.
