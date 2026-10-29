# Lobby scenes

The lobby menu loads the project `lobby_scene/day` (client_patch.md, the lobby menu's
resources query, section 3.42). The other lobbies are projects of their own beside it, for the
players' choice of lobby. Each has a section here; the selection (exe and lobby movie) is its
own work.

What a lobby project gives the 0.26g lobby, and what it does not:

- The exe puts the lobby character at the world origin facing -z (yaw pi), the squad
  panels' characters at (1, 0, 0.5) and (-1, 0, 0.5). The camera (lobby_camera) looks at
  (0, 1.2, 0) from yaw 0, pitch 0, distance 1.96 (the wheel: 1.5..4.6; pitch within +-60
  degrees), the eye shortened by a ray against the project's collision. None of that is in
  the project: a scene is placed around the origin.
- From the project: the render visuals, the collision objects (the camera's ray hits
  cmask 20, cgroup 10, as `level/lobby/base.model/collision`), the client objects (lights,
  ambient lights, the environment, its probe, particles, sounds).
- The workbench of patch 45 (BENCH, section 3.42) is a constant of the exe, made for the
  classic room; each scene below gives its own spot for it and for an artefact.

## STK2 blockpost (`lobby_scene/stk2_blockpost`, pack lobby-stk2@1)

The cancelled S.T.A.L.K.E.R. 2's own lobby (2011-12, `lobby_scene/project.xprj` of its
`xr2-prog` tree): a small guard hut (STK2 `location/a02_blockpost/blockpost`) with a
campfire on the floor, a sentry cabinet (`props/furniture/sentry_a`), a mattress and a
stool, on a flat grass terrain.

- Made by resource-porter's `build-stk2-lobby-026` (`ports/stk2-lobby-to-026g.json`,
  branch feat/lobby-stk2-026) from the STK2 archive's exported models, source settings,
  material instances and TGA textures; shipped as the content pack lobby-stk2@1
  (release/content.tsv). New paths only: `projects/lobby_scene/stk2_blockpost/client_project`
  (and `.lua`), `models/level/stk2_lobby/*`, `material_instances/ported/stk2/lobby/*`,
  `textures/ported/stk2/lobby/*` (135 files, 29 MB; textures at most 1024 px).
  `lobby_scene/day` is not changed.
- The STK2 scene is baked into the lobby's frame (meshes and collision moved, the
  visuals at the identity): the character's spot on the hut floor at the origin, the
  hut's axis from its door to the far wall (STK2 +x) along +z.
- The default view: the camera stands just inside the door, the character in front of
  it, the campfire to his left, the mattress on the floor beyond it, the cabinet behind
  him on the right, a window in the right wall, the corrugated roof and its beams above.
- Spots (lobby world, metres; data:
  `srp-026g/scripts/lobby-scenes/stk2_blockpost.tsv`):

      character          ( 0.000, 0.000,  0.000)  facing -z (the exe's)
      camera, default    ( 0.000, 1.200, -1.960)  yaw 0
      weapon (BENCH)     ( 0.900, 1.550,  2.550)  camera yaw 0 (from the room, along +z)
      artefact (stool)   (-0.468, 0.500,  2.310)  camera yaw 0
      artefact (counter) ( 1.500, 1.200,  2.800)  camera yaw 0
      campfire           (-0.698, 0.000,  0.740)
      room               x -3.21..2.11, z -2.05..3.27, floor 0, ceiling about 2.56
      door               x -1.21..0.10 at z -2.38..-2.05 (behind the camera)

- The weapon spot is 0.35 m above the cabinet's counter (top y 1.20, x 0.12..1.67,
  z 2.43..3.19; its back panel at z 3.21, up to y 2.5): a rifle's 0.95 m along x and its
  spin stay over the counter. Patch 45's camera there (distance 1.2, pitch -0.25) stands
  at about (0.90, 1.85, 1.39), in the room. The classic BENCH looks along +x (yaw -pi/2);
  here the view is along +z.
- Materials: native 0.26g templates with the STK2 textures bound: opaque walls, floor,
  wood and fabric on `tiled/concrete/concrete_01`; alpha-tested decals on
  `tiled/trash/trash_concrete_alpha`; the asbestos roof and its leaves (two-sided) on
  `tiled/roof/roof_03_alpha_2side`; the campfire logs (emissive embers) on
  `props/light/bonefire_fire`. The window panes use the native
  `tiled/window/glass_tile_forward` as it is. Only the slots a template reads are
  filled (diffuse, normal; specular where the template uses it).
- Lights and sounds: the classic lobby's sky, sun, environment probe and its four lights
  about the character (`point_light` lowered under the hut's ceiling to y 2.3,
  `point_light(2)`, `point_light(4)`, `point_light(5)`, unmoved: the character is lit as
  in the classic room), a warm point light over the fire (range 6 m, intensity 3), a cool
  fill under the ceiling (STK2's sphere_light spot), two warm ambient fills, and the
  lobby's `fire`, `lobby_background` and `lobby_room_tone` sounds.
- Not ported: STK2's own particle effect `campfire_middle` (an editor-format source
  particle; 0.26g's are compiled in another schema): the native `fx/lights/bonfire`
  burns there instead, as the STALKER map ports do. STK2's terrain (four 64 m cells, all
  height 0, all `terrain/grass_dry`) is a flat 80 m grass plane with its texture. STK2's
  sky and post-process are not used (0.26g's environment_day). STK2's spec maps are not
  shipped where the template does not read them.
- Seen: a headless capture of the lobby with the project in place of `lobby_scene/day`
  (a local test patch, not shipped), 2026-10-29. Not seen: other camera angles (no input
  runs), the weapon and artefact displays (they need the selection's exe).
