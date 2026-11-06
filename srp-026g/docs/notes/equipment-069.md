# Armour from 0.69d (pack equipment-069)

0.69d-only armour (`scripts/equipment-069`):

- `equipment-069@1`: the two backpacks that equipment-046 left out (the owner,
  2026-10-30), the Medic's bag (`leagues_back_2`) and the Soldier's backpack
  (`leagues_back_5`).
- `equipment-069@2` adds the owner's pick of 2026-11-05: the Snow-20 vest, pants,
  gloves, boots and helmet, and the Delta mask (below, "Snow-20 and the Delta
  mask"); `@3` (2026-11-06, the owner after seeing it in game) makes the Delta
  mask a light, aim-speed mask; `@4` draws metal without diffuse (the Snow-20
  vest's plates were white; below, "Metal") and copies the Sniper set of
  equipment-046@8 (level 12).

Every number here is a proposal. Balance is the owner's call.

## What ships (the backpacks)

- Models, materials and textures come from resource-porter's
  `vostok-port build-armour-069d` (manifest
  `ports/survarium-069d-armour-to-026g.json`). The pack's scripts write the
  items around them. Everything stays at the donor's own paths
  (`.../special_items/league_regeneration_back/`, `.../league_battle_back/`),
  and the released game has none of those paths.
- Surfaces: LOD0 only, in 0.26g's four-bone layout. Both bags are skinned to
  one spine bone (Spin_1, Spin). That bone binds where the native rig binds
  it, so no vertex moves.
- Model folder: the bind pose, skeleton, settings and properties come from a
  native back piece: the Antiradar Veil for the bag, the Assault fighter's
  backpack for the rucksack. The bounds are stored as native models store
  them (`-x, y, -z`).
- Materials: a native back material (the same two pieces) with the donor's
  albedo, normal, smoothness and metalness maps:
  - smoothness goes into the gloss channel, at 0.07..0.27;
  - metalness goes into the Fresnel map, at 0.11..0.59;
  - the bag keeps its fuzziness.
  - Both are switch combinations a released character draws.
  - The ranges come from seven backpacks that both builds ship: they are the
    medians of per-texel fits of the native maps against 0.69d's.
    0.69d's albedo is as bright as 0.26g's diffuse where it is not metal.
  - Metal (since @4): 0.69d draws a metallic texel's albedo as the colour of
    its reflection and gives it no diffuse light, so the diffuse map is
    albedo x (1 - metalness) (ImageMagick composites it; the albedo's alpha
    left out). Before @4 the albedo was the diffuse as it is, and metal drew as
    bright as its albedo: the Snow-20 vest's chest plates white where 0.69d and
    its icon show them dark. The bags' buckles and the rucksack's axe blade
    darken the same way (`private/renders/equipment-069-snow-delta/`).
- Textures: 2048 down to the native 1024, using the maps' own mips. The bag's
  DXT5 albedo becomes DXT1 with the same colours (0.26g's character colour
  maps carry no alpha, and its material uses none). Each map gets the native
  options of its role.
- Fit: the builder measures each bag against every stock torso that leaves
  the back slot free (52 torso surfaces). The exoskeletons take the slot.
  - The measure is the share of the bag's vertices that lie more than 5 mm
    inside a torso, within 5 cm of its surface. The same is measured for the
    18 native back pieces (oxygen cylinders included).
  - A piece may sink into no torso deeper than the deepest native piece does.
  - Soldier's backpack: at most 5.2 % (the tank jacket, where natives reach up
    to 48.5 %). It is at or below the native median on 50 of 52 surfaces.
  - Medic's bag: at most 19.2 % (the tank jacket again). It is above the
    native median on 30 surfaces and below the native maximum on all of them.
  - All of this is measured in the bind pose, as for the native pieces.
- Ids: 0.69d's own dictionary ids, 1357 and 1359. Neither is taken by the
  stock db, a released patch or any pack (equipment-046 included).
- Icons: the 0.69d item icons (`equipment_13_1`, `equipment_13_3`), fitted
  into atlas cells as equipment-046's are. They go in the free
  equipment-atlas column 8, rows 14 and 15 (equipment-046 fills rows 1-13).
- Texts: `st_srp_equipment069_<0.69d name>_name/_desc` in the ten catalog
  TSVs:
  - English, German, Polish, Russian and Ukrainian are 0.69d's own names, with
    English cased as the stock "Stalker's backpack";
  - Serbian, Czech, Hungarian, Finnish and Belarusian are translated in the
    stock wording;
  - 0.69d has no descriptions, so each language gets its "description not
    available yet".
- Config: the template piece's config (parameters, defect and modification
  masks, repair, the rustle sound 0.69d uses too), with this piece's model,
  level, faction, weight, drop weight, bonuses, card (weight), icon and texts.

