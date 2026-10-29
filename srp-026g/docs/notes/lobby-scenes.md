# Lobby scenes

What a lobby project must give and how the lobby is chosen: the selection's notes
(feat/lobby-select, patch 81). Each lobby has its section.

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
- For the selection (patch 81, patch 45 round 6): one `track` object (the classic's,
  unmoved), the root `camera` (where the lobby camera starts), and the root table
  `srp_lobby = { weapon = float3( 0.9, 1.55, 2.55 ), weapon_view = 0.0 }` (the weapon
  over the cabinet's counter, seen from the room along +z). The hut's STK2 collision
  mesh (walls, roof, the porch) is a collision object (cmask 20, cgroup 10): the camera's
  ray stops at its walls; the door stays open, so the camera leaves through it only when
  pulled straight back.
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
- Lighting as STK2 authored it (the owner, 2026-10-29), not the classic lobby's: the
  lobby_scene project's own lights (the ceiling's white sphere light, intensity 5.51,
  range 5; the campfire composite's warm sphere light, 0.98, range 7; the two outside
  the hut), their colours, ranges, attenuations and shadows as authored, and its
  environment's post-process (`default_post_process`) on the 0.26g environment: fog
  colour and distances, sun colour, no bloom, no colour grading, no grain, aberration,
  vignette, god rays or lens flares (STK2's has none). The project has no sun: the sun
  object is there with intensity 0. Two changes, the same for every STK2 lobby: every
  STK2 light at twice its intensity, and one dim warm fill in front of the character
  (`character_fill`, intensity 1, range 3.5). STK2 lit its rooms with light propagation
  volumes (bounce light) and a tonemap median exposure; 0.26g has neither, and the
  lights alone left the rooms and the character near black.
- No counterpart in 0.26g (in pack.json's notes): STK2's ambient and skylight colours,
  its tonemap median and thresholds, levels (frame lights and shadows), shadow
  transparency, bloom's maximum colour, depth of field. 0.26g's day sky stays behind the
  windows (STK2's `sky_default` is not ported). No environment probe (STK2 has none).
- Sounds: the lobby's `fire` (at the campfire), `lobby_background` and `lobby_room_tone`.
- Not ported: STK2's own particle effect `campfire_middle` (an editor-format source
  particle; 0.26g's are compiled in another schema): the native `fx/lights/bonfire`
  burns there instead, as the STALKER map ports do. STK2's terrain (four 64 m cells, all
  height 0, all `terrain/grass_dry`) is a flat 80 m grass plane with its texture. STK2's
  sky and post-process are not used (0.26g's environment_day). STK2's spec maps are not
  shipped where the template does not read them.
- Seen (2026-10-29, headless, no input but the sign-in): the lobby with the project in
  place of `lobby_scene/day` (a local test patch), and with feat/lobby-select's exe (a
  debug build choosing `stk2_blockpost`, the pack as a local patch): patch 81 took the
  project (its track and config there), the hut shown. Not seen: other camera angles
  (no input runs), the weapon in its upgrade window, the choice from user.cfg.
