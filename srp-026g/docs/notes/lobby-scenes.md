# Lobby scenes

What a lobby project must give and how the lobby is chosen: the selection's notes
(feat/lobby-select, patch 81). Each lobby has its section.

## STK2 lighting (the Bar, the bomb shelter; the blockpost alike)

- As STK2 authored it (the owner, 2026-10-29), not the classic lobby's: the project's own
  lights (point, sphere and box lights as points; spots as spots with their cones and
  directions; the sun with its colour, intensity, shadow and influences), and its
  environment's post-process on the 0.26g environment object: fog colour and distances,
  sun colour, bloom, adaptation speed, no colour grading, no grain, aberration, vignette,
  god rays or lens flares (STK2's post-process has none). No environment probe.
- Two changes, the same for every STK2 lobby: every STK2 light at twice its intensity,
  and one dim warm fill in front of the character (`character_fill`, intensity 1, range
  3.5). STK2 lit its rooms with light propagation volumes (its lamps' `use_with_lpv`) and
  a tonemap median exposure; 0.26g has neither, and the lights alone left the rooms dark
  and the character a silhouette (captures below, `cand-*` of the first survey runs).
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

## STK2 Bar (`lobby_scene/stk2_bar`, pack lobby-stk2-bar@1)

The Bar of STK2's `bar_shooting_gallery` project (of its six variants the one with the
most objects, 463 KB; `_mir`, `_mir2` and the three `_light_test` copies hold fewer): the
gallery on its lower level, with the shooting lanes, paper and colour targets, the
camouflage nets, the bottle shelf at the lanes' end and the lamps.

- Spot: the character on the gallery's floor tiles (STK2 (97.0, -3.17, 0.6)), the
  lanes and targets behind him. Chosen of four (the owner may pick another): the bar
  counter upstairs, the trophy shelf, the gallery, the lounge; the gallery has the most
  detail and light on screen.
- Kept: the lower level's box, x 77..102.5, y -3.6..0.3, z -7..6 (STK2 world): 287 of
  the project's 544 visuals, the 7 lights inside the box (its own and its lamps'), the sun (a weak blue,
  unshadowed: STK2's fill for the whole room). 25 MB.
- Spots (lobby world; data `srp-026g/scripts/lobby-scenes/stk2_bar.tsv`):

      character          ( 0.000, 0.000,  0.000)
      camera, default    ( 0.000, 1.200, -1.960)  yaw 0
      weapon             ( 0.100, 1.320,  2.000)  weapon_view 0 (over the lane)
      artefact           (-0.900, 1.250,  4.200)  on the bottle shelf

- Not ported: 16 models the archives have no export of (barrels, kegs, buckets, cement
  bags, broken bottles); the light cone effect (`fx/conus_grad`, a volumetric cone with
  no 0.26g counterpart); the textures `forge/vents` and `inbox/diana/ventilation`
  (missing: their surfaces are left out).

## STK2 bomb shelter (`lobby_scene/stk2_bunker`, pack lobby-stk2-bunker@1)

The shelter of STK2's dressed level `scenario/a03_bomb_shelter`, not the bare
`bomb_shelter_test` project (its shell only, no props): the octagonal store room with
the gas-mask pile (`bomb_shelter_gasmask_garbage`), shelves, ammunition and gas-mask
crates, beds and lockers, and the vaults beside it.

- Spot: where STK2's designers stood a novice (STK2 (37.49, -4.37, -24.41)), facing the
  room's west half. Chosen of four: the gas-mask room, the sleeping quarters, the store
  room, the crates by its door; the store room is the most dressed and the best lit.
- Kept: x 28..48, y -5..0.5, z -31..-8 (the store room, the gas-mask room, the quarters
  and the vault beside them): 241 of 348 visuals, 5 lights. a03's sun (shadowed, it
  lights the level's outside) is left out: the sun object is there with intensity 0.
  26 MB.
- Spots (data `srp-026g/scripts/lobby-scenes/stk2_bunker.tsv`):

      character          ( 0.000, 0.000,  0.000)
      camera, default    ( 0.000, 1.200, -1.960)  yaw 0
      weapon             (-1.990, 1.370, -1.510)  weapon_view 1.5708 (over a metal box)
      artefact           ( 1.460, 0.610, -1.100)  on a gas-mask crate

- Not ported: two decal materials without a diffuse (`decal_dirt_006`,
  `moss_012_alpha`): their surfaces are left out; the outdoor trees (speedtree) and the
  power line outside.

## Captures

Headless, the debug exe of feat/lobby-select choosing `stk2_blockpost` with each
project in its place (a local test patch), no input but the sign-in. On the machine
that made them: `/mnt/hdd/Projects/lobby-stk2/survey/captures/` (`cand-*` the
candidates, `pick-*` the packs).