## The backpacks

- Level: the shared models' median. A 0.69d level-2 piece is a level-5 piece
  in 0.26g, and a level-5 piece is level 14. This is the median 0.26g level of
  the 0.69d armour that uses the same model (0.69d levels 1..5 ->
  3, 5, 8, 12, 14).
- Faction: 0.69d sells both as "Leagues" items with no faction. Each fills a
  gap. The Black Market has no back piece below level 9, and the Scavengers
  have none at level 14 (only the oxygen cylinder).
- Built-in bonuses: 0.69d's own, where 0.26g has the modification.
  - 0.69d's pocket depth becomes 0.26g's pocket size.
  - Dropped: medkit use time -10 % (0.26g has no such modification) and
    reputation +1 % (0.26g's reputation bonuses are the premium pieces'
    +5 %).
  - None of these bonuses rolls, so the second-strongest rule does not apply.
- **Medic's bag** (1357): Black Market, level 5, Black Market rank 5.
  - 4.5 kg, 10,830, drop weight 110.
  - 1 pocket + pocket size +1 (1030, 1170).
  - There is no native level-5 back piece. The weight and price sit between the
    level-3 and level-6 back pieces' medians (5 kg / 6,650 and 4.25 kg /
    12,925), as equipment-046 priced its level-4 gloves.
  - Peer: the "Marodeur" container (level 3, the same bonuses, 5 kg).
- **Soldier's backpack** (1359): Scavengers, level 14, Scavenger rank 5.
  - 8 kg, 42,750 (the level-14 back median), drop weight 20.
  - 2 ammo bags + 1 pocket + pocket size +2 (1051, 1030, 1171).
  - It carries the most of any level-14 back, so it weighs what the Assault
    fighter's backpack weighs (8 kg, 2 pockets + size +2; also 0.69d's own
    8 kg).
  - Peers at level 14: the Assault fighter's backpack, the "Wanderer"
    container (4 kg, artefact + size) and the Antiradar Veil (4 kg, pocket
    + respawn -20 %). None is strictly better or worse than it.
- `balance_audit.py`: 3 comparisons and no strict dominance. The Medic's bag has
  no level-5 peer.

## Needs a look in game (the backpacks)

- The fit from every side: the bag low on the back, the rucksack over the
  hips. The measure says they sink into torsos no more than native pieces.
- The materials: gloss and Fresnel come from fitted ranges, not from 0.69d's
  lighting. The bag's fuzz should be checked too.
- The two icons in the lobby.

## Not here

- `blackmarket_helmet_3_summer17` (Summer helmet): the builder converts its
  surfaces, material and textures the same way. A helmet needs a fit of its
  own (over the native heads and the masks), and the builder has only the back
  fit. Not added.

## Owner's decisions (2026-10-30)

- Factions as proposed: the Medic's bag with the Black Market, the Soldier's
  backpack with the Scavengers.
- The Medic's bag stays as 0.69d shipped it, without straps. Its mesh has
  none: one spine-weighted box on the back, nothing over the shoulders, and
  0.69d adds none when a back piece is worn. Straps borrowed from the
  Backpack with container were proposed and turned down.

