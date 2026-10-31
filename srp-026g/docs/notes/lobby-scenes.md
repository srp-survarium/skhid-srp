# Lobby scenes

What a lobby project must give and how the lobby is chosen: the selection's notes
(feat/lobby-select, patch 81). Each lobby has its section.

## STK2 lighting (the bomb shelter; the blockpost as at @2)

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
  textures at most 512 px (props 256). Weapons on tables and STK2's static characters
  are left out.

## STK2 Bar (dropped)

The owner dropped the Bar lobby (2026-10-30: one lobby per faction, the STK2 bomb shelter
the Bandits'). Its manifest stays in resource-porter
(`ports/stk2-bar-lobby-to-026g.json`, relit and camera-fixed as @3), not shipped.

## STK2 bomb shelter (`lobby_scene/stk2_bunker`, pack lobby-stk2-bunker@5)

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
- The camera: boxes over the floor and around the props the eye got into (@2), and one
  across the front arch to the next vault (@3: at yaw 0, 4.6 m the eye went through it
  into the dark).
- Beyond the arches: the other rooms' lamps as warm as the store room's, and a dim warm
  light in the vault beyond the west arch and in the corridor beyond the east one (they
  showed black).
- Not ported: two decal materials without a diffuse (`decal_dirt_006`,
  `moss_012_alpha`): their surfaces are left out; the outdoor trees (speedtree) and the
  power line outside.

## Captures

Headless, the debug exe of feat/lobby-select choosing `stk2_blockpost` with each
project in its place (a local test patch), no input but the sign-in. On the machine
that made them: `/mnt/hdd/Projects/lobby-stk2/survey/captures/` (`cand-*` the
candidates, `pick-*` the packs).
