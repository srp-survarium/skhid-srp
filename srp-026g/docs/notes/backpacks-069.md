# Backpacks from 0.69d (pack equipment-069)

The two 0.69d-only backpacks that equipment-046 left out (the owner, 2026-10-30):
the Medic's bag (`leagues_back_2`) and the Soldier's backpack (`leagues_back_5`).
They ship in the content pack `equipment-069@1` (`scripts/equipment-069`).
Every number here is a proposal. Balance is the owner's call.

## What ships

- Models, materials and textures come from resource-porter's
  `vostok-port build-armour-069d` (manifest
  `ports/survarium-069d-backpacks-to-026g.json`). The pack's scripts write the
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
    0.69d's albedo is as bright as 0.26g's diffuse, so it is used as it is.
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

## The pieces

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

## Needs a look in game

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