## Open questions for the owner

- The levels, weights, prices and drop weights above.

## Snow-20 and the Delta mask (equipment-069@2 to @4)

The owner's pick of 2026-11-05 ("make sure they fit properly and are well
balanced with their quirks"): the Snow-20 vest, pants, gloves, boots and helmet
(`snow_20_*_7`) and the Delta mask (`league_shadow_mask`). 0.69d's ids (2055-2059,
1919) are taken by Skhid's scopes, so they take 1878-1883.

### Snow-20: the Sniper set in snow

- 0.69d's Snow-20 is the Sniper set (`renesanse_sniper_07_*`) recoloured: the
  same armour, isolation, weight, slowdown and stamina, but where the Sniper
  pieces have a recoil built-in (127, -2.8 %), the Snow-20 ones have an isolation
  one (139). Every Snow-20 surface has 0.69d's Sniper vertices and triangles
  exactly; only the materials differ (`_newyears_2020`).
- Skhid ships the Sniper set from 0.31a2 (equipment-046). Its 15 surfaces have
  the same vertex positions and index lists as the Snow-20's, bit for bit, in
  the 0.26g layout (offline comparison, 2026-11-05).
- Models: the porter converts the snow surfaces (`same_as` pieces of the
  manifest), checking each against 0.69d's Sniper model of the same path as
  equipment-046's; `check_pack.py` checks every built surface against the
  shipped Sniper surface it copies (positions and indices). The template models
  are stock Renaissance ones of the slot (the combat torso and legs at their
  three lengths, the tank's hands, the scout's boots, the tank's helmet: the
  combat helmet carries headwear-1's fly-off bone, 70 bones to 0.69d's 69).
- Materials: 0.69d's snow maps are its physically based ones (smoothness,
  metalness), not the 0.31a2 Sniper's, so they go through the same conversion
  as the backpacks' (the gloss and Fresnel ranges above). The vest's and pants'
  ghillie keeps 0.69d's two-sidedness (the builder now carries a donor's
  `two_sided`); its template is the combat torso's material. 0.26g has no
  character material with an alpha test, both sides and all four maps whose
  textures ship (the Stalker backpack's are placeholders), and the live Sniper
  ghillie draws without an alpha test too (0.69d's own snow ghillie has no
  clear texels; it is dark, mean 24/255).
- The vest's plates are metal: since @4 they draw dark as in 0.69d (backpacks'
  "Metal" above). The three lengths of the vest and of the pants draw the same
  two materials each (torso or legs, and the ghillie).
- The pants end where the Sniper pants do (the same mesh): lowest vertex at
  8.4-8.9 cm, the native combat pants' at 10.7-11.0 cm; the Snow-20 boots (and
  the Sniper's) pick the medium length, which goes into the boot (its top at
  27.5 cm). Nothing cut.
- Config: the equipment-046 Sniper piece's whole (`template`
  `equipment-046:<id>`, equipment-046@8: parameters, protection, regions,
  models' parts, sounds, card, price, level 12, Renaissance Army, drop weight
  40), drawn with the snow
  models, with the row's built-ins: the piece's own built-in bonus becomes the
  winter isolation, +20 % on every region (1023, the second-strongest of its
  kind, as built-in-bonuses-1 sets them). The live Sniper pieces carry 0.31a2's
  bonuses, not 0.69d's recoil one, so the Snow-20 piece swaps its Sniper twin's
  one bonus (vest: a pocket; pants: pocket size; gloves: aim time -8 %; boots:
  energy costs -8 %) for the isolation. The Sniper helmet has no built-in
  bonus, so the Snow-20 helmet has none either: an isolation bonus added to it
  would make it strictly better than the Sniper helmet.
