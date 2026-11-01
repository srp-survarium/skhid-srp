# Lobby packs: the STK2 bomb shelter

The lobby project resource-porter makes from S.T.A.L.K.E.R. 2's (2011-12) bomb shelter, pack
lobby-stk2-bunker (build-stk2-lobby-026, ports/stk2-bunker-lobby-to-026g.json): its source, the
manifest's choices and each version. What a lobby project must give, the lobby window and the
upgrade view are the lobby selection's (lobby-scenes.md, #778).

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

## STK2 bomb shelter (`lobby_scene/stk2_bunker`, pack lobby-stk2-bunker@10)

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
- The merged model is `level/stk2_bunker/main_v<version>` (`main_v10`), a new folder with each
  version: its surfaces are numbered by material, and a resources.db patch only adds files, so a
  surface folder of an earlier version left in the same model failed all its materials (seen with
  @6's files over @5's: "material not loaded: nomaterial", the shelter untextured).
- No shadows (@8, the owner: a hard pitch-black band down the inner face of the big arch, "a
  shadow of something"; "decrease intensity of light a bit"): traced from each shadowing light
  through the band, the caster is the wooden shelf's post by the gas-mask crates, 1.7 m from STK2's
  store-room fill `point_light(5)`, which stands right beside the shelf. STK2 lit with it as a soft
  bounce; 0.26 drew the post's shadow hard and black. No light of the bunker casts a shadow now;
  the main lights are 18 % lower (the store room's lamps 1.97), the ambient and the bulbs' glow kept.
- The vault (@9, the owner, 2026-11-01: "this is roof being a bit strange": over the pendant
  the vault darker in a blotch, its edge speckled): the shell's vaults are `test_object/p_brick`
  (plaster over brick), and over most of the store room's vault, 6 to 18 mm in front of it, STK2 laid
  `tiled/wall/stucco_alpha_01`, a dirty-plaster overlay: a darker, greener plaster on a 2048 px
  alpha mask that is a speckled oval (binary at full size, 61 % opaque), alpha-tested (STK2's
  `alpha_ref` 0.95, the 0.26g template's 0.25: either way the edge is the mask's speckle). Not a
  depth fight (1 cm at 3 m is far above the depth step) and not the lights: the blotch is the
  overlay's own texture and the speckle its mask. @9 left it out (the manifest's `materials`,
  hidden); the vault was the plaster of p_brick, as round the overlay. Other alpha instances
  whose own `alpha_ref` differs from the templates' 0.25 (`decals/listja` 0.6, the ventilation
  motor 0.5, `decal_for_floor` 0.15): out of the lobby's view, not carried.
- The vault's seams (@10, the owner's captures of @9: straight seams on the vault, plain when
  looking up): each octagonal bay is a cloister vault of eight `p_brick` panels, and STK2 mapped
  each panel on its own (rotated, 0.11 to 0.31 uv a metre: 91 uv seams, 70 m, round the pendant
  alone); its overlay, one uv tile over each whole bay, hid them. p_brick's tile is plaster on its
  top half and bricks below, so no one mapping of it covers a ceiling, and the overlay's own
  texture is the darker stain plaster (mean grey 0.41 against p_brick's plaster 0.53). Kept and
  opaque on the concrete template it made the ceiling darker, with a dark speckled band along
  its rim (tried, not shipped). @10 brings the overlay back as a plain ceiling over each bay:
  0.26's own tiled plaster, `tiled/stucco/stucco_01` (the manifest's `materials`, native), its
  uvs mapped anew from above at 0.4 uv a metre (`planar_uvs`, build-stk2-lobby-026: u along x, v
  along z; 97 % of the overlay faces within 45 degrees of down). One mapping over each bay's
  ceiling, so no seam in it; it meets p_brick at the bay's rim, along the vault's crease.
- The doorway up to the corridor (@9, the owner: "see this green thing there and also strangely
  stretched surface"; the store room's doorway with the steps, by the fuse boxes, lobby x 9.2):
  - the green: STK2's `nature/moss/moss` on the sill (0.27 x 0.83 m, 295 triangles), its
    `moss_m` a saturated moss photo with no alpha (a 24-bit TGA, DXT1) on the alpha-tested
    two-sided template, so a solid bright green strip. Left out (the slice's `exclude`);
  - the stretch: faces STK2 left unmapped in the shell, their corners at one uv or on one line
    (`(0, 1) (0, 1) (1, 1)`), so one row of texels runs across them: that sill's riser, a floor
    strip by the west arch (1.2 x 0.4 m, by the character), a sill in the east vault, the
    ventilation shaft's ceiling (93 triangles, 1.6 m2, all in `bomb_shelter.model`'s
    `crete_floor_clean` and `p_brick` but a sliver of the ventilation hatch). build-stk2-lobby-026
    now gives each such face corners of its own, projected along the axis it faces at its
    surface's median texel density, tangents along the projection;
  - the dark beyond it: no light in that corridor. Its plafond (0.26's fixture where STK2 hung a
    cage) glows but STK2 put no light there; the nearest were the store room's lamp 5.3 m away
    behind the wall and STK2's light under the corridor's pendant, 6.6 m along with a 5 m range.
    A dim warm light under the plafond (@9: `corridor_glow`, as the vault glows: 1.15, 4.5 m).
    Not a cut: the box keeps the corridor. From the store room the doorway looks 22 m down that
    corridor to its end wall, and @9's light, 3 m over the floor, still left it nearly black
    (the doorway's grey 23 of 255). @10: `corridor_glow` lower and stronger, deeper in
    (12.3, 2.7, 0.65; 2.0, 4 m: 4.3 m from the store room's floor at the wall, out of its
    reach), and STK2's light by the corridor's far plafond (`point_light(7)`) 1.6 (0.4), 6 m (5),
    which lights its end wall: the doorway's grey 43, the store room's frame unchanged (82.6).
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

Headless, the debug exe of feat/lobby-select choosing `stk2_blockpost` with each
project in its place (a local test patch), no input but the sign-in. On the machine
that made them: `/mnt/hdd/Projects/lobby-stk2/survey/captures/` (`cand-*` the
candidates, `pick-*` the packs).