- The numbers (equipment-046@8's Sniper set, the owner, 2026-11-06): vest
  75 / 5, 6 kg, 73,500; pants 75 / 5, 1.5 kg, 36,750; helmet 55 / 5, 1 kg,
  45,500; gloves 75 / 2, 0.2 kg, 18,380; boots 75 / 2, 0.5 kg, 18,380.
- `balance_audit.py` compares each Snow-20 piece with the level-12 pieces of
  the same kind, regions and slots, its Sniper twin among them: none is
  strictly better or worse; the helmets are equal.

### The Delta mask

- 0.69d's Delta mask (league item, no faction): light armour, face armour 45,
  isolation 2, 0.5 kg, a built-in isolation bonus, tier 5. 0.69d's tier 5 is
  Skhid's level 14 (the backpacks' level mapping above).
- Level 14, no faction (sold by everyone at no rank, as the STK2 items), a
  light mask that speeds aiming (the owner, 2026-11-06, after seeing @2 in
  game: weak protection, an aiming bonus in place of the isolation one):
  armour 15, isolation 5, 0.5 kg, aim time -8 % (1083, the second-strongest of
  its kind; the aim bonus the Sniper gloves carry), 23,630 (the level-14
  masks' median), drop weight 20 (theirs). The config starts from the level-14
  Scavenger mask (`scav_mask_9`, 941): its parameters, face region and card,
  the row's numbers. (@2 had armour 45, isolation 15 and mask isolation +20,
  1183.)
- Against the three live level-14 masks (ren_mask_5 35 / 70, 1.2 kg;
  scav_mask_9 30 / 70, 0.8 kg; edge_mask_5 50 / 25, 2 kg): the least armour
  and isolation of the four, the lightest, and the only one with a built-in
  bonus. None is strictly better or worse (`balance_audit.py`).
- Fit: resource-porter's mask fitting of the face-wear masks
  (`face_items_069d::fit_surface`, the `mask` pieces of the manifest): over the
  native head (no `remove_head_model`: a harness and a face guard, not a closed
  shell), the neck tucked into the collars, then the relief and the gate over
  the game's 19 clips: least gap over the head 0.79 mm (cheeks and jaw 3.33 mm).
  It is fitted over the bare head (`covers_helmet` in the manifest: its night
  vision mount is rigid; pulling it under the helmets also fails today: the
  shell builder refuses the helmets headwear-1 re-rigged).
- Slots: the mask slot only, as every live mask (the owner's T28 rule for the
  Protective mask): a helmet goes on over it. Under most helmets the face guard
  and straps stay hidden; the night-vision mount on the brow shows in front of
  the brim of some (pictures below).

### Pictures (offline, bind pose)

`private/renders/equipment-069-snow-delta/` (README.txt there): the Delta mask
on the three native heads and under eight live helmets, and a close-up of its
mount under the helmets; the Sniper set (before) and the Snow-20 (after); the
Snow-20 with a backpack and masks. Since @4 also: 0.69d's own look (its icons,
its model drawn with its maps) beside ours @3 | @4; the vest on its three
lengths; the set; the pants' ends with and without boots beside native pants;
the backpacks' and the Delta mask's metal @3 | @4.

### Decisions (the owner, 2026-11-05)

- The Delta mask keeps its night-vision mount, and it may show in front of
  the brim of scavengers_05, edge_assault_05, scavenger_scout_07 and
  edge_researcher_03; the mask stays in the mask slot only (a helmet goes on
  over it).
- 2026-11-06, after seeing it in game: the Delta mask's isolation bonus goes;
  its protection is low and it speeds aiming instead (above).

### Needs a look in game

- The snow materials (gloss and Fresnel from the fitted ranges), the dark
  ghillie (0.69d's own albedo is dark, mean 24/255; drawn without an alpha
  test like the live Sniper's), the metal now without diffuse (the vest's
  plates, the bags' buckles: their shine is the Fresnel map's).
- The six icons in the lobby (0.69d's, cells column 8 rows 20-22, column 9
  rows 14-16).

### Open questions for the owner

- The Snow-20's bonus swap (each piece loses its Sniper twin's bonus for the
  isolation; the helmet without one), and the Delta mask's level, faction,
  exact numbers and price above.
